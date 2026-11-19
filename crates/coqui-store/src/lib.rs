//! SQLite-backed crawl storage.
//!
//! One database file is a portable crawl workspace. The schema is versioned with
//! `PRAGMA user_version`; migrations are additive and run on open.

use coqui_core::{
    AxeViolation, CrawlCheckpoint, CrawlConfig, CrawlStatus, CrawlSummary, Issue, IssueCount,
    IssueSeverity, LinkRecord, NewUrl, StructuredData, UrlPage, UrlRecord, WebVitals, now_ms,
};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

pub const SCHEMA_VERSION: i32 = 22;

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
    vitals_json       TEXT,
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

CREATE TABLE IF NOT EXISTS images (
    crawl_id TEXT NOT NULL,
    url_id   INTEGER NOT NULL,
    src      TEXT NOT NULL,
    alt      TEXT,
    width    INTEGER,
    height   INTEGER,
    PRIMARY KEY (crawl_id, url_id, src)
);

CREATE TABLE IF NOT EXISTS lighthouse_reports (
    crawl_id   TEXT NOT NULL,
    url_id     INTEGER NOT NULL,
    summary_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (crawl_id, url_id)
);

CREATE TABLE IF NOT EXISTS screenshot_files (
    crawl_id   TEXT NOT NULL,
    url_id     INTEGER NOT NULL,
    path       TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (crawl_id, url_id)
);

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

CREATE TABLE IF NOT EXISTS saved_views (
    name          TEXT PRIMARY KEY,
    search        TEXT,
    status        INTEGER,
    issue         TEXT,
    content_class TEXT,
    max_depth     INTEGER,
    indexability  TEXT,
    created_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS issue_suppressions (
    crawl_id TEXT NOT NULL,
    rule     TEXT NOT NULL,
    PRIMARY KEY (crawl_id, rule)
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
    #[error("xlsx error: {0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("store lock poisoned")]
    Poisoned,
    #[error("crawl not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// A crawl store. Cheap to clone through `Arc`; writes are serialised by SQLite anyway.
/// A saved grid view (name + filters).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedView {
    pub name: String,
    pub search: Option<String>,
    pub status: Option<u16>,
    pub issue: Option<String>,
    pub content_class: Option<String>,
    pub max_depth: Option<u32>,
    pub indexability: Option<String>,
}

/// A scheduled crawl.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleRecord {
    pub id: i64,
    pub seed_kind: String,
    pub seed: String,
    pub interval_seconds: i64,
    pub config_json: String,
    pub next_run_at: i64,
    pub last_run_at: Option<i64>,
    pub enabled: bool,
}

pub struct Store {
    conn: Mutex<Connection>,
    ids: Mutex<std::collections::HashMap<(String, String), i64>>,
    root: std::path::PathBuf,
}

