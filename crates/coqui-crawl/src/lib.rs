//! Crawl engine: breadth-first frontier, HTTP transport, robots.txt and extraction.
//!
//! Scope today: one seed, bounded depth/URL count, bounded concurrency, robots.txt, manual
//! redirect chains, HTML extraction (title, meta, robots, canonical, H1, word count, links) and
//! durable rows. Authentication, rendering and the full extraction model are tracked in E02/E03/E04.

pub mod extract;
pub mod robots;

use coqui_core::{
    CrawlConfig, CrawlProgress, LinkRecord, NewUrl, UrlError, now_ms, resolve_url, same_host,
};
use coqui_store::{Store, StoreError};
use futures::{StreamExt, stream};
use reqwest::Client;
use reqwest::header::{CONTENT_TYPE, LOCATION};
use robots::RobotsRules;
use std::collections::HashSet;
use std::sync::Arc;
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
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CrawlStats {
    pub crawled: u64,
    pub discovered: u64,
    pub failed: u64,
    pub redirected: u64,
    pub blocked_by_robots: u64,
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
}

/// Run a crawl to completion (or cancellation).
///
/// The caller owns crawl lifecycle status transitions around this function; it does not mark the
/// crawl as running or completed, so the same engine can serve the Tauri shell, CLI and tests.
pub async fn run_crawl(
    store: Arc<Store>,
    crawl_id: String,
    seed: String,
    config: CrawlConfig,
    cancel: CancellationToken,
    on_progress: Arc<dyn Fn(CrawlProgress) + Send + Sync>,
) -> Result<CrawlStats, CrawlError> {
    let client = Client::builder()
        .user_agent(config.user_agent.clone())
        .timeout(Duration::from_millis(config.request_timeout_ms))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let robots = if config.respect_robots {
        fetch_robots(&client, &seed, &config.user_agent).await
    } else {
        RobotsRules::allow_all()
    };

    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(seed.clone());
    let mut frontier: Vec<String> = vec![seed.clone()];
    let mut stats = CrawlStats::default();
    let mut depth = 0u32;
    let max_discovered = (config.max_urls as usize)
        .saturating_mul(MAX_DISCOVERED_MULTIPLIER)
        .max(1);

    if config.max_urls == 0 {
        return Ok(stats);
    }

    while !frontier.is_empty() && depth <= config.max_depth {
        if cancel.is_cancelled() {
            return Err(CrawlError::Cancelled);
        }
        let remaining = config.max_urls.saturating_sub(stats.crawled as u32) as usize;
        if remaining == 0 {
            break;
        }
        let batch: Vec<String> = frontier.drain(..).take(remaining).collect();
        let concurrency = usize::from(config.concurrency.max(1));
        let results: Vec<(String, FetchOutcome)> = stream::iter(batch.into_iter().map(|url| {
            let client = client.clone();
            async move {
                let outcome = fetch(&client, &url).await;
                (url, outcome)
            }
        }))
        .buffer_unordered(concurrency)
        .collect()
        .await;

        let mut next: Vec<String> = Vec::new();
        for (url, outcome) in results {
            if !outcome.chain.is_empty() {
                stats.redirected += 1;
                let record = NewUrl {
                    crawl_id: crawl_id.clone(),
                    url: url.clone(),
                    status: outcome.status,
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
                    && (!config.same_host_only || same_host(&outcome.final_url, &seed))
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
                .filter(|_| is_html(outcome.content_type.as_deref()))
                .map(|body| extract::extract_page(body, &url));

            let record = NewUrl {
                crawl_id: crawl_id.clone(),
                url: url.clone(),
                status: outcome.status,
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
            store.insert_url(&record)?;

            if let Some(page) = extracted {
                for link in &page.links {
                    store.insert_link(
                        &crawl_id,
                        &LinkRecord {
                            from_url: url.clone(),
                            to_url: link.url.clone(),
                            anchor: link.anchor.clone(),
                            rel: link.rel.clone(),
                            internal: same_host(&link.url, &seed),
                        },
                    )?;
                }
                for link in page.links {
                    if next.len() + seen.len() > max_discovered {
                        break;
                    }
                    if config.same_host_only && !same_host(&link.url, &seed) {
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

        frontier = next;
        depth += 1;
    }

    if cancel.is_cancelled() {
        return Err(CrawlError::Cancelled);
    }
    Ok(stats)
}

fn is_html(content_type: Option<&str>) -> bool {
    content_type
        .map(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("text/html") || value.contains("application/xhtml")
        })
        .unwrap_or(false)
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

async fn fetch(client: &Client, requested: &str) -> FetchOutcome {
    let started = Instant::now();
    let mut current = requested.to_string();
    let mut chain: Vec<String> = Vec::new();
    let mut first_status: Option<u16> = None;
    let mut hops = 0usize;

    loop {
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
                };
            }
        };

        let status = response.status().as_u16();
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
                    };
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
            elapsed_ms: started.elapsed().as_millis() as u64,
            error: None,
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
    }
}

async fn fetch_robots(client: &Client, seed: &str, user_agent: &str) -> RobotsRules {
    let Ok(mut url) = Url::parse(seed) else {
        return RobotsRules::allow_all();
    };
    url.set_path("/robots.txt");
    url.set_query(None);
    url.set_fragment(None);
    match client.get(url).send().await {
        Ok(response) if response.status().is_success() => {
            if response.content_length().unwrap_or(0) > MAX_ROBOTS_BYTES {
                return RobotsRules::allow_all();
            }
            match response.text().await {
                Ok(text) => RobotsRules::parse(&text, user_agent),
                Err(_) => RobotsRules::allow_all(),
            }
        }
        _ => RobotsRules::allow_all(),
    }
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
