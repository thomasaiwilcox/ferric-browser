const GLOBAL_SCOPES: &[&str] = &["global"];
const PROFILE_SCOPES: &[&str] = &["profile"];
const GLOBAL_PROFILE_SCOPES: &[&str] = &["global", "profile"];
const GLOBAL_PROFILE_SITE_SCOPES: &[&str] = &["global", "profile", "site"];

/// Declarative metadata for one configuration setting.
///
/// The values are stable protocol/documentation strings. Validation remains
/// owned by the typed configuration structs, while this table makes the
/// setting contract discoverable to UI and IPC clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingMetadata {
    pub key: &'static str,
    pub value_type: &'static str,
    pub default_value: &'static str,
    pub supported_scopes: &'static [&'static str],
    pub apply_time: &'static str,
    pub prerequisite: &'static str,
    pub sensitivity: &'static str,
}

/// The authoritative typed catalog of supported configuration settings.
///
/// Parsing and validation remain owned by the configuration model, while this
/// registry supplies the shared contract used by documentation, completion,
/// IPC discovery, and presentation adapters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SettingRegistry;

impl SettingRegistry {
    /// Returns the current stable setting catalog.
    #[must_use]
    pub const fn default_v1() -> Self {
        Self
    }

    /// Returns all static setting definitions in deterministic catalog order.
    #[must_use]
    pub const fn definitions(self) -> &'static [SettingMetadata] {
        SETTING_METADATA
    }

    /// Resolves an exact setting or a supported dynamic setting namespace.
    #[must_use]
    pub fn resolve(self, key: &str) -> Option<&'static SettingMetadata> {
        if let Some(metadata) = SETTING_METADATA.iter().find(|metadata| metadata.key == key) {
            return Some(metadata);
        }
        if key.starts_with("search_engines.")
            || key.starts_with("action_targets.")
            || key.starts_with("bindings.")
        {
            return Some(&DYNAMIC_GLOBAL_PROFILE_METADATA);
        }
        if key.starts_with("site_rules.") || key.starts_with("permission_rules.") {
            return Some(&DYNAMIC_GLOBAL_METADATA);
        }
        None
    }
}

/// A configuration value that cannot affect the current process immediately.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingSettingChange {
    pub key: String,
    pub current: toml::Value,
    pub pending: toml::Value,
    pub apply_time: &'static str,
}

macro_rules! setting_metadata {
    ($key:literal, $kind:literal, $default:literal, $scopes:expr, $apply:literal, $requires:literal, $sensitivity:literal) => {
        SettingMetadata {
            key: $key,
            value_type: $kind,
            default_value: $default,
            supported_scopes: $scopes,
            apply_time: $apply,
            prerequisite: $requires,
            sensitivity: $sensitivity,
        }
    };
}

