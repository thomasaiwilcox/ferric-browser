//! Bounded, single-line diagnostics. This limits error presentation, never
//! accepted configuration data. Do not format a full TOML excerpt first.
use std::{fmt, path::Path};

use crate::ConfigError;

pub const MAX_CONFIGURATION_DIAGNOSTIC_BYTES: usize = 1024;
const MAX_DIAGNOSTIC_CONTEXT_BYTES: usize = 256;
const MARKER: &str = "...";

struct Diagnostic {
    limit: usize,
    text: String,
    marker_boundary: usize,
    truncated: bool,
}

impl Diagnostic {
    fn append(&mut self, part: &str) -> fmt::Result {
        if self.text.len() + part.len() > self.limit {
            self.truncated = true;
            return Err(fmt::Error);
        }
        self.text.push_str(part);
        if self.text.len() <= self.limit - MARKER.len() {
            // A complete UTF-8 character or control escape. Truncation never
            // cuts an escape sequence or a code point to make room for MARKER.
            self.marker_boundary = self.text.len();
        }
        Ok(())
    }
}

impl fmt::Write for Diagnostic {
    fn write_str(&mut self, mut text: &str) -> fmt::Result {
        while let Some(character) = text.chars().next() {
            // Stored diagnostics and bounded paths have already escaped their
            // controls. Preserve complete escapes when wrapping them again.
            if let Some(length) = escape_length(text) {
                self.append(&text[..length])?;
                text = &text[length..];
                continue;
            }
            if character.is_control() || matches!(character, '\u{2028}' | '\u{2029}') {
                self.append(&character.escape_debug().to_string())?;
            } else {
                self.append(character.encode_utf8(&mut [0; 4]))?;
            }
            text = &text[character.len_utf8()..];
        }
        Ok(())
    }
}

pub(crate) fn bounded_diagnostic(arguments: fmt::Arguments<'_>) -> String {
    bounded_with_limit(arguments, MAX_CONFIGURATION_DIAGNOSTIC_BYTES)
}

pub(crate) fn bounded_path(path: &Path) -> String {
    // Preserve room for the reason/span; retain the full path in ConfigError.
    bounded_context(format_args!("{}", path.display()))
}

pub(crate) fn bounded_context(arguments: fmt::Arguments<'_>) -> String {
    bounded_with_limit(arguments, MAX_DIAGNOSTIC_CONTEXT_BYTES)
}

fn bounded_with_limit(arguments: fmt::Arguments<'_>, limit: usize) -> String {
    let mut diagnostic = Diagnostic {
        limit,
        text: String::with_capacity(limit),
        marker_boundary: 0,
        truncated: false,
    };
    if fmt::write(&mut diagnostic, arguments).is_err() {
        diagnostic.truncated = true;
    }
    if diagnostic.truncated {
        diagnostic.text.truncate(diagnostic.marker_boundary);
        diagnostic.text.push_str(MARKER);
    }
    diagnostic.text
}

fn escape_length(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if *bytes.first()? != b'\\' {
        return None;
    }
    match *bytes.get(1)? {
        // Only control escapes are produced by this formatter. Literal
        // backslashes/quotes stay literal; pairing adjacent backslashes would
        // steal the beginning of a following generated control escape.
        b'n' | b'r' | b't' | b'0' => Some(2),
        b'u' if bytes.get(2) == Some(&b'{') => {
            for index in 3..=9 {
                let byte = *bytes.get(index)?;
                if byte == b'}' {
                    return (index > 3).then_some(index + 1);
                }
                if !byte.is_ascii_hexdigit() {
                    return None;
                }
            }
            None
        }
        b'x' if bytes.get(2)?.is_ascii_hexdigit() && bytes.get(3)?.is_ascii_hexdigit() => Some(4),
        _ => None,
    }
}

impl ConfigError {
    pub(crate) fn from_toml(path: &Path, mut error: toml::de::Error) -> Self {
        let span = error.span();
        // Retain the parser's message and field path, without its potentially
        // enormous source-line/caret rendering. Do this before formatting.
        error.set_input(None);
        let message = if let Some(span) = span {
            bounded_diagnostic(format_args!(
                "at bytes {}..{}: {error}",
                span.start, span.end
            ))
        } else {
            bounded_diagnostic(format_args!("{error}"))
        };
        Self::Parse {
            path: path.to_owned(),
            message,
        }
    }

