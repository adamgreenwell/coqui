//! SQLite-backed crawl storage.
//!
//! One database file is a portable crawl workspace. The schema is versioned with
//! `PRAGMA user_version`; migrations are additive and run on open.

use coqui_core::{CrawlConfig, CrawlStatus, CrawlSummary, NewUrl, UrlPage, UrlRecord};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

pub const SCHEMA_VERSION: i32 = 1;

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS crawls (
    id           TEXT PRIMARY KEY,
    seed         TEXT NOT NULL,
    status       TEXT NOT NULL,
    config_json  TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    started_at   INTEGER,
    finished_at  INTEGER
);

CREATE TABLE IF NOT EXISTS urls (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    crawl_id          TEXT NOT NULL REFERENCES crawls(id) ON DELETE CASCADE,
    url               TEXT NOT NULL,
    status            INTEGER,
    content_type      TEXT,
    title             TEXT,
    meta_description  TEXT,
    h1                TEXT,
    word_count        INTEGER,
    depth             INTEGER NOT NULL,
    response_time_ms  INTEGER,
    size_bytes        INTEGER,
    error             TEXT,
    discovered_at     INTEGER NOT NULL,
    UNIQUE (crawl_id, url)
);

CREATE INDEX IF NOT EXISTS idx_urls_crawl_id ON urls (crawl_id, id);
CREATE INDEX IF NOT EXISTS idx_urls_status ON urls (crawl_id, status);
"#;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("store lock poisoned")]
    Poisoned,
    #[error("crawl not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// A crawl store. Cheap to clone through `Arc`; writes are serialised by SQLite anyway.
pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::from_connection(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| StoreError::Poisoned)
    }

    pub fn create_crawl(
        &self,
        id: &str,
        seed: &str,
        config: &CrawlConfig,
        created_at: i64,
    ) -> Result<CrawlSummary> {
        let config_json = serde_json::to_string(config)?;
        self.lock()?.execute(
            "INSERT INTO crawls (id, seed, status, config_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, seed, CrawlStatus::Pending.as_str(), config_json, created_at],
        )?;
        self.get_crawl(id)
    }

    pub fn get_crawl(&self, id: &str) -> Result<CrawlSummary> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, seed, status, config_json, created_at, started_at, finished_at,
                    (SELECT COUNT(*) FROM urls WHERE urls.crawl_id = crawls.id)
             FROM crawls WHERE id = ?1",
        )?;
        let summary = stmt
            .query_row(params![id], |row| {
                let config_json: String = row.get(3)?;
                Ok(CrawlSummary {
                    id: row.get(0)?,
                    seed: row.get(1)?,
                    status: CrawlStatus::parse(&row.get::<_, String>(2)?),
                    config: serde_json::from_str(&config_json).unwrap_or_default(),
                    created_at: row.get(4)?,
                    started_at: row.get(5)?,
                    finished_at: row.get(6)?,
                    total_urls: row.get(7)?,
                })
            })
            .optional()?;
        summary.ok_or_else(|| StoreError::NotFound(id.to_string()))
    }

    pub fn mark_running(&self, id: &str, started_at: i64) -> Result<()> {
        self.lock()?.execute(
            "UPDATE crawls SET status = ?2, started_at = ?3 WHERE id = ?1",
            params![id, CrawlStatus::Running.as_str(), started_at],
        )?;
        Ok(())
    }

    pub fn mark_finished(&self, id: &str, status: CrawlStatus, finished_at: i64) -> Result<()> {
        self.lock()?.execute(
            "UPDATE crawls SET status = ?2, finished_at = ?3 WHERE id = ?1",
            params![id, status.as_str(), finished_at],
        )?;
        Ok(())
    }

    /// Insert a URL. Returns `true` when a new row was created.
    pub fn insert_url(&self, record: &NewUrl) -> Result<bool> {
        let response_time_ms = record.response_time_ms.map(|value| value as i64);
        let size_bytes = record.size_bytes.map(|value| value as i64);
        let changed = self.lock()?.execute(
            "INSERT OR IGNORE INTO urls
                (crawl_id, url, status, content_type, title, meta_description, h1, word_count,
                 depth, response_time_ms, size_bytes, error, discovered_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                record.crawl_id,
                record.url,
                record.status,
                record.content_type,
                record.title,
                record.meta_description,
                record.h1,
                record.word_count,
                record.depth,
                response_time_ms,
                size_bytes,
                record.error,
                record.discovered_at,
            ],
        )?;
        Ok(changed > 0)
    }

    pub fn total_urls(&self, crawl_id: &str) -> Result<i64> {
        let count = self.lock()?.query_row(
            "SELECT COUNT(*) FROM urls WHERE crawl_id = ?1",
            params![crawl_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn list_urls(&self, crawl_id: &str, page: u32, page_size: u32) -> Result<UrlPage> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 500);
        let offset = (page - 1) as i64 * page_size as i64;
        let conn = self.lock()?;
        let total = conn.query_row(
            "SELECT COUNT(*) FROM urls WHERE crawl_id = ?1",
            params![crawl_id],
            |row| row.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT id, crawl_id, url, status, content_type, title, meta_description, h1,
                    word_count, depth, response_time_ms, size_bytes, error, discovered_at
             FROM urls WHERE crawl_id = ?1 ORDER BY id LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt
            .query_map(params![crawl_id, page_size as i64, offset], |row| {
                Ok(UrlRecord {
                    id: row.get(0)?,
                    crawl_id: row.get(1)?,
                    url: row.get(2)?,
                    status: row.get(3)?,
                    content_type: row.get(4)?,
                    title: row.get(5)?,
                    meta_description: row.get(6)?,
                    h1: row.get(7)?,
                    word_count: row.get(8)?,
                    depth: row.get(9)?,
                    response_time_ms: row.get::<_, Option<i64>>(10)?.map(|value| value as u64),
                    size_bytes: row.get::<_, Option<i64>>(11)?.map(|value| value as u64),
                    error: row.get(12)?,
                    discovered_at: row.get(13)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(UrlPage {
            rows,
            total,
            page,
            page_size,
        })
    }

    /// Write every URL row of a crawl to `path` as CSV. Returns the row count written.
    pub fn export_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT url, status, content_type, title, meta_description, h1, word_count, depth,
                    response_time_ms, size_bytes, error, discovered_at
             FROM urls WHERE crawl_id = ?1 ORDER BY id",
        )?;
        let mut reader = stmt.query(params![crawl_id])?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record([
            "url",
            "status",
            "content_type",
            "title",
            "meta_description",
            "h1",
            "word_count",
            "depth",
            "response_time_ms",
            "size_bytes",
            "error",
            "discovered_at",
        ])?;
        let mut written = 0u64;
        while let Some(row) = reader.next()? {
            let record: Vec<String> = (0..12)
                .map(|i| {
                    row.get::<_, Option<String>>(i)
                        .ok()
                        .flatten()
                        .or_else(|| {
                            row.get::<_, Option<i64>>(i)
                                .ok()
                                .flatten()
                                .map(|v| v.to_string())
                        })
                        .or_else(|| {
                            row.get::<_, Option<u32>>(i)
                                .ok()
                                .flatten()
                                .map(|v| v.to_string())
                        })
                        .unwrap_or_default()
                })
                .collect();
            writer.write_record(record)?;
            written += 1;
        }
        writer.flush()?;
        Ok(written)
    }

    /// Delete a crawl and all of its rows.
    pub fn delete_crawl(&self, id: &str) -> Result<()> {
        self.lock()?
            .execute("DELETE FROM crawls WHERE id = ?1", params![id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coqui_core::now_ms;

    fn sample_record(crawl_id: &str, url: &str) -> NewUrl {
        NewUrl {
            crawl_id: crawl_id.to_string(),
            url: url.to_string(),
            status: Some(200),
            content_type: Some("text/html".into()),
            title: Some("Example".into()),
            meta_description: None,
            h1: Some("Hello".into()),
            word_count: Some(42),
            depth: 0,
            response_time_ms: Some(12),
            size_bytes: Some(1024),
            error: None,
            discovered_at: now_ms(),
        }
    }

    #[test]
    fn crawl_round_trip_and_paging() {
        let store = Store::open_in_memory().unwrap();
        let id = "crawl-1";
        store
            .create_crawl(
                id,
                "https://example.com/",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        assert!(
            store
                .insert_url(&sample_record(id, "https://example.com/"))
                .unwrap()
        );
        assert!(
            store
                .insert_url(&sample_record(id, "https://example.com/about"))
                .unwrap()
        );
        // Duplicate URL is ignored.
        assert!(
            !store
                .insert_url(&sample_record(id, "https://example.com/about"))
                .unwrap()
        );

        let page = store.list_urls(id, 1, 1).unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].url, "https://example.com/");

        let page2 = store.list_urls(id, 2, 1).unwrap();
        assert_eq!(page2.rows[0].url, "https://example.com/about");

        let summary = store.get_crawl(id).unwrap();
        assert_eq!(summary.total_urls, 2);
        assert_eq!(summary.status, CrawlStatus::Pending);
    }

    #[test]
    fn status_transitions() {
        let store = Store::open_in_memory().unwrap();
        let id = "crawl-2";
        store
            .create_crawl(
                id,
                "https://example.com/",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store.mark_running(id, now_ms()).unwrap();
        assert_eq!(store.get_crawl(id).unwrap().status, CrawlStatus::Running);
        store
            .mark_finished(id, CrawlStatus::Completed, now_ms())
            .unwrap();
        let summary = store.get_crawl(id).unwrap();
        assert_eq!(summary.status, CrawlStatus::Completed);
        assert!(summary.finished_at.is_some());
    }

    #[test]
    fn exports_csv() {
        let store = Store::open_in_memory().unwrap();
        let id = "crawl-3";
        store
            .create_crawl(
                id,
                "https://example.com/",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&sample_record(id, "https://example.com/"))
            .unwrap();
        let path = std::env::temp_dir().join(format!("coqui-export-{}.csv", uuid::Uuid::new_v4()));
        let written = store.export_csv(id, &path).unwrap();
        assert_eq!(written, 1);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("https://example.com/"));
        assert!(text.lines().next().unwrap().starts_with("url,status"));
        std::fs::remove_file(path).ok();
    }
}
