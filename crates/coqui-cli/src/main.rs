//! Headless Coqui CLI. First cut: `coqui crawl`.

use coqui_core::{CrawlConfig, CrawlProgress, now_ms};
use coqui_store::Store;
use serde::Deserialize;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileConfig {
    out: Option<String>,
    max_urls: Option<u32>,
    depth: Option<u32>,
    concurrency: Option<u16>,
    respect_robots: Option<bool>,
    user_agent: Option<String>,
    headers: Option<Vec<String>>,
    auth: Option<String>,
    timeout_ms: Option<u64>,
    render: Option<bool>,
    screenshots: Option<bool>,
    accessibility: Option<bool>,
    web_vitals: Option<bool>,
    render_workers: Option<u32>,
}

impl Args {
    fn from_config(config: &FileConfig) -> Self {
        let (auth_user, auth_password) = config
            .auth
            .as_deref()
            .and_then(|value| value.split_once(':'))
            .map(|(user, password)| (user.to_string(), password.to_string()))
            .map_or((None, None), |(user, password)| {
                (Some(user), Some(password))
            });
        Args {
            kind: String::new(),
            seed: String::new(),
            out: config
                .out
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("coqui-out")),
            max_urls: config.max_urls.unwrap_or(5000),
            depth: config.depth.unwrap_or(3),
            concurrency: config.concurrency.unwrap_or(8),
            respect_robots: config.respect_robots.unwrap_or(true),
            user_agent: config.user_agent.clone(),
            headers: config.headers.clone().unwrap_or_default(),
            auth: auth_user.zip(auth_password),
            timeout_ms: config.timeout_ms.unwrap_or(20_000),
            render: config.render.unwrap_or(false),
            screenshots: config.screenshots.unwrap_or(false),
            accessibility: config.accessibility.unwrap_or(false),
            web_vitals: config.web_vitals.unwrap_or(false),
            render_workers: config.render_workers.unwrap_or(2),
            store_text: false,
            webhook_url: None,
            extractors: Vec::new(),
            custom_js: Vec::new(),
            resume: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    kind: String,
    seed: String,
    out: PathBuf,
    max_urls: u32,
    depth: u32,
    concurrency: u16,
    respect_robots: bool,
    user_agent: Option<String>,
    headers: Vec<String>,
    auth: Option<(String, String)>,
    timeout_ms: u64,
    render: bool,
    screenshots: bool,
    accessibility: bool,
    web_vitals: bool,
    render_workers: u32,
    store_text: bool,
    webhook_url: Option<String>,
    extractors: Vec<(String, String)>,
    custom_js: Vec<String>,
    resume: bool,
}

fn load_config(path: &Path, required: bool) -> Result<FileConfig, String> {
    if !path.exists() {
        if required {
            return Err(format!("config not found: {}", path.display()));
        }
        return Ok(FileConfig::default());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    toml::from_str(&text).map_err(|error| format!("parse {}: {error}", path.display()))
}

fn usage() -> &'static str {
    "coqui crawl <url|sitemap|list> <seed> [--out DIR] [--max-urls N] [--depth N] [--concurrency N] [--ignore-robots] [--webhook URL]\n       coqui resume [--out DIR] [--max-urls N] [--concurrency N]\n       coqui report [summary|issues|status|performance|redirects|broken-links|overview] [--out DIR]\n       coqui report compare --with <crawl id> [--out DIR]\n       coqui reports\n       coqui export [--out DIR] [--family urls|issues|links|headers|images|a11y|structured|vitals|console|text|markdown|extractions|custom-js]\n       coqui extractions [--out DIR]\n       coqui sitemap [--out DIR] [--split] [--gzip]\n       coqui schedule add <kind> <seed> --every SECONDS [--out DIR] [--max-urls N] [--webhook URL]\n       coqui schedule list|remove <id>|run-due [--out DIR]\n       coqui serve [--out DIR] [--addr 127.0.0.1:8787]
       coqui issue-settings <export|import|diff> [--out DIR] [--file PATH] [--with CRAWL]
       coqui doctor [--out DIR]
       coqui mcp [--out DIR]"
}

fn parse_args(values: &[String], config: &FileConfig) -> Result<Args, String> {
    if values.len() < 3 || values[1] != "crawl" {
        return Err(usage().to_string());
    }
    let kind = values[2].clone();
    if !matches!(kind.as_str(), "url" | "sitemap" | "list") {
        return Err(format!("unknown seed kind '{kind}'"));
    }
    let seed = values
        .get(3)
        .ok_or_else(|| "missing seed".to_string())?
        .clone();
    let mut args = Args::from_config(config);
    args.kind = kind;
    args.seed = seed;
    let mut index = 4;
    while index < values.len() {
        match values[index].as_str() {
            "--config" => {
                index += 1;
            }
            "--out" => {
                index += 1;
                args.out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            "--max-urls" => {
                index += 1;
                args.max_urls = values
                    .get(index)
                    .ok_or("missing --max-urls value")?
                    .parse()
                    .map_err(|_| "invalid --max-urls".to_string())?;
            }
            "--depth" => {
                index += 1;
                args.depth = values
                    .get(index)
                    .ok_or("missing --depth value")?
                    .parse()
                    .map_err(|_| "invalid --depth".to_string())?;
            }
            "--concurrency" => {
                index += 1;
                args.concurrency = values
                    .get(index)
                    .ok_or("missing --concurrency value")?
                    .parse()
                    .map_err(|_| "invalid --concurrency".to_string())?;
            }
            "--ignore-robots" => args.respect_robots = false,
            "--render" => args.render = true,
            "--screenshots" => args.screenshots = true,
            "--a11y" => args.accessibility = true,
            "--vitals" => args.web_vitals = true,
            "--store-text" => args.store_text = true,
            "--webhook" => {
                index += 1;
                args.webhook_url =
                    Some(values.get(index).ok_or("missing --webhook value")?.clone());
            }
            "--js" => {
                index += 1;
                args.custom_js
                    .push(values.get(index).ok_or("missing --js value")?.clone());
            }
            "--extract" => {
                index += 1;
                let value = values.get(index).ok_or("missing --extract value")?;
                let (name, selector) = value
                    .split_once('=')
                    .ok_or("--extract must be NAME=SELECTOR")?;
                args.extractors
                    .push((name.trim().to_string(), selector.trim().to_string()));
            }
            "--user-agent" => {
                index += 1;
                args.user_agent = Some(
                    values
                        .get(index)
                        .ok_or("missing --user-agent value")?
                        .clone(),
                );
            }
            "--header" => {
                index += 1;
                args.headers
                    .push(values.get(index).ok_or("missing --header value")?.clone());
            }
            "--auth" => {
                index += 1;
                let value = values.get(index).ok_or("missing --auth value")?;
                let (user, password) = value
                    .split_once(':')
                    .ok_or("--auth must be user:password")?;
                args.auth = Some((user.to_string(), password.to_string()));
            }
            "--timeout-ms" => {
                index += 1;
                args.timeout_ms = values
                    .get(index)
                    .ok_or("missing --timeout-ms value")?
                    .parse()
                    .map_err(|_| "invalid --timeout-ms".to_string())?;
            }
            "--render-workers" => {
                index += 1;
                args.render_workers = values
                    .get(index)
                    .ok_or("missing --render-workers value")?
                    .parse()
                    .map_err(|_| "invalid --render-workers".to_string())?;
            }
            other => return Err(format!("unknown option '{other}'\n{}", usage())),
        }
        index += 1;
    }
    Ok(args)
}

fn parse_resume(values: &[String], config: &FileConfig) -> Result<Args, String> {
    let mut args = Args::from_config(config);
    args.resume = true;
    let mut index = 2;
    while index < values.len() {
        match values[index].as_str() {
            "--config" => {
                index += 1;
            }
            "--out" => {
                index += 1;
                args.out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            "--max-urls" => {
                index += 1;
                args.max_urls = values
                    .get(index)
                    .ok_or("missing --max-urls value")?
                    .parse()
                    .map_err(|_| "invalid --max-urls".to_string())?;
            }
            "--concurrency" => {
                index += 1;
                args.concurrency = values
                    .get(index)
                    .ok_or("missing --concurrency value")?
                    .parse()
                    .map_err(|_| "invalid --concurrency".to_string())?;
            }
            other => return Err(format!("unknown option '{other}'\n{}", usage())),
        }
        index += 1;
    }
    Ok(args)
}

fn run_report(values: &[String]) -> Result<(), String> {
    let mut kind = "summary".to_string();
    let mut out = PathBuf::from("coqui-out");
    let mut with: Option<String> = None;
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--out" => {
                index += 1;
                out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            "--with" => {
                index += 1;
                with = Some(values.get(index).ok_or("missing --with value")?.clone());
            }
            "--top" | "--status" | "--title" => {
                index += 1;
            }
            other if !other.starts_with("--") => kind = other.to_string(),
            other => return Err(format!("unknown option '{other}'")),
        }
        index += 1;
    }
    let mut options = coqui_report::ReportOptions::default();
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--out" => {
                index += 1;
            }
            "--top" => {
                index += 1;
                options.top = values.get(index).ok_or("missing --top value")?.parse().ok();
            }
            "--status" => {
                index += 1;
                options.status = values
                    .get(index)
                    .ok_or("missing --status value")?
                    .parse()
                    .ok();
            }
            _ => {}
        }
        index += 1;
    }
    if kind == "compare" {
        let other = with.ok_or_else(|| "--with <crawl id> is required".to_string())?;
        let store = Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?;
        let crawl = store
            .latest_crawl()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "no crawl found".to_string())?;
        let path = out.join("report-compare.html");
        let urls = coqui_report::write_comparison(&store, &crawl.id, &other, &path)?;
        println!("report: {} ({} urls)", path.display(), urls);
        return Ok(());
    }
    if !coqui_report::REPORTS.iter().any(|(id, _, _)| *id == kind) {
        return Err(format!("unknown report '{kind}'"));
    }
    let store = Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?;
    let crawl = store
        .latest_crawl()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "no crawl found".to_string())?;
    let path = out.join(format!("report-{kind}.html"));
    let urls = coqui_report::write_report_with(&store, &crawl.id, &path, &kind, &options)?;
    println!("report: {} ({} urls)", path.display(), urls);
    Ok(())
}

