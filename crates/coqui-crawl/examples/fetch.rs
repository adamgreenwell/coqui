//! Tiny manual probe for transport timing: `cargo run -p coqui-crawl --example fetch -- <url>`

use reqwest::Client;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com/".to_string());
    let started = Instant::now();
    let client = Client::builder()
        .user_agent("Coqui/0.1 (transport probe)")
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .expect("client");
    println!("client built in {:?}", started.elapsed());
    let response = client.get(&url).send().await.expect("send");
    println!(
        "status {} headers in {:?}",
        response.status(),
        started.elapsed()
    );
    let body = response.bytes().await.expect("body");
    println!("body {} bytes in {:?}", body.len(), started.elapsed());
}
