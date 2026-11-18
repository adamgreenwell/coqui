//! Seed resolution: a single URL, a URL list file, or a sitemap (including sitemap indexes).

use coqui_core::{UrlError, normalize_url};
use flate2::read::GzDecoder;
use reqwest::Client;
use roxmltree::Document;
use std::collections::HashSet;
use std::io::Read;
use url::Url;

const MAX_SITEMAPS: usize = 20;
const MAX_SEEDS: usize = 50_000;

#[derive(Debug, thiserror::Error)]
pub enum SeedError {
    #[error("invalid seed: {0}")]
    Invalid(String),
    #[error("failed to fetch sitemap: {0}")]
    Http(#[from] reqwest::Error),
    #[error("sitemap parse error: {0}")]
    Xml(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("no seeds found")]
    Empty,
}

impl From<UrlError> for SeedError {
    fn from(error: UrlError) -> Self {
        SeedError::Invalid(error.to_string())
    }
}

/// Resolve a seed specification into a deduplicated list of crawl URLs.
pub async fn resolve(client: &Client, kind: &str, value: &str) -> Result<Vec<String>, SeedError> {
    match kind {
        "url" => Ok(vec![normalize_url(value)?]),
        "list" => resolve_list(value),
        "sitemap" => resolve_sitemap(client, value).await,
        other => Err(SeedError::Invalid(format!("unknown seed kind '{other}'"))),
    }
}

fn resolve_list(path: &str) -> Result<Vec<String>, SeedError> {
    let text = std::fs::read_to_string(path)?;
    let mut seen = HashSet::new();
    let mut seeds = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Ok(url) = normalize_url(line) {
            if seen.insert(url.clone()) {
                seeds.push(url);
            }
        }
        if seeds.len() >= MAX_SEEDS {
            break;
        }
    }
    if seeds.is_empty() {
        Err(SeedError::Empty)
    } else {
        Ok(seeds)
    }
}

async fn resolve_sitemap(client: &Client, url: &str) -> Result<Vec<String>, SeedError> {
    let mut seeds = Vec::new();
    let mut seen = HashSet::new();
    let mut sitemaps = 0usize;
    collect_sitemap(client, url, 0, &mut sitemaps, &mut seen, &mut seeds).await?;
    if seeds.is_empty() {
        Err(SeedError::Empty)
    } else {
        Ok(seeds)
    }
}

async fn collect_sitemap(
    client: &Client,
    url: &str,
    depth: usize,
    sitemaps: &mut usize,
    seen: &mut HashSet<String>,
    seeds: &mut Vec<String>,
) -> Result<(), SeedError> {
    if depth > 2 || *sitemaps >= MAX_SITEMAPS || seeds.len() >= MAX_SEEDS {
        return Ok(());
    }
    *sitemaps += 1;

    let response = crate::get_following(client, url, 5).await?;
    if !response.status().is_success() {
        return Err(SeedError::Invalid(format!(
            "sitemap returned HTTP {} for {url}",
            response.status()
        )));
    }
    let bytes = response.bytes().await?;
    let text = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = GzDecoder::new(bytes.as_ref());
        let mut text = String::new();
        decoder
            .read_to_string(&mut text)
            .map_err(|error| SeedError::Xml(error.to_string()))?;
        text
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };

    let document = Document::parse(&text).map_err(|error| SeedError::Xml(error.to_string()))?;
    let root = document.root_element();
    let tag = root.tag_name().name();
    match tag {
        "urlset" => {
            for node in root.descendants().filter(|node| node.has_tag_name("loc")) {
                let Some(value) = node.text() else { continue };
                let Ok(url) = normalize_url(value.trim()) else {
                    continue;
                };
                if seen.insert(url.clone()) {
                    seeds.push(url);
                }
                if seeds.len() >= MAX_SEEDS {
                    break;
                }
            }
        }
        "sitemapindex" => {
            let locations: Vec<String> = root
                .descendants()
                .filter(|node| node.has_tag_name("loc"))
                .filter_map(|node| node.text().map(str::trim).map(str::to_string))
                .collect();
            for location in locations {
                Box::pin(collect_sitemap(
                    client,
                    &location,
                    depth + 1,
                    sitemaps,
                    seen,
                    seeds,
                ))
                .await?;
            }
        }
        other => {
            return Err(SeedError::Xml(format!(
                "unsupported sitemap root element '{other}'"
            )));
        }
    }
    Ok(())
}

/// True when the URL has an http(s) host that can be parsed.
pub fn host_of(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|host| host.to_ascii_lowercase()))
}
