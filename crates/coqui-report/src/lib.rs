//! Static HTML reports for a crawl.

use coqui_store::Store;
use std::collections::BTreeMap;
use std::path::Path;

/// Render a self-contained HTML report for one crawl.
pub fn render_html(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    let issues = store
        .issue_counts(crawl_id)
        .map_err(|error| error.to_string())?;

    let mut statuses: BTreeMap<u16, usize> = BTreeMap::new();
    let mut blocked = 0usize;
    let mut noindex = 0usize;
    for url in &urls {
        if url.blocked {
            blocked += 1;
        }
        if url
            .meta_robots
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("noindex")
        {
            noindex += 1;
        }
        *statuses.entry(url.status.unwrap_or(0)).or_default() += 1;
    }
    let mut slowest: Vec<_> = urls
        .iter()
        .filter_map(|url| url.response_time_ms.map(|ms| (ms, url.url.clone())))
        .collect();
    slowest.sort_by_key(|item| std::cmp::Reverse(item.0));
    slowest.truncate(10);
    let total_issues: i64 = issues.iter().map(|count| count.count).sum();

    let mut html = String::new();
    html.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    html.push_str("<title>Coqui crawl report</title><style>");
    html.push_str(
        "body{font-family:system-ui,sans-serif;margin:32px;color:#1c2321;background:#f7faf9}\
         h1{margin:0 0 4px}h2{margin-top:32px}table{border-collapse:collapse;width:100%;background:#fff}\
         th,td{border:1px solid #d8e2df;padding:6px 10px;text-align:left;font-size:14px}\
         th{background:#eef5f2}.muted{color:#5d6f6b}.card{background:#fff;border:1px solid #d8e2df;\
         padding:16px;border-radius:8px;margin-top:16px}",
    );
    html.push_str("</style></head><body>");
    html.push_str("<h1>Coqui crawl report</h1>");
    html.push_str(&format!(
        "<p class=\"muted\">{} · crawl {} · generated {}</p>",
        escape(&crawl.seed),
        escape(&crawl.id),
        coqui_core::now_ms() / 1000
    ));
    html.push_str("<div class=\"card\"><strong>Summary</strong><br>");
    html.push_str(&format!(
        "{} URLs · {} issues · {} blocked · {} noindex",
        urls.len(),
        total_issues,
        blocked,
        noindex
    ));
    if let (Some(started), Some(finished)) = (crawl.started_at, crawl.finished_at) {
        let seconds = (finished - started) as f64 / 1000.0;
        if seconds > 0.0 {
            html.push_str(&format!("<br>{:.1} URLs/s", urls.len() as f64 / seconds));
        }
    }
    html.push_str("</div>");

    html.push_str("<h2>Status codes</h2><table><tr><th>Status</th><th>URLs</th></tr>");
    for (status, count) in &statuses {
        let label = if *status == 0 {
            "No response".to_string()
        } else {
            status.to_string()
        };
        html.push_str(&format!("<tr><td>{label}</td><td>{count}</td></tr>"));
    }
    html.push_str("</table>");

    html.push_str("<h2>Issues</h2><table><tr><th>Rule</th><th>URLs</th></tr>");
    for issue in &issues {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            escape(&issue.rule),
            issue.count
        ));
    }
    html.push_str("</table>");

    if !slowest.is_empty() {
        html.push_str("<h2>Slowest URLs</h2><table><tr><th>URL</th><th>ms</th></tr>");
        for (ms, url) in &slowest {
            html.push_str(&format!("<tr><td>{}</td><td>{ms}</td></tr>", escape(url)));
        }
        html.push_str("</table>");
    }
    html.push_str("</body></html>");
    Ok(html)
}

/// Write the HTML report and return the number of URLs covered.
pub fn write_html(store: &Store, crawl_id: &str, path: impl AsRef<Path>) -> Result<u64, String> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let html = render_html(store, crawl_id)?;
    std::fs::write(path, html).map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    Ok(urls.len() as u64)
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Options applied when rendering a report.
#[derive(Debug, Clone, Default)]
pub struct ReportOptions {
    /// Maximum rows in list-based reports (performance, issues).
    pub top: Option<usize>,
    /// Restrict to a single HTTP status code.
    pub status: Option<u16>,
    /// Override the report title (document title and H1).
    pub title: Option<String>,
}

