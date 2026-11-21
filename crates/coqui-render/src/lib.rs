//! Chromium DevTools Protocol renderer.
//!
//! One headless browser is driven over a multiplexed CDP connection; each page renders in its own
//! tab, with up to `render_workers` tabs in flight. Session cookies, headers and user agent are
//! applied per tab before navigation.

use coqui_core::{AxeViolation, WebVitals};
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

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

/// Session material applied to every rendered page.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    pub user_agent: Option<String>,
    pub headers: Vec<(String, String)>,
    pub cookies: Vec<(String, String)>,
    pub screenshot: bool,
    pub block_assets: bool,
    pub wait_dom: bool,
    pub block_patterns: Vec<String>,
    pub accessibility: bool,
    pub custom_js: Vec<String>,
    pub web_vitals: bool,
    pub wait_selector: Option<String>,
}

/// Rendered HTML plus page evidence.
pub struct RenderOutput {
    pub html: String,
    pub screenshot: Option<Vec<u8>>,
    pub console: Vec<String>,
    pub axe: Vec<AxeViolation>,
    pub vitals: Option<WebVitals>,
    pub custom_js: Vec<Value>,
}

/// A background task that is aborted when dropped, including early-error paths.
struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// A multiplexed CDP connection: concurrent calls are correlated by id, events are broadcast.
struct CdpClient {
    tx: mpsc::Sender<String>,
    pending: Arc<tokio::sync::Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>,
    events: broadcast::Sender<Value>,
    next_id: AtomicU64,
}

impl CdpClient {
    async fn connect(ws_url: &str) -> Result<Arc<Self>, RenderError> {
        let (ws, _) = connect_async(ws_url)
            .await
            .map_err(|error| RenderError::Browser(error.to_string()))?;
        let (mut sink, mut stream) = ws.split();
        let (tx, mut rx) = mpsc::channel::<String>(256);
        let (events, _) = broadcast::channel::<Value>(4096);
        let pending: Arc<tokio::sync::Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>> =
            Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        tokio::spawn(async move {
            while let Some(text) = rx.recv().await {
                if sink.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
        });

        let reader_pending = pending.clone();
        let reader_events = events.clone();
        tokio::spawn(async move {
            while let Some(frame) = stream.next().await {
                let Ok(Message::Text(text)) = frame else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if let Some(id) = value.get("id").and_then(Value::as_u64) {
                    if let Some(sender) = reader_pending.lock().await.remove(&id) {
                        if let Some(error) = value.get("error") {
                            let _ = sender.send(Err(error.to_string()));
                        } else {
                            let _ = sender
                                .send(Ok(value.get("result").cloned().unwrap_or(Value::Null)));
                        }
                    }
                    continue;
                }
                let _ = reader_events.send(value);
            }
            let mut waiting = reader_pending.lock().await;
            for (_, sender) in waiting.drain() {
                let _ = sender.send(Err("browser closed the connection".into()));
            }
        });

        Ok(Arc::new(Self {
            tx,
            pending,
            events,
            next_id: AtomicU64::new(1),
        }))
    }

    fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.events.subscribe()
    }

    async fn call(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value, RenderError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut message = json!({"id": id, "method": method, "params": params});
        if let Some(session) = session {
            message["sessionId"] = Value::String(session.to_string());
        }
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(id, sender);
        self.tx
            .send(message.to_string())
            .await
            .map_err(|_| RenderError::Protocol("browser connection closed".into()))?;
        let response = tokio::time::timeout(Duration::from_secs(30), receiver)
            .await
            .map_err(|_| RenderError::Timeout(30_000))?
            .map_err(|_| RenderError::Protocol("browser did not answer".into()))?;
        response.map_err(RenderError::Protocol)
    }
}

