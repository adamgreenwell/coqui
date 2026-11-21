//! Import a Screaming Frog `.dbseospider` crawl file.
//!
//! The proprietary file is a ZIP archive whose `results_*/sql` member is a
//! SQLite database. We copy that member out and map its `urls` and `links`
//! tables onto Coqui's schema.

use crate::{Result, Store, StoreError};
use coqui_core::{CrawlConfig, CrawlStatus, NewUrl, now_ms};
use rusqlite::{Connection, OpenFlags, Row};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Summary of an imported Screaming Frog crawl.
#[derive(Debug, Clone)]
pub struct SfImportStats {
    pub crawl_id: String,
    pub seed: String,
    pub urls: usize,
    pub links: usize,
}

fn text(row: &Row<'_>, name: &str) -> Option<String> {
    row.get::<_, Option<String>>(name)
        .ok()
        .flatten()
        .filter(|value| !value.is_empty())
}

fn number(row: &Row<'_>, name: &str) -> Option<i64> {
    row.get::<_, Option<i64>>(name).ok().flatten()
}

fn extract_sql_member(archive_path: &Path) -> Result<PathBuf> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| StoreError::Io(std::io::Error::other(error.to_string())))?;
    let member = archive
        .file_names()
        .filter_map(|name| name.ok())
        .find(|name| name.starts_with("results_") && name.ends_with("/sql"))
        .map(|name| name.to_string())
        .ok_or_else(|| {
            StoreError::Io(std::io::Error::other(
                "no results_*/sql member in the crawl file",
            ))
        })?;
    let mut entry = archive
        .by_name(&member)
        .map_err(|error| StoreError::Io(std::io::Error::other(error.to_string())))?;
    let destination = std::env::temp_dir().join(format!("coqui-sf-{}.db", now_ms()));
    let mut output = std::fs::File::create(&destination)?;
    std::io::copy(&mut entry, &mut output)?;
    Ok(destination)
}

fn meta_description(row: &Row<'_>) -> Option<String> {
    for index in 1..=20 {
        if text(row, &format!("meta_name_{index}"))
            .map(|name| name.eq_ignore_ascii_case("description"))
            .unwrap_or(false)
        {
            return text(row, &format!("meta_content_{index}"));
        }
    }
    None
}

