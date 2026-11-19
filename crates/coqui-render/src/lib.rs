//! Minimal Chromium DevTools Protocol renderer.
//!
//! The renderer drives an external Chrome/Edge (headless by default), creates a tab per page,
//! waits for `document.readyState === "complete"`, and returns the rendered outer HTML.
//! Rendering is serialised: one page at a time per renderer instance.

use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("no Chrome/Edge binary found; set COQUI_BROWSER_PATH")]
    NoBrowser,
    #[error("could not reach the browser: {0}")]
    Browser(String),
    #[error("CDP error: {0}")]
    Protocol(String),
    #[error("page did not finish loading within {0} ms")]
    Timeout(u64),
    #[error("render failed: {0}")]
    Other(String),
}

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Session material applied to every rendered page.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    pub user_agent: Option<String>,
    pub headers: Vec<(String, String)>,
    pub cookies: Vec<(String, String)>,
    pub screenshot: bool,
    pub block_assets: bool,
    pub wait_dom: bool,
}

/// Rendered HTML plus page evidence.
pub struct RenderOutput {
    pub html: String,
    pub screenshot: Option<Vec<u8>>,
    pub console: Vec<String>,
}

pub struct Renderer {
    ws: Mutex<Ws>,
    next_id: AtomicU64,
    child: StdMutex<Option<Child>>,
    wait_ms: u64,
    options: RenderOptions,
    events: StdMutex<Vec<Value>>,
}

