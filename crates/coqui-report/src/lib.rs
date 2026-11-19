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
    slowest.sort_by(|left, right| right.0.cmp(&left.0));
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
];

/// Render a named report.
pub fn render_report(store: &Store, crawl_id: &str, kind: &str) -> Result<String, String> {
    match kind {
        "issues" => render_issues(store, crawl_id),
        "status" => render_status(store, crawl_id),
        "performance" => render_performance(store, crawl_id),
        _ => render_html(store, crawl_id),
    }
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
         th{{background:#eef5f2}}.muted{{color:#5d6f6b}}</style></head><body>\
         <h1>{title}</h1><p class=\"muted\">{}</p>{body}</body></html>",
        escape(crawl)
    )
}

fn render_issues(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let issues = store
        .issue_counts(crawl_id)
        .map_err(|error| error.to_string())?;
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

fn render_status(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
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

fn render_performance(store: &Store, crawl_id: &str) -> Result<String, String> {
    let crawl = store
        .get_crawl(crawl_id)
        .map_err(|error| error.to_string())?;
    let urls = store
        .all_urls(crawl_id)
        .map_err(|error| error.to_string())?;
    let mut slowest: Vec<_> = urls
        .iter()
        .filter_map(|url| url.response_time_ms.map(|ms| (ms, url.url.clone())))
        .collect();
    slowest.sort_by(|left, right| right.0.cmp(&left.0));
    slowest.truncate(25);
    let mut body = String::from("<table><tr><th>URL</th><th>ms</th></tr>");
    for (ms, url) in &slowest {
        body.push_str(&format!("<tr><td>{}</td><td>{ms}</td></tr>", escape(url)));
    }
    body.push_str("</table>");
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
        std::fs::remove_dir_all(dir).ok();
    }
}
