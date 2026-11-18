//! Bounded Interaction selector grammar, separate from legacy general CSS.
//! Compounds retain their CSS spelling for browser matching; ancestry hops are
//! explicit so a collector can enforce root boundaries and charge each step.
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionSelectorRelation {
    Descendant,
    Child,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionSelectorAttributeOperator {
    Present,
    Equals,
    Prefix,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InteractionSelectorTest {
    Type {
        name: String,
    },
    Id {
        value: String,
    },
    Class {
        value: String,
    },
    Attribute {
        name: String,
        operator: InteractionSelectorAttributeOperator,
        value: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InteractionSelectorStep {
    pub relation: Option<InteractionSelectorRelation>,
    pub compound: String,
    pub tests: Vec<InteractionSelectorTest>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InteractionSelector {
    steps: Vec<InteractionSelectorStep>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InteractionSelectorError {
    pub offset: usize,
    pub reason: &'static str,
}
impl std::fmt::Display for InteractionSelectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Interaction selector at byte {}: {}",
            self.offset, self.reason
        )
    }
}
impl std::error::Error for InteractionSelectorError {}

impl InteractionSelector {
    pub fn parse(source: &str) -> Result<Self, InteractionSelectorError> {
        if source.is_empty() || source.len() > crate::MAX_INTERACTION_SELECTOR_BYTES {
            return Err(InteractionSelectorError {
                offset: 0,
                reason: "expected 1..=1024 UTF-8 bytes",
            });
        }
        if let Some(offset) = source.find('\0') {
            return Err(InteractionSelectorError {
                offset,
                reason: "literal NUL is not supported",
            });
        }
        let mut parser = Parser {
            source,
            position: 0,
        };
        parser.space();
        let mut steps = vec![parser.compound(None)?];
        loop {
            let spaced = parser.space();
            if parser.peek().is_none() {
                break;
            }
            let relation = if parser.peek() == Some('>') {
                parser.bump();
                parser.space();
                InteractionSelectorRelation::Child
            } else if spaced {
                InteractionSelectorRelation::Descendant
            } else {
                return parser.fail("only descendant and direct-child combinators are supported");
            };
            steps.push(parser.compound(Some(relation))?);
        }
        Ok(Self { steps })
    }
    #[must_use]
    pub fn steps(&self) -> &[InteractionSelectorStep] {
        &self.steps
    }
}

struct Parser<'a> {
    source: &'a str,
    position: usize,
}
impl Parser<'_> {
    fn fail<T>(&self, reason: &'static str) -> Result<T, InteractionSelectorError> {
        Err(InteractionSelectorError {
            offset: self.position,
            reason,
        })
    }
    fn peek(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.position += c.len_utf8();
        Some(c)
    }
    fn space(&mut self) -> bool {
        let start = self.position;
        while self.peek().is_some_and(css_space) {
            self.bump();
        }
        self.position != start
    }
    fn compound(
        &mut self,
        relation: Option<InteractionSelectorRelation>,
    ) -> Result<InteractionSelectorStep, InteractionSelectorError> {
        let start = self.position;
        let mut tests = Vec::new();
        if self.ident_start() {
            tests.push(InteractionSelectorTest::Type {
                name: self.ident()?,
            });
        }
        loop {
            match self.peek() {
                Some('#') => {
                    self.bump();
                    tests.push(InteractionSelectorTest::Id {
                        value: self.ident()?,
                    });
                }
                Some('.') => {
                    self.bump();
                    tests.push(InteractionSelectorTest::Class {
                        value: self.ident()?,
                    });
                }
                Some('[') => {
                    tests.push(self.attribute()?);
                }
                _ => break,
            }
        }
        if tests.is_empty() {
            return self.fail("expected a type, ID, class or attribute selector");
        }
        Ok(InteractionSelectorStep {
            relation,
            compound: self.source[start..self.position].to_owned(),
            tests,
        })
    }
    fn ident_start(&self) -> bool {
        let mut chars = self.source[self.position..].chars();
        match chars.next() {
            Some(c) if name_start(c) => true,
            Some('\\') => chars.next().is_some_and(|c| !css_newline(c)),
            Some('-') => match chars.next() {
                Some(c) if name_start(c) || c == '-' => true,
                Some('\\') => chars.next().is_some_and(|c| !css_newline(c)),
                _ => false,
            },
            _ => false,
        }
    }
    fn ident(&mut self) -> Result<String, InteractionSelectorError> {
        if !self.ident_start() {
            return self
                .fail("expected a CSS identifier (quote attribute values starting with digits)");
        }
        let mut result = String::new();
        while let Some(c) = self.peek() {
            if name_start(c) || c.is_ascii_digit() || c == '-' {
                result.push(c);
                self.bump();
            } else if c == '\\' {
                self.bump();
                result.push(self.escape()?);
            } else {
                break;
            }
        }
        Ok(result)
    }
    fn escape(&mut self) -> Result<char, InteractionSelectorError> {
        let Some(first) = self.peek() else {
            return self.fail("unfinished CSS escape");
        };
        if css_newline(first) {
            return self.fail("newline is not an identifier escape");
        }
        if first.is_ascii_hexdigit() {
            let mut value = 0;
            for _ in 0..6 {
                let Some(digit) = self
                    .peek()
                    .and_then(|c| c.to_digit(16).filter(|_| c.is_ascii()))
                else {
                    break;
                };
                value = value * 16 + digit;
                self.bump();
            }
            if self.peek().is_some_and(css_space) {
                let whitespace = self.bump();
                if whitespace == Some('\r') && self.peek() == Some('\n') {
                    self.bump();
                }
            }
            Ok(char::from_u32(value)
                .filter(|c| *c != '\0')
                .unwrap_or('\u{fffd}'))
        } else {
            self.bump();
            Ok(first)
        }
    }
    fn string(&mut self) -> Result<String, InteractionSelectorError> {
        let quote = self.bump().expect("string starts at quote");
        let mut result = String::new();
        loop {
            match self.bump() {
                Some(c) if c == quote => return Ok(result),
                Some('\\') => {
                    if self.peek().is_some_and(css_newline) {
                        let newline = self.bump();
                        if newline == Some('\r') && self.peek() == Some('\n') {
                            self.bump();
                        }
                    } else {
                        result.push(self.escape()?);
                    }
                }
                Some(c) if css_newline(c) => {
                    return self.fail("unescaped newline in attribute string");
                }
                Some(c) => result.push(c),
                None => return self.fail("unfinished attribute string"),
            }
        }
    }
    fn attribute(&mut self) -> Result<InteractionSelectorTest, InteractionSelectorError> {
        self.bump(); // [
        self.space();
        let name = self.ident()?;
        self.space();
        let operator = match self.bump() {
            Some(']') => {
                return Ok(InteractionSelectorTest::Attribute {
                    name,
                    operator: InteractionSelectorAttributeOperator::Present,
                    value: None,
                });
            }
            Some('=') => InteractionSelectorAttributeOperator::Equals,
            Some('^') if self.bump() == Some('=') => InteractionSelectorAttributeOperator::Prefix,
            _ => {
                return self
                    .fail("only attribute presence, equality and prefix tests are supported");
            }
        };
        self.space();
        let value = match self.peek() {
            Some('\'' | '"') => self.string()?,
            _ => self.ident()?,
        };
        self.space();
        if self.bump() != Some(']') {
            return self.fail("expected closing ] without attribute flags");
        }
        Ok(InteractionSelectorTest::Attribute {
            name,
            operator,
            value: Some(value),
        })
    }
}
fn name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()
}
fn css_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}
fn css_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\x0c')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supported_grammar_has_explicit_bounded_ancestry() {
        let selector =
            InteractionSelector::parse(" nav#global a.item[data-id][href^='/watch'] > span ")
                .unwrap();
        assert_eq!(selector.steps.len(), 3);
        assert_eq!(selector.steps[0].relation, None);
        assert_eq!(
            selector.steps[1].relation,
            Some(InteractionSelectorRelation::Descendant)
        );
        assert_eq!(
            selector.steps[2].relation,
            Some(InteractionSelectorRelation::Child)
        );
        assert_eq!(
            selector.steps[1].compound,
            "a.item[data-id][href^='/watch']"
        );
        assert_eq!(selector.steps[1].tests.len(), 4);
        for source in [
            ".é[data-é='привет']",
            "#\\31 23",
            ".foo\\:bar[data\\:id=\\31 23]",
            "[data-x=\"a\\\"b\"]",
            "[data-x='a\\\nb']",
            "--custom",
            "a\n>\tb",
        ] {
            assert!(InteractionSelector::parse(source).is_ok(), "{source}");
        }
        let escaped = InteractionSelector::parse("#\\31 23").unwrap();
        assert_eq!(
            escaped.steps[0].tests,
            [InteractionSelectorTest::Id {
                value: "123".into()
            }]
        );
    }
    #[test]
    fn unsupported_grammar_and_malformed_tokens_are_rejected() {
        for source in [
            "",
            " ",
            "*",
            "a,b",
            "a:hover",
            "a:has(b)",
            "a:not(b)",
            "a + b",
            "a ~ b",
            "a >>> b",
            "a >",
            ".",
            "#",
            "[x=123]",
            "[x~=y]",
            "[x$=y]",
            "[x='a' i]",
            "[x='unfinished]",
            "[x='a\nb']",
            "a|b",
            "a/**/.x",
            "#\\",
            "#\\\n",
            "[x=]",
            "a\0b",
        ] {
            assert!(InteractionSelector::parse(source).is_err(), "{source}");
        }
        assert!(InteractionSelector::parse(&format!(".{}", "a".repeat(1023))).is_ok());
        assert!(InteractionSelector::parse(&format!(".{}", "a".repeat(1024))).is_err());
        assert!(InteractionSelector::parse(&format!(".{}", "é".repeat(511))).is_ok());
        assert!(InteractionSelector::parse(&format!(".{}", "é".repeat(512))).is_err());
    }

    #[test]
    fn escaped_codepoints_follow_css_replacement_and_whitespace_rules() {
        for source in ["#\\0", "#\\d800", "#\\110000", "#\\ffffff"] {
            let selector = InteractionSelector::parse(source).unwrap();
            assert_eq!(
                selector.steps[0].tests,
                [InteractionSelectorTest::Id {
                    value: "\u{fffd}".into()
                }]
            );
        }
        let selector = InteractionSelector::parse("#\\31\r\n23").unwrap();
        assert_eq!(
            selector.steps[0].tests,
            [InteractionSelectorTest::Id {
                value: "123".into()
            }]
        );
        assert_eq!(InteractionSelector::parse("a\0b").unwrap_err().offset, 1);
    }

    #[test]
    fn short_ascii_adversarial_inputs_never_panic_or_report_outside_input() {
        for first in 0_u8..=127 {
            for second in 0_u8..=127 {
                let source = String::from_utf8(vec![first, second]).unwrap();
                if let Err(error) = InteractionSelector::parse(&source) {
                    assert!(error.offset <= source.len());
                }
            }
        }
    }
}
