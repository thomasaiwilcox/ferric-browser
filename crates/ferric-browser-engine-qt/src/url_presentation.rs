//! URL conversion and safe presentation at the Qt boundary.
//!
//! This module is deliberately the sole place where a `WebEngine` `QString` is
//! canonicalized alongside Rust URL policy and where untrusted URLs become
//! display or IPC-preview data.

use crate::{presentation_text::sanitize_display_text, qobject, url_safety::safe_ipc_url};
use cxx_qt_lib::QString;
use ferric_browser_core::{CleanLinkResult, CleaningRules, ValidatedUrl, canonical_origin};
use serde_json::{Value, json};

pub(super) fn canonical_engine_url(url: QString) -> Result<String, String> {
    let raw_url = url.to_string();
    let rust_raw_url = ValidatedUrl::parse(raw_url)
        .map(|url| url.to_string())
        .map_err(|error| format!("Rust rejected the engine URL: {error}"))?;
    let qt_url = qobject::ferric_browser_canonicalize_url(&url).to_string();
    if qt_url.is_empty() {
        return Err("Qt rejected the engine URL".into());
    }
    drop(url);
    let rust_qt_url = ValidatedUrl::parse(qt_url)
        .map(|url| url.to_string())
        .map_err(|error| format!("Qt/Rust URL canonicalization failed: {error}"))?;
    let rust_origin = canonical_origin(&rust_raw_url);
    let qt_origin = canonical_origin(&rust_qt_url);
    if rust_origin != qt_origin {
        let describe = |origin: Option<String>| origin.unwrap_or_else(|| "opaque".into());
        return Err(format!(
            "URL normalization mismatch; Rust origin={} Qt origin={}",
            describe(rust_origin),
            describe(qt_origin)
        ));
    }
    Ok(rust_qt_url)
}

fn display_authority(authority: &str) -> String {
    let (prefix, host, suffix) = if let Some(value) = authority.strip_prefix('[')
        && let Some(close) = value.find(']')
    {
        ("[", &value[..close], format!("]{}", &value[close + 1..]))
    } else if let Some((host, port)) = authority.rsplit_once(':')
        && !port.is_empty()
        && port.chars().all(|character| character.is_ascii_digit())
    {
        ("", host, format!(":{port}"))
    } else {
        ("", authority, String::new())
    };
    let ascii_host = qobject::ferric_browser_to_ascii_host(&QString::from(host)).to_string();
    format!(
        "{}{}{}",
        sanitize_display_text(prefix, false),
        sanitize_display_text(&ascii_host, true),
        sanitize_display_text(&suffix, false)
    )
}

#[must_use]
pub(super) fn display_url(url: &str) -> String {
    let safe = safe_ipc_url(url);
    let Some(scheme_end) = safe.find("://") else {
        return sanitize_display_text(&safe, false);
    };
    let authority_start = scheme_end + 3;
    let authority_end = safe[authority_start..]
        .find(['/', '?'])
        .map_or(safe.len(), |offset| authority_start + offset);
    let prefix = format!(
        "{}{}",
        sanitize_display_text(&safe[..authority_start], false),
        display_authority(&safe[authority_start..authority_end])
    );
    let tail = sanitize_display_text(&safe[authority_end..], false);
    if tail.is_empty() {
        prefix
    } else {
        format!("{prefix}\u{2068}{tail}\u{2069}")
    }
}

fn query_parameter_names(url: &str) -> Vec<String> {
    let Some((_, query_and_fragment)) = url.split_once('?') else {
        return Vec::new();
    };
    let query = query_and_fragment.split('#').next().unwrap_or_default();
    query
        .split('&')
        .filter_map(|component| {
            let name = component
                .split_once('=')
                .map_or(component, |(name, _)| name);
            (!name.is_empty()).then(|| name.to_owned())
        })
        .collect()
}

