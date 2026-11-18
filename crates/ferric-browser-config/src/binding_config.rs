//! Binding identity at authored-source and existing layer merge boundaries.
use std::collections::{BTreeMap, BTreeSet};

use ferric_browser_core::binding_key_tokens;

use crate::{ConfigError, configuration_diagnostic::bounded_context};

pub(super) struct BindingIndex(BTreeMap<Vec<String>, String>);

impl BindingIndex {
    pub(super) fn new<'a>(keys: impl Iterator<Item = &'a str>) -> Self {
        Self(
            keys.filter_map(|key| binding_key_tokens(key).map(|keys| (keys, key.to_owned())))
                .collect(),
        )
    }

    /// Returns the equivalent lower spelling to remove. Build once per merged
    /// mode, so an authored table is not rescanned for each incoming binding.
    pub(super) fn insert(&mut self, key: &str) -> Option<String> {
        binding_key_tokens(key).and_then(|keys| self.0.insert(keys, key.to_owned()))
    }
}

pub(super) fn replace(bindings: &mut BTreeMap<String, String>, key: &str, command: String) {
    let mut index = BindingIndex::new(bindings.keys().map(String::as_str));
    if let Some(lower) = index.insert(key) {
        bindings.remove(&lower);
    }
    bindings.insert(key.to_owned(), command);
}

pub(super) fn validate_keys<'a>(
    mode: &str,
    keys: impl Iterator<Item = &'a str>,
) -> Result<(), ConfigError> {
    let mut seen = BTreeSet::new();
    for key in keys {
        let canonical = binding_key_tokens(key).ok_or_else(|| {
            ConfigError::validation(format_args!(
                "binding keychain in mode {} is not a bounded canonical key sequence",
                bounded_context(format_args!("{mode:?}"))
            ))
        })?;
        if !seen.insert(canonical) {
            return Err(ConfigError::validation(format_args!(
                "binding mode {} contains equivalent duplicate keychains",
                bounded_context(format_args!("{mode:?}"))
            )));
        }
    }
    Ok(())
}

/// Validate source aliases before a higher table or binding can hide them.
/// Other types retain the loader's existing final typed validation behavior.
pub(super) fn validate_raw(value: &toml::Value) -> Result<(), ConfigError> {
    if let Some(modes) = value.as_table() {
        for (mode, entries) in modes {
            if let Some(entries) = entries.as_table() {
                validate_keys(mode, entries.keys().map(String::as_str))?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_setting(path: &str, value: &toml::Value) -> Result<(), ConfigError> {
    if path == "bindings" {
        validate_raw(value)?;
    } else if let Some(mode) = path.strip_prefix("bindings.")
        && !mode.contains('.')
        && let Some(entries) = value.as_table()
    {
        validate_keys(mode, entries.keys().map(String::as_str))?;
    }
    Ok(())
}