pub struct Renderer {
    client: Arc<CdpClient>,
    child: StdMutex<Option<Child>>,
    wait_ms: u64,
    options: RenderOptions,
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
        let client = CdpClient::connect(ws_url).await?;
        Ok(Arc::new(Self {
            client,
            child: StdMutex::new(child),
            wait_ms,
            options,
        }))
    }

    /// Render one URL and return its outer HTML.
    pub async fn render(&self, url: &str) -> Result<String, RenderError> {
        Ok(self.render_page(url).await?.html)
    }

    /// Render one URL and return HTML, screenshot and console errors.
    pub async fn render_page(&self, url: &str) -> Result<RenderOutput, RenderError> {
        let mut events = self.client.subscribe();
        let target = self
            .client
            .call("Target.createTarget", json!({"url": "about:blank"}), None)
            .await?;
        let target_id = target
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| RenderError::Protocol("no target id".into()))?
            .to_string();
        let result = self.render_target(&mut events, &target_id, url).await;
        let _ = self
            .client
            .call("Target.closeTarget", json!({"targetId": target_id}), None)
            .await;
        result
    }

    async fn render_target(
        &self,
        events: &mut broadcast::Receiver<Value>,
        target_id: &str,
        url: &str,
    ) -> Result<RenderOutput, RenderError> {
        let attached = self
            .client
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

        self.client
            .call("Page.enable", json!({}), Some(&session))
            .await?;
        self.client
            .call("Network.enable", json!({}), Some(&session))
            .await?;
        self.client
            .call("Runtime.enable", json!({}), Some(&session))
            .await?;
        self.client
            .call("Log.enable", json!({}), Some(&session))
            .await?;

        let mut blocked: Vec<String> = Vec::new();
        if self.options.block_assets {
            blocked.extend(
                [
                    "*://*/*.woff",
                    "*://*/*.woff2",
                    "*://*/*.ttf",
                    "*://*/*.otf",
                    "*://*/*.mp4",
                    "*://*/*.webm",
                    "*://*/*.mp3",
                    "*://*/*.ogg",
                ]
                .iter()
                .map(|pattern| pattern.to_string()),
            );
        }
        blocked.extend(self.options.block_patterns.iter().cloned());
        if !blocked.is_empty() {
            self.client
                .call(
                    "Network.setBlockedURLs",
                    json!({"urls": blocked}),
                    Some(&session),
                )
                .await?;
        }
        if let Some(user_agent) = &self.options.user_agent {
            self.client
                .call(
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
            self.client
                .call(
                    "Network.setExtraHTTPHeaders",
                    json!({"headers": headers}),
                    Some(&session),
                )
                .await?;
        }
        for (name, value) in &self.options.cookies {
            self.client
                .call(
                    "Network.setCookie",
                    json!({"name": name, "value": value, "url": url}),
                    Some(&session),
                )
                .await?;
        }

        let network_bytes = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let image_bytes = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let network_session = session.clone();
        let bytes_counter = network_bytes.clone();
        let image_counter = image_bytes.clone();
        let mut byte_events = self.client.subscribe();
        let bytes_task = AbortOnDrop(tokio::spawn(async move {
            let mut images: std::collections::HashSet<String> = std::collections::HashSet::new();
            while let Ok(event) = byte_events.recv().await {
                if event.get("sessionId").and_then(Value::as_str) != Some(network_session.as_str())
                {
                    continue;
                }
                let method = event
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let params = event.get("params");
                match method {
                    "Network.responseReceived" => {
                        let request = params
                            .and_then(|params| params.get("requestId"))
                            .and_then(Value::as_str);
                        let mime = params
                            .and_then(|params| params.get("response"))
                            .and_then(|response| response.get("mimeType"))
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let kind = params
                            .and_then(|params| params.get("type"))
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        if let Some(request) = request
                            && (kind.eq_ignore_ascii_case("image") || mime.starts_with("image/"))
                        {
                            images.insert(request.to_string());
                        }
                    }
                    "Network.loadingFinished" => {
                        if let Some(request) = params
                            .and_then(|params| params.get("requestId"))
                            .and_then(Value::as_str)
                            && let Some(length) = params
                                .and_then(|params| params.get("encodedDataLength"))
                                .and_then(Value::as_u64)
                        {
                            bytes_counter.fetch_add(length, Ordering::Relaxed);
                            if images.remove(request) {
                                image_counter.fetch_add(length, Ordering::Relaxed);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }));

        let _ = self
            .client
            .call(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({"source": "performance.setResourceTimingBufferSize(2000);"}),
                Some(&session),
            )
            .await;
        self.client
            .call("Page.navigate", json!({"url": url}), Some(&session))
            .await?;

        let deadline = tokio::time::Instant::now() + Duration::from_millis(20_000);
        loop {
            let state = self
                .client
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
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
        if let Some(selector) = &self.options.wait_selector {
            let expression = format!(
                "!!document.querySelector({})",
                serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string())
            );
            let selector_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
            loop {
                let probe = self
                    .client
                    .call(
                        "Runtime.evaluate",
                        json!({"expression": expression, "returnByValue": true}),
                        Some(&session),
                    )
                    .await?;
                let found = probe
                    .get("result")
                    .and_then(|value| value.get("value"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if found {
                    break;
                }
                if tokio::time::Instant::now() >= selector_deadline {
                    return Err(RenderError::Timeout(10_000));
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        if self.wait_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.wait_ms)).await;
        }

        let console = drain_console(events, &session);
        if self.options.web_vitals {
            // Force a paint so paint timing entries exist in headless mode.
            let _ = self
                .client
                .call(
                    "Page.captureScreenshot",
                    json!({"format": "png", "clip": {"x": 0, "y": 0, "width": 1, "height": 1, "scale": 1}}),
                    Some(&session),
                )
                .await;
            // Paint entries arrive after load in headless Chrome; poll briefly so LCP/FCP exist.
            for _ in 0..20 {
                let probe = self
                    .client
                    .call(
                        "Runtime.evaluate",
                        json!({"expression": "performance.getEntriesByType('paint').length", "returnByValue": true}),
                        Some(&session),
                    )
                    .await?;
                let count = probe
                    .get("result")
                    .and_then(|value| value.get("value"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                if count > 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        let mut vitals = if self.options.web_vitals {
            let value = self
                .client
                .call(
                    "Runtime.evaluate",
                    json!({
                        "expression": "(async () => new Promise((resolve) => { let lcp = 0, cls = 0, tbt = 0; try { new PerformanceObserver((list) => { for (const entry of list.getEntries()) lcp = entry.startTime; }).observe({ type: 'largest-contentful-paint', buffered: true }); } catch (error) {} try { new PerformanceObserver((list) => { for (const entry of list.getEntries()) { if (!entry.hadRecentInput) cls += entry.value; } }).observe({ type: 'layout-shift', buffered: true }); } catch (error) {} try { new PerformanceObserver((list) => { for (const entry of list.getEntries()) tbt += Math.max(0, entry.duration - 50); }).observe({ type: 'longtask', buffered: true }); } catch (error) {} setTimeout(() => { const nav = performance.getEntriesByType('navigation')[0]; const paint = performance.getEntriesByName('first-contentful-paint')[0]; const resources = performance.getEntriesByType('resource'); const blockingSupported = resources.some((entry) => 'renderBlockingStatus' in entry); const blocking = blockingSupported ? resources.filter((entry) => entry.renderBlockingStatus === 'blocking').length : document.querySelectorAll('head script[src]:not([async]):not([defer]):not([type=\"module\"])').length + [...document.querySelectorAll('head link[rel=\"stylesheet\"][href]')].filter((link) => !link.media || window.matchMedia(link.media).matches).length; const transferBytes = (nav ? nav.transferSize || 0 : 0) + resources.reduce((sum, entry) => sum + (entry.transferSize || 0), 0); resolve(JSON.stringify({ lcpMs: Math.round(lcp), cls: Number(cls.toFixed(3)), tbtMs: Math.round(tbt), ttfbMs: nav ? Math.round(nav.responseStart) : 0, fcpMs: paint ? Math.round(paint.startTime) : 0, domNodes: document.getElementsByTagName('*').length, renderBlocking: blocking, transferBytes, resourceCount: resources.length, imageBytes: resources.filter((entry) => entry.initiatorType === 'img').reduce((sum, entry) => sum + (entry.transferSize || 0), 0), hasViewport: !!document.querySelector('meta[name=\"viewport\"]') })); }, 300); }))()",
                        "awaitPromise": true,
                        "returnByValue": true
                    }),
                    Some(&session),
                )
                .await?;
            value
                .get("result")
                .and_then(|value| value.get("value"))
                .and_then(Value::as_str)
                .and_then(|value| serde_json::from_str::<WebVitals>(value).ok())
        } else {
            None
        };
        if let Some(vitals) = vitals.as_mut() {
            let total = network_bytes.load(Ordering::Relaxed);
            if total > 0 {
                vitals.transfer_bytes = Some(total);
            }
            let images = image_bytes.load(Ordering::Relaxed);
            if images > 0 {
                vitals.image_bytes = Some(vitals.image_bytes.unwrap_or(0).max(images));
            }
        }
        bytes_task.0.abort();
        let axe = if self.options.accessibility {
            self.client
                .call(
                    "Runtime.evaluate",
                    json!({"expression": include_str!("../assets/axe.min.js")}),
                    Some(&session),
                )
                .await?;
            let result = self
                .client
                .call(
                    "Runtime.evaluate",
                    json!({
                        "expression": "(async () => { const r = await axe.run(document, { resultTypes: ['violations'] }); return JSON.stringify(r.violations.map(v => ({ id: v.id, impact: v.impact, help: v.help, helpUrl: v.helpUrl, nodes: v.nodes.length }))); })()",
                        "awaitPromise": true,
                        "returnByValue": true
                    }),
                    Some(&session),
                )
                .await?;
            result
                .get("result")
                .and_then(|value| value.get("value"))
                .and_then(Value::as_str)
                .and_then(|value| serde_json::from_str::<Vec<AxeViolation>>(value).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let screenshot = if self.options.screenshot {
            let shot = self
                .client
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
        let html = self
            .client
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
        let mut custom_js = Vec::new();
        for (index, expression) in self.options.custom_js.iter().enumerate() {
            let value = self
                .client
                .call(
                    "Runtime.evaluate",
                    json!({
                        "expression": expression,
                        "returnByValue": true,
                        "awaitPromise": true
                    }),
                    Some(&session),
                )
                .await;
            match value {
                Ok(value) => {
                    if let Some(exception) = value.get("exceptionDetails") {
                        custom_js.push(json!({
                            "index": index,
                            "error": exception
                                .get("text")
                                .cloned()
                                .unwrap_or_else(|| json!("exception")),
                        }));
                    } else {
                        custom_js.push(
                            value
                                .get("result")
                                .and_then(|result| result.get("value"))
                                .cloned()
                                .unwrap_or(Value::Null),
                        );
                    }
                }
                Err(error) => custom_js.push(json!({
                    "index": index,
                    "error": error.to_string(),
                })),
            }
        }
        Ok(RenderOutput {
            html,
            screenshot,
            console,
            axe,
            vitals,
            custom_js,
        })
    }
}

fn drain_console(events: &mut broadcast::Receiver<Value>, session: &str) -> Vec<String> {
    let mut console = Vec::new();
    loop {
        match events.try_recv() {
            Ok(event) => {
                if event.get("sessionId").and_then(Value::as_str) != Some(session) {
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
                        if event.pointer("/params/type").and_then(Value::as_str)
                            == Some("error") =>
                    {
                        let mut parts = Vec::new();
                        if let Some(args) = event.pointer("/params/args").and_then(Value::as_array)
                        {
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
                        if let Some(text) =
                            event.pointer("/params/entry/text").and_then(Value::as_str)
                        {
                            console.push(format!("network: {text}"));
                        }
                    }
                    _ => {}
                }
            }
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(broadcast::error::TryRecvError::Closed) => break,
        }
    }
    console.truncate(20);
    console
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = child.start_kill();
        }
    }
}

/// A pool of tabs on a single browser. `workers` limits concurrent tabs.
pub struct RenderPool {
    renderer: Arc<Renderer>,
    permits: tokio::sync::Semaphore,
}

impl RenderPool {
    pub async fn connect(
        workers: usize,
        browser_path: Option<&str>,
        wait_ms: u64,
        options: RenderOptions,
    ) -> Result<Arc<Self>, RenderError> {
        let workers = workers.clamp(1, 8);
        let renderer = Renderer::connect(browser_path, wait_ms, options).await?;
        Ok(Arc::new(Self {
            renderer,
            permits: tokio::sync::Semaphore::new(workers),
        }))
    }

    pub async fn render(&self, url: &str) -> Result<String, RenderError> {
        Ok(self.render_page(url).await?.html)
    }

    pub async fn render_page(&self, url: &str) -> Result<RenderOutput, RenderError> {
        let _permit = self
            .permits
            .acquire()
            .await
            .map_err(|_| RenderError::Other("render pool closed".into()))?;
        self.renderer.render_page(url).await
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
        .arg("--disable-background-timer-throttling")
        .arg("--disable-backgrounding-occluded-windows")
        .arg("--disable-renderer-backgrounding")
        .arg("--disable-features=CalculateNativeWinOcclusion")
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

/// Resolve the configured or discovered Chrome/Edge binary.
pub fn browser_executable(configured: Option<&str>) -> Option<PathBuf> {
    find_browser(configured)
}

fn cached_browser() -> Option<PathBuf> {
    let root = if cfg!(windows) {
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("coqui")
            .join("browser")
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?)
            .join("Library")
            .join("Caches")
            .join("coqui")
            .join("browser")
    } else {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
        base.join("coqui").join("browser")
    };
    if !root.exists() {
        return None;
    }
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("chrome") | Some("chrome.exe") | Some("Google Chrome for Testing")
            ) {
                return Some(path);
            }
        }
    }
    None
}

fn find_browser(configured: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("COQUI_BROWSER_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    if let Some(path) = configured.map(PathBuf::from)
        && path.exists()
    {
        return Some(path);
    }
    if let Some(path) = cached_browser() {
        return Some(path);
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