/// Named reports available from the report engine.
pub const REPORTS: &[(&str, &str, &str)] = &[
    (
        "summary",
        "Crawl summary",
        "Statuses, issues and slowest URLs.",
    ),
    (
        "issues",
        "Issue breakdown",
        "Issue counts across the crawl.",
    ),
    ("status", "Status codes", "Response status distribution."),
    (
        "performance",
        "Performance",
        "Slowest URLs by response time.",
    ),
    (
        "redirects",
        "Redirects",
        "3xx responses with their targets.",
    ),
    (
        "broken-links",
        "Broken links",
        "4xx/5xx and failed responses.",
    ),
    (
        "overview",
        "Crawl overview",
        "Status, indexability, content, depth and issue totals.",
    ),
    ("hreflang", "Hreflang", "Hreflang link pairs per page."),
    (
        "canonical-chains",
        "Canonical chains",
        "Canonical targets followed across the crawl, with loops.",
    ),
    (
        "pagination",
        "Pagination",
        "rel=next/prev targets and whether they resolve to 200.",
    ),
    (
        "orphans",
        "Orphan pages",
        "Indexable 200 pages with no internal inlinks.",
    ),
    (
        "segments",
        "Segments overview",
        "URL counts per configured segment.",
    ),
    (
        "serp",
        "SERP summary",
        "Titles, meta descriptions, H1s and word counts with flags.",
    ),
    (
        "canonicals",
        "Canonicals",
        "Self, non-self and missing canonical declarations.",
    ),
];

/// Render a named report.
pub fn render_report(store: &Store, crawl_id: &str, kind: &str) -> Result<String, String> {
    render_report_with(store, crawl_id, kind, &ReportOptions::default())
}

/// Render a named report with options.
pub fn render_report_with(
    store: &Store,
    crawl_id: &str,
    kind: &str,
    options: &ReportOptions,
) -> Result<String, String> {
    match kind {
        "issues" => render_issues(store, crawl_id, options.top),
        "status" => render_status(store, crawl_id, options.status),
        "performance" => render_performance(store, crawl_id, options.top, options.status),
        "redirects" => render_redirects(store, crawl_id),
        "broken-links" => render_broken_links(store, crawl_id),
        "overview" => render_overview(store, crawl_id),
        "hreflang" => render_hreflang(store, crawl_id),
        "canonical-chains" => render_canonical_chains(store, crawl_id),
        "pagination" => render_pagination(store, crawl_id),
        "orphans" => render_orphans(store, crawl_id),
        "segments" => render_segments(store, crawl_id),
        "serp" => render_serp(store, crawl_id),
        "canonicals" => render_canonicals(store, crawl_id),
        _ => render_html(store, crawl_id),
    }
}

/// Write a named report with options and return the number of URLs covered.
pub fn write_report_with(
    store: &Store,
    crawl_id: &str,
    path: impl AsRef<Path>,
    kind: &str,
    options: &ReportOptions,
) -> Result<u64, String> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let html = render_report_with(store, crawl_id, kind, options)?;
    std::fs::write(path, html).map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    Ok(urls.len() as u64)
}

/// Write a named report and return the number of URLs covered.
pub fn write_report(
    store: &Store,
    crawl_id: &str,
    path: impl AsRef<Path>,
    kind: &str,
) -> Result<u64, String> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let html = render_report(store, crawl_id, kind)?;
    std::fs::write(path, html).map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    Ok(urls.len() as u64)
}

