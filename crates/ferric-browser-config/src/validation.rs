//! Configuration invariant validation.

use super::{
    BTreeSet, CURRENT_SCHEMA_VERSION, Config, ConfigError, ContextsConfig, MAX_RUNTIME_KEY_BYTES,
    MAX_RUNTIME_SETTINGS, MAX_RUNTIME_VALUE_BYTES, PermissionDecision, ProfileDefinition,
    ProfilesConfig, RUNTIME_SCHEMA_VERSION, RuntimeOverrides, apply_runtime_overrides,
    parse_context_pattern, parse_site_pattern, setting_supports_site_scope,
    valid_link_update_source, valid_sha256, validate_site_rule,
};

/// Validates an in-memory configuration candidate before it becomes effective.
///
/// # Errors
///
/// Returns an error when a value violates the supported configuration schema
/// or one of its cross-field invariants.
#[allow(clippy::too_many_lines)]
pub fn validate(config: &Config) -> Result<(), ConfigError> {
    if config.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(ConfigError::Validation(format!(
            "unsupported schema_version {}",
            config.schema_version
        )));
    }
    if !matches!(
        config.ui.statusbar.as_str(),
        "always" | "in-mode" | "command" | "never"
    ) {
        return Err(ConfigError::Validation(
            "ui.statusbar must be always, in-mode, or never".into(),
        ));
    }
    if config.privacy.remote_suggestions {
        return Err(ConfigError::Validation(
            "privacy.remote_suggestions is unavailable; no remote suggestion provider is implemented".into(),
        ));
    }
    if !matches!(
        config.ui.tabs.as_str(),
        "always" | "multiple" | "switching" | "never"
    ) {
        return Err(ConfigError::Validation(
            "ui.tabs has an invalid value".into(),
        ));
    }
    if !matches!(
        config.ui.tab_position.as_str(),
        "top" | "bottom" | "left" | "right"
    ) {
        return Err(ConfigError::Validation(
            "ui.tab_position has an invalid value".into(),
        ));
    }
    if config.ui.font_family.trim().is_empty() || !(6.0..=40.0).contains(&config.ui.font_size_pt) {
        return Err(ConfigError::Validation(
            "ui font settings are outside their allowed range".into(),
        ));
    }
    if !(100..=5_000).contains(&config.input.keychain_timeout_ms)
        || !(1..=9_999).contains(&config.input.count_limit)
    {
        return Err(ConfigError::Validation(
            "input limits are outside their allowed range".into(),
        ));
    }
    let mut hint_chars = BTreeSet::new();
    if !(2..=32).contains(&config.hints.chars.len())
        || !config.hints.chars.is_ascii()
        || config
            .hints
            .chars
            .bytes()
            .any(|byte| !byte.is_ascii_graphic() || byte == b'/' || !hint_chars.insert(byte))
    {
        return Err(ConfigError::Validation(
            "hints.chars must contain 2..=32 unique printable ASCII characters excluding /".into(),
        ));
    }
    if !(1..=8).contains(&config.hints.min_chars) {
        return Err(ConfigError::Validation(
            "hints.min_chars must be 1..=8".into(),
        ));
    }
    if !(0.75..=2.0).contains(&config.hints.marker_scale) {
        return Err(ConfigError::Validation(
            "hints.marker_scale must be 0.75..=2.0".into(),
        ));
    }
    if !(100..=2_000).contains(&config.discovery.keychain_overlay_delay_ms) {
        return Err(ConfigError::Validation(
            "discovery.keychain_overlay_delay_ms must be 100..=2000".into(),
        ));
    }
    if config.search_engines.is_empty() {
        return Err(ConfigError::Validation(
            "at least one search engine is required".into(),
        ));
    }
    for (name, template) in &config.search_engines {
        if name.trim().is_empty()
            || !template.starts_with("https://")
            || template.matches("{query}").count() != 1
            || template.contains('{') && template.replace("{query}", "").contains('{')
            || template.contains('}') && template.replace("{query}", "").contains('}')
        {
            return Err(ConfigError::Validation(format!(
                "search engine {name:?} must be an HTTPS template with exactly one {{query}}"
            )));
        }
    }
    if config.tools.editor.len() > 256 {
        return Err(ConfigError::Validation(
            "tools.editor supports at most 256 argv values".into(),
        ));
    }
    if !config.tools.editor.is_empty() {
        if config.tools.editor[0].is_empty()
            || config.tools.editor[0].starts_with('-')
            || config.tools.editor[0].chars().any(char::is_control)
        {
            return Err(ConfigError::Validation(
                "tools.editor executable is invalid".into(),
            ));
        }
        let placeholders = config
            .tools
            .editor
            .iter()
            .filter(|argument| argument.as_str() == "{file}")
            .count();
        if placeholders != 1 {
            return Err(ConfigError::Validation(
                "tools.editor must contain exactly one complete {file} argument".into(),
            ));
        }
    }
    for argument in &config.tools.editor {
        if argument.is_empty()
            || argument.len() > 4 * 1024
            || argument.chars().any(char::is_control)
            || (argument != "{file}" && argument.contains(['{', '}']))
        {
            return Err(ConfigError::Validation(
                "tools.editor arguments must be literal values with one complete {file} argument"
                    .into(),
            ));
        }
    }
    if config.action_targets.len() > 64 {
        return Err(ConfigError::Validation(
            "at most 64 action targets are supported".into(),
        ));
    }
    for (name, target) in &config.action_targets {
        validate_slug(name, "action target name")?;
        if target.subject_types.is_empty() || target.subject_types.len() > 4 {
            return Err(ConfigError::Validation(format!(
                "action target {name} must declare 1..=4 subject types"
            )));
        }
        let mut subjects = BTreeSet::new();
        for subject in &target.subject_types {
            if !matches!(subject.as_str(), "url" | "link" | "selection" | "tab")
                || !subjects.insert(subject.as_str())
            {
                return Err(ConfigError::Validation(format!(
                    "action target {name} has an unsupported or duplicate subject type"
                )));
            }
        }
        if target.executable.is_empty()
            || target.executable.len() > 256
            || target.executable.starts_with('-')
            || target.executable.chars().any(char::is_control)
        {
            return Err(ConfigError::Validation(format!(
                "action target {name} has an invalid executable"
            )));
        }
        if target.argv.len() > 256 {
            return Err(ConfigError::Validation(format!(
                "action target {name} has too many argv values"
            )));
        }
        for argument in &target.argv {
            if argument.is_empty()
                || argument.len() > 4 * 1024
                || argument.chars().any(char::is_control)
            {
                return Err(ConfigError::Validation(format!(
                    "action target {name} has an invalid argv value"
                )));
            }
            if argument.contains('{') || argument.contains('}') {
                if !matches!(argument.as_str(), "{url}" | "{title}" | "{selection}") {
                    return Err(ConfigError::Validation(format!(
                        "action target {name} uses an invalid or non-complete placeholder"
                    )));
                }
                let placeholder = argument.trim_matches(['{', '}']);
                if placeholder == "url"
                    && !subjects.contains("url")
                    && !subjects.contains("link")
                    && !subjects.contains("tab")
                {
                    return Err(ConfigError::Validation(format!(
                        "action target {name} declares {{url}} without a URL subject"
                    )));
                }
                if placeholder == "title"
                    && !subjects.contains("url")
                    && !subjects.contains("link")
                    && !subjects.contains("tab")
                {
                    return Err(ConfigError::Validation(format!(
                        "action target {name} declares {{title}} without a URL subject"
                    )));
                }
                if placeholder == "selection" && !subjects.contains("selection") {
                    return Err(ConfigError::Validation(format!(
                        "action target {name} declares {{selection}} without a selection subject"
                    )));
                }
            }
        }
    }
    if !config
        .search_engines
        .contains_key(&config.navigation.default_search)
    {
        return Err(ConfigError::Validation(
            "navigation.default_search is not registered".into(),
        ));
    }
    if !(0.25..=5.0).contains(&config.content.zoom)
        || !(1..=60).contains(&config.session.checkpoint_seconds)
    {
        return Err(ConfigError::Validation(
            "content.zoom or session.checkpoint_seconds is outside its allowed range".into(),
        ));
    }
    validate_spellcheck_languages(&config.spellcheck.languages)?;
    if !(0..=1000).contains(&config.tabs.undo_limit)
        || !(10..=1000).contains(&config.switcher.max_results)
        || !(0..=3650).contains(&config.history.retention_days)
        || !(0..=10_000).contains(&config.history.command_limit)
    {
        return Err(ConfigError::Validation(
            "retention, undo, switcher, or history limits are outside their allowed range".into(),
        ));
    }
    if !(1..=168).contains(&config.blocking.update_interval_hours)
        || !(1..=50).contains(&config.logging.max_file_mib)
        || !(1..=10).contains(&config.logging.retained_files)
    {
        return Err(ConfigError::Validation(
            "blocking or logging limits are outside their allowed range".into(),
        ));
    }
    match (
        config.links.cleaning.update_source.as_deref(),
        config.links.cleaning.update_sha256.as_deref(),
    ) {
        (Some(source), Some(checksum)) => {
            if !valid_link_update_source(source) || !valid_sha256(checksum) {
                return Err(ConfigError::Validation(
                    "links.cleaning update source/checksum is invalid".into(),
                ));
            }
        }
        (Some(_), None) => {
            return Err(ConfigError::Validation(
                "links.cleaning.update_sha256 is required with update_source".into(),
            ));
        }
        (None, Some(_)) => {
            return Err(ConfigError::Validation(
                "links.cleaning.update_source is required with update_sha256".into(),
            ));
        }
        (None, None) => {}
    }
    if config.blocking.bypass_sites.len() > 64 {
        return Err(ConfigError::Validation(
            "at most 64 blocking bypass sites are supported".into(),
        ));
    }
    let mut bypass_sites = BTreeSet::new();
    for site in &config.blocking.bypass_sites {
        if !valid_blocking_site_pattern(site) || !bypass_sites.insert(site) {
            return Err(ConfigError::Validation(
                "blocking bypass sites must be unique valid host patterns".into(),
            ));
        }
    }
    if config.blocking.security_deny_hosts.len() > 64 {
        return Err(ConfigError::Validation(
            "at most 64 security deny hosts are supported".into(),
        ));
    }
    let mut security_deny_hosts = BTreeSet::new();
    for site in &config.blocking.security_deny_hosts {
        if !valid_blocking_site_pattern(site) || !security_deny_hosts.insert(site) {
            return Err(ConfigError::Validation(
                "security deny hosts must be unique valid host patterns".into(),
            ));
        }
    }
    if config.permissions.screen_capture == PermissionDecision::Allow {
        return Err(ConfigError::Validation(
            "screen capture cannot be globally allowed".into(),
        ));
    }
    let mut permission_rule_keys = BTreeSet::new();
    if config.permission_rules.len() > 256 {
        return Err(ConfigError::Validation(
            "at most 256 permission rules are supported".into(),
        ));
    }
    for rule in &config.permission_rules {
        validate_slug(&rule.id, "permission rule id")?;
        validate_slug(&rule.profile, "permission rule profile")?;
        validate_permission_origin(&rule.origin)?;
        if !matches!(
            rule.permission.as_str(),
            "camera" | "microphone" | "notifications" | "geolocation" | "clipboard" | "local-fonts"
        ) {
            return Err(ConfigError::Validation(format!(
                "permission rule {} uses an unsupported permission",
                rule.id
            )));
        }
        let key = (
            rule.profile.as_str(),
            rule.origin.to_ascii_lowercase(),
            rule.permission.as_str(),
        );
        if !permission_rule_keys.insert(key) {
            return Err(ConfigError::Validation(format!(
                "permission rule {} duplicates a profile/origin/type key",
                rule.id
            )));
        }
    }
    if config.site_rules.len() > 256 {
        return Err(ConfigError::Validation(
            "at most 256 site rules are supported".into(),
        ));
    }
    let mut site_rule_ids = BTreeSet::new();
    for rule in &config.site_rules {
        if validate_slug(&rule.id, "site rule id").is_err() || !site_rule_ids.insert(&rule.id) {
            return Err(ConfigError::Validation(
                "site rule IDs must be unique, nonempty, and lowercase slugs".into(),
            ));
        }
        parse_site_pattern(&rule.pattern).map_err(|_| {
            ConfigError::Validation(format!(
                "site rule {} has an invalid HTTP(S) pattern",
                rule.id
            ))
        })?;
        if rule.set.len() > 32 {
            return Err(ConfigError::Validation(format!(
                "site rule {} has too many settings",
                rule.id
            )));
        }
        for (key, value) in &rule.set {
            let valid = match key.as_str() {
                "input.entry_mode" => value
                    .as_str()
                    .is_some_and(|value| matches!(value, "normal" | "insert" | "pass-through")),
                "content.javascript" | "content.images" | "content.force_dark" => {
                    value.as_bool().is_some()
                }
                "content.autoplay" => value
                    .as_str()
                    .is_some_and(|value| matches!(value, "engine-default" | "require-gesture")),
                "content.zoom" => {
                    value
                        .as_float()
                        .is_some_and(|value| (0.25..=5.0).contains(&value))
                        || value
                            .as_integer()
                            .is_some_and(|value| (1..=5).contains(&value))
                }
                _ => false,
            };
            if !setting_supports_site_scope(key) || !valid {
                return Err(ConfigError::Validation(format!(
                    "site rule {} has an invalid or unsupported setting {key}",
                    rule.id
                )));
            }
        }
    }
    for (mode, bindings) in &config.bindings {
        if !matches!(
            mode.as_str(),
            "normal" | "insert" | "command" | "search" | "hint" | "caret" | "pass-through"
        ) {
            return Err(ConfigError::Validation(format!(
                "bindings mode {mode:?} is not supported"
            )));
        }
        for (keychain, command) in bindings {
            if keychain.is_empty() || keychain.len() > 128 || keychain.chars().any(char::is_control)
            {
                return Err(ConfigError::Validation(format!(
                    "binding keychain in mode {mode:?} is empty, too long, or contains a control character"
                )));
            }
            if command.is_empty() || command.len() > 256 || command.chars().any(char::is_control) {
                return Err(ConfigError::Validation(format!(
                    "binding {keychain:?} in mode {mode:?} has an invalid command"
                )));
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_profile_definition(profile: &ProfileDefinition) -> Result<(), ConfigError> {
    validate_slug(&profile.name, "profile name")?;
    if profile.label.trim().is_empty()
        || profile.label.len() > 128
        || profile.label.chars().any(char::is_control)
    {
        return Err(ConfigError::Validation(format!(
            "profile {} has an invalid label",
            profile.name
        )));
    }
    if profile.overrides.len() > 128 {
        return Err(ConfigError::Validation(format!(
            "profile {} has too many overrides",
            profile.name
        )));
    }
    for (key, value) in &profile.overrides {
        validate_runtime_key(key)?;
        if matches!(
            key.as_str(),
            "schema_version" | "include" | "bindings" | "site_rules" | "permission_rules"
        ) || key.starts_with("site_rules.")
            || key.starts_with("permission_rules.")
        {
            return Err(ConfigError::Validation(format!(
                "profile {} cannot override configuration key {key}",
                profile.name
            )));
        }
        if value.to_string().len() > MAX_RUNTIME_VALUE_BYTES {
            return Err(ConfigError::Validation(format!(
                "profile {} override {key} is too large",
                profile.name
            )));
        }
    }
    let overrides = RuntimeOverrides {
        settings: profile.overrides.clone(),
        ..RuntimeOverrides::default()
    };
    validate_runtime_overrides(&overrides)?;
    let _ = apply_runtime_overrides(&Config::default(), &overrides)?;
    Ok(())
}

pub(crate) fn validate_profiles(config: &ProfilesConfig) -> Result<(), ConfigError> {
    if config.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(ConfigError::Validation(format!(
            "unsupported profiles schema_version {}",
            config.schema_version
        )));
    }
    if config.profiles.len() > 64 {
        return Err(ConfigError::Validation(
            "at most 64 profile definitions are supported".into(),
        ));
    }
    let mut names = BTreeSet::new();
    let mut defaults = 0;
    for profile in &config.profiles {
        validate_profile_definition(profile)?;
        if !names.insert(&profile.name) {
            return Err(ConfigError::Validation(format!(
                "duplicate profile name: {}",
                profile.name
            )));
        }
        if profile.default {
            defaults += 1;
        }
    }
    if defaults > 1 {
        return Err(ConfigError::Validation(
            "at most one profile can be marked default".into(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
pub(crate) fn validate_contexts(config: &ContextsConfig) -> Result<(), ConfigError> {
    const ALLOWED_ROUTE_ENTRY_POINTS: [&str; 3] =
        ["external-open", "explicit-open", "typed-initial-url"];
    if config.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(ConfigError::Validation(format!(
            "unsupported contexts schema_version {}",
            config.schema_version
        )));
    }
    let mut names = BTreeSet::new();
    for context in &config.contexts {
        validate_slug(&context.name, "context name")?;
        if context.label.is_empty()
            || context.label.len() > 128
            || context.label.chars().any(char::is_control)
        {
            return Err(ConfigError::Validation(
                "context label is empty, too long, or contains a control character".into(),
            ));
        }
        validate_slug(&context.profile, "context profile")?;
        if !names.insert(&context.name) {
            return Err(ConfigError::Validation(format!(
                "duplicate context name: {}",
                context.name
            )));
        }
        if context.sessions.len() > 128
            || context
                .sessions
                .iter()
                .any(|session| validate_slug(session, "context session").is_err())
        {
            return Err(ConfigError::Validation(format!(
                "context {} has invalid session references",
                context.name
            )));
        }
        if let Some(workspace) = context.workspace.as_deref()
            && (workspace.is_empty()
                || workspace.len() > 128
                || workspace.chars().any(char::is_control))
        {
            return Err(ConfigError::Validation(format!(
                "context {} has an invalid workspace selector",
                context.name
            )));
        }
        if let Some(accent) = context.accent.as_deref()
            && !valid_accent(accent)
        {
            return Err(ConfigError::Validation(format!(
                "context {} has an invalid accent",
                context.name
            )));
        }
        if !matches!(
            context.default_target.as_str(),
            "reuse" | "reuse-or-window" | "window"
        ) {
            return Err(ConfigError::Validation(format!(
                "context {} has an invalid default_target",
                context.name
            )));
        }
    }
    let mut route_ids = BTreeSet::new();
    for route in &config.routes {
        if route.id.is_empty()
            || route.id.len() > 64
            || route.id.chars().any(char::is_control)
            || !route_ids.insert(&route.id)
        {
            return Err(ConfigError::Validation(
                "context route IDs must be unique, nonempty, and bounded".into(),
            ));
        }
        if route.pattern.is_empty()
            || route.pattern.len() > 512
            || route.pattern.chars().any(char::is_control)
            || parse_context_pattern(&route.pattern).is_none()
        {
            return Err(ConfigError::Validation(format!(
                "context route {} has an invalid HTTP(S) match pattern",
                route.id
            )));
        }
        if !names.contains(&route.context) {
            return Err(ConfigError::Validation(format!(
                "context route {} references unknown context {}",
                route.id, route.context
            )));
        }
        if !matches!(route.behavior.as_str(), "prompt" | "suggest") {
            return Err(ConfigError::Validation(format!(
                "context route {} behavior must be prompt or suggest",
                route.id
            )));
        }
        if route.entry_points.len() > 8
            || route.entry_points.iter().any(|entry| {
                entry.is_empty()
                    || entry.len() > 64
                    || entry.chars().any(char::is_control)
                    || !ALLOWED_ROUTE_ENTRY_POINTS.contains(&entry.as_str())
            })
        {
            return Err(ConfigError::Validation(format!(
                "context route {} has invalid entry points; allowed values are external-open, explicit-open, and typed-initial-url",
                route.id
            )));
        }
    }
    for (index, route) in config.routes.iter().enumerate() {
        if config.routes.iter().skip(index + 1).any(|other| {
            other.pattern == route.pattern
                && other.priority == route.priority
                && other.context != route.context
                && route.entry_points.iter().any(|entry| {
                    other
                        .entry_points
                        .iter()
                        .any(|candidate| candidate == entry)
                })
        }) {
            return Err(ConfigError::Validation(format!(
                "context route {} conflicts with another route using pattern {:?} and priority {}",
                route.id, route.pattern, route.priority
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_slug(value: &str, label: &str) -> Result<(), ConfigError> {
    if value.is_empty()
        || value.len() > 32
        || value.chars().any(|character| {
            !(character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-')
        })
    {
        Err(ConfigError::Validation(format!(
            "{label} must be a 1–32 character lowercase slug"
        )))
    } else {
        Ok(())
    }
}

fn validate_permission_origin(origin: &str) -> Result<(), ConfigError> {
    let (scheme, authority) = origin.split_once("://").ok_or_else(|| {
        ConfigError::Validation("permission rule origin must have a scheme".into())
    })?;
    if scheme != "https" && scheme != "http" {
        return Err(ConfigError::Validation(
            "permission rule origin must use HTTP or HTTPS".into(),
        ));
    }
    if authority.is_empty()
        || authority.contains(['/', '?', '#', '@', '*'])
        || authority.contains(':') && !authority.starts_with('[')
    {
        return Err(ConfigError::Validation(
            "permission rule origin must be an exact authority".into(),
        ));
    }
    let host = if authority.starts_with('[') {
        authority
            .split_once(']')
            .map_or(authority, |(host, _)| host.trim_start_matches('['))
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    if host.is_empty() || scheme == "http" && !matches!(host, "localhost" | "127.0.0.1" | "::1") {
        return Err(ConfigError::Validation(
            "permission rule HTTP origins must be loopback".into(),
        ));
    }
    Ok(())
}

fn valid_blocking_site_pattern(pattern: &str) -> bool {
    if pattern.is_empty()
        || pattern.len() > 253
        || pattern.contains(['/', '\\', '@', '?', '#'])
        || pattern.contains("..")
        || pattern.starts_with('.')
        || pattern.ends_with('.')
    {
        return false;
    }
    let wildcard_count = pattern.matches('*').count();
    if wildcard_count > 1 || wildcard_count == 1 && !pattern.starts_with("*.") {
        return false;
    }
    let host = pattern.strip_prefix("*.").unwrap_or(pattern);
    !host.is_empty()
        && host.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '.' | ':')
        })
}

fn valid_accent(value: &str) -> bool {
    matches!(value.len(), 7 | 9)
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn validate_spellcheck_languages(languages: &[String]) -> Result<(), ConfigError> {
    if languages.is_empty() || languages.len() > 16 {
        return Err(ConfigError::Validation(
            "spellcheck.languages must contain 1..=16 language tags".into(),
        ));
    }

    let mut seen = BTreeSet::new();
    for language in languages {
        if language == "system" {
            if !seen.insert(language.to_ascii_lowercase()) {
                return Err(ConfigError::Validation(
                    "spellcheck.languages must not contain duplicate tags".into(),
                ));
            }
            continue;
        }
        if language.eq_ignore_ascii_case("system") {
            return Err(ConfigError::Validation(
                "spellcheck language uses the reserved system token with the wrong case".into(),
            ));
        }
        if !valid_bcp47_tag(language) {
            return Err(ConfigError::Validation(format!(
                "spellcheck language {language:?} is not a valid BCP 47 tag"
            )));
        }
        if !seen.insert(language.to_ascii_lowercase()) {
            return Err(ConfigError::Validation(
                "spellcheck.languages must not contain duplicate tags".into(),
            ));
        }
    }
    Ok(())
}

const BCP47_GRANDFATHERED: &[&str] = &[
    "art-lojban",
    "cel-gaulish",
    "en-gb-oed",
    "i-ami",
    "i-bnn",
    "i-default",
    "i-enochian",
    "i-hak",
    "i-klingon",
    "i-lux",
    "i-mingo",
    "i-navajo",
    "i-pwn",
    "i-tao",
    "i-tay",
    "i-tsu",
    "no-bok",
    "no-nyn",
    "sgn-be-fr",
    "sgn-be-nl",
    "sgn-ch-de",
    "zh-guoyu",
    "zh-hakka",
    "zh-min",
    "zh-min-nan",
    "zh-xiang",
];

/// Checks the bounded BCP 47 structural grammar used by Qt spellcheck.
/// Registry membership is deliberately not required: dictionary providers can
/// legitimately ship private-use or newly registered language identifiers.
fn valid_bcp47_tag(language: &str) -> bool {
    if language.is_empty()
        || language.len() > 64
        || language.starts_with('-')
        || language.ends_with('-')
        || language.contains("--")
    {
        return false;
    }
    if BCP47_GRANDFATHERED.contains(&language.to_ascii_lowercase().as_str()) {
        return true;
    }
    let parts = language.split('-').collect::<Vec<_>>();
    let primary = parts.first().copied().unwrap_or_default();
    if primary.eq_ignore_ascii_case("x") {
        return parts.len() > 1
            && parts[1..].iter().all(|part| {
                (1..=8).contains(&part.len())
                    && part
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
            });
    }
    valid_bcp47_standard_parts(&parts)
}

fn valid_bcp47_standard_parts(parts: &[&str]) -> bool {
    let primary = parts.first().copied().unwrap_or_default();
    if !(2..=8).contains(&primary.len())
        || !primary
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return false;
    }

    let mut index = 1;
    let mut extlang_count = 0;
    while primary.len() <= 3
        && extlang_count < 3
        && parts.get(index).is_some_and(|part| {
            part.len() == 3
                && part
                    .chars()
                    .all(|character| character.is_ascii_alphabetic())
        })
    {
        index += 1;
        extlang_count += 1;
    }
    if parts.get(index).is_some_and(|part| {
        part.len() == 4
            && part
                .chars()
                .all(|character| character.is_ascii_alphabetic())
    }) {
        index += 1;
    }
    if parts.get(index).is_some_and(|part| {
        (part.len() == 2
            && part
                .chars()
                .all(|character| character.is_ascii_alphabetic()))
            || (part.len() == 3 && part.chars().all(|character| character.is_ascii_digit()))
    }) {
        index += 1;
    }
    while parts.get(index).is_some_and(|part| {
        (5..=8).contains(&part.len())
            && part
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
            || part.len() == 4
                && part
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_digit())
                && part[1..]
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
    }) {
        index += 1;
    }
    while let Some(part) = parts.get(index).copied() {
        if part.len() != 1
            || !part
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            return false;
        }
        if part.eq_ignore_ascii_case("x") {
            return index + 1 < parts.len()
                && parts[index + 1..].iter().all(|private_part| {
                    (1..=8).contains(&private_part.len())
                        && private_part
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric())
                });
        }
        index += 1;
        let extension_start = index;
        while parts.get(index).is_some_and(|extension_part| {
            (2..=8).contains(&extension_part.len())
                && extension_part
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        }) {
            index += 1;
        }
        if index == extension_start {
            return false;
        }
    }
    index == parts.len()
}

pub(super) fn validate_runtime_key(key: &str) -> Result<(), ConfigError> {
    if key.is_empty()
        || key.len() > MAX_RUNTIME_KEY_BYTES
        || key.starts_with('.')
        || key.ends_with('.')
        || key.split('.').any(str::is_empty)
        || key.chars().any(char::is_control)
    {
        return Err(ConfigError::Validation(
            "runtime override key is empty, too long, or unsafe".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_runtime_overrides(overrides: &RuntimeOverrides) -> Result<(), ConfigError> {
    if overrides.schema_version != RUNTIME_SCHEMA_VERSION {
        return Err(ConfigError::Validation(format!(
            "unsupported runtime override schema_version {}",
            overrides.schema_version
        )));
    }
    if overrides.settings.len() > MAX_RUNTIME_SETTINGS {
        return Err(ConfigError::Validation(
            "at most 256 runtime settings are supported".into(),
        ));
    }
    for (key, value) in &overrides.settings {
        validate_runtime_key(key)?;
        if value.to_string().len() > MAX_RUNTIME_VALUE_BYTES {
            return Err(ConfigError::Validation(format!(
                "runtime override value for {key:?} is too large"
            )));
        }
    }
    if overrides.bindings.len() > MAX_RUNTIME_SETTINGS {
        return Err(ConfigError::Validation(
            "at most 256 runtime binding modes are supported".into(),
        ));
    }
    for (mode, bindings) in &overrides.bindings {
        if bindings.len() > MAX_RUNTIME_SETTINGS {
            return Err(ConfigError::Validation(format!(
                "runtime binding mode {mode:?} has too many entries"
            )));
        }
        for (key, command) in bindings {
            validate_runtime_binding(mode, key, command)?;
        }
    }
    if overrides.site_rules.len() > MAX_RUNTIME_SETTINGS {
        return Err(ConfigError::Validation(
            "at most 256 runtime site rules are supported".into(),
        ));
    }
    let mut site_rule_ids = BTreeSet::new();
    for rule in &overrides.site_rules {
        validate_site_rule(rule)?;
        if !site_rule_ids.insert(&rule.id) {
            return Err(ConfigError::Validation(
                "runtime site rule IDs must be unique".into(),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_runtime_site_pattern(pattern: &str) -> Result<(), ConfigError> {
    parse_site_pattern(pattern).map_err(|_| {
        ConfigError::Validation("runtime site pattern must be a bounded HTTP(S) pattern".into())
    })?;
    Ok(())
}

pub(super) fn runtime_site_rule_id(pattern: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in pattern.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("runtime-site-{hash:016x}")
}

pub(super) fn validate_runtime_binding(
    mode: &str,
    keychain: &str,
    command: &str,
) -> Result<(), ConfigError> {
    if !matches!(
        mode,
        "normal" | "insert" | "command" | "caret" | "search" | "hint" | "pass-through"
    ) {
        return Err(ConfigError::Validation(format!(
            "runtime binding mode {mode:?} is not supported"
        )));
    }
    if keychain.is_empty() || keychain.len() > 128 || keychain.chars().any(char::is_control) {
        return Err(ConfigError::Validation(
            "runtime binding keychain is empty, too long, or unsafe".into(),
        ));
    }
    if command.len() > 256 || command.chars().any(char::is_control) {
        return Err(ConfigError::Validation(
            "runtime binding command is too long or unsafe".into(),
        ));
    }
    Ok(())
}
