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
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    while buffer.len() < 8192 {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                buffer.extend_from_slice(&chunk[..read]);
                if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let request = String::from_utf8_lossy(&buffer);
    let mut lines = request.lines();
    let path = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let path = path.split('?').next().unwrap_or(path);
    let mut host = "127.0.0.1".to_string();
    let mut authorization = String::new();
    for line in lines {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.eq_ignore_ascii_case("host") {
            host = value.trim().to_string();
        }
        if key.eq_ignore_ascii_case("authorization") {
            authorization = value.trim().to_string();
        }
    }
    let origin = format!("http://{host}");
    let expected_auth = format!("Basic {}", "dXNlcjpwYXNz");

    let (status, content_type, extra, body) = match path {
        "/robots.txt" => (
            "200 OK",
            "text/plain",
            "",
            "User-agent: *\nDisallow: /blocked\n".to_string(),
        ),
        "/sitemap-redirect" => ("301 Moved Permanently", "text/html", "Location: /sitemap.xml\r\n", String::new()),
        "/sitemap-images.xml" => (
            "200 OK",
            "application/xml",
            "",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
                <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
                        xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">
                  <url><loc>{origin}/a</loc>
                    <image:image><image:loc>{origin}/a.png</image:loc></image:image>
                  </url>
                  <url><loc>{origin}/b</loc></url>
                </urlset>"#
            ),
        ),
        "/sitemap.xml" => (
            "200 OK",
            "application/xml",
            "",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
                <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                  <url><loc>{origin}/a</loc></url>
                  <url><loc>{origin}/b</loc></url>
                  <url><loc>{origin}/about</loc></url>
                </urlset>"#
            ),
        ),
        "/sitemap-index.xml" => (
            "200 OK",
            "application/xml",
            "",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
                <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                  <sitemap><loc>{origin}/sitemap.xml</loc></sitemap>
                </sitemapindex>"#
            ),
        ),
        "/challenge" => (
            "403 Forbidden",
            "text/html",
            "cf-mitigated: challenge\r\n",
            "<html><head><title>Just a moment...</title></head><body>cf-challenge</body></html>"
                .to_string(),
        ),
        "/auth" => {
            if authorization == expected_auth {
                (
                    "200 OK",
                    "text/html; charset=utf-8",
                    "",
                    "<!doctype html><html><head><title>Private</title></head><body><h1>Private</h1></body></html>".to_string(),
                )
            } else {
                (
                    "401 Unauthorized",
                    "text/html",
                    "WWW-Authenticate: Basic realm=\"fixture\"\r\n",
                    "<html><body>auth required</body></html>".to_string(),
                )
            }
        }
        "/about" => (
            "200 OK",
            "text/html; charset=utf-8",
            "",
            "<!doctype html><html><head><title>About</title><meta name=\"description\" content=\"About\"></head><body><h1>About</h1></body></html>".to_string(),
        ),
        "/" => (
            "200 OK",
            "text/html; charset=utf-8",
            "",
            r#"<!doctype html><html><head><title>Home</title>
                 <meta name="description" content="Test home">
                 <link rel="canonical" href="/">
               </head><body><h1>Home</h1><h1>Second</h1>
                 <a href="/a">A</a>
                 <a href="/b">B</a>
                 <a href="/redirect">Redirect</a>
                 <a href="/blocked">Blocked</a>
                 <a href="https://external.invalid/x">External</a>
               </body></html>"#
                .to_string(),
        ),
        "/a" => (
            "200 OK",
            "text/html; charset=utf-8",
            "",
            r#"<!doctype html><html><head><title>A</title></head>
               <body><h1>A</h1><a href="/">Home</a></body></html>"#
                .to_string(),
        ),
        "/b" => (
            "200 OK",
            "text/html; charset=utf-8",
            "",
            "<!doctype html><html><head><title>B</title></head><body><h1>B</h1></body></html>"
                .to_string(),
        ),
        "/redirect" => ("302 Found", "text/html", "Location: /a\r\n", String::new()),
        "/blocked" => (
            "200 OK",
            "text/html; charset=utf-8",
            "",
            "<!doctype html><html><head><title>Blocked</title></head><body><h1>Blocked</h1></body></html>"
                .to_string(),
        ),
        _ => (
            "404 Not Found",
            "text/html",
            "",
            "<html><body>not found</body></html>".to_string(),
        ),
    };

    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
}

fn config() -> CrawlConfig {
    CrawlConfig {
        max_depth: 2,
        max_urls: 20,
        concurrency: 4,
        ..CrawlConfig::default()
    }
}