    /// Constructs a bounded single-line validation diagnostic without first
    /// allocating a complete rendered message or untrusted source excerpt.
    #[must_use]
    pub fn validation(arguments: fmt::Arguments<'_>) -> Self {
        Self::Validation(bounded_diagnostic(arguments))
    }
}

impl fmt::Debug for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Io { .. } => "Io",
            Self::Parse { .. } => "Parse",
            Self::IncludeDepth { .. } => "IncludeDepth",
            Self::IncludeCycle { .. } => "IncludeCycle",
            Self::IncludeRepeated { .. } => "IncludeRepeated",
            Self::SizeLimit => "SizeLimit",
            Self::Validation(_) => "Validation",
        };
        formatter.write_str(&bounded_diagnostic(format_args!(
            "ConfigError::{kind}: {self}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_utf8_and_escape_boundaries_preserve_complete_parts() {
        for length in [1023, 1024, 1025] {
            let text = "a".repeat(length);
            let rendered = bounded_diagnostic(format_args!("{text}"));
            assert!(rendered.len() <= MAX_CONFIGURATION_DIAGNOSTIC_BYTES);
            assert_eq!(
                rendered == text,
                length <= MAX_CONFIGURATION_DIAGNOSTIC_BYTES
            );
            assert_eq!(
                rendered.ends_with(MARKER),
                length > MAX_CONFIGURATION_DIAGNOSTIC_BYTES
            );
        }
        let unicode = "é".repeat(513);
        assert_eq!(
            bounded_diagnostic(format_args!("{unicode}")),
            format!("{}...", "é".repeat(510))
        );
        let escape = format!("{}\u{1b}xyz", "a".repeat(1016));
        assert_eq!(
            bounded_diagnostic(format_args!("{escape}")),
            format!("{}...", "a".repeat(1016))
        );
    }

    #[test]
    fn overflowing_first_argument_stops_later_formatting() {
        struct Never;
        impl fmt::Display for Never {
            fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
                panic!("formatting must stop at its budget")
            }
        }
        let output = bounded_diagnostic(format_args!("{}{}", "x".repeat(1025), Never));
        assert_eq!(output.len(), MAX_CONFIGURATION_DIAGNOSTIC_BYTES);
    }

    #[test]
    fn regression_nested_rendering_keeps_already_escaped_control_atoms_whole() {
        let source =
            <toml::de::Error as serde::de::Error>::custom(format!("{}\u{1b}", "a".repeat(1016)));
        let error = ConfigError::from_toml(Path::new("x"), source);
        assert_eq!(error.to_string(), format!("x: {}...", "a".repeat(1016)));
    }

    #[test]
    fn regression_literal_backslashes_do_not_steal_a_following_control_escape() {
        for count in [1, 2, 3] {
            let prefix = format!("{}{}", "a".repeat(1016 - count), "\\".repeat(count));
            let source = <toml::de::Error as serde::de::Error>::custom(format!("{prefix}\u{1b}"));
            let error = ConfigError::from_toml(Path::new("x"), source);
            assert_eq!(error.to_string(), format!("x: {prefix}..."));
        }
    }

    #[test]
    fn regression_unicode_line_and_paragraph_separators_are_escaped() {
        let display = ConfigError::Validation("one\u{2028}two\u{2029}three".into()).to_string();
        assert!(!display.contains(['\u{2028}', '\u{2029}']));
        assert!(display.contains("\\u{2028}"));
        assert!(display.contains("\\u{2029}"));
    }

    #[test]
    fn regression_long_source_path_cannot_hide_the_failure_reason() {
        let error = ConfigError::Parse {
            path: std::path::PathBuf::from("x".repeat(2000)),
            message: "at bytes 7..8: invalid predicate".into(),
        };
        let display = error.to_string();
        assert!(display.len() <= MAX_CONFIGURATION_DIAGNOSTIC_BYTES);
        assert!(display.contains("at bytes 7..8: invalid predicate"));
        let source: toml::Value = toml::from_str("[[site_rules]]\nid='valid'\npattern='https://example.test/*'\n[site_rules.set]\nunsupported=true\n").unwrap();
        let path = std::path::PathBuf::from("x".repeat(2000));
        let error = crate::ConfigurationSourceDocument::parse(&path, &source).unwrap_err();
        assert!(error.to_string().contains("unsupported setting"));
    }
}
