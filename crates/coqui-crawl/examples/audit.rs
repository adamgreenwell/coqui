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
use reqwest::header::{COOKIE, HeaderMap, HeaderValue};
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
    let (cookie, user_agent) = load_session();
    if kind == "probe" {
        probe(&seed, cookie, user_agent).await;
        return;
    }
    let max_urls_arg: Option<u32> = args.next().and_then(|value| value.parse().ok());
    let concurrency: u16 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(4);
    let max_urls = max_urls_arg.unwrap_or(5000);

    std::fs::create_dir_all(&out_dir).expect("create output dir");
    let store = Arc::new(Store::open(format!("{out_dir}/coqui.db")).expect("open store"));
    let resume = std::env::var_os("COQUI_RESUME").is_some();

    let (crawl_id, config, checkpoint) = if resume {
        let latest = store
            .latest_crawl()
            .expect("read crawls")
            .expect("no crawl to resume");
        let mut config = latest.config.clone();
        config.concurrency = concurrency;
        if let Some(max_urls) = max_urls_arg {
            config.max_urls = max_urls;
        }
        let checkpoint = store.load_checkpoint(&latest.id).expect("load checkpoint");
        println!(
            "resuming crawl {} ({} rows stored, frontier {})",
            latest.id,
            latest.total_urls,
            checkpoint
                .as_ref()
                .map(|state| state.frontier.len())
                .unwrap_or(0)
        );
        (latest.id, config, checkpoint)
    } else {
        let crawl_id = format!("audit-{}", now_ms());
        let mut config = CrawlConfig {
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
        (crawl_id, config, None)
    };
    let mut config = config;
    if let Some(user_agent) = &user_agent {
        config.user_agent = user_agent.clone();
    }
    if let Some(cookie) = &cookie {
        println!(
            "session: cookie loaded ({} chars, not persisted)",
            cookie.len()
        );
    }
    if user_agent.is_some() {
        println!("session: user agent override applied");
    }

    // Session material is used in memory only; the stored crawl config stays credential-free.
    let mut run_config = config.clone();
    if let Some(cookie) = &cookie {
        run_config.custom_headers.push(format!("Cookie: {cookie}"));
    }

    if std::env::var_os("COQUI_DEBUG_SEEDS").is_some() {
        eprintln!("session: crawl prepared");
    }
    let started = Instant::now();
    let result = run_crawl(
        store.clone(),
        crawl_id.clone(),
        kind.clone(),
        seed.clone(),
        run_config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        checkpoint,
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

/// Load an authorised browser session from `.secrets/cloudflare.txt` or environment variables.
/// Format:
///   user-agent: Mozilla/5.0 ...
///   cookie: cf_clearance=...; __cf_bm=...
fn load_session() -> (Option<String>, Option<String>) {
    let mut user_agent = std::env::var("COQUI_USER_AGENT").ok();
    let mut cookie = std::env::var("COQUI_COOKIE").ok();
    let path = std::env::var("COQUI_SESSION_FILE")
        .unwrap_or_else(|_| ".secrets/cloudflare.txt".to_string());
    if let Ok(text) = std::fs::read_to_string(&path) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            match key.trim().to_ascii_lowercase().as_str() {
                "user-agent" | "ua" => user_agent = Some(value.trim().to_string()),
                "cookie" => cookie = Some(value.trim().to_string()),
                _ => {}
            }
        }
    }
    (cookie, user_agent)
}

async fn probe(url: &str, cookie: Option<String>, user_agent: Option<String>) {
    let mut headers = HeaderMap::new();
    if let Some(cookie) = &cookie {
        match HeaderValue::from_str(cookie) {
            Ok(value) => {
                headers.insert(COOKIE, value);
            }
            Err(error) => {
                println!("cookie header invalid: {error}");
                return;
            }
        }
    }
    let client = reqwest::Client::builder()
        .user_agent(user_agent.unwrap_or_else(|| "Coqui/0.1".to_string()))
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("client");
    match client.get(url).send().await {
        Ok(response) => {
            let status = response.status();
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_string();
            let body = response.text().await.unwrap_or_default();
            let title = body
                .split("<title>")
                .nth(1)
                .and_then(|rest| rest.split("</title>").next())
                .unwrap_or("")
                .trim()
                .to_string();
            let challenge = status.as_u16() == 403
                && (body.contains("Just a moment") || body.contains("cf-challenge"));
            println!(
                "probe: status={status} content-type={content_type} bytes={} cookie={} challenge={challenge}",
                body.len(),
                cookie.is_some()
            );
            println!("probe title: {title}");
        }
        Err(error) => println!("probe failed: {error}"),
    }
}
