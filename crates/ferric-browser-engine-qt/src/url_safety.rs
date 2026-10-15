//! Safe, display-free URL values for IPC, diagnostics, and native prompts.
//!
//! This module deliberately does not use Qt types. Callers must use these
//! values instead of exposing credentials, fragments, or common secret-bearing
//! query parameters across a presentation boundary.

use ferric_browser_core::canonical_origin;

/// Removes fragments, userinfo, and secret-bearing query parameters.
#[must_use]
pub(super) fn safe_ipc_url(url: &str) -> String {
    let without_fragment = url.split('#').next().unwrap_or_default();
    let without_userinfo = strip_url_userinfo(without_fragment);
    let Some((base, query)) = without_userinfo.split_once('?') else {
        return without_userinfo;
    };
    let safe_query = query
        .split('&')
        .filter(|part| {
            let key = part.split('=').next().unwrap_or_default();
            !sensitive_ipc_query_key(key)
        })
        .collect::<Vec<_>>();
    if safe_query.is_empty() {
        base.to_owned()
    } else {
        format!("{base}?{}", safe_query.join("&"))
    }
}

/// Returns whether a query-string key must be redacted from public output.
#[must_use]
pub(super) fn sensitive_ipc_query_key(key: &str) -> bool {
    let Some(key) = percent_decode_query_key(key) else {
        return true;
    };
    matches!(
        key.as_str(),
        "access-token"
            | "access_token"
            | "api-key"
            | "api_key"
            | "apikey"
            | "auth"
            | "authorization"
            | "bearer"
            | "client-secret"
            | "client_secret"
            | "code"
            | "credential"
            | "credentials"
            | "jwt"
            | "key"
            | "nonce"
            | "password"
            | "passwd"
            | "private-key"
            | "private_key"
            | "refresh-token"
            | "refresh_token"
            | "secret"
            | "session"
            | "sig"
            | "signature"
            | "token"
    )
}

/// Normalizes an origin before it reaches a permission presentation model.
#[must_use]
pub(super) fn safe_site_origin(url: &str) -> Option<String> {
    let origin = canonical_origin(url)?;
    ferric_browser_storage::normalize_permission_origin(&origin).ok()
}

/// Qt's client-certificate selection supplies a scheme-less `QUrl` authority.
/// Its transport is TLS; do not infer an identity from arbitrary page text.
pub(super) fn safe_client_certificate_origin(authority: &str) -> Option<String> {
    let host_and_port = authority.strip_prefix("//")?;
    if host_and_port.is_empty() || host_and_port.contains(['/', '@', '?', '#', '\\']) {
        return None;
    }
    safe_site_origin(&format!("https:{authority}"))
}

/// Extracts a display-safe host from a normalized origin.
#[must_use]
pub(super) fn safe_site_host(origin: &str) -> Option<String> {
    let authority = origin.split_once("//")?.1;
    let authority = authority.split('/').next()?;
    if let Some(host) = authority.strip_prefix('[') {
        return host.split(']').next().map(ToOwned::to_owned);
    }
    Some(authority.split(':').next()?.to_owned())
}

fn percent_decode_query_key(key: &str) -> Option<String> {
    let bytes = key.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = bytes.get(index + 1).and_then(|value| hex_value(*value))?;
            let low = bytes.get(index + 2).and_then(|value| hex_value(*value))?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded)
        .ok()
        .map(|key| key.to_ascii_lowercase())
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn strip_url_userinfo(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_owned();
    };
    let authority_start = scheme_end + 3;
    let authority_end = url[authority_start..]
        .find(['/', '?'])
        .map_or(url.len(), |offset| authority_start + offset);
    let authority = &url[authority_start..authority_end];
    let Some(at) = authority.rfind('@') else {
        return url.to_owned();
    };
    format!(
        "{}{}{}",
        &url[..authority_start],
        &authority[at + 1..],
        &url[authority_end..]
    )
}

/// Native start and blank documents are safe tab descriptors, but never visits.
pub(super) fn is_safe_tab_descriptor(url: &str) -> bool {
    ferric_browser_core::InternalPage::from_url(url).is_some()
        || ferric_browser_storage::is_safe_history_url(url)
}

/// A new native tab has no committed URL until its view is constructed.
pub(super) fn pending_native_tab_url<'a>(
    actions: impl IntoIterator<Item = &'a str>,
    index: usize,
) -> Option<&'a str> {
    actions.into_iter().find_map(|action| {
        let mut fields = action.splitn(3, '\t');
        if fields.next()? != "navigate" || fields.next()?.parse::<usize>().ok()? != index {
            return None;
        }
        let url = fields.next()?;
        ferric_browser_core::InternalPage::from_url(url).map(|_| url)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_native_tabs_use_only_their_exact_queued_descriptor() {
        let actions = [
            "navigate\t0\trb://blank",
            "navigate\t1\trb://start?token=secret",
            "navigate\t1\thttps://example.test",
            "navigate\t1\trb://start",
        ];
        assert_eq!(pending_native_tab_url(actions, 0), Some("rb://blank"));
        assert_eq!(pending_native_tab_url(actions, 1), Some("rb://start"));
        assert_eq!(pending_native_tab_url(actions, 2), None);
    }

    #[test]
    fn redacts_encoded_and_malformed_secret_keys() {
        assert_eq!(
            safe_ipc_url("https://user:password@example.test/path?keep=1&token=secret#private"),
            "https://example.test/path?keep=1"
        );
        assert_eq!(
            safe_ipc_url("https://example.test/path?access%ZZtoken=secret&keep=1"),
            "https://example.test/path?keep=1"
        );
        assert!(sensitive_ipc_query_key("%61ccess_token"));
    }

    #[test]
    fn normalizes_permission_origins_without_credentials_or_paths() {
        assert_eq!(
            safe_site_origin("HTTPS://user:password@Example.test:443/private?token=secret"),
            Some("https://example.test".into())
        );
        assert_eq!(safe_site_host("https://[::1]"), Some("::1".into()));
    }

    #[test]
    fn client_certificate_identity_requires_qt_tls_authority() {
        assert_eq!(
            safe_client_certificate_origin("//EXAMPLE.test:8443"),
            Some("https://example.test:8443".into())
        );
        assert_eq!(
            safe_client_certificate_origin("//[::1]:443"),
            Some("https://[::1]".into())
        );
        for invalid in [
            "",
            "//",
            "example.test:443",
            "https://example.test",
            "//user:secret@example.test",
            "//example.test/path",
            "//example.test?token=secret",
            "//example.test#fragment",
            "//example.test\\other",
        ] {
            assert_eq!(safe_client_certificate_origin(invalid), None, "{invalid}");
        }
    }
}
