//! Crawl engine: breadth-first frontier, HTTP transport, robots.txt and extraction.
//!
//! Scope today: one seed, bounded depth/URL count, bounded concurrency, robots.txt, manual
//! redirect chains, HTML extraction (title, meta, robots, canonical, H1, word count, links) and
//! durable rows. Authentication, rendering and the full extraction model are tracked in E02/E03/E04.

pub mod extract;
pub mod robots;
pub mod seeds;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use coqui_core::{
    CrawlCheckpoint, CrawlConfig, CrawlProgress, NewUrl, UrlError, now_ms, resolve_url,
};
use coqui_render::{RenderOptions, RenderPool};
use coqui_store::{Store, StoreError};
use futures::{StreamExt, stream};
use regex::Regex;
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, LOCATION};
use robots::RobotsRules;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use url::Url;

const MAX_BODY_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ROBOTS_BYTES: u64 = 512 * 1024;
const MAX_REDIRECTS: usize = 10;
const MAX_DISCOVERED_MULTIPLIER: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum CrawlError {
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("http client error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("crawl cancelled")]
    Cancelled,
    #[error("seed error: {0}")]
    Seeds(String),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("site blocked the crawler ({0} challenge responses)")]
    Blocked(u64),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CrawlStats {
    pub crawled: u64,
    pub discovered: u64,
    pub failed: u64,
    pub redirected: u64,
    pub blocked_by_robots: u64,
    pub blocked: u64,
}

struct FetchOutcome {
    status: Option<u16>,
    final_url: String,
    chain: Vec<String>,
    content_type: Option<String>,
    body: Option<String>,
    size_bytes: u64,
    elapsed_ms: u64,
    error: Option<String>,
    blocked: bool,
    console: Vec<String>,
    screenshot: Option<Vec<u8>>,
    axe: Vec<coqui_core::AxeViolation>,
    vitals: Option<coqui_core::WebVitals>,
    headers: Vec<(String, String)>,
    custom_js: Vec<serde_json::Value>,
}

/// Run a crawl to completion (or cancellation).
///
/// The caller owns crawl lifecycle status transitions around this function; it does not mark the
/// crawl as running or completed, so the same engine can serve the Tauri shell, CLI and tests.
#[derive(Debug)]
pub struct CrawlResult {
    pub stats: CrawlStats,
    pub paused: bool,
}

/// Run a crawl to completion, pause or cancellation.
///
/// The caller owns crawl lifecycle status transitions around this function; it does not mark the
/// crawl as running or completed, so the same engine can serve the Tauri shell, CLI and tests.
///
/// A checkpoint is written after every frontier batch so a crash or explicit pause can resume
/// without refetching completed URLs.
pub async fn run_crawl(
    store: Arc<Store>,
    crawl_id: String,
    seed_kind: String,
    seed_value: String,
    config: CrawlConfig,
    cancel: CancellationToken,
    pause: Arc<AtomicBool>,
    checkpoint: Option<CrawlCheckpoint>,
    on_progress: Arc<dyn Fn(CrawlProgress) + Send + Sync>,
) -> Result<CrawlResult, CrawlError> {
    let extractors = store.list_extractors(&crawl_id).unwrap_or_default();
    let mut headers = HeaderMap::new();
    for line in &config.custom_headers {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = HeaderName::from_bytes(name.trim().as_bytes())
            .map_err(|error| CrawlError::Config(format!("invalid header name: {error}")))?;
        let value = HeaderValue::from_str(value.trim())
            .map_err(|error| CrawlError::Config(format!("invalid header value: {error}")))?;
        headers.insert(name, value);
    }
    let builder = Client::builder()
        .user_agent(config.user_agent.clone())
        .timeout(Duration::from_millis(config.request_timeout_ms))
        .redirect(reqwest::redirect::Policy::none());
    if let Some(user) = &config.auth_user {
        let token = STANDARD.encode(format!(
            "{}:{}",
            user,
            config.auth_password.clone().unwrap_or_default()
        ));
        let value = HeaderValue::from_str(&format!("Basic {token}"))
            .map_err(|error| CrawlError::Config(format!("invalid basic auth value: {error}")))?;
        headers.insert(AUTHORIZATION, value);
    }
    let client = builder.default_headers(headers).build()?;

    let includes = compile_patterns(&config.include_patterns)?;
    let excludes = compile_patterns(&config.exclude_patterns)?;

    let resolved_seeds = if checkpoint.is_none() {
        seeds::resolve(&client, &seed_kind, &seed_value)
            .await
            .map_err(|error| CrawlError::Seeds(error.to_string()))?
    } else {
        Vec::new()
    };

    let debug = std::env::var_os("COQUI_DEBUG_CRAWL").is_some();
    if debug {
        eprintln!("crawl: resolved {} seeds", resolved_seeds.len());
    }

    let robots_base = resolved_seeds
        .first()
        .cloned()
        .or_else(|| (seed_kind != "list").then(|| seed_value.clone()));
    let robots = if config.respect_robots {
        match robots_base {
            Some(base) => fetch_robots(&client, &base, &config.user_agent).await,
            None => RobotsRules::allow_all(),
        }
    } else {
        RobotsRules::allow_all()
    };

    let (mut frontier, mut seen, mut depth, mut stats, primary_hosts) = match checkpoint {
        Some(checkpoint) => {
            let seen: HashSet<String> = checkpoint.seen.into_iter().collect();
            let hosts: HashSet<String> = if checkpoint.hosts.is_empty() {
                seen.iter().filter_map(|url| seeds::host_of(url)).collect()
            } else {
                checkpoint.hosts.into_iter().collect()
            };
            (
                checkpoint.frontier,
                seen,
                checkpoint.depth,
                CrawlStats {
                    crawled: checkpoint.crawled,
                    discovered: checkpoint.discovered,
                    failed: checkpoint.failed,
                    redirected: checkpoint.redirected,
                    blocked_by_robots: checkpoint.blocked_by_robots,
                    blocked: checkpoint.blocked,
                },
                hosts,
            )
        }
        None => {
            if resolved_seeds.is_empty() {
                return Err(CrawlError::Seeds("no crawlable seeds".into()));
            }
            let seen: HashSet<String> = resolved_seeds.iter().cloned().collect();
            let hosts: HashSet<String> = resolved_seeds
                .iter()
                .filter_map(|url| seeds::host_of(url))
                .collect();
            (resolved_seeds, seen, 0u32, CrawlStats::default(), hosts)
        }
    };

    let in_scope = |url: &str| -> bool {
        if !includes.is_empty() && !includes.iter().any(|pattern| pattern.is_match(url)) {
            return false;
        }
        !excludes.iter().any(|pattern| pattern.is_match(url))
    };
    let in_primary_hosts = |url: &str| -> bool {
        seeds::host_of(url)
            .map(|host| primary_hosts.contains(&host))
            .unwrap_or(false)
    };

    if debug {
        eprintln!(
            "crawl: start frontier={} seen={} depth={} stats={:?}",
            frontier.len(),
            seen.len(),
            depth,
            stats
        );
    }

    let max_discovered = (config.max_urls as usize)
        .saturating_mul(MAX_DISCOVERED_MULTIPLIER)
        .max(1);

    if config.max_urls == 0 {
        let result = CrawlResult {
            stats,
            paused: false,
        };
        notify_webhook(&config, &crawl_id, &result).await;
        return Ok(result);
    }

    let renderer = if config.render {
        let mut headers = Vec::new();
        let mut cookies = Vec::new();
        for line in &config.custom_headers {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim().to_string();
            let value = value.trim().to_string();
            if name.eq_ignore_ascii_case("cookie") {
                for pair in value.split(';') {
                    if let Some((cookie, cookie_value)) = pair.trim().split_once('=') {
                        cookies.push((cookie.trim().to_string(), cookie_value.trim().to_string()));
                    }
                }
            } else {
                headers.push((name, value));
            }
        }
        if let (Some(user), password) = (&config.auth_user, &config.auth_password) {
            let token = STANDARD.encode(format!("{user}:{}", password.clone().unwrap_or_default()));
            headers.push(("Authorization".into(), format!("Basic {token}")));
        }
        let options = RenderOptions {
            user_agent: Some(config.user_agent.clone()),
            headers,
            cookies,
            screenshot: config.screenshots,
            block_assets: config.render_block_assets,
            wait_dom: config.render_early,
            block_patterns: config.render_block_patterns.clone(),
            accessibility: config.accessibility,
            custom_js: config.custom_js.clone(),
            web_vitals: config.web_vitals,
        };
        Some(
            RenderPool::connect(
                config.render_workers as usize,
                config.browser_path.as_deref(),
                config.render_wait_ms,
                options,
            )
            .await
            .map_err(|error| CrawlError::Config(format!("renderer unavailable: {error}")))?,
        )
    } else {
        None
    };
    let concurrency = usize::from(config.concurrency.max(1));
    let chunk_size = (concurrency * 4).clamp(8, 400);
    let mut last_checkpoint = Instant::now();

    while !frontier.is_empty() && depth <= config.max_depth {
        if cancel.is_cancelled() {
            return Err(CrawlError::Cancelled);
        }
        let mut next: Vec<String> = Vec::new();

        let throttle = Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::<
            String,
            std::time::Instant,
        >::new()));
        let delay_ms = config.crawl_delay_ms;
        while !frontier.is_empty() && (stats.crawled as u32) < config.max_urls {
            let remaining = config.max_urls.saturating_sub(stats.crawled as u32) as usize;
            if remaining == 0 {
                break;
            }
            let take = remaining.min(chunk_size).min(frontier.len());
            let batch: Vec<String> = frontier.drain(..take).collect();
            let results: Vec<(String, FetchOutcome)> = stream::iter(batch.into_iter().map(|url| {
                let client = client.clone();
                let renderer = renderer.clone();
                let throttle = throttle.clone();
                async move {
                    let outcome = fetch(&client, &url, renderer, throttle, delay_ms).await;
                    (url, outcome)
                }
            }))
            .buffer_unordered(concurrency)
            .collect()
            .await;

            if debug {
                eprintln!("crawl: batch {} at depth {}", results.len(), depth);
            }
            for (url, outcome) in results {
                if !outcome.chain.is_empty() {
                    stats.redirected += 1;
                    let record = NewUrl {
                        crawl_id: crawl_id.clone(),
                        url: url.clone(),
                        status: outcome.status,
                        blocked: outcome.blocked,
                        console_errors: outcome.console.clone(),
                        axe: outcome.axe.clone(),
                        vitals: outcome.vitals,
                        headers: outcome.headers.clone(),
                        hreflang: Vec::new(),
                        pagination_next: None,
                        pagination_prev: None,
                        structured_data: Default::default(),
                        content_type: outcome.content_type.clone(),
                        title: None,
                        meta_description: None,
                        h1: None,
                        h1_count: 0,
                        canonical: None,
                        meta_robots: None,
                        redirect_target: Some(outcome.final_url.clone()),
                        redirect_chain: outcome.chain.clone(),
                        word_count: None,
                        depth,
                        response_time_ms: Some(outcome.elapsed_ms),
                        size_bytes: Some(outcome.size_bytes),
                        error: outcome.error.clone(),
                        discovered_at: now_ms(),
                    };
                    stats.crawled += 1;
                    store.insert_url(&record)?;
                    if outcome.final_url != url
                        && (!config.same_host_only || in_primary_hosts(&outcome.final_url))
                        && seen.insert(outcome.final_url.clone())
                    {
                        next.push(outcome.final_url.clone());
                    }
                    stats.discovered = seen.len() as u64;
                    on_progress(CrawlProgress {
                        crawl_id: crawl_id.clone(),
                        crawled: stats.crawled,
                        discovered: stats.discovered,
                        current_url: url,
                    });
                    continue;
                }

                let extracted = outcome
                    .body
                    .as_deref()
                    .filter(|_| !outcome.blocked)
                    .filter(|_| is_html(outcome.content_type.as_deref()))
                    .map(|body| extract::extract_page(body, &url));

                let record = NewUrl {
                    crawl_id: crawl_id.clone(),
                    url: url.clone(),
                    status: outcome.status,
                    blocked: outcome.blocked,
                    console_errors: outcome.console.clone(),
                    axe: outcome.axe.clone(),
                    vitals: outcome.vitals,
                    headers: outcome.headers.clone(),
                    hreflang: extracted
                        .as_ref()
                        .map(|page| page.hreflang.clone())
                        .unwrap_or_default(),
                    pagination_next: extracted
                        .as_ref()
                        .and_then(|page| page.pagination_next.clone()),
                    pagination_prev: extracted
                        .as_ref()
                        .and_then(|page| page.pagination_prev.clone()),
                    structured_data: extracted
                        .as_ref()
                        .map(|page| page.structured_data.clone())
                        .unwrap_or_default(),
                    content_type: outcome.content_type.clone(),
                    title: extracted.as_ref().and_then(|page| page.title.clone()),
                    meta_description: extracted
                        .as_ref()
                        .and_then(|page| page.meta_description.clone()),
                    h1: extracted.as_ref().and_then(|page| page.h1.clone()),
                    h1_count: extracted.as_ref().map(|page| page.h1_count).unwrap_or(0),
                    canonical: extracted.as_ref().and_then(|page| page.canonical.clone()),
                    meta_robots: extracted.as_ref().and_then(|page| page.meta_robots.clone()),
                    redirect_target: None,
                    redirect_chain: Vec::new(),
                    word_count: extracted.as_ref().map(|page| page.word_count),
                    depth,
                    response_time_ms: Some(outcome.elapsed_ms),
                    size_bytes: Some(outcome.size_bytes),
                    error: outcome.error.clone(),
                    discovered_at: now_ms(),
                };

                stats.crawled += 1;
                if outcome.error.is_some() || outcome.status.is_none() {
                    stats.failed += 1;
                }
                if outcome.blocked {
                    stats.blocked += 1;
                }
                store.insert_url(&record)?;
                let page_id = store.url_id(&crawl_id, &url)?.unwrap_or_default();
                if page_id > 0
                    && let Some(data) = &outcome.screenshot
                {
                    store.save_screenshot(&crawl_id, page_id, data)?;
                }

                if !extractors.is_empty() {
                    let body = outcome.body.as_deref().unwrap_or_default();
                    for (name, value) in extract::apply_extractors(body, &extractors) {
                        store.save_extraction(&crawl_id, page_id, &name, &value)?;
                    }
                }
                if !outcome.custom_js.is_empty() {
                    store.save_custom_js(&crawl_id, page_id, &outcome.custom_js)?;
                }
                if let Some(page) = extracted {
                    store.replace_images(&crawl_id, page_id, &page.images)?;
                    if config.store_text {
                        store.save_page_text(&crawl_id, page_id, &page.text, &page.markdown)?;
                    }
                    for link in &page.links {
                        let to_id = store.ensure_url_id(&crawl_id, &link.url)?;
                        store.insert_link(
                            &crawl_id,
                            page_id,
                            to_id,
                            link.anchor.as_deref(),
                            link.rel.as_deref(),
                            in_primary_hosts(&link.url),
                        )?;
                    }
                    for link in page.links {
                        if next.len() + seen.len() > max_discovered {
                            break;
                        }
                        if config.same_host_only && !in_primary_hosts(&link.url) {
                            continue;
                        }
                        if !in_scope(&link.url) {
                            continue;
                        }
                        if config.respect_robots && !robots.allows(&path_of(&link.url)) {
                            stats.blocked_by_robots += 1;
                            continue;
                        }
                        if seen.insert(link.url.clone()) {
                            next.push(link.url);
                        }
                    }
                }

                stats.discovered = seen.len() as u64;
                on_progress(CrawlProgress {
                    crawl_id: crawl_id.clone(),
                    crawled: stats.crawled,
                    discovered: stats.discovered,
                    current_url: url,
                });
            }

            let pause_requested = pause.load(Ordering::Relaxed);
            let blocked_abort = (stats.blocked >= 20 && stats.blocked * 2 > stats.crawled)
                || (stats.blocked >= 100 && stats.blocked * 10 > stats.crawled);
            let should_checkpoint = pause_requested
                || blocked_abort
                || frontier.is_empty()
                || last_checkpoint.elapsed() >= Duration::from_secs(10);
            if should_checkpoint {
                let mut resume_frontier = frontier.clone();
                resume_frontier.extend(next.iter().cloned());
                let saved_frontier = resume_frontier.len();
                let checkpoint_state = CrawlCheckpoint {
                    frontier: resume_frontier,
                    seen: seen.iter().cloned().collect(),
                    hosts: primary_hosts.iter().cloned().collect(),
                    depth,
                    crawled: stats.crawled,
                    discovered: stats.discovered,
                    failed: stats.failed,
                    redirected: stats.redirected,
                    blocked_by_robots: stats.blocked_by_robots,
                    blocked: stats.blocked,
                };
                store.save_checkpoint(&crawl_id, &checkpoint_state)?;
                last_checkpoint = Instant::now();
                if debug {
                    eprintln!(
                        "crawl: checkpoint crawled={} frontier_saved={}",
                        stats.crawled, saved_frontier
                    );
                }
            }
            if pause_requested {
                let result = CrawlResult {
                    stats,
                    paused: true,
                };
                notify_webhook(&config, &crawl_id, &result).await;
                return Ok(result);
            }
            if blocked_abort {
                return Err(CrawlError::Blocked(stats.blocked));
            }
        }

        frontier = next;
        depth += 1;
    }

    if cancel.is_cancelled() {
        return Err(CrawlError::Cancelled);
    }
    let result = CrawlResult {
        stats,
        paused: false,
    };
    notify_webhook(&config, &crawl_id, &result).await;
    Ok(result)
}

