//! Headless Coqui CLI. First cut: `coqui crawl`.

use coqui_core::{CrawlConfig, CrawlProgress, now_ms};
use coqui_store::Store;
use serde::Deserialize;
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
            extractors: Vec::new(),
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
    extractors: Vec<(String, String)>,
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
    "coqui crawl <url|sitemap|list> <seed> [--out DIR] [--max-urls N] [--depth N] [--concurrency N] [--ignore-robots]\n       coqui resume [--out DIR] [--max-urls N] [--concurrency N]\n       coqui report [summary|issues|status|performance] [--out DIR]\n       coqui reports\n       coqui export [--out DIR]\n       coqui extractions [--out DIR]\n       coqui sitemap [--out DIR] [--split] [--gzip]\n       coqui schedule add <kind> <seed> --every SECONDS [--out DIR] [--max-urls N]\n       coqui schedule list|remove <id>|run-due [--out DIR]"
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
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--out" => {
                index += 1;
                out = PathBuf::from(values.get(index).ok_or("missing --out value")?);
            }
            "--top" | "--status" => {
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

fn run_export(values: &[String]) -> Result<(), String> {
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
    let path = out.join(format!("urls-{}.csv", now_ms()));
    let started = std::time::Instant::now();
    let rows = store
        .export_urls_csv(&crawl.id, &path, &[], None)
        .map_err(|error| error.to_string())?;
    println!(
        "export: {} ({} rows in {:.3}s)",
        path.display(),
        rows,
        started.elapsed().as_secs_f64()
    );
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
            if progress.crawled % 250 == 0 {
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
