//! Configuration composition, overrides, loading, and persistence.

use super::{
    ActionTargetConfig, BTreeMap, BTreeSet, BlockingConfig, CURRENT_SCHEMA_VERSION, ContentConfig,
    ContextsConfig, Deserialize, DesktopConfig, DiscoveryConfig, DownloadsConfig, HintsConfig,
    HistoryConfig, HyprlandConfig, InputConfig, IpcConfig, LinksConfig, LoggingConfig,
    MAX_INCLUDE_DEPTH, MAX_RUNTIME_SETTINGS, MAX_RUNTIME_VALUE_BYTES, MAX_TOTAL_BYTES,
    NavigationConfig, Path, PathBuf, PermissionRuleConfig, PermissionsConfig, PrivacyConfig,
    ProfileDefinition, ProfilesConfig, RUNTIME_SCHEMA_VERSION, Serialize, SessionConfig,
    SiteDoctorConfig, SiteRule, SpellcheckConfig, SwitcherConfig, SystemTime, TabsConfig,
    ThemeConfig, ToolsConfig, UNIX_EPOCH, UiConfig, Write, default_search_engines, fs, io_error,
    runtime_site_rule_id, set_private_file_mode, set_private_permissions,
    setting_supports_site_scope, validate, validate_contexts, validate_profile_definition,
    validate_profiles, validate_runtime_binding, validate_runtime_key, validate_runtime_overrides,
    validate_runtime_site_pattern,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u64,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub input: InputConfig,
    #[serde(default)]
    pub hints: HintsConfig,
    #[serde(default)]
    pub discovery: DiscoveryConfig,
    #[serde(default)]
    pub navigation: NavigationConfig,
    #[serde(default)]
    pub links: LinksConfig,
    #[serde(default = "default_search_engines")]
    pub search_engines: BTreeMap<String, String>,
    #[serde(default)]
    pub tabs: TabsConfig,
    #[serde(default)]
    pub session: SessionConfig,
    #[serde(default)]
    pub switcher: SwitcherConfig,
    #[serde(default)]
    pub history: HistoryConfig,
    #[serde(default)]
    pub content: ContentConfig,
    #[serde(default)]
    pub privacy: PrivacyConfig,
    #[serde(default)]
    pub permissions: PermissionsConfig,
    #[serde(default)]
    pub blocking: BlockingConfig,
    #[serde(default)]
    pub downloads: DownloadsConfig,
    #[serde(default)]
    pub spellcheck: SpellcheckConfig,
    #[serde(default)]
    pub desktop: DesktopConfig,
    #[serde(default)]
    pub hyprland: HyprlandConfig,
    #[serde(default)]
    pub site_doctor: SiteDoctorConfig,
    #[serde(default)]
    pub ipc: IpcConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub action_targets: BTreeMap<String, ActionTargetConfig>,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub bindings: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub site_rules: Vec<SiteRule>,
    #[serde(default)]
    pub permission_rules: Vec<PermissionRuleConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            include: Vec::new(),
            ui: UiConfig::default(),
            theme: ThemeConfig::default(),
            input: InputConfig::default(),
            hints: HintsConfig::default(),
            discovery: DiscoveryConfig::default(),
            navigation: NavigationConfig::default(),
            links: LinksConfig::default(),
            search_engines: default_search_engines(),
            tabs: TabsConfig::default(),
            session: SessionConfig::default(),
            switcher: SwitcherConfig::default(),
            history: HistoryConfig::default(),
            content: ContentConfig::default(),
            privacy: PrivacyConfig::default(),
            permissions: PermissionsConfig::default(),
            blocking: BlockingConfig::default(),
            downloads: DownloadsConfig::default(),
            spellcheck: SpellcheckConfig::default(),
            desktop: DesktopConfig::default(),
            hyprland: HyprlandConfig::default(),
            site_doctor: SiteDoctorConfig::default(),
            ipc: IpcConfig::default(),
            tools: ToolsConfig::default(),
            action_targets: BTreeMap::new(),
            logging: LoggingConfig::default(),
            bindings: BTreeMap::new(),
            site_rules: Vec::new(),
            permission_rules: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigError {
    Io { path: PathBuf, message: String },
    Parse { path: PathBuf, message: String },
    IncludeDepth { path: PathBuf },
    IncludeCycle { path: PathBuf },
    IncludeRepeated { path: PathBuf },
    SizeLimit,
    Validation(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } | Self::Parse { path, message } => {
                write!(formatter, "{}: {message}", path.display())
            }
            Self::IncludeDepth { path } => write!(
                formatter,
                "include depth exceeds {MAX_INCLUDE_DEPTH}: {}",
                path.display()
            ),
            Self::IncludeCycle { path } => write!(formatter, "include cycle at {}", path.display()),
            Self::IncludeRepeated { path } => {
                write!(formatter, "include repeated: {}", path.display())
            }
            Self::SizeLimit => formatter.write_str("configuration includes exceed 2 MiB"),
            Self::Validation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedConfig {
    pub config: Config,
    pub sources: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConfigStore {
    current: Option<LoadedConfig>,
    revision: u64,
}

/// Generated settings and bindings written by interactive configuration
/// commands. This document is intentionally separate from the user-edited
/// configuration so an interactive change can never rewrite lower layers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeOverrides {
    pub schema_version: u64,
    #[serde(default)]
    pub settings: BTreeMap<String, toml::Value>,
    /// Explicitly saved bindings, including an empty string for an unbound
    /// key. An empty value therefore masks a lower-layer binding instead of
    /// deleting it from the generated document.
    #[serde(default)]
    pub bindings: BTreeMap<String, BTreeMap<String, String>>,
    /// Generated site rules written by interactive `set --pattern` commands.
    #[serde(default)]
    pub site_rules: Vec<SiteRule>,
}

impl Default for RuntimeOverrides {
    fn default() -> Self {
        Self {
            schema_version: RUNTIME_SCHEMA_VERSION,
            settings: BTreeMap::new(),
            bindings: BTreeMap::new(),
            site_rules: Vec::new(),
        }
    }
}

impl RuntimeOverrides {
    /// Adds or replaces a typed TOML setting value.
    ///
    /// # Errors
    ///
    /// Returns an error when the key, value, or override document exceeds its
    /// validation bounds.
    pub fn set_setting(&mut self, key: &str, value: toml::Value) -> Result<(), ConfigError> {
        validate_runtime_key(key)?;
        let encoded = value.to_string();
        if encoded.len() > MAX_RUNTIME_VALUE_BYTES {
            return Err(ConfigError::Validation(
                "runtime override value is too large".into(),
            ));
        }
        if !value.is_datetime()
            && !value.is_float()
            && !value.is_integer()
            && !value.is_bool()
            && !value.is_str()
            && !value.is_array()
        {
            return Err(ConfigError::Validation(
                "runtime override value must be a TOML scalar or array".into(),
            ));
        }
        self.settings.insert(key.to_owned(), value);
        validate_runtime_overrides(self)
    }

    /// Parses the value portion of a `KEY=VALUE` command argument. TOML
    /// literals are preferred; an unquoted value is retained as a string.
    ///
    /// # Errors
    ///
    /// Returns an error for empty/control-containing input or an invalid
    /// override key.
    pub fn set_literal(&mut self, key: &str, literal: &str) -> Result<(), ConfigError> {
        if literal.is_empty() || literal.chars().any(char::is_control) {
            return Err(ConfigError::Validation(
                "runtime override value must be nonempty and contain no controls".into(),
            ));
        }
        let value = toml::from_str::<toml::Value>(&format!("value = {literal}"))
            .ok()
            .and_then(|table| table.get("value").cloned())
            .unwrap_or_else(|| toml::Value::String(literal.to_owned()));
        self.set_setting(key, value)
    }

    /// Removes a generated setting. Lower-layer configuration remains intact
    /// and becomes effective after the next configuration application.
    ///
    /// # Errors
    ///
    /// Returns an error when the key is invalid.
    pub fn unset_setting(&mut self, key: &str) -> Result<bool, ConfigError> {
        validate_runtime_key(key)?;
        Ok(self.settings.remove(key).is_some())
    }

    /// Adds or replaces a generated site-scoped setting for a validated
    /// pattern. Runtime rules use the highest priority so they remain above
    /// authored site rules, matching CONFIG-002's generated-override layer.
    pub fn set_site_literal(
        &mut self,
        pattern: &str,
        key: &str,
        literal: &str,
    ) -> Result<(), ConfigError> {
        if literal.is_empty() || literal.chars().any(char::is_control) {
            return Err(ConfigError::Validation(
                "runtime site value must be nonempty and contain no controls".into(),
            ));
        }
        let value = toml::from_str::<toml::Value>(&format!("value = {literal}"))
            .ok()
            .and_then(|table| table.get("value").cloned())
            .unwrap_or_else(|| toml::Value::String(literal.to_owned()));
        self.set_site_value(pattern, key, value)
    }

    fn set_site_value(
        &mut self,
        pattern: &str,
        key: &str,
        value: toml::Value,
    ) -> Result<(), ConfigError> {
        validate_runtime_site_pattern(pattern)?;
        if !setting_supports_site_scope(key) {
            return Err(ConfigError::Validation(format!(
                "configuration key {key} does not support site scope"
            )));
        }
        let index = if let Some(index) = self
            .site_rules
            .iter()
            .position(|rule| rule.pattern == pattern)
        {
            index
        } else {
            if self.site_rules.len() >= MAX_RUNTIME_SETTINGS {
                return Err(ConfigError::Validation(
                    "at most 256 runtime site rules are supported".into(),
                ));
            }
            self.site_rules.push(SiteRule {
                id: runtime_site_rule_id(pattern),
                pattern: pattern.to_owned(),
                priority: i32::MAX,
                set: BTreeMap::new(),
            });
            self.site_rules.len() - 1
        };
        self.site_rules[index].set.insert(key.to_owned(), value);
        validate_runtime_overrides(self)
    }

    /// Removes one generated setting and deletes an empty generated rule.
    pub fn unset_site_setting(&mut self, pattern: &str, key: &str) -> Result<bool, ConfigError> {
        validate_runtime_site_pattern(pattern)?;
        if !setting_supports_site_scope(key) {
            return Err(ConfigError::Validation(format!(
                "configuration key {key} does not support site scope"
            )));
        }
        let Some(index) = self
            .site_rules
            .iter()
            .position(|rule| rule.pattern == pattern)
        else {
            return Ok(false);
        };
        let changed = self.site_rules[index].set.remove(key).is_some();
        if self.site_rules[index].set.is_empty() {
            self.site_rules.remove(index);
        }
        validate_runtime_overrides(self)?;
        Ok(changed)
    }

    /// Adds or replaces a generated binding. An empty command is the explicit
    /// unbound marker and intentionally masks a lower-layer binding.
    ///
    /// # Errors
    ///
    /// Returns an error when the mode, keychain, command, or document bounds
    /// are invalid.
    pub fn set_binding(
        &mut self,
        mode: &str,
        keychain: &str,
        command: &str,
    ) -> Result<(), ConfigError> {
        validate_runtime_binding(mode, keychain, command)?;
        self.bindings
            .entry(mode.to_owned())
            .or_default()
            .insert(keychain.to_owned(), command.to_owned());
        validate_runtime_overrides(self)
    }
}

/// Loads the generated runtime override document. A missing file is the
/// normal first-run state and returns an empty document.
///
/// # Errors
///
/// Returns an error when the file is unreadable, oversized, malformed, or
/// violates the runtime override bounds.
pub fn load_runtime_overrides(path: impl AsRef<Path>) -> Result<RuntimeOverrides, ConfigError> {
    let path = path.as_ref();
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RuntimeOverrides::default());
        }
        Err(error) => return Err(io_error(path, &error)),
    };
    if bytes.len() as u64 > MAX_TOTAL_BYTES {
        return Err(ConfigError::SizeLimit);
    }
    let text = String::from_utf8(bytes).map_err(|error| ConfigError::Parse {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    let overrides: RuntimeOverrides =
        toml::from_str(&text).map_err(|error| ConfigError::Parse {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    validate_runtime_overrides(&overrides)?;
    Ok(overrides)
}

/// Atomically writes generated runtime overrides with private permissions.
/// The user configuration file is never opened or rewritten.
///
/// # Errors
///
/// Returns an error when validation, serialization, directory preparation, or
/// the synchronized atomic write fails.
pub fn save_runtime_overrides_atomic(
    path: impl AsRef<Path>,
    overrides: &RuntimeOverrides,
) -> Result<(), ConfigError> {
    let path = path.as_ref();
    validate_runtime_overrides(overrides)?;
    let bytes = toml::to_string_pretty(overrides)
        .map_err(|error| {
            ConfigError::Validation(format!("could not serialize overrides: {error}"))
        })?
        .into_bytes();
    if bytes.len() as u64 > MAX_TOTAL_BYTES {
        return Err(ConfigError::SizeLimit);
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| io_error(parent, &error))?;
    set_private_permissions(parent).map_err(|error| io_error(parent, &error))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ConfigError::Validation("runtime override filename is invalid".into()))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let temporary = parent.join(format!(
        ".{file_name}.tmp-{}-{timestamp}",
        std::process::id()
    ));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        set_private_file_mode(&mut options);
        let mut file = options
            .open(&temporary)
            .map_err(|error| io_error(&temporary, &error))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| io_error(&temporary, &error))?;
        fs::rename(&temporary, path).map_err(|error| io_error(path, &error))?;
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| io_error(parent, &error))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Applies generated settings over a validated configuration and validates
/// the resulting typed document before returning it.
///
/// # Errors
///
/// Returns an error when an override is invalid, names no existing setting, or
/// produces a configuration that fails typed validation.
pub fn apply_runtime_overrides(
    config: &Config,
    overrides: &RuntimeOverrides,
) -> Result<Config, ConfigError> {
    validate_runtime_overrides(overrides)?;
    let mut value = toml::Value::try_from(config).map_err(|error| {
        ConfigError::Validation(format!("could not encode configuration: {error}"))
    })?;
    for (key, override_value) in &overrides.settings {
        let parts = key.split('.').collect::<Vec<_>>();
        set_config_value(&mut value, &parts, override_value, key)?;
    }
    let mut updated: Config = value.try_into().map_err(|error: toml::de::Error| {
        ConfigError::Validation(format!("runtime override has the wrong type: {error}"))
    })?;
    for (mode, bindings) in &overrides.bindings {
        let layer = updated.bindings.entry(mode.clone()).or_default();
        for (keychain, command) in bindings {
            layer.insert(
                keychain.clone(),
                if command.is_empty() {
                    "unbound".into()
                } else {
                    command.clone()
                },
            );
        }
    }
    for rule in &overrides.site_rules {
        if let Some(index) = updated
            .site_rules
            .iter()
            .position(|candidate| candidate.id == rule.id)
        {
            updated.site_rules.remove(index);
        }
        updated.site_rules.push(rule.clone());
    }
    validate(&updated)?;
    Ok(updated)
}

/// Resolves the effective configuration in the documented layer order.
///
/// Profile defaults are applied first, followed by persisted runtime changes,
/// command-line overrides, and finally temporary overrides. Keeping this
/// precedence in the configuration crate prevents a presentation adapter from
/// becoming the source of configuration policy.
///
/// # Errors
///
/// Returns an error when any layer is invalid or the resulting configuration
/// fails typed validation.
pub fn resolve_runtime_override_layers(
    base: &Config,
    profile: &RuntimeOverrides,
    runtime: &RuntimeOverrides,
    command_line: &RuntimeOverrides,
    temporary: &RuntimeOverrides,
) -> Result<Config, ConfigError> {
    let profile = apply_runtime_overrides(base, profile)?;
    let runtime = apply_runtime_overrides(&profile, runtime)?;
    let command_line = apply_runtime_overrides(&runtime, command_line)?;
    apply_runtime_overrides(&command_line, temporary)
}

/// Returns the profile definition file adjacent to a primary configuration
/// file.
#[must_use]
pub fn profile_config_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("profiles.toml")
}

