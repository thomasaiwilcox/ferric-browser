//! Registry-backed presentation records for the Settings surface.
//!
//! The QML view must not own a second list of supported settings.  This
//! projection is the single adapter from the authoritative setting registry
//! and effective configuration to presentation rows.

use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QStringList, QVariant};
use ferric_browser_config::setting_metadata_all;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SettingsRow {
    pub key: &'static str,
    pub label: &'static str,
    pub editor_type: &'static str,
    pub scope: String,
    pub apply_time: &'static str,
    pub value: String,
    pub options: &'static [&'static str],
}

#[must_use]
pub(super) fn project(config: &Value) -> Vec<SettingsRow> {
    SETTINGS_SURFACE
        .iter()
        .filter_map(|definition| {
            let metadata = setting_metadata_all()
                .iter()
                .find(|metadata| metadata.key == definition.key)?;
            value_at(config, definition.key).and_then(|value| {
                display_value(value, definition.editor_type).map(|value| SettingsRow {
                    key: definition.key,
                    label: definition.label,
                    editor_type: definition.editor_type,
                    scope: metadata.supported_scopes.join("/"),
                    apply_time: metadata.apply_time,
                    value,
                    options: definition.options,
                })
            })
        })
        .collect()
}

#[must_use]
pub(super) fn project_variant(config: &Value) -> QVariant {
    let rows = project(config);
    let mut values = QList::<QVariant>::default();
    values.reserve(rows.len().try_into().unwrap_or(isize::MAX));
    for row in rows {
        let mut value = QMap::<QMapPair_QString_QVariant>::default();
        insert_text(&mut value, "key", row.key);
        insert_text(&mut value, "label", row.label);
        insert_text(&mut value, "type", row.editor_type);
        insert_text(&mut value, "scope", &row.scope);
        insert_text(&mut value, "apply", row.apply_time);
        insert_text(&mut value, "value", &row.value);
        let options: QStringList = row
            .options
            .iter()
            .map(|option| QString::from(*option))
            .collect();
        value.insert(QString::from("options"), QVariant::from(&options));
        values.append(QVariant::from(&value));
    }
    QVariant::from(&values)
}

fn insert_text(map: &mut QMap<QMapPair_QString_QVariant>, key: &str, value: &str) {
    let value = QString::from(value);
    map.insert(QString::from(key), QVariant::from(&value));
}

struct SettingsSurfaceDefinition {
    key: &'static str,
    label: &'static str,
    editor_type: &'static str,
    options: &'static [&'static str],
}

const NO_OPTIONS: &[&str] = &[];

// This is deliberately limited to values the pre-alpha settings surface can
// edit. The configuration registry remains the authoritative source for
// validation, scope, apply time, documentation, and completion. Keeping the
// surface allowlist here prevents QML from becoming a second configuration
// schema while avoiding controls for values that need a dedicated workflow.
const SETTINGS_SURFACE: &[SettingsSurfaceDefinition] = &[
    SettingsSurfaceDefinition {
        key: "ui.font_family",
        label: "Chrome font family",
        editor_type: "text",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "ui.font_size_pt",
        label: "Chrome font size (pt)",
        editor_type: "number",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "ui.reduced_motion",
        label: "Reduced motion",
        editor_type: "enum",
        options: &["system", "on", "off"],
    },
    SettingsSurfaceDefinition {
        key: "input.entry_mode",
        label: "Page entry mode",
        editor_type: "enum",
        options: &["normal", "insert", "passthrough"],
    },
    SettingsSurfaceDefinition {
        key: "discovery.learning_mode",
        label: "Learning mode",
        editor_type: "bool",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "links.cleaning.enabled",
        label: "Clean-link operations",
        editor_type: "bool",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "content.javascript",
        label: "JavaScript",
        editor_type: "bool",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "content.images",
        label: "Images",
        editor_type: "bool",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "content.force_dark",
        label: "Force dark pages",
        editor_type: "bool",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "content.autoplay",
        label: "Autoplay policy",
        editor_type: "enum",
        options: &["engine-default", "require-gesture"],
    },
    SettingsSurfaceDefinition {
        key: "content.zoom",
        label: "Default page zoom",
        editor_type: "number",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "privacy.remote_suggestions",
        label: "Remote suggestions",
        editor_type: "bool",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "blocking.enabled",
        label: "Network blocking",
        editor_type: "bool",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "blocking.update_interval_hours",
        label: "Blocklist update interval (hours)",
        editor_type: "number",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "downloads.ask_destination",
        label: "Ask for download destination",
        editor_type: "bool",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "downloads.collision",
        label: "Download collision policy",
        editor_type: "enum",
        options: &["ask", "rename"],
    },
    SettingsSurfaceDefinition {
        key: "spellcheck.enabled",
        label: "Spellcheck",
        editor_type: "bool",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "spellcheck.languages",
        label: "Spellcheck languages",
        editor_type: "languages",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "desktop.portals",
        label: "Desktop portals",
        editor_type: "enum",
        options: &["auto", "required"],
    },
    SettingsSurfaceDefinition {
        key: "desktop.notifications",
        label: "Desktop notifications",
        editor_type: "bool",
        options: NO_OPTIONS,
    },
    SettingsSurfaceDefinition {
        key: "logging.level",
        label: "Logging level",
        editor_type: "enum",
        options: &["error", "warn", "info", "debug"],
    },
];

fn display_value(value: &Value, editor_type: &str) -> Option<String> {
    match (editor_type, value) {
        ("bool", Value::Bool(value)) => Some(value.to_string()),
        ("number", Value::Number(value)) => Some(value.to_string()),
        ("text" | "enum", Value::String(value)) => Some(value.clone()),
        ("languages", Value::Array(values)) => values
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<_>>>()
            .map(|values| values.join(", ")),
        _ => None,
    }
}

fn value_at<'a>(root: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.')
        .try_fold(root, |value, part| value.as_object()?.get(part))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_config::Config;

    #[test]
    fn rows_follow_the_authoritative_registry_and_effective_values() {
        let config = serde_json::to_value(Config::default()).expect("default config serializes");
        let rows = project(&config);
        assert!(!rows.is_empty());
        assert_eq!(rows.len(), SETTINGS_SURFACE.len());
        let zoom = rows.iter().find(|row| row.key == "content.zoom").unwrap();
        assert_eq!(zoom.editor_type, "number");
        assert_eq!(zoom.value, "1.0");
        assert!(zoom.scope.contains("site"));
    }

    #[test]
    fn presentation_rows_use_registry_scope_and_apply_metadata() {
        let config = serde_json::to_value(Config::default()).expect("default config serializes");
        let row = project(&config)
            .into_iter()
            .find(|row| row.key == "content.javascript")
            .expect("javascript row");
        assert_eq!(row.scope, "global/profile/site");
        assert_eq!(row.apply_time, "navigation");
        assert_eq!(row.value, "true");
    }

    #[test]
    fn rows_cross_the_qml_boundary_as_structured_records() {
        let config = serde_json::to_value(Config::default()).expect("default config serializes");
        let rows = project_variant(&config);
        assert_eq!(rows.type_id(), cxx_qt_lib::QMetaTypeType::QVariantList);
    }
}
