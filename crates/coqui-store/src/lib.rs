//! SQLite-backed crawl storage.
//!
//! One database file is a portable crawl workspace. The schema is versioned with
//! `PRAGMA user_version`; migrations are additive and run on open.

use coqui_core::{
    CrawlCheckpoint, CrawlConfig, CrawlStatus, CrawlSummary, Issue, IssueCount, IssueSeverity,
    LinkRecord, NewUrl, UrlPage, UrlRecord, now_ms,
};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

pub const SCHEMA_VERSION: i32 = 9;

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS crawls (
    id           TEXT PRIMARY KEY,
    seed         TEXT NOT NULL,
    seed_kind    TEXT NOT NULL DEFAULT 'url',
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
    h1_count          INTEGER NOT NULL DEFAULT 0,
    canonical         TEXT,
    meta_robots       TEXT,
    redirect_target   TEXT,
    redirect_chain    TEXT,
    word_count        INTEGER,
    depth             INTEGER NOT NULL,
    response_time_ms  INTEGER,
    size_bytes        INTEGER,
    error             TEXT,
    blocked           INTEGER NOT NULL DEFAULT 0,
    hreflang_json     TEXT,
    pagination_next   TEXT,
    pagination_prev   TEXT,
    structured_data_json TEXT,
    console_json      TEXT,
    axe_json          TEXT,
    discovered_at     INTEGER NOT NULL,
    UNIQUE (crawl_id, url)
);

CREATE INDEX IF NOT EXISTS idx_urls_crawl_id ON urls (crawl_id, id);
CREATE INDEX IF NOT EXISTS idx_urls_status ON urls (crawl_id, status);

CREATE TABLE IF NOT EXISTS url_map (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    crawl_id TEXT NOT NULL,
    url      TEXT NOT NULL,
    UNIQUE (crawl_id, url)
);

CREATE TABLE IF NOT EXISTS links (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    crawl_id   TEXT NOT NULL,
    from_id    INTEGER NOT NULL,
    to_id      INTEGER NOT NULL,
    anchor     TEXT NOT NULL DEFAULT '',
    rel        TEXT,
    internal   INTEGER NOT NULL
);



CREATE TABLE IF NOT EXISTS issues (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    crawl_id   TEXT NOT NULL,
    url        TEXT NOT NULL,
    rule       TEXT NOT NULL,
    severity   TEXT NOT NULL,
    detail     TEXT,
    UNIQUE (crawl_id, url, rule)
);

CREATE INDEX IF NOT EXISTS idx_issues_crawl_rule ON issues (crawl_id, rule);

CREATE TABLE IF NOT EXISTS screenshots (
    crawl_id   TEXT NOT NULL,
    url_id     INTEGER NOT NULL,
    data       BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (crawl_id, url_id)
);