const EXPORT_FAMILIES: &[&str] = &[
    "urls",
    "issues",
    "links",
    "headers",
    "images",
    "a11y",
    "structured",
    "vitals",
    "console",
    "text",
    "markdown",
    "extractions",
    "custom-js",
];

fn export_one(
    store: &Store,
    crawl_id: &str,
    family: &str,
    path: &std::path::Path,
) -> Result<u64, String> {
    match family {
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
    }
    .map_err(|error| error.to_string())
}

fn run_export(values: &[String]) -> Result<(), String> {
    let mut out = PathBuf::from("coqui-out");
    let mut family = "urls".to_string();
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--out" => {
                index += 1;
                out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            "--family" => {
                index += 1;
                family = values.get(index).ok_or("missing --family value")?.clone();
            }
            other => return Err(format!("unknown option '{other}'")),
        }
        index += 1;
    }
    let store = Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?;
    let crawl = store
        .latest_crawl()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "no crawl found".to_string())?;
    let stamp = now_ms();
    let started = std::time::Instant::now();
    let write = |family: &str| -> Result<(PathBuf, u64), String> {
        let path = out.join(format!("{family}-{stamp}.csv"));
        let rows = export_one(&store, &crawl.id, family, &path)?;
        Ok((path, rows))
    };
    if family == "all" {
        for name in EXPORT_FAMILIES {
            let (path, rows) = write(name)?;
            println!("export {name}: {} ({rows} rows)", path.display());
        }
        println!(
            "export all: {} families in {:.3}s",
            EXPORT_FAMILIES.len(),
            started.elapsed().as_secs_f64()
        );
        return Ok(());
    }
    let (path, rows) = write(&family)?;
    println!(
        "export {}: {} ({} rows in {:.3}s)",
        family,
        path.display(),
        rows,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn first_existing(paths: &[&str]) -> Option<String> {
    paths
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .map(|path| (*path).to_string())
}

async fn run_serve(values: &[String]) -> Result<(), String> {
    let mut out = PathBuf::from("coqui-out");
    let mut addr = "127.0.0.1:8787".to_string();
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--out" => {
                index += 1;
                out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            "--addr" => {
                index += 1;
                addr = values.get(index).ok_or("missing --addr value")?.clone();
            }
            other => return Err(format!("unknown option '{other}'")),
        }
        index += 1;
    }
    let store =
        std::sync::Arc::new(Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?);
    println!("coqui serve: http://{addr}");
    coqui_api::serve(store, &addr).await
}

