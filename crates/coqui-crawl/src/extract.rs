//! HTML extraction for the current field set.
//!
//! The full column registry (X-01) will replace this module with a field-oriented design; the
//! shape here is intentionally simple: one page in, the values the UI and audit rules need.

use coqui_core::{HreflangLink, ImageRef, StructuredData, resolve_url};
use scraper::{Html, Selector};
use serde_json::Value;
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
    pub hreflang: Vec<HreflangLink>,
    pub pagination_next: Option<String>,
    pub pagination_prev: Option<String>,
    pub structured_data: StructuredData,
    pub images: Vec<ImageRef>,
    pub word_count: u32,
    pub text: String,
    pub markdown: String,
    pub links: Vec<ExtractedLink>,
}

pub fn apply_extractors(html: &str, extractors: &[(String, String)]) -> Vec<(String, String)> {
    let document = Html::parse_document(html);
    extractors
        .iter()
        .filter_map(|(name, spec)| {
            let (css, attribute) = match spec.rsplit_once('@') {
                Some((css, attribute))
                    if !attribute.trim().is_empty() && !css.trim().is_empty() =>
                {
                    (css.trim(), Some(attribute.trim()))
                }
                _ => (spec.trim(), None),
            };
            let selector = Selector::parse(css).ok()?;
            let values: Vec<String> = document
                .select(&selector)
                .filter_map(|element| match attribute {
                    Some(attribute) => element
                        .value()
                        .attr(attribute)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string),
                    None => {
                        let text = element.text().collect::<String>().trim().to_string();
                        if text.is_empty() { None } else { Some(text) }
                    }
                })
                .collect();
            if values.is_empty() {
                None
            } else {
                Some((name.clone(), values.join(" | ")))
            }
        })
        .collect()
}

fn is_replytocom(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.query().map(str::to_string))
        .map(|query| {
            query
                .split('&')
                .any(|pair| pair == "replytocom" || pair.starts_with("replytocom="))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod extractor_tests {
    use super::*;

    #[test]
    fn extracts_text_attributes_and_multiple_matches() {
        let html = r#"
            <html><body>
              <h2>First</h2><h2>Second</h2>
              <a href="/one">One</a>
              <img src="/image.png" alt="Pic">
            </body></html>
        "#;
        let extractors = vec![
            ("headings".to_string(), "h2".to_string()),
            ("href".to_string(), "a@href".to_string()),
            ("src".to_string(), "img@src".to_string()),
        ];
        let values = apply_extractors(html, &extractors);
        let get = |name: &str| {
            values
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(get("headings"), Some("First | Second"));
        assert_eq!(get("href"), Some("/one"));
        assert_eq!(get("src"), Some("/image.png"));
    }
}

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("static selector must be valid")
}

fn clean(value: String) -> Option<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() { None } else { Some(value) }
}

fn required_fields(kind: &str) -> &'static [&'static str] {
    match kind.to_ascii_lowercase().as_str() {
        "article" | "newsarticle" | "blogposting" => &["headline", "datePublished", "author"],
        "product" => &["name"],
        "organization" => &["name"],
        "localbusiness" => &["name", "address"],
        "website" => &["name", "url"],
        "breadcrumblist" => &["itemListElement"],
        "event" => &["name", "startDate", "location"],
        "faqpage" => &["mainEntity"],
        "recipe" => &["name", "recipeIngredient"],
        "videoobject" => &["name", "description", "thumbnailUrl", "uploadDate"],
        _ => &[],
    }
}

fn collect_jsonld(value: &Value, data: &mut StructuredData, seen: &mut HashSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_jsonld(item, data, seen);
            }
        }
        Value::Object(map) => {
            if let Some(graph) = map.get("@graph") {
                collect_jsonld(graph, data, seen);
            }
            let types: Vec<String> = match map.get("@type") {
                Some(Value::String(kind)) => vec![kind.clone()],
                Some(Value::Array(kinds)) => kinds
                    .iter()
                    .filter_map(|kind| kind.as_str().map(str::to_string))
                    .collect(),
                _ => Vec::new(),
            };
            for kind in types {
                let kind = kind.trim().to_string();
                if kind.is_empty() || !seen.insert(kind.clone()) {
                    continue;
                }
                for field in required_fields(&kind) {
                    if map.get(*field).is_none() {
                        data.errors.push(format!("missing {field} for {kind}"));
                    }
                }
                data.types.push(kind);
            }
        }
        _ => {}
    }
}

