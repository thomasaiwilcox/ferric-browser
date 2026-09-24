//! Read-only, bounded presentation of in-memory macro state.

use crate::{
    BrowserUiRust,
    macro_policy::{MAX_MACRO_COMMANDS, MAX_MACRO_DEPTH},
};
use serde_json::{Value, json};

pub(super) fn state_value(rust: &BrowserUiRust) -> Value {
    let recording = rust.recording_macro.as_ref().map(|(register, commands)| {
        json!({
            "register": register,
            "command_count": commands.len()
        })
    });
    let registers = rust
        .macro_registers
        .iter()
        .map(|(register, commands)| {
            json!({
                "register": register,
                "command_count": commands.len()
            })
        })
        .collect::<Vec<_>>();
    json!({
        "recording": recording,
        "registers": registers,
        "replay": {
            "depth": rust.macro_depth,
            "expanded_command_count": rust.macro_expanded_commands
        },
        "limits": {
            "recording_commands": MAX_MACRO_COMMANDS,
            "expanded_commands": MAX_MACRO_COMMANDS,
            "nested_depth": MAX_MACRO_DEPTH
        },
        "persistence": "memory-only"
    })
}

/// Produces the complete, presentation-safe macro summary for the status bar.
///
/// QML deliberately receives this as text rather than a serialized state object:
/// the Rust boundary owns how macro state is interpreted, while QML only renders
/// the value alongside the normal browser status text.
pub(super) fn status_text(rust: &BrowserUiRust) -> String {
    if let Some((register, commands)) = rust.recording_macro.as_ref() {
        return format!(" · recording macro @{register} ({})", commands.len());
    }

    let labels = rust
        .macro_registers
        .iter()
        .map(|(register, commands)| format!("@{register}:{}", commands.len()))
        .collect::<Vec<_>>();
    if labels.is_empty() {
        String::new()
    } else {
        format!(" · macros {}", labels.join(" "))
    }
}
