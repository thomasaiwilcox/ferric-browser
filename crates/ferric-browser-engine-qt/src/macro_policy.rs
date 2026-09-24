//! Bounded policy for repeatable commands and keyboard macros.

pub(super) const MAX_MACRO_COMMANDS: usize = 1_000;
pub(super) const MAX_MACRO_DEPTH: u8 = 8;

pub(super) fn is_repeatable_command(name: &str) -> bool {
    matches!(
        name,
        "open"
            | "back"
            | "forward"
            | "reload"
            | "stop"
            | "zoom"
            | "search"
            | "search-next"
            | "scroll"
            | "scroll-page"
            | "scroll-to"
            | "tab-open"
            | "tab-next"
            | "tab-prev"
    )
}

pub(super) fn valid_macro_register(register: &str) -> bool {
    register.len() == 1
        && register
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeatable_commands_and_registers_are_bounded() {
        assert_eq!(MAX_MACRO_COMMANDS, 1_000);
        assert_eq!(MAX_MACRO_DEPTH, 8);
        for command in [
            "open",
            "scroll",
            "scroll-page",
            "scroll-to",
            "zoom",
            "search-next",
            "tab-next",
        ] {
            assert!(is_repeatable_command(command), "{command} should repeat");
        }
        for command in [
            "paste-open",
            "permission-decision",
            "yank",
            "spawn",
            "download-open",
            "site-data-clear",
        ] {
            assert!(!is_repeatable_command(command), "{command} must not repeat");
        }
        assert!(valid_macro_register("a"));
        assert!(valid_macro_register("9"));
        assert!(!valid_macro_register(""));
        assert!(!valid_macro_register("aa"));
        assert!(!valid_macro_register("é"));
    }
}