impl Store {
    /// Directory holding the database and crawl artifacts.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let root = path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::from_connection(conn, root)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(
            Connection::open_in_memory()?,
            std::env::temp_dir().join("coqui-memory"),
        )
    }

    fn from_connection(conn: Connection, root: std::path::PathBuf) -> Result<Self> {
        conn.execute_batch(SCHEMA)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            ids: Mutex::new(std::collections::HashMap::new()),
            root,
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

    pub fn list_crawls(&self) -> Result<Vec<CrawlSummary>> {
        let ids: Vec<String> = {
            let conn = self.lock()?;
            let mut stmt = conn.prepare("SELECT id FROM crawls ORDER BY rowid DESC LIMIT 100")?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        ids.into_iter().map(|id| self.get_crawl(&id)).collect()
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
                 pagination_prev, structured_data_json, console_json, axe_json, vitals_json,
                 headers_json, discovered_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                     ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)",
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
                serde_json::to_string(&record.vitals).unwrap_or_else(|_| "null".into()),
                serde_json::to_string(&record.headers).unwrap_or_else(|_| "[]".into()),
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

    pub fn suppress_rule(&self, crawl_id: &str, rule: &str) -> Result<()> {
        self.lock()?.execute(
            "INSERT OR IGNORE INTO issue_suppressions (crawl_id, rule) VALUES (?1, ?2)",
            params![crawl_id, rule],
        )?;
        Ok(())
    }

    pub fn restore_rule(&self, crawl_id: &str, rule: &str) -> Result<usize> {
        let changed = self.lock()?.execute(
            "DELETE FROM issue_suppressions WHERE crawl_id = ?1 AND rule = ?2",
            params![crawl_id, rule],
        )?;
        Ok(changed)
    }

    pub fn suppressed_rules(&self, crawl_id: &str) -> Result<Vec<String>> {
        let conn = self.lock()?;
        let mut stmt =
            conn.prepare("SELECT rule FROM issue_suppressions WHERE crawl_id = ?1 ORDER BY rule")?;
        let rows = stmt
            .query_map(params![crawl_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn issue_counts(&self, crawl_id: &str) -> Result<Vec<IssueCount>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT rule, severity, COUNT(*) FROM issues
             WHERE crawl_id = ?1
               AND rule NOT IN (SELECT rule FROM issue_suppressions WHERE crawl_id = ?1)
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

    pub fn url_extractions(&self, crawl_id: &str, url_id: i64) -> Result<Vec<(String, String)>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT name, value FROM extractions WHERE crawl_id = ?1 AND url_id = ?2 ORDER BY name",
        )?;
        let rows = stmt
            .query_map(params![crawl_id, url_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn url_custom_js(&self, crawl_id: &str, url_id: i64) -> Result<Vec<(i64, String)>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT script_index, value_json FROM custom_js
             WHERE crawl_id = ?1 AND url_id = ?2 ORDER BY script_index",
        )?;
        let rows = stmt
            .query_map(params![crawl_id, url_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn link_counts(&self, crawl_id: &str, url_id: i64) -> Result<(i64, i64)> {
        let conn = self.lock()?;
        let incoming = conn.query_row(
            "SELECT COUNT(*) FROM links WHERE crawl_id = ?1 AND to_id = ?2",
            params![crawl_id, url_id],
            |row| row.get(0),
        )?;
        let outgoing = conn.query_row(
            "SELECT COUNT(*) FROM links WHERE crawl_id = ?1 AND from_id = ?2",
            params![crawl_id, url_id],
            |row| row.get(0),
        )?;
        Ok((incoming, outgoing))
    }

    /// Count, average and p95 response time (ms) for timed URLs.
    pub fn response_time_stats(&self, crawl_id: &str) -> Result<(i64, i64, i64)> {
        let conn = self.lock()?;
        let (count, average): (i64, f64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(AVG(response_time_ms), 0) FROM urls
             WHERE crawl_id = ?1 AND response_time_ms IS NOT NULL",
            params![crawl_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if count == 0 {
            return Ok((0, 0, 0));
        }
        let offset = ((count as f64 * 0.95).floor() as i64).clamp(0, count - 1);
        let p95: i64 = conn.query_row(
            "SELECT response_time_ms FROM urls
             WHERE crawl_id = ?1 AND response_time_ms IS NOT NULL
             ORDER BY response_time_ms LIMIT 1 OFFSET ?2",
            params![crawl_id, offset],
            |row| row.get(0),
        )?;
        Ok((count, average.round() as i64, p95))
    }

    /// Indexable, noindex and blocked URL counts.
    pub fn indexability_counts(&self, crawl_id: &str) -> Result<(i64, i64, i64)> {
        let conn = self.lock()?;
        let counts = conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN blocked = 0 AND
                    (meta_robots IS NULL OR meta_robots NOT LIKE '%noindex%')
                    THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN blocked = 0 AND meta_robots LIKE '%noindex%'
                    THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN blocked = 1 THEN 1 ELSE 0 END), 0)
             FROM urls WHERE crawl_id = ?1",
            params![crawl_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        Ok(counts)
    }

    /// URL counts grouped by crawl depth.
    pub fn depth_counts(&self, crawl_id: &str) -> Result<Vec<(i64, i64)>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT depth, COUNT(*) FROM urls WHERE crawl_id = ?1 GROUP BY depth ORDER BY depth",
        )?;
        let rows = stmt
            .query_map(params![crawl_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// HTML, XML, image and other content counts.
    pub fn content_counts(&self, crawl_id: &str) -> Result<(i64, i64, i64, i64)> {
        let conn = self.lock()?;
        let counts = conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN content_type LIKE 'text/html%' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN content_type LIKE '%xml%'
                    AND content_type NOT LIKE 'text/html%' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN content_type LIKE 'image/%' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN content_type IS NULL OR
                    (content_type NOT LIKE 'text/html%' AND content_type NOT LIKE '%xml%'
                     AND content_type NOT LIKE 'image/%') THEN 1 ELSE 0 END), 0)
             FROM urls WHERE crawl_id = ?1",
            params![crawl_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        Ok(counts)
    }

    /// Response time histogram buckets (ms).
    pub fn response_time_buckets(&self, crawl_id: &str) -> Result<Vec<(String, i64)>> {
        let conn = self.lock()?;
        let (fast, good, slow, very_slow): (i64, i64, i64, i64) = conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN response_time_ms < 100 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN response_time_ms >= 100 AND response_time_ms < 300
                    THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN response_time_ms >= 300 AND response_time_ms < 1000
                    THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN response_time_ms >= 1000 THEN 1 ELSE 0 END), 0)
             FROM urls WHERE crawl_id = ?1 AND response_time_ms IS NOT NULL",
            params![crawl_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        Ok(vec![
            ("<100 ms".into(), fast),
            ("100–299 ms".into(), good),
            ("300–999 ms".into(), slow),
            ("1000+ ms".into(), very_slow),
        ])
    }

    pub fn status_counts(&self, crawl_id: &str) -> Result<Vec<(Option<u16>, i64)>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT status, COUNT(*) FROM urls WHERE crawl_id = ?1 GROUP BY status ORDER BY status",
        )?;
        let rows = stmt
            .query_map(params![crawl_id], |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?.map(|value| value as u16),
                    row.get(1)?,
                ))
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

    pub fn list_urls_filtered(
        &self,
        crawl_id: &str,
        page: u32,
        page_size: u32,
        issue_rule: Option<&str>,
        search: Option<&str>,
        status: Option<u16>,
        status_class: Option<u16>,
        content_class: Option<&str>,
        max_depth: Option<u32>,
        indexability: Option<&str>,
        sort: Option<&str>,
        descending: bool,
    ) -> Result<UrlPage> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 500);
        let offset = (page - 1) as i64 * page_size as i64;
        let filter = issue_rule.map(str::to_string);
        let search_like = search
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                format!(
                    "%{}%",
                    value
                        .replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                )
            });
        let conn = self.lock()?;
        let total = conn.query_row(
            "SELECT COUNT(*) FROM urls WHERE crawl_id = ?1
               AND (?2 IS NULL OR EXISTS (
                    SELECT 1 FROM issues
                    WHERE issues.crawl_id = urls.crawl_id AND issues.url = urls.url AND issues.rule = ?2))
               AND (?3 IS NULL OR urls.url LIKE ?3 ESCAPE '\\' OR urls.title LIKE ?3 ESCAPE '\\')
               AND (?4 IS NULL OR urls.status = ?4)
               AND (?5 IS NULL OR urls.status BETWEEN ?5 * 100 AND ?5 * 100 + 99)
               AND (?6 IS NULL OR
                    (?6 = 'html' AND urls.content_type LIKE 'text/html%') OR
                    (?6 = 'xml' AND urls.content_type LIKE '%xml%') OR
                    (?6 = 'image' AND urls.content_type LIKE 'image/%') OR
                    (?6 = 'other' AND (urls.content_type IS NULL OR
                        (urls.content_type NOT LIKE 'text/html%' AND
                         urls.content_type NOT LIKE '%xml%' AND
                         urls.content_type NOT LIKE 'image/%'))))
               AND (?7 IS NULL OR urls.depth <= ?7)
               AND (?8 IS NULL OR
                    (?8 = 'noindex' AND urls.meta_robots LIKE '%noindex%') OR
                    (?8 = 'blocked' AND urls.blocked = 1) OR
                    (?8 = 'indexable' AND urls.blocked = 0 AND
                        (urls.meta_robots IS NULL OR urls.meta_robots NOT LIKE '%noindex%')))",
            params![
                crawl_id,
                filter,
                search_like,
                status,
                status_class,
                content_class,
                max_depth.map(i64::from),
                indexability
            ],
            |row| row.get(0),
        )?;
        let order_column = match sort {
            Some("url") => "url",
            Some("status") => "status",
            Some("depth") => "depth",
            Some("words") => "word_count",
            Some("time") => "response_time_ms",
            Some("size") => "size_bytes",
            _ => "id",
        };
        let direction = if descending { "DESC" } else { "ASC" };
        let sql = format!(
            "SELECT id, crawl_id, url, status, content_type, title, meta_description, h1, h1_count,
                    canonical, meta_robots, redirect_target, redirect_chain, word_count, depth,
                    response_time_ms, size_bytes, error, discovered_at, blocked, hreflang_json,
                    pagination_next, pagination_prev, structured_data_json, console_json,
                    axe_json, vitals_json, headers_json
             FROM urls WHERE crawl_id = ?1
               AND (?3 IS NULL OR EXISTS (
                    SELECT 1 FROM issues
                    WHERE issues.crawl_id = urls.crawl_id AND issues.url = urls.url AND issues.rule = ?3))
               AND (?4 IS NULL OR urls.url LIKE ?4 ESCAPE '\\' OR urls.title LIKE ?4 ESCAPE '\\')
               AND (?5 IS NULL OR urls.status = ?5)
               AND (?6 IS NULL OR urls.status BETWEEN ?6 * 100 AND ?6 * 100 + 99)
               AND (?7 IS NULL OR
                    (?7 = 'html' AND urls.content_type LIKE 'text/html%') OR
                    (?7 = 'xml' AND urls.content_type LIKE '%xml%') OR
                    (?7 = 'image' AND urls.content_type LIKE 'image/%') OR
                    (?7 = 'other' AND (urls.content_type IS NULL OR
                        (urls.content_type NOT LIKE 'text/html%' AND
                         urls.content_type NOT LIKE '%xml%' AND
                         urls.content_type NOT LIKE 'image/%'))))
               AND (?8 IS NULL OR urls.depth <= ?8)
               AND (?9 IS NULL OR
                    (?9 = 'noindex' AND urls.meta_robots LIKE '%noindex%') OR
                    (?9 = 'blocked' AND urls.blocked = 1) OR
                    (?9 = 'indexable' AND urls.blocked = 0 AND
                        (urls.meta_robots IS NULL OR urls.meta_robots NOT LIKE '%noindex%')))
             ORDER BY {order_column} {direction}, id LIMIT ?2 OFFSET ?10"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(
                params![
                    crawl_id,
                    page_size as i64,
                    filter,
                    search_like,
                    status,
                    status_class,
                    content_class,
                    max_depth.map(i64::from),
                    indexability,
                    offset
                ],
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

    /// Page a crawl's URLs without search or status filters.
    pub fn list_urls(
        &self,
        crawl_id: &str,
        page: u32,
        page_size: u32,
        issue_rule: Option<&str>,
    ) -> Result<UrlPage> {
        self.list_urls_filtered(
            crawl_id, page, page_size, issue_rule, None, None, None, None, None, None, None, false,
        )
    }

    /// Every URL of a crawl, used by post-crawl analysis. Callers should not use this for UI paging.
    pub fn all_urls(&self, crawl_id: &str) -> Result<Vec<UrlRecord>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, crawl_id, url, status, content_type, title, meta_description, h1, h1_count,
                    canonical, meta_robots, redirect_target, redirect_chain, word_count, depth,
                    response_time_ms, size_bytes, error, discovered_at, blocked, hreflang_json,
                    pagination_next, pagination_prev, structured_data_json, console_json,
                    axe_json, vitals_json, headers_json
             FROM urls WHERE crawl_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![crawl_id], map_url_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn save_screenshot(&self, crawl_id: &str, url_id: i64, data: &[u8]) -> Result<()> {
        let directory = self.root.join("screenshots").join(crawl_id);
        std::fs::create_dir_all(&directory)?;
        let file = directory.join(format!("{url_id}.jpg"));
        std::fs::write(&file, data)?;
        let relative = file
            .strip_prefix(&self.root)
            .unwrap_or(&file)
            .to_string_lossy()
            .to_string();
        self.lock()?.execute(
            "INSERT OR REPLACE INTO screenshot_files (crawl_id, url_id, path, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![crawl_id, url_id, relative, now_ms()],
        )?;
        Ok(())
    }

    pub fn get_screenshot(&self, crawl_id: &str, url_id: i64) -> Result<Option<Vec<u8>>> {
        let relative: Option<String> = self
            .lock()?
            .query_row(
                "SELECT path FROM screenshot_files WHERE crawl_id = ?1 AND url_id = ?2",
                params![crawl_id, url_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(relative) = relative {
            let file = self.root.join(relative);
            return Ok(std::fs::read(file).ok());
        }
        let legacy: Option<Vec<u8>> = self
            .lock()?
            .query_row(
                "SELECT data FROM screenshots WHERE crawl_id = ?1 AND url_id = ?2",
                params![crawl_id, url_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(legacy)
    }

    pub fn get_url(&self, crawl_id: &str, url_id: i64) -> Result<Option<UrlRecord>> {
        Ok(self
            .all_urls(crawl_id)?
            .into_iter()
            .find(|record| record.id == url_id))
    }
    pub fn sample_urls(&self, crawl_id: &str, limit: u32) -> Result<Vec<UrlRecord>> {
        let urls: Vec<UrlRecord> = self
            .all_urls(crawl_id)?
            .into_iter()
            .filter(|record| {
                !record.blocked
                    && record
                        .status
                        .map(|status| (200..300).contains(&status))
                        .unwrap_or(false)
                    && record
                        .content_type
                        .as_deref()
                        .map(|value| value.to_ascii_lowercase().contains("text/html"))
                        .unwrap_or(false)
            })
            .collect();
        if urls.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let limit = limit as usize;
        if urls.len() <= limit {
            return Ok(urls);
        }
        let step = urls.len() / limit;
        Ok((0..limit).map(|index| urls[index * step].clone()).collect())
    }

    pub fn export_screenshots(&self, crawl_id: &str, directory: impl AsRef<Path>) -> Result<u64> {
        let directory = directory.as_ref();
        std::fs::create_dir_all(directory)?;
        let rows: Vec<(String, i64, String)> = {
            let conn = self.lock()?;
            let mut stmt = conn.prepare(
                "SELECT u.url, s.url_id, s.path FROM screenshot_files s
                 JOIN urls u ON u.id = s.url_id AND u.crawl_id = s.crawl_id
                 WHERE s.crawl_id = ?1 ORDER BY s.url_id",
            )?;
            let rows = stmt
                .query_map(params![crawl_id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        let mut manifest = csv::Writer::from_path(directory.join("screenshots.csv"))?;
        manifest.write_record(["url", "file"])?;
        let mut copied = 0u64;
        for (url, url_id, relative) in rows {
            let source = self.root.join(&relative);
            if !source.exists() {
                continue;
            }
            let file = format!("{url_id}.jpg");
            std::fs::copy(&source, directory.join(&file))?;
            manifest.write_record([url, file])?;
            copied += 1;
        }
        manifest.flush()?;
        Ok(copied)
    }

    pub fn replace_images(
        &self,
        crawl_id: &str,
        url_id: i64,
        images: &[coqui_core::ImageRef],
    ) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM images WHERE crawl_id = ?1 AND url_id = ?2",
            params![crawl_id, url_id],
        )?;
        let mut stmt = conn.prepare(
            "INSERT OR IGNORE INTO images (crawl_id, url_id, src, alt, width, height)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for image in images {
            stmt.execute(params![
                crawl_id,
                url_id,
                image.src,
                image.alt,
                image.width.map(i64::from),
                image.height.map(i64::from)
            ])?;
        }
        Ok(())
    }

    pub fn export_images_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT u.url, i.src, i.alt, i.width, i.height FROM images i
             JOIN urls u ON u.id = i.url_id AND u.crawl_id = i.crawl_id
             WHERE i.crawl_id = ?1 ORDER BY u.url, i.src",
        )?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["url", "src", "alt", "width", "height"])?;
        let mut rows = 0u64;
        let mut records = stmt.query(params![crawl_id])?;
        while let Some(row) = records.next()? {
            let url: String = row.get(0)?;
            let src: String = row.get(1)?;
            let alt: Option<String> = row.get(2)?;
            let width: Option<i64> = row.get(3)?;
            let height: Option<i64> = row.get(4)?;
            writer.write_record([
                url,
                src,
                alt.unwrap_or_default(),
                width.map(|value| value.to_string()).unwrap_or_default(),
                height.map(|value| value.to_string()).unwrap_or_default(),
            ])?;
            rows += 1;
        }
        writer.flush()?;
        Ok(rows)
    }

    pub fn add_extractor(&self, crawl_id: &str, name: &str, selector: &str) -> Result<()> {
        self.lock()?.execute(
            "INSERT INTO extractors (crawl_id, name, selector) VALUES (?1, ?2, ?3)
             ON CONFLICT(crawl_id, name) DO UPDATE SET selector = excluded.selector",
            params![crawl_id, name, selector],
        )?;
        Ok(())
    }

    pub fn list_extractors(&self, crawl_id: &str) -> Result<Vec<(String, String)>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT name, selector FROM extractors WHERE crawl_id = ?1 ORDER BY name")?;
        let rows = stmt
            .query_map(params![crawl_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_extractor(&self, crawl_id: &str, name: &str) -> Result<usize> {
        let changed = self.lock()?.execute(
            "DELETE FROM extractors WHERE crawl_id = ?1 AND name = ?2",
            params![crawl_id, name],
        )?;
        Ok(changed)
    }

    pub fn save_custom_js(
        &self,
        crawl_id: &str,
        url_id: i64,
        values: &[serde_json::Value],
    ) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM custom_js WHERE crawl_id = ?1 AND url_id = ?2",
            params![crawl_id, url_id],
        )?;
        let mut stmt = conn.prepare(
            "INSERT OR REPLACE INTO custom_js (crawl_id, url_id, script_index, value_json)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for (index, value) in values.iter().enumerate() {
            stmt.execute(params![crawl_id, url_id, index as i64, value.to_string()])?;
        }
        Ok(())
    }

    pub fn export_custom_js_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT u.url, c.script_index, c.value_json FROM custom_js c
             JOIN urls u ON u.id = c.url_id AND u.crawl_id = c.crawl_id
             WHERE c.crawl_id = ?1 ORDER BY u.url, c.script_index",
        )?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["url", "script", "value"])?;
        let mut rows = 0u64;
        let mut records = stmt.query(params![crawl_id])?;
        while let Some(row) = records.next()? {
            let url: String = row.get(0)?;
            let index: i64 = row.get(1)?;
            let value: String = row.get(2)?;
            writer.write_record([url, index.to_string(), value])?;
            rows += 1;
        }
        writer.flush()?;
        Ok(rows)
    }

    pub fn save_extraction(
        &self,
        crawl_id: &str,
        url_id: i64,
        name: &str,
        value: &str,
    ) -> Result<()> {
        self.lock()?.execute(
            "INSERT INTO extractions (crawl_id, url_id, name, value) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(crawl_id, url_id, name) DO UPDATE SET value = excluded.value",
            params![crawl_id, url_id, name, value],
        )?;
        Ok(())
    }

    pub fn export_extractions_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT u.url, e.name, e.value FROM extractions e
             JOIN urls u ON u.id = e.url_id AND u.crawl_id = e.crawl_id
             WHERE e.crawl_id = ?1 ORDER BY u.url, e.name",
        )?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["url", "name", "value"])?;
        let mut rows = 0u64;
        let mut records = stmt.query(params![crawl_id])?;
        while let Some(row) = records.next()? {
            let url: String = row.get(0)?;
            let name: String = row.get(1)?;
            let value: String = row.get(2)?;
            writer.write_record([url, name, value])?;
            rows += 1;
        }
        writer.flush()?;
        Ok(rows)
    }

    pub fn save_page_text(
        &self,
        crawl_id: &str,
        url_id: i64,
        text: &str,
        markdown: &str,
    ) -> Result<()> {
        self.lock()?.execute(
            "INSERT INTO page_text (crawl_id, url_id, text, markdown) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(crawl_id, url_id) DO UPDATE SET text = excluded.text,
                                                        markdown = excluded.markdown",
            params![crawl_id, url_id, text, markdown],
        )?;
        Ok(())
    }

    pub fn export_text_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        self.export_page_text(crawl_id, path, "text")
    }

    pub fn export_markdown_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        self.export_page_text(crawl_id, path, "markdown")
    }

    fn export_page_text(
        &self,
        crawl_id: &str,
        path: impl AsRef<Path>,
        column: &str,
    ) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT u.url, p.{column} FROM page_text p
             JOIN urls u ON u.id = p.url_id AND u.crawl_id = p.crawl_id
             WHERE p.crawl_id = ?1 ORDER BY u.url"
        ))?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["url", column])?;
        let mut rows = 0u64;
        let mut records = stmt.query(params![crawl_id])?;
        while let Some(row) = records.next()? {
            let url: String = row.get(0)?;
            let value: String = row.get(1)?;
            writer.write_record([url, value])?;
            rows += 1;
        }
        writer.flush()?;
        Ok(rows)
    }

    pub fn save_lighthouse(&self, crawl_id: &str, url_id: i64, summary_json: &str) -> Result<()> {
        self.lock()?.execute(
            "INSERT OR REPLACE INTO lighthouse_reports (crawl_id, url_id, summary_json, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![crawl_id, url_id, summary_json, now_ms()],
        )?;
        Ok(())
    }

    pub fn get_lighthouse(&self, crawl_id: &str, url_id: i64) -> Result<Option<String>> {
        let summary = self
            .lock()?
            .query_row(
                "SELECT summary_json FROM lighthouse_reports WHERE crawl_id = ?1 AND url_id = ?2",
                params![crawl_id, url_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(summary)
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
    pub fn export_urls_csv(
        &self,
        crawl_id: &str,
        path: impl AsRef<Path>,
        columns: &[String],
        issue: Option<&str>,
    ) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let allowed: Vec<String> = if columns.is_empty() {
            vec!["url".into(), "status".into(), "title".into()]
        } else {
            columns.to_vec()
        };
        let conn = self.lock()?;
        let mut stmt = if issue.is_some() {
            conn.prepare(
                "SELECT url, status, title, h1, word_count, canonical, meta_robots,
                        redirect_target, depth, response_time_ms, vitals_json
                 FROM urls WHERE crawl_id = ?1
                   AND EXISTS (SELECT 1 FROM issues i
                               WHERE i.crawl_id = urls.crawl_id AND i.url = urls.url AND i.rule = ?2)
                 ORDER BY id",
            )?
        } else {
            conn.prepare(
                "SELECT url, status, title, h1, word_count, canonical, meta_robots,
                        redirect_target, depth, response_time_ms, vitals_json
                 FROM urls WHERE crawl_id = ?1 ORDER BY id",
            )?
        };
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(allowed.iter().map(|key| csv_header(key)))?;
        let mut rows = 0u64;
        let mut write = |record: &rusqlite::Row<'_>| -> Result<()> {
            let values: Vec<String> = allowed
                .iter()
                .map(|key| row_csv_value(key, record))
                .collect();
            writer.write_record(values)?;
            rows += 1;
            Ok(())
        };
        if let Some(rule) = issue {
            let mut query = stmt.query(params![crawl_id, rule])?;
            while let Some(record) = query.next()? {
                write(record)?;
            }
        } else {
            let mut query = stmt.query(params![crawl_id])?;
            while let Some(record) = query.next()? {
                write(record)?;
            }
        }
        writer.flush()?;
        Ok(rows)
    }

    pub fn export_urls_xlsx(
        &self,
        crawl_id: &str,
        path: impl AsRef<Path>,
        columns: &[String],
        issue: Option<&str>,
    ) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let allowed: Vec<String> = if columns.is_empty() {
            vec!["url".into(), "status".into(), "title".into()]
        } else {
            columns.to_vec()
        };
        let issue_urls: Option<std::collections::HashSet<String>> = match issue {
            Some(rule) => {
                let conn = self.lock()?;
                let mut stmt =
                    conn.prepare("SELECT url FROM issues WHERE crawl_id = ?1 AND rule = ?2")?;
                let rows = stmt
                    .query_map(params![crawl_id, rule], |row| row.get::<_, String>(0))?
                    .collect::<std::result::Result<std::collections::HashSet<_>, _>>()?;
                Some(rows)
            }
            None => None,
        };
        let urls = self.all_urls(crawl_id)?;
        let mut workbook = rust_xlsxwriter::Workbook::new();
        let sheet = workbook.add_worksheet();
        for (column, key) in allowed.iter().enumerate() {
            sheet.write_string(0, column as u16, csv_header(key))?;
        }
        let mut row = 1u32;
        for url in &urls {
            if let Some(filter) = &issue_urls {
                if !filter.contains(&url.url) {
                    continue;
                }
            }
            for (column, key) in allowed.iter().enumerate() {
                sheet.write_string(row, column as u16, url_csv_value(url, key))?;
            }
            row += 1;
        }
        workbook.save(path)?;
        Ok(u64::from(row.saturating_sub(1)))
    }

    pub fn export_issues_xlsx(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT url, rule, severity, detail FROM issues WHERE crawl_id = ?1 ORDER BY rule, url",
        )?;
        let mut workbook = rust_xlsxwriter::Workbook::new();
        let sheet = workbook.add_worksheet();
        for (column, header) in ["url", "rule", "severity", "detail"].iter().enumerate() {
            sheet.write_string(0, column as u16, *header)?;
        }
        let mut rows = 0u32;
        let mut records = stmt.query(params![crawl_id])?;
        while let Some(row) = records.next()? {
            let values = [
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            ];
            for (column, value) in values.iter().enumerate() {
                sheet.write_string(rows + 1, column as u16, value)?;
            }
            rows += 1;
        }
        workbook.save(path)?;
        Ok(u64::from(rows))
    }

    pub fn export_links_xlsx(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let links = self.all_links(crawl_id)?;
        let mut workbook = rust_xlsxwriter::Workbook::new();
        let sheet = workbook.add_worksheet();
        for (column, header) in ["from_url", "to_url", "anchor", "rel", "internal"]
            .iter()
            .enumerate()
        {
            sheet.write_string(0, column as u16, *header)?;
        }
        for (index, link) in links.iter().enumerate() {
            let row = index as u32 + 1;
            sheet.write_string(row, 0, &link.from_url)?;
            sheet.write_string(row, 1, &link.to_url)?;
            sheet.write_string(row, 2, link.anchor.clone().unwrap_or_default())?;
            sheet.write_string(row, 3, link.rel.clone().unwrap_or_default())?;
            sheet.write_string(row, 4, link.internal.to_string())?;
        }
        workbook.save(path)?;
        Ok(links.len() as u64)
    }

    pub fn export_sitemap(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        use std::io::Write;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut urls = conn.prepare(
            "SELECT id, url, meta_robots, headers_json FROM urls
             WHERE crawl_id = ?1 AND status = 200 ORDER BY id",
        )?;
        let mut images = conn
            .prepare("SELECT src FROM images WHERE crawl_id = ?1 AND url_id = ?2 ORDER BY src")?;
        let mut file = std::fs::File::create(path)?;
        writeln!(file, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
        writeln!(
            file,
            r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">"#
        )?;
        let mut rows = urls.query(params![crawl_id])?;
        let mut count = 0u64;
        while let Some(row) = rows.next()? {
            if count >= 50_000 {
                break;
            }
            let id: i64 = row.get(0)?;
            let url: String = row.get(1)?;
            let robots: Option<String> = row.get(2)?;
            let headers: Option<String> = row.get(3)?;
            if robots
                .unwrap_or_default()
                .to_ascii_lowercase()
                .contains("noindex")
            {
                continue;
            }
            writeln!(file, "  <url>")?;
            writeln!(file, "    <loc>{}</loc>", xml_escape(&url))?;
            if let Some(lastmod) = lastmod_from_headers(headers.as_deref()) {
                writeln!(file, "    <lastmod>{lastmod}</lastmod>")?;
            }
            let mut image_rows = images.query(params![crawl_id, id])?;
            while let Some(image) = image_rows.next()? {
                let src: String = image.get(0)?;
                writeln!(file, "    <image:image>")?;
                writeln!(file, "      <image:loc>{}</image:loc>", xml_escape(&src))?;
                writeln!(file, "    </image:image>")?;
            }
            writeln!(file, "  </url>")?;
            count += 1;
        }
        writeln!(file, "</urlset>")?;
        Ok(count)
    }

    pub fn export_sitemap_bundle(
        &self,
        crawl_id: &str,
        directory: impl AsRef<Path>,
        gzip: bool,
    ) -> Result<u64> {
        use std::io::Write;
        let directory = directory.as_ref();
        std::fs::create_dir_all(directory)?;
        let conn = self.lock()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM urls WHERE crawl_id = ?1 AND status = 200
             AND (meta_robots IS NULL OR lower(meta_robots) NOT LIKE '%noindex%')",
            params![crawl_id],
            |row| row.get(0),
        )?;
        let count = count.max(0) as u64;
        let chunks = count.div_ceil(50_000).max(1);
        let suffix = if gzip { ".xml.gz" } else { ".xml" };
        let mut urls = conn.prepare(
            "SELECT id, url, headers_json FROM urls WHERE crawl_id = ?1 AND status = 200
             AND (meta_robots IS NULL OR lower(meta_robots) NOT LIKE '%noindex%')
             ORDER BY id LIMIT 50000 OFFSET ?2",
        )?;
        let mut images = conn
            .prepare("SELECT src FROM images WHERE crawl_id = ?1 AND url_id = ?2 ORDER BY src")?;
        let mut total = 0u64;
        for chunk in 0..chunks {
            let name = format!("sitemap-{}{suffix}", chunk + 1);
            let file = std::fs::File::create(directory.join(&name))?;
            let mut writer: Box<dyn Write> = if gzip {
                Box::new(flate2::write::GzEncoder::new(
                    file,
                    flate2::Compression::default(),
                ))
            } else {
                Box::new(file)
            };
            writeln!(writer, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
            writeln!(
                writer,
                r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">"#
            )?;
            let mut rows = urls.query(params![crawl_id, (chunk * 50_000) as i64])?;
            while let Some(row) = rows.next()? {
                let id: i64 = row.get(0)?;
                let url: String = row.get(1)?;
                let headers: Option<String> = row.get(2)?;
                writeln!(writer, "  <url>")?;
                writeln!(writer, "    <loc>{}</loc>", xml_escape(&url))?;
                if let Some(lastmod) = lastmod_from_headers(headers.as_deref()) {
                    writeln!(writer, "    <lastmod>{lastmod}</lastmod>")?;
                }
                let mut image_rows = images.query(params![crawl_id, id])?;
                while let Some(image) = image_rows.next()? {
                    let src: String = image.get(0)?;
                    writeln!(writer, "    <image:image>")?;
                    writeln!(writer, "      <image:loc>{}</image:loc>", xml_escape(&src))?;
                    writeln!(writer, "    </image:image>")?;
                }
                writeln!(writer, "  </url>")?;
                total += 1;
            }
            writeln!(writer, "</urlset>")?;
            writer.flush()?;
        }
        if chunks > 1 {
            let file = std::fs::File::create(directory.join("sitemap-index.xml"))?;
            let mut writer: Box<dyn Write> = if gzip {
                Box::new(flate2::write::GzEncoder::new(
                    file,
                    flate2::Compression::default(),
                ))
            } else {
                Box::new(file)
            };
            writeln!(writer, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
            writeln!(
                writer,
                r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#
            )?;
            for chunk in 0..chunks {
                writeln!(writer, "  <sitemap>")?;
                writeln!(writer, "    <loc>sitemap-{}{suffix}</loc>", chunk + 1)?;
                writeln!(writer, "  </sitemap>")?;
            }
            writeln!(writer, "</sitemapindex>")?;
            writer.flush()?;
        }
        Ok(total)
    }

    pub fn export_headers_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT url, headers_json FROM urls WHERE crawl_id = ?1 AND headers_json IS NOT NULL",
        )?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["url", "name", "value"])?;
        let mut rows = 0u64;
        let mut records = stmt.query(params![crawl_id])?;
        while let Some(row) = records.next()? {
            let url: String = row.get(0)?;
            let json: String = row.get(1)?;
            let headers: Vec<(String, String)> = serde_json::from_str(&json).unwrap_or_default();
            for (name, value) in headers {
                writer.write_record([url.clone(), name, value])?;
                rows += 1;
            }
        }
        writer.flush()?;
        Ok(rows)
    }

    pub fn export_accessibility_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT url, axe_json FROM urls WHERE crawl_id = ?1 AND axe_json IS NOT NULL AND axe_json <> '[]'",
        )?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["url", "rule", "impact", "help", "help_url", "nodes"])?;
        let mut rows = 0u64;
        let mut records = stmt.query(params![crawl_id])?;
        while let Some(row) = records.next()? {
            let url: String = row.get(0)?;
            let json: String = row.get(1)?;
            let violations: Vec<AxeViolation> = serde_json::from_str(&json).unwrap_or_default();
            for violation in violations {
                writer.write_record([
                    url.clone(),
                    violation.id,
                    violation.impact.unwrap_or_default(),
                    violation.help,
                    violation.help_url,
                    violation.nodes.to_string(),
                ])?;
                rows += 1;
            }
        }
        writer.flush()?;
        Ok(rows)
    }

    pub fn export_structured_data_csv(
        &self,
        crawl_id: &str,
        path: impl AsRef<Path>,
    ) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT url, structured_data_json FROM urls WHERE crawl_id = ?1 AND structured_data_json IS NOT NULL",
        )?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["url", "types", "errors"])?;
        let mut rows = 0u64;
        let mut records = stmt.query(params![crawl_id])?;
        while let Some(row) = records.next()? {
            let url: String = row.get(0)?;
            let json: String = row.get(1)?;
            let data: StructuredData = serde_json::from_str(&json).unwrap_or_default();
            writer.write_record([url, data.types.join("; "), data.errors.join("; ")])?;
            rows += 1;
        }
        writer.flush()?;
        Ok(rows)
    }

    pub fn export_vitals_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT url, vitals_json FROM urls WHERE crawl_id = ?1 AND vitals_json IS NOT NULL",
        )?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["url", "lcp_ms", "cls", "tbt_ms", "ttfb_ms", "fcp_ms"])?;
        let mut rows = 0u64;
        let mut records = stmt.query(params![crawl_id])?;
        while let Some(row) = records.next()? {
            let url: String = row.get(0)?;
            let json: String = row.get(1)?;
            let Some(vitals) = serde_json::from_str::<WebVitals>(&json).ok() else {
                continue;
            };
            writer.write_record([
                url,
                vitals.lcp_ms.to_string(),
                vitals.cls.to_string(),
                vitals.tbt_ms.to_string(),
                vitals.ttfb_ms.to_string(),
                vitals.fcp_ms.to_string(),
            ])?;
            rows += 1;
        }
        writer.flush()?;
        Ok(rows)
    }

    pub fn export_console_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT url, console_json FROM urls WHERE crawl_id = ?1 AND console_json IS NOT NULL AND console_json <> '[]'",
        )?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["url", "message"])?;
        let mut rows = 0u64;
        let mut records = stmt.query(params![crawl_id])?;
        while let Some(row) = records.next()? {
            let url: String = row.get(0)?;
            let json: String = row.get(1)?;
            let messages: Vec<String> = serde_json::from_str(&json).unwrap_or_default();
            for message in messages {
                writer.write_record([url.clone(), message])?;
                rows += 1;
            }
        }
        writer.flush()?;
        Ok(rows)
    }

    pub fn export_issues_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT url, rule, severity, detail FROM issues WHERE crawl_id = ?1 ORDER BY rule, url",
        )?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["url", "rule", "severity", "detail"])?;
        let mut rows = 0u64;
        let mut records = stmt.query(params![crawl_id])?;
        while let Some(row) = records.next()? {
            let url: String = row.get(0)?;
            let rule: String = row.get(1)?;
            let severity: String = row.get(2)?;
            let detail: Option<String> = row.get(3)?;
            writer.write_record([url, rule, severity, detail.unwrap_or_default()])?;
            rows += 1;
        }
        writer.flush()?;
        Ok(rows)
    }

    pub fn export_links_csv(&self, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let links = self.all_links(crawl_id)?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["from_url", "to_url", "anchor", "rel", "internal"])?;
        for link in &links {
            writer.write_record([
                link.from_url.clone(),
                link.to_url.clone(),
                link.anchor.clone().unwrap_or_default(),
                link.rel.clone().unwrap_or_default(),
                link.internal.to_string(),
            ])?;
        }
        writer.flush()?;
        Ok(links.len() as u64)
    }

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
    pub fn save_view(
        &self,
        name: &str,
        search: Option<&str>,
        status: Option<u16>,
        issue: Option<&str>,
        content_class: Option<&str>,
        max_depth: Option<u32>,
        indexability: Option<&str>,
    ) -> Result<()> {
        self.lock()?.execute(
            "INSERT INTO saved_views (name, search, status, issue, content_class, max_depth,
                                      indexability, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(name) DO UPDATE SET search = excluded.search,
                                            status = excluded.status,
                                            issue = excluded.issue,
                                            content_class = excluded.content_class,
                                            max_depth = excluded.max_depth,
                                            indexability = excluded.indexability,
                                            created_at = excluded.created_at",
            params![
                name,
                search,
                status,
                issue,
                content_class,
                max_depth.map(i64::from),
                indexability,
                now_ms()
            ],
        )?;
        Ok(())
    }

    pub fn saved_views(&self) -> Result<Vec<SavedView>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT name, search, status, issue, content_class, max_depth, indexability
             FROM saved_views ORDER BY name",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SavedView {
                    name: row.get(0)?,
                    search: row.get(1)?,
                    status: row.get::<_, Option<i64>>(2)?.map(|value| value as u16),
                    issue: row.get(3)?,
                    content_class: row.get(4)?,
                    max_depth: row.get::<_, Option<i64>>(5)?.map(|value| value as u32),
                    indexability: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_view(&self, name: &str) -> Result<usize> {
        let changed = self
            .lock()?
            .execute("DELETE FROM saved_views WHERE name = ?1", params![name])?;
        Ok(changed)
    }

    pub fn add_schedule(
        &self,
        seed_kind: &str,
        seed: &str,
        interval_seconds: i64,
        config_json: &str,
        next_run_at: i64,
    ) -> Result<i64> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO schedules (seed_kind, seed, interval_seconds, config_json, next_run_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![seed_kind, seed, interval_seconds, config_json, next_run_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn map_schedule(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduleRecord> {
        Ok(ScheduleRecord {
            id: row.get(0)?,
            seed_kind: row.get(1)?,
            seed: row.get(2)?,
            interval_seconds: row.get(3)?,
            config_json: row.get(4)?,
            next_run_at: row.get(5)?,
            last_run_at: row.get(6)?,
            enabled: row.get::<_, i64>(7)? != 0,
        })
    }

    fn schedule_rows(
        conn: &rusqlite::Connection,
        query: &str,
        now: Option<i64>,
    ) -> Result<Vec<ScheduleRecord>> {
        let mut stmt = conn.prepare(query)?;
        let rows = match now {
            Some(now) => stmt
                .query_map(params![now], Self::map_schedule)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            None => stmt
                .query_map([], Self::map_schedule)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
        };
        Ok(rows)
    }

    pub fn schedules(&self) -> Result<Vec<ScheduleRecord>> {
        let conn = self.lock()?;
        Self::schedule_rows(
            &conn,
            "SELECT id, seed_kind, seed, interval_seconds, config_json, next_run_at, last_run_at,
                    enabled
             FROM schedules ORDER BY next_run_at, id",
            None,
        )
    }

    pub fn due_schedules(&self, now: i64) -> Result<Vec<ScheduleRecord>> {
        let conn = self.lock()?;
        Self::schedule_rows(
            &conn,
            "SELECT id, seed_kind, seed, interval_seconds, config_json, next_run_at, last_run_at,
                    enabled
             FROM schedules WHERE enabled = 1 AND next_run_at <= ?1 ORDER BY next_run_at, id",
            Some(now),
        )
    }

    pub fn remove_schedule(&self, id: i64) -> Result<usize> {
        let changed = self
            .lock()?
            .execute("DELETE FROM schedules WHERE id = ?1", params![id])?;
        Ok(changed)
    }

    pub fn mark_schedule_run(&self, id: i64, last_run_at: i64, next_run_at: i64) -> Result<()> {
        self.lock()?.execute(
            "UPDATE schedules SET last_run_at = ?2, next_run_at = ?3 WHERE id = ?1",
            params![id, last_run_at, next_run_at],
        )?;
        Ok(())
    }

    pub fn save_profile(&self, name: &str, config_json: &str) -> Result<()> {
        self.lock()?.execute(
            "INSERT INTO profiles (name, config_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET config_json = excluded.config_json,
                                            updated_at = excluded.updated_at",
            params![name, config_json, now_ms()],
        )?;
        Ok(())
    }

    pub fn list_profiles(&self) -> Result<Vec<(String, i64)>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT name, updated_at FROM profiles ORDER BY name")?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_profile(&self, name: &str) -> Result<Option<String>> {
        let conn = self.lock()?;
        let json = conn
            .query_row(
                "SELECT config_json FROM profiles WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;
        Ok(json)
    }

    pub fn delete_profile(&self, name: &str) -> Result<usize> {
        let changed = self
            .lock()?
            .execute("DELETE FROM profiles WHERE name = ?1", params![name])?;
        Ok(changed)
    }

    pub fn delete_crawl(&self, id: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM issues WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM checkpoints WHERE crawl_id = ?1", params![id])?;
        let paths: Vec<String> = {
            let mut stmt = conn.prepare("SELECT path FROM screenshot_files WHERE crawl_id = ?1")?;
            let rows = stmt
                .query_map(params![id], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        for relative in paths {
            let _ = std::fs::remove_file(self.root.join(relative));
        }
        conn.execute(
            "DELETE FROM screenshot_files WHERE crawl_id = ?1",
            params![id],
        )?;
        let _ = std::fs::remove_dir_all(self.root.join("screenshots").join(id));
        conn.execute("DELETE FROM screenshots WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM images WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM custom_js WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM page_text WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM extractions WHERE crawl_id = ?1", params![id])?;
        conn.execute("DELETE FROM extractors WHERE crawl_id = ?1", params![id])?;
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
        vitals: row
            .get::<_, Option<String>>(26)?
            .and_then(|value| serde_json::from_str(&value).ok()),
        headers: row
            .get::<_, Option<String>>(27)?
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
    if version < 21 {
        if !has_column(conn, "saved_views", "content_class")? {
            conn.execute("ALTER TABLE saved_views ADD COLUMN content_class TEXT", [])?;
        }
        if !has_column(conn, "saved_views", "max_depth")? {
            conn.execute("ALTER TABLE saved_views ADD COLUMN max_depth INTEGER", [])?;
        }
        if !has_column(conn, "saved_views", "indexability")? {
            conn.execute("ALTER TABLE saved_views ADD COLUMN indexability TEXT", [])?;
        }
    }
    if version < 20 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS saved_views (
                name       TEXT PRIMARY KEY,
                search     TEXT,
                status     INTEGER,
                issue      TEXT,
                created_at INTEGER NOT NULL
            );",
        )?;
    }
    if version < 19 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS saved_views (
    name          TEXT PRIMARY KEY,
    search        TEXT,
    status        INTEGER,
    issue         TEXT,
    content_class TEXT,
    max_depth     INTEGER,
    indexability  TEXT,
    created_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS custom_js (
                crawl_id     TEXT NOT NULL,
                url_id       INTEGER NOT NULL,
                script_index INTEGER NOT NULL,
                value_json   TEXT NOT NULL,
                PRIMARY KEY (crawl_id, url_id, script_index)
            );",
        )?;
    }
    if version < 18 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schedules (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                seed_kind        TEXT NOT NULL,
                seed             TEXT NOT NULL,
                interval_seconds INTEGER NOT NULL,
                config_json      TEXT NOT NULL,
                next_run_at      INTEGER NOT NULL,
                last_run_at      INTEGER,
                enabled          INTEGER NOT NULL DEFAULT 1
            );",
        )?;
    }
    if version < 17 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS extractors (
                crawl_id TEXT NOT NULL,
                name     TEXT NOT NULL,
                selector TEXT NOT NULL,
                PRIMARY KEY (crawl_id, name)
            );
            CREATE TABLE IF NOT EXISTS extractions (
                crawl_id TEXT NOT NULL,
                url_id   INTEGER NOT NULL,
                name     TEXT NOT NULL,
                value    TEXT NOT NULL,
                PRIMARY KEY (crawl_id, url_id, name)
            );",
        )?;
    }
    if version < 16 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS extractors (
    crawl_id TEXT NOT NULL,
    name     TEXT NOT NULL,
    selector TEXT NOT NULL,
    PRIMARY KEY (crawl_id, name)
);

