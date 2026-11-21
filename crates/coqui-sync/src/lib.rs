//! Cloud sync client for Coqui.
//!
//! Implements the desktop sync contract in `coqui-platform/docs/desktop-sync-contract.md`:
//! initialize a crawl run, stream URLs/links/issues in bounded chunks, then finalize.
//! The same client backs the CLI and the desktop "Send to cloud" flow.

use coqui_core::{CrawlConfig, CrawlStatus};
use coqui_store::Store;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_API_URL: &str = "https://api.coquiseo.com";
pub const URL_CHUNK: u32 = 1_000;
pub const LINK_CHUNK: u32 = 2_500;
pub const ISSUE_CHUNK: u32 = 2_500;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("store error: {0}")]
    Store(#[from] coqui_store::StoreError),
    #[error("invalid api url: {0}")]
    ApiUrl(String),
    #[error("only completed crawls can be synced (crawl is {0})")]
    NotCompleted(String),
    #[error("cloud API returned {status}: {message}")]
    Api { status: u16, message: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("invalid response: {0}")]
    Response(String),
    #[error("invalid json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SyncError>;

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub api_url: String,
    pub token: String,
    pub project: String,
    pub dry_run: bool,
}

impl SyncOptions {
    pub fn new(
        api_url: impl Into<String>,
        token: impl Into<String>,
        project: impl Into<String>,
    ) -> Self {
        Self {
            api_url: api_url.into(),
            token: token.into(),
            project: project.into(),
            dry_run: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncPhase {
    Preparing,
    Urls,
    Links,
    Issues,
    Finalizing,
    Done,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub phase: SyncPhase,
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub external_crawl_id: String,
    pub project: String,
    pub source: String,
    pub urls: u64,
    pub links: u64,
    pub issues: u64,
    pub chunks: u64,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudProject {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub workspace_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProjectPage {
    data: Vec<CloudProject>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CrawlPayload<'a> {
    id: &'a str,
    seed: &'a str,
    seed_kind: &'a str,
    status: &'a str,
    config: &'a CrawlConfig,
    created_at: i64,
    started_at: Option<i64>,
    finished_at: Option<i64>,
    total_urls: u64,
    source: &'a str,
}

fn api_base(input: &str) -> Result<String> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(SyncError::ApiUrl("empty api url".into()));
    }
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .ok_or_else(|| SyncError::ApiUrl("expected http:// or https://".into()))?;
    if without_scheme.is_empty() {
        return Err(SyncError::ApiUrl("missing host".into()));
    }
    if trimmed.ends_with("/api/v1") {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("{trimmed}/api/v1"))
    }
}

/// Stable cloud identity for a crawl. Imported crawls already use their UUID v5;
/// native crawls with non-UUID ids (CLI) get a deterministic UUID v5 from the id.
pub fn external_crawl_id(crawl_id: &str) -> String {
    match Uuid::parse_str(crawl_id) {
        Ok(uuid) => uuid.to_string(),
        Err(_) => Uuid::new_v5(&Uuid::NAMESPACE_URL, crawl_id.as_bytes()).to_string(),
    }
}

/// Remove credentials and secret-bearing URLs before anything leaves the machine.
pub fn sanitize_config(config: &CrawlConfig) -> CrawlConfig {
    let mut safe = config.clone();
    safe.auth_user = None;
    safe.auth_password = None;
    safe.cookie = None;
    // A proxy URL can carry user:password@.
    safe.proxy_url = None;
    // Webhook URLs are bearer-like secrets.
    safe.webhook_url = None;
    safe.custom_headers.retain(|line| {
        let name = line
            .split_once(':')
            .map(|(name, _)| name.trim().to_ascii_lowercase())
            .unwrap_or_default();
        !matches!(
            name.as_str(),
            "authorization" | "cookie" | "proxy-authorization" | "x-api-key"
        )
    });
    safe
}

fn error_message(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(message) = value.get("message").and_then(|value| value.as_str()) {
            return message.to_string();
        }
    }
    body.trim().chars().take(300).collect()
}

async fn send(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: &str,
    token: &str,
    body: Option<&serde_json::Value>,
) -> Result<serde_json::Value> {
    let mut request = client.request(method, url).bearer_auth(token);
    if let Some(body) = body {
        request = request.json(body);
    }
    let response = request
        .send()
        .await
        .map_err(|error| SyncError::Network(error.to_string()))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| SyncError::Network(error.to_string()))?;
    if !status.is_success() {
        return Err(SyncError::Api {
            status: status.as_u16(),
            message: error_message(&text),
        });
    }
    if text.trim().is_empty() {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_str(&text).map_err(|error| SyncError::Response(error.to_string()))
}

fn post_json<T: Serialize>(value: &T) -> Result<serde_json::Value> {
    serde_json::to_value(value).map_err(SyncError::from)
}

/// List projects visible to a workspace token (the platform project picker).
pub async fn list_projects(api_url: &str, token: &str) -> Result<Vec<CloudProject>> {
    let base = api_base(api_url)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| SyncError::Network(error.to_string()))?;
    let mut projects = Vec::new();
    let mut page = 1u32;
    loop {
        let url = format!("{base}/projects?per_page=100&page={page}");
        let value = send(&client, reqwest::Method::GET, &url, token, None).await?;
        let parsed: ProjectPage = serde_json::from_value(value)
            .map_err(|error| SyncError::Response(error.to_string()))?;
        let count = parsed.data.len();
        projects.extend(parsed.data);
        if count < 100 || page >= 10 {
            break;
        }
        page += 1;
    }
    Ok(projects)
}

