//! Audit rules over a completed crawl.
//!
//! Rules run after the crawl so they can use data the crawl itself leaves behind (link targets,
//! final status codes). The rule set is intentionally small for M1; the full parity taxonomy is
//! tracked in A-01..A-09.

use coqui_core::{Issue, IssueCount, IssueSeverity, LinkRecord, UrlRecord};
use coqui_store::{Store, StoreError};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct RuleDef {
    pub id: &'static str,
    pub label: &'static str,
    pub severity: IssueSeverity,
}

pub const RULES: &[RuleDef] = &[
    RuleDef {
        id: "missing_title",
        label: "Missing title",
        severity: IssueSeverity::Error,
    },
    RuleDef {
        id: "title_too_long",
        label: "Title too long",
        severity: IssueSeverity::Warning,
    },
    RuleDef {
        id: "missing_meta_description",
        label: "Missing meta description",
        severity: IssueSeverity::Warning,
    },
    RuleDef {
        id: "multiple_h1",
        label: "Multiple H1 headings",
        severity: IssueSeverity::Warning,
    },
    RuleDef {
        id: "thin_content",
        label: "Thin content",
        severity: IssueSeverity::Opportunity,
    },
    RuleDef {
        id: "broken_internal_link",
        label: "Broken internal link",
        severity: IssueSeverity::Error,
    },
    RuleDef {
        id: "missing_h1",
        label: "Missing H1",
        severity: IssueSeverity::Warning,
    },
    RuleDef {
        id: "canonical_mismatch",
        label: "Canonical mismatch",
        severity: IssueSeverity::Warning,
    },
    RuleDef {
        id: "missing_canonical",
        label: "Missing canonical",
        severity: IssueSeverity::Opportunity,
    },
    RuleDef {
        id: "duplicate_title",
        label: "Duplicate title",
        severity: IssueSeverity::Warning,
    },
    RuleDef {
        id: "duplicate_meta_description",
        label: "Duplicate meta description",
        severity: IssueSeverity::Warning,
    },
    RuleDef {
        id: "title_too_short",
        label: "Title too short",
        severity: IssueSeverity::Opportunity,
    },
    RuleDef {
        id: "meta_description_too_long",
        label: "Meta description too long",
        severity: IssueSeverity::Warning,
    },
];

pub fn rule_label(rule: &str) -> &str {
    RULES
        .iter()
        .find(|definition| definition.id == rule)
        .map(|definition| definition.label)
        .unwrap_or(rule)
}

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}

pub type Result<T> = std::result::Result<T, AuditError>;

