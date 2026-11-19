//! Tauri commands for the Coqui desktop shell.
//!
//! The shell owns crawl lifecycle and event emission; crawling itself lives in `coqui-crawl` so
//! the same engine can back the CLI and an HTTP API later.

use coqui_core::{
    CrawlConfig, CrawlProgress, CrawlStatus, CrawlSummary, IssueCount, UrlPage, normalize_url,
    now_ms,
};
use coqui_store::Store;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

pub struct AppState {
    pub store: Arc<Store>,
    pub cancels: Mutex<HashMap<String, CancellationToken>>,
    pub pauses: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// Session-only headers (Cookie/Authorization) kept out of the persisted crawl config.
    pub sessions: Mutex<HashMap<String, Vec<String>>>,
    pub data_dir: PathBuf,
}

#[derive(Debug, serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CrawlFinished {
    crawl_id: String,
    status: CrawlStatus,
    error: Option<String>,
}

fn split_session_headers(config: &mut CrawlConfig) -> Vec<String> {
    let mut session = Vec::new();
    config.custom_headers.retain(|line| {
        let name = line
            .split_once(':')
            .map(|(name, _)| name.trim().to_ascii_lowercase())
            .unwrap_or_default();
        if name == "cookie" || name == "authorization" {
            session.push(line.clone());
            false
        } else {
            true
        }
    });
    session
}
#[tauri::command]
fn create_crawl(
    state: State<'_, AppState>,
    seed_kind: String,
    seed: String,
    mut config: CrawlConfig,
) -> Result<CrawlSummary, String> {
    if !matches!(seed_kind.as_str(), "url" | "sitemap" | "list") {
        return Err(format!("Unknown seed kind '{seed_kind}'"));
    }
    let seed = match seed_kind.as_str() {
        "url" | "sitemap" => {
            normalize_url(&seed).map_err(|error| format!("Invalid seed URL: {error}"))?
        }
        _ => seed,
    };
    let session_headers = split_session_headers(&mut config);
    let id = uuid::Uuid::new_v4().to_string();
    if !session_headers.is_empty() {
        state
            .sessions
            .lock()
            .map_err(|_| "crawl registry poisoned".to_string())?
            .insert(id.clone(), session_headers);
    }
    state
        .store
        .create_crawl(&id, &seed, &seed_kind, &config, now_ms())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_crawl(state: State<'_, AppState>, crawl_id: String) -> Result<CrawlSummary, String> {
    state
        .store
        .get_crawl(&crawl_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_urls(
    state: State<'_, AppState>,
    crawl_id: String,
    page: u32,
    page_size: u32,
    issue: Option<String>,
) -> Result<UrlPage, String> {
    state
        .store
        .list_urls(&crawl_id, page, page_size, issue.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_issue_counts(
    state: State<'_, AppState>,
    crawl_id: String,
) -> Result<Vec<IssueCount>, String> {
    state
        .store
        .issue_counts(&crawl_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn start_crawl(app: AppHandle, state: State<'_, AppState>, crawl_id: String) -> Result<(), String> {
    let summary = state
        .store
        .get_crawl(&crawl_id)
        .map_err(|error| error.to_string())?;
    if summary.status == CrawlStatus::Completed {
        return Err("This crawl has already finished. Start a new crawl.".into());
    }
    if summary.status == CrawlStatus::Running {
        return Err("This crawl is already running.".into());
    }

    let token = CancellationToken::new();
    let pause = Arc::new(AtomicBool::new(false));
    {
        let mut cancels = state
            .cancels
            .lock()
            .map_err(|_| "crawl registry poisoned".to_string())?;
        cancels.insert(crawl_id.clone(), token.clone());
    }
    {
        let mut pauses = state
            .pauses
            .lock()
            .map_err(|_| "crawl registry poisoned".to_string())?;
        pauses.insert(crawl_id.clone(), pause.clone());
    }
    let checkpoint = state
        .store
        .load_checkpoint(&crawl_id)
        .map_err(|error| error.to_string())?;

    let session_headers = state
        .sessions
        .lock()
        .map_err(|_| "crawl registry poisoned".to_string())?
        .get(&crawl_id)
        .cloned()
        .unwrap_or_default();
    let store = state.store.clone();
    let app_handle = app.clone();
    let id = crawl_id.clone();

    tauri::async_runtime::spawn(async move {
        let seed_kind = summary.seed_kind.clone();
        let seed_value = summary.seed.clone();
        let mut config = summary.config.clone();
        config.custom_headers.extend(session_headers);
        let progress_app = app_handle.clone();
        let progress: Arc<dyn Fn(CrawlProgress) + Send + Sync> = Arc::new(move |event| {
            let _ = progress_app.emit("crawl-progress", event);
        });

        let _ = store.mark_running(&id, now_ms());
        let result = coqui_crawl::run_crawl(
            store.clone(),
            id.clone(),
            seed_kind,
            seed_value,
            config,
            token,
            pause,
            checkpoint,
            progress,
        )
        .await;

        let (status, error) = match result {
            Ok(result) if result.paused => (CrawlStatus::Paused, None),
            Ok(_) => (CrawlStatus::Completed, None),
            Err(coqui_crawl::CrawlError::Cancelled) => (CrawlStatus::Cancelled, None),
            Err(error) => (CrawlStatus::Failed, Some(error.to_string())),
        };
        match status {
            CrawlStatus::Completed => {
                if let Err(error) = coqui_audit::analyze_crawl(&store, &id) {
                    tracing::warn!(%error, crawl_id = %id, "audit rules failed");
                }
                let _ = store.delete_checkpoint(&id);
            }
            CrawlStatus::Cancelled => {
                let _ = store.delete_checkpoint(&id);
            }
            _ => {}
        }
        let _ = store.mark_finished(&id, status, now_ms());
        if let Ok(mut registry) = app_handle.state::<AppState>().cancels.lock() {
            registry.remove(&id);
        }
        if let Ok(mut pauses) = app_handle.state::<AppState>().pauses.lock() {
            pauses.remove(&id);
        }
        if let Ok(mut sessions) = app_handle.state::<AppState>().sessions.lock() {
            sessions.remove(&id);
        }
        let _ = app_handle.emit(
            "crawl-finished",
            CrawlFinished {
                crawl_id: id,
                status,
                error,
            },
        );
    });

    Ok(())
}

#[tauri::command]
fn cancel_crawl(state: State<'_, AppState>, crawl_id: String) -> Result<bool, String> {
    let registry = state
        .cancels
        .lock()
        .map_err(|_| "crawl registry poisoned".to_string())?;
    if let Some(token) = registry.get(&crawl_id) {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn pause_crawl(state: State<'_, AppState>, crawl_id: String) -> Result<bool, String> {
    let pauses = state
        .pauses
        .lock()
        .map_err(|_| "crawl registry poisoned".to_string())?;
    if let Some(flag) = pauses.get(&crawl_id) {
        flag.store(true, Ordering::Relaxed);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn export_csv(state: State<'_, AppState>, crawl_id: String) -> Result<String, String> {
    let path = state
        .data_dir
        .join("exports")
        .join(format!("{crawl_id}.csv"));
    state
        .store
        .export_csv(&crawl_id, &path)
        .map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn browser_path() -> Option<std::path::PathBuf> {
    std::env::var_os("CHROME_PATH")
        .map(std::path::PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| {
            [
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
                "/usr/bin/google-chrome",
                "/usr/bin/chromium",
            ]
            .iter()
            .map(std::path::PathBuf::from)
            .find(|path| path.exists())
        })
}

fn lighthouse_command(url: &str, output_path: &std::path::Path) -> tokio::process::Command {
    let sidecar = |name: &str| {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
            .filter(|path| path.exists())
    };
    let chrome = browser_path();
    let node = std::env::var_os("COQUI_NODE")
        .map(std::path::PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| sidecar(if cfg!(windows) { "node.exe" } else { "node" }));
    let cli = std::env::var_os("COQUI_LIGHTHOUSE_CLI")
        .map(std::path::PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| sidecar("lighthouse/lighthouse-cli/index.js"));
    let mut command = match (node, cli) {
        (Some(node), Some(cli)) => {
            let mut command = tokio::process::Command::new(node);
            command.arg(cli);
            command
        }
        _ => {
            let mut command =
                tokio::process::Command::new(if cfg!(windows) { "npx.cmd" } else { "npx" });
            command.arg("--yes").arg("lighthouse@12.8.2");
            command
        }
    };
    if let Some(chrome) = chrome {
        command.env("CHROME_PATH", chrome);
    }
    command
        .arg(url)
        .arg("--quiet")
        .arg("--output=json")
        .arg(format!("--output-path={}", output_path.display()))
        .arg("--only-categories=performance,accessibility,best-practices,seo")
        .arg("--chrome-flags=--headless=new --no-sandbox --disable-gpu")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    command
}

async fn lighthouse_summary(url: &str) -> Result<coqui_lighthouse::LighthouseSummary, String> {
    let output_path =
        std::env::temp_dir().join(format!("coqui-lighthouse-{}.json", uuid::Uuid::new_v4()));
    let mut command = lighthouse_command(url, &output_path);
    let output = tokio::time::timeout(std::time::Duration::from_secs(240), command.output())
        .await
        .map_err(|_| "Lighthouse timed out after 240 seconds".to_string())?
        .map_err(|error| {
            format!("could not run npx: {error}. Install Node.js or add it to PATH.")
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let snippet: String = stderr.chars().take(500).collect();
        return Err(format!("Lighthouse failed: {snippet}"));
    }
    let report = std::fs::read_to_string(&output_path)
        .map_err(|error| format!("Lighthouse produced no report: {error}"))?;
    let _ = std::fs::remove_file(&output_path);
    coqui_lighthouse::summarize(&report).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_screenshot(
    state: State<'_, AppState>,
    crawl_id: String,
    url_id: i64,
) -> Result<Option<String>, String> {
    use base64::Engine as _;
    let data = state
        .store
        .get_screenshot(&crawl_id, url_id)
        .map_err(|error| error.to_string())?;
    Ok(data.map(|bytes| {
        format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }))
}

#[tauri::command]
async fn run_lighthouse(
    state: State<'_, AppState>,
    crawl_id: String,
    url_id: i64,
) -> Result<coqui_lighthouse::LighthouseSummary, String> {
    let record = state
        .store
        .get_url(&crawl_id, url_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "URL not found".to_string())?;
    let summary = lighthouse_summary(&record.url).await?;
    let json = serde_json::to_string(&summary).map_err(|error| error.to_string())?;
    state
        .store
        .save_lighthouse(&crawl_id, url_id, &json)
        .map_err(|error| error.to_string())?;
    Ok(summary)
}

#[tauri::command]
async fn run_lighthouse_sample(
    state: State<'_, AppState>,
    crawl_id: String,
    sample: u32,
) -> Result<Vec<coqui_lighthouse::LighthouseSummary>, String> {
    let urls = state
        .store
        .sample_urls(&crawl_id, sample)
        .map_err(|error| error.to_string())?;
    if urls.is_empty() {
        return Err("No 2xx HTML URLs available to sample".to_string());
    }
    let mut summaries = Vec::new();
    for record in urls {
        let summary = lighthouse_summary(&record.url).await?;
        let json = serde_json::to_string(&summary).map_err(|error| error.to_string())?;
        state
            .store
            .save_lighthouse(&crawl_id, record.id, &json)
            .map_err(|error| error.to_string())?;
        summaries.push(summary);
    }
    Ok(summaries)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionProbe {
    status: u16,
    ms: u64,
    final_url: String,
    content_type: Option<String>,
}

#[tauri::command]
async fn test_connection(seed: String, headers: Vec<String>) -> Result<ConnectionProbe, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Coqui/0.1 connection test")
        .build()
        .map_err(|error| error.to_string())?;
    let mut request = client.get(&seed);
    for header in headers {
        if let Some((name, value)) = header.split_once(':') {
            request = request.header(name.trim(), value.trim());
        }
    }
    let started = std::time::Instant::now();
    let response = request.send().await.map_err(|error| error.to_string())?;
    let ms = started.elapsed().as_millis() as u64;
    Ok(ConnectionProbe {
        status: response.status().as_u16(),
        ms,
        final_url: response.url().to_string(),
        content_type: response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
    })
}

#[tauri::command]
fn export_html_report(
    state: State<'_, AppState>,
    crawl_id: String,
    pattern: Option<String>,
    report: Option<String>,
    top: Option<u32>,
    status: Option<u16>,
) -> Result<String, String> {
    let dir = state.data_dir.join("exports");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let crawl = state
        .store
        .get_crawl(&crawl_id)
        .map_err(|error| error.to_string())?;
    let filename = coqui_core::export_filename(
        pattern.as_deref().unwrap_or(""),
        &seed_domain(&crawl.seed),
        "report",
        &crawl_id,
        now_ms(),
        "html",
    );
    let path = dir.join(filename);
    let options = coqui_report::ReportOptions {
        top: top.map(|value| value as usize),
        status,
    };
    coqui_report::write_report_with(
        &state.store,
        &crawl_id,
        &path,
        report.as_deref().unwrap_or("summary"),
        &options,
    )?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
async fn export_pdf_report(
    state: State<'_, AppState>,
    crawl_id: String,
    pattern: Option<String>,
    report: Option<String>,
    top: Option<u32>,
    status: Option<u16>,
) -> Result<String, String> {
    let browser =
        browser_path().ok_or_else(|| "No Chrome or Edge found; set CHROME_PATH".to_string())?;
    let dir = state.data_dir.join("exports");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let crawl = state
        .store
        .get_crawl(&crawl_id)
        .map_err(|error| error.to_string())?;
    let filename = coqui_core::export_filename(
        pattern.as_deref().unwrap_or(""),
        &seed_domain(&crawl.seed),
        "report",
        &crawl_id,
        now_ms(),
        "pdf",
    );
    let pdf = dir.join(filename);
    let html = pdf.with_extension("html");
    let options = coqui_report::ReportOptions {
        top: top.map(|value| value as usize),
        status,
    };
    coqui_report::write_report_with(
        &state.store,
        &crawl_id,
        &html,
        report.as_deref().unwrap_or("summary"),
        &options,
    )?;
    let location = format!("file:///{}", html.to_string_lossy().replace('\\', "/"));
    let mut command = tokio::process::Command::new(browser);
    command
        .arg("--headless=new")
        .arg("--disable-gpu")
        .arg("--no-pdf-header-footer")
        .arg(format!("--print-to-pdf={}", pdf.display()))
        .arg(location);
    let status = tokio::time::timeout(std::time::Duration::from_secs(60), command.status())
        .await
        .map_err(|_| "PDF generation timed out after 60 seconds".to_string())?
        .map_err(|error| format!("could not run Chrome/Edge: {error}"))?;
    let _ = std::fs::remove_file(&html);
    if !status.success() {
        return Err("Chrome/Edge failed to print the report".to_string());
    }
    Ok(pdf.to_string_lossy().to_string())
}

#[tauri::command]
fn export_catalog() -> Vec<coqui_core::ExportDefinition> {
    coqui_core::export_catalog()
}

#[tauri::command]
fn export_view(
    state: State<'_, AppState>,
    crawl_id: String,
    view: String,
    issue: Option<String>,
    columns: Vec<String>,
    format: Option<String>,
    pattern: Option<String>,
    skip_empty: Option<bool>,
) -> Result<String, String> {
    let dir = state.data_dir.join("exports");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    if view == "sitemap-bundle" {
        let target = dir.join(format!("{crawl_id}-sitemaps-{}", now_ms()));
        state
            .store
            .export_sitemap_bundle(&crawl_id, &target, true)
            .map_err(|error| error.to_string())?;
        return Ok(target.to_string_lossy().to_string());
    }
    if view == "screenshots" {
        let target = dir.join(format!("{crawl_id}-screenshots-{}", now_ms()));
        state
            .store
            .export_screenshots(&crawl_id, &target)
            .map_err(|error| error.to_string())?;
        return Ok(target.to_string_lossy().to_string());
    }
    let format = format.unwrap_or_else(|| "csv".to_string());
    let crawl = state
        .store
        .get_crawl(&crawl_id)
        .map_err(|error| error.to_string())?;
    let domain = seed_domain(&crawl.seed);
    let filename = coqui_core::export_filename(
        pattern.as_deref().unwrap_or(""),
        &domain,
        &view,
        &crawl_id,
        now_ms(),
        &format,
    );
    let path = dir.join(filename);
    let rows = match (view.as_str(), format.as_str()) {
        ("issues", "xlsx") => state
            .store
            .export_issues_xlsx(&crawl_id, &path)
            .map_err(|error| error.to_string())?,
        ("links", "xlsx") => state
            .store
            .export_links_xlsx(&crawl_id, &path)
            .map_err(|error| error.to_string())?,
        ("urls", "xlsx") => state
            .store
            .export_urls_xlsx(&crawl_id, &path, &columns, issue.as_deref())
            .map_err(|error| error.to_string())?,
        ("headers", _) => state
            .store
            .export_headers_csv(&crawl_id, &path)
            .map_err(|error| error.to_string())?,
        ("sitemap", _) => state
            .store
            .export_sitemap(&crawl_id, &path)
            .map_err(|error| error.to_string())?,
        ("images", _) => state
            .store
            .export_images_csv(&crawl_id, &path)
            .map_err(|error| error.to_string())?,
        ("accessibility", _) => state
            .store
            .export_accessibility_csv(&crawl_id, &path)
            .map_err(|error| error.to_string())?,
        ("structured", _) => state
            .store
            .export_structured_data_csv(&crawl_id, &path)
            .map_err(|error| error.to_string())?,
        ("vitals", _) => state
            .store
            .export_vitals_csv(&crawl_id, &path)
            .map_err(|error| error.to_string())?,
        ("console", _) => state
            .store
            .export_console_csv(&crawl_id, &path)
            .map_err(|error| error.to_string())?,
        ("issues", _) => state
            .store
            .export_issues_csv(&crawl_id, &path)
            .map_err(|error| error.to_string())?,
        ("links", _) => state
            .store
            .export_links_csv(&crawl_id, &path)
            .map_err(|error| error.to_string())?,
        _ => state
            .store
            .export_urls_csv(&crawl_id, &path, &columns, issue.as_deref())
            .map_err(|error| error.to_string())?,
    };
    if skip_empty.unwrap_or(false) && rows == 0 {
        let _ = std::fs::remove_file(&path);
        return Ok(String::new());
    }
    Ok(path.to_string_lossy().to_string())
}

fn seed_domain(seed: &str) -> String {
    seed.split("://")
        .nth(1)
        .unwrap_or(seed)
        .split('/')
        .next()
        .unwrap_or(seed)
        .to_string()
}

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let store = Store::open(data_dir.join("coqui.db"))?;
            if let Ok(count) = store.pause_interrupted_crawls() {
                if count > 0 {
                    tracing::info!(count, "marked interrupted crawls as paused");
                }
            }
            app.manage(AppState {
                store: Arc::new(store),
                cancels: Mutex::new(HashMap::new()),
                pauses: Mutex::new(HashMap::new()),
                sessions: Mutex::new(HashMap::new()),
                data_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_crawl,
            get_crawl,
            list_urls,
            get_issue_counts,
            start_crawl,
            cancel_crawl,
            pause_crawl,
            export_csv,
            get_screenshot,
            run_lighthouse,
            run_lighthouse_sample,
            test_connection,
            export_catalog,
            export_html_report,
            export_pdf_report,
            export_view
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coqui");
}