pub(super) const SETTING_METADATA: &[SettingMetadata] = &[
    setting_metadata!(
        "schema_version",
        "integer",
        "1",
        GLOBAL_SCOPES,
        "reload",
        "none",
        "normal"
    ),
    setting_metadata!(
        "include",
        "list<string>",
        "[]",
        GLOBAL_SCOPES,
        "reload",
        "none",
        "normal"
    ),
    setting_metadata!(
        "ui.statusbar",
        "enum",
        "in-mode",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "ui.tabs",
        "enum",
        "multiple",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "ui.tab_position",
        "enum",
        "top",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "ui.font_family",
        "string",
        "monospace",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "ui.font_size_pt",
        "number",
        "10.0",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "ui.reduced_motion",
        "enum",
        "system",
        GLOBAL_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "theme.source",
        "enum",
        "auto",
        GLOBAL_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "theme.path",
        "path",
        "",
        GLOBAL_SCOPES,
        "live",
        "none",
        "trusted"
    ),
    setting_metadata!(
        "input.entry_mode",
        "enum",
        "normal",
        GLOBAL_PROFILE_SITE_SCOPES,
        "navigation",
        "none",
        "normal"
    ),
    setting_metadata!(
        "input.auto_insert",
        "boolean",
        "true",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "input.keychain_timeout_ms",
        "integer",
        "1000",
        GLOBAL_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "input.count_limit",
        "integer",
        "9999",
        GLOBAL_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "hints.chars",
        "string",
        "asdfghjkl",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "hints.min_chars",
        "integer",
        "1",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "hints.auto_follow",
        "enum",
        "full-match",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "hints.unmatched",
        "enum",
        "hide",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "hints.rapid_unmatched",
        "enum",
        "hide",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "hints.marker_scale",
        "number",
        "1.0",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "discovery.keychain_overlay",
        "boolean",
        "true",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "discovery.keychain_overlay_delay_ms",
        "integer",
        "350",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "discovery.learning_mode",
        "boolean",
        "false",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "navigation.default_search",
        "string",
        "ddg",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "network",
        "normal"
    ),
    setting_metadata!(
        "navigation.start_pages",
        "list<string>",
        "[\"rb://start\"]",
        GLOBAL_PROFILE_SCOPES,
        "startup",
        "none",
        "normal"
    ),
    setting_metadata!(
        "navigation.new_tab",
        "string",
        "rb://blank",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "navigation.external_links",
        "enum",
        "tab",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "links.cleaning.enabled",
        "boolean",
        "true",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "links.cleaning.rules",
        "string",
        "builtin",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "links.cleaning.confirm_navigation",
        "boolean",
        "true",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "links.cleaning.update_source",
        "optional<https-url>",
        "null",
        GLOBAL_SCOPES,
        "live",
        "network",
        "normal"
    ),
    setting_metadata!(
        "links.cleaning.update_sha256",
        "optional<sha256>",
        "null",
        GLOBAL_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "search_engines.*",
        "https-template",
        "built-in",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "network",
        "normal"
    ),
    setting_metadata!(
        "tabs.last_close",
        "enum",
        "blank",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "tabs.related_position",
        "enum",
        "after-opener",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "tabs.undo_limit",
        "integer",
        "100",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "tabs.auto_discard",
        "boolean",
        "false",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "session.restore",
        "enum",
        "ask-after-crash",
        GLOBAL_PROFILE_SCOPES,
        "startup",
        "durable-storage",
        "normal"
    ),
    setting_metadata!(
        "session.lazy_restore",
        "boolean",
        "true",
        GLOBAL_PROFILE_SCOPES,
        "startup",
        "durable-storage",
        "normal"
    ),
    setting_metadata!(
        "session.checkpoint_seconds",
        "integer",
        "5",
        GLOBAL_SCOPES,
        "live",
        "durable-storage",
        "normal"
    ),
    setting_metadata!(
        "switcher.max_results",
        "integer",
        "100",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "switcher.include_private",
        "boolean",
        "false",
        GLOBAL_SCOPES,
        "live",
        "none",
        "sensitive"
    ),
    setting_metadata!(
        "history.retention_days",
        "integer",
        "90",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "durable-storage",
        "sensitive"
    ),
    setting_metadata!(
        "history.command_limit",
        "integer",
        "1000",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "durable-storage",
        "sensitive"
    ),
    setting_metadata!(
        "content.javascript",
        "boolean",
        "true",
        GLOBAL_PROFILE_SITE_SCOPES,
        "navigation",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "content.images",
        "boolean",
        "true",
        GLOBAL_PROFILE_SITE_SCOPES,
        "navigation",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "content.force_dark",
        "boolean",
        "false",
        GLOBAL_PROFILE_SITE_SCOPES,
        "navigation",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "content.autoplay",
        "enum",
        "engine-default",
        GLOBAL_PROFILE_SITE_SCOPES,
        "navigation",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "content.zoom",
        "number",
        "1.0",
        GLOBAL_PROFILE_SITE_SCOPES,
        "live",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "privacy.remote_suggestions",
        "boolean",
        "false",
        GLOBAL_SCOPES,
        "live",
        "none",
        "sensitive"
    ),
    setting_metadata!(
        "privacy.push_service",
        "boolean",
        "false",
        PROFILE_SCOPES,
        "restart",
        "external-service",
        "sensitive"
    ),
    setting_metadata!(
        "privacy.private_history_suggestions",
        "boolean",
        "false",
        GLOBAL_SCOPES,
        "live",
        "durable-storage",
        "sensitive"
    ),
    setting_metadata!(
        "permissions.camera",
        "enum",
        "ask",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "sensitive"
    ),
    setting_metadata!(
        "permissions.microphone",
        "enum",
        "ask",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "sensitive"
    ),
    setting_metadata!(
        "permissions.screen_capture",
        "enum",
        "ask",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "portal",
        "sensitive"
    ),
    setting_metadata!(
        "permissions.notifications",
        "enum",
        "ask",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "sensitive"
    ),
    setting_metadata!(
        "permissions.geolocation",
        "enum",
        "ask",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "sensitive"
    ),
    setting_metadata!(
        "permissions.clipboard",
        "enum",
        "ask",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "sensitive"
    ),
    setting_metadata!(
        "permissions.local_fonts",
        "enum",
        "ask",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "blocking.enabled",
        "boolean",
        "true",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "blocking.network_filtering",
        "boolean",
        "true",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "blocking.cosmetic_filtering",
        "boolean",
        "false",
        GLOBAL_PROFILE_SCOPES,
        "navigation",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "blocking.update_interval_hours",
        "integer",
        "24",
        GLOBAL_SCOPES,
        "live",
        "network",
        "normal"
    ),
    setting_metadata!(
        "blocking.lists",
        "list<string>",
        "[\"easylist\",\"easyprivacy\"]",
        GLOBAL_SCOPES,
        "live",
        "network",
        "normal"
    ),
    setting_metadata!(
        "blocking.bypass_sites",
        "list<host-pattern>",
        "[]",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "blocking.security_deny_hosts",
        "list<host-pattern>",
        "[]",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "security",
        "normal"
    ),
    setting_metadata!(
        "downloads.directory",
        "enum-or-path",
        "xdg-downloads",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "filesystem",
        "normal"
    ),
    setting_metadata!(
        "downloads.ask_destination",
        "boolean",
        "true",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "portal",
        "normal"
    ),
    setting_metadata!(
        "downloads.collision",
        "enum",
        "ask",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "filesystem",
        "normal"
    ),
    setting_metadata!(
        "downloads.open_when_complete",
        "boolean",
        "false",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "spellcheck.enabled",
        "boolean",
        "true",
        PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "spellcheck.languages",
        "list<bcp47>",
        "[\"system\"]",
        PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "normal"
    ),
    setting_metadata!(
        "desktop.portals",
        "enum",
        "auto",
        GLOBAL_SCOPES,
        "restart",
        "portal",
        "normal"
    ),
    setting_metadata!(
        "desktop.notifications",
        "boolean",
        "true",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "portal",
        "sensitive"
    ),
    setting_metadata!(
        "desktop.media_keys",
        "boolean",
        "true",
        GLOBAL_SCOPES,
        "live",
        "desktop",
        "normal"
    ),
    setting_metadata!(
        "hyprland.enabled",
        "enum",
        "auto",
        GLOBAL_SCOPES,
        "live",
        "hyprland",
        "normal"
    ),
    setting_metadata!(
        "hyprland.workspace_routing",
        "boolean",
        "false",
        GLOBAL_SCOPES,
        "live",
        "hyprland",
        "normal"
    ),
    setting_metadata!(
        "site_doctor.temporary_experiments",
        "boolean",
        "true",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "qt-webengine",
        "sensitive"
    ),
    setting_metadata!(
        "ipc.enabled",
        "boolean",
        "true",
        GLOBAL_SCOPES,
        "restart",
        "af-unix",
        "sensitive"
    ),
    setting_metadata!(
        "ipc.private_queries",
        "boolean",
        "false",
        GLOBAL_SCOPES,
        "live",
        "af-unix",
        "sensitive"
    ),
    setting_metadata!(
        "tools.editor",
        "list<string>",
        "[]",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "filesystem",
        "trusted-code"
    ),
    setting_metadata!(
        "action_targets.*",
        "typed-table",
        "{}",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "filesystem",
        "trusted-code"
    ),
    setting_metadata!(
        "logging.level",
        "enum",
        "info",
        GLOBAL_SCOPES,
        "live",
        "filesystem",
        "sensitive"
    ),
    setting_metadata!(
        "logging.max_file_mib",
        "integer",
        "5",
        GLOBAL_SCOPES,
        "live",
        "filesystem",
        "normal"
    ),
    setting_metadata!(
        "logging.retained_files",
        "integer",
        "3",
        GLOBAL_SCOPES,
        "live",
        "filesystem",
        "normal"
    ),
    setting_metadata!(
        "bindings.*",
        "typed-map",
        "{}",
        GLOBAL_PROFILE_SCOPES,
        "live",
        "none",
        "normal"
    ),
    setting_metadata!(
        "site_rules.*",
        "typed-list",
        "[]",
        GLOBAL_SCOPES,
        "reload",
        "none",
        "normal"
    ),
    setting_metadata!(
        "permission_rules.*",
        "typed-list",
        "[]",
        GLOBAL_SCOPES,
        "reload",
        "durable-storage",
        "sensitive"
    ),
];