/// Loads the generated override layer for a named profile.
///
/// A missing profile document, or a profile absent from that document, is the
/// normal no-overrides state.
///
/// # Errors
///
/// Returns an error when the profile document cannot be loaded or its selected
/// profile cannot be converted to runtime overrides.
pub fn load_profile_runtime_overrides(
    config_path: &Path,
    profile_name: &str,
) -> Result<RuntimeOverrides, ConfigError> {
    let path = profile_config_path(config_path);
    if !path.exists() {
        return Ok(RuntimeOverrides::default());
    }
    let profiles = load_profiles(&path)?;
    let Some(profile) = profiles
        .profiles
        .iter()
        .find(|profile| profile.name == profile_name)
    else {
        return Ok(RuntimeOverrides::default());
    };
    profile_override_layer(profile)
}

fn set_config_value(
    cursor: &mut toml::Value,
    parts: &[&str],
    replacement: &toml::Value,
    key: &str,
) -> Result<(), ConfigError> {
    let table = cursor.as_table_mut().ok_or_else(|| {
        ConfigError::Validation(format!("configuration key is not a table path: {key}"))
    })?;
    let part = parts
        .first()
        .copied()
        .ok_or_else(|| ConfigError::Validation(format!("configuration key is empty: {key}")))?;
    if parts.len() == 1 {
        if !table.contains_key(part) {
            return Err(ConfigError::Validation(format!(
                "configuration key not found: {key}"
            )));
        }
        table.insert(part.to_owned(), replacement.clone());
        return Ok(());
    }
    let child = table
        .get_mut(part)
        .ok_or_else(|| ConfigError::Validation(format!("configuration key not found: {key}")))?;
    set_config_value(child, &parts[1..], replacement, key)
}