async fn notify_webhook(config: &CrawlConfig, crawl_id: &str, result: &CrawlResult) {
    let Some(url) = config
        .webhook_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    else {
        return;
    };
    let payload = serde_json::json!({
        "event": "crawl.finished",
        "crawlId": crawl_id,
        "paused": result.paused,
        "stats": {
            "crawled": result.stats.crawled,
            "discovered": result.stats.discovered,
            "failed": result.stats.failed,
            "redirected": result.stats.redirected,
            "blocked": result.stats.blocked,
        },
    });
    let Ok(client) = Client::builder().timeout(Duration::from_secs(5)).build() else {
        return;
    };
    let _ = client.post(url).json(&payload).send().await;
}

fn compile_patterns(patterns: &[String]) -> Result<Vec<Regex>, CrawlError> {
    patterns
        .iter()
        .map(|pattern| pattern.trim())
        .filter(|pattern| !pattern.is_empty())
        .map(|pattern| {
            Regex::new(pattern).map_err(|error| {
                CrawlError::Config(format!("invalid pattern '{pattern}': {error}"))
            })
        })
        .collect()
}

fn is_html(content_type: Option<&str>) -> bool {
    content_type
        .map(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("text/html") || value.contains("application/xhtml")
        })
        .unwrap_or(false)
}

