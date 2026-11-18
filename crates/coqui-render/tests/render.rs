use coqui_render::{RenderOptions, RenderPool, Renderer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn serve_once(listener: tokio::net::TcpListener, body: &'static str, require_session: bool) {
    if let Ok((mut socket, _)) = listener.accept().await {
        let mut buffer = [0u8; 4096];
        let read = socket.read(&mut buffer).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
        let authorised = !require_session
            || (request.contains("cookie: session=ok") && request.contains("x-test: yes"));
        let (status, payload) = if authorised {
            ("200 OK", body)
        } else {
            (
                "401 Unauthorized",
                "<html><body><h1>Denied</h1></body></html>",
            )
        };
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
            payload.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
    }
}

#[tokio::test]
#[ignore = "requires a local Chrome/Edge installation"]
async fn renders_javascript_generated_dom() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(serve_once(
        listener,
        r#"<!doctype html><html><head><title>Static</title></head><body>
            <div id="app"></div>
            <script>
              document.title = 'Rendered title';
              document.getElementById('app').innerHTML = '<h1>Rendered H1</h1>';
            </script>
        </body></html>"#,
        false,
    ));

    let renderer = Renderer::connect(None, 300, RenderOptions::default())
        .await
        .expect("a local Chrome/Edge is required");
    let html = renderer
        .render(&format!("http://{address}/"))
        .await
        .expect("render");
    assert!(
        html.contains("Rendered H1"),
        "rendered HTML missing h1: {html}"
    );
    assert!(
        html.contains("Rendered title"),
        "rendered HTML missing title"
    );
}

#[tokio::test]
#[ignore = "requires a local Chrome/Edge installation"]
async fn applies_session_headers_and_cookies() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(serve_once(
        listener,
        "<!doctype html><html><body><h1>Authorised</h1></body></html>",
        true,
    ));

    let options = RenderOptions {
        user_agent: Some("Coqui-Test".into()),
        headers: vec![("X-Test".into(), "yes".to_string())],
        cookies: vec![("session".into(), "ok".to_string())],
    };
    let renderer = Renderer::connect(None, 200, options)
        .await
        .expect("a local Chrome/Edge is required");
    let html = renderer
        .render(&format!("http://{address}/"))
        .await
        .expect("render");
    assert!(
        html.contains("Authorised"),
        "session was not applied: {html}"
    );
}

#[tokio::test]
#[ignore = "requires a local Chrome/Edge installation"]
async fn pool_renders_two_pages() {
    let first_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let second_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let first_address = first_listener.local_addr().unwrap();
    let second_address = second_listener.local_addr().unwrap();
    tokio::spawn(serve_once(
        first_listener,
        "<!doctype html><html><body><h1>Pooled</h1></body></html>",
        false,
    ));
    tokio::spawn(serve_once(
        second_listener,
        "<!doctype html><html><body><h1>Pooled</h1></body></html>",
        false,
    ));

    let pool = RenderPool::connect(2, None, 100, RenderOptions::default())
        .await
        .expect("a local Chrome/Edge is required");
    let first_url = format!("http://{first_address}/");
    let second_url = format!("http://{second_address}/");
    let first = pool.render(&first_url);
    let second = pool.render(&second_url);
    let (first, second) = tokio::join!(first, second);
    assert!(first.expect("first").contains("Pooled"));
    assert!(second.expect("second").contains("Pooled"));
}
