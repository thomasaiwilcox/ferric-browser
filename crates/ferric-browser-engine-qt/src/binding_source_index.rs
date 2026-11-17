//! Cold binding metadata joined by logical identity, retaining authored inputs.
//!
//! This does not resolve policy or select active bindings. Callers classify
//! active winners from the already effective configuration. Layer indexes are
//! read-only assignment inputs, not another publication or execution authority.
use std::collections::BTreeMap;

use ferric_browser_config::RuntimeOverrides;
use ferric_browser_core::{Mode, binding_key_tokens};
use serde_json::Value;

use crate::binding_policy::configured_mode_name;

#[derive(Clone, Copy)]
pub(super) struct BindingSource<'a> {
    pub(super) keychain: &'a str,
    pub(super) command: &'a str,
}

#[derive(Default)]
pub(super) struct BindingSources<'a> {
    modes: BTreeMap<Mode, BTreeMap<Vec<String>, BindingSource<'a>>>,
}

pub(super) struct BindingLayers<'a> {
    pub(super) global_source: String,
    pub(super) layers: [(&'static str, BindingSources<'a>); 5],
}

impl BindingLayers<'_> {
    pub(super) fn provenance(&self, mode: Mode, keys: &[String]) -> Value {
        Value::Array(
            self.layers
                .iter()
                .filter_map(|(layer, sources)| {
                    let binding = sources.get(mode, keys)?;
                    Some(serde_json::json!({
                        "layer": layer,
                        "source": (*layer == "global").then_some(&self.global_source),
                        "keychain": binding.keychain,
                        "value": binding.command,
                        "status": if matches!(binding.command, "" | "unbound" | "none") {
                            "mask"
                        } else {
                            "contributing"
                        }
                    }))
                })
                .collect(),
        )
    }
}

impl<'a> BindingSources<'a> {
    pub(super) fn from_config(config: &'a Value) -> Self {
        let mut result = Self::default();
        if let Some(modes) = config.get("bindings").and_then(Value::as_object) {
            for (mode, entries) in modes {
                if let Some(mode) = configured_mode_name(mode)
                    && let Some(entries) = entries.as_object()
                {
                    for (key, command) in entries {
                        if let Some(command) = command.as_str() {
                            result.insert(mode, key, command);
                        }
                    }
                }
            }
        }
        result
    }

    pub(super) fn from_overrides(overrides: &'a RuntimeOverrides) -> Self {
        let mut result = Self::default();
        // Existing compiler order: sorted generic assignments, then dedicated
        // binding assignments. A table replaces its subtree; it is not merged.
        for (path, value) in &overrides.settings {
            let parts = path.split('.').collect::<Vec<_>>();
            match parts.as_slice() {
                ["bindings"] => {
                    result.modes.clear();
                    if let Some(modes) = value.as_table() {
                        for (mode, entries) in modes {
                            result.replace_mode(mode, entries);
                        }
                    }
                }
                ["bindings", mode] => result.replace_mode(mode, value),
                ["bindings", mode, key] => {
                    if let Some(mode) = configured_mode_name(mode)
                        && let Some(command) = value.as_str()
                    {
                        result.insert(mode, key, command);
                    }
                }
                _ => {}
            }
        }
        for (mode, entries) in &overrides.bindings {
            if let Some(mode) = configured_mode_name(mode) {
                for (key, command) in entries {
                    result.insert(mode, key, command);
                }
            }
        }
        result
    }