fn looks_blocked(status: u16, headers: &HeaderMap, body: Option<&str>) -> bool {
    if !matches!(status, 403 | 503) {
        return false;
    }
    let cloudflare = headers.get("cf-mitigated").is_some()
        || headers
            .get("server")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.eq_ignore_ascii_case("cloudflare"))
            .unwrap_or(false);
    let marker = body
        .map(|body| {
            body.contains("Just a moment")
                || body.contains("cf-challenge")
                || body.contains("Attention Required")
                || body.contains("you have been blocked")
        })
        .unwrap_or(false);
    if std::env::var_os("COQUI_DEBUG_CRAWL").is_some() && matches!(status, 403 | 503) {
        eprintln!(
            "blocked check status={status} cf_mitigated={} cloudflare_header={} marker={}",
            headers.get("cf-mitigated").is_some(),
            headers
                .get("server")
                .and_then(|value| value.to_str().ok())
                .unwrap_or(""),
            marker
        );
    }
    cloudflare || marker
}

fn path_of(url: &str) -> String {
    Url::parse(url)
        .map(|parsed| {
            let mut path = parsed.path().to_string();
            if let Some(query) = parsed.query() {
                path.push('?');
                path.push_str(query);
            }
            path
        })
        .unwrap_or_else(|_| "/".to_string())
}