impl Store {
    /// Import a Screaming Frog crawl file into a new crawl in this store.
    pub fn import_seospider(&self, path: &Path) -> Result<SfImportStats> {
        let sqlite_path = extract_sql_member(path)?;
        let source = Connection::open_with_flags(&sqlite_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(StoreError::Sqlite)?;

        let seed: String = source
            .query_row(
                "select encoded_url from urls where is_internal = 1 order by crawl_depth, rowid limit 1",
                [],
                |row| row.get(0),
            )
            .optional_seed()
            .unwrap_or_else(|| "https://imported.invalid/".to_string());
        let crawl_id = format!("sf-{}", now_ms());
        self.create_crawl(&crawl_id, &seed, "url", &CrawlConfig::default(), now_ms())?;

        let populated = (|| -> Result<(usize, usize)> {
            let mut ids: HashMap<String, i64> = HashMap::new();
            let mut urls = 0usize;
            {
                let mut statement = source
                    .prepare(
                        "select * from urls
                     where is_internal = 1 and ifnull(loaded_as_a_resource, 0) = 0
                     order by crawl_depth, rowid",
                    )
                    .map_err(StoreError::Sqlite)?;
                let mut rows = statement.query([]).map_err(StoreError::Sqlite)?;
                while let Some(row) = rows.next().map_err(StoreError::Sqlite)? {
                    let Some(url) = text(row, "encoded_url") else {
                        continue;
                    };
                    let record = NewUrl {
                        crawl_id: crawl_id.clone(),
                        url: url.clone(),
                        status: number(row, "response_code").map(|value| value as u16),
                        blocked: number(row, "blocked_by_robots_txt").unwrap_or(0) != 0,
                        content_type: text(row, "content_type"),
                        title: text(row, "title_1"),
                        meta_description: meta_description(row),
                        h1: text(row, "h1_1"),
                        h1_count: number(row, "num_h1").unwrap_or(0) as u32,
                        canonical: None,
                        meta_robots: None,
                        hreflang: Vec::new(),
                        pagination_next: None,
                        pagination_prev: None,
                        structured_data: Default::default(),
                        console_errors: Vec::new(),
                        axe: Vec::new(),
                        vitals: None,
                        headers: Vec::new(),
                        redirect_target: None,
                        redirect_chain: Vec::new(),
                        word_count: number(row, "word_count").map(|value| value as u32),
                        depth: number(row, "crawl_depth").unwrap_or(0) as u32,
                        response_time_ms: number(row, "response_time_ms").map(|value| value as u64),
                        size_bytes: number(row, "page_size").map(|value| value as u64),
                        error: number(row, "response_code")
                            .filter(|code| *code >= 400)
                            .and_then(|_| text(row, "response_msg")),
                        discovered_at: now_ms(),
                    };
                    self.insert_url(&record)?;
                    let id = self.ensure_url_id(&crawl_id, &url)?;
                    ids.insert(url, id);
                    urls += 1;
                }
            }

            let mut links = 0usize;
            {
                let mut statement = source
                .prepare(
                    "select s.encoded_url src, d.encoded_url dst, l.link_text, l.nofollow, l.scope
                     from links l
                     join urls s on s.rowid = l.src_id
                     join urls d on d.rowid = l.dst_id
                     where l.link_type = 1",
                )
                .map_err(StoreError::Sqlite)?;
                let mut rows = statement.query([]).map_err(StoreError::Sqlite)?;
                while let Some(row) = rows.next().map_err(StoreError::Sqlite)? {
                    let (Some(from), Some(to)) = (text(row, "src"), text(row, "dst")) else {
                        continue;
                    };
                    let from_id = match ids.get(&from) {
                        Some(id) => *id,
                        None => self.ensure_url_id(&crawl_id, &from)?,
                    };
                    let to_id = match ids.get(&to) {
                        Some(id) => *id,
                        None => self.ensure_url_id(&crawl_id, &to)?,
                    };
                    let internal = number(row, "scope").unwrap_or(0) == 1;
                    let rel = if number(row, "nofollow").unwrap_or(0) != 0 {
                        Some("nofollow")
                    } else {
                        None
                    };
                    self.insert_link(
                        &crawl_id,
                        from_id,
                        to_id,
                        text(row, "link_text").as_deref(),
                        rel,
                        internal,
                    )?;
                    links += 1;
                }
            }

            Ok((urls, links))
        })();
        let (urls, links) = match populated {
            Ok(pair) => pair,
            Err(error) => {
                let _ = self.delete_crawl(&crawl_id);
                std::fs::remove_file(&sqlite_path).ok();
                return Err(error);
            }
        };
        self.mark_finished(&crawl_id, CrawlStatus::Completed, now_ms())?;
        std::fs::remove_file(&sqlite_path).ok();
        Ok(SfImportStats {
            crawl_id,
            seed,
            urls,
            links,
        })
    }
}

trait OptionalSeed {
    fn optional_seed(self) -> Option<String>;
}

impl OptionalSeed for rusqlite::Result<String> {
    fn optional_seed(self) -> Option<String> {
        self.ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn imports_urls_and_links_from_a_seospider_archive() {
        let dir = std::env::temp_dir().join(format!("coqui-sf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sqlite_path = dir.join("source.db");
        {
            let conn = Connection::open(&sqlite_path).unwrap();
            conn.execute_batch(
                "create table urls (
                    encoded_url text, response_code integer, response_msg text,
                    blocked_by_robots_txt integer, content_type text, title_1 text,
                    h1_1 text, num_h1 integer, is_canonicalised integer, word_count integer,
                    crawl_depth integer, response_time_ms integer, page_size integer,
                    is_internal integer, loaded_as_a_resource integer
                );
                create table links (
                    link_type integer, src_id integer, dst_id integer,
                    link_text text, nofollow integer, scope integer
                );",
            )
            .unwrap();
            conn.execute(
                "insert into urls values ('https://example.com/', 200, 'OK', 0, 'text/html; charset=utf-8', 'Home', 'H1', 1, 0, 100, 0, 50, 1000, 1, 0)",
                [],
            )
            .unwrap();
            conn.execute(
                "insert into urls values ('https://example.com/about', 200, 'OK', 0, 'text/html; charset=utf-8', 'About', 'About', 1, 0, 50, 1, 40, 800, 1, 0)",
                [],
            )
            .unwrap();
            conn.execute("insert into links values (1, 1, 2, 'About', 0, 1)", [])
                .unwrap();
        }
        let archive_path = dir.join("crawl.dbseospider");
        {
            let file = std::fs::File::create(&archive_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("results_test/sql", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(&std::fs::read(&sqlite_path).unwrap())
                .unwrap();
            zip.finish().unwrap();
        }
        let store = Store::open(dir.join("coqui.db")).unwrap();
        let stats = store.import_seospider(&archive_path).unwrap();
        assert_eq!(stats.urls, 2);
        assert_eq!(stats.links, 1);
        assert_eq!(store.all_urls(&stats.crawl_id).unwrap().len(), 2);
        assert_eq!(store.all_links(&stats.crawl_id).unwrap().len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }
}