/// Evaluate every rule against a crawl, replace its issues, and return the new counts.
pub fn analyze_crawl(store: &Store, crawl_id: &str) -> Result<Vec<IssueCount>> {
    store.clear_issues(crawl_id)?;
    let urls = store.all_urls(crawl_id)?;
    let links = store.all_links(crawl_id)?;

    let mut status_by_url: HashMap<&str, &UrlRecord> = HashMap::new();
    for url in &urls {
        status_by_url.insert(url.url.as_str(), url);
    }

    let mut duplicate_titles: HashMap<String, Vec<String>> = HashMap::new();
    let mut duplicate_meta: HashMap<String, Vec<String>> = HashMap::new();
    for url in &urls {
        let is_ok_html = url
            .content_type
            .as_deref()
            .map(|value| value.to_ascii_lowercase().contains("text/html"))
            .unwrap_or(false)
            && url
                .status
                .map(|status| (200..300).contains(&status))
                .unwrap_or(false)
            && !url.blocked;
        if !is_ok_html {
            continue;
        }
        if let Some(title) = url
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            duplicate_titles
                .entry(title.to_ascii_lowercase())
                .or_default()
                .push(url.url.clone());
        }
        if let Some(meta) = url
            .meta_description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            duplicate_meta
                .entry(meta.to_ascii_lowercase())
                .or_default()
                .push(url.url.clone());
        }
    }

    let mut broken: HashMap<String, Vec<String>> = HashMap::new();
    for LinkRecord {
        from_url,
        to_url,
        internal,
        ..
    } in &links
    {
        if !internal {
            continue;
        }
        let Some(target) = status_by_url.get(to_url.as_str()) else {
            continue;
        };
        if target.blocked || target.status == Some(403) {
            continue;
        }
        let is_broken =
            target.status.map(|status| status >= 400).unwrap_or(true) || target.error.is_some();
        if is_broken {
            broken
                .entry(from_url.clone())
                .or_default()
                .push(to_url.clone());
        }
    }

    for url in &urls {
        let is_ok_html = url
            .content_type
            .as_deref()
            .map(|value| value.to_ascii_lowercase().contains("text/html"))
            .unwrap_or(false)
            && url
                .status
                .map(|status| (200..300).contains(&status))
                .unwrap_or(false)
            && !url.blocked;

        if !is_ok_html {
            continue;
        }

        let title = url.title.as_deref().unwrap_or("").trim();
        if title.is_empty() {
            issue(store, crawl_id, &url.url, "missing_title", None)?;
        } else if title.chars().count() > 60 {
            issue(
                store,
                crawl_id,
                &url.url,
                "title_too_long",
                Some(format!("{} characters", title.chars().count())),
            )?;
        } else if title.chars().count() < 30 {
            issue(
                store,
                crawl_id,
                &url.url,
                "title_too_short",
                Some(format!("{} characters", title.chars().count())),
            )?;
        }

        if !title.is_empty() {
            if let Some(group) = duplicate_titles.get(&title.to_ascii_lowercase()) {
                if group.len() > 1 {
                    issue(
                        store,
                        crawl_id,
                        &url.url,
                        "duplicate_title",
                        Some(format!("{} pages share this title", group.len())),
                    )?;
                }
            }
        }

        let meta = url.meta_description.as_deref().unwrap_or("").trim();
        if meta.is_empty() {
            issue(store, crawl_id, &url.url, "missing_meta_description", None)?;
        } else {
            if meta.chars().count() > 155 {
                issue(
                    store,
                    crawl_id,
                    &url.url,
                    "meta_description_too_long",
                    Some(format!("{} characters", meta.chars().count())),
                )?;
            }
            if let Some(group) = duplicate_meta.get(&meta.to_ascii_lowercase()) {
                if group.len() > 1 {
                    issue(
                        store,
                        crawl_id,
                        &url.url,
                        "duplicate_meta_description",
                        Some(format!("{} pages share this description", group.len())),
                    )?;
                }
            }
        }

        if url.h1_count == 0 {
            issue(store, crawl_id, &url.url, "missing_h1", None)?;
        }

        match url.canonical.as_deref() {
            Some(canonical) if !canonical.trim().is_empty() => {
                let normalised =
                    coqui_core::normalize_url(canonical).unwrap_or_else(|_| canonical.to_string());
                if normalised != url.url {
                    issue(
                        store,
                        crawl_id,
                        &url.url,
                        "canonical_mismatch",
                        Some(format!("canonical points to {normalised}")),
                    )?;
                }
            }
            _ => {
                issue(store, crawl_id, &url.url, "missing_canonical", None)?;
            }
        }

        if url.h1_count > 1 {
            issue(
                store,
                crawl_id,
                &url.url,
                "multiple_h1",
                Some(format!("{} H1 elements", url.h1_count)),
            )?;
        }

        if url.word_count.unwrap_or(0) < 200 {
            issue(
                store,
                crawl_id,
                &url.url,
                "thin_content",
                Some(format!("{} words", url.word_count.unwrap_or(0))),
            )?;
        }

        if let Some(targets) = broken.get(&url.url) {
            issue(
                store,
                crawl_id,
                &url.url,
                "broken_internal_link",
                Some(format!(
                    "{} broken link(s), first: {}",
                    targets.len(),
                    targets.first().cloned().unwrap_or_default()
                )),
            )?;
        }
    }

    Ok(store.issue_counts(crawl_id)?)
}