fn shell(crawl: &str, title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>body{{font-family:system-ui,sans-serif;margin:32px;color:#1c2321;background:#f7faf9}}\
         table{{border-collapse:collapse;width:100%;background:#fff}}\
         th,td{{border:1px solid #d8e2df;padding:6px 10px;text-align:left;font-size:14px}}\
         th{{background:#eef5f2}}.muted{{color:#5d6f6b}}\
         @page{{size:A4;margin:12mm}}\
         @media print{{body{{margin:0;background:#fff;color:#000}}h2{{break-after:avoid}}\
         table{{break-inside:avoid}}tr{{break-inside:avoid}}}}\
         </style></head><body>\
         <h1>{title}</h1><p class=\"muted\">{}</p>{body}</body></html>",
        escape(crawl)
    )
}

fn render_issues(store: &Store, crawl_id: &str, top: Option<usize>) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let mut issues = store
        .issue_counts(crawl_id)
        .map_err(|error| error.to_string())?;
    if let Some(top) = top {
        issues.truncate(top);
    }
    let mut body = String::from("<table><tr><th>Rule</th><th>URLs</th></tr>");
    for issue in &issues {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            escape(&issue.rule),
            issue.count
        ));
    }
    body.push_str("</table>");
    Ok(shell(&crawl.seed, "Issue breakdown", &body))
}

fn render_status(
    store: &Store,
    crawl_id: &str,
    status_filter: Option<u16>,
) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls: Vec<_> = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|url| status_filter.is_none_or(|status| url.status == Some(status)))
        .collect();
    let mut statuses: BTreeMap<u16, usize> = BTreeMap::new();
    for url in &urls {
        *statuses.entry(url.status.unwrap_or(0)).or_default() += 1;
    }
    let mut body = String::from("<table><tr><th>Status</th><th>URLs</th></tr>");
    for (status, count) in &statuses {
        let label = if *status == 0 {
            "No response".to_string()
        } else {
            status.to_string()
        };
        body.push_str(&format!("<tr><td>{label}</td><td>{count}</td></tr>"));
    }
    body.push_str("</table>");
    Ok(shell(&crawl.seed, "Status codes", &body))
}

fn render_redirects(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    let mut body = String::from("<table><tr><th>URL</th><th>Status</th><th>Location</th></tr>");
    for url in urls
        .iter()
        .filter(|url| matches!(url.status, Some(301 | 302 | 303 | 307 | 308)))
    {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&url.url),
            url.status.unwrap_or_default(),
            escape(url.redirect_target.as_deref().unwrap_or(""))
        ));
    }
    body.push_str("</table>");
    Ok(shell(&crawl.seed, "Redirects", &body))
}

/// Compare issue counts between two crawls.
pub fn render_comparison(store: &Store, left_id: &str, right_id: &str) -> Result<String, String> {
    let left = store
        .get_crawl(left_id)
        .map_err(|error| error.to_string())?;
    let right = store
        .get_crawl(right_id)
        .map_err(|error| error.to_string())?;
    let left_issues = store
        .issue_counts(left_id)
        .map_err(|error| error.to_string())?;
    let right_issues = store
        .issue_counts(right_id)
        .map_err(|error| error.to_string())?;
    let mut rules: std::collections::BTreeMap<String, (i64, i64)> =
        std::collections::BTreeMap::new();
    for issue in left_issues {
        rules.entry(issue.rule).or_default().0 += issue.count;
    }
    for issue in right_issues {
        rules.entry(issue.rule).or_default().1 += issue.count;
    }
    let mut body = format!(
        "<p>Left: <code>{}</code> ({})<br>Right: <code>{}</code> ({})</p>",
        escape(&left.seed),
        left.id,
        escape(&right.seed),
        right.id
    );
    body.push_str("<table><tr><th>Rule</th><th>Left</th><th>Right</th><th>Delta</th></tr>");
    for (rule, (left_count, right_count)) in rules {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{left_count}</td><td>{right_count}</td><td>{:+}</td></tr>",
            escape(&rule),
            right_count - left_count
        ));
    }
    body.push_str("</table>");
    Ok(shell(&left.seed, "Crawl comparison", &body))
}

/// Write a two-crawl comparison report.
pub fn write_comparison(
    store: &Store,
    left_id: &str,
    right_id: &str,
    path: &std::path::Path,
) -> Result<u64, String> {
    let html = render_comparison(store, left_id, right_id)?;
    std::fs::write(path, html).map_err(|error| error.to_string())?;
    store
        .total_urls(left_id)
        .map(|count| count as u64)
        .map_err(|error| error.to_string())
}

