//! Minimal but real crawl engine for the M0 walking skeleton.
//!
//! Scope today: breadth-first discovery from one seed, bounded depth/URL count, bounded
//! concurrency, HTML extraction (title, meta description, h1, word count, links) and durable
//! rows. Robots handling, auth, rendering and the full extraction model are tracked in E02/E03/E04.

pub mod extract;

use coqui_core::{CrawlConfig, CrawlProgress, NewUrl, UrlError, now_ms, same_host};
use coqui_store::{Store, StoreError};
use futures::{StreamExt, stream};
use reqwest::Client;
use reqwest::header::CONTENT_TYPE;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

const MAX_BODY_BYTES: u64 = 4 * 1024 * 1024;
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
}

struct FetchOutcome {
    status: Option<u16>,
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
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?;

    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(seed.clone());
    let mut frontier: Vec<String> = vec![seed.clone()];
    let mut stats = CrawlStats::default();
    let mut depth = 0u32;
    let max_discovered = (config.max_urls as usize).saturating_mul(MAX_DISCOVERED_MULTIPLIER).max(1);

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
                title: extracted.as_ref().and_then(|p| p.title.clone()),
                meta_description: extracted.as_ref().and_then(|p| p.meta_description.clone()),
                h1: extracted.as_ref().and_then(|p| p.h1.clone()),
                word_count: extracted.as_ref().map(|p| p.word_count),
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
                for link in page.links {
                    if next.len() + seen.len() > max_discovered {
                        break;
                    }
                    if config.same_host_only && !same_host(&link, &seed) {
                        continue;
                    }
                    if seen.insert(link.clone()) {
                        next.push(link);
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

async fn fetch(client: &Client, url: &str) -> FetchOutcome {
    let started = Instant::now();
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(error) => {
            return FetchOutcome {
                status: None,
                content_type: None,
                body: None,
                size_bytes: 0,
                elapsed_ms: started.elapsed().as_millis() as u64,
                error: Some(truncate(&error.to_string(), 500)),
            };
        }
    };

    let status = Some(response.status().as_u16());
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
                    status,
                    content_type,
                    body: None,
                    size_bytes,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    error: Some(truncate(&error.to_string(), 500)),
                };
            }
        }
    }

    FetchOutcome {
        status,
        content_type,
        body,
        size_bytes,
        elapsed_ms: started.elapsed().as_millis() as u64,
        error: None,
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


