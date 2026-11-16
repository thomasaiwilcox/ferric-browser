//! Static native-delivery requirements, not qualification results. The runtime
//! must separately consult build/backend/layout evidence and prove source
//! admission, current receiver authority, focus safety and output ownership.
use ferric_browser_core::{LogicalKey, LogicalKeyToken, LogicalNamedKey};
use serde::{Serialize, Serializer, ser::SerializeStruct};

use crate::{InteractionPageReceiver, InteractionRemapContext};

/// Informational vocabulary groups. One member's native evidence never
/// qualifies another member or a modifier chord merely sharing this class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PageKeyDestinationClass {
    AsciiLower,
    AsciiUpper,
    Digits,
    PunctuationSpace,
    EnterTabBacktab,
    EditingNavigation,
    BackForward,
    #[serde(rename = "f1-f12")]
    FunctionKeys,
    ModifierChords,
}

impl PageKeyDestinationClass {
    fn classify(token: &LogicalKeyToken) -> Result<Self, &'static str> {
        let base = match token.key() {
            LogicalKey::Text(text) => match text.as_bytes() {
                [b'a'..=b'z'] => Ok(Self::AsciiLower),
                [b'A'..=b'Z'] => Ok(Self::AsciiUpper),
                [b'0'..=b'9'] => Ok(Self::Digits),
                [byte] if byte.is_ascii_punctuation() => Ok(Self::PunctuationSpace),
                _ => Err("remap destination requires printable ASCII or a supported named key"),
            },
            LogicalKey::Named(named) => match named {
                LogicalNamedKey::Space => Ok(Self::PunctuationSpace),
                LogicalNamedKey::Enter | LogicalNamedKey::Tab | LogicalNamedKey::Backtab => {
                    Ok(Self::EnterTabBacktab)
                }
                LogicalNamedKey::Backspace
                | LogicalNamedKey::Delete
                | LogicalNamedKey::Insert
                | LogicalNamedKey::Escape
                | LogicalNamedKey::Left
                | LogicalNamedKey::Right
                | LogicalNamedKey::Up
                | LogicalNamedKey::Down
                | LogicalNamedKey::Home
                | LogicalNamedKey::End
                | LogicalNamedKey::PgUp
                | LogicalNamedKey::PgDown => Ok(Self::EditingNavigation),
                LogicalNamedKey::Back | LogicalNamedKey::Forward => Ok(Self::BackForward),
                LogicalNamedKey::Function(1..=12) => Ok(Self::FunctionKeys),
                LogicalNamedKey::Function(_) => {
                    Err("remap function-key destination must be F1 through F12")
                }
            },
        }?;
        Ok(if token.modifiers().is_empty() {
            base
        } else {
            Self::ModifierChords
        })
    }
}

/// Native route shape only. It contains neither allowed origins nor a live
/// receiver association; admission uses the effective policy's full descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeReceiverScopeRequirement {
    TopDocument,
    FocusedFrame,
    TopOrFocusedFrame,
}

impl NativeReceiverScopeRequirement {
    pub(crate) fn compile(receiver: Option<&InteractionPageReceiver>) -> Result<Self, String> {
        if let Some(receiver) = receiver {
            receiver.validate()?;
        }
        Ok(match receiver {
            None | Some(InteractionPageReceiver::TopDocument {}) => Self::TopDocument,
            Some(InteractionPageReceiver::FocusedFrame { .. }) => Self::FocusedFrame,
            Some(InteractionPageReceiver::TopOrFocusedFrame { .. }) => Self::TopOrFocusedFrame,
        })
    }
}

/// Exact configured gesture/context and receiver route shape needed by native
/// qualification and admission. Logical modifiers remain on the tokens;
/// uppercase and Backtab may also require implicit destination flags, which
/// the native adapter must qualify. No scan codes or physical modifier
/// transitions are invented by this syntax compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRemapCapabilityRequirement {
    source: LogicalKeyToken,
    destination: LogicalKeyToken,
    destination_class: PageKeyDestinationClass,
    context: InteractionRemapContext,
    receiver_scope: NativeReceiverScopeRequirement,
}

impl NativeRemapCapabilityRequirement {
    pub(crate) fn compile(
        source: &LogicalKeyToken,
        destination: &LogicalKeyToken,
        context: InteractionRemapContext,
        receiver_scope: NativeReceiverScopeRequirement,
    ) -> Result<Self, String> {
        if source.is_protected_rescue() || destination.is_protected_rescue() {
            return Err("protected rescue cannot be a remap source or destination".into());
        }
        if context == InteractionRemapContext::HintPageKey && source.is_hint_grammar_control() {
            return Err("Hint-context remap source is owned by Hint grammar".into());
        }
        Ok(Self {
            source: source.clone(),
            destination: destination.clone(),
            destination_class: PageKeyDestinationClass::classify(destination)?,
            context,
            receiver_scope,
        })
    }

    #[must_use]
    pub fn source(&self) -> &LogicalKeyToken {
        &self.source
    }
    #[must_use]
    pub fn destination(&self) -> &LogicalKeyToken {
        &self.destination
    }
    #[must_use]
    pub const fn destination_class(&self) -> PageKeyDestinationClass {
        self.destination_class
    }
    #[must_use]
    pub const fn context(&self) -> InteractionRemapContext {
        self.context
    }
    /// Capability scope only. This does not prove the current receiver or its
    /// origin; equality of requirements cannot establish live ownership.
    #[must_use]
    pub const fn receiver_scope(&self) -> NativeReceiverScopeRequirement {
        self.receiver_scope
    }
}

impl Serialize for NativeRemapCapabilityRequirement {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut output = serializer.serialize_struct("NativeRemapCapabilityRequirement", 6)?;
        output.serialize_field("source", self.source.as_str())?;
        output.serialize_field("destination", self.destination.as_str())?;
        output.serialize_field("destination_class", &self.destination_class)?;
        output.serialize_field("context", &self.context)?;
        output.serialize_field("receiver_scope", &self.receiver_scope)?;
        output.serialize_field("qualification", "required")?;
        output.end()
    }
}

#[cfg(test)]
mod tests;