impl ConfigStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn current(&self) -> Option<&LoadedConfig> {
        self.current.as_ref()
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Commits a candidate only if loading and validation succeed.
    ///
    /// # Errors
    ///
    /// Returns the parse, include, or validation error and leaves the current
    /// committed configuration unchanged.
    pub fn reload(&mut self, path: impl AsRef<Path>) -> Result<&LoadedConfig, ConfigError> {
        let candidate = load(path)?;
        self.revision = self.revision.saturating_add(1);
        self.current = Some(candidate);
        Ok(self
            .current
            .as_ref()
            .unwrap_or_else(|| unreachable!("candidate was assigned")))
    }
}

/// Loads a root TOML file, its trusted local includes, and applies root values
/// last. Includes are relative to their including file and are bounded.
///
/// # Errors
///
/// Returns an error for unreadable, malformed, repeated, cyclic, or oversized
/// includes, unsupported schema values, and invalid settings.
pub fn load(path: impl AsRef<Path>) -> Result<LoadedConfig, ConfigError> {
    let root = path
        .as_ref()
        .canonicalize()
        .map_err(|error| io_error(path.as_ref(), &error))?;
    let mut state = LoadState {
        total_bytes: 0,
        active: BTreeSet::new(),
        seen: BTreeSet::new(),
        sources: Vec::new(),
    };
    let value = state.read_file(&root, 0)?;
    let config: Config = value
        .try_into()
        .map_err(|error: toml::de::Error| ConfigError::Parse {
            path: root.clone(),
            message: error.to_string(),
        })?;
    validate(&config)?;
    Ok(LoadedConfig {
        config,
        sources: state.sources,
    })
}

