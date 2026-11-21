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
    /// Custom JavaScript expressions evaluated after rendering.
    #[serde(default)]
    pub custom_js: Vec<String>,
    /// Run the axe-core accessibility audit on rendered pages.
    #[serde(default)]
    pub accessibility: bool,
    /// Capture Web Vitals (LCP, CLS, TBT, TTFB, FCP) from rendered pages.
    #[serde(default)]
    pub web_vitals: bool,
    /// Store extracted text and Markdown for each page (off by default).
    #[serde(default)]
    pub store_text: bool,
    /// Optional webhook URL posted on crawl completion.
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// Minimum delay between HTTP requests, in milliseconds (0 = none).
    #[serde(default)]
    pub crawl_delay_ms: u64,
    /// Maximum requests per second per host (0 = unlimited).
    #[serde(default)]
    pub max_rps: u32,
    /// Optional HTTP proxy URL for all requests.
    #[serde(default)]
    pub proxy_url: Option<String>,
    /// Optional Cookie header value for authenticated crawling.
    #[serde(default)]
    pub cookie: Option<String>,
    /// Store raw HTML alongside text/Markdown (off by default).
    #[serde(default)]
    pub store_html: bool,
    /// Optional CSS selector to wait for after load when rendering.
    #[serde(default)]
    pub render_wait_selector: Option<String>,
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
            custom_js: Vec::new(),
            accessibility: false,
            web_vitals: false,
            store_text: false,
            webhook_url: None,
            crawl_delay_ms: 0,
            max_rps: 0,
            proxy_url: None,
            cookie: None,
            store_html: false,
            render_wait_selector: None,
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
    #[serde(default)]
    pub dom_nodes: Option<u32>,
    #[serde(default)]
    pub render_blocking: Option<u32>,
    #[serde(default)]
    pub transfer_bytes: Option<u64>,
    #[serde(default)]
    pub resource_count: Option<u32>,
    #[serde(default)]
    pub image_bytes: Option<u64>,
    #[serde(default)]
    pub has_viewport: Option<bool>,
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
    #[serde(default)]
    pub title: Option<String>,
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

/// Resolve a filename pattern for an export.
///
/// Tokens: {date}, {datetime}, {domain}, {crawl}, {view}, {report}.
/// Values are sanitised; the extension is appended when missing.
pub fn export_filename(
    pattern: &str,
    domain: &str,
    view: &str,
    crawl_id: &str,
    now_ms: i64,
    extension: &str,
) -> String {
    let (date, datetime) = timestamp_tokens(now_ms);
    let pattern = if pattern.trim().is_empty() {
        "{crawl}-{view}-{datetime}"
    } else {
        pattern
    };
    let mut name = pattern.to_string();
    for (token, value) in [
        ("{date}", date),
        ("{datetime}", datetime),
        ("{domain}", sanitize_token(domain)),
        ("{crawl}", sanitize_token(crawl_id)),
        ("{view}", sanitize_token(view)),
        ("{report}", sanitize_token(view)),
    ] {
        name = name.replace(token, &value);
    }
    name = sanitize_filename(&name);
    let suffix = format!(".{}", extension.trim_start_matches('.'));
    if !name.to_ascii_lowercase().ends_with(&suffix) {
        name.push_str(&suffix);
    }
    if name.trim_matches('.').is_empty() {
        name = format!("{crawl_id}-{view}{suffix}");
    }
    name
}

fn sanitize_token(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn sanitize_filename(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ' ') {
                character
            } else {
                '-'
            }
        })
        .collect();
    cleaned
        .trim_start_matches(['.', '-'])
        .trim()
        .replace(' ', "-")
}