#[tokio::test]
async fn crawls_a_small_site() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "fixture-crawl";
    let seed = format!("{}/", site.base);
    let config = config();
    store
        .create_crawl(crawl_id, &seed, "url", &config, now_ms())
        .unwrap();

    let result = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "url".into(),
        seed.clone(),
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .expect("crawl succeeds");

    // home + a + b + redirect row; /blocked is disallowed by robots.txt.
    assert_eq!(result.stats.crawled, 4);
    assert_eq!(result.stats.redirected, 1);
    assert_eq!(result.stats.failed, 0);
    assert!(result.stats.blocked_by_robots >= 1);

    let page = store.list_urls(crawl_id, 1, 20, None).unwrap();
    assert_eq!(page.total, 4);
    let home = page
        .rows
        .iter()
        .find(|row| row.url == seed)
        .expect("home row");
    assert_eq!(home.title.as_deref(), Some("Home"));
    assert_eq!(home.h1_count, 2);
    assert_eq!(home.canonical.as_deref(), Some(seed.as_str()));

    let redirect = page
        .rows
        .iter()
        .find(|row| row.url.ends_with("/redirect"))
        .expect("redirect row");
    assert_eq!(redirect.status, Some(302));
    let redirect_target = format!("{}/a", site.base);
    assert_eq!(
        redirect.redirect_target.as_deref(),
        Some(redirect_target.as_str())
    );
    assert!(redirect.redirect_chain.len() >= 1);

    assert!(
        page.rows.iter().all(|row| !row.url.ends_with("/blocked")),
        "robots-disallowed URLs must not be crawled"
    );
    assert_eq!(store.all_links(crawl_id).unwrap().len(), 6);
}

