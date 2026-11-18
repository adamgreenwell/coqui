use coqui_render::Renderer;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
#[ignore = "requires a local Chrome/Edge installation"]
async fn renders_javascript_generated_dom() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buffer = [0u8; 2048];
            let _ = socket.read(&mut buffer).await;
            let body = r#"<!doctype html><html><head><title>Static</title></head><body>
                <div id="app"></div>
                <script>
                  document.title = 'Rendered title';
                  document.getElementById('app').innerHTML = '<h1>Rendered H1</h1>';
                </script>
            </body></html>"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });

    let renderer = Renderer::connect(None, 300)
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
