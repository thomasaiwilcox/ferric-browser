//! Typed configuration projection at the Qt boundary.
//!
//! This module translates configuration values for native editor processes and
//! UI previews without allowing the presentation layer to reinterpret the
//! configuration graph.

use crate::input_validation::is_bounded_untrusted_text;
use ferric_browser_config::RuntimeOverrides;
use serde_json::Value;
use std::path::Path;

#[must_use]
pub(super) fn toml_string_array_literal(values: &[String]) -> String {
    toml::Value::Array(values.iter().cloned().map(toml::Value::String).collect()).to_string()
}

pub(super) fn configured_editor_argv(
    config: &Value,
    file: &Path,
) -> Result<(String, Vec<String>), String> {
    let editor = config
        .get("tools")
        .and_then(Value::as_object)
        .and_then(|tools| tools.get("editor"))
        .and_then(Value::as_array)
        .ok_or_else(|| "tools.editor is not configured".to_owned())?;
    if editor.is_empty() {
        return Err("tools.editor is not configured".into());
    }
    let arguments = editor
        .iter()
        .map(|argument| {
            argument
                .as_str()
                .filter(|value| is_bounded_untrusted_text(value))
                .map(ToOwned::to_owned)
                .ok_or_else(|| {
                    "tools.editor contains an empty, oversized, or invalid argv value".to_owned()
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let placeholders = arguments
        .iter()
        .filter(|argument| argument.as_str() == "{file}")
        .count();
    if placeholders != 1 {
        return Err("tools.editor must contain exactly one complete {file} argument".into());
    }
    let executable = arguments
        .first()
        .filter(|value| !value.starts_with('-') && value.as_str() != "{file}")
        .cloned()
        .ok_or_else(|| "tools.editor executable is invalid".to_owned())?;
    let file = file.to_string_lossy().into_owned();
    if !is_bounded_untrusted_text(&file) {
        return Err("tools.editor file path is empty, oversized, or invalid".into());
    }
    let arguments = arguments
        .into_iter()
        .skip(1)
        .map(|argument| {
            if argument == "{file}" {
                file.clone()
            } else {
                argument
            }
        })
        .collect();
    Ok((executable, arguments))
}

#[must_use]
pub(super) fn config_value_at_path<'a>(root: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.')
        .try_fold(root, |value, part| value.as_object()?.get(part))
}

pub(super) fn runtime_override_value(overrides: &RuntimeOverrides, key: &str) -> Option<Value> {
    overrides
        .settings
        .get(key)
        .and_then(|value| serde_json::to_value(value).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lookup_preserves_the_typed_value_at_a_dotted_path() {
        let value = json!({"downloads": {"directory": {"path": "/tmp/downloads"}}});
        assert_eq!(
            config_value_at_path(&value, "downloads.directory.path"),
            Some(&json!("/tmp/downloads"))
        );
        assert!(config_value_at_path(&value, "downloads.unknown").is_none());
    }

    #[test]
    fn editor_argv_requires_one_complete_file_placeholder() {
        let config = json!({"tools": {"editor": ["nvim", "--", "{file}"]}});
        assert_eq!(
            configured_editor_argv(&config, Path::new("/tmp/settings.toml")).expect("argv"),
            (
                "nvim".into(),
                vec!["--".into(), "/tmp/settings.toml".into()]
            )
        );
        let invalid = json!({"tools": {"editor": ["nvim", "{file}", "{file}"]}});
        assert!(configured_editor_argv(&invalid, Path::new("/tmp/settings.toml")).is_err());
    }
}