impl Renderer {
    /// Connect to `COQUI_CDP_ENDPOINT` if set, otherwise launch a headless browser.
    pub async fn connect(
        browser_path: Option<&str>,
        wait_ms: u64,
        options: RenderOptions,
    ) -> Result<Arc<Self>, RenderError> {
        let (endpoint, child) = match std::env::var("COQUI_CDP_ENDPOINT") {
            Ok(endpoint) if !endpoint.is_empty() => {
                (endpoint.trim_end_matches('/').to_string(), None)
            }
            _ => launch(browser_path).await?,
        };
        let version: Value = reqwest::get(format!("{endpoint}/json/version"))
            .await
            .map_err(|error| RenderError::Browser(error.to_string()))?
            .json()
            .await
            .map_err(|error| RenderError::Browser(error.to_string()))?;
        let ws_url = version
            .get("webSocketDebuggerUrl")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RenderError::Protocol("browser did not expose a websocket URL".into())
            })?;
        let (ws, _) = connect_async(ws_url)
            .await
            .map_err(|error| RenderError::Browser(error.to_string()))?;
        Ok(Arc::new(Self {
            ws: Mutex::new(ws),
            next_id: AtomicU64::new(1),
            child: StdMutex::new(child),
            wait_ms,
            options,
            events: StdMutex::new(Vec::new()),
        }))
    }

    /// Render one URL and return its outer HTML.
    pub async fn render(&self, url: &str) -> Result<String, RenderError> {
        Ok(self.render_page(url).await?.html)
    }

    /// Render one URL and return HTML, screenshot and console errors.
    pub async fn render_page(&self, url: &str) -> Result<RenderOutput, RenderError> {
        let target = self
            .call("Target.createTarget", json!({"url": "about:blank"}), None)
            .await?;
        let target_id = target
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| RenderError::Protocol("no target id".into()))?
            .to_string();
        let result = self.render_target(&target_id, url).await;
        let _ = self
            .call("Target.closeTarget", json!({"targetId": target_id}), None)
            .await;
        result
    }

    async fn render_target(&self, target_id: &str, url: &str) -> Result<RenderOutput, RenderError> {
        let started = tokio::time::Instant::now();
        let attached = self
            .call(
                "Target.attachToTarget",
                json!({"targetId": target_id, "flatten": true}),
                None,
            )
            .await?;
        let session = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| RenderError::Protocol("no session id".into()))?
            .to_string();
        self.call("Page.enable", json!({}), Some(&session)).await?;
        self.call("Network.enable", json!({}), Some(&session))
            .await?;
        if self.options.block_assets {
            self.call(
                "Network.setBlockedURLs",
                json!({
                    "urls": [
                        "*://*/*.woff",
                        "*://*/*.woff2",
                        "*://*/*.ttf",
                        "*://*/*.otf",
                        "*://*/*.mp4",
                        "*://*/*.webm",
                        "*://*/*.mp3",
                        "*://*/*.ogg"
                    ]
                }),
                Some(&session),
            )
            .await?;
        }
        if let Some(user_agent) = &self.options.user_agent {
            self.call(
                "Emulation.setUserAgentOverride",
                json!({"userAgent": user_agent}),
                Some(&session),
            )
            .await?;
        }
        if !self.options.headers.is_empty() {
            let headers: serde_json::Map<String, Value> = self
                .options
                .headers
                .iter()
                .map(|(name, value)| (name.clone(), Value::String(value.clone())))
                .collect();
            self.call(
                "Network.setExtraHTTPHeaders",
                json!({"headers": headers}),
                Some(&session),
            )
            .await?;
        }
        for (name, value) in &self.options.cookies {
            self.call(
                "Network.setCookie",
                json!({"name": name, "value": value, "url": url}),
                Some(&session),
            )
            .await?;
        }
        if let Ok(mut events) = self.events.lock() {
            events.clear();
        }
        self.call("Runtime.enable", json!({}), Some(&session))
            .await?;
        self.call("Log.enable", json!({}), Some(&session)).await?;
        self.call("Page.navigate", json!({"url": url}), Some(&session))
            .await?;

        let deadline = tokio::time::Instant::now() + Duration::from_millis(20_000);
        loop {
            let state = self
                .call(
                    "Runtime.evaluate",
                    json!({"expression": "document.readyState", "returnByValue": true}),
                    Some(&session),
                )
                .await?;
            let ready = state
                .get("result")
                .and_then(|value| value.get("value"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let ready_enough = if self.options.wait_dom {
                ready == "interactive" || ready == "complete"
            } else {
                ready == "complete"
            };
            if ready_enough {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(RenderError::Timeout(20_000));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if std::env::var_os("COQUI_DEBUG_RENDER").is_some() {
            eprintln!("render: ready for {url} in {:?}", started.elapsed());
        }
        if self.wait_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.wait_ms)).await;
        }
        let events = self
            .events
            .lock()
            .map_err(|_| RenderError::Other("event log poisoned".into()))?
            .clone();
        let mut console = Vec::new();
        for event in events {
            if event.get("sessionId").and_then(Value::as_str) != Some(session.as_str()) {
                continue;
            }
            match event.get("method").and_then(Value::as_str).unwrap_or("") {
                "Runtime.exceptionThrown" => {
                    if let Some(details) = event.pointer("/params/exceptionDetails") {
                        let text = details
                            .pointer("/exception/description")
                            .and_then(Value::as_str)
                            .or_else(|| details.get("text").and_then(Value::as_str))
                            .unwrap_or("Uncaught exception");
                        console.push(format!("js: {}", text.lines().next().unwrap_or(text)));
                    }
                }
                "Runtime.consoleAPICalled"
                    if event.pointer("/params/type").and_then(Value::as_str) == Some("error") =>
                {
                    let mut parts = Vec::new();
                    if let Some(args) = event.pointer("/params/args").and_then(Value::as_array) {
                        for arg in args {
                            if let Some(value) = arg.get("value").and_then(Value::as_str) {
                                parts.push(value.to_string());
                            } else if let Some(description) =
                                arg.get("description").and_then(Value::as_str)
                            {
                                parts.push(description.to_string());
                            }
                        }
                    }
                    if !parts.is_empty() {
                        console.push(format!("console: {}", parts.join(" ")));
                    }
                }
                "Log.entryAdded"
                    if event.pointer("/params/entry/level").and_then(Value::as_str)
                        == Some("error") =>
                {
                    if let Some(text) = event.pointer("/params/entry/text").and_then(Value::as_str)
                    {
                        console.push(format!("network: {text}"));
                    }
                }
                _ => {}
            }
        }
        console.truncate(20);

        let screenshot = if self.options.screenshot {
            let shot = self
                .call(
                    "Page.captureScreenshot",
                    json!({"format": "jpeg", "quality": 60}),
                    Some(&session),
                )
                .await?;
            shot.get("data").and_then(Value::as_str).and_then(|data| {
                use base64::Engine as _;
                base64::engine::general_purpose::STANDARD.decode(data).ok()
            })
        } else {
            None
        };

        if std::env::var_os("COQUI_DEBUG_RENDER").is_some() {
            eprintln!("render: evidence for {url} in {:?}", started.elapsed());
        }
        let html = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": "document.documentElement ? document.documentElement.outerHTML : ''",
                    "returnByValue": true
                }),
                Some(&session),
            )
            .await?;
        let html = html
            .get("result")
            .and_then(|value| value.get("value"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| RenderError::Protocol("no rendered HTML".into()))?;
        Ok(RenderOutput {
            html,
            screenshot,
            console,
        })
    }

    async fn call(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value, RenderError> {
        let mut ws = self.ws.lock().await;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut message = json!({"id": id, "method": method, "params": params});
        if let Some(session) = session {
            message["sessionId"] = Value::String(session.to_string());
        }
        ws.send(Message::Text(message.to_string().into()))
            .await
            .map_err(|error| RenderError::Protocol(error.to_string()))?;
        loop {
            let frame = ws
                .next()
                .await
                .ok_or_else(|| RenderError::Protocol("browser closed the connection".into()))?
                .map_err(|error| RenderError::Protocol(error.to_string()))?;
            let Message::Text(text) = frame else {
                continue;
            };
            let value: Value = serde_json::from_str(&text)
                .map_err(|error| RenderError::Protocol(error.to_string()))?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                if value.get("method").is_some() {
                    if let Ok(mut events) = self.events.lock() {
                        if events.len() < 500 {
                            events.push(value);
                        }
                    }
                }
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(RenderError::Protocol(error.to_string()));
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            if let Some(child) = child.as_mut() {
                let _ = child.start_kill();
            }
        }
    }
}

