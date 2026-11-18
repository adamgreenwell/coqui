//! URL identity: normalisation and link resolution.
//!
//! Coqui follows the same rules it will expose in the UI: lowercase scheme/host (the `url` crate
//! does this), no fragment, explicit `/` path, preserved query. Parameter stripping and rewrite
//! rules are configuration and land with C-04; this module is the deterministic base.

use thiserror::Error;
use url::{ParseError, Url};

#[derive(Debug, Error)]
pub enum UrlError {
    #[error("invalid URL: {0}")]
    Parse(#[from] ParseError),
    #[error("unsupported URL scheme: {0}")]
    UnsupportedScheme(String),
    #[error("empty or unresolvable URL")]
    Unresolvable,
}

fn is_crawlable_scheme(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

/// Normalise an absolute URL into its crawl identity.
pub fn normalize_url(raw: &str) -> Result<String, UrlError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(UrlError::Unresolvable);
    }
    let mut url = Url::parse(trimmed)?;
    if !is_crawlable_scheme(&url) {
        return Err(UrlError::UnsupportedScheme(url.scheme().to_string()));
    }
    url.set_fragment(None);
    if url.path().is_empty() {
        url.set_path("/");
    }
    Ok(url.to_string())
}

/// Resolve a possibly-relative `href` against `base` and normalise it.
///
/// Returns an error for non-http(s) schemes and in-page fragments, which are not crawlable URLs.
pub fn resolve_url(base: &str, href: &str) -> Result<String, UrlError> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        return Err(UrlError::Unresolvable);
    }
    let lower = href.to_ascii_lowercase();
    if lower.starts_with("mailto:")
        || lower.starts_with("tel:")
        || lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("blob:")
    {
        return Err(UrlError::UnsupportedScheme(
            lower.split(':').next().unwrap_or_default().to_string(),
        ));
    }
    let base_url = Url::parse(base)?;
    let joined = base_url.join(href)?;
    if !is_crawlable_scheme(&joined) {
        return Err(UrlError::UnsupportedScheme(joined.scheme().to_string()));
    }
    normalize_url(joined.as_str())
}

/// True when both URLs share a host (case-insensitive).
pub fn same_host(a: &str, b: &str) -> bool {
    match (Url::parse(a), Url::parse(b)) {
        (Ok(a), Ok(b)) => a.host_str().map(str::to_ascii_lowercase) == b.host_str().map(str::to_ascii_lowercase),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_host_case_path_and_fragment() {
        assert_eq!(
            normalize_url("HTTPS://Example.COM#top").unwrap(),
            "https://example.com/"
        );
        assert_eq!(
            normalize_url("https://example.com/a?#frag").unwrap(),
            "https://example.com/a?"
        );
    }

    #[test]
    fn resolves_relative_links() {
        assert_eq!(
            resolve_url("https://example.com/blog/", "post-1").unwrap(),
            "https://example.com/blog/post-1"
        );
        assert_eq!(
            resolve_url("https://example.com/blog/", "../about").unwrap(),
            "https://example.com/about"
        );
        assert_eq!(
            resolve_url("https://example.com/", "/contact?x=1#form").unwrap(),
            "https://example.com/contact?x=1"
        );
    }

    #[test]
    fn rejects_non_crawlable_links() {
        assert!(resolve_url("https://example.com/", "mailto:a@b.c").is_err());
        assert!(resolve_url("https://example.com/", "#section").is_err());
        assert!(resolve_url("https://example.com/", "javascript:void(0)").is_err());
        assert!(normalize_url("ftp://example.com/file").is_err());
    }

    #[test]
    fn compares_hosts() {
        assert!(same_host("https://example.com/a", "http://EXAMPLE.com/b"));
        assert!(!same_host("https://example.com/a", "https://www.example.com/b"));
    }
}