fn timestamp_tokens(now_ms: i64) -> (String, String) {
    let seconds = now_ms.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hours = day_seconds / 3600;
    let minutes = (day_seconds % 3600) / 60;
    let secs = day_seconds % 60;
    (
        format!("{year:04}-{month:02}-{day:02}"),
        format!("{year:04}{month:02}{day:02}-{hours:02}{minutes:02}{secs:02}"),
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

#[cfg(test)]
mod export_filename_tests {
    use super::*;

    #[test]
    fn resolves_tokens_and_extension() {
        let name = export_filename(
            "{domain}-{view}-{date}",
            "example.com",
            "urls",
            "c1",
            1_700_000_000_000,
            "csv",
        );
        assert_eq!(name, "example.com-urls-2023-11-14.csv");
    }

    #[test]
    fn default_pattern_and_sanitising() {
        let name = export_filename("../bad/{view}", "example.com", "issues", "c1", 0, "xlsx");
        assert!(!name.contains('/'));
        assert!(name.ends_with(".xlsx"));
        assert!(name.starts_with("bad-"));
    }
}

/// A named export that the app and CLI can offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDefinition {
    pub id: String,
    pub label: String,
    pub group: String,
    pub view: String,
    pub format: String,
    pub description: String,
}

/// Every export currently implemented, in catalogue order.
pub fn export_catalog() -> Vec<ExportDefinition> {
    let entry =
        |id: &str, label: &str, group: &str, view: &str, format: &str, description: &str| {
            ExportDefinition {
                id: id.to_string(),
                label: label.to_string(),
                group: group.to_string(),
                view: view.to_string(),
                format: format.to_string(),
                description: description.to_string(),
            }
        };
    vec![
        entry(
            "urls-csv",
            "URLs (CSV)",
            "Views",
            "urls",
            "csv",
            "Every URL with the selected columns and active issue filter.",
        ),
        entry(
            "urls-xlsx",
            "URLs (XLSX)",
            "Views",
            "urls",
            "xlsx",
            "Every URL as an Excel workbook.",
        ),
        entry(
            "issues-csv",
            "Issues (CSV)",
            "Views",
            "issues",
            "csv",
            "One row per issue with url, rule, severity and detail.",
        ),
        entry(
            "issues-xlsx",
            "Issues (XLSX)",
            "Views",
            "issues",
            "xlsx",
            "Issues as an Excel workbook.",
        ),
        entry(
            "links-csv",
            "Links (CSV)",
            "Bulk",
            "links",
            "csv",
            "The crawl's link graph.",
        ),
        entry(
            "links-xlsx",
            "Links (XLSX)",
            "Bulk",
            "links",
            "xlsx",
            "The link graph as an Excel workbook.",
        ),
        entry(
            "headers-csv",
            "Response headers",
            "Bulk",
            "headers",
            "csv",
            "Final response headers per URL.",
        ),
        entry(
            "images-csv",
            "Images",
            "Bulk",
            "images",
            "csv",
            "Every image reference with alt and dimensions.",
        ),
        entry(
            "text-csv",
            "Page text",
            "Bulk",
            "text",
            "csv",
            "Extracted body text per URL (requires store text).",
        ),
        entry(
            "markdown-csv",
            "Page Markdown",
            "Bulk",
            "markdown",
            "csv",
            "Generated Markdown per URL (requires store text).",
        ),
        entry(
            "extractions-csv",
            "Custom extraction",
            "Bulk",
            "extractions",
            "csv",
            "Values captured by user-defined CSS extractors.",
        ),
        entry(
            "accessibility-csv",
            "Accessibility",
            "Bulk",
            "accessibility",
            "csv",
            "One row per axe violation.",
        ),
        entry(
            "structured-csv",
            "Structured data",
            "Bulk",
            "structured",
            "csv",
            "JSON-LD types and validation errors.",
        ),
        entry(
            "vitals-csv",
            "Web Vitals",
            "Bulk",
            "vitals",
            "csv",
            "LCP, CLS, TBT, TTFB and FCP per URL.",
        ),
        entry(
            "console-csv",
            "Console errors",
            "Bulk",
            "console",
            "csv",
            "Captured console and network errors.",
        ),
        entry(
            "screenshots",
            "Screenshots",
            "Bulk",
            "screenshots",
            "folder",
            "JPEGs plus screenshots.csv in a folder.",
        ),
    ]
}

#[cfg(test)]
mod export_catalog_tests {
    use super::*;

    #[test]
    fn catalogue_is_unique_and_non_empty() {
        let catalogue = export_catalog();
        assert!(catalogue.len() >= 10);
        let mut ids: Vec<_> = catalogue.iter().map(|entry| entry.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), catalogue.len());
        assert!(catalogue.iter().all(|entry| !entry.view.is_empty()));
    }
}