fn render_canonicals(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    let statuses: std::collections::HashMap<&str, Option<u16>> = urls
        .iter()
        .map(|url| (url.url.as_str(), url.status))
        .collect();
    let mut rows = Vec::new();
    let mut self_canonical = 0usize;
    let mut non_self = 0usize;
    let mut missing = 0usize;
    for url in urls
        .iter()
        .filter(|url| url.status == Some(200) && !url.blocked)
    {
        match url.canonical.as_deref() {
            None => {
                missing += 1;
                rows.push((url.url.clone(), String::new(), "missing", String::new()));
            }
            Some(canonical) if canonical.trim_end_matches('/') == url.url.trim_end_matches('/') => {
                self_canonical += 1;
                rows.push((
                    url.url.clone(),
                    canonical.to_string(),
                    "self",
                    String::new(),
                ));
            }
            Some(canonical) => {
                non_self += 1;
                let status = statuses
                    .get(canonical)
                    .copied()
                    .flatten()
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "not crawled".into());
                rows.push((url.url.clone(), canonical.to_string(), "non-self", status));
            }
        }
    }
    let mut body = format!(
        "<p class=\"muted\">{} pages: {self_canonical} self, {non_self} non-self, {missing} missing</p>",
        rows.len()
    );
    body.push_str(
        "<table><tr><th>URL</th><th>Canonical</th><th>Type</th><th>Target status</th></tr>",
    );
    for (url, canonical, kind, status) in &rows {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{kind}</td><td>{}</td></tr>",
            escape(url),
            escape(canonical),
            escape(status)
        ));
    }
    body.push_str("</table>");
    Ok(shell(&crawl.seed, "Canonicals", &body))
}

fn render_serp(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    let eligible: Vec<&coqui_core::UrlRecord> = urls
        .iter()
        .filter(|url| {
            url.status == Some(200)
                && !url.blocked
                && url
                    .content_type
                    .as_deref()
                    .map(|value| {
                        let value = value.to_ascii_lowercase();
                        value.contains("text/html") || value.contains("application/xhtml")
                    })
                    .unwrap_or(false)
                && !url
                    .meta_robots
                    .as_deref()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains("noindex")
        })
        .collect();
    let mut title_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut meta_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for url in &eligible {
        if let Some(title) = url.title.as_deref() {
            *title_counts.entry(title.to_ascii_lowercase()).or_default() += 1;
        }
        if let Some(meta) = url.meta_description.as_deref() {
            *meta_counts.entry(meta.to_ascii_lowercase()).or_default() += 1;
        }
    }
    let mut rows = Vec::new();
    let mut flagged = 0usize;
    for url in &eligible {
        let mut flags = Vec::new();
        match url.title.as_deref() {
            None => flags.push("missing title"),
            Some(title) => {
                if title.chars().count() > 60 {
                    flags.push("title > 60 chars");
                }
                if title_counts
                    .get(&title.to_ascii_lowercase())
                    .copied()
                    .unwrap_or(0)
                    > 1
                {
                    flags.push("duplicate title");
                }
            }
        }
        match url.meta_description.as_deref() {
            None => flags.push("missing meta"),
            Some(meta) => {
                if meta.chars().count() > 160 {
                    flags.push("meta > 160 chars");
                }
                if meta_counts
                    .get(&meta.to_ascii_lowercase())
                    .copied()
                    .unwrap_or(0)
                    > 1
                {
                    flags.push("duplicate meta");
                }
            }
        }
        if url.h1.is_none() {
            flags.push("missing H1");
        }
        if url.word_count.unwrap_or(0) < 200 {
            flags.push("thin content");
        }
        if !flags.is_empty() {
            flagged += 1;
        }
        rows.push((
            url.url.clone(),
            url.title.clone().unwrap_or_default(),
            url.meta_description
                .as_deref()
                .unwrap_or("")
                .chars()
                .count(),
            url.h1.clone().unwrap_or_default(),
            url.word_count.unwrap_or(0),
            flags.join(", "),
        ));
    }
    let mut body = format!(
        "<p class=\"muted\">{} eligible HTML pages; {flagged} need SERP attention</p>",
        rows.len()
    );
    body.push_str("<table><tr><th>URL</th><th>Title</th><th>Meta length</th><th>H1</th><th>Words</th><th>Flags</th></tr>");
    for (url, title, meta_length, h1, words, flags) in &rows {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{meta_length}</td><td>{}</td><td>{words}</td><td>{}</td></tr>",
            escape(url),
            escape(title),
            escape(h1),
            escape(flags)
        ));
    }
    body.push_str("</table>");
    Ok(shell(&crawl.seed, "SERP summary", &body))
}

