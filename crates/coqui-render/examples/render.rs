use coqui_render::Renderer;

#[tokio::main]
async fn main() {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com/".into());
    let wait_ms = std::env::args()
        .nth(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or(500);
    match Renderer::connect(None, wait_ms, coqui_render::RenderOptions::default()).await {
        Ok(renderer) => match renderer.render(&url).await {
            Ok(html) => {
                println!("rendered {} bytes", html.len());
                println!("{}", &html[..html.len().min(400)]);
            }
            Err(error) => eprintln!("render failed: {error}"),
        },
        Err(error) => eprintln!("renderer unavailable: {error}"),
    }
}
