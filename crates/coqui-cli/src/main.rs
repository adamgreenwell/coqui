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
    let config = CrawlConfig {
        max_depth: args.depth,
        max_urls: args.max_urls,
        concurrency: args.concurrency,
        respect_robots: args.respect_robots,
        ..CrawlConfig::default()
    };
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
    fn rejects_unknown_kind() {
        assert!(parse_args(&args(&["crawl", "ftp", "x"])).is_err());
    }
}