fn issue(
    store: &Store,
    crawl_id: &str,
    url: &str,
    rule: &str,
    detail: Option<String>,
) -> Result<()> {
    let severity = RULES
        .iter()
        .find(|definition| definition.id == rule)
        .map(|definition| definition.severity)
        .unwrap_or(IssueSeverity::Warning);
    store.insert_issue(&Issue {
        crawl_id: crawl_id.to_string(),
        url: url.to_string(),
        rule: rule.to_string(),
        severity,
        detail,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use coqui_core::{CrawlConfig, NewUrl, now_ms};

    fn add_url(
        store: &Store,
        crawl_id: &str,
        url: &str,
        title: Option<&str>,
        h1_count: u32,
        words: u32,
    ) {
        store
            .insert_url(&NewUrl {
                crawl_id: crawl_id.to_string(),
                url: url.to_string(),
                status: Some(200),
                blocked: false,
                content_type: Some("text/html".into()),
                title: title.map(str::to_string),
                meta_description: None,
                h1: Some("H1".into()),
                h1_count,
                canonical: None,
                meta_robots: None,
                redirect_target: None,
                redirect_chain: Vec::new(),
                word_count: Some(words),
                depth: 0,
                response_time_ms: Some(10),
                size_bytes: Some(100),
                error: None,
                discovered_at: now_ms(),
            })
            .unwrap();
    }

    #[test]
    fn finds_the_expected_issues() {
        let store = Store::open_in_memory().unwrap();
        let id = "audit";
        store
            .create_crawl(
                id,
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        add_url(&store, id, "https://example.com/", None, 2, 50);
        add_url(
            &store,
            id,
            "https://example.com/good",
            Some("A perfectly reasonable title"),
            1,
            600,
        );
        // Target status 404 so the link from home is broken.
        store
            .insert_url(&NewUrl {
                crawl_id: id.into(),
                url: "https://example.com/missing".into(),
                status: Some(404),
                blocked: false,
                content_type: Some("text/html".into()),
                title: Some("Not found".into()),
                meta_description: Some("missing".into()),
                h1: Some("Not found".into()),
                h1_count: 1,
                canonical: None,
                meta_robots: None,
                redirect_target: None,
                redirect_chain: Vec::new(),
                word_count: Some(2),
                depth: 1,
                response_time_ms: Some(5),
                size_bytes: Some(10),
                error: None,
                discovered_at: now_ms(),
            })
            .unwrap();
        let from = store.ensure_url_id(id, "https://example.com/").unwrap();
        let to = store
            .ensure_url_id(id, "https://example.com/missing")
            .unwrap();
        store
            .insert_link(id, from, to, Some("Missing"), None, true)
            .unwrap();
        let forbidden = store
            .ensure_url_id(id, "https://example.com/forbidden")
            .unwrap();
        store
            .insert_url(&NewUrl {
                crawl_id: id.into(),
                url: "https://example.com/forbidden".into(),
                status: Some(403),
                blocked: true,
                content_type: Some("text/html".into()),
                title: Some("Just a moment...".into()),
                meta_description: None,
                h1: None,
                h1_count: 0,
                canonical: None,
                meta_robots: None,
                redirect_target: None,
                redirect_chain: Vec::new(),
                word_count: Some(1),
                depth: 1,
                response_time_ms: Some(5),
                size_bytes: Some(10),
                error: None,
                discovered_at: now_ms(),
            })
            .unwrap();
        store
            .insert_link(id, from, forbidden, Some("Forbidden"), None, true)
            .unwrap();

        let counts = analyze_crawl(&store, id).unwrap();
        let by_rule: HashMap<_, _> = counts.iter().map(|c| (c.rule.as_str(), c.count)).collect();
        assert_eq!(by_rule.get("missing_title"), Some(&1));
        assert_eq!(by_rule.get("multiple_h1"), Some(&1));
        assert_eq!(by_rule.get("missing_meta_description"), Some(&2));
        // Only the home page is thin: the 404 target is not audited for content rules.
        assert_eq!(by_rule.get("thin_content"), Some(&1));
        assert_eq!(by_rule.get("broken_internal_link"), Some(&1));
        assert!(!by_rule.contains_key("title_too_long"));

        // Re-running replaces rather than duplicates.
        let again = analyze_crawl(&store, id).unwrap();
        assert_eq!(again.len(), counts.len());
    }
}

#[cfg(test)]
mod wordpress_rules {
    use super::*;
    use coqui_core::{CrawlConfig, NewUrl, now_ms};

    fn add(
        store: &Store,
        id: &str,
        url: &str,
        title: &str,
        meta: &str,
        canonical: Option<&str>,
        h1: u32,
    ) {
        store
            .insert_url(&NewUrl {
                crawl_id: id.into(),
                url: url.into(),
                status: Some(200),
                blocked: false,
                content_type: Some("text/html".into()),
                title: Some(title.into()),
                meta_description: Some(meta.into()),
                h1: if h1 > 0 { Some("Heading".into()) } else { None },
                h1_count: h1,
                canonical: canonical.map(str::to_string),
                meta_robots: None,
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
    }

    #[test]
    fn finds_canonical_duplicate_and_h1_issues() {
        let store = Store::open_in_memory().unwrap();
        let id = "wp";
        store
            .create_crawl(
                id,
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                now_ms(),
            )
            .unwrap();
        add(
            &store,
            id,
            "https://example.com/a",
            "Same title here",
            "Same description here",
            Some("https://example.com/b"),
            0,
        );
        add(
            &store,
            id,
            "https://example.com/b",
            "Same title here",
            "Same description here",
            Some("https://example.com/b"),
            1,
        );
        add(
            &store,
            id,
            "https://example.com/c",
            "A different title for c",
            "Another description",
            Some("https://example.com/c"),
            1,
        );

        let counts = analyze_crawl(&store, id).unwrap();
        let by_rule: HashMap<_, _> = counts.iter().map(|c| (c.rule.as_str(), c.count)).collect();
        assert_eq!(by_rule.get("canonical_mismatch"), Some(&1));
        assert_eq!(by_rule.get("missing_h1"), Some(&1));
        assert_eq!(by_rule.get("duplicate_title"), Some(&2));
        assert_eq!(by_rule.get("duplicate_meta_description"), Some(&2));
        assert!(!by_rule.contains_key("missing_canonical"));
    }
}
