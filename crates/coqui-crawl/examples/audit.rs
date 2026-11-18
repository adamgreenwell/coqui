//! Manual crawl harness used for validating Coqui against real sites.
//!
//! Usage:
//!   cargo run -p coqui-crawl --example audit -- <kind> <seed> <output_dir> [max_urls] [concurrency]
//!
//! kind: url | sitemap | list

use coqui_audit::analyze_crawl;
use coqui_core::{CrawlConfig, CrawlProgress, now_ms};
use coqui_crawl::run_crawl;
use coqui_store::Store;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let kind = args.next().unwrap_or_else(|| "sitemap".into());
    let seed = args
        .next()
        .expect("usage: audit <kind> <seed> <output_dir> [max_urls] [concurrency]");
    let out_dir = args.next().unwrap_or_else(|| "coqui-audit-output".into());
    let max_urls: u32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5000);
    let concurrency: u16 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(4);

    std::fs::create_dir_all(&out_dir).expect("create output dir");
    let store = Arc::new(Store::open(format!("{out_dir}/coqui.db")).expect("open store"));
    let crawl_id = format!("audit-{}", now_ms());
    let config = CrawlConfig {
        max_depth: 3,
        max_urls,
        concurrency,
        request_timeout_ms: 20_000,
        respect_robots: true,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(&crawl_id, &seed, &kind, &config, now_ms())
        .expect("create crawl");

    let started = Instant::now();
    let result = run_crawl(
        store.clone(),
        crawl_id.clone(),
        kind.clone(),
        seed.clone(),
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .expect("crawl");
    let crawl_elapsed = started.elapsed();
    let counts = analyze_crawl(&store, &crawl_id).expect("analyse");
    let elapsed = started.elapsed();

    let urls = store.all_urls(&crawl_id).unwrap();
    let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
    for url in &urls {
        let key = url
            .status
            .map(|status| status.to_string())
            .unwrap_or_else(|| "error".into());
        *statuses.entry(key).or_default() += 1;
    }
    let html = urls
        .iter()
        .filter(|url| {
            url.content_type
                .as_deref()
                .map(|value| value.contains("text/html"))
                .unwrap_or(false)
        })
        .count();
    let with_title = urls
        .iter()
        .filter(|url| url.title.as_deref().map(|t| !t.is_empty()).unwrap_or(false))
        .count();
    let with_meta = urls
        .iter()
        .filter(|url| {
            url.meta_description
                .as_deref()
                .map(|value| !value.is_empty())
                .unwrap_or(false)
        })
        .count();
    let with_canonical = urls.iter().filter(|url| url.canonical.is_some()).count();

    println!("seed: {seed} ({kind})");
    println!("crawl: {crawl_id} in {out_dir}");
    println!(
        "time: crawl {:.1}s total {:.1}s | urls {}/{} in {:.1}s => {:.1} urls/s",
        crawl_elapsed.as_secs_f64(),
        elapsed.as_secs_f64(),
        result.stats.crawled,
        max_urls,
        elapsed.as_secs_f64(),
        result.stats.crawled as f64 / elapsed.as_secs_f64().max(0.001)
    );
    println!(
        "stats: discovered={} failed={} redirected={} robots_blocked={} paused={}",
        result.stats.discovered,
        result.stats.failed,
        result.stats.redirected,
        result.stats.blocked_by_robots,
        result.paused
    );
    println!("status: {statuses:?}");
    println!(
        "html: {html} | title: {with_title} | meta: {with_meta} | canonical: {with_canonical}"
    );
    println!("issues:");
    for issue in &counts {
        println!(
            "  {:>6}  {:<10} {}",
            issue.count,
            format!("{:?}", issue.severity).to_lowercase(),
            issue.rule
        );
    }

    let csv = format!("{out_dir}/urls.csv");
    let written = store.export_csv(&crawl_id, &csv).expect("export csv");
    println!("exported {written} rows to {csv}");
    let db_size = std::fs::metadata(format!("{out_dir}/coqui.db"))
        .map(|meta| meta.len())
        .unwrap_or(0);
    println!("db size: {:.1} MB", db_size as f64 / 1_048_576.0);
}