async fn launch(browser_path: Option<&str>) -> Result<(String, Option<Child>), RenderError> {
    let browser = find_browser(browser_path).ok_or(RenderError::NoBrowser)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|error| RenderError::Browser(error.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|error| RenderError::Browser(error.to_string()))?
        .port();
    drop(listener);

    let profile = std::env::temp_dir().join(format!("coqui-render-{}", uuid::Uuid::new_v4()));
    let mut command = Command::new(&browser);
    command
        .arg("--headless=new")
        .arg(format!("--remote-debugging-port={port}"))
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-gpu")
        .arg("--disable-dev-shm-usage")
        .arg("about:blank")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command
        .spawn()
        .map_err(|error| RenderError::Browser(error.to_string()))?;

    let endpoint = format!("http://127.0.0.1:{port}");
    for _ in 0..150 {
        if reqwest::get(format!("{endpoint}/json/version"))
            .await
            .is_ok()
        {
            return Ok((endpoint, Some(child)));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(RenderError::Browser(
        "browser did not expose a debugging port".into(),
    ))
}

fn find_browser(configured: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("COQUI_BROWSER_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    if let Some(path) = configured.map(PathBuf::from) {
        if path.exists() {
            return Some(path);
        }
    }
    let candidates = if cfg!(windows) {
        [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ]
        .as_slice()
    } else if cfg!(target_os = "macos") {
        [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
        ]
        .as_slice()
    } else {
        [
            "/usr/bin/google-chrome",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
        ]
        .as_slice()
    };
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
}

/// A fixed-size pool of renderer browsers with session material applied to each.
pub struct RenderPool {
    workers: Vec<Arc<Renderer>>,
    available: tokio::sync::mpsc::Sender<usize>,
    slots: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<usize>>,
}

impl RenderPool {
    pub async fn connect(
        workers: usize,
        browser_path: Option<&str>,
        wait_ms: u64,
        options: RenderOptions,
    ) -> Result<Arc<Self>, RenderError> {
        let workers = workers.clamp(1, 8);
        let mut pool = Vec::with_capacity(workers);
        for _ in 0..workers {
            pool.push(Renderer::connect(browser_path, wait_ms, options.clone()).await?);
        }
        let (available, slots) = tokio::sync::mpsc::channel(workers);
        for index in 0..workers {
            let _ = available.try_send(index);
        }
        Ok(Arc::new(Self {
            workers: pool,
            available,
            slots: tokio::sync::Mutex::new(slots),
        }))
    }

    pub async fn render(&self, url: &str) -> Result<String, RenderError> {
        Ok(self.render_page(url).await?.html)
    }

    pub async fn render_page(&self, url: &str) -> Result<RenderOutput, RenderError> {
        let index = self
            .slots
            .lock()
            .await
            .recv()
            .await
            .ok_or_else(|| RenderError::Other("render pool closed".into()))?;
        let result = self.workers[index].render_page(url).await;
        let _ = self.available.send(index).await;
        result
    }
}