fn render_segments(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let segments = store
        .segment_counts(crawl_id)
        .map_err(|error| error.to_string())?;
    let total: i64 = segments.iter().map(|(_, count)| *count).sum();
    let mut body = format!(
        "<p class=\"muted\">{total} URLs across {} segments</p>",
        segments.len()
    );
    body.push_str("<table><tr><th>Segment</th><th>URLs</th><th>Share</th></tr>");
    for (segment, count) in &segments {
        let share = if total > 0 {
            *count as f64 * 100.0 / total as f64
        } else {
            0.0
        };
        body.push_str(&format!(
            "<tr><td>{}</td><td>{count}</td><td>{share:.1}%</td></tr>",
            escape(segment)
        ));
    }
    body.push_str("</table>");
    Ok(shell(&crawl.seed, "Segments", &body))
}

fn render_orphans(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    let links = store
        .all_links(crawl_id)
        .map_err(|error| error.to_string())?;
    let inlinks: std::collections::HashSet<&str> = links
        .iter()
        .filter(|link| link.internal && link.from_url != link.to_url)
        .map(|link| link.to_url.as_str())
        .collect();
    let mut orphans = Vec::new();
    for url in &urls {
        if url.url.trim_end_matches('/') == crawl.seed.trim_end_matches('/')
            || url.status != Some(200)
            || url.blocked
        {
            continue;
        }
        let noindex = url
            .meta_robots
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("noindex");
        if noindex || inlinks.contains(url.url.as_str()) {
            continue;
        }
        orphans.push(url);
    }
    let mut body = format!(
        "<p class=\"muted\">{} orphan pages with no internal inlinks</p>",
        orphans.len()
    );
    body.push_str("<table><tr><th>URL</th><th>Title</th><th>Depth</th></tr>");
    for url in &orphans {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&url.url),
            escape(url.title.as_deref().unwrap_or("")),
            url.depth
        ));
    }
    body.push_str("</table>");
    Ok(shell(&crawl.seed, "Orphan pages", &body))
}

fn render_pagination(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    let statuses: std::collections::HashMap<&str, Option<u16>> = urls
        .iter()
        .map(|url| (url.url.as_str(), url.status))
        .collect();
    let crawled: std::collections::HashSet<&str> =
        urls.iter().map(|url| url.url.as_str()).collect();
    let describe = |target: Option<&str>| -> (String, bool, bool) {
        let Some(target) = target else {
            return (String::new(), false, false);
        };
        match statuses.get(target) {
            None => ("not crawled".to_string(), false, true),
            Some(Some(200)) => ("200".to_string(), false, false),
            Some(Some(status)) => (status.to_string(), true, false),
            Some(None) => ("no response".to_string(), true, false),
        }
    };
    let mut rows = Vec::new();
    let mut non_200 = 0usize;
    let mut no_response = 0usize;
    let mut uncrawled = 0usize;
    for url in &urls {
        if url.pagination_next.is_none() && url.pagination_prev.is_none() {
            continue;
        }
        let (next_label, next_non_200, next_uncrawled) = describe(url.pagination_next.as_deref());
        let (prev_label, prev_non_200, prev_uncrawled) = describe(url.pagination_prev.as_deref());
        non_200 += usize::from(next_non_200) + usize::from(prev_non_200);
        uncrawled += usize::from(next_uncrawled) + usize::from(prev_uncrawled);
        if url.pagination_next.as_deref().is_some_and(|next| {
            crawled.contains(next) && statuses.get(next).copied().flatten().is_none()
        }) {
            no_response += 1;
        }
        if url.pagination_prev.as_deref().is_some_and(|prev| {
            crawled.contains(prev) && statuses.get(prev).copied().flatten().is_none()
        }) {
            no_response += 1;
        }
        rows.push((
            url.url.clone(),
            url.pagination_prev.clone(),
            prev_label,
            url.pagination_next.clone(),
            next_label,
        ));
    }
    let mut body = format!(
        "<p class=\"muted\">{} pages use pagination; {non_200} targets non-200, {no_response} failed without a response, {uncrawled} not crawled</p>",
        rows.len()
    );
    body.push_str(
        "<table><tr><th>Page</th><th>Previous</th><th>Prev status</th><th>Next</th><th>Next status</th></tr>",
    );
    for (page, prev, prev_status, next, next_status) in &rows {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(page),
            escape(prev.as_deref().unwrap_or("")),
            escape(prev_status),
            escape(next.as_deref().unwrap_or("")),
            escape(next_status)
        ));
    }
    body.push_str("</table>");
    Ok(shell(&crawl.seed, "Pagination", &body))
}