/// Loads and validates a separate user-maintained `contexts.toml` document.
///
/// Generated IDs and saved membership are deliberately not read or written by
/// this function; those belong to the storage registry.
///
/// # Errors
///
/// Returns an I/O, TOML, size, schema, or context/routing validation error.
pub fn load_contexts(path: impl AsRef<Path>) -> Result<ContextsConfig, ConfigError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| io_error(path, &error))?;
    if bytes.len() as u64 > MAX_TOTAL_BYTES {
        return Err(ConfigError::SizeLimit);
    }
    let text = String::from_utf8(bytes).map_err(|error| ConfigError::Parse {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    let config: ContextsConfig = toml::from_str(&text).map_err(|error| ConfigError::Parse {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    validate_contexts(&config)?;
    Ok(config)
}

/// Loads and validates a separate user-maintained `profiles.toml` document.
///
/// Profile definitions are data only. Their overrides are validated against
/// the typed configuration schema but are not persisted into `config.toml`.
///
/// # Errors
///
/// Returns an I/O, TOML, size, schema, or profile validation error.
pub fn load_profiles(path: impl AsRef<Path>) -> Result<ProfilesConfig, ConfigError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| io_error(path, &error))?;
    if bytes.len() as u64 > MAX_TOTAL_BYTES {
        return Err(ConfigError::SizeLimit);
    }
    let text = String::from_utf8(bytes).map_err(|error| ConfigError::Parse {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    let config: ProfilesConfig = toml::from_str(&text).map_err(|error| ConfigError::Parse {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    validate_profiles(&config)?;
    Ok(config)
}

/// Converts one profile's overrides into the same validated layer used by
/// runtime configuration. This keeps profile values typed and makes unknown
/// or site-only fields fail before startup or reload can commit them.
pub fn profile_override_layer(
    profile: &ProfileDefinition,
) -> Result<RuntimeOverrides, ConfigError> {
    validate_profile_definition(profile)?;
    let overrides = RuntimeOverrides {
        settings: profile.overrides.clone(),
        ..RuntimeOverrides::default()
    };
    validate_runtime_overrides(&overrides)?;
    let _ = apply_runtime_overrides(&Config::default(), &overrides)?;
    Ok(overrides)
}

/// Atomically writes the validated user-maintained `contexts.toml` document.
///
/// The command surface uses this only for explicit route-management actions;
/// generated context membership remains owned by the storage registry.
///
/// # Errors
///
/// Returns an I/O, size, serialization, or context/routing validation error.
pub fn save_contexts_atomic(
    path: impl AsRef<Path>,
    config: &ContextsConfig,
) -> Result<(), ConfigError> {
    let path = path.as_ref();
    validate_contexts(config)?;
    let bytes = toml::to_string_pretty(config)
        .map_err(|error| ConfigError::Validation(format!("could not serialize contexts: {error}")))?
        .into_bytes();
    if bytes.len() as u64 > MAX_TOTAL_BYTES {
        return Err(ConfigError::SizeLimit);
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| io_error(parent, &error))?;
    set_private_permissions(parent).map_err(|error| io_error(parent, &error))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ConfigError::Validation("contexts filename is invalid".into()))?;
    let temporary = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        set_private_file_mode(&mut options);
        let mut file = options
            .open(&temporary)
            .map_err(|error| io_error(&temporary, &error))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| io_error(&temporary, &error))?;
        fs::rename(&temporary, path).map_err(|error| io_error(path, &error))?;
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| io_error(parent, &error))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

struct LoadState {
    total_bytes: u64,
    active: BTreeSet<PathBuf>,
    seen: BTreeSet<PathBuf>,
    sources: Vec<PathBuf>,
}

impl LoadState {
    fn read_file(&mut self, path: &Path, depth: usize) -> Result<toml::Value, ConfigError> {
        if depth >= MAX_INCLUDE_DEPTH {
            return Err(ConfigError::IncludeDepth {
                path: path.to_owned(),
            });
        }
        let canonical = path
            .canonicalize()
            .map_err(|error| io_error(path, &error))?;
        if !self.active.insert(canonical.clone()) {
            return Err(ConfigError::IncludeCycle { path: canonical });
        }
        if !self.seen.insert(canonical.clone()) {
            return Err(ConfigError::IncludeRepeated { path: canonical });
        }
        let bytes = fs::read(&canonical).map_err(|error| io_error(&canonical, &error))?;
        self.total_bytes = self.total_bytes.saturating_add(bytes.len() as u64);
        if self.total_bytes > MAX_TOTAL_BYTES {
            return Err(ConfigError::SizeLimit);
        }
        let text = String::from_utf8(bytes).map_err(|error| ConfigError::Parse {
            path: canonical.clone(),
            message: error.to_string(),
        })?;
        let mut value: toml::Value =
            toml::from_str(&text).map_err(|error: toml::de::Error| ConfigError::Parse {
                path: canonical.clone(),
                message: error.to_string(),
            })?;
        let includes = value
            .get("include")
            .and_then(toml::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut merged = toml::Value::Table(toml::map::Map::new());
        for include in includes {
            let include = include.as_str().ok_or_else(|| {
                ConfigError::Validation(format!(
                    "{}: include entries must be strings",
                    canonical.display()
                ))
            })?;
            if include.contains('\0') || include.contains("$HOME") || include.contains("${") {
                return Err(ConfigError::Validation(format!(
                    "{}: include must be a trusted literal local path",
                    canonical.display()
                )));
            }
            let include_path = canonical.parent().unwrap_or(Path::new(".")).join(include);
            let included_value = self.read_file(&include_path, depth + 1)?;
            merge(&mut merged, &included_value);
        }
        if let Some(table) = value.as_table_mut() {
            table.remove("include");
        }
        merge(&mut merged, &value);
        self.active.remove(&canonical);
        self.sources.push(canonical);
        Ok(merged)
    }
}

fn merge(destination: &mut toml::Value, source: &toml::Value) {
    if let (Some(destination), Some(source)) = (destination.as_table_mut(), source.as_table()) {
        for (key, value) in source {
            if let Some(existing) = destination.get_mut(key)
                && existing.is_table()
                && value.is_table()
            {
                merge(existing, value);
            } else {
                destination.insert(key.clone(), value.clone());
            }
        }
    }
}
