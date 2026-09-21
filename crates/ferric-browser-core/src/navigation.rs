use std::{collections::BTreeMap, fmt, path::Path};

use crate::{UrlError, ValidatedUrl};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NavigationSource {
    Quickmark,
    SearchKeyword,
    ExplicitUrl,
    LocalPath,
    Host,
    Search,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationResult {
    pub requested: String,
    pub url: ValidatedUrl,
    pub source: NavigationSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NavigationError {
    Empty,
    InvalidUrl(UrlError),
    ExternalSchemeRequiresConfirmation { scheme: String, url: String },
    InvalidSearchTemplate,
}

impl fmt::Display for NavigationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("navigation input is empty"),
            Self::InvalidUrl(error) => write!(formatter, "invalid navigation URL: {error}"),
            Self::ExternalSchemeRequiresConfirmation { scheme, url } => write!(
                formatter,
                "external URI requires confirmation ({scheme}): {url}"
            ),
            Self::InvalidSearchTemplate => {
                formatter.write_str("search keyword template is invalid")
            }
        }
    }
}

impl std::error::Error for NavigationError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NavigationContext {
    pub quickmarks: BTreeMap<String, String>,
    pub search_keywords: BTreeMap<String, String>,
    /// The configured fallback search template, using either `{}` or
    /// `{query}` as its single data-only placeholder.
    pub default_search_template: Option<String>,
    pub trusted_local_input: bool,
}

/// Resolves an input without probing the network or performing DNS lookups.
///
/// Resolution follows NAV-001's deterministic order: quickmark, keyword,
/// explicit supported URI, trusted local path, plausible host, then the
/// configured search endpoint. Search input is percent-encoded as a query
/// value, never interpolated as executable template syntax.
///
/// # Errors
///
/// Returns a typed error for empty input, malformed explicit URLs, or invalid
/// keyword templates.
pub fn resolve_input(
    input: &str,
    context: &NavigationContext,
) -> Result<NavigationResult, NavigationError> {
    let requested = input.trim().to_owned();
    if requested.is_empty() {
        return Err(NavigationError::Empty);
    }

    if let Some(value) = context.quickmarks.get(&requested) {
        return result(&requested, value, NavigationSource::Quickmark);
    }

    if let Some((keyword, query)) = requested.split_once(' ')
        && let Some(template) = context.search_keywords.get(keyword)
    {
        let value = apply_search_template(template, query.trim())?;
        return result(&requested, &value, NavigationSource::SearchKeyword);
    }

    if has_explicit_scheme(&requested) {
        if let Some(scheme) = external_scheme(&requested) {
            return Err(NavigationError::ExternalSchemeRequiresConfirmation {
                scheme,
                url: requested,
            });
        }
        return result(&requested, &requested, NavigationSource::ExplicitUrl);
    }

    if context.trusted_local_input && is_local_path(&requested) {
        let path = Path::new(&requested);
        let value = format!("file://{}", path.to_string_lossy());
        return result(&requested, &value, NavigationSource::LocalPath);
    }

    if is_plausible_host(&requested) {
        let value = format!("https://{requested}");
        return result(&requested, &value, NavigationSource::Host);
    }

    let template = context
        .default_search_template
        .as_deref()
        .unwrap_or("https://duckduckgo.com/?q={}");
    let value = apply_search_template(template, &requested)?;
    result(&requested, &value, NavigationSource::Search)
}

fn result(
    requested: &str,
    value: &str,
    source: NavigationSource,
) -> Result<NavigationResult, NavigationError> {
    Ok(NavigationResult {
        requested: requested.into(),
        url: ValidatedUrl::parse(value).map_err(NavigationError::InvalidUrl)?,
        source,
    })
}

fn apply_search_template(template: &str, query: &str) -> Result<String, NavigationError> {
    let normalized = if template.matches("{}").count() == 1 && !template.contains("{ ") {
        template.to_owned()
    } else if template.matches("{query}").count() == 1
        && !template.replace("{query}", "").contains(['{', '}'])
    {
        template.replace("{query}", "{}")
    } else {
        return Err(NavigationError::InvalidSearchTemplate);
    };
    Ok(normalized.replace("{}", &percent_encode(query)))
}

/// Resolves a configured search-engine template with data-only query
/// interpolation.
///
/// # Errors
///
/// Returns an error when the template is not a bounded HTTPS template or the
/// interpolated value is not a valid URL.
pub fn resolve_search_query(template: &str, query: &str) -> Result<ValidatedUrl, NavigationError> {
    if template.trim().is_empty()
        || !template.starts_with("https://")
        || template.matches("{query}").count() != 1
        || template.replace("{query}", "").contains(['{', '}'])
    {
        return Err(NavigationError::InvalidSearchTemplate);
    }
    let value = template.replace("{query}", &percent_encode(query));
    ValidatedUrl::parse(value).map_err(NavigationError::InvalidUrl)
}