#[must_use]
pub(super) fn link_result_value(
    command: &str,
    result: &CleanLinkResult,
    rules: &CleaningRules,
) -> Value {
    let presentation = link_preview_presentation(command, result, false);
    json!({
        "status": "preview",
        "command": command,
        "revision": rules.revision,
        "source": rules.source,
        "changed": result.changed,
        "original": presentation.original,
        "cleaned": presentation.cleaned,
        "applied_rules": presentation.applied_rules,
        "removed_parameters": presentation.removed_parameters,
        "retained_parameters": presentation.retained_parameters,
        "explanation": presentation.explanation,
    })
}

/// Typed presentation fields for the clean-link dialog.  IPC uses a JSON
/// envelope, while QML consumes these bounded scalar and list values directly.
pub(super) struct LinkPreviewPresentation {
    pub(super) command: String,
    pub(super) original: String,
    pub(super) cleaned: String,
    pub(super) applied_rules: Vec<String>,
    pub(super) removed_parameters: Vec<String>,
    pub(super) retained_parameters: Vec<String>,
    pub(super) explanation: String,
    pub(super) requires_confirmation: bool,
}

#[must_use]
pub(super) fn link_preview_presentation(
    command: &str,
    result: &CleanLinkResult,
    requires_confirmation: bool,
) -> LinkPreviewPresentation {
    let retained_parameters = if result.changed {
        result.retained_parameters.clone()
    } else {
        query_parameter_names(&result.original)
    };
    LinkPreviewPresentation {
        command: command.to_owned(),
        original: safe_ipc_url(&result.original),
        cleaned: safe_ipc_url(&result.cleaned),
        applied_rules: result.applied_rules.clone(),
        removed_parameters: result.removed_parameters.clone(),
        retained_parameters,
        explanation: result.explanation.clone(),
        requires_confirmation,
    }
}

#[must_use]
pub(super) fn blocking_site_host(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, bracketed_ipv6) = if authority.starts_with('[') {
        let close = authority.find(']')?;
        let suffix = &authority[close + 1..];
        if !suffix.is_empty() {
            let port = suffix.strip_prefix(':')?;
            if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
        }
        (&authority[1..close], true)
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
                (host, false)
            }
            Some((_, _)) => return None,
            None => (authority, false),
        }
    };
    if host.is_empty()
        || host.len() > 253
        || host.contains("..")
        || host.chars().any(char::is_control)
        || (!bracketed_ipv6 && host.contains(':'))
        || (!host.contains(':')
            && !host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-'))
    {
        return None;
    }
    Some(host.trim_end_matches('.').to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_names_stop_at_fragments_and_skip_empty_items() {
        assert_eq!(
            query_parameter_names("https://example.test/?one=1&&two#fragment?ignored"),
            ["one", "two"]
        );
        assert!(query_parameter_names("https://example.test/").is_empty());
    }

    #[test]
    fn link_preview_projection_matches_the_ipc_safe_parameter_view() {
        let result = CleanLinkResult {
            original: "https://example.test/?keep=1&secret=2#fragment".into(),
            cleaned: "https://example.test/?keep=1#fragment".into(),
            changed: false,
            applied_rules: vec![],
            removed_parameters: vec![],
            retained_parameters: vec![],
            explanation: "No cleanup was necessary".into(),
        };

        let presentation = link_preview_presentation("url-explain", &result, false);

        assert_eq!(presentation.command, "url-explain");
        assert_eq!(presentation.retained_parameters, ["keep", "secret"]);
        assert!(!presentation.requires_confirmation);
        assert_eq!(presentation.original, "https://example.test/?keep=1");
    }

    #[test]
    fn blocking_hosts_reject_unsafe_authorities() {
        assert_eq!(
            blocking_site_host("HTTPS://Example.test:443/path"),
            Some("example.test".into())
        );
        assert_eq!(blocking_site_host("http://[::1]:8080/"), Some("::1".into()));
        assert!(blocking_site_host("https://user@example.test/").is_none());
        assert!(blocking_site_host("https://example.test:bad/").is_none());
    }
}
