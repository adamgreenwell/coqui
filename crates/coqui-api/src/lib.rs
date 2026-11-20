//! Read-only HTTP API over a Coqui crawl database.

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
    routing::get,
};
use coqui_core::{CrawlSummary, IssueCount, UrlPage};
use coqui_store::Store;
use serde::Deserialize;

#[derive(Clone)]
struct AppState {
    store: Arc<Store>,
}

/// Build the read-only API router for a store.
pub fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/crawls", get(list_crawls))
        .route("/crawls/{id}", get(get_crawl))
        .route("/crawls/{id}/issues", get(issue_counts))
        .route("/crawls/{id}/issues/{rule}", get(issue_rule))
        .route("/crawls/{id}/urls", get(list_urls))
        .route("/crawls/{id}/export/{family}", get(export_csv))
        .route("/crawls/{id}/report/{kind}", get(report))
        .with_state(AppState { store })
}

/// Serve the API on an address until the process exits.
pub async fn serve(store: Arc<Store>, addr: &str) -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|error| error.to_string())?;
    axum::serve(listener, router(store))
        .await
        .map_err(|error| error.to_string())
}

async fn health() -> &'static str {
    "ok"
}

async fn list_crawls(
    State(state): State<AppState>,
) -> Result<Json<Vec<CrawlSummary>>, (StatusCode, String)> {
    state.store.list_crawls().map(Json).map_err(internal)
}

async fn get_crawl(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CrawlSummary>, (StatusCode, String)> {
    state.store.get_crawl(&id).map(Json).map_err(internal)
}

async fn issue_counts(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<IssueCount>>, (StatusCode, String)> {
    state.store.issue_counts(&id).map(Json).map_err(internal)
}

async fn issue_rule(
    State(state): State<AppState>,
    Path((id, rule)): Path<(String, String)>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, String)> {
    let rows = state.store.issues_for_rule(&id, &rule).map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|(url_id, url, detail)| {
                serde_json::json!({"id": url_id, "url": url, "detail": detail})
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UrlQuery {
    page: Option<u32>,
    limit: Option<u32>,
    query: Option<String>,
    status: Option<u16>,
    status_class: Option<u16>,
    content_class: Option<String>,
    max_depth: Option<u32>,
    indexability: Option<String>,
    sort: Option<String>,
    descending: Option<bool>,
}

async fn list_urls(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<UrlQuery>,
) -> Result<Json<UrlPage>, (StatusCode, String)> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(50).clamp(1, 500);
    let search = params
        .query
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty());
    let page_result = state
        .store
        .list_urls_filtered(
            &id,
            page,
            limit,
            None,
            search,
            params.status,
            params.status_class,
            params.content_class.as_deref(),
            params.max_depth,
            params.indexability.as_deref(),
            params.sort.as_deref(),
            params.descending.unwrap_or(false),
        )
        .map_err(internal)?;
    Ok(Json(page_result))
}

async fn export_csv(
    State(state): State<AppState>,
    Path((id, family)): Path<(String, String)>,
) -> Result<Response, (StatusCode, String)> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let path =
        std::env::temp_dir().join(format!("coqui-export-{}-{stamp}.csv", std::process::id()));
    export_family(&state.store, &id, &family, &path)
        .map_err(|message| (StatusCode::BAD_REQUEST, message))?;
    let bytes = std::fs::read(&path).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("read export: {error}"),
        )
    })?;
    let _ = std::fs::remove_file(&path);
    Response::builder()
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{family}.csv\""),
        )
        .body(Body::from(bytes))
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

async fn report(
    State(state): State<AppState>,
    Path((id, kind)): Path<(String, String)>,
) -> Result<Response, (StatusCode, String)> {
    if !coqui_report::REPORTS
        .iter()
        .any(|(name, _, _)| *name == kind)
    {
        return Err((StatusCode::BAD_REQUEST, format!("unknown report '{kind}'")));
    }
    let html = coqui_report::render_report(&state.store, &id, &kind)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

fn export_family(
    store: &Store,
    crawl_id: &str,
    family: &str,
    path: &std::path::Path,
) -> Result<u64, String> {
    let result = match family {
        "urls" => store.export_urls_csv(crawl_id, path, &[], None),
        "issues" => store.export_issues_csv(crawl_id, path),
        "links" => store.export_links_csv(crawl_id, path),
        "headers" => store.export_headers_csv(crawl_id, path),
        "images" => store.export_images_csv(crawl_id, path),
        "a11y" => store.export_accessibility_csv(crawl_id, path),
        "structured" => store.export_structured_data_csv(crawl_id, path),
        "vitals" => store.export_vitals_csv(crawl_id, path),
        "console" => store.export_console_csv(crawl_id, path),
        "text" => store.export_text_csv(crawl_id, path),
        "markdown" => store.export_markdown_csv(crawl_id, path),
        "extractions" => store.export_extractions_csv(crawl_id, path),
        "custom-js" => store.export_custom_js_csv(crawl_id, path),
        other => return Err(format!("unknown export family '{other}'")),
    };
    result.map_err(|error| error.to_string())
}

fn internal(error: coqui_store::StoreError) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