fn has_explicit_scheme(value: &str) -> bool {
    value.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme
                .chars()
                .all(|character| character.is_ascii_alphabetic())
    })
}

fn external_scheme(value: &str) -> Option<String> {
    let (scheme, _) = value.split_once(':')?;
    let normalized = scheme.to_ascii_lowercase();
    matches!(normalized.as_str(), "mailto" | "tel" | "sms" | "geo").then_some(normalized)
}

fn is_local_path(value: &str) -> bool {
    value.starts_with('/')
        || value == "."
        || value == ".."
        || value.starts_with("./")
        || value.starts_with("../")
}

fn is_plausible_host(value: &str) -> bool {
    let host = value.split('/').next().unwrap_or_default();
    !host.is_empty()
        && (host.contains('.')
            || host.starts_with("localhost")
            || host.starts_with("127.")
            || host.starts_with('[')
            || host.contains(':'))
        && !host.chars().any(char::is_whitespace)
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(hex(byte >> 4));
            encoded.push(hex(byte & 0x0f));
        }
    }
    encoded
}

fn hex(nibble: u8) -> char {
    char::from(b"0123456789ABCDEF"[usize::from(nibble)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_resolution_order_without_network_probes() {
        let mut context = NavigationContext {
            trusted_local_input: true,
            ..NavigationContext::default()
        };
        context
            .quickmarks
            .insert("docs".into(), "https://docs.example/".into());
        context
            .search_keywords
            .insert("w".into(), "https://search.example/?q={}".into());

        let quickmark = resolve_input("docs", &context).unwrap();
        assert_eq!(quickmark.source, NavigationSource::Quickmark);
        let keyword = resolve_input("w ferric browser", &context).unwrap();
        assert_eq!(
            keyword.url.as_str(),
            "https://search.example/?q=ferric%20browser"
        );
        assert_eq!(
            resolve_input("https://example.test/a", &context)
                .unwrap()
                .source,
            NavigationSource::ExplicitUrl
        );
        assert_eq!(
            resolve_input("/tmp/report.html", &context).unwrap().source,
            NavigationSource::LocalPath
        );
        assert_eq!(
            resolve_input("example.test", &context).unwrap().source,
            NavigationSource::Host
        );
        assert_eq!(
            resolve_input("ferric browser", &context).unwrap().source,
            NavigationSource::Search
        );
    }

    #[test]
    fn search_queries_are_data_and_templates_are_bounded() {
        let context = NavigationContext::default();
        let result = resolve_input("$() `quoted` ;;", &context).unwrap();
        assert_eq!(
            result.url.as_str(),
            "https://duckduckgo.com/?q=%24%28%29%20%60quoted%60%20%3B%3B"
        );
        let mut bad = NavigationContext::default();
        bad.search_keywords
            .insert("x".into(), "https://example.test/?q={}&x={}".into());
        assert_eq!(
            resolve_input("x query", &bad),
            Err(NavigationError::InvalidSearchTemplate)
        );
        assert_eq!(
            resolve_search_query("https://search.example/?q={query}", "a b $()")
                .expect("configured search query")
                .as_str(),
            "https://search.example/?q=a%20b%20%24%28%29"
        );
        assert_eq!(
            resolve_search_query("https://search.example/?q={query}&x={query}", "query"),
            Err(NavigationError::InvalidSearchTemplate)
        );
        let mut configured = NavigationContext {
            default_search_template: Some("https://search.example/?q={query}".into()),
            ..NavigationContext::default()
        };
        let configured_result = resolve_input("ferric browser", &configured).unwrap();
        assert_eq!(
            configured_result.url.as_str(),
            "https://search.example/?q=ferric%20browser"
        );
        configured
            .search_keywords
            .insert("w".into(), "https://search.example/?q={query}".into());
        assert_eq!(
            resolve_input("w rust", &configured).unwrap().url.as_str(),
            "https://search.example/?q=rust"
        );
    }

    #[test]
    fn external_uris_are_a_confirmation_request_not_browser_navigation() {
        for (scheme, input) in [
            ("mailto", "mailto:person@example.test"),
            ("tel", "tel:+1234"),
        ] {
            assert_eq!(
                resolve_input(input, &NavigationContext::default()),
                Err(NavigationError::ExternalSchemeRequiresConfirmation {
                    scheme: scheme.into(),
                    url: input.into(),
                })
            );
        }

        assert!(matches!(
            resolve_input("javascript:alert(1)", &NavigationContext::default()),
            Err(NavigationError::InvalidUrl(UrlError::UnsupportedScheme(scheme)))
                if scheme == "javascript"
        ));
    }
}