const DYNAMIC_GLOBAL_PROFILE_METADATA: SettingMetadata = setting_metadata!(
    "dynamic",
    "dynamic",
    "{}",
    GLOBAL_PROFILE_SCOPES,
    "live",
    "none",
    "normal"
);

const DYNAMIC_GLOBAL_METADATA: SettingMetadata = setting_metadata!(
    "dynamic",
    "dynamic",
    "{}",
    GLOBAL_SCOPES,
    "live",
    "none",
    "normal"
);

/// Returns the complete metadata for a known setting or a bounded dynamic
/// namespace such as `search_engines.<name>` or `bindings.<mode>.<key>`.
#[must_use]
pub fn setting_metadata(key: &str) -> Option<&'static SettingMetadata> {
    SettingRegistry::default_v1().resolve(key)
}

/// Returns the static metadata rows used to generate user-facing setting
/// documentation and registry-backed configuration surfaces.
#[must_use]
pub const fn setting_metadata_all() -> &'static [SettingMetadata] {
    SettingRegistry::default_v1().definitions()
}

/// Returns the validated scopes for a setting. The returned scope names are
/// stable protocol data used by configuration explanations and UI metadata.
#[must_use]
pub fn setting_supported_scopes(key: &str) -> &'static [&'static str] {
    setting_metadata(key).map_or(GLOBAL_PROFILE_SCOPES, |metadata| metadata.supported_scopes)
}