CREATE TABLE IF NOT EXISTS extractions (
    crawl_id TEXT NOT NULL,
    url_id   INTEGER NOT NULL,
    name     TEXT NOT NULL,
    value    TEXT NOT NULL,
    PRIMARY KEY (crawl_id, url_id, name)
);

CREATE TABLE IF NOT EXISTS saved_views (
    name          TEXT PRIMARY KEY,
    search        TEXT,
    status        INTEGER,
    issue         TEXT,
    content_class TEXT,
    max_depth     INTEGER,
    indexability  TEXT,
    created_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS custom_js (
    crawl_id     TEXT NOT NULL,
    url_id       INTEGER NOT NULL,
    script_index INTEGER NOT NULL,
    value_json   TEXT NOT NULL,
    PRIMARY KEY (crawl_id, url_id, script_index)
);

CREATE TABLE IF NOT EXISTS page_text (
                crawl_id TEXT NOT NULL,
                url_id   INTEGER NOT NULL,
                text     TEXT NOT NULL,
                markdown TEXT NOT NULL,
                PRIMARY KEY (crawl_id, url_id)
            );",
        )?;
    }
    if version < 15 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schedules (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    seed_kind        TEXT NOT NULL,
    seed             TEXT NOT NULL,
    interval_seconds INTEGER NOT NULL,
    config_json      TEXT NOT NULL,
    next_run_at      INTEGER NOT NULL,
    last_run_at      INTEGER,
    enabled          INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS saved_views (
    name          TEXT PRIMARY KEY,
    search        TEXT,
    status        INTEGER,
    issue         TEXT,
    content_class TEXT,
    max_depth     INTEGER,
    indexability  TEXT,
    created_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS profiles (
                name        TEXT PRIMARY KEY,
                config_json TEXT NOT NULL,
                updated_at  INTEGER NOT NULL
            );",
        )?;
    }
    if version < 14 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS images (
                crawl_id TEXT NOT NULL,
                url_id   INTEGER NOT NULL,
                src      TEXT NOT NULL,
                alt      TEXT,
                width    INTEGER,
                height   INTEGER,
                PRIMARY KEY (crawl_id, url_id, src)
            );",
        )?;
    }
    if version < 13 && !has_column(conn, "urls", "headers_json")? {
        conn.execute("ALTER TABLE urls ADD COLUMN headers_json TEXT", [])?;
    }
    if version < 12 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS lighthouse_reports (
                crawl_id     TEXT NOT NULL,
                url_id       INTEGER NOT NULL,
                summary_json TEXT NOT NULL,
                created_at   INTEGER NOT NULL,
                PRIMARY KEY (crawl_id, url_id)
            );",
        )?;
    }
    if version < 11 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS screenshot_files (
                crawl_id   TEXT NOT NULL,
                url_id     INTEGER NOT NULL,
                path       TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (crawl_id, url_id)
            );",
        )?;
    }
    if version < 10 && !has_column(conn, "urls", "vitals_json")? {
        conn.execute("ALTER TABLE urls ADD COLUMN vitals_json TEXT", [])?;
    }
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
            vitals: None,
            headers: Vec::new(),
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
    fn exports_xlsx_workbook() {
        let dir = std::env::temp_dir().join(format!("coqui-xlsx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&sample_record("c1", "https://example.com/"))
            .unwrap();
        let path = dir.join("urls.xlsx");
        let rows = store
            .export_urls_xlsx("c1", &path, &["url".to_string(), "title".to_string()], None)
            .unwrap();
        assert_eq!(rows, 1);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..2], b"PK");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn screenshots_round_trip_through_files() {
        let dir = std::env::temp_dir().join(format!("coqui-shots-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&sample_record("c1", "https://example.com/"))
            .unwrap();
        let url_id = store.url_id("c1", "https://example.com/").unwrap().unwrap();
        let bytes = b"\xff\xd8fake-jpeg";
        store.save_screenshot("c1", url_id, bytes).unwrap();
        assert_eq!(
            store.get_screenshot("c1", url_id).unwrap().as_deref(),
            Some(bytes.as_slice())
        );
        assert!(dir.join(format!("screenshots/c1/{url_id}.jpg")).exists());
        let bundle = dir.join("bundle");
        assert_eq!(store.export_screenshots("c1", &bundle).unwrap(), 1);
        assert!(bundle.join(format!("{url_id}.jpg")).exists());
        assert!(bundle.join("screenshots.csv").exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn bulk_data_exports_flatten_evidence() {
        use coqui_core::{AxeViolation, HreflangLink, ImageRef, StructuredData, WebVitals};
        let dir = std::env::temp_dir().join(format!("coqui-bulk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&NewUrl {
                crawl_id: "c1".into(),
                url: "https://example.com/".into(),
                status: Some(200),
                blocked: false,
                content_type: Some("text/html".into()),
                title: Some("Example".into()),
                meta_description: None,
                h1: Some("Example".into()),
                h1_count: 1,
                canonical: None,
                meta_robots: None,
                hreflang: Vec::<HreflangLink>::new(),
                pagination_next: None,
                pagination_prev: None,
                structured_data: StructuredData {
                    types: vec!["Article".into()],
                    errors: vec!["missing headline for Article".into()],
                },
                console_errors: vec!["js: boom".into(), "network: 404".into()],
                axe: vec![AxeViolation {
                    id: "image-alt".into(),
                    impact: Some("critical".into()),
                    help: "Images must have alternate text".into(),
                    help_url: "https://example.com/rule".into(),
                    nodes: 2,
                }],
                vitals: Some(WebVitals {
                    lcp_ms: 3000,
                    cls: 0.2,
                    tbt_ms: 250,
                    ttfb_ms: 900,
                    fcp_ms: 1200,
                }),
                headers: vec![("content-type".into(), "text/html".into())],
                redirect_target: None,
                redirect_chain: Vec::new(),
                word_count: Some(500),
                depth: 0,
                response_time_ms: Some(10),
                size_bytes: Some(100),
                error: None,
                discovered_at: now_ms(),
            })
            .unwrap();

        let url_id = store.url_id("c1", "https://example.com/").unwrap().unwrap();
        store
            .replace_images(
                "c1",
                url_id,
                &[ImageRef {
                    src: "https://example.com/a.png".into(),
                    alt: Some("A frog".into()),
                    width: Some(10),
                    height: Some(20),
                }],
            )
            .unwrap();
        let accessibility = dir.join("a11y.csv");
        assert_eq!(
            store
                .export_accessibility_csv("c1", &accessibility)
                .unwrap(),
            1
        );
        let structured = dir.join("structured.csv");
        assert_eq!(
            store.export_structured_data_csv("c1", &structured).unwrap(),
            1
        );
        let vitals = dir.join("vitals.csv");
        assert_eq!(store.export_vitals_csv("c1", &vitals).unwrap(), 1);
        let console = dir.join("console.csv");
        assert_eq!(store.export_console_csv("c1", &console).unwrap(), 2);
        let headers = dir.join("headers.csv");
        assert_eq!(store.export_headers_csv("c1", &headers).unwrap(), 1);
        let images = dir.join("images.csv");
        assert_eq!(store.export_images_csv("c1", &images).unwrap(), 1);
        let text = std::fs::read_to_string(&accessibility).unwrap();
        assert!(text.contains("image-alt"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn sitemap_skips_noindex_and_escapes_images() {
        let dir = std::env::temp_dir().join(format!("coqui-sitemap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        let mut page = sample_record("c1", "https://example.com/");
        page.headers = vec![(
            "Last-Modified".into(),
            "Wed, 21 Oct 2015 07:28:00 GMT".into(),
        )];
        store.insert_url(&page).unwrap();
        let mut hidden = sample_record("c1", "https://example.com/private");
        hidden.meta_robots = Some("noindex,follow".into());
        store.insert_url(&hidden).unwrap();
        let url_id = store.url_id("c1", "https://example.com/").unwrap().unwrap();
        store
            .replace_images(
                "c1",
                url_id,
                &[coqui_core::ImageRef {
                    src: "https://example.com/a.png?a=1&b=2".into(),
                    alt: None,
                    width: None,
                    height: None,
                }],
            )
            .unwrap();
        let path = dir.join("sitemap.xml");
        assert_eq!(store.export_sitemap("c1", &path).unwrap(), 1);
        let xml = std::fs::read_to_string(&path).unwrap();
        assert!(xml.contains("<lastmod>2015-10-21T07:28:00Z</lastmod>"));
        assert!(xml.contains("a.png?a=1&amp;b=2"));
        assert!(!xml.contains("private"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn sitemap_bundle_splits_and_gzips() {
        let dir = std::env::temp_dir().join(format!("coqui-sitemap-bundle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        for path in ["https://example.com/", "https://example.com/about"] {
            store.insert_url(&sample_record("c1", path)).unwrap();
        }
        let out = dir.join("sitemaps");
        assert_eq!(store.export_sitemap_bundle("c1", &out, false).unwrap(), 2);
        assert!(out.join("sitemap-1.xml").exists());
        assert!(!out.join("sitemap-index.xml").exists());
        let gz = dir.join("sitemaps-gz");
        assert_eq!(store.export_sitemap_bundle("c1", &gz, true).unwrap(), 2);
        let bytes = std::fs::read(gz.join("sitemap-1.xml.gz")).unwrap();
        assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn custom_extractions_round_trip() {
        let dir = std::env::temp_dir().join(format!("coqui-extract-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&sample_record("c1", "https://example.com/"))
            .unwrap();
        let id = store.url_id("c1", "https://example.com/").unwrap().unwrap();
        store.add_extractor("c1", "h1", "h1").unwrap();
        assert_eq!(store.list_extractors("c1").unwrap().len(), 1);
        store.save_extraction("c1", id, "h1", "Hello").unwrap();
        let path = dir.join("extractions.csv");
        assert_eq!(store.export_extractions_csv("c1", &path).unwrap(), 1);
        assert!(std::fs::read_to_string(&path).unwrap().contains("Hello"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn page_text_exports() {
        let dir = std::env::temp_dir().join(format!("coqui-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&sample_record("c1", "https://example.com/"))
            .unwrap();
        let id = store.url_id("c1", "https://example.com/").unwrap().unwrap();
        store
            .save_page_text("c1", id, "hello world", "# Hello\n\nhello world")
            .unwrap();
        let text = dir.join("text.csv");
        assert_eq!(store.export_text_csv("c1", &text).unwrap(), 1);
        assert!(
            std::fs::read_to_string(&text)
                .unwrap()
                .contains("hello world")
        );
        let markdown = dir.join("markdown.csv");
        assert_eq!(store.export_markdown_csv("c1", &markdown).unwrap(), 1);
        assert!(
            std::fs::read_to_string(&markdown)
                .unwrap()
                .contains("# Hello")
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn schedules_round_trip() {
        let dir = std::env::temp_dir().join(format!("coqui-sched-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        let id = store
            .add_schedule("url", "https://example.com/", 60, "{}", 100)
            .unwrap();
        assert_eq!(store.schedules().unwrap().len(), 1);
        assert_eq!(store.due_schedules(50).unwrap().len(), 0);
        assert_eq!(store.due_schedules(150).unwrap().len(), 1);
        store.mark_schedule_run(id, 150, 210).unwrap();
        let record = &store.schedules().unwrap()[0];
        assert_eq!(record.last_run_at, Some(150));
        assert_eq!(record.next_run_at, 210);
        assert_eq!(store.remove_schedule(id).unwrap(), 1);
        assert!(store.schedules().unwrap().is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn profiles_round_trip() {
        let dir = std::env::temp_dir().join(format!("coqui-profiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store.save_profile("fast", "{\"maxUrls\":10}").unwrap();
        store.save_profile("fast", "{\"maxUrls\":20}").unwrap();
        let profiles = store.list_profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(
            store.get_profile("fast").unwrap().as_deref(),
            Some("{\"maxUrls\":20}")
        );
        assert_eq!(store.delete_profile("fast").unwrap(), 1);
        assert!(store.get_profile("fast").unwrap().is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn list_urls_searches_and_filters_status() {
        let dir = std::env::temp_dir().join(format!("coqui-search-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&sample_record("c1", "https://example.com/"))
            .unwrap();
        let mut missing = sample_record("c1", "https://example.com/missing");
        missing.status = Some(404);
        missing.title = Some("Missing page".into());
        store.insert_url(&missing).unwrap();
        assert_eq!(
            store
                .list_urls_filtered(
                    "c1",
                    1,
                    10,
                    None,
                    Some("Missing"),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    false
                )
                .unwrap()
                .total,
            1
        );
        assert_eq!(
            store
                .list_urls_filtered(
                    "c1",
                    1,
                    10,
                    None,
                    None,
                    Some(404),
                    None,
                    None,
                    None,
                    None,
                    None,
                    false
                )
                .unwrap()
                .total,
            1
        );
        assert_eq!(
            store
                .list_urls_filtered(
                    "c1", 1, 10, None, None, None, None, None, None, None, None, false
                )
                .unwrap()
                .total,
            2
        );
        let sorted = store
            .list_urls_filtered(
                "c1",
                1,
                10,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some("status"),
                true,
            )
            .unwrap();
        assert_eq!(sorted.rows[0].status, Some(404));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn url_detail_helpers_return_rows() {
        let dir = std::env::temp_dir().join(format!("coqui-detail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&sample_record("c1", "https://example.com/"))
            .unwrap();
        let id = store.url_id("c1", "https://example.com/").unwrap().unwrap();
        store.add_extractor("c1", "h1", "h1").unwrap();
        store.save_extraction("c1", id, "h1", "Hello").unwrap();
        store
            .save_custom_js("c1", id, &[serde_json::json!("Rendered title")])
            .unwrap();
        assert_eq!(store.url_extractions("c1", id).unwrap().len(), 1);
        let custom = store.url_custom_js("c1", id).unwrap();
        assert_eq!(custom.len(), 1);
        assert!(custom[0].1.contains("Rendered title"));
        store
            .insert_link("c1", id, id, Some("self"), None, true)
            .unwrap();
        assert_eq!(store.link_counts("c1", id).unwrap(), (1, 1));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn suppressed_rules_hide_issue_counts() {
        let dir = std::env::temp_dir().join(format!("coqui-suppress-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_issue(&Issue {
                crawl_id: "c1".into(),
                url: "https://example.com/".into(),
                rule: "missing_title".into(),
                severity: IssueSeverity::Error,
                detail: None,
            })
            .unwrap();
        assert_eq!(store.issue_counts("c1").unwrap().len(), 1);
        store.suppress_rule("c1", "missing_title").unwrap();
        assert_eq!(store.suppressed_rules("c1").unwrap(), vec!["missing_title"]);
        assert!(store.issue_counts("c1").unwrap().is_empty());
        assert_eq!(store.restore_rule("c1", "missing_title").unwrap(), 1);
        assert_eq!(store.issue_counts("c1").unwrap().len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn response_buckets_group_times() {
        let dir = std::env::temp_dir().join(format!("coqui-buckets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        for (index, time) in [50, 200, 500, 1500].into_iter().enumerate() {
            let mut record = sample_record("c1", &format!("https://example.com/{index}"));
            record.response_time_ms = Some(time);
            store.insert_url(&record).unwrap();
        }
        let counts: Vec<i64> = store
            .response_time_buckets("c1")
            .unwrap()
            .into_iter()
            .map(|(_, count)| count)
            .collect();
        assert_eq!(counts, vec![1, 1, 1, 1]);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn content_counts_classify_types() {
        let dir = std::env::temp_dir().join(format!("coqui-content-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        for (index, content_type) in [
            "text/html; charset=utf-8",
            "application/xml",
            "image/png",
            "application/pdf",
        ]
        .into_iter()
        .enumerate()
        {
            let mut record = sample_record("c1", &format!("https://example.com/{index}"));
            record.content_type = Some(content_type.into());
            store.insert_url(&record).unwrap();
        }
        assert_eq!(store.content_counts("c1").unwrap(), (1, 1, 1, 1));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn depth_counts_group_by_depth() {
        let dir = std::env::temp_dir().join(format!("coqui-depth-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        for (index, depth) in [0u32, 1, 1].into_iter().enumerate() {
            let mut record = sample_record("c1", &format!("https://example.com/{index}"));
            record.depth = depth;
            store.insert_url(&record).unwrap();
        }
        assert_eq!(store.depth_counts("c1").unwrap(), vec![(0, 1), (1, 2)]);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn indexability_counts_classify_urls() {
        let dir = std::env::temp_dir().join(format!("coqui-index-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&sample_record("c1", "https://example.com/"))
            .unwrap();
        let mut noindex = sample_record("c1", "https://example.com/noindex");
        noindex.meta_robots = Some("noindex,follow".into());
        store.insert_url(&noindex).unwrap();
        let mut blocked = sample_record("c1", "https://example.com/blocked");
        blocked.blocked = true;
        store.insert_url(&blocked).unwrap();
        assert_eq!(store.indexability_counts("c1").unwrap(), (1, 1, 1));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn response_stats_are_aggregated() {
        let dir = std::env::temp_dir().join(format!("coqui-stats-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        for (index, time) in [100, 300, 500].into_iter().enumerate() {
            let mut record = sample_record("c1", &format!("https://example.com/{index}"));
            record.response_time_ms = Some(time);
            store.insert_url(&record).unwrap();
        }
        assert_eq!(store.response_time_stats("c1").unwrap(), (3, 300, 500));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn status_counts_group_by_code() {
        let dir = std::env::temp_dir().join(format!("coqui-counts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        store
            .insert_url(&sample_record("c1", "https://example.com/"))
            .unwrap();
        let mut missing = sample_record("c1", "https://example.com/missing");
        missing.status = Some(404);
        store.insert_url(&missing).unwrap();
        let counts = store.status_counts("c1").unwrap();
        assert_eq!(counts.len(), 2);
        assert!(counts.contains(&(Some(200), 1)));
        assert!(counts.contains(&(Some(404), 1)));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn saved_views_round_trip() {
        let dir = std::env::temp_dir().join(format!("coqui-views-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .save_view("broken", Some("Missing"), Some(404), None, None, None, None)
            .unwrap();
        store
            .save_view(
                "broken",
                Some("Gone"),
                Some(410),
                Some("missing_title"),
                Some("html"),
                Some(3),
                Some("indexable"),
            )
            .unwrap();
        let views = store.saved_views().unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].search.as_deref(), Some("Gone"));
        assert_eq!(views[0].status, Some(410));
        assert_eq!(views[0].issue.as_deref(), Some("missing_title"));
        assert_eq!(views[0].content_class.as_deref(), Some("html"));
        assert_eq!(views[0].max_depth, Some(3));
        assert_eq!(views[0].indexability.as_deref(), Some("indexable"));
        assert_eq!(store.delete_view("broken").unwrap(), 1);
        assert!(store.saved_views().unwrap().is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn lighthouse_reports_round_trip() {
        let store = Store::open_in_memory().unwrap();
        let summary = r#"{"performance":0.72,"accessibility":0.95,"bestPractices":0.57,"seo":0.42,"audits":[],"finalUrl":null,"fetchTime":null}"#;
        store.save_lighthouse("c1", 3, summary).unwrap();
        assert_eq!(
            store.get_lighthouse("c1", 3).unwrap().as_deref(),
            Some(summary)
        );
        assert!(store.get_lighthouse("c1", 4).unwrap().is_none());
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

fn csv_header(key: &str) -> String {
    match key {
        "words" => "word_count".into(),
        "lcp" => "lcp_ms".into(),
        "cls" => "cls".into(),
        other => other.to_string(),
    }
}

fn url_csv_value(url: &UrlRecord, key: &str) -> String {
    match key {
        "url" => url.url.clone(),
        "status" => url
            .status
            .map(|value| value.to_string())
            .unwrap_or_default(),
        "title" => url.title.clone().unwrap_or_default(),
        "h1" => url.h1.clone().unwrap_or_default(),
        "words" => url
            .word_count
            .map(|value| value.to_string())
            .unwrap_or_default(),
        "canonical" => url.canonical.clone().unwrap_or_default(),
        "robots" => url.meta_robots.clone().unwrap_or_default(),
        "redirect" => url.redirect_target.clone().unwrap_or_default(),
        "depth" => url.depth.to_string(),
        "time" => url
            .response_time_ms
            .map(|value| value.to_string())
            .unwrap_or_default(),
        "lcp" => url
            .vitals
            .map(|vitals| vitals.lcp_ms.to_string())
            .unwrap_or_default(),
        "cls" => url
            .vitals
            .map(|vitals| vitals.cls.to_string())
            .unwrap_or_default(),
        other => other.to_string(),
    }
}

fn row_csv_value(key: &str, row: &rusqlite::Row<'_>) -> String {
    let string = |index: usize| -> String {
        row.get::<_, Option<String>>(index)
            .ok()
            .flatten()
            .unwrap_or_default()
    };
    let number = |index: usize| -> String {
        row.get::<_, Option<i64>>(index)
            .ok()
            .flatten()
            .map(|value| value.to_string())
            .unwrap_or_default()
    };
    match key {
        "url" => string(0),
        "status" => number(1),
        "title" => string(2),
        "h1" => string(3),
        "words" => number(4),
        "canonical" => string(5),
        "robots" => string(6),
        "redirect" => string(7),
        "depth" => number(8),
        "time" => number(9),
        "lcp" | "cls" => {
            let vitals = row
                .get::<_, Option<String>>(10)
                .ok()
                .flatten()
                .and_then(|json| serde_json::from_str::<WebVitals>(&json).ok());
            match (key, vitals) {
                ("lcp", Some(vitals)) => vitals.lcp_ms.to_string(),
                ("cls", Some(vitals)) => vitals.cls.to_string(),
                _ => String::new(),
            }
        }
        _ => String::new(),
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn lastmod_from_headers(json: Option<&str>) -> Option<String> {
    use time::format_description::well_known::{Rfc2822, Rfc3339};
    let headers: Vec<(String, String)> = serde_json::from_str(json?).ok()?;
    let value = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("last-modified"))?
        .1
        .clone();
    let parsed = time::OffsetDateTime::parse(&value, &Rfc2822).ok()?;
    let formatted = parsed
        .to_offset(time::UtcOffset::UTC)
        .format(&Rfc3339)
        .ok()?;
    Some(formatted.replace("+00:00", "Z"))
}
