//! HTML extraction for the current field set.
//!
//! The full column registry (X-01) will replace this module with a field-oriented design; the
//! shape here is intentionally simple: one page in, the values the UI and audit rules need.

use coqui_core::resolve_url;
use scraper::{Html, Selector};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedLink {
    pub url: String,
    pub anchor: Option<String>,
    pub rel: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExtractedPage {
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub h1: Option<String>,
    pub h1_count: u32,
    pub canonical: Option<String>,
    pub meta_robots: Option<String>,
    pub word_count: u32,
    pub links: Vec<ExtractedLink>,
}

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("static selector must be valid")
}

fn clean(value: String) -> Option<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() { None } else { Some(value) }
}

/// Extract the current field set and the crawlable outlinks of a document.
pub fn extract_page(html: &str, base_url: &str) -> ExtractedPage {
    let document = Html::parse_document(html);

    let title = document
        .select(&selector("title"))
        .next()
        .and_then(|element| clean(element.text().collect::<String>()));

    let meta_description = document
        .select(&selector("meta[name=description]"))
        .next()
        .and_then(|element| element.value().attr("content"))
        .and_then(|content| clean(content.to_string()));

    let meta_robots = document
        .select(&selector("meta[name=robots]"))
        .next()
        .and_then(|element| element.value().attr("content"))
        .and_then(|content| clean(content.to_string()))
        .map(|value| value.to_ascii_lowercase());

    let h1 = document
        .select(&selector("h1"))
        .next()
        .and_then(|element| clean(element.text().collect::<String>()));
    let h1_count = document.select(&selector("h1")).count() as u32;

    let canonical = document
        .select(&selector("link[rel=canonical]"))
        .next()
        .and_then(|element| element.value().attr("href"))
        .and_then(|href| resolve_url(base_url, href).ok());

    let body_text = document
        .select(&selector("body"))
        .next()
        .map(|body| body.text().collect::<String>())
        .unwrap_or_default();
    let word_count = body_text.split_whitespace().count() as u32;

    let mut seen = HashSet::new();
    let mut links = Vec::new();
    for anchor in document.select(&selector("a[href]")) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let Ok(url) = resolve_url(base_url, href) else {
            continue;
        };
        if !seen.insert(url.clone()) {
            continue;
        }
        links.push(ExtractedLink {
            url,
            anchor: clean(anchor.text().collect::<String>()),
            rel: anchor.value().attr("rel").map(|value| value.to_string()),
        });
    }

    ExtractedPage {
        title,
        meta_description,
        h1,
        h1_count,
        canonical,
        meta_robots,
        word_count,
        links,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = r##"
        <!doctype html>
        <html>
          <head>
            <title>  A   page about frogs </title>
            <meta name="description" content="Tiny frogs, loud calls.">
            <meta name="robots" content="index, follow">
            <link rel="canonical" href="/frogs">
          </head>
          <body>
            <h1>Coquí</h1>
            <h1>Second heading</h1>
            <p>One two three four five.</p>
            <a href="/about" rel="nofollow">About</a>
            <a href="https://example.com/about#team">About again</a>
            <a href="mailto:hi@example.com">Mail</a>
            <a href="#top">Top</a>
          </body>
        </html>
    "##;

    #[test]
    fn extracts_core_fields() {
        let page = extract_page(HTML, "https://example.com/");
        assert_eq!(page.title.as_deref(), Some("A page about frogs"));
        assert_eq!(
            page.meta_description.as_deref(),
            Some("Tiny frogs, loud calls.")
        );
        assert_eq!(page.h1.as_deref(), Some("Coquí"));
        assert_eq!(page.h1_count, 2);
        assert_eq!(page.meta_robots.as_deref(), Some("index, follow"));
        assert_eq!(page.canonical.as_deref(), Some("https://example.com/frogs"));
        assert!(page.word_count >= 6);
    }

    #[test]
    fn normalises_and_deduplicates_links() {
        let page = extract_page(HTML, "https://example.com/");
        // `/about` and `https://example.com/about#team` share one identity after normalisation.
        assert_eq!(page.links.len(), 1);
        assert_eq!(page.links[0].url, "https://example.com/about");
        assert_eq!(page.links[0].anchor.as_deref(), Some("About"));
        assert_eq!(page.links[0].rel.as_deref(), Some("nofollow"));
    }

    #[test]
    fn tolerates_malformed_html() {
        let page = extract_page("<html><body><h1>Only a heading", "https://example.com/");
        assert_eq!(page.h1.as_deref(), Some("Only a heading"));
    }
}
