//! Coqui domain model and URL identity.
//!
//! Everything in this crate is deterministic and free of I/O, so it can be used by the
//! crawler, the store, the Tauri shell and (later) the CLI and server.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub mod url_norm;

pub use url_norm::{UrlError, normalize_url, resolve_url, same_host};

/// Identifier of a crawl. UUID v4 string.
pub type CrawlId = String;

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// The configuration a crawl is started with.
///
/// This is deliberately a small subset of the full parity surface (see issue G-01); fields are
/// added as the corresponding features land.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CrawlConfig {
    /// Maximum link depth from the seed. 0 crawls only the seed.
    pub max_depth: u32,
    /// Hard cap on the number of URLs fetched.
    pub max_urls: u32,
    /// Number of concurrent requests.
    pub concurrency: u16,
    /// Per-request timeout in milliseconds.
    pub request_timeout_ms: u64,
    /// User-Agent sent with every request.
    pub user_agent: String,
    /// When true, only URLs on the seed host are enqueued.
    pub same_host_only: bool,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_urls: 500,
            concurrency: 8,
            request_timeout_ms: 15_000,
            user_agent: format!(
                "Coqui/{} (+https://github.com/adamgreenwell/coqui)",
                env!("CARGO_PKG_VERSION")
            ),
            same_host_only: true,
        }
    }
}

/// Lifecycle state of a crawl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CrawlStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl CrawlStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "paused" => Self::Paused,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Pending,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// A crawl as shown in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawlSummary {
    pub id: CrawlId,
    pub seed: String,
    pub status: CrawlStatus,
    pub config: CrawlConfig,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    /// Number of URL rows currently stored for this crawl.
    pub total_urls: i64,
}

/// One crawled (or attempted) URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlRecord {
    pub id: i64,
    pub crawl_id: CrawlId,
    pub url: String,
    pub status: Option<u16>,
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub h1: Option<String>,
    pub word_count: Option<u32>,
    pub depth: u32,
    pub response_time_ms: Option<u64>,
    pub size_bytes: Option<u64>,
    pub error: Option<String>,
    pub discovered_at: i64,
}

/// A page of URL rows plus the total for the crawl.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlPage {
    pub rows: Vec<UrlRecord>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

/// Progress emitted while a crawl runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawlProgress {
    pub crawl_id: CrawlId,
    pub crawled: u64,
    pub discovered: u64,
    pub current_url: String,
}

/// A URL as it enters the store, before an id is assigned.
#[derive(Debug, Clone)]
pub struct NewUrl {
    pub crawl_id: CrawlId,
    pub url: String,
    pub status: Option<u16>,
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub h1: Option<String>,
    pub word_count: Option<u32>,
    pub depth: u32,
    pub response_time_ms: Option<u64>,
    pub size_bytes: Option<u64>,
    pub error: Option<String>,
    pub discovered_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_sane() {
        let config = CrawlConfig::default();
        assert!(config.max_urls > 0);
        assert!(config.concurrency > 0);
        assert!(config.request_timeout_ms > 0);
        assert!(config.user_agent.starts_with("Coqui/"));
    }

    #[test]
    fn status_round_trips() {
        for status in [
            CrawlStatus::Pending,
            CrawlStatus::Running,
            CrawlStatus::Paused,
            CrawlStatus::Completed,
            CrawlStatus::Failed,
            CrawlStatus::Cancelled,
        ] {
            assert_eq!(CrawlStatus::parse(status.as_str()), status);
        }
    }

    #[test]
    fn config_serialises_as_camel_case() {
        let json = serde_json::to_string(&CrawlConfig::default()).unwrap();
        assert!(json.contains("maxDepth"));
        assert!(json.contains("maxUrls"));
    }
}