fn render_canonical_chains(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    let canonicals: std::collections::HashMap<&str, &str> = urls
        .iter()
        .filter_map(|url| {
            url.canonical
                .as_deref()
                .map(|canonical| (url.url.as_str(), canonical))
        })
        .collect();
    let mut chains = Vec::new();
    for url in &urls {
        let mut chain = vec![url.url.clone()];
        let mut seen = std::collections::HashSet::new();
        seen.insert(url.url.clone());
        let mut current = url.url.as_str();
        while let Some(target) = canonicals.get(current).copied() {
            if target == current {
                break;
            }
            if !seen.insert(target.to_string()) {
                chain.push(format!("{target} (loop)"));
                break;
            }
            chain.push(target.to_string());
            current = target;
        }
        if chain.len() > 1 {
            chains.push((url.url.clone(), chain));
        }
    }
    let mut body = format!(
        "<p class=\"muted\">{} URLs with a non-self canonical chain</p>",
        chains.len()
    );
    body.push_str("<table><tr><th>Start</th><th>Canonical target</th><th>Chain</th></tr>");
    for (start, chain) in &chains {
        let canonical = chain.get(1).cloned().unwrap_or_default();
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(start),
            escape(&canonical),
            escape(&chain.join(" → "))
        ));
    }
    body.push_str("</table>");
    Ok(shell(&crawl.seed, "Canonical chains", &body))
}

fn render_hreflang(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let rows = store
        .hreflang_rows(crawl_id)
        .map_err(|error| error.to_string())?;
    let pages = rows
        .iter()
        .map(|(page, _, _)| page)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let mut body = format!(
        "<p class=\"muted\">{} hreflang links across {pages} pages</p>",
        rows.len()
    );
    body.push_str("<table><tr><th>Page</th><th>Hreflang</th><th>Target</th></tr>");
    for (page, lang, target) in &rows {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(page),
            escape(lang),
            escape(target)
        ));
    }
    body.push_str("</table>");
    Ok(shell(&crawl.seed, "Hreflang", &body))
}

fn render_overview(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let mut body = String::from("<h2>Status</h2><table><tr><th>Status</th><th>URLs</th></tr>");
    for (status, count) in store
        .status_counts(crawl_id)
        .map_err(|error| error.to_string())?
    {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{count}</td></tr>",
            status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".into())
        ));
    }
    body.push_str("</table>");
    let (indexable, noindex, blocked) = store
        .indexability_counts(crawl_id)
        .map_err(|error| error.to_string())?;
    body.push_str(&format!(
        "<h2>Indexability</h2><table><tr><th>Indexable</th><th>Noindex</th><th>Blocked</th></tr>\
         <tr><td>{indexable}</td><td>{noindex}</td><td>{blocked}</td></tr></table>"
    ));
    let (html, xml, image, other) = store
        .content_counts(crawl_id)
        .map_err(|error| error.to_string())?;
    body.push_str(&format!(
        "<h2>Content types</h2><table><tr><th>HTML</th><th>XML</th><th>Images</th><th>Other</th></tr>\
         <tr><td>{html}</td><td>{xml}</td><td>{image}</td><td>{other}</td></tr></table>"
    ));
    body.push_str("<h2>Depth</h2><table><tr><th>Depth</th><th>URLs</th></tr>");
    for (depth, count) in store
        .depth_counts(crawl_id)
        .map_err(|error| error.to_string())?
    {
        body.push_str(&format!("<tr><td>{depth}</td><td>{count}</td></tr>"));
    }
    body.push_str("</table>");
    let issues = store
        .issue_counts(crawl_id)
        .map_err(|error| error.to_string())?;
    let mut totals = [0i64; 3];
    for issue in &issues {
        let index = match issue.severity {
            coqui_core::IssueSeverity::Error => 0,
            coqui_core::IssueSeverity::Warning => 1,
            coqui_core::IssueSeverity::Opportunity => 2,
        };
        totals[index] += issue.count;
    }
    body.push_str(&format!(
        "<h2>Issues</h2><table><tr><th>Severity</th><th>Count</th></tr>\
         <tr><td>Error</td><td>{}</td></tr><tr><td>Warning</td><td>{}</td></tr>\
         <tr><td>Opportunity</td><td>{}</td></tr></table>",
        totals[0], totals[1], totals[2]
    ));
    Ok(shell(&crawl.seed, "Crawl overview", &body))
}

