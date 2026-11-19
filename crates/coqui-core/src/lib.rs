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
    /// Respect robots.txt rules for the configured user agent.
    #[serde(default = "default_true")]
    pub respect_robots: bool,
    /// Render pages in a headless Chromium before extraction.
    #[serde(default)]
    pub render: bool,
    /// Optional explicit Chrome/Edge executable.
    #[serde(default)]
    pub browser_path: Option<String>,
    /// Extra delay after load before reading the DOM.
    #[serde(default = "default_render_wait_ms")]
    pub render_wait_ms: u64,
    /// Number of concurrent renderer browsers.
    #[serde(default = "default_render_workers")]
    pub render_workers: u32,
    /// Capture a JPEG screenshot for each rendered page.
    #[serde(default)]
    pub screenshots: bool,
    /// Block fonts and media during rendering.
    #[serde(default)]
    pub render_block_assets: bool,
    /// Continue at DOMContentLoaded instead of waiting for full load.
    #[serde(default)]
    pub render_early: bool,
    /// Extra URL patterns to block while rendering (for example third-party scripts).
    #[serde(default)]
    pub render_block_patterns: Vec<String>,
    /// Run the axe-core accessibility audit on rendered pages.
    #[serde(default)]
    pub accessibility: bool,
    /// Capture Web Vitals (LCP, CLS, TBT, TTFB, FCP) from rendered pages.
    #[serde(default)]
    pub web_vitals: bool,
    /// Only crawl discovered URLs matching at least one pattern (empty = all).
    #[serde(default)]
    pub include_patterns: Vec<String>,
    /// Never crawl discovered URLs matching any pattern.
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    /// Optional HTTP basic-auth username.
    #[serde(default)]
    pub auth_user: Option<String>,
    /// Optional HTTP basic-auth password.
    #[serde(default)]
    pub auth_password: Option<String>,
    /// Extra request headers, each `Name: value`.
    #[serde(default)]
    pub custom_headers: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_render_wait_ms() -> u64 {
    500
}

fn default_render_workers() -> u32 {
    2
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
            respect_robots: true,
            render: false,
            browser_path: None,
            render_wait_ms: 500,
            render_workers: 2,
            screenshots: false,
            render_block_assets: false,
            render_early: false,
            render_block_patterns: Vec::new(),
            accessibility: false,
            web_vitals: false,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            auth_user: None,
            auth_password: None,
            custom_headers: Vec::new(),
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
    /// How the seed was supplied: `url`, `sitemap` or `list`.
    pub seed_kind: String,
    pub status: CrawlStatus,
    pub config: CrawlConfig,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    /// Number of URL rows currently stored for this crawl.
    pub total_urls: i64,
    /// URLs that returned a WAF/challenge response rather than content.
    pub blocked_urls: i64,
}

/// One crawled (or attempted) URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlRecord {
    pub id: i64,
    pub crawl_id: CrawlId,
    pub url: String,
    pub status: Option<u16>,
    pub blocked: bool,
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub h1: Option<String>,
    pub h1_count: u32,
    pub canonical: Option<String>,
    pub meta_robots: Option<String>,
    pub hreflang: Vec<HreflangLink>,
    pub pagination_next: Option<String>,
    pub pagination_prev: Option<String>,
    pub structured_data: StructuredData,
    pub console_errors: Vec<String>,
    pub axe: Vec<AxeViolation>,
    pub vitals: Option<WebVitals>,
    /// Final response headers (name, value), in wire order.
    pub headers: Vec<(String, String)>,
    pub redirect_target: Option<String>,
    pub redirect_chain: Vec<String>,
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
    pub blocked: bool,
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub h1: Option<String>,
    pub h1_count: u32,
    pub canonical: Option<String>,
    pub meta_robots: Option<String>,
    pub hreflang: Vec<HreflangLink>,
    pub pagination_next: Option<String>,
    pub pagination_prev: Option<String>,
    pub structured_data: StructuredData,
    pub console_errors: Vec<String>,
    pub axe: Vec<AxeViolation>,
    pub vitals: Option<WebVitals>,
    /// Final response headers (name, value), in wire order.
    pub headers: Vec<(String, String)>,
    pub redirect_target: Option<String>,
    pub redirect_chain: Vec<String>,
    pub word_count: Option<u32>,
    pub depth: u32,
    pub response_time_ms: Option<u64>,
    pub size_bytes: Option<u64>,
    pub error: Option<String>,
    pub discovered_at: i64,
}

/// Severity of an audit issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueSeverity {
    Error,
    Warning,
    Opportunity,
}

impl IssueSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Opportunity => "opportunity",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "error" => Self::Error,
            "opportunity" => Self::Opportunity,
            _ => Self::Warning,
        }
    }
}

/// Aggregated issue count for a rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueCount {
    pub rule: String,
    pub severity: IssueSeverity,
    pub count: i64,
}

/// Persisted crawl state so a crawl can pause and resume without refetching.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CrawlCheckpoint {
    pub frontier: Vec<String>,
    pub seen: Vec<String>,
    #[serde(default)]
    pub hosts: Vec<String>,
    pub depth: u32,
    pub crawled: u64,
    pub discovered: u64,
    pub failed: u64,
    pub redirected: u64,
    pub blocked_by_robots: u64,
    #[serde(default)]
    pub blocked: u64,
}

/// Web Vitals captured from a rendered page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebVitals {
    pub lcp_ms: u64,
    pub cls: f64,
    pub tbt_ms: u64,
    pub ttfb_ms: u64,
    pub fcp_ms: u64,
}

/// One axe-core accessibility violation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AxeViolation {
    pub id: String,
    pub impact: Option<String>,
    pub help: String,
    pub help_url: String,
    pub nodes: u32,
}

/// An image referenced by a page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRef {
    pub src: String,
    pub alt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// An hreflang alternate declared by a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HreflangLink {
    pub lang: String,
    pub url: String,
}

/// JSON-LD structured data discovered on a page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredData {
    /// Distinct `@type` values found in valid JSON-LD blocks.
    pub types: Vec<String>,
    /// Syntax and required-field problems, each with a short prefix.
    pub errors: Vec<String>,
}

/// A link edge discovered on a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRecord {
    pub from_url: String,
    pub to_url: String,
    pub anchor: Option<String>,
    pub rel: Option<String>,
    pub internal: bool,
}

/// A single issue occurrence, stored per URL and rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub crawl_id: CrawlId,
    pub url: String,
    pub rule: String,
    pub severity: IssueSeverity,
    pub detail: Option<String>,
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