fn run_issue_settings(values: &[String]) -> Result<(), String> {
    let action = values
        .first()
        .ok_or("usage: issue-settings <export|import> [--out DIR] [--file PATH]")?;
    let mut out = PathBuf::from("coqui-out");
    let mut file = None;
    let mut with = None;
    let mut index = 1;
    while index < values.len() {
        match values[index].as_str() {
            "--out" => {
                index += 1;
                out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            "--file" => {
                index += 1;
                file = Some(PathBuf::from(
                    values.get(index).ok_or("missing --file value")?,
                ));
            }
            "--with" => {
                index += 1;
                with = Some(values.get(index).ok_or("missing --with value")?.clone());
            }
            other => return Err(format!("unknown option '{other}'")),
        }
        index += 1;
    }
    let store = Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?;
    let crawl = store
        .latest_crawl()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "no crawl found".to_string())?;
    match action.as_str() {
        "export" => {
            let path = file.unwrap_or_else(|| out.join("issue-settings.json"));
            let value = store
                .issue_settings(&crawl.id)
                .map_err(|error| error.to_string())?;
            std::fs::write(
                &path,
                serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            println!("issue-settings: {}", path.display());
        }
        "import" => {
            let path = file.ok_or("import requires --file PATH")?;
            let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
            let value: serde_json::Value =
                serde_json::from_str(&text).map_err(|error| error.to_string())?;
            store
                .apply_issue_settings(&crawl.id, &value)
                .map_err(|error| error.to_string())?;
            println!("issue-settings imported from {}", path.display());
        }
        "diff" => {
            let other = with.ok_or("diff requires --with <crawl id>")?;
            let left = store
                .issue_settings(&crawl.id)
                .map_err(|error| error.to_string())?;
            let right = store
                .issue_settings(&other)
                .map_err(|error| error.to_string())?;
            let collect = |value: &serde_json::Value, key: &str, field: &str| {
                let mut map = std::collections::BTreeMap::new();
                if let Some(items) = value.get(key).and_then(|value| value.as_array()) {
                    for item in items {
                        let Some(rule) = item.get("rule").and_then(|value| value.as_str()) else {
                            continue;
                        };
                        let entry = item
                            .get(field)
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string();
                        map.insert(rule.to_string(), entry);
                    }
                }
                map
            };
            for (key, field) in [("suppressions", "reason"), ("overrides", "severity")] {
                let a = collect(&left, key, field);
                let b = collect(&right, key, field);
                let rules: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
                for rule in rules {
                    let left_value = a.get(rule).cloned().unwrap_or_else(|| "none".into());
                    let right_value = b.get(rule).cloned().unwrap_or_else(|| "none".into());
                    if left_value != right_value {
                        println!("{key} {rule}: left={left_value} right={right_value}");
                    }
                }
            }
        }
        other => return Err(format!("unknown action '{other}'")),
    }
    Ok(())
}

fn run_doctor(values: &[String]) -> Result<(), String> {
    let mut out = PathBuf::from("coqui-out");
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--out" => {
                index += 1;
                out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            other => return Err(format!("unknown option '{other}'")),
        }
        index += 1;
    }
    println!("coqui {}", env!("CARGO_PKG_VERSION"));
    Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?;
    println!("database: ok ({})", out.join("coqui.db").display());

    let sidecar_node = [
        "sidecars/node.exe",
        "src-tauri/sidecars/node.exe",
        "sidecars/node",
        "src-tauri/sidecars/node",
    ];
    let node = std::env::var_os("COQUI_NODE")
        .map(|value| value.to_string_lossy().to_string())
        .filter(|path| std::path::Path::new(path).exists())
        .or_else(|| first_existing(&sidecar_node))
        .or_else(|| {
            std::process::Command::new("node")
                .arg("--version")
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        });
    match node {
        Some(value) => println!("node: ok ({value})"),
        None => println!("node: missing (lighthouse runs need Node)"),
    }

    let lighthouse = std::env::var_os("COQUI_LIGHTHOUSE_CLI")
        .map(|value| value.to_string_lossy().to_string())
        .filter(|path| std::path::Path::new(path).exists())
        .or_else(|| {
            first_existing(&[
                "sidecars/lighthouse/node_modules/lighthouse/cli/index.js",
                "src-tauri/sidecars/lighthouse/node_modules/lighthouse/cli/index.js",
            ])
        });
    match lighthouse {
        Some(value) => println!("lighthouse: ok ({value})"),
        None => println!("lighthouse: missing (run `npm run sidecars`)"),
    }

    let browser = std::env::var_os("CHROME_PATH")
        .map(|value| value.to_string_lossy().to_string())
        .filter(|path| std::path::Path::new(path).exists())
        .or_else(|| {
            first_existing(&[
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
                "/usr/bin/google-chrome",
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
            ])
        });
    match browser {
        Some(value) => println!("browser: ok ({value})"),
        None => println!("browser: missing (render and lighthouse need Chrome/Edge)"),
    }
    Ok(())
}

fn run_extractions(values: &[String]) -> Result<(), String> {
    let mut out = PathBuf::from("coqui-out");
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--out" => {
                index += 1;
                out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            other => return Err(format!("unknown option '{other}'")),
        }
        index += 1;
    }
    let store = Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?;
    let crawl = store
        .latest_crawl()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "no crawl found".to_string())?;
    let path = out.join(format!("extractions-{}.csv", now_ms()));
    let rows = store
        .export_extractions_csv(&crawl.id, &path)
        .map_err(|error| error.to_string())?;
    println!("extractions: {} ({} rows)", path.display(), rows);
    Ok(())
}

fn run_sitemap(values: &[String]) -> Result<(), String> {
    let mut out = PathBuf::from("coqui-out");
    let mut split = false;
    let mut gzip = false;
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--out" => {
                index += 1;
                out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            "--split" => split = true,
            "--gzip" => gzip = true,
            other => return Err(format!("unknown option '{other}'")),
        }
        index += 1;
    }
    let store = Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?;
    let crawl = store
        .latest_crawl()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "no crawl found".to_string())?;
    if split {
        let directory = out.join("sitemaps");
        let rows = store
            .export_sitemap_bundle(&crawl.id, &directory, gzip)
            .map_err(|error| error.to_string())?;
        println!(
            "sitemap bundle: {} ({} urls, gzip={gzip})",
            directory.display(),
            rows
        );
    } else {
        let path = out.join("sitemap.xml");
        let rows = store
            .export_sitemap(&crawl.id, &path)
            .map_err(|error| error.to_string())?;
        println!("sitemap: {} ({} urls)", path.display(), rows);
    }
    Ok(())
}

