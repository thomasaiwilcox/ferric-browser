//! One logical gesture language for configuration, ownership and adapters.
//! Syntax is separate from receiver authorization and native qualification.
use unicode_segmentation::UnicodeSegmentation;

pub const MAX_LOGICAL_KEY_TOKEN_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
// Four independent modifier flags have sixteen valid combinations, rather
// than mutually exclusive states. Keep their meaning explicit to adapters.
#[allow(clippy::struct_excessive_bools)]
pub struct LogicalKeyModifiers {
    ctrl: bool,
    alt: bool,
    meta: bool,
    shift: bool,
}
impl LogicalKeyModifiers {
    #[must_use]
    pub fn ctrl(self) -> bool {
        self.ctrl
    }
    #[must_use]
    pub fn alt(self) -> bool {
        self.alt
    }
    #[must_use]
    pub fn meta(self) -> bool {
        self.meta
    }
    #[must_use]
    pub fn shift(self) -> bool {
        self.shift
    }
    #[must_use]
    pub fn is_empty(self) -> bool {
        self == Self::default()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LogicalNamedKey {
    Space,
    Enter,
    Tab,
    Backtab,
    Backspace,
    Delete,
    Insert,
    Escape,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PgUp,
    PgDown,
    Back,
    Forward,
    Function(u8),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LogicalKey {
    Text(String),
    Named(LogicalNamedKey),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LogicalKeyToken {
    canonical: String,
    key: LogicalKey,
    modifiers: LogicalKeyModifiers,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalKeyTokenError {
    Empty,
    ModifierOnly,
    MultiKeySequence,
    ControlCharacter,
    TextTooLong,
    UnknownLogicalToken,
    DuplicateModifier,
    AmbiguousShift,
    TokenTooLong,
}
impl std::fmt::Display for LogicalKeyTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Empty => "key token is empty",
            Self::ModifierOnly => "modifier-only tokens are not supported",
            Self::MultiKeySequence => "page key must be one logical key gesture",
            Self::ControlCharacter => "control characters are not valid page keys",
            Self::TextTooLong => "logical key text exceeds four UTF-16 code units",
            Self::UnknownLogicalToken => {
                "unknown key token; use a supported named key or produced literal"
            }
            Self::DuplicateModifier => "key token repeats a modifier",
            Self::AmbiguousShift => {
                "use the produced punctuation/Unicode literal without a Shift+ alias"
            }
            Self::TokenTooLong => "key token exceeds 64 UTF-8 bytes",
        })
    }
}
impl std::error::Error for LogicalKeyTokenError {}

impl LogicalKeyToken {
    /// Parses syntax only. Unicode text retains the pre-existing single-grapheme
    /// and four-UTF-16-unit source bound; it does not promise Unicode output.
    ///
    /// # Errors
    /// Rejects sequences, controls, duplicate/modifier-only chords and ambiguous
    /// Shift+literal spellings. Platform delivery is qualified separately.
    pub fn parse(source: &str) -> Result<Self, LogicalKeyTokenError> {
        if source.is_empty() {
            return Err(LogicalKeyTokenError::Empty);
        }
        if source.len() > MAX_LOGICAL_KEY_TOKEN_BYTES {
            return Err(LogicalKeyTokenError::TokenTooLong);
        }
        let mut remainder = source;
        let mut modifiers = LogicalKeyModifiers::default();
        loop {
            let (flag, rest) = if let Some(rest) = remainder.strip_prefix("Ctrl+") {
                (&mut modifiers.ctrl, rest)
            } else if let Some(rest) = remainder.strip_prefix("Alt+") {
                (&mut modifiers.alt, rest)
            } else if let Some(rest) = remainder.strip_prefix("Meta+") {
                (&mut modifiers.meta, rest)
            } else if let Some(rest) = remainder.strip_prefix("Shift+") {
                (&mut modifiers.shift, rest)
            } else {
                break;
            };
            if *flag {
                return Err(LogicalKeyTokenError::DuplicateModifier);
            }
            *flag = true;
            remainder = rest;
        }
        if remainder.is_empty() || matches!(remainder, "Ctrl" | "Alt" | "Meta" | "Shift") {
            return Err(LogicalKeyTokenError::ModifierOnly);
        }
        let mut key = if let Some(named) = named_key(remainder) {
            LogicalKey::Named(named)
        } else {
            let literal = if remainder == "Plus" { "+" } else { remainder };
            if literal != "+" && literal.contains('+') {
                return Err(LogicalKeyTokenError::UnknownLogicalToken);
            }
            if literal.encode_utf16().count() > 4 {
                return Err(LogicalKeyTokenError::TextTooLong);
            }
            if literal.chars().any(|c| c < ' ' || c == '\u{7f}') {
                return Err(LogicalKeyTokenError::ControlCharacter);
            }
            if literal.graphemes(true).count() != 1 {
                return Err(LogicalKeyTokenError::MultiKeySequence);
            }
            LogicalKey::Text(literal.to_owned())
        };
        match &mut key {
            LogicalKey::Named(LogicalNamedKey::Tab | LogicalNamedKey::Backtab)
                if modifiers.shift =>
            {
                key = LogicalKey::Named(LogicalNamedKey::Backtab);
                modifiers.shift = false;
            }
            LogicalKey::Text(text) => {
                let ascii_letter = text.len() == 1 && text.as_bytes()[0].is_ascii_alphabetic();
                if ascii_letter {
                    if modifiers.ctrl || modifiers.alt || modifiers.meta {
                        modifiers.shift |= text.as_bytes()[0].is_ascii_uppercase();
                        text.make_ascii_lowercase();
                    } else if modifiers.shift {
                        text.make_ascii_uppercase();
                        modifiers.shift = false;
                    }
                } else if modifiers.shift {
                    return Err(LogicalKeyTokenError::AmbiguousShift);
                }
            }
            LogicalKey::Named(_) => {}
        }
        let mut canonical = String::new();
        for (present, spelling) in [
            (modifiers.ctrl, "Ctrl+"),
            (modifiers.alt, "Alt+"),
            (modifiers.meta, "Meta+"),
            (modifiers.shift, "Shift+"),
        ] {
            if present {
                canonical.push_str(spelling);
            }
        }
        match &key {
            LogicalKey::Text(text) => canonical.push_str(if text == "+" { "Plus" } else { text }),
            LogicalKey::Named(named) => canonical.push_str(&named.spelling()),
        }
        Ok(Self {
            canonical,
            key,
            modifiers,
        })
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.canonical
    }
    #[must_use]
    pub fn key(&self) -> &LogicalKey {
        &self.key
    }
    #[must_use]
    pub fn modifiers(&self) -> LogicalKeyModifiers {
        self.modifiers
    }
    #[must_use]
    pub fn is_protected_rescue(&self) -> bool {
        self.key == LogicalKey::Named(LogicalNamedKey::Escape)
            && self.modifiers.ctrl
            && !self.modifiers.alt
            && !self.modifiers.meta
    }
    /// Controls owned by the existing Hint label/loading grammar. Query,
    /// editor, palette and Pointer substates retain separate ownership guards.
    #[must_use]
    pub fn is_hint_grammar_control(&self) -> bool {
        match &self.key {
            LogicalKey::Named(LogicalNamedKey::Space) => {
                self.modifiers.ctrl || (!self.modifiers.alt && !self.modifiers.meta)
            }
            LogicalKey::Named(
                LogicalNamedKey::Escape
                | LogicalNamedKey::Enter
                | LogicalNamedKey::Tab
                | LogicalNamedKey::Backtab
                | LogicalNamedKey::Backspace
                | LogicalNamedKey::Left
                | LogicalNamedKey::Right
                | LogicalNamedKey::Up
                | LogicalNamedKey::Down,
            ) => true,
            LogicalKey::Text(text) => {
                (text == "/" && self.modifiers.is_empty()) || (text == "u" && self.modifiers.ctrl)
            }
            LogicalKey::Named(_) => false,
        }
    }
    /// Single produced character, for source-side Hint label reservation.
    #[must_use]
    pub fn character(&self) -> Option<char> {
        if !self.modifiers.is_empty() {
            return None;
        }
        match &self.key {
            LogicalKey::Named(LogicalNamedKey::Space) => Some(' '),
            LogicalKey::Text(text) => {
                let mut chars = text.chars();
                let first = chars.next()?;
                chars.next().is_none().then_some(first)
            }
            LogicalKey::Named(_) => None,
        }
    }
}

impl LogicalNamedKey {
    /// Canonical name shared by presentation and native event adapters.
    #[must_use]
    pub fn spelling(self) -> String {
        if let Self::Function(number) = self {
            return format!("F{number}");
        }
        match self {
            Self::Space => "Space",
            Self::Enter => "Enter",
            Self::Tab => "Tab",
            Self::Backtab => "Backtab",
            Self::Backspace => "Backspace",
            Self::Delete => "Delete",
            Self::Insert => "Insert",
            Self::Escape => "Escape",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Up => "Up",
            Self::Down => "Down",
            Self::Home => "Home",
            Self::End => "End",
            Self::PgUp => "PgUp",
            Self::PgDown => "PgDown",
            Self::Back => "Back",
            Self::Forward => "Forward",
            Self::Function(_) => unreachable!(),
        }
        .to_owned()
    }
}
fn named_key(value: &str) -> Option<LogicalNamedKey> {
    Some(match value {
        " " | "Space" => LogicalNamedKey::Space,
        "Return" | "Enter" => LogicalNamedKey::Enter,
        "Tab" => LogicalNamedKey::Tab,
        "Backtab" => LogicalNamedKey::Backtab,
        "Backspace" => LogicalNamedKey::Backspace,
        "Delete" => LogicalNamedKey::Delete,
        "Insert" => LogicalNamedKey::Insert,
        "Escape" => LogicalNamedKey::Escape,
        "Left" => LogicalNamedKey::Left,
        "Right" => LogicalNamedKey::Right,
        "Up" => LogicalNamedKey::Up,
        "Down" => LogicalNamedKey::Down,
        "Home" => LogicalNamedKey::Home,
        "End" => LogicalNamedKey::End,
        "PageUp" | "PgUp" => LogicalNamedKey::PgUp,
        "PageDown" | "PgDown" => LogicalNamedKey::PgDown,
        "Back" => LogicalNamedKey::Back,
        "Forward" => LogicalNamedKey::Forward,
        _ => {
            let suffix = value.strip_prefix('F')?;
            let number = suffix.parse::<u8>().ok()?;
            if !(1..=12).contains(&number) || suffix != number.to_string() {
                return None;
            }
            LogicalNamedKey::Function(number)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_aliases_are_idempotent_and_keep_typed_modifiers() {
        for (source, expected) in [
            ("Shift+p", "P"),
            ("Ctrl+P", "Ctrl+Shift+p"),
            ("Shift+Ctrl+p", "Ctrl+Shift+p"),
            ("Shift+Tab", "Backtab"),
            ("Ctrl+Shift+Tab", "Ctrl+Backtab"),
            ("Shift+Backtab", "Backtab"),
            ("Return", "Enter"),
            ("Shift+Return", "Shift+Enter"),
            ("PageUp", "PgUp"),
            ("PageDown", "PgDown"),
            (" ", "Space"),
            ("+", "Plus"),
            ("Ctrl++", "Ctrl+Plus"),
            ("Ctrl+^", "Ctrl+^"),
            ("Meta+Alt+Ctrl+p", "Ctrl+Alt+Meta+p"),
        ] {
            let token = LogicalKeyToken::parse(source).unwrap();
            assert_eq!(token.as_str(), expected);
            assert_eq!(LogicalKeyToken::parse(expected).unwrap(), token);
        }
        assert_ne!(
            LogicalKeyToken::parse("Ctrl+p").unwrap(),
            LogicalKeyToken::parse("Ctrl+P").unwrap()
        );
        assert!(
            LogicalKeyToken::parse("Shift+Ctrl+Escape")
                .unwrap()
                .is_protected_rescue()
        );
        assert_eq!(LogicalKeyToken::parse("Ctrl+k").unwrap().character(), None);
        assert_eq!(LogicalKeyToken::parse("k").unwrap().character(), Some('k'));
        assert_eq!(
            LogicalKeyToken::parse("Shift+p").unwrap().character(),
            Some('P')
        );
    }
    #[test]
    fn invalid_tokens_and_unicode_source_bounds_are_explicit() {
        for source in [
            "Ctrl",
            "Ctrl+",
            "Ctrl+Ctrl+p",
            "AltGr+p",
            "Shift+1",
            "Shift+é",
            "Ctrl+Shift+^",
            "gg",
            "F13",
            "F01",
            "\t",
            "p q",
        ] {
            assert!(LogicalKeyToken::parse(source).is_err(), "{source}");
        }
        for source in [
            "😀",
            "e\u{301}",
            "É",
            "Ctrl+é",
            "F1",
            "F12",
            "Insert",
            "Shift+Enter",
            "Ctrl+Shift+Space",
        ] {
            assert!(LogicalKeyToken::parse(source).is_ok(), "{source}");
        }
    }
}
