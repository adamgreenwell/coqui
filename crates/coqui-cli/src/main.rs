//! Headless Coqui CLI. First cut: `coqui crawl`.

use coqui_core::{CrawlConfig, CrawlProgress, now_ms};
use coqui_store::Store;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio_util::sync::CancellationToken;

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
}

fn usage() -> &'static str {
    "coqui crawl <url|sitemap|list> <seed> [--out DIR] [--max-urls N] [--depth N] [--concurrency N] [--ignore-robots]"
}

fn parse_args(values: &[String]) -> Result<Args, String> {
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
    let mut args = Args {
        kind,
        seed,
        out: PathBuf::from("coqui-out"),
        max_urls: 5000,
        depth: 3,
        concurrency: 8,
        respect_robots: true,
        user_agent: None,
        headers: Vec::new(),
        auth: None,
        timeout_ms: 20_000,
        render: false,
        screenshots: false,
        accessibility: false,
        web_vitals: false,
        render_workers: 2,
    };
    let mut index = 4;
    while index < values.len() {
        match values[index].as_str() {
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

#[tokio::main]
async fn main() {
    let values: Vec<String> = std::env::args().collect();
    if values.len() == 2 && matches!(values[1].as_str(), "--version" | "-V") {
        println!("coqui {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let args = match parse_args(&values) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    std::fs::create_dir_all(&args.out).expect("create output directory");
    let db = args.out.join("coqui.db");
    let store = Arc::new(Store::open(&db).expect("open crawl store"));
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

    println!(
        "coqui: crawling {} ({}) into {}",
        args.seed,
        args.kind,
        db.display()
    );
    let started = std::time::Instant::now();
    let result = coqui_crawl::run_crawl(
        store.clone(),
        crawl_id.clone(),
        args.kind.clone(),
        args.seed.clone(),
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
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
        let parsed = parse_args(&args(&["crawl", "url", "https://example.com"])).unwrap();
        assert_eq!(parsed.max_urls, 5000);
        assert_eq!(parsed.depth, 3);
        assert!(parsed.respect_robots);
    }

    #[test]
    fn parses_flags() {
        let parsed = parse_args(&args(&[
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
        ]))
        .unwrap();
        assert_eq!(parsed.max_urls, 50);
        assert_eq!(parsed.depth, 1);
        assert_eq!(parsed.concurrency, 4);
        assert!(!parsed.respect_robots);
    }

    #[test]
    fn parses_auth_headers_and_rendering() {
        let parsed = parse_args(&args(&[
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
        ]))
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
        assert!(parse_args(&args(&["crawl", "ftp", "x"])).is_err());
    }
}