/// Push one crawl to a project using the sync contract.
pub async fn sync_crawl<F>(
    store: &Store,
    crawl_id: &str,
    options: &SyncOptions,
    mut progress: F,
) -> Result<SyncReport>
where
    F: FnMut(SyncProgress),
{
    progress(SyncProgress {
        phase: SyncPhase::Preparing,
        done: 0,
        total: 0,
    });

    let summary = store.get_crawl(crawl_id)?;
    if summary.status != CrawlStatus::Completed {
        return Err(SyncError::NotCompleted(summary.status.as_str().to_string()));
    }
    let external_id = external_crawl_id(&summary.id);
    let source = if summary.source == "import" {
        "import"
    } else {
        "desktop"
    };
    let (urls, links, issues) = store.crawl_counts(crawl_id)?;
    let issues_to_send = if source == "import" { 0 } else { issues };
    let config = sanitize_config(&summary.config);
    let total = urls + links + issues_to_send;
    let base = api_base(&options.api_url)?;
    let project = options.project.trim();
    if project.is_empty() {
        return Err(SyncError::ApiUrl("project id is required".into()));
    }

    if options.dry_run {
        progress(SyncProgress {
            phase: SyncPhase::Done,
            done: total,
            total,
        });
        return Ok(SyncReport {
            external_crawl_id: external_id,
            project: options.project.clone(),
            source: source.to_string(),
            urls,
            links,
            issues: issues_to_send,
            chunks: 0,
            dry_run: true,
        });
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|error| SyncError::Network(error.to_string()))?;
    let crawl_url = format!("{base}/projects/{project}/crawls/{external_id}");

    // 1. Initialize (or reopen) the crawl run.
    let payload = CrawlPayload {
        id: &external_id,
        seed: &summary.seed,
        seed_kind: &summary.seed_kind,
        status: "completed",
        config: &config,
        created_at: summary.created_at,
        started_at: summary.started_at,
        finished_at: summary.finished_at,
        total_urls: urls,
        source,
    };
    let body = post_json(&payload)?;
    send(
        &client,
        reqwest::Method::POST,
        &format!("{base}/projects/{project}/crawls/sync"),
        &options.token,
        Some(&body),
    )
    .await?;

    // 2. Stream chunks. Each type has its own cursor so a retry never duplicates rows.
    let mut chunks = 0u64;
    let mut done = 0u64;
    let mut cursor = 0i64;
    loop {
        let rows = store.url_rows_after(crawl_id, cursor, URL_CHUNK)?;
        if rows.is_empty() {
            break;
        }
        cursor = rows.last().map(|row| row.id).unwrap_or(cursor);
        let body = serde_json::json!({ "urls": &rows });
        send(
            &client,
            reqwest::Method::POST,
            &format!("{crawl_url}/chunks"),
            &options.token,
            Some(&body),
        )
        .await?;
        chunks += 1;
        done += rows.len() as u64;
        progress(SyncProgress {
            phase: SyncPhase::Urls,
            done,
            total,
        });
    }

    cursor = 0;
    loop {
        let rows = store.link_rows_after(crawl_id, cursor, LINK_CHUNK)?;
        if rows.is_empty() {
            break;
        }
        cursor = rows.last().map(|(id, _)| *id).unwrap_or(cursor);
        let link_rows: Vec<&coqui_core::LinkRecord> = rows.iter().map(|(_, row)| row).collect();
        let body = serde_json::json!({ "links": link_rows });
        send(
            &client,
            reqwest::Method::POST,
            &format!("{crawl_url}/chunks"),
            &options.token,
            Some(&body),
        )
        .await?;
        chunks += 1;
        done += rows.len() as u64;
        progress(SyncProgress {
            phase: SyncPhase::Links,
            done,
            total,
        });
    }

    if issues_to_send > 0 {
        cursor = 0;
        loop {
            let rows = store.issue_rows_after(crawl_id, cursor, ISSUE_CHUNK)?;
            if rows.is_empty() {
                break;
            }
            cursor = rows.last().map(|(id, _)| *id).unwrap_or(cursor);
            let issue_rows: Vec<&coqui_core::Issue> = rows.iter().map(|(_, row)| row).collect();
            let body = serde_json::json!({ "issues": issue_rows });
            send(
                &client,
                reqwest::Method::POST,
                &format!("{crawl_url}/chunks"),
                &options.token,
                Some(&body),
            )
            .await?;
            chunks += 1;
            done += rows.len() as u64;
            progress(SyncProgress {
                phase: SyncPhase::Issues,
                done,
                total,
            });
        }
    }

    // 3. Finalize. No issueCounts: the platform counts what was uploaded.
    progress(SyncProgress {
        phase: SyncPhase::Finalizing,
        done,
        total,
    });
    let body = serde_json::json!({ "status": "completed" });
    send(
        &client,
        reqwest::Method::POST,
        &format!("{crawl_url}/finalize"),
        &options.token,
        Some(&body),
    )
    .await?;

    progress(SyncProgress {
        phase: SyncPhase::Done,
        done: total,
        total,
    });
    Ok(SyncReport {
        external_crawl_id: external_id,
        project: project.to_string(),
        source: source.to_string(),
        urls,
        links,
        issues: issues_to_send,
        chunks,
        dry_run: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use coqui_store::SfImportOptions;
    use std::path::Path;

    #[tokio::test]
    async fn imported_crawl_dry_run_reports_structure_without_issues() {
        let dir = std::env::temp_dir().join(format!("coqui-sync-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/parity/fixtures/reference/reference.dbseospider");
        let stats = store
            .import_seospider_with(&fixture, SfImportOptions::default())
            .unwrap();
        let report = sync_crawl(
            &store,
            &stats.crawl_id,
            &SyncOptions {
                api_url: "http://127.0.0.1:9".into(),
                token: "unused".into(),
                project: "550e8400-e29b-41d4-a716-446655440000".into(),
                dry_run: true,
            },
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(report.source, "import");
        assert_eq!(report.urls, stats.urls as u64);
        assert_eq!(report.links, stats.links as u64);
        assert_eq!(report.issues, 0);
        assert_eq!(report.external_crawl_id, stats.crawl_id);
        assert!(report.dry_run);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn api_base_appends_version_once() {
        assert_eq!(
            api_base("http://localhost:8000").unwrap(),
            "http://localhost:8000/api/v1"
        );
        assert_eq!(
            api_base("https://api.example.com/").unwrap(),
            "https://api.example.com/api/v1"
        );
        assert_eq!(
            api_base("https://api.example.com/api/v1/").unwrap(),
            "https://api.example.com/api/v1"
        );
    }

    #[test]
    fn api_base_rejects_missing_scheme() {
        assert!(api_base("api.example.com").is_err());
        assert!(api_base("").is_err());
    }

    #[test]
    fn external_ids_are_stable_and_uuids() {
        let native = external_crawl_id("cli-123");
        assert_eq!(native, external_crawl_id("cli-123"));
        assert!(Uuid::parse_str(&native).is_ok());
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        assert_eq!(external_crawl_id(uuid), uuid);
    }

    #[test]
    fn config_is_scrubbed() {
        let mut config = CrawlConfig::default();
        config.auth_user = Some("user".into());
        config.auth_password = Some("secret".into());
        config.cookie = Some("session=abc".into());
        config.proxy_url = Some("http://user:pass@proxy".into());
        config.webhook_url = Some("https://hooks.example/secret".into());
        config.custom_headers = vec![
            "Authorization: Bearer abc".into(),
            "Cookie: a=b".into(),
            "X-Custom: safe".into(),
        ];
        let safe = sanitize_config(&config);
        assert!(safe.auth_user.is_none());
        assert!(safe.auth_password.is_none());
        assert!(safe.cookie.is_none());
        assert!(safe.proxy_url.is_none());
        assert!(safe.webhook_url.is_none());
        assert_eq!(safe.custom_headers, vec!["X-Custom: safe".to_string()]);
    }
}
