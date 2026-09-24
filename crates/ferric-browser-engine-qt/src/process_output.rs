//! Safe, bounded presentation of output emitted by external processes.
//!
//! External editors, printer commands, and userscripts are all untrusted from
//! the browser's perspective. This module provides their shared redaction
//! boundary without giving any of those features ownership of the policy.

use super::safe_ipc_url;
use super::url_safety::sensitive_ipc_query_key;

pub(super) fn sanitize_process_stderr(bytes: &[u8]) -> String {
    const MAX_STDERR_BYTES: usize = 4 * 1024;
    let mut text = String::from_utf8_lossy(bytes)
        .chars()
        .map(|character| {
            if character.is_control() && !matches!(character, '\n' | '\r' | '\t') {
                char::REPLACEMENT_CHARACTER
            } else {
                character
            }
        })
        .collect::<String>();
    if text.len() > MAX_STDERR_BYTES {
        text.truncate(MAX_STDERR_BYTES);
        text.push('…');
    }
    text.split_whitespace()
        .map(redact_process_stderr_token)
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn redact_process_stderr_token(token: &str) -> String {
    if token.starts_with("http://") || token.starts_with("https://") {
        return safe_ipc_url(token);
    }
    let mut parts = token.splitn(2, '=');
    let Some(key) = parts.next() else {
        return token.into();
    };
    let Some(value) = parts.next() else {
        return safe_ipc_url(token);
    };
    if sensitive_ipc_query_key(key) {
        return format!("{key}=[redacted]");
    }
    format!("{key}={}", safe_ipc_url(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_tokens_without_preserving_controls_or_urls_with_secrets() {
        assert_eq!(
            sanitize_process_stderr(
                b"refresh_token=secret https://user:password@example.test/?keep=1&token=no"
            ),
            "refresh_token=[redacted] https://example.test/?keep=1"
        );
        assert_eq!(sanitize_process_stderr(b"ok\x07"), "ok�");
        assert_eq!(
            sanitize_process_stderr(b"source=https://user:password@example.test/?token=no"),
            "source=https://example.test/"
        );
    }
}