async fn run_schedule(values: &[String]) -> Result<(), String> {
    let command = values
        .first()
        .map(String::as_str)
        .ok_or("usage: schedule <add|list|remove|run-due>")?;
    let mut out = PathBuf::from("coqui-out");
    let mut every = 3600i64;
    let mut max_urls = None;
    let mut webhook = None;
    let mut remove_id = None;
    let mut kind = String::new();
    let mut seed = String::new();
    let mut index = 1;
    while index < values.len() {
        match values[index].as_str() {
            "--out" => {
                index += 1;
                out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            "--every" => {
                index += 1;
                every = values
                    .get(index)
                    .ok_or("missing --every value")?
                    .parse()
                    .map_err(|_| "invalid --every".to_string())?;
            }
            "--max-urls" => {
                index += 1;
                max_urls = Some(
                    values
                        .get(index)
                        .ok_or("missing --max-urls value")?
                        .parse::<u32>()
                        .map_err(|_| "invalid --max-urls".to_string())?,
                );
            }
            "--webhook" => {
                index += 1;
                webhook = Some(values.get(index).ok_or("missing --webhook value")?.clone());
            }
            other => match command {
                "add" if kind.is_empty() => kind = other.to_string(),
                "add" if seed.is_empty() => seed = other.to_string(),
                "remove" => {
                    remove_id = Some(other.parse::<i64>().map_err(|_| "invalid id".to_string())?)
                }
                _ => return Err(format!("unknown option '{other}'")),
            },
        }
        index += 1;
    }
    let store = Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?;
    match command {
        "add" => {
            if kind.is_empty() || seed.is_empty() {
                return Err("usage: schedule add <kind> <seed> --every SECONDS".into());
            }
            let mut config = CrawlConfig::default();
            if let Some(max_urls) = max_urls {
                config.max_urls = max_urls;
            }
            if let Some(webhook) = webhook.clone() {
                config.webhook_url = Some(webhook);
            }
            let json = serde_json::to_string(&config).map_err(|error| error.to_string())?;
            let id = store
                .add_schedule(&kind, &seed, every, &json, now_ms() + every * 1000)
                .map_err(|error| error.to_string())?;
            println!("schedule {id}: {kind} {seed} every {every}s");
        }
        "list" => {
            for schedule in store.schedules().map_err(|error| error.to_string())? {
                println!(
                    "{}\t{}\t{}\tevery {}s\tnext {}\tlast {}",
                    schedule.id,
                    schedule.seed_kind,
                    schedule.seed,
                    schedule.interval_seconds,
                    schedule.next_run_at,
                    schedule
                        .last_run_at
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".into())
                );
            }
        }
        "remove" => {
            let id = remove_id.ok_or("usage: schedule remove <id>")?;
            let removed = store
                .remove_schedule(id)
                .map_err(|error| error.to_string())?;
            println!("removed {removed} schedule(s)");
        }
        "run-due" => {
            let due = store
                .due_schedules(now_ms())
                .map_err(|error| error.to_string())?;
            if due.is_empty() {
                println!("no schedules due");
                return Ok(());
            }
            for schedule in due {
                let config: CrawlConfig = serde_json::from_str(&schedule.config_json)
                    .map_err(|error| format!("schedule {}: {error}", schedule.id))?;
                let crawl_id = format!("sched-{}-{}", schedule.id, now_ms());
                let runner = std::sync::Arc::new(
                    Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?,
                );
                runner
                    .create_crawl(
                        &crawl_id,
                        &schedule.seed,
                        &schedule.seed_kind,
                        &config,
                        now_ms(),
                    )
                    .map_err(|error| error.to_string())?;
                let result = coqui_crawl::run_crawl(
                    runner.clone(),
                    crawl_id.clone(),
                    schedule.seed_kind.clone(),
                    schedule.seed.clone(),
                    config,
                    CancellationToken::new(),
                    Arc::new(AtomicBool::new(false)),
                    None,
                    Arc::new(|_| {}),
                )
                .await;
                match result {
                    Ok(_) => {
                        let path = out.join(format!("{crawl_id}.csv"));
                        let store =
                            Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?;
                        let rows = store
                            .export_urls_csv(&crawl_id, &path, &[], None)
                            .map_err(|error| error.to_string())?;
                        let now = now_ms();
                        store
                            .mark_schedule_run(
                                schedule.id,
                                now,
                                now + schedule.interval_seconds * 1000,
                            )
                            .map_err(|error| error.to_string())?;
                        println!(
                            "schedule {}: {} rows -> {}",
                            schedule.id,
                            rows,
                            path.display()
                        );
                    }
                    Err(error) => eprintln!("schedule {} failed: {error}", schedule.id),
                }
            }
        }
        other => return Err(format!("unknown schedule command '{other}'")),
    }
    Ok(())
}

fn rpc_result(id: serde_json::Value, result: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn rpc_error(id: serde_json::Value, code: i64, message: &str) -> serde_json::Value {
    serde_json::json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn tool_definitions() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "list_crawls",
            "description": "List recent crawls in the Coqui crawl database.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "get_crawl",
            "description": "Get a crawl summary by id.",
            "inputSchema": {"type": "object", "properties": {"crawl_id": {"type": "string"}}, "required": ["crawl_id"]}
        },
        {
            "name": "list_urls",
            "description": "List URLs for a crawl.",
            "inputSchema": {"type": "object", "properties": {"crawl_id": {"type": "string"}, "limit": {"type": "integer"}, "page": {"type": "integer"}}, "required": ["crawl_id"]}
        },
        {
            "name": "issue_counts",
            "description": "Issue counts for a crawl.",
            "inputSchema": {"type": "object", "properties": {"crawl_id": {"type": "string"}}, "required": ["crawl_id"]}
        },
        {
            "name": "export_urls",
            "description": "Export a crawl's URLs to CSV and return the path.",
            "inputSchema": {"type": "object", "properties": {"crawl_id": {"type": "string"}}, "required": ["crawl_id"]}
        },
        {
            "name": "export_family",
            "description": "Export a crawl table family to CSV and return the path.",
            "inputSchema": {"type": "object", "properties": {"crawl_id": {"type": "string"}, "family": {"type": "string"}, "path": {"type": "string"}}, "required": ["crawl_id", "family"]}
        },
        {
            "name": "compare_crawls",
            "description": "Compare issue counts between two crawls.",
            "inputSchema": {"type": "object", "properties": {"left": {"type": "string"}, "right": {"type": "string"}}, "required": ["left", "right"]}
        },
        {
            "name": "start_crawl",
            "description": "Start a crawl in the background and return its id.",
            "inputSchema": {"type": "object", "properties": {"seed": {"type": "string"}, "seedKind": {"type": "string"}, "maxUrls": {"type": "integer"}}, "required": ["seed"]}
        },
        {
            "name": "run_schedule_due",
            "description": "Run every due schedule in the background.",
            "inputSchema": {"type": "object", "properties": {}}
        }
    ])
}