async fn fetch(
    client: &Client,
    requested: &str,
    renderer: Option<Arc<RenderPool>>,
    throttle: Arc<tokio::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>>,
    delay_ms: u64,
) -> FetchOutcome {
    let debug = std::env::var_os("COQUI_DEBUG_CRAWL").is_some();
    if debug {
        eprintln!("fetch: GET {requested}");
    }
    let started = Instant::now();
    let mut current = requested.to_string();
    let mut chain: Vec<String> = Vec::new();
    let mut first_status: Option<u16> = None;
    let mut hops = 0usize;
    let mut rate_limit_retries = 0u8;

    loop {
        if delay_ms > 0 {
            let host = url::Url::parse(&current)
                .ok()
                .and_then(|parsed| parsed.host_str().map(str::to_string))
                .unwrap_or_default();
            let mut last = throttle.lock().await;
            if let Some(previous) = last.get(&host).copied() {
                let elapsed = previous.elapsed();
                let target = Duration::from_millis(delay_ms);
                if elapsed < target {
                    tokio::time::sleep(target - elapsed).await;
                }
            }
            last.insert(host, std::time::Instant::now());
        }
        let response = match client.get(&current).send().await {
            Ok(response) => response,
            Err(error) => {
                return FetchOutcome {
                    status: first_status,
                    final_url: current,
                    chain,
                    content_type: None,
                    body: None,
                    size_bytes: 0,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    error: Some(truncate(&error.to_string(), 500)),
                    blocked: false,
                    console: Vec::new(),
                    screenshot: None,
                    axe: Vec::new(),
                    vitals: None,
                    custom_js: Vec::new(),
                    headers: Vec::new(),
                };
            }
        };

        let status = response.status().as_u16();
        if status == 429 && rate_limit_retries < 3 {
            let wait = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(1)
                .min(5);
            rate_limit_retries += 1;
            if debug {
                eprintln!("fetch: 429 for {current}, retry {rate_limit_retries} in {wait}s");
            }
            tokio::time::sleep(Duration::from_secs(wait)).await;
            continue;
        }
        if debug {
            eprintln!("fetch: {status} for {current}");
        }
        if first_status.is_none() {
            first_status = Some(status);
        }

        if (300..400).contains(&status) && hops < MAX_REDIRECTS {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            if let Some(location) = location {
                match resolve_url(&current, &location) {
                    Ok(next) => {
                        chain.push(current);
                        current = next;
                        hops += 1;
                        continue;
                    }
                    Err(_) => break,
                }
            }
        }

        let response_headers = response.headers().clone();
        let header_pairs: Vec<(String, String)> = response_headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    value.to_str().unwrap_or("").to_string(),
                )
            })
            .collect();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string());
        let len = response.content_length().unwrap_or(0);
        let html = is_html(content_type.as_deref());
        let mut body = None;
        let mut size_bytes = len;
        if html && len <= MAX_BODY_BYTES {
            match response.bytes().await {
                Ok(bytes) => {
                    size_bytes = bytes.len() as u64;
                    body = Some(String::from_utf8_lossy(&bytes).into_owned());
                }
                Err(error) => {
                    return FetchOutcome {
                        status: first_status,
                        final_url: current,
                        chain,
                        content_type,
                        body: None,
                        size_bytes,
                        elapsed_ms: started.elapsed().as_millis() as u64,
                        error: Some(truncate(&error.to_string(), 500)),
                        blocked: looks_blocked(status, &response_headers, body.as_deref()),
                        console: Vec::new(),
                        screenshot: None,
                        axe: Vec::new(),
                        vitals: None,
                        custom_js: Vec::new(),
                        headers: header_pairs,
                    };
                }
            }
        }
        let mut body = body;
        let mut console = Vec::new();
        let mut screenshot = None;
        let mut axe = Vec::new();
        let mut custom_js: Vec<serde_json::Value> = Vec::new();
        let mut vitals = None;
        let blocked = looks_blocked(status, &response_headers, body.as_deref());
        let http_elapsed_ms = started.elapsed().as_millis() as u64;
        if !blocked
            && let Some(renderer) = renderer
            && html
            && (200..300).contains(&status)
        {
            match renderer.render_page(&current).await {
                Ok(rendered) => {
                    size_bytes = rendered.html.len() as u64;
                    body = Some(rendered.html);
                    console = rendered.console;
                    screenshot = rendered.screenshot;
                    axe = rendered.axe;
                    custom_js = rendered.custom_js;
                    vitals = rendered.vitals;
                }
                Err(error) => {
                    if std::env::var_os("COQUI_DEBUG_CRAWL").is_some() {
                        eprintln!("render: failed for {current}: {error}");
                    }
                }
            }
        }
        return FetchOutcome {
            status: first_status,
            final_url: current,
            chain,
            content_type,
            body,
            size_bytes,
            elapsed_ms: http_elapsed_ms,
            error: None,
            blocked,
            console,
            screenshot,
            axe,
            vitals,
            headers: header_pairs,
            custom_js,
        };
    }

    FetchOutcome {
        status: first_status,
        final_url: current,
        chain,
        content_type: None,
        body: None,
        size_bytes: 0,
        elapsed_ms: started.elapsed().as_millis() as u64,
        error: Some("too many redirects or unresolvable redirect target".to_string()),
        blocked: false,
        console: Vec::new(),
        screenshot: None,
        axe: Vec::new(),
        vitals: None,
        custom_js: Vec::new(),
        headers: Vec::new(),
    }
}