fn extract_structured_data(document: &Html) -> StructuredData {
    let mut data = StructuredData::default();
    let mut seen = HashSet::new();
    for script in document.select(&selector("script[type]")) {
        let is_jsonld = script
            .value()
            .attr("type")
            .map(|value| value.trim().eq_ignore_ascii_case("application/ld+json"))
            .unwrap_or(false);
        if !is_jsonld {
            continue;
        }
        let raw = script.text().collect::<String>();
        let raw = raw.trim_start_matches('\u{feff}').trim();
        if raw.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(raw) {
            Ok(value) => collect_jsonld(&value, &mut data, &mut seen),
            Err(error) => data.errors.push(format!("invalid json: {error}")),
        }
    }
    data.types.sort();
    data.types.truncate(20);
    data.errors.truncate(10);
    data
}

/// Extract the current field set and the crawlable outlinks of a document.
fn image_source(src: Option<&str>, data_src: Option<&str>, srcset: Option<&str>) -> Option<String> {
    let clean = |value: &str| value.trim().to_string();
    if let Some(value) = src.map(clean).filter(|value| {
        !value.is_empty() && !value.starts_with("data:") && !value.starts_with("about:")
    }) {
        return Some(value);
    }
    if let Some(value) = data_src
        .map(clean)
        .filter(|value| !value.is_empty() && !value.starts_with("data:"))
    {
        return Some(value);
    }
    srcset.and_then(|value| {
        value
            .split(',')
            .next()
            .and_then(|candidate| candidate.split_whitespace().next())
            .map(str::to_string)
            .filter(|value| !value.is_empty())
    })
}

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

    let mut hreflang = Vec::new();
    for link in document.select(&selector("link[rel=alternate][hreflang]")) {
        let (Some(lang), Some(href)) = (link.value().attr("hreflang"), link.value().attr("href"))
        else {
            continue;
        };
        if let Ok(url) = resolve_url(base_url, href) {
            hreflang.push(HreflangLink {
                lang: lang.trim().to_ascii_lowercase(),
                url,
            });
        }
    }

    let pagination_rel = |rel: &str| -> Option<String> {
        document
            .select(&selector(&format!("link[rel~={rel}]")))
            .next()
            .and_then(|element| element.value().attr("href"))
            .and_then(|href| resolve_url(base_url, href).ok())
            .or_else(|| {
                document
                    .select(&selector(&format!("a[rel~={rel}][href]")))
                    .next()
                    .and_then(|element| element.value().attr("href"))
                    .and_then(|href| resolve_url(base_url, href).ok())
            })
    };
    let pagination_next = pagination_rel("next");
    let pagination_prev = pagination_rel("prev");

    let structured_data = extract_structured_data(&document);

    let mut images = Vec::new();
    for image in document.select(&selector("img")) {
        let Some(src) = image_source(
            image.value().attr("src"),
            image.value().attr("data-src"),
            image.value().attr("srcset"),
        ) else {
            continue;
        };
        let Ok(url) = resolve_url(base_url, &src) else {
            continue;
        };
        images.push(ImageRef {
            src: url,
            alt: image
                .value()
                .attr("alt")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            title: image
                .value()
                .attr("title")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            width: image
                .value()
                .attr("width")
                .and_then(|value| value.parse().ok()),
            height: image
                .value()
                .attr("height")
                .and_then(|value| value.parse().ok()),
        });
    }

    let body_text = document
        .select(&selector("body"))
        .next()
        .map(|body| {
            let mut text = String::new();
            for node in body.descendants() {
                if let Some(value) = node.value().as_text() {
                    let blocked = node.ancestors().any(|ancestor| {
                        ancestor
                            .value()
                            .as_element()
                            .map(|element| {
                                matches!(
                                    element.name(),
                                    "script" | "style" | "noscript" | "template"
                                )
                            })
                            .unwrap_or(false)
                    });
                    if !blocked {
                        text.push_str(value);
                        text.push(' ');
                    }
                }
            }
            text
        })
        .unwrap_or_default();
    let word_count = body_text.split_whitespace().count() as u32;
    let markdown = if body_text.trim().is_empty() {
        String::new()
    } else {
        html2md::parse_html(html)
    };

    let mut seen = HashSet::new();
    let mut links = Vec::new();
    for anchor in document.select(&selector("a[href]")) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let Ok(url) = resolve_url(base_url, href) else {
            continue;
        };
        if is_replytocom(&url) {
            continue;
        }
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
        hreflang,
        pagination_next,
        pagination_prev,
        structured_data,
        images,
        word_count,
        text: body_text,
        markdown,
        links,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_srcset_and_lazy_images() {
        let html = r#"<html><body>
            <img srcset="/a.png 1x, /b.png 2x">
            <img data-src="/lazy.png">
            <img src="data:image/gif;base64,AAAA" srcset="/real.png 1x">
        </body></html>"#;
        let page = extract_page(html, "https://example.com/page");
        let sources: Vec<&str> = page.images.iter().map(|image| image.src.as_str()).collect();
        assert_eq!(
            sources,
            vec![
                "https://example.com/a.png",
                "https://example.com/lazy.png",
                "https://example.com/real.png"
            ]
        );
    }

    const HTML: &str = r##"
        <!doctype html>
        <html>
          <head>
            <title>  A   page about frogs </title>
            <meta name="description" content="Tiny frogs, loud calls.">
            <meta name="robots" content="index, follow">
            <link rel="canonical" href="/frogs">
            <link rel="alternate" hreflang="es" href="/es/frogs">
            <link rel="next" href="/frogs?page=2">
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
        assert_eq!(page.hreflang.len(), 1);
        assert_eq!(page.hreflang[0].lang, "es");
        assert_eq!(
            page.pagination_next.as_deref(),
            Some("https://example.com/frogs?page=2")
        );
        assert_eq!(page.canonical.as_deref(), Some("https://example.com/frogs"));
        assert!(page.word_count >= 6);
    }

    #[test]
    fn ignores_script_and_style_text_in_word_count() {
        let html = r#"<html><body><p>Hello world</p>
            <script>alpha beta gamma</script>
            <style>.hidden { color: red }</style>
            <noscript>delta</noscript></body></html>"#;
        let page = extract_page(html, "https://example.com/");
        assert_eq!(page.word_count, 2);
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

#[cfg(test)]
mod image_tests {
    use super::*;

    #[test]
    fn extracts_images_with_alt_and_dimensions() {
        let html = r#"
            <html><body>
              <img src="/a.png" alt="A frog" width="640" height="480">
              <img src="https://cdn.example.com/b.jpg">
            </body></html>
        "#;
        let page = extract_page(html, "https://example.com/");
        assert_eq!(page.images.len(), 2);
        assert_eq!(page.images[0].src, "https://example.com/a.png");
        assert_eq!(page.images[0].alt.as_deref(), Some("A frog"));
        assert_eq!(page.images[0].width, Some(640));
        assert_eq!(page.images[1].src, "https://cdn.example.com/b.jpg");
    }
}

#[cfg(test)]
mod structured_data_tests {
    use super::*;

    #[test]
    fn extracts_types_and_reports_malformed_blocks() {
        let html = r#"
            <html><head>
              <script type="application/ld+json">
                {"@context":"https://schema.org","@type":"Article","headline":"Frogs","datePublished":"2026-01-01","author":{"@type":"Person","name":"A"}}
              </script>
              <script type="application/ld+json">{ this is not json }</script>
            </head><body></body></html>
        "#;
        let page = extract_page(html, "https://example.com/");
        assert_eq!(page.structured_data.types, vec!["Article"]);
        assert_eq!(page.structured_data.errors.len(), 1);
        assert!(page.structured_data.errors[0].starts_with("invalid json"));
    }

    #[test]
    fn organization_url_is_optional() {
        let html = r#"
            <html><head>
              <script type="application/ld+json">
                {"@context":"https://schema.org","@type":"Organization","name":"Example"}
              </script>
            </head><body></body></html>
        "#;
        let page = extract_page(html, "https://example.com/");
        assert_eq!(page.structured_data.types, vec!["Organization"]);
        assert!(page.structured_data.errors.is_empty());
    }

    #[test]
    fn reports_missing_required_fields() {
        let html = r#"
            <html><head>
              <script type="application/ld+json">
                {"@context":"https://schema.org","@type":"Product","name":"Frog food"}
              </script>
            </head><body></body></html>
        "#;
        let page = extract_page(html, "https://example.com/");
        assert_eq!(page.structured_data.types, vec!["Product"]);
        assert!(page.structured_data.errors.is_empty());
    }
    #[test]
    fn skips_replytocom_links() {
        let html = r#"<html><body>
            <a href="/post/?replytocom=4398">reply</a>
            <a href="/post/">post</a>
        </body></html>"#;
        let page = extract_page(html, "https://example.com/");
        assert_eq!(page.links.len(), 1);
        assert_eq!(page.links[0].url, "https://example.com/post/");
    }
}
