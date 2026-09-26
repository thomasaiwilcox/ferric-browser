//! Typed encoding for spatial Grid commands.

use super::{EncodedCommand, ParsedCommand};

pub(super) fn encode(command: &ParsedCommand) -> Result<Option<EncodedCommand>, String> {
    let arguments = match command.name.as_str() {
        "grid-refine" => {
            let [cell] = command.arguments.as_slice() else {
                return Err("grid-refine requires one cell from 1 to 9".into());
            };
            if !matches!(
                cell.as_str(),
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
            ) {
                return Err("grid-refine cell must be from 1 to 9".into());
            }
            serde_json::json!({"cell": cell})
        }
        "grid-click" => {
            let [button] = command.arguments.as_slice() else {
                return Err("grid-click requires one button: left, right, or middle".into());
            };
            if !matches!(button.as_str(), "left" | "right" | "middle") {
                return Err("grid-click button must be left, right, or middle".into());
            }
            serde_json::json!({"button": button})
        }
        "grid" | "grid-hover" | "grid-back" | "grid-reset" | "grid-help" | "grid-cancel" => {
            if !command.arguments.is_empty() {
                return Err(format!("{} does not accept arguments", command.name));
            }
            serde_json::json!({})
        }
        _ => return Ok(None),
    };
    Ok(Some(EncodedCommand::new(arguments)))
}
