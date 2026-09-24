//! Bounded, safe display text for values received from browser and system APIs.

use std::fmt::Write as _;

pub(super) const MAX_PAGE_TITLE_BYTES: usize = 4 * 1024;
const MAX_NAVIGATION_FAILURE_DETAIL_BYTES: usize = 2 * 1024;

#[must_use]
pub(super) fn normalized_navigation_failure_kind(value: &str) -> &'static str {
    match value {
        "dns" => "dns",
        "network" => "network",
        "tls" => "tls",
        "http" => "http",
        "download" => "download",
        _ => "unknown",
    }
}

#[must_use]
pub(super) fn sanitize_display_text(value: &str, ascii_only: bool) -> String {
    let mut display = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(
            character,
            '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        ) {
            display.push_str("[bidi]");
        } else if character.is_control() {
            display.push('�');
        } else if ascii_only && !character.is_ascii() {
            let _ = write!(display, "\\u{{{:x}}}", character as u32);
        } else {
            display.push(character);
        }
    }
    display
}

#[must_use]
pub(super) fn bounded_navigation_failure_detail(value: &str) -> String {
    let sanitized = sanitize_display_text(value, false);
    let mut end = sanitized.len().min(MAX_NAVIGATION_FAILURE_DETAIL_BYTES);
    while end > 0 && !sanitized.is_char_boundary(end) {
        end -= 1;
    }
    sanitized[..end].to_owned()
}

#[must_use]
pub(super) fn sanitize_untrusted_title(value: &str) -> String {
    let sanitized = sanitize_display_text(value, false);
    let mut title = String::new();
    for character in sanitized.chars() {
        if title.len() + character.len_utf8() > MAX_PAGE_TITLE_BYTES {
            break;
        }
        title.push(character);
    }
    title
}

#[cfg(test)]
mod tests {
    use super::{
        bounded_navigation_failure_detail, sanitize_display_text, sanitize_untrusted_title,
    };

    #[test]
    fn sanitization_neutralizes_controls_and_directional_isolates() {
        assert_eq!(
            sanitize_display_text("a\u{202e}b\u{0007}", false),
            "a[bidi]b�"
        );
        assert_eq!(sanitize_display_text("é", true), "\\u{e9}");
    }

    #[test]
    fn bounds_preserve_utf8_boundaries() {
        assert!(bounded_navigation_failure_detail(&"é".repeat(2_000)).is_char_boundary(0));
        assert!(sanitize_untrusted_title(&"é".repeat(4_000)).len() <= 4 * 1024);
    }
}