fn render_broken_links(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    let mut body = String::from("<table><tr><th>URL</th><th>Status</th><th>Error</th></tr>");
    for url in urls
        .iter()
        .filter(|url| url.error.is_some() || url.status.map(|status| status >= 400).unwrap_or(true))
    {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&url.url),
            url.status
                .map(|status| status.to_string())
                .unwrap_or_else(|| "-".into()),
            escape(url.error.as_deref().unwrap_or(""))
        ));
    }
    body.push_str("</table>");
    Ok(shell(&crawl.seed, "Broken links", &body))
}

fn render_performance(
    store: &Store,
    crawl_id: &str,
    top: Option<usize>,
    status_filter: Option<u16>,
) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls: Vec<_> = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|url| status_filter.is_none_or(|status| url.status == Some(status)))
        .collect();
    let mut slowest: Vec<_> = urls
        .iter()
        .filter_map(|url| url.response_time_ms.map(|ms| (ms, url.url.clone())))
        .collect();
    slowest.sort_by_key(|item| std::cmp::Reverse(item.0));
    slowest.truncate(top.unwrap_or(25));
    let mut body = String::new();
    if let (Some(started), Some(finished)) = (crawl.started_at, crawl.finished_at) {
        let seconds = (finished - started) as f64 / 1000.0;
        if seconds > 0.0 {
            body.push_str(&format!(
                "<p class=\"muted\">{:.1} URLs/s over {:.1}s</p>",
                urls.len() as f64 / seconds,
                seconds
            ));
        }
    }
    body.push_str("<table><tr><th>URL</th><th>ms</th></tr>");
    for (ms, url) in &slowest {
        body.push_str(&format!("<tr><td>{}</td><td>{ms}</td></tr>", escape(url)));
    }
    body.push_str("</table>");
    let timeline = store.throughput_buckets(crawl_id).unwrap_or_default();
    if !timeline.is_empty() {
        body.push_str("<h2>Throughput by second</h2><table><tr><th>Second</th><th>URLs</th></tr>");
        for (second, count) in timeline.iter().take(120) {
            body.push_str(&format!("<tr><td>{second}</td><td>{count}</td></tr>"));
        }
        body.push_str("</table>");
    }
    Ok(shell(&crawl.seed, "Performance", &body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use coqui_core::CrawlConfig;

    #[test]
    fn renders_summary_and_status() {
        let dir = std::env::temp_dir().join(format!("coqui-report-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("coqui.db")).unwrap();
        store
            .create_crawl(
                "c1",
                "https://example.com/",
                "url",
                &CrawlConfig::default(),
                coqui_core::now_ms(),
            )
            .unwrap();
        let rows = store.all_urls("c1").unwrap();
        assert!(rows.is_empty());
        let html = render_html(&store, "c1").unwrap();
        assert!(html.contains("https://example.com/"));
        assert!(html.contains("Status codes"));
        assert!(
            render_comparison(&store, "c1", "c1")
                .unwrap()
                .contains("Crawl comparison")
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