fn call_tool(
    store: &std::sync::Arc<Store>,
    out: &Path,
    message: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let params = message
        .get("params")
        .ok_or_else(|| "missing params".to_string())?;
    let name = params
        .get("name")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "missing tool name".to_string())?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let crawl_id = || {
        arguments
            .get("crawl_id")
            .and_then(|value| value.as_str())
            .ok_or_else(|| "crawl_id is required".to_string())
    };
    match name {
        "list_crawls" => {
            let crawls = store.list_crawls().map_err(|error| error.to_string())?;
            let values: Vec<_> = crawls
                .iter()
                .map(|crawl| {
                    serde_json::json!({
                        "id": crawl.id,
                        "seed": crawl.seed,
                        "seedKind": crawl.seed_kind,
                        "status": format!("{:?}", crawl.status),
                        "totalUrls": crawl.total_urls,
                        "blockedUrls": crawl.blocked_urls,
                    })
                })
                .collect();
            Ok(serde_json::Value::Array(values))
        }
        "get_crawl" => {
            let crawl = store
                .get_crawl(crawl_id()?)
                .map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "id": crawl.id,
                "seed": crawl.seed,
                "seedKind": crawl.seed_kind,
                "status": format!("{:?}", crawl.status),
                "totalUrls": crawl.total_urls,
                "blockedUrls": crawl.blocked_urls,
            }))
        }
        "list_urls" => {
            let page = arguments
                .get("page")
                .and_then(|value| value.as_u64())
                .unwrap_or(1) as u32;
            let limit = arguments
                .get("limit")
                .and_then(|value| value.as_u64())
                .unwrap_or(20) as u32;
            let status = arguments
                .get("status")
                .and_then(|value| value.as_u64())
                .map(|value| value as u16);
            let status_class = arguments
                .get("statusClass")
                .and_then(|value| value.as_u64())
                .map(|value| value as u16);
            let content_class = arguments
                .get("contentClass")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let max_depth = arguments
                .get("maxDepth")
                .and_then(|value| value.as_u64())
                .map(|value| value as u32);
            let indexability = arguments
                .get("indexability")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let sort = arguments
                .get("sort")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let descending = arguments
                .get("descending")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let query = arguments
                .get("query")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let result = store
                .list_urls_filtered(
                    crawl_id()?,
                    page,
                    limit,
                    None,
                    query.as_deref(),
                    status,
                    status_class,
                    content_class.as_deref(),
                    max_depth,
                    indexability.as_deref(),
                    sort.as_deref(),
                    descending,
                )
                .map_err(|error| error.to_string())?;
            let rows: Vec<_> = result
                .rows
                .iter()
                .map(|url| {
                    serde_json::json!({
                        "url": url.url,
                        "status": url.status,
                        "title": url.title,
                        "h1": url.h1,
                        "wordCount": url.word_count,
                    })
                })
                .collect();
            Ok(serde_json::json!({"total": result.total, "rows": rows}))
        }
        "issue_counts" => {
            let counts = store
                .issue_counts(crawl_id()?)
                .map_err(|error| error.to_string())?;
            let values: Vec<_> = counts
                .iter()
                .map(|count| serde_json::json!({"rule": count.rule, "count": count.count}))
                .collect();
            Ok(serde_json::Value::Array(values))
        }
        "export_urls" => {
            let crawl_id_value = crawl_id()?;
            let path = out.join(format!("export-{crawl_id_value}.csv"));
            let rows = store
                .export_urls_csv(crawl_id_value, &path, &[], None)
                .map_err(|error| error.to_string())?;
            Ok(serde_json::json!({"path": path.to_string_lossy(), "rows": rows}))
        }
        "export_family" => {
            let crawl_id_value = crawl_id()?;
            let family = arguments
                .get("family")
                .and_then(|value| value.as_str())
                .ok_or_else(|| "family is required".to_string())?;
            let path = arguments
                .get("path")
                .and_then(|value| value.as_str())
                .map(PathBuf::from)
                .unwrap_or_else(|| out.join(format!("{family}-{}.csv", now_ms())));
            if family == "all" {
                let mut results = Vec::new();
                for name in EXPORT_FAMILIES {
                    let path = out.join(format!("{name}-{}.csv", now_ms()));
                    let rows = export_one(store, crawl_id_value, name, &path)?;
                    results.push(serde_json::json!({
                        "family": name,
                        "path": path.to_string_lossy(),
                        "rows": rows,
                    }));
                }
                return Ok(serde_json::json!({"families": results}));
            }
            let rows = export_one(store, crawl_id_value, family, &path)?;
            Ok(serde_json::json!({
                "path": path.to_string_lossy(),
                "rows": rows,
                "family": family,
            }))
        }
        "compare_crawls" => {
            let left = arguments
                .get("left")
                .and_then(|value| value.as_str())
                .ok_or_else(|| "left is required".to_string())?;
            let right = arguments
                .get("right")
                .and_then(|value| value.as_str())
                .ok_or_else(|| "right is required".to_string())?;
            let mut rules: std::collections::BTreeMap<String, (i64, i64)> =
                std::collections::BTreeMap::new();
            for issue in store
                .issue_counts(left)
                .map_err(|error| error.to_string())?
            {
                rules.entry(issue.rule).or_default().0 += issue.count;
            }
            for issue in store
                .issue_counts(right)
                .map_err(|error| error.to_string())?
            {
                rules.entry(issue.rule).or_default().1 += issue.count;
            }
            Ok(serde_json::Value::Array(
                rules
                    .into_iter()
                    .map(|(rule, (left_count, right_count))| {
                        serde_json::json!({
                            "rule": rule,
                            "left": left_count,
                            "right": right_count,
                            "delta": right_count - left_count,
                        })
                    })
                    .collect(),
            ))
        }
        "start_crawl" => {
            let seed = arguments
                .get("seed")
                .and_then(|value| value.as_str())
                .ok_or_else(|| "seed is required".to_string())?
                .to_string();
            let seed_kind = arguments
                .get("seedKind")
                .and_then(|value| value.as_str())
                .unwrap_or("url")
                .to_string();
            if !matches!(seed_kind.as_str(), "url" | "sitemap" | "list") {
                return Err(format!("unknown seed kind '{seed_kind}'"));
            }
            let mut config = CrawlConfig::default();
            if let Some(max_urls) = arguments.get("maxUrls").and_then(|value| value.as_u64()) {
                config.max_urls = max_urls as u32;
            }
            let crawl_id = format!("mcp-{}", now_ms());
            store
                .create_crawl(&crawl_id, &seed, &seed_kind, &config, now_ms())
                .map_err(|error| error.to_string())?;
            let task_store = std::sync::Arc::clone(store);
            let task_id = crawl_id.clone();
            tokio::spawn(async move {
                let result = coqui_crawl::run_crawl(
                    task_store.clone(),
                    task_id.clone(),
                    seed_kind,
                    seed,
                    config,
                    CancellationToken::new(),
                    Arc::new(AtomicBool::new(false)),
                    None,
                    Arc::new(|_| {}),
                )
                .await;
                if result.is_ok() {
                    let _ = coqui_audit::analyze_crawl(&task_store, &task_id);
                }
            });
            Ok(serde_json::json!({"crawlId": crawl_id, "status": "started"}))
        }
        "run_schedule_due" => {
            let due = store
                .due_schedules(now_ms())
                .map_err(|error| error.to_string())?;
            let ids: Vec<i64> = due.iter().map(|schedule| schedule.id).collect();
            let task_store = std::sync::Arc::clone(store);
            tokio::spawn(async move {
                for schedule in due {
                    let Ok(config) = serde_json::from_str::<CrawlConfig>(&schedule.config_json)
                    else {
                        continue;
                    };
                    let crawl_id = format!("mcp-sched-{}-{}", schedule.id, now_ms());
                    if task_store
                        .create_crawl(
                            &crawl_id,
                            &schedule.seed,
                            &schedule.seed_kind,
                            &config,
                            now_ms(),
                        )
                        .is_err()
                    {
                        continue;
                    }
                    let result = coqui_crawl::run_crawl(
                        task_store.clone(),
                        crawl_id.clone(),
                        schedule.seed_kind.clone(),
                        schedule.seed.clone(),
                        config,
                        CancellationToken::new(),
                        Arc::new(AtomicBool::new(false)),
                        None,
                        Arc::new(|_| {}),
                    )
                    .await;
                    if result.is_ok() {
                        let _ = coqui_audit::analyze_crawl(&task_store, &crawl_id);
                        let now = now_ms();
                        let _ = task_store.mark_schedule_run(
                            schedule.id,
                            now,
                            now + schedule.interval_seconds * 1000,
                        );
                    }
                }
            });
            Ok(serde_json::json!({"schedules": ids, "status": "started"}))
        }
        other => Err(format!("unknown tool '{other}'")),
    }
}

