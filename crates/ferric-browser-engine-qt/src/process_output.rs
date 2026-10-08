//! Safe, bounded presentation of output emitted by external processes.
//!
//! External editors, printer commands, and userscripts are all untrusted from
//! the browser's perspective. This module provides their shared redaction
//! boundary without giving any of those features ownership of the policy.
//! Cookie and authentication header lines are suppressed as whole values;
//! paths and URI secrets in retained diagnostic prefixes use the same scrubber.

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
        let mut limit = MAX_STDERR_BYTES - '…'.len_utf8();
        while !text.is_char_boundary(limit) {
            limit -= 1;
        }
        text.truncate(limit);
        text.push('…');
    }
    let mut redacted = Vec::new();
    for line in text.lines() {
        if let Some(header) = sensitive_process_header(line) {
            redacted.push(header);
            continue;
        }
        for token in line.split_whitespace() {
            let contains_path = token_contains_local_path(token);
            redacted.push(redact_process_stderr_token(token));
            if contains_path {
                // A path may contain spaces, so suppress the rest of its line
                // after its first recognizable component.
                break;
            }
        }
    }
    redacted.join(" ")
}

fn sensitive_process_header(line: &str) -> Option<String> {
    let words = line.split_whitespace().collect::<Vec<_>>();
    for (index, word) in words.iter().enumerate() {
        let header = match word.split_once(':') {
            Some((header, _)) => header,
            None if words.get(index + 1) == Some(&":") => *word,
            None => continue,
        };
        let normalized = header
            .trim_matches(|character| {
                matches!(
                    character,
                    '[' | ']' | '(' | ')' | '{' | '}' | '<' | '>' | '"' | '\''
                )
            })
            .to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "authorization" | "proxy-authorization" | "cookie" | "set-cookie" | "x-api-key"
        ) {
            let mut safe_prefix = Vec::new();
            for token in &words[..index] {
                let contains_path = token_contains_local_path(token);
                safe_prefix.push(redact_process_stderr_token(token));
                if contains_path {
                    break;
                }
            }
            let prefix = safe_prefix.join(" ");
            return Some(if prefix.is_empty() {
                format!("{normalized}: [redacted]")
            } else {
                format!("{prefix} {normalized}: [redacted]")
            });
        }
    }
    None
}

pub(super) fn redact_process_stderr_token(token: &str) -> String {
    if token.starts_with("http://") || token.starts_with("https://") {
        return safe_ipc_url(token);
    }
    if looks_like_local_path(token) {
        return "[local-path]".into();
    }
    let mut parts = token.splitn(2, '=');
    let Some(key) = parts.next() else {
        return token.into();
    };
    let Some(value) = parts.next() else {
        return safe_ipc_url(token);
    };
    if sensitive_ipc_query_key(key)
        || matches!(
            key.to_ascii_lowercase().as_str(),
            "directory" | "file" | "filename" | "filepath" | "path"
        )
    {
        return format!("{key}=[redacted]");
    }
    if looks_like_local_path(value) {
        return format!("{key}=[local-path]");
    }
    format!("{key}={}", safe_ipc_url(value))
}

fn looks_like_local_path(value: &str) -> bool {
    let value = value.trim_matches(|character: char| {
        matches!(
            character,
            '\'' | '"' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
        )
    });
    let bytes = value.as_bytes();
    value.starts_with('/')
        || value.starts_with("\\\\")
        || value.starts_with("~/")
        || value.starts_with("~\\")
        || value.starts_with("file://")
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'))
}

fn token_contains_local_path(token: &str) -> bool {
    let Some((key, value)) = token.split_once('=') else {
        return looks_like_local_path(token);
    };
    matches!(
        key.to_ascii_lowercase().as_str(),
        "directory" | "file" | "filename" | "filepath" | "path"
    ) || looks_like_local_path(value)
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

    #[test]
    fn redacts_cookie_and_authorization_diagnostics() {
        assert_eq!(
            sanitize_process_stderr(
                b"cookie=inline-secret request headers Cookie: sid=header-secret; theme=dark\nopen /home/alice/private/archive.mhtml Cookie: sid=path-prefix-secret\ncookie : sid=spaced-header-secret\nAuthorization: Bearer bearer-secret\nSet-Cookie: sid=reply-secret; Path=/\nhttps://example.test/?%43ookie=query-secret&safe=1"
            ),
            "cookie=[redacted] request headers cookie: [redacted] open [local-path] cookie: [redacted] cookie: [redacted] authorization: [redacted] set-cookie: [redacted] https://example.test/?safe=1"
        );
    }

    #[test]
    fn redacts_unix_windows_unc_and_file_url_paths() {
        assert_eq!(
            sanitize_process_stderr(b"open /home/alice/private/archive.mhtml"),
            "open [local-path]"
        );
        assert_eq!(
            sanitize_process_stderr(b"open C:\\Users\\Alice\\secret.txt"),
            "open [local-path]"
        );
        assert_eq!(
            sanitize_process_stderr(b"open \\\\server\\share\\private.db"),
            "open [local-path]"
        );
        assert_eq!(
            sanitize_process_stderr(b"open file:///home/alice/config.toml"),
            "open [local-path]"
        );
        assert_eq!(
            sanitize_process_stderr(b"path=/tmp/private/profile"),
            "path=[redacted]"
        );
    }

    #[test]
    fn a_spaced_local_path_does_not_leave_its_suffix_in_stderr() {
        assert_eq!(
            sanitize_process_stderr(
                b"open \"/home/alice/private archive.mhtml\" was rejected\nnext safe detail"
            ),
            "open [local-path] next safe detail"
        );
    }

    #[test]
    fn stderr_byte_limit_does_not_split_utf8_or_exceed_its_bound() {
        let stderr = format!("{}🔥x", "a".repeat(4095));
        let sanitized = sanitize_process_stderr(stderr.as_bytes());
        assert!(sanitized.ends_with('…'));
        assert!(!sanitized.contains('🔥'));
        assert!(sanitized.len() <= 4 * 1024);
    }
}
