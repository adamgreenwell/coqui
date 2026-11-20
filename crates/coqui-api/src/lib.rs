//! Read-only HTTP API over a Coqui crawl database.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
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
        .route("/crawls/{id}/urls", get(list_urls))
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

#[derive(Deserialize)]
struct UrlQuery {
    page: Option<u32>,
    limit: Option<u32>,
    query: Option<String>,
}

async fn list_urls(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<UrlQuery>,
) -> Result<Json<UrlPage>, (StatusCode, String)> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(50).clamp(1, 500);
    let page_result = match params
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
    {
        Some(search) => state
            .store
            .list_urls_filtered(
                &id,
                page,
                limit,
                None,
                Some(search),
                None,
                None,
                None,
                None,
                None,
                None,
                false,
            )
            .map_err(internal)?,
        None => state
            .store
            .list_urls(&id, page, limit, None)
            .map_err(internal)?,
    };
    Ok(Json(page_result))
}

fn internal(error: coqui_store::StoreError) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