fn resource_definitions() -> serde_json::Value {
    serde_json::json!({
        "resources": [{
            "uri": "coqui://crawls",
            "name": "Crawls",
            "description": "Recent crawls in the database.",
            "mimeType": "application/json",
        }]
    })
}

fn resource_templates() -> serde_json::Value {
    serde_json::json!({
        "resourceTemplates": [
            {
                "uriTemplate": "coqui://crawl/{id}",
                "name": "Crawl summary",
                "description": "Summary for one crawl.",
                "mimeType": "application/json",
            },
            {
                "uriTemplate": "coqui://crawl/{id}/issues",
                "name": "Crawl issues",
                "description": "Issue counts for one crawl.",
                "mimeType": "application/json",
            }
        ]
    })
}

fn read_resource(store: &Store, uri: &str) -> Result<String, String> {
    if uri == "coqui://crawls" {
        let crawls = store.list_crawls().map_err(|error| error.to_string())?;
        let values: Vec<_> = crawls
            .iter()
            .map(|crawl| {
                serde_json::json!({
                    "id": crawl.id,
                    "seed": crawl.seed,
                    "seedKind": crawl.seed_kind,
                    "status": format!("{:?}", crawl.status),
                    "totalUrls": crawl.total_urls,
                })
            })
            .collect();
        return Ok(serde_json::Value::Array(values).to_string());
    }
    let rest = uri
        .strip_prefix("coqui://crawl/")
        .ok_or_else(|| format!("unknown resource '{uri}'"))?;
    let (id, suffix) = match rest.split_once('/') {
        Some((id, suffix)) => (id, Some(suffix)),
        None => (rest, None),
    };
    if suffix == Some("issues") {
        let counts = store.issue_counts(id).map_err(|error| error.to_string())?;
        let values: Vec<_> = counts
            .iter()
            .map(|count| serde_json::json!({"rule": count.rule, "count": count.count}))
            .collect();
        return Ok(serde_json::Value::Array(values).to_string());
    }
    let crawl = store.get_crawl(id).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "id": crawl.id,
        "seed": crawl.seed,
        "seedKind": crawl.seed_kind,
        "status": format!("{:?}", crawl.status),
        "totalUrls": crawl.total_urls,
        "blockedUrls": crawl.blocked_urls,
    })
    .to_string())
}

fn prompt_definitions() -> serde_json::Value {
    serde_json::json!({
        "prompts": [
            {
                "name": "audit-crawl",
                "description": "Review a crawl and prioritise fixes.",
                "arguments": [{"name": "crawl_id", "description": "Crawl id", "required": true}]
            },
            {
                "name": "summarize-issues",
                "description": "Summarise a crawl's issues by rule.",
                "arguments": [{"name": "crawl_id", "description": "Crawl id", "required": true}]
            },
            {
                "name": "export-crawl",
                "description": "Export a crawl's URLs to CSV.",
                "arguments": [{"name": "crawl_id", "description": "Crawl id", "required": true}]
            }
        ]
    })
}