CREATE TABLE IF NOT EXISTS checkpoints (
    crawl_id   TEXT PRIMARY KEY,
    state_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
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
    ids: Mutex<std::collections::HashMap<(String, String), i64>>,
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
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            ids: Mutex::new(std::collections::HashMap::new()),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| StoreError::Poisoned)
    }

    pub fn create_crawl(
        &self,
        id: &str,
        seed: &str,
        seed_kind: &str,
        config: &CrawlConfig,
        created_at: i64,
    ) -> Result<CrawlSummary> {
        let config_json = serde_json::to_string(config)?;
        self.lock()?.execute(
            "INSERT INTO crawls (id, seed, seed_kind, status, config_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                seed,
                seed_kind,
                CrawlStatus::Pending.as_str(),
                config_json,
                created_at
            ],
        )?;
        self.get_crawl(id)
    }

    pub fn get_crawl(&self, id: &str) -> Result<CrawlSummary> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, seed, seed_kind, status, config_json, created_at, started_at, finished_at,
                    (SELECT COUNT(*) FROM urls WHERE urls.crawl_id = crawls.id),
                    (SELECT COUNT(*) FROM urls WHERE urls.crawl_id = crawls.id AND urls.blocked = 1)
             FROM crawls WHERE id = ?1",
        )?;
        let summary = stmt
            .query_row(params![id], |row| {
                let config_json: String = row.get(4)?;
                Ok(CrawlSummary {
                    id: row.get(0)?,
                    seed: row.get(1)?,
                    seed_kind: row.get(2)?,
                    status: CrawlStatus::parse(&row.get::<_, String>(3)?),
                    config: serde_json::from_str(&config_json).unwrap_or_default(),
                    created_at: row.get(5)?,
                    started_at: row.get(6)?,
                    finished_at: row.get(7)?,
                    total_urls: row.get(8)?,
                    blocked_urls: row.get(9)?,
                })
            })
            .optional()?;
        summary.ok_or_else(|| StoreError::NotFound(id.to_string()))
    }

    pub fn latest_crawl(&self) -> Result<Option<CrawlSummary>> {
        let id: Option<String> = {
            let conn = self.lock()?;
            conn.query_row(
                "SELECT id FROM crawls ORDER BY rowid DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?
        };
        match id {
            Some(id) => Ok(Some(self.get_crawl(&id)?)),
            None => Ok(None),
        }
    }

    pub fn mark_running(&self, id: &str, started_at: i64) -> Result<()> {
        self.lock()?.execute(
            "UPDATE crawls SET status = ?2, started_at = ?3 WHERE id = ?1",
            params![id, CrawlStatus::Running.as_str(), started_at],
        )?;
        Ok(())
    }

    /// Mark crawls that were running when the process died as paused, so they can resume.
    pub fn pause_interrupted_crawls(&self) -> Result<usize> {
        let changed = self.lock()?.execute(
            "UPDATE crawls SET status = ?1 WHERE status = ?2",
            params![CrawlStatus::Paused.as_str(), CrawlStatus::Running.as_str()],
        )?;
        Ok(changed)
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
        let redirect_chain = if record.redirect_chain.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&record.redirect_chain)?)
        };
        let changed = self.lock()?.execute(
            "INSERT OR IGNORE INTO urls
                (crawl_id, url, status, content_type, title, meta_description, h1, h1_count,
                 canonical, meta_robots, redirect_target, redirect_chain, word_count, depth,
                 response_time_ms, size_bytes, error, blocked, hreflang_json, pagination_next,
                 pagination_prev, structured_data_json, console_json, axe_json, discovered_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                     ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
            params![
                record.crawl_id,
                record.url,
                record.status,
                record.content_type,
                record.title,
                record.meta_description,
                record.h1,
                record.h1_count,
                record.canonical,
                record.meta_robots,
                record.redirect_target,
                redirect_chain,
                record.word_count,
                record.depth,
                response_time_ms,
                size_bytes,
                record.error,
                i64::from(record.blocked),
                serde_json::to_string(&record.hreflang).unwrap_or_else(|_| "[]".into()),
                record.pagination_next,
                record.pagination_prev,
                serde_json::to_string(&record.structured_data)
                    .unwrap_or_else(|_| "{\"types\":[],\"errors\":[]}".into()),
                serde_json::to_string(&record.console_errors).unwrap_or_else(|_| "[]".into()),
                serde_json::to_string(&record.axe).unwrap_or_else(|_| "[]".into()),
                record.discovered_at,
            ],
        )?;
        Ok(changed > 0)
    }

    pub fn url_id(&self, crawl_id: &str, url: &str) -> Result<Option<i64>> {
        let id = self
            .lock()?
            .query_row(
                "SELECT id FROM urls WHERE crawl_id = ?1 AND url = ?2",
                params![crawl_id, url],
                |row| row.get(0),
            )
            .optional()?;
        Ok(id)
    }

    /// Resolve a URL to its id in the crawl's URL map, inserting it if needed.
    pub fn ensure_url_id(&self, crawl_id: &str, url: &str) -> Result<i64> {
        let key = (crawl_id.to_string(), url.to_string());
        if let Some(id) = self
            .ids
            .lock()
            .map_err(|_| StoreError::Poisoned)?
            .get(&key)
            .copied()
        {
            return Ok(id);
        }
        let id: i64 = {
            let conn = self.lock()?;
            conn.execute(
                "INSERT OR IGNORE INTO url_map (crawl_id, url) VALUES (?1, ?2)",
                params![crawl_id, url],
            )?;
            conn.query_row(
                "SELECT id FROM url_map WHERE crawl_id = ?1 AND url = ?2",
                params![crawl_id, url],
                |row| row.get(0),
            )?
        };
        self.ids
            .lock()
            .map_err(|_| StoreError::Poisoned)?
            .insert(key, id);
        Ok(id)
    }

    pub fn insert_link(
        &self,
        crawl_id: &str,
        from_id: i64,
        to_id: i64,
        anchor: Option<&str>,
        rel: Option<&str>,
        internal: bool,
    ) -> Result<()> {
        self.lock()?.execute(
            "INSERT INTO links (crawl_id, from_id, to_id, anchor, rel, internal)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                crawl_id,
                from_id,
                to_id,
                anchor.unwrap_or(""),
                rel,
                i64::from(internal)
            ],
        )?;
        Ok(())
    }

    pub fn insert_issue(&self, issue: &Issue) -> Result<bool> {
        let changed = self.lock()?.execute(
            "INSERT OR IGNORE INTO issues (crawl_id, url, rule, severity, detail)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                issue.crawl_id,
                issue.url,
                issue.rule,
                issue.severity.as_str(),
                issue.detail,
            ],
        )?;
        Ok(changed > 0)
    }

    pub fn clear_issues(&self, crawl_id: &str) -> Result<()> {
        self.lock()?
            .execute("DELETE FROM issues WHERE crawl_id = ?1", params![crawl_id])?;
        Ok(())
    }

    pub fn issue_counts(&self, crawl_id: &str) -> Result<Vec<IssueCount>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT rule, severity, COUNT(*) FROM issues WHERE crawl_id = ?1
             GROUP BY rule, severity ORDER BY COUNT(*) DESC, rule",
        )?;
        let rows = stmt
            .query_map(params![crawl_id], |row| {
                let severity: String = row.get(1)?;
                Ok(IssueCount {
                    rule: row.get(0)?,
                    severity: IssueSeverity::parse(&severity),
                    count: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn total_urls(&self, crawl_id: &str) -> Result<i64> {
        let count = self.lock()?.query_row(
            "SELECT COUNT(*) FROM urls WHERE crawl_id = ?1",
            params![crawl_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn list_urls(
        &self,
        crawl_id: &str,
        page: u32,
        page_size: u32,
        issue_rule: Option<&str>,
    ) -> Result<UrlPage> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 500);
        let offset = (page - 1) as i64 * page_size as i64;
        let filter = issue_rule.map(str::to_string);
        let conn = self.lock()?;
        let total = conn.query_row(
            "SELECT COUNT(*) FROM urls WHERE crawl_id = ?1
               AND (?2 IS NULL OR EXISTS (
                    SELECT 1 FROM issues
                    WHERE issues.crawl_id = urls.crawl_id AND issues.url = urls.url AND issues.rule = ?2))",
            params![crawl_id, filter],
            |row| row.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT id, crawl_id, url, status, content_type, title, meta_description, h1, h1_count,
                    canonical, meta_robots, redirect_target, redirect_chain, word_count, depth,
                    response_time_ms, size_bytes, error, discovered_at, blocked, hreflang_json,
                    pagination_next, pagination_prev, structured_data_json, console_json,
                    axe_json
             FROM urls WHERE crawl_id = ?1
               AND (?3 IS NULL OR EXISTS (
                    SELECT 1 FROM issues
                    WHERE issues.crawl_id = urls.crawl_id AND issues.url = urls.url AND issues.rule = ?3))
             ORDER BY id LIMIT ?2 OFFSET ?4",
        )?;
        let rows = stmt
            .query_map(
                params![crawl_id, page_size as i64, filter, offset],
                map_url_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(UrlPage {
            rows,
            total,
            page,
            page_size,
        })
    }

    /// Every URL of a crawl, used by post-crawl analysis. Callers should not use this for UI paging.
    pub fn all_urls(&self, crawl_id: &str) -> Result<Vec<UrlRecord>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, crawl_id, url, status, content_type, title, meta_description, h1, h1_count,
                    canonical, meta_robots, redirect_target, redirect_chain, word_count, depth,
                    response_time_ms, size_bytes, error, discovered_at, blocked, hreflang_json,
                    pagination_next, pagination_prev, structured_data_json, console_json,
                    axe_json
             FROM urls WHERE crawl_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![crawl_id], map_url_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn save_screenshot(&self, crawl_id: &str, url_id: i64, data: &[u8]) -> Result<()> {
        self.lock()?.execute(
            "INSERT OR REPLACE INTO screenshots (crawl_id, url_id, data, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![crawl_id, url_id, data, now_ms()],
        )?;
        Ok(())
    }

    pub fn get_screenshot(&self, crawl_id: &str, url_id: i64) -> Result<Option<Vec<u8>>> {
        let data = self
            .lock()?
            .query_row(
                "SELECT data FROM screenshots WHERE crawl_id = ?1 AND url_id = ?2",
                params![crawl_id, url_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(data)
    }

    pub fn all_links(&self, crawl_id: &str) -> Result<Vec<LinkRecord>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT f.url, t.url, l.anchor, l.rel, l.internal
             FROM links l
             JOIN url_map f ON f.id = l.from_id
             JOIN url_map t ON t.id = l.to_id
             WHERE l.crawl_id = ?1",
        )?;
        let rows = stmt
            .query_map(params![crawl_id], |row| {
                Ok(LinkRecord {
                    from_url: row.get(0)?,
                    to_url: row.get(1)?,
                    anchor: {
                        let anchor: String = row.get(2)?;
                        if anchor.is_empty() {
                            None
                        } else {
                            Some(anchor)
                        }
                    },
                    rel: row.get(3)?,
                    internal: row.get::<_, i64>(4)? != 0,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Write every URL row of a crawl to `path` as CSV. Returns the row count written.
    pub fn export_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let urls = self.all_urls(crawl_id)?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record([
            "url",
            "status",
            "content_type",
            "title",
            "meta_description",
            "h1",
            "h1_count",
            "canonical",
            "meta_robots",
            "redirect_target",
            "word_count",
            "depth",
            "response_time_ms",
            "size_bytes",
            "error",
            "discovered_at",
        ])?;
        for url in &urls {
            writer.write_record([
                url.url.clone(),
                url.status.map(|v| v.to_string()).unwrap_or_default(),
                url.content_type.clone().unwrap_or_default(),
                url.title.clone().unwrap_or_default(),
                url.meta_description.clone().unwrap_or_default(),
                url.h1.clone().unwrap_or_default(),
                url.h1_count.to_string(),
                url.canonical.clone().unwrap_or_default(),
                url.meta_robots.clone().unwrap_or_default(),
                url.redirect_target.clone().unwrap_or_default(),
                url.word_count.map(|v| v.to_string()).unwrap_or_default(),
                url.depth.to_string(),
                url.response_time_ms
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                url.size_bytes.map(|v| v.to_string()).unwrap_or_default(),
                url.error.clone().unwrap_or_default(),
                url.discovered_at.to_string(),
            ])?;
        }
        writer.flush()?;
        Ok(urls.len() as u64)
    }

    pub fn save_checkpoint(&self, crawl_id: &str, checkpoint: &CrawlCheckpoint) -> Result<()> {
        let json = serde_json::to_string(checkpoint)?;
        self.lock()?.execute(
            "INSERT INTO checkpoints (crawl_id, state_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(crawl_id) DO UPDATE SET state_json = excluded.state_json, updated_at = excluded.updated_at",
            params![crawl_id, json, coqui_core::now_ms()],
        )?;
        Ok(())
    }

    pub fn load_checkpoint(&self, crawl_id: &str) -> Result<Option<CrawlCheckpoint>> {
        let json: Option<String> = self
            .lock()?
            .query_row(
                "SELECT state_json FROM checkpoints WHERE crawl_id = ?1",
                params![crawl_id],
                |row| row.get(0),
            )
            .optional()?;
        match json {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    pub fn delete_checkpoint(&self, crawl_id: &str) -> Result<()> {
        self.lock()?.execute(
            "DELETE FROM checkpoints WHERE crawl_id = ?1",
            params![crawl_id],
        )?;
        Ok(())
    }

    /// Delete a crawl and all of its rows.
    pub fn delete_crawl(&self, id: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM issues WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM checkpoints WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM screenshots WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM links WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM url_map WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM urls WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM crawls WHERE id = ?1", params![id])?;
        Ok(())
    }
}

fn map_url_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<UrlRecord> {
    let chain: Option<String> = row.get(12)?;
    Ok(UrlRecord {
        id: row.get(0)?,
        crawl_id: row.get(1)?,
        url: row.get(2)?,
        status: row.get(3)?,
        content_type: row.get(4)?,
        title: row.get(5)?,
        meta_description: row.get(6)?,
        h1: row.get(7)?,
        h1_count: row.get(8)?,
        canonical: row.get(9)?,
        meta_robots: row.get(10)?,
        redirect_target: row.get(11)?,
        redirect_chain: chain
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default(),
        word_count: row.get(13)?,
        depth: row.get(14)?,
        response_time_ms: row.get::<_, Option<i64>>(15)?.map(|value| value as u64),
        size_bytes: row.get::<_, Option<i64>>(16)?.map(|value| value as u64),
        error: row.get(17)?,
        blocked: row.get::<_, i64>(19)? != 0,
        hreflang: row
            .get::<_, Option<String>>(20)?
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default(),
        pagination_next: row.get(21)?,
        pagination_prev: row.get(22)?,
        structured_data: row
            .get::<_, Option<String>>(23)?
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default(),
        console_errors: row
            .get::<_, Option<String>>(24)?
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default(),
        axe: row
            .get::<_, Option<String>>(25)?
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default(),
        discovered_at: row.get(18)?,
    })
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1"),
        params![column],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 4 && has_column(conn, "links", "from_url")? {
        conn.execute_batch(
            "ALTER TABLE links RENAME TO links_v3;
             DROP INDEX IF EXISTS idx_links_crawl_from;
             DROP INDEX IF EXISTS idx_links_crawl_to;
             CREATE TABLE links (
                 id       INTEGER PRIMARY KEY AUTOINCREMENT,
                 crawl_id TEXT NOT NULL,
                 from_id  INTEGER NOT NULL,
                 to_id    INTEGER NOT NULL,
                 anchor   TEXT NOT NULL DEFAULT '',
                 rel      TEXT,
                 internal INTEGER NOT NULL
             );
             INSERT OR IGNORE INTO url_map (crawl_id, url) SELECT crawl_id, url FROM urls;
             INSERT OR IGNORE INTO url_map (crawl_id, url) SELECT crawl_id, from_url FROM links_v3;
             INSERT OR IGNORE INTO url_map (crawl_id, url) SELECT crawl_id, to_url FROM links_v3;
             INSERT INTO links (crawl_id, from_id, to_id, anchor, rel, internal)
                 SELECT l.crawl_id, f.id, t.id, COALESCE(l.anchor, ''), l.rel, l.internal
                 FROM links_v3 l
                 JOIN url_map f ON f.crawl_id = l.crawl_id AND f.url = l.from_url
                 JOIN url_map t ON t.crawl_id = l.crawl_id AND t.url = l.to_url;
             DROP TABLE links_v3;
             CREATE INDEX IF NOT EXISTS idx_links_crawl_from ON links (crawl_id, from_id);
             CREATE INDEX IF NOT EXISTS idx_links_crawl_to ON links (crawl_id, to_id);",
        )?;
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_links_crawl_from ON links (crawl_id, from_id);
         CREATE INDEX IF NOT EXISTS idx_links_crawl_to ON links (crawl_id, to_id);",
    )?;
    if version < 9 && !has_column(conn, "urls", "axe_json")? {
        conn.execute("ALTER TABLE urls ADD COLUMN axe_json TEXT", [])?;
    }
    if version < 8 {
        if !has_column(conn, "urls", "console_json")? {
            conn.execute("ALTER TABLE urls ADD COLUMN console_json TEXT", [])?;
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS screenshots (
                crawl_id   TEXT NOT NULL,
                url_id     INTEGER NOT NULL,
                data       BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (crawl_id, url_id)
            );",
        )?;
    }
    if version < 7 && !has_column(conn, "urls", "structured_data_json")? {
        conn.execute("ALTER TABLE urls ADD COLUMN structured_data_json TEXT", [])?;
    }
    if version < 6 {
        for (column, definition) in [
            ("hreflang_json", "TEXT"),
            ("pagination_next", "TEXT"),
            ("pagination_prev", "TEXT"),
        ] {
            if !has_column(conn, "urls", column)? {
                conn.execute(
                    &format!("ALTER TABLE urls ADD COLUMN {column} {definition}"),
                    [],
                )?;
            }
        }
        conn.execute("DROP INDEX IF EXISTS idx_links_crawl_to", [])?;
    }
    if version < 5 && !has_column(conn, "urls", "blocked")? {
        conn.execute(
            "ALTER TABLE urls ADD COLUMN blocked INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if version < 3 && !has_column(conn, "crawls", "seed_kind")? {
        conn.execute(
            "ALTER TABLE crawls ADD COLUMN seed_kind TEXT NOT NULL DEFAULT 'url'",
            [],
        )?;
    }
    if version < 2 {
        let additions = [
            ("h1_count", "INTEGER NOT NULL DEFAULT 0"),
            ("canonical", "TEXT"),
            ("meta_robots", "TEXT"),
            ("redirect_target", "TEXT"),
            ("redirect_chain", "TEXT"),
        ];
        for (column, sql_type) in additions {
            if !has_column(conn, "urls", column)? {
                conn.execute(
                    &format!("ALTER TABLE urls ADD COLUMN {column} {sql_type}"),
                    [],
                )?;
            }
        }
    }
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use coqui_core::{Issue, IssueSeverity, now_ms};

    fn sample_record(crawl_id: &str, url: &str) -> NewUrl {
        NewUrl {
            crawl_id: crawl_id.to_string(),
            url: url.to_string(),
            status: Some(200),
            blocked: false,

            hreflang: Vec::new(),

            pagination_next: None,

            pagination_prev: None,

            structured_data: Default::default(),
            console_errors: Vec::new(),
            axe: Vec::new(),
            content_type: Some("text/html".into()),
            title: Some("Example".into()),
            meta_description: None,
            h1: Some("Hello".into()),
            h1_count: 1,
            canonical: Some(url.to_string()),
            meta_robots: None,
            redirect_target: None,
            redirect_chain: Vec::new(),
            word_count: Some(420),
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
                "url",
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
        assert!(
            !store
                .insert_url(&sample_record(id, "https://example.com/about"))
                .unwrap()
        );

        let page = store.list_urls(id, 1, 1, None).unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].url, "https://example.com/");
        assert_eq!(
            page.rows[0].canonical.as_deref(),
            Some("https://example.com/")
        );

        let page2 = store.list_urls(id, 2, 1, None).unwrap();
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
                "url",
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
    fn links_and_issue_filters() {
        let store = Store::open_in_memory().unwrap();
        let id = "crawl-links";
        store
            .create_crawl(
                id,
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&sample_record(id, "https://example.com/"))
            .unwrap();
        store
            .insert_url(&sample_record(id, "https://example.com/missing"))
            .unwrap();
        let from = store.ensure_url_id(id, "https://example.com/").unwrap();
        let to = store
            .ensure_url_id(id, "https://example.com/missing")
            .unwrap();
        store
            .insert_link(id, from, to, Some("Missing"), None, true)
            .unwrap();
        assert_eq!(store.all_links(id).unwrap().len(), 1);

        store
            .insert_issue(&Issue {
                crawl_id: id.into(),
                url: "https://example.com/".into(),
                rule: "missing_meta_description".into(),
                severity: IssueSeverity::Warning,
                detail: None,
            })
            .unwrap();
        let counts = store.issue_counts(id).unwrap();
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[0].count, 1);

        let filtered = store
            .list_urls(id, 1, 10, Some("missing_meta_description"))
            .unwrap();
        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.rows[0].url, "https://example.com/");
    }

    #[test]
    fn exports_csv() {
        let store = Store::open_in_memory().unwrap();
        let id = "crawl-3";
        store
            .create_crawl(
                id,
                "https://example.com/",
                "url",
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
        assert!(text.contains("redirect_target"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn checkpoint_round_trip() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.load_checkpoint("c").unwrap().is_none());
        let checkpoint = CrawlCheckpoint {
            frontier: vec!["https://example.com/a".into()],
            seen: vec!["https://example.com/".into()],
            hosts: vec!["example.com".into()],
            depth: 1,
            crawled: 3,
            discovered: 4,
            failed: 0,
            redirected: 1,
            blocked_by_robots: 2,
            blocked: 0,
        };
        store.save_checkpoint("c", &checkpoint).unwrap();
        let loaded = store.load_checkpoint("c").unwrap().unwrap();
        assert_eq!(loaded.frontier, checkpoint.frontier);
        assert_eq!(loaded.blocked_by_robots, 2);
        assert_eq!(loaded.hosts, vec!["example.com".to_string()]);
        store.delete_checkpoint("c").unwrap();
        assert!(store.load_checkpoint("c").unwrap().is_none());
    }

    #[test]
    fn migrates_v1_database() {
        let path = std::env::temp_dir().join(format!("coqui-v1-{}.db", uuid::Uuid::new_v4()));
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE crawls (id TEXT PRIMARY KEY, seed TEXT NOT NULL, status TEXT NOT NULL,
                    config_json TEXT NOT NULL, created_at INTEGER NOT NULL, started_at INTEGER, finished_at INTEGER);
                 CREATE TABLE urls (id INTEGER PRIMARY KEY AUTOINCREMENT, crawl_id TEXT NOT NULL,
                    url TEXT NOT NULL, status INTEGER, content_type TEXT, title TEXT,
                    meta_description TEXT, h1 TEXT, word_count INTEGER, depth INTEGER NOT NULL,
                    response_time_ms INTEGER, size_bytes INTEGER, error TEXT, discovered_at INTEGER NOT NULL,
                    UNIQUE (crawl_id, url));
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.get_crawl("nope").unwrap_err().to_string(),
            "crawl not found: nope"
        );
        store
            .create_crawl(
                "v1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&sample_record("v1", "https://example.com/"))
            .unwrap();
        let row = &store.list_urls("v1", 1, 10, None).unwrap().rows[0];
        assert_eq!(row.h1_count, 1);
        std::fs::remove_file(path).ok();
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    #[test]
    fn migrates_v3_string_links_to_ids() {
        let path = std::env::temp_dir().join(format!("coqui-v3-{}.db", uuid::Uuid::new_v4()));
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE crawls (id TEXT PRIMARY KEY, seed TEXT NOT NULL,
                    seed_kind TEXT NOT NULL DEFAULT 'url', status TEXT NOT NULL,
                    config_json TEXT NOT NULL, created_at INTEGER NOT NULL,
                    started_at INTEGER, finished_at INTEGER);
                 CREATE TABLE urls (id INTEGER PRIMARY KEY AUTOINCREMENT, crawl_id TEXT NOT NULL,
                    url TEXT NOT NULL, status INTEGER, content_type TEXT, title TEXT,
                    meta_description TEXT, h1 TEXT, h1_count INTEGER NOT NULL DEFAULT 0,
                    canonical TEXT, meta_robots TEXT, redirect_target TEXT, redirect_chain TEXT,
                    word_count INTEGER, depth INTEGER NOT NULL, response_time_ms INTEGER,
                    size_bytes INTEGER, error TEXT, discovered_at INTEGER NOT NULL,
                    UNIQUE (crawl_id, url));
                 CREATE TABLE links (id INTEGER PRIMARY KEY AUTOINCREMENT, crawl_id TEXT NOT NULL,
                    from_url TEXT NOT NULL, to_url TEXT NOT NULL, anchor TEXT NOT NULL DEFAULT '',
                    rel TEXT, internal INTEGER NOT NULL,
                    UNIQUE (crawl_id, from_url, to_url, anchor));
                 PRAGMA user_version = 3;",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO crawls (id, seed, seed_kind, status, config_json, created_at)
                 VALUES ('c1', 'https://example.com/', 'url', 'completed', '{}', 1)",
                [],
            )
            .unwrap();
            for url in ["https://example.com/", "https://example.com/missing"] {
                conn.execute(
                    "INSERT INTO urls (crawl_id, url, status, depth, discovered_at)
                     VALUES ('c1', ?1, 200, 0, 1)",
                    params![url],
                )
                .unwrap();
            }
            conn.execute(
                "INSERT INTO links (crawl_id, from_url, to_url, anchor, internal)
                 VALUES ('c1', 'https://example.com/', 'https://example.com/missing', 'Missing', 1)",
                [],
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let links = store.all_links("c1").unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].from_url, "https://example.com/");
        assert_eq!(links[0].to_url, "https://example.com/missing");
        assert_eq!(links[0].anchor.as_deref(), Some("Missing"));

        // The migrated database must accept the new integer link API.
        let from = store.ensure_url_id("c1", "https://example.com/").unwrap();
        let to = store
            .ensure_url_id("c1", "https://example.com/missing")
            .unwrap();
        store
            .insert_link("c1", from, to, Some("Again"), None, true)
            .unwrap();
        assert_eq!(store.all_links("c1").unwrap().len(), 2);
        std::fs::remove_file(path).ok();
    }
}