pub(crate) async fn get_following(
    client: &Client,
    url: &str,
    max_hops: usize,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut current = url.to_string();
    for _ in 0..max_hops {
        let response = client.get(&current).send().await?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            if let Some(location) = location
                && let Ok(next) = resolve_url(&current, &location)
            {
                current = next;
                continue;
            }
        }
        return Ok(response);
    }
    client.get(&current).send().await
}

async fn fetch_robots(client: &Client, seed: &str, user_agent: &str) -> RobotsRules {
    let Ok(mut url) = Url::parse(seed) else {
        return RobotsRules::allow_all();
    };
    url.set_path("/robots.txt");
    url.set_query(None);
    url.set_fragment(None);

    for attempt in 0..3 {
        match get_following(client, url.as_str(), 3).await {
            Ok(response) if response.status().is_success() => {
                if std::env::var_os("COQUI_DEBUG_CRAWL").is_some() {
                    eprintln!("robots: {} from {}", response.status(), url);
                }
                if response.content_length().unwrap_or(0) > MAX_ROBOTS_BYTES {
                    return RobotsRules::allow_all();
                }
                return match response.text().await {
                    Ok(text) => RobotsRules::parse(&text, user_agent),
                    Err(_) => RobotsRules::allow_all(),
                };
            }
            Ok(response) => {
                if std::env::var_os("COQUI_DEBUG_CRAWL").is_some() {
                    eprintln!(
                        "robots: unexpected status {} from {}",
                        response.status(),
                        url
                    );
                }
                return RobotsRules::allow_all();
            }
            Err(error) => {
                if std::env::var_os("COQUI_DEBUG_CRAWL").is_some() {
                    eprintln!("robots: attempt {attempt} failed for {url}: {error}");
                }
                if attempt < 2 {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
            }
        }
    }
    RobotsRules::allow_all()
}

fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        value.to_string()
    } else {
        format!("{}…", &value[..max])
    }
}

/// Resolve the seed exactly the way the UI does, exposed for callers that accept user input.
pub fn normalize_seed(raw: &str) -> Result<String, UrlError> {
    coqui_core::normalize_url(raw)
}
