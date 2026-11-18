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
            export_csv
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coqui");
}
