use coqui_core::{CrawlConfig, CrawlProgress, now_ms};
use coqui_crawl::run_crawl;
use coqui_store::Store;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

struct TestSite {
    base: String,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestSite {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test site");
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        listener.set_nonblocking(true).unwrap();
        let stop_thread = stop.clone();
        let handle = thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        thread::spawn(move || handle_connection(stream));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        Self {
            base: format!("http://127.0.0.1:{port}"),
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for TestSite {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0u8; 2048];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let (status, body) = match path {
        "/" => (
            "200 OK",
            r#"<!doctype html><html><head><title>Home</title>
                 <meta name="description" content="Test home">
               </head><body><h1>Home</h1>
                 <a href="/a">A</a>
                 <a href="/b">B</a>
                 <a href="https://external.invalid/x">External</a>
               </body></html>"#,
        ),
        "/a" => (
            "200 OK",
            r#"<!doctype html><html><head><title>A</title></head>
               <body><h1>A</h1><a href="/">Home</a></body></html>"#,
        ),
        "/b" => (
            "200 OK",
            "<!doctype html><html><head><title>B</title></head><body><h1>B</h1></body></html>",
        ),
        _ => ("404 Not Found", "<html><body>not found</body></html>"),
    };

    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

#[tokio::test]
async fn crawls_a_small_site() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "fixture-crawl";
    let seed = format!("{}/", site.base);
    let config = CrawlConfig {
        max_depth: 2,
        max_urls: 10,
        concurrency: 4,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, &config, now_ms())
        .unwrap();

    let progress = Arc::new(|_: CrawlProgress| {});
    let stats = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        seed,
        config,
        CancellationToken::new(),
        progress,
    )
    .await
    .expect("crawl succeeds");

    assert_eq!(stats.crawled, 3, "home + a + b");
    assert_eq!(stats.failed, 0);

    let page = store.list_urls(crawl_id, 1, 10).unwrap();
    assert_eq!(page.total, 3);
    assert!(page.rows.iter().any(|row| row.title.as_deref() == Some("Home")));
    assert!(
        page.rows
            .iter()
            .all(|row| !row.url.contains("external.invalid")),
        "same-host crawl must not follow external links"
    );
}

#[tokio::test]
async fn respects_max_urls() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "limited-crawl";
    let seed = format!("{}/", site.base);
    let config = CrawlConfig {
        max_depth: 5,
        max_urls: 1,
        concurrency: 2,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, &config, now_ms())
        .unwrap();

    let stats = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert_eq!(stats.crawled, 1);
    assert_eq!(store.total_urls(crawl_id).unwrap(), 1);
}

#[tokio::test]
async fn cancellation_stops_the_crawl() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "cancelled-crawl";
    let seed = format!("{}/", site.base);
    let config = CrawlConfig {
        max_depth: 5,
        max_urls: 50,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, &config, now_ms())
        .unwrap();

    let token = CancellationToken::new();
    token.cancel();
    let result = run_crawl(
        store,
        crawl_id.to_string(),
        seed,
        config,
        token,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await;
    assert!(matches!(result, Err(coqui_crawl::CrawlError::Cancelled)));
}