fn get_prompt(message: &serde_json::Value) -> Result<serde_json::Value, String> {
    let params = message
        .get("params")
        .ok_or_else(|| "missing params".to_string())?;
    let name = params
        .get("name")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "missing prompt name".to_string())?;
    let crawl_id = params
        .get("arguments")
        .and_then(|arguments| arguments.get("crawl_id"))
        .and_then(|value| value.as_str())
        .unwrap_or("<crawl-id>");
    let text = match name {
        "audit-crawl" => format!(
            "Audit Coqui crawl {crawl_id}. Call get_crawl, issue_counts and list_urls, then summarise the most important issues with concrete fixes."
        ),
        "summarize-issues" => format!(
            "Summarise the issues for Coqui crawl {crawl_id} using issue_counts. Prioritise by affected URLs and explain likely causes."
        ),
        "export-crawl" => format!(
            "Export Coqui crawl {crawl_id} with export_urls and report the CSV path and row count."
        ),
        other => return Err(format!("unknown prompt '{other}'")),
    };
    Ok(serde_json::json!({
        "description": format!("Coqui prompt {name}"),
        "messages": [{"role": "user", "content": {"type": "text", "text": text}}]
    }))
}

fn run_mcp(values: &[String]) -> Result<(), String> {
    let mut out = PathBuf::from("coqui-out");
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--out" => {
                index += 1;
                out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            other => return Err(format!("unknown option '{other}'")),
        }
        index += 1;
    }
    let store =
        std::sync::Arc::new(Store::open(out.join("coqui.db")).map_err(|error| error.to_string())?);
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let id = message
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let method = message
            .get("method")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if method == "notifications/initialized" {
            continue;
        }
        let response = match method {
            "initialize" => rpc_result(
                id,
                serde_json::json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {"tools": {}, "resources": {}, "prompts": {}},
                    "serverInfo": {"name": "coqui", "version": env!("CARGO_PKG_VERSION")},
                }),
            ),
            "ping" => rpc_result(id, serde_json::json!({})),
            "tools/list" => rpc_result(id, serde_json::json!({"tools": tool_definitions()})),
            "prompts/list" => rpc_result(id, prompt_definitions()),
            "prompts/get" => match get_prompt(&message) {
                Ok(value) => rpc_result(id, value),
                Err(error) => rpc_error(id, -32602, &error),
            },
            "resources/list" => rpc_result(id, resource_definitions()),
            "resources/templates/list" => rpc_result(id, resource_templates()),
            "resources/read" => {
                let uri = message
                    .get("params")
                    .and_then(|params| params.get("uri"))
                    .and_then(|uri| uri.as_str())
                    .unwrap_or_default();
                match read_resource(&store, uri) {
                    Ok(text) => rpc_result(
                        id,
                        serde_json::json!({
                            "contents": [{"uri": uri, "mimeType": "application/json", "text": text}],
                        }),
                    ),
                    Err(error) => rpc_error(id, -32602, &error),
                }
            }
            "tools/call" => match call_tool(&store, &out, &message) {
                Ok(value) => rpc_result(
                    id,
                    serde_json::json!({
                        "content": [{"type": "text", "text": value.to_string()}],
                    }),
                ),
                Err(error) => rpc_result(
                    id,
                    serde_json::json!({
                        "content": [{"type": "text", "text": error}],
                        "isError": true,
                    }),
                ),
            },
            other => rpc_error(id, -32601, &format!("method not found: {other}")),
        };
        writeln!(stdout, "{response}").map_err(|error| error.to_string())?;
        stdout.flush().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let values: Vec<String> = std::env::args().collect();
    if values.len() == 2 && matches!(values[1].as_str(), "--version" | "-V") {
        println!("coqui {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if values.get(1).map(String::as_str) == Some("exports") {
        for entry in coqui_core::export_catalog() {
            println!(
                "{}\\t{}\\t{}\\t{}",
                entry.id, entry.format, entry.label, entry.description
            );
        }
        return;
    }
    if values.get(1).map(String::as_str) == Some("mcp") {
        match run_mcp(&values[2..]) {
            Ok(()) => return,
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(2);
            }
        }
    }
    if values.get(1).map(String::as_str) == Some("serve") {
        match run_serve(&values[2..]).await {
            Ok(()) => return,
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(2);
            }
        }
    }
    if values.get(1).map(String::as_str) == Some("issue-settings") {
        match run_issue_settings(&values[2..]) {
            Ok(()) => return,
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(2);
            }
        }
    }
    if values.get(1).map(String::as_str) == Some("doctor") {
        match run_doctor(&values[2..]) {
            Ok(()) => return,
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(2);
            }
        }
    }
    if values.get(1).map(String::as_str) == Some("schedule") {
        match run_schedule(&values[2..]).await {
            Ok(()) => return,
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(2);
            }
        }
    }
    if values.get(1).map(String::as_str) == Some("sitemap") {
        match run_sitemap(&values[2..]) {
            Ok(()) => return,
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(2);
            }
        }
    }
    if values.get(1).map(String::as_str) == Some("extractions") {
        match run_extractions(&values[2..]) {
            Ok(()) => return,
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(2);
            }
        }
    }
    if values.get(1).map(String::as_str) == Some("export") {
        match run_export(&values[2..]) {
            Ok(()) => return,
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(2);
            }
        }
    }
    if values.get(1).map(String::as_str) == Some("reports") {
        for (id, label, description) in coqui_report::REPORTS {
            println!("{id}\t{label}\t{description}");
        }
        return;
    }
    if values.get(1).map(String::as_str) == Some("report") {
        match run_report(&values[2..]) {
            Ok(()) => return,
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(2);
            }
        }
    }
    let explicit_config = values.windows(2).any(|pair| pair[0] == "--config");
    let config_path = values
        .windows(2)
        .find(|pair| pair[0] == "--config")
        .map(|pair| pair[1].clone())
        .unwrap_or_else(|| "coqui.toml".to_string());
    let config = match load_config(Path::new(&config_path), explicit_config) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let parsed = if values.get(1).map(String::as_str) == Some("resume") {
        parse_resume(&values, &config)
    } else {
        parse_args(&values, &config)
    };
    let args = match parsed {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    std::fs::create_dir_all(&args.out).expect("create output directory");
    let db = args.out.join("coqui.db");
    let store = Arc::new(Store::open(&db).expect("open crawl store"));
    let (crawl_id, kind, seed, config, checkpoint) = if args.resume {
        let latest = store
            .latest_crawl()
            .expect("read crawls")
            .expect("no crawl to resume");
        let mut config = latest.config.clone();
        if args.max_urls > 0 {
            config.max_urls = args.max_urls;
        }
        if args.concurrency > 0 {
            config.concurrency = args.concurrency;
        }
        let checkpoint = store.load_checkpoint(&latest.id).expect("load checkpoint");
        println!("coqui: resuming {} in {}", latest.id, db.display());
        (latest.id, latest.seed_kind, latest.seed, config, checkpoint)
    } else {
        let crawl_id = format!("cli-{}", now_ms());
        let mut config = CrawlConfig {
            max_depth: args.depth,
            max_urls: args.max_urls,
            concurrency: args.concurrency,
            respect_robots: args.respect_robots,
            request_timeout_ms: args.timeout_ms,
            custom_headers: args.headers.clone(),
            render: args.render,
            screenshots: args.screenshots,
            accessibility: args.accessibility,
            web_vitals: args.web_vitals,
            store_text: args.store_text,
            webhook_url: args.webhook_url.clone(),
            custom_js: args.custom_js.clone(),
            render_workers: args.render_workers,
            ..CrawlConfig::default()
        };
        if let Some(user_agent) = &args.user_agent {
            config.user_agent = user_agent.clone();
        }
        if let Some((user, password)) = &args.auth {
            config.auth_user = Some(user.clone());
            config.auth_password = Some(password.clone());
        }
        store
            .create_crawl(&crawl_id, &args.seed, &args.kind, &config, now_ms())
            .expect("create crawl");
        for (name, selector) in &args.extractors {
            store
                .add_extractor(&crawl_id, name, selector)
                .expect("save extractor");
        }
        println!(
            "coqui: crawling {} ({}) into {}",
            args.seed,
            args.kind,
            db.display()
        );
        (crawl_id, args.kind.clone(), args.seed.clone(), config, None)
    };
    let started = std::time::Instant::now();
    let result = coqui_crawl::run_crawl(
        store.clone(),
        crawl_id.clone(),
        kind.clone(),
        seed.clone(),
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        checkpoint,
        Arc::new(|progress: CrawlProgress| {
            if progress.crawled.is_multiple_of(250) {
                println!(
                    "  crawled {} / discovered {}",
                    progress.crawled, progress.discovered
                );
            }
        }),
    )
    .await;

    match result {
        Ok(result) => {
            let counts = coqui_audit::analyze_crawl(&store, &crawl_id).expect("analyse crawl");
            let csv = args.out.join("urls.csv");
            store.export_csv(&crawl_id, &csv).expect("export csv");
            println!(
                "done: {} crawled, {} discovered, {} issues in {:.1}s",
                result.stats.crawled,
                result.stats.discovered,
                counts.iter().map(|count| count.count).sum::<i64>(),
                started.elapsed().as_secs_f64()
            );
            println!("csv: {}", csv.display());
        }
        Err(error) => {
            eprintln!("crawl failed: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        std::iter::once("coqui")
            .chain(values.iter().copied())
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn parses_defaults() {
        let parsed = parse_args(
            &args(&["crawl", "url", "https://example.com"]),
            &FileConfig::default(),
        )
        .unwrap();
        assert_eq!(parsed.max_urls, 5000);
        assert_eq!(parsed.depth, 3);
        assert!(parsed.respect_robots);
    }

    #[test]
    fn config_supplies_defaults_and_flags_win() {
        let config = FileConfig {
            max_urls: Some(10),
            concurrency: Some(2),
            ..FileConfig::default()
        };
        let parsed = parse_args(&args(&["crawl", "url", "https://example.com"]), &config).unwrap();
        assert_eq!(parsed.max_urls, 10);
        assert_eq!(parsed.concurrency, 2);
        let overridden = parse_args(
            &args(&["crawl", "url", "https://example.com", "--max-urls", "50"]),
            &config,
        )
        .unwrap();
        assert_eq!(overridden.max_urls, 50);
    }

    #[test]
    fn parses_flags() {
        let parsed = parse_args(
            &args(&[
                "crawl",
                "sitemap",
                "https://example.com/sitemap.xml",
                "--max-urls",
                "50",
                "--depth",
                "1",
                "--concurrency",
                "4",
                "--ignore-robots",
            ]),
            &FileConfig::default(),
        )
        .unwrap();
        assert_eq!(parsed.max_urls, 50);
        assert_eq!(parsed.depth, 1);
        assert_eq!(parsed.concurrency, 4);
        assert!(!parsed.respect_robots);
    }

    #[test]
    fn parses_auth_headers_and_rendering() {
        let parsed = parse_args(
            &args(&[
                "crawl",
                "url",
                "https://example.com",
                "--user-agent",
                "Coqui/1.0",
                "--header",
                "X-Test: yes",
                "--auth",
                "user:pass",
                "--timeout-ms",
                "5000",
                "--render",
                "--screenshots",
                "--a11y",
                "--vitals",
                "--render-workers",
                "4",
            ]),
            &FileConfig::default(),
        )
        .unwrap();
        assert_eq!(parsed.user_agent.as_deref(), Some("Coqui/1.0"));
        assert_eq!(parsed.headers, vec!["X-Test: yes"]);
        assert_eq!(parsed.auth, Some(("user".into(), "pass".into())));
        assert_eq!(parsed.timeout_ms, 5000);
        assert!(parsed.render && parsed.screenshots);
        assert!(parsed.accessibility && parsed.web_vitals);
        assert_eq!(parsed.render_workers, 4);
    }

    #[test]
    fn rejects_unknown_kind() {
        assert!(parse_args(&args(&["crawl", "ftp", "x"]), &FileConfig::default()).is_err());
    }

    #[test]
    fn parses_resume() {
        let parsed = parse_resume(
            &args(&["resume", "--out", "crawls/example", "--max-urls", "25"]),
            &FileConfig::default(),
        )
        .unwrap();
        assert!(parsed.resume);
        assert_eq!(parsed.out, PathBuf::from("crawls/example"));
        assert_eq!(parsed.max_urls, 25);
    }
}