    pub(super) fn get(&self, mode: Mode, keys: &[String]) -> Option<BindingSource<'a>> {
        self.modes.get(&mode)?.get(keys).copied()
    }

    fn insert(&mut self, mode: Mode, key: &'a str, command: &'a str) {
        if let Some(keys) = binding_key_tokens(key) {
            self.modes.entry(mode).or_default().insert(
                keys,
                BindingSource {
                    keychain: key,
                    command,
                },
            );
        }
    }

    fn replace_mode(&mut self, mode: &str, entries: &'a toml::Value) {
        if let Some(mode) = configured_mode_name(mode) {
            self.modes.remove(&mode);
            if let Some(entries) = entries.as_table() {
                for (key, command) in entries {
                    if let Some(command) = command.as_str() {
                        self.insert(mode, key, command);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BindingLayers, BindingSources};
    use ferric_browser_config::{Config, RuntimeOverrides, apply_runtime_overrides_traced};
    use ferric_browser_core::{Mode, binding_key_tokens};

    fn base() -> Config {
        let mut base = Config::default();
        base.bindings.insert(
            "normal".into(),
            [
                ("Plus".into(), "zoom in".into()),
                ("j".into(), "scroll down".into()),
            ]
            .into(),
        );
        base.bindings.insert(
            "insert".into(),
            [("Escape".into(), "mode-enter normal".into())].into(),
        );
        base
    }

    fn check(overrides: &RuntimeOverrides, expected: &[(Mode, &str, &str)]) {
        // Compare metadata only after the existing compiler accepts the input.
        let applied = apply_runtime_overrides_traced(&base(), overrides).unwrap();
        assert_ne!(applied.assignments.len(), 0);
        let effective = serde_json::to_value(&applied.configuration).unwrap();
        let active = BindingSources::from_config(&effective);
        let source = BindingSources::from_overrides(overrides);
        let count: usize = source
            .modes
            .values()
            .map(std::collections::BTreeMap::len)
            .sum();
        assert_eq!(count, expected.len());
        for &(mode, keychain, command) in expected {
            let keys = binding_key_tokens(keychain).unwrap();
            let input = source
                .get(mode, &keys)
                .expect("surviving explicit assignment");
            assert_eq!((input.keychain, input.command), (keychain, command));
            let winner = active
                .get(mode, &keys)
                .expect("compiler published assignment");
            assert_eq!(winner.keychain, keychain);
            assert_eq!(
                winner.command,
                if command.is_empty() {
                    "unbound"
                } else {
                    command
                }
            );
        }
    }

    #[test]
    fn source_indexes_match_successful_generic_and_dedicated_assignments() {
        let mut layer = RuntimeOverrides::default();
        layer.settings.insert(
            "bindings".into(),
            toml::from_str("normal = { Plus = 'zoom out', j = 'scroll up' }").unwrap(),
        );
        check(
            &layer,
            &[
                (Mode::Normal, "Plus", "zoom out"),
                (Mode::Normal, "j", "scroll up"),
            ],
        );
        layer.settings.insert(
            "bindings.normal".into(),
            toml::from_str("'+' = 'zoom reset'\nj = 'scroll down'").unwrap(),
        );
        layer
            .settings
            .insert("bindings.normal.j".into(), "scroll up".into());
        check(
            &layer,
            &[
                (Mode::Normal, "+", "zoom reset"),
                (Mode::Normal, "j", "scroll up"),
            ],
        );
        layer.set_binding("normal", "Plus", "").unwrap();
        layer
            .set_binding("normal", "Ctrl+v,g", "zoom 1.25")
            .unwrap();
        layer.set_binding("normal", "g,.", "zoom reset").unwrap();
        check(
            &layer,
            &[
                (Mode::Normal, "Plus", ""),
                (Mode::Normal, "j", "scroll up"),
                (Mode::Normal, "Ctrl+v,g", "zoom 1.25"),
                (Mode::Normal, "g,.", "zoom reset"),
            ],
        );
    }

    #[test]
    fn source_indexes_drop_leaves_removed_by_empty_table_assignments() {
        for path in ["bindings", "bindings.normal"] {
            let mut layer = RuntimeOverrides::default();
            layer
                .settings
                .insert(path.into(), toml::Value::Table(toml::map::Map::new()));
            check(&layer, &[]);
            let applied = apply_runtime_overrides_traced(&base(), &layer).unwrap();
            assert!(
                applied
                    .configuration
                    .bindings
                    .get("normal")
                    .is_none_or(std::collections::BTreeMap::is_empty)
            );
            layer.set_binding("normal", "g,g", "zoom reset").unwrap();
            check(&layer, &[(Mode::Normal, "g,g", "zoom reset")]);
        }
    }

    #[test]
    fn provenance_preserves_authored_masks_and_uses_token_vectors() {
        for marker in ["", "unbound", "none"] {
            let mut layer = RuntimeOverrides::default();
            layer.set_binding("normal", "Ctrl+v,g", marker).unwrap();
            check(&layer, &[(Mode::Normal, "Ctrl+v,g", marker)]);
            let layers = BindingLayers {
                global_source: "test base".into(),
                layers: [
                    ("global", BindingSources::default()),
                    ("profile", BindingSources::default()),
                    ("runtime", BindingSources::from_overrides(&layer)),
                    ("cli", BindingSources::default()),
                    ("temporary", BindingSources::default()),
                ],
            };
            let provenance =
                layers.provenance(Mode::Normal, &binding_key_tokens("Ctrl+v,g").unwrap());
            assert_eq!(provenance[0]["keychain"], "Ctrl+v,g");
            assert_eq!(provenance[0]["value"], marker);
            assert_eq!(provenance[0]["status"], "mask");
        }
    }
}
