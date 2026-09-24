//! Bounded validation for strings crossing into browser policy.

use super::MAX_UNTRUSTED_ARGUMENT_BYTES;

pub(super) const MAX_JSEVAL_SCRIPT_BYTES: usize = 64 * 1024;
pub(super) const MAX_JOURNEY_QUERY_BYTES: usize = 256;

#[must_use]
pub(super) fn is_bounded_untrusted_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_UNTRUSTED_ARGUMENT_BYTES
        && !value.chars().any(char::is_control)
}

#[must_use]
pub(super) fn is_bounded_journey_query(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_JOURNEY_QUERY_BYTES
        && !value.chars().any(char::is_control)
}

pub(super) fn validate_jseval_script(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_JSEVAL_SCRIPT_BYTES {
        return Err("jseval script must be nonempty and at most 64 KiB".into());
    }
    if value.chars().any(|character| {
        character == '\0' || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    }) {
        return Err("jseval script contains a disallowed control character".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_controls_and_oversized_scripts() {
        assert!(validate_jseval_script("function f() {\n\treturn 1\n}").is_ok());
        assert!(validate_jseval_script("1\u{0007}").is_err());
        assert!(validate_jseval_script(&"x".repeat(MAX_JSEVAL_SCRIPT_BYTES + 1)).is_err());
    }
}