/// Returns whether a setting has a validated per-site rule representation.
///
/// A profile-wide engine value must not be made to look site-scoped merely by
/// changing it when a tab gains focus; that would leak behavior across tabs.
#[must_use]
pub fn setting_supports_site_scope(key: &str) -> bool {
    setting_supported_scopes(key).contains(&"site")
}

fn applies_without_restart(apply_time: &str) -> bool {
    matches!(apply_time, "live" | "navigation")
}

fn is_pending_apply_time(apply_time: &str) -> bool {
    matches!(apply_time, "reload" | "startup" | "restart")
}

/// Applies only settings that are safe to observe in the current process.
///
/// Reload-, startup-, and restart-scoped values remain in the candidate used
/// for the next lifecycle boundary, but do not replace unrelated live state.
#[must_use]
pub fn apply_immediate_config_changes(
    current: &toml::Value,
    candidate: &toml::Value,
) -> toml::Value {
    apply_immediate_config_changes_at(current, candidate, "")
}

fn apply_immediate_config_changes_at(
    current: &toml::Value,
    candidate: &toml::Value,
    prefix: &str,
) -> toml::Value {
    let key_metadata = (!prefix.is_empty()).then(|| setting_metadata(prefix));
    if let Some(Some(metadata)) = key_metadata
        && applies_without_restart(metadata.apply_time)
    {
        return candidate.clone();
    }

    let (Some(current_table), Some(candidate_table)) = (current.as_table(), candidate.as_table())
    else {
        return current.clone();
    };
    let mut merged = current_table.clone();
    for (key, candidate_value) in candidate_table {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        let next = if let Some(current_value) = current_table.get(key) {
            apply_immediate_config_changes_at(current_value, candidate_value, &full_key)
        } else if setting_metadata(&full_key)
            .is_some_and(|metadata| applies_without_restart(metadata.apply_time))
        {
            candidate_value.clone()
        } else {
            continue;
        };
        merged.insert(key.clone(), next);
    }
    for key in current_table.keys() {
        if candidate_table.contains_key(key) {
            continue;
        }
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if setting_metadata(&full_key)
            .is_some_and(|metadata| applies_without_restart(metadata.apply_time))
        {
            merged.remove(key);
        }
    }
    toml::Value::Table(merged)
}

/// Lists configuration changes that require a later lifecycle boundary.
#[must_use]
pub fn pending_config_changes(
    current: &toml::Value,
    candidate: &toml::Value,
) -> Vec<PendingSettingChange> {
    let mut changes = Vec::new();
    collect_pending_config_changes(current, candidate, "", &mut changes);
    changes
}

fn collect_pending_config_changes(
    current: &toml::Value,
    candidate: &toml::Value,
    prefix: &str,
    changes: &mut Vec<PendingSettingChange>,
) {
    if let (Some(current_table), Some(candidate_table)) = (current.as_table(), candidate.as_table())
    {
        for (key, candidate_value) in candidate_table {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            let current_value = current_table
                .get(key)
                .unwrap_or(&toml::Value::Boolean(false));
            if setting_metadata(&full_key)
                .is_some_and(|metadata| is_pending_apply_time(metadata.apply_time))
            {
                if current_value != candidate_value {
                    let metadata = setting_metadata(&full_key).expect("metadata just matched");
                    changes.push(PendingSettingChange {
                        key: full_key,
                        current: current_value.clone(),
                        pending: candidate_value.clone(),
                        apply_time: metadata.apply_time,
                    });
                }
            } else {
                collect_pending_config_changes(current_value, candidate_value, &full_key, changes);
            }
        }
        return;
    }

    if !prefix.is_empty()
        && current != candidate
        && setting_metadata(prefix)
            .is_some_and(|metadata| is_pending_apply_time(metadata.apply_time))
    {
        let metadata = setting_metadata(prefix).expect("metadata just matched");
        changes.push(PendingSettingChange {
            key: prefix.to_owned(),
            current: current.clone(),
            pending: candidate.clone(),
            apply_time: metadata.apply_time,
        });
    }
}