#[tokio::test]
async fn can_ignore_robots() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "no-robots";
    let seed = format!("{}/", site.base);
    let config = CrawlConfig {
        respect_robots: false,
        ..config()
    };
    store
        .create_crawl(crawl_id, &seed, "url", &config, now_ms())
        .unwrap();
    let result = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "url".into(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert_eq!(result.stats.crawled, 5);
    assert!(
        store
            .list_urls(crawl_id, 1, 20, None)
            .unwrap()
            .rows
            .iter()
            .any(|row| row.url.ends_with("/blocked"))
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
        .create_crawl(crawl_id, &seed, "url", &config, now_ms())
        .unwrap();

    let result = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "url".into(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert_eq!(result.stats.crawled, 1);
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
        .create_crawl(crawl_id, &seed, "url", &config, now_ms())
        .unwrap();

    let token = CancellationToken::new();
    token.cancel();
    let result = run_crawl(
        store,
        crawl_id.to_string(),
        "url".into(),
        seed,
        config,
        token,
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await;
    assert!(matches!(result, Err(coqui_crawl::CrawlError::Cancelled)));
}

#[tokio::test]
async fn pauses_and_resumes() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "pause-resume";
    let seed = format!("{}/", site.base);
    let config = CrawlConfig {
        max_depth: 3,
        max_urls: 20,
        concurrency: 2,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, "url", &config, now_ms())
        .unwrap();

    let pause = Arc::new(AtomicBool::new(false));
    let trigger = pause.clone();
    let progress = Arc::new(move |event: CrawlProgress| {
        if event.crawled >= 1 {
            trigger.store(true, Ordering::Relaxed);
        }
    });
    let first = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "url".into(),
        seed.clone(),
        config.clone(),
        CancellationToken::new(),
        pause,
        None,
        progress,
    )
    .await
    .unwrap();
    assert!(first.paused);
    assert!(store.load_checkpoint(crawl_id).unwrap().is_some());
    let rows_after_pause = store.total_urls(crawl_id).unwrap();
    assert!(rows_after_pause >= 1);

    let checkpoint = store.load_checkpoint(crawl_id).unwrap();
    let second = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "url".into(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        checkpoint,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert!(!second.paused);
    // home + /a + /b + the /redirect row; /blocked stays disallowed by robots.txt.
    assert_eq!(store.total_urls(crawl_id).unwrap(), 4);
    assert!(store.load_checkpoint(crawl_id).unwrap().is_some());
}

#[tokio::test]
async fn sitemap_seed_crawls_every_location() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "sitemap-seed";
    let seed = format!("{}/sitemap.xml", site.base);
    let config = CrawlConfig {
        max_depth: 0,
        max_urls: 10,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, "sitemap", &config, now_ms())
        .unwrap();
    let result = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "sitemap".into(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert_eq!(result.stats.crawled, 3);
    let urls: Vec<String> = store
        .list_urls(crawl_id, 1, 10, None)
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row.url)
        .collect();
    assert!(urls.iter().any(|url| url.ends_with("/a")));
    assert!(urls.iter().any(|url| url.ends_with("/b")));
    assert!(urls.iter().any(|url| url.ends_with("/about")));
}

#[tokio::test]
async fn sitemap_index_is_followed() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "sitemap-index";
    let seed = format!("{}/sitemap-index.xml", site.base);
    let config = CrawlConfig {
        max_depth: 0,
        max_urls: 10,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, "sitemap", &config, now_ms())
        .unwrap();
    let result = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "sitemap".into(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert_eq!(result.stats.crawled, 3);
}

#[tokio::test]
async fn list_seed_reads_a_file() {
    let site = TestSite::start();
    let list_path = std::env::temp_dir().join(format!("coqui-list-{}.txt", uuid::Uuid::new_v4()));
    std::fs::write(
        &list_path,
        format!("{}/a\n# comment\n{}/b\n", site.base, site.base),
    )
    .unwrap();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "list-seed";
    let seed = list_path.to_string_lossy().to_string();
    let config = CrawlConfig {
        max_depth: 0,
        max_urls: 10,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, "list", &config, now_ms())
        .unwrap();
    let result = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "list".into(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert_eq!(result.stats.crawled, 2);
    std::fs::remove_file(list_path).ok();
}

#[tokio::test]
async fn excludes_keep_urls_out_of_the_crawl() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "scoped";
    let seed = format!("{}/", site.base);
    let config = CrawlConfig {
        max_depth: 2,
        max_urls: 20,
        exclude_patterns: vec!["/b$".into(), "/redirect$".into()],
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, "url", &config, now_ms())
        .unwrap();
    run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "url".into(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    let urls: Vec<String> = store
        .list_urls(crawl_id, 1, 20, None)
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row.url)
        .collect();
    assert_eq!(urls.len(), 2, "home and /a only: {urls:?}");
    assert!(!urls.iter().any(|url| url.ends_with("/b")));
    assert!(!urls.iter().any(|url| url.ends_with("/redirect")));
}

#[tokio::test]
async fn basic_auth_sends_credentials() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let seed = format!("{}/auth", site.base);
    let config = CrawlConfig {
        max_depth: 0,
        max_urls: 1,
        auth_user: Some("user".into()),
        auth_password: Some("pass".into()),
        ..CrawlConfig::default()
    };
    store
        .create_crawl("auth-ok", &seed, "url", &config, now_ms())
        .unwrap();
    run_crawl(
        store.clone(),
        "auth-ok".to_string(),
        "url".into(),
        seed.clone(),
        config.clone(),
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert_eq!(
        store.list_urls("auth-ok", 1, 1, None).unwrap().rows[0].status,
        Some(200)
    );

    let no_auth = CrawlConfig {
        auth_user: None,
        auth_password: None,
        ..config
    };
    store
        .create_crawl("auth-no", &seed, "url", &no_auth, now_ms())
        .unwrap();
    run_crawl(
        store.clone(),
        "auth-no".to_string(),
        "url".into(),
        seed,
        no_auth,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert_eq!(
        store.list_urls("auth-no", 1, 1, None).unwrap().rows[0].status,
        Some(401)
    );
}

#[tokio::test]
async fn sitemap_seed_follows_redirect() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "sitemap-redirect";
    let seed = format!("{}/sitemap-redirect", site.base);
    let config = CrawlConfig {
        max_depth: 0,
        max_urls: 10,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, "sitemap", &config, now_ms())
        .unwrap();
    let result = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "sitemap".into(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert_eq!(result.stats.crawled, 3);
}

#[tokio::test]
async fn sitemap_image_locations_are_not_seeds() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "sitemap-images";
    let seed = format!("{}/sitemap-images.xml", site.base);
    let config = CrawlConfig {
        max_depth: 0,
        max_urls: 10,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, "sitemap", &config, now_ms())
        .unwrap();
    let result = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "sitemap".into(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert_eq!(result.stats.crawled, 2);
    let urls: Vec<String> = store
        .list_urls(crawl_id, 1, 10, None)
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row.url)
        .collect();
    assert!(!urls.iter().any(|url| url.ends_with(".png")));
}

#[tokio::test]
async fn challenge_responses_are_blocked_not_content() {
    let site = TestSite::start();
    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "challenge";
    let seed = format!("{}/challenge", site.base);
    let config = CrawlConfig {
        max_depth: 0,
        max_urls: 1,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, "url", &config, now_ms())
        .unwrap();
    let result = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "url".into(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await
    .unwrap();
    assert_eq!(result.stats.blocked, 1);
    let row = &store.list_urls(crawl_id, 1, 1, None).unwrap().rows[0];
    assert!(row.blocked);
    assert!(row.title.is_none(), "challenge pages must not be extracted");
}

#[tokio::test]
async fn sustained_challenges_abort_the_crawl() {
    let site = TestSite::start();
    let list_path =
        std::env::temp_dir().join(format!("coqui-challenge-{}.txt", uuid::Uuid::new_v4()));
    let mut lines = String::new();
    for index in 0..30 {
        lines.push_str(&format!("{}/challenge?i={index}\n", site.base));
    }
    std::fs::write(&list_path, lines).unwrap();

    let store = Arc::new(Store::open_in_memory().unwrap());
    let crawl_id = "challenge-abort";
    let seed = list_path.to_string_lossy().to_string();
    let config = CrawlConfig {
        max_depth: 0,
        max_urls: 30,
        concurrency: 5,
        ..CrawlConfig::default()
    };
    store
        .create_crawl(crawl_id, &seed, "list", &config, now_ms())
        .unwrap();
    let result = run_crawl(
        store.clone(),
        crawl_id.to_string(),
        "list".into(),
        seed,
        config,
        CancellationToken::new(),
        Arc::new(AtomicBool::new(false)),
        None,
        Arc::new(|_: CrawlProgress| {}),
    )
    .await;
    assert!(
        matches!(result, Err(coqui_crawl::CrawlError::Blocked(_))),
        "expected Blocked, got {result:?}"
    );
    std::fs::remove_file(list_path).ok();
}
