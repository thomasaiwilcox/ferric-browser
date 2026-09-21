//! Typed, data-only configuration for `RustBrowser`.
//!
//! This crate deliberately has no Qt, display-server, network, or command
//! execution dependencies. Files are parsed into a candidate configuration and
//! only committed by [`ConfigStore`] after validation succeeds.

#![allow(clippy::missing_errors_doc)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

const CURRENT_SCHEMA_VERSION: u64 = 1;
const MAX_INCLUDE_DEPTH: usize = 8;
const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024;
const MAX_RUNTIME_SETTINGS: usize = 256;
const MAX_RUNTIME_KEY_BYTES: usize = 256;
const MAX_RUNTIME_VALUE_BYTES: usize = 16 * 1024;
const RUNTIME_SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReducedMotion {
    #[default]
    System,
    On,
    Off,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeSource {
    #[default]
    Auto,
    Builtin,
    File,
    Omarchy,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryMode {
    #[default]
    Normal,
    Insert,
    Passthrough,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExternalLinks {
    #[default]
    Tab,
    TabBg,
    Window,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LastClose {
    #[default]
    Blank,
    Window,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionRestore {
    Never,
    #[default]
    AskAfterCrash,
    Always,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionDecision {
    #[default]
    Ask,
    Allow,
    Deny,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CollisionPolicy {
    #[default]
    Ask,
    Rename,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PortalMode {
    #[default]
    Auto,
    Required,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiConfig {
    #[serde(default = "default_statusbar")]
    pub statusbar: String,
    #[serde(default = "default_tabs")]
    pub tabs: String,
    #[serde(default = "default_tab_position")]
    pub tab_position: String,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size_pt: f64,
    #[serde(default)]
    pub reduced_motion: ReducedMotion,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            statusbar: default_statusbar(),
            tabs: default_tabs(),
            tab_position: default_tab_position(),
            font_family: default_font_family(),
            font_size_pt: default_font_size(),
            reduced_motion: ReducedMotion::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeConfig {
    #[serde(default)]
    pub source: ThemeSource,
    #[serde(default)]
    pub path: String,
}

const MAX_THEME_PALETTE_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThemePalette {
    pub source: String,
    pub path: String,
    pub provider_layout: String,
    pub provider_version: String,
    pub provider_commit: String,
    pub mode: String,
    pub background: String,
    pub surface: String,
    pub foreground: String,
    pub muted: String,
    pub accent: String,
    pub border: String,
    pub selection_background: String,
    pub selection_foreground: String,
    pub error: String,
    pub warning: String,
    pub success: String,
    pub private: String,
    pub mode_insert: String,
    // Legacy aliases remain serialized for existing QML/provider data.
    pub selection: String,
    pub dark_background: String,
    pub lighter_background: String,
    pub red: String,
    pub yellow: String,
    pub green: String,
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self {
            source: "builtin".into(),
            path: String::new(),
            provider_layout: String::new(),
            provider_version: String::new(),
            provider_commit: String::new(),
            mode: "dark".into(),
            background: "#1e1e2e".into(),
            surface: "#313244".into(),
            foreground: "#cdd6f4".into(),
            muted: "#a6adc8".into(),
            accent: "#89b4fa".into(),
            border: "#585b70".into(),
            selection_background: "#45475a".into(),
            selection_foreground: "#cdd6f4".into(),
            error: "#f38ba8".into(),
            warning: "#f9e2af".into(),
            success: "#a6e3a1".into(),
            private: "#cba6f7".into(),
            mode_insert: "#f9e2af".into(),
            selection: "#45475a".into(),
            dark_background: "#181825".into(),
            lighter_background: "#313244".into(),
            red: "#f38ba8".into(),
            yellow: "#f9e2af".into(),
            green: "#a6e3a1".into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThemePaletteError {
    InvalidPath(String),
    NotFound(String),
    Io(String),
    Parse(String),
    InvalidColor(String),
}

impl std::fmt::Display for ThemePaletteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath(message)
            | Self::NotFound(message)
            | Self::Io(message)
            | Self::Parse(message)
            | Self::InvalidColor(message) => formatter.write_str(message),
        }
    }
}

/// Loads the configured desktop palette or its deterministic fallback.
///
/// # Errors
///
/// Returns an error when an explicitly selected palette path is invalid or
/// cannot be parsed safely.
pub fn load_theme_palette(config: &ThemeConfig) -> Result<ThemePalette, ThemePaletteError> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    load_theme_palette_at(config, home.as_deref())
}

fn load_theme_palette_at(
    config: &ThemeConfig,
    home: Option<&Path>,
) -> Result<ThemePalette, ThemePaletteError> {
    if !config.path.trim().is_empty() {
        let path = explicit_palette_path(&config.path, home)?;
        return parse_theme_palette_file(&path, "file");
    }

    let mut discovered = home.into_iter().flat_map(|home| {
        [
            home.join(".local/state/omarchy/current/theme/colors.toml"),
            home.join(".config/omarchy/current/theme/colors.toml"),
        ]
    });
    match config.source {
        ThemeSource::Builtin => Ok(ThemePalette::default()),
        ThemeSource::File => Err(ThemePaletteError::InvalidPath(
            "theme.path is required when theme.source is file".into(),
        )),
        ThemeSource::Omarchy => {
            let path = discovered.find(|path| path.is_file()).ok_or_else(|| {
                ThemePaletteError::NotFound("no Omarchy colors.toml was found".into())
            })?;
            parse_theme_palette_file(&path, "omarchy")
        }
        ThemeSource::Auto => {
            for path in discovered {
                if path.is_file()
                    && let Ok(palette) = parse_theme_palette_file(&path, "omarchy")
                {
                    return Ok(palette);
                }
            }
            Ok(ThemePalette::default())
        }
    }
}

fn explicit_palette_path(value: &str, home: Option<&Path>) -> Result<PathBuf, ThemePaletteError> {
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(ThemePaletteError::InvalidPath(
            "theme.path is too long or contains a control character".into(),
        ));
    }
    let path = if value == "~" {
        home.map(Path::to_path_buf).ok_or_else(|| {
            ThemePaletteError::InvalidPath("theme.path uses ~ but HOME is unavailable".into())
        })?
    } else if let Some(relative) = value.strip_prefix("~/") {
        home.map(|home| home.join(relative)).ok_or_else(|| {
            ThemePaletteError::InvalidPath("theme.path uses ~ but HOME is unavailable".into())
        })?
    } else {
        PathBuf::from(value)
    };
    Ok(if path.is_dir() {
        path.join("colors.toml")
    } else {
        path
    })
}

fn parse_theme_palette_file(path: &Path, source: &str) -> Result<ThemePalette, ThemePaletteError> {
    let metadata = fs::metadata(path).map_err(|error| {
        ThemePaletteError::NotFound(format!("theme palette is unavailable: {error}"))
    })?;
    if !metadata.is_file() {
        return Err(ThemePaletteError::InvalidPath(
            "theme palette is not a regular file".into(),
        ));
    }
    if metadata.len() > MAX_THEME_PALETTE_BYTES {
        return Err(ThemePaletteError::Parse(
            "theme palette exceeds 256 KiB".into(),
        ));
    }
    let bytes = fs::read(path)
        .map_err(|error| ThemePaletteError::Io(format!("could not read theme palette: {error}")))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| ThemePaletteError::Parse("theme palette is not UTF-8".into()))?;
    parse_theme_palette(&text, source, path)
}

fn bounded_metadata_value(path: &Path, max_bytes: usize) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > max_bytes as u64 {
        return None;
    }
    let value = fs::read_to_string(path).ok()?;
    let value = value.trim();
    (!value.is_empty() && !value.chars().any(char::is_control)).then(|| value.to_owned())
}

fn git_commit_at(root: &Path) -> Option<String> {
    let git = root.join(".git");
    let head = bounded_metadata_value(&git.join("HEAD"), 256)?;
    if head.len() == 40 && head.chars().all(|character| character.is_ascii_hexdigit()) {
        return Some(head);
    }
    let reference = head.strip_prefix("ref: ")?;
    if !reference.starts_with("refs/")
        || reference.len() > 128
        || reference.contains("..")
        || reference.chars().any(char::is_control)
    {
        return None;
    }
    let value = bounded_metadata_value(&git.join(reference), 256)?;
    (value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit()))
        .then_some(value)
}

fn omarchy_provider_metadata(path: &Path) -> (String, String, String) {
    let layout = if path.ends_with(".local/state/omarchy/current/theme/colors.toml") {
        "current".to_owned()
    } else if path.ends_with(".config/omarchy/current/theme/colors.toml") {
        "legacy".to_owned()
    } else {
        String::new()
    };
    let version =
        bounded_metadata_value(Path::new("/usr/share/omarchy/version"), 128).unwrap_or_default();
    let commit = [
        PathBuf::from("/usr/share/omarchy"),
        PathBuf::from("/opt/omarchy"),
    ]
    .into_iter()
    .find_map(|candidate| git_commit_at(&candidate))
    .unwrap_or_default();
    (layout, version, commit)
}

fn parse_theme_palette(
    text: &str,
    source: &str,
    path: &Path,
) -> Result<ThemePalette, ThemePaletteError> {
    let table = toml::from_str::<toml::Table>(text).map_err(|error| {
        ThemePaletteError::Parse(format!("invalid theme palette TOML: {error}"))
    })?;
    let defaults = ThemePalette::default();
    let mode = palette_string(&table, "mode", &defaults.mode)?;
    let background = palette_color(&table, "background", &defaults.background)?;
    let surface_fallback = table
        .get("lighter_background")
        .and_then(toml::Value::as_str)
        .unwrap_or(&defaults.surface);
    let surface = palette_color(&table, "surface", surface_fallback)?;
    let foreground = palette_color(&table, "foreground", &defaults.foreground)?;
    let accent = palette_color(&table, "accent", &defaults.accent)?;
    let selection_background = palette_color(
        &table,
        "selection_background",
        &palette_color(&table, "selection", &defaults.selection_background)?,
    )?;
    let selection_foreground = palette_color(&table, "selection_foreground", &foreground)?;
    let muted = palette_color(&table, "muted", &defaults.muted)?;
    let border = palette_color(&table, "border", &defaults.border)?;
    let dark_background = palette_color(&table, "dark_background", &background)?;
    let lighter_background = palette_color(&table, "lighter_background", &surface)?;
    let error = palette_color(
        &table,
        "error",
        &palette_color(&table, "red", &defaults.error)?,
    )?;
    let warning = palette_color(
        &table,
        "warning",
        &palette_color(&table, "yellow", &defaults.warning)?,
    )?;
    let success = palette_color(
        &table,
        "success",
        &palette_color(&table, "green", &defaults.success)?,
    )?;
    let private = palette_color(&table, "private", &defaults.private)?;
    let mode_insert = palette_color(&table, "mode_insert", &defaults.mode_insert)?;
    let (provider_layout, provider_version, provider_commit) = if source == "omarchy" {
        omarchy_provider_metadata(path)
    } else {
        (String::new(), String::new(), String::new())
    };
    Ok(ThemePalette {
        source: source.into(),
        path: path.to_string_lossy().into_owned(),
        provider_layout,
        provider_version,
        provider_commit,
        mode,
        background,
        surface: surface.clone(),
        foreground,
        muted,
        accent,
        border,
        selection_background: selection_background.clone(),
        selection_foreground: selection_foreground.clone(),
        error: error.clone(),
        warning: warning.clone(),
        success: success.clone(),
        private,
        mode_insert,
        selection: selection_background,
        dark_background,
        lighter_background,
        red: error,
        yellow: warning,
        green: success,
    })
}

fn palette_string(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    fallback: &str,
) -> Result<String, ThemePaletteError> {
    match table.get(key) {
        None => Ok(fallback.into()),
        Some(toml::Value::String(value)) if !value.is_empty() && value.len() <= 32 => {
            Ok(value.clone())
        }
        Some(_) => Err(ThemePaletteError::Parse(format!(
            "theme palette key {key} must be a bounded string"
        ))),
    }
}

fn palette_color(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    fallback: &str,
) -> Result<String, ThemePaletteError> {
    let value = palette_string(table, key, fallback)?;
    if valid_hex_color(&value) {
        Ok(value)
    } else {
        Err(ThemePaletteError::InvalidColor(format!(
            "theme palette key {key} is not a #RRGGBB or #RRGGBBAA color"
        )))
    }
}

fn valid_hex_color(value: &str) -> bool {
    matches!(value.len(), 7 | 9)
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

/// A WCAG contrast observation for one rendered chrome color pair.
///
/// User-provided palettes are intentionally never rejected for failing this
/// check. The result is an actionable diagnostic so a theme can remain
/// importable while the browser still identifies readability risks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContrastCheck {
    pub name: String,
    pub foreground: String,
    pub background: String,
    pub ratio: f64,
    pub passes_aa: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThemeContrastReport {
    pub status: String,
    pub checks: Vec<ContrastCheck>,
    pub failing: Vec<String>,
    pub reason: String,
}

/// Returns the WCAG 2.x contrast ratio for two validated theme colors.
/// Eight-digit colors are composited over the supplied background before the
/// ratio is calculated, matching the colors presented by the QML chrome.
#[must_use]
pub fn contrast_ratio(foreground: &str, background: &str) -> Option<f64> {
    let background = parse_rgba(background)?;
    let foreground = parse_rgba(foreground)?;
    let background = composite(background, [0, 0, 0]);
    let foreground = composite(foreground, background);
    let foreground_luminance = relative_luminance(foreground);
    let background_luminance = relative_luminance(background);
    Some(
        (foreground_luminance.max(background_luminance) + 0.05)
            / (foreground_luminance.min(background_luminance) + 0.05),
    )
}

/// Assesses the semantic pairs used by the rendered chrome. The normal-text
/// threshold is WCAG AA (4.5:1); state colors are checked too so diagnostics
/// can identify an unreadable user theme without making color the only state
/// signal in the UI.
#[must_use]
pub fn theme_contrast_report(palette: &ThemePalette) -> ThemeContrastReport {
    let pairs = [
        (
            "foreground-on-background",
            &palette.foreground,
            &palette.background,
        ),
        ("muted-on-background", &palette.muted, &palette.background),
        ("accent-on-background", &palette.accent, &palette.background),
        ("error-on-background", &palette.error, &palette.background),
        (
            "warning-on-background",
            &palette.warning,
            &palette.background,
        ),
        (
            "success-on-background",
            &palette.success,
            &palette.background,
        ),
        (
            "private-on-background",
            &palette.private,
            &palette.background,
        ),
        (
            "selection-foreground-on-selection",
            &palette.selection_foreground,
            &palette.selection_background,
        ),
    ];
    let checks = pairs
        .into_iter()
        .map(|(name, foreground, background)| {
            let ratio = contrast_ratio(foreground, background).unwrap_or(0.0);
            ContrastCheck {
                name: name.into(),
                foreground: foreground.clone(),
                background: background.clone(),
                ratio,
                passes_aa: ratio >= 4.5,
            }
        })
        .collect::<Vec<_>>();
    let failing = checks
        .iter()
        .filter(|check| !check.passes_aa)
        .map(|check| check.name.clone())
        .collect::<Vec<_>>();
    let status = if failing.is_empty() {
        "pass"
    } else {
        "warning"
    };
    let reason = if failing.is_empty() {
        "all assessed semantic chrome pairs meet the WCAG AA normal-text threshold".into()
    } else {
        format!(
            "{} semantic chrome pair(s) fall below the WCAG AA normal-text threshold; the palette remains importable",
            failing.len()
        )
    };
    ThemeContrastReport {
        status: status.into(),
        checks,
        failing,
        reason,
    }
}

fn parse_rgba(value: &str) -> Option<([u8; 3], u8)> {
    if !valid_hex_color(value) {
        return None;
    }
    let channel = |start| u8::from_str_radix(&value[start..start + 2], 16).ok();
    Some((
        [channel(1)?, channel(3)?, channel(5)?],
        if value.len() == 9 { channel(7)? } else { 255 },
    ))
}

fn composite((color, alpha): ([u8; 3], u8), background: [u8; 3]) -> [u8; 3] {
    let alpha = u16::from(alpha);
    [0, 1, 2].map(|index| {
        u8::try_from(
            (u16::from(color[index]) * alpha + u16::from(background[index]) * (255 - alpha) + 127)
                / 255,
        )
        .expect("alpha composite channel fits in u8")
    })
}

fn relative_luminance(color: [u8; 3]) -> f64 {
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color[0]) + 0.7152 * channel(color[1]) + 0.0722 * channel(color[2])
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputConfig {
    #[serde(default)]
    pub entry_mode: EntryMode,
    #[serde(default = "default_true")]
    pub auto_insert: bool,
    #[serde(default = "default_keychain_timeout")]
    pub keychain_timeout_ms: u64,
    #[serde(default = "default_count_limit")]
    pub count_limit: u32,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            entry_mode: EntryMode::default(),
            auto_insert: true,
            keychain_timeout_ms: default_keychain_timeout(),
            count_limit: default_count_limit(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryConfig {
    #[serde(default = "default_true")]
    pub keychain_overlay: bool,
    #[serde(default = "default_overlay_delay")]
    pub keychain_overlay_delay_ms: u64,
    #[serde(default)]
    pub learning_mode: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            keychain_overlay: true,
            keychain_overlay_delay_ms: default_overlay_delay(),
            learning_mode: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationConfig {
    #[serde(default = "default_search_engine")]
    pub default_search: String,
    #[serde(default = "default_start_pages")]
    pub start_pages: Vec<String>,
    #[serde(default = "default_new_tab")]
    pub new_tab: String,
    #[serde(default)]
    pub external_links: ExternalLinks,
}

impl Default for NavigationConfig {
    fn default() -> Self {
        Self {
            default_search: default_search_engine(),
            start_pages: default_start_pages(),
            new_tab: default_new_tab(),
            external_links: ExternalLinks::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkCleaningConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_builtin")]
    pub rules: String,
    #[serde(default = "default_true")]
    pub confirm_navigation: bool,
    /// Optional explicit HTTPS source for a signed-by-configuration rule
    /// manifest. It is never contacted unless the user invokes the update
    /// command.
    #[serde(default)]
    pub update_source: Option<String>,
    /// SHA-256 of the exact manifest bytes expected from `update_source`.
    #[serde(default)]
    pub update_sha256: Option<String>,
}

impl Default for LinkCleaningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: default_builtin(),
            confirm_navigation: true,
            update_source: None,
            update_sha256: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinksConfig {
    #[serde(default)]
    pub cleaning: LinkCleaningConfig,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabsConfig {
    #[serde(default)]
    pub last_close: LastClose,
    #[serde(default = "default_related_position")]
    pub related_position: String,
    #[serde(default = "default_undo_limit")]
    pub undo_limit: u32,
    #[serde(default)]
    pub auto_discard: bool,
}

impl Default for TabsConfig {
    fn default() -> Self {
        Self {
            last_close: LastClose::default(),
            related_position: default_related_position(),
            undo_limit: default_undo_limit(),
            auto_discard: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionConfig {
    #[serde(default)]
    pub restore: SessionRestore,
    #[serde(default = "default_true")]
    pub lazy_restore: bool,
    #[serde(default = "default_checkpoint")]
    pub checkpoint_seconds: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            restore: SessionRestore::default(),
            lazy_restore: true,
            checkpoint_seconds: default_checkpoint(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwitcherConfig {
    #[serde(default = "default_undo_limit")]
    pub max_results: u32,
    #[serde(default)]
    pub include_private: bool,
}

impl Default for SwitcherConfig {
    fn default() -> Self {
        Self {
            max_results: default_undo_limit(),
            include_private: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryConfig {
    #[serde(default = "default_retention")]
    pub retention_days: u32,
    #[serde(default = "default_command_limit")]
    pub command_limit: u32,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            retention_days: default_retention(),
            command_limit: default_command_limit(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Autoplay {
    #[default]
    EngineDefault,
    RequireGesture,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentConfig {
    #[serde(default = "default_true")]
    pub javascript: bool,
    #[serde(default = "default_true")]
    pub images: bool,
    #[serde(default)]
    pub force_dark: bool,
    #[serde(default)]
    pub autoplay: Autoplay,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
}

impl Default for ContentConfig {
    fn default() -> Self {
        Self {
            javascript: true,
            images: true,
            force_dark: false,
            autoplay: Autoplay::default(),
            zoom: default_zoom(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyConfig {
    #[serde(default)]
    pub remote_suggestions: bool,
    #[serde(default)]
    pub push_service: bool,
    #[serde(default)]
    pub private_history_suggestions: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionsConfig {
    #[serde(default)]
    pub camera: PermissionDecision,
    #[serde(default)]
    pub microphone: PermissionDecision,
    #[serde(default)]
    pub screen_capture: PermissionDecision,
    #[serde(default)]
    pub notifications: PermissionDecision,
    #[serde(default)]
    pub geolocation: PermissionDecision,
    #[serde(default)]
    pub clipboard: PermissionDecision,
    #[serde(default)]
    pub local_fonts: PermissionDecision,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockingConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub network_filtering: bool,
    #[serde(default)]
    pub cosmetic_filtering: bool,
    #[serde(default = "default_update_interval")]
    pub update_interval_hours: u32,
    #[serde(default = "default_lists")]
    pub lists: Vec<String>,
    /// Exact hosts or one-label wildcard suffixes that bypass network
    /// blocking for this profile. This is a generated/runtime-friendly
    /// durable setting; it never grants permissions or weakens TLS.
    #[serde(default)]
    pub bypass_sites: Vec<String>,
    /// Exact hosts or one-label wildcard suffixes for security-sensitive
    /// network denial. These rules cannot be bypassed by site exceptions.
    #[serde(default)]
    pub security_deny_hosts: Vec<String>,
}

impl Default for BlockingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            network_filtering: true,
            cosmetic_filtering: false,
            update_interval_hours: default_update_interval(),
            lists: default_lists(),
            bypass_sites: Vec::new(),
            security_deny_hosts: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DownloadDirectory {
    #[default]
    XdgDownloads,
    Path(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadsConfig {
    #[serde(default)]
    pub directory: DownloadDirectory,
    #[serde(default = "default_true")]
    pub ask_destination: bool,
    #[serde(default)]
    pub collision: CollisionPolicy,
    #[serde(default)]
    pub open_when_complete: bool,
}

impl Default for DownloadsConfig {
    fn default() -> Self {
        Self {
            directory: DownloadDirectory::default(),
            ask_destination: true,
            collision: CollisionPolicy::default(),
            open_when_complete: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpellcheckConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
}

impl Default for SpellcheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            languages: default_languages(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopConfig {
    #[serde(default)]
    pub portals: PortalMode,
    #[serde(default = "default_true")]
    pub notifications: bool,
    #[serde(default = "default_true")]
    pub media_keys: bool,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            portals: PortalMode::default(),
            notifications: true,
            media_keys: true,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TriState {
    #[default]
    Auto,
    On,
    Off,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HyprlandConfig {
    #[serde(default)]
    pub enabled: TriState,
    #[serde(default)]
    pub workspace_routing: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteDoctorConfig {
    #[serde(default = "default_true")]
    pub temporary_experiments: bool,
}

impl Default for SiteDoctorConfig {
    fn default() -> Self {
        Self {
            temporary_experiments: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IpcConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub private_queries: bool,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            private_queries: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionTargetConfig {
    #[serde(default)]
    pub subject_types: Vec<String>,
    pub executable: String,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub detach: bool,
    #[serde(default)]
    pub allow_private: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolsConfig {
    /// Direct argv for trusted external editing. The complete argument
    /// `{file}` is replaced with the managed file path at invocation time.
    #[serde(default)]
    pub editor: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    #[serde(default)]
    pub level: LogLevel,
    #[serde(default = "default_log_size")]
    pub max_file_mib: u32,
    #[serde(default = "default_retained_files")]
    pub retained_files: u32,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::default(),
            max_file_mib: default_log_size(),
            retained_files: default_retained_files(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteRule {
    pub id: String,
    pub pattern: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub set: BTreeMap<String, toml::Value>,
}

/// Validates one site rule independently of the surrounding configuration.
/// Runtime-generated rules use this same contract before they can be merged
/// into the effective configuration.
pub fn validate_site_rule(rule: &SiteRule) -> Result<(), ConfigError> {
    if validate_slug(&rule.id, "site rule id").is_err() {
        return Err(ConfigError::Validation(
            "site rule IDs must be unique, nonempty, and lowercase slugs".into(),
        ));
    }
    if rule.pattern.trim().is_empty()
        || rule.pattern.len() > 512
        || rule.pattern.chars().any(char::is_control)
        || parse_site_pattern(&rule.pattern).is_err()
    {
        return Err(ConfigError::Validation(format!(
            "site rule {} has an invalid HTTP(S) pattern",
            rule.id
        )));
    }
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
    Ok(())
}

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

const SETTING_METADATA: &[SettingMetadata] = &[
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
        "always",
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

/// Returns the static metadata rows used to generate user-facing setting
/// documentation and registry-backed configuration surfaces.
#[must_use]
pub const fn setting_metadata_all() -> &'static [SettingMetadata] {
    SETTING_METADATA
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

/// A user-maintained declarative profile definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileDefinition {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub overrides: BTreeMap<String, toml::Value>,
}

/// The separate user-maintained `profiles.toml` document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilesConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u64,
    #[serde(default)]
    pub profiles: Vec<ProfileDefinition>,
}

impl Default for ProfilesConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            profiles: Vec::new(),
        }
    }
}

impl ProfilesConfig {
    /// Returns the explicitly selected default profile, if one exists.
    #[must_use]
    pub fn default_profile(&self) -> Option<&ProfileDefinition> {
        self.profiles.iter().find(|profile| profile.default)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionRuleConfig {
    pub id: String,
    pub profile: String,
    pub origin: String,
    pub permission: String,
    pub decision: PermissionDecision,
}

/// Returns the user-defined site rules that match an HTTP(S) URL.
///
/// Matching is deliberately a small glob language: `*` matches any sequence
/// of characters, and all other characters match literally. Rules are
/// returned in the precedence order required by CONFIG-002: ascending
/// priority followed by source order.
#[must_use]
pub fn matching_site_rules<'a>(config: &'a Config, url: &str) -> Vec<&'a SiteRule> {
    if !is_http_url(url) {
        return Vec::new();
    }
    let mut rules = config
        .site_rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| site_pattern_matches(&rule.pattern, url))
        .collect::<Vec<_>>();
    rules.sort_by_key(|(index, rule)| (rule.priority, *index));
    rules.into_iter().map(|(_, rule)| rule).collect()
}

fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SiteScheme {
    Http,
    Https,
    Any,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SitePattern {
    scheme: SiteScheme,
    host: String,
    port: Option<u16>,
    path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SiteTarget {
    scheme: SiteScheme,
    host: String,
    port: u16,
    path: String,
}

fn parse_site_pattern(pattern: &str) -> Result<SitePattern, String> {
    if pattern.is_empty()
        || pattern.len() > 512
        || pattern.chars().any(char::is_control)
        || pattern.chars().any(char::is_whitespace)
    {
        return Err("site pattern is empty, too long, or contains unsafe characters".into());
    }
    let (scheme, remainder) = pattern
        .split_once("://")
        .ok_or_else(|| "site pattern requires a scheme and authority".to_owned())?;
    let scheme = match scheme {
        "http" => SiteScheme::Http,
        "https" => SiteScheme::Https,
        "*" => SiteScheme::Any,
        _ => return Err("site pattern scheme must be http, https, or *".into()),
    };
    let authority_end = remainder.find('/').unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    let path = if authority_end == remainder.len() {
        "/".to_owned()
    } else {
        remainder[authority_end..].to_owned()
    };
    if path.contains('?') || path.contains('#') || !path.starts_with('/') {
        return Err("site pattern path must be an HTTP path without query or fragment".into());
    }
    let (host, port) = parse_site_authority(authority, true)?;
    Ok(SitePattern {
        scheme,
        host,
        port,
        path,
    })
}

fn parse_site_target(url: &str) -> Option<SiteTarget> {
    let (scheme, remainder) = url.split_once("://")?;
    let scheme = match scheme {
        "http" => SiteScheme::Http,
        "https" => SiteScheme::Https,
        _ => return None,
    };
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let (host, explicit_port) = parse_site_authority(&remainder[..authority_end], false).ok()?;
    let port = explicit_port.unwrap_or(match scheme {
        SiteScheme::Http => 80,
        SiteScheme::Https => 443,
        SiteScheme::Any => return None,
    });
    let path_end = remainder[authority_end..]
        .find(['?', '#'])
        .map_or(remainder.len(), |index| authority_end + index);
    let path = if authority_end == remainder.len() || remainder.as_bytes()[authority_end] != b'/' {
        "/".to_owned()
    } else {
        remainder[authority_end..path_end].to_owned()
    };
    Some(SiteTarget {
        scheme,
        host,
        port,
        path,
    })
}

fn parse_site_authority(
    authority: &str,
    allow_wildcards: bool,
) -> Result<(String, Option<u16>), String> {
    if authority.is_empty() || authority.contains('@') {
        return Err("site pattern authority is empty or contains userinfo".into());
    }
    let (host, port) = if authority.starts_with('[') {
        let end = authority
            .find(']')
            .ok_or_else(|| "bracketed IPv6 authority is incomplete".to_owned())?;
        let host = &authority[1..end];
        let suffix = &authority[end + 1..];
        let port = if suffix.is_empty() {
            None
        } else {
            suffix
                .strip_prefix(':')
                .ok_or_else(|| "IPv6 authority has an invalid suffix".to_owned())?
                .parse::<u16>()
                .ok()
                .filter(|port| *port != 0)
                .ok_or_else(|| "site authority port must be 1..65535".to_owned())
                .map(Some)?
        };
        (host.to_owned(), port)
    } else {
        if authority.matches(':').count() > 1 {
            return Err("IPv6 hosts require bracket syntax".into());
        }
        let (host, port) = authority
            .split_once(':')
            .map_or((authority, None), |(host, port)| {
                (host, port.parse::<u16>().ok().filter(|port| *port != 0))
            });
        if authority.contains(':') && port.is_none() {
            return Err("site authority port must be 1..65535".into());
        }
        (host.to_owned(), port)
    };
    let host = host.to_ascii_lowercase();
    if !valid_site_host(&host, allow_wildcards) {
        return Err("site authority host is invalid".into());
    }
    Ok((host, port))
}

fn valid_site_host(host: &str, allow_wildcards: bool) -> bool {
    if host == "*" {
        return allow_wildcards;
    }
    if host.contains(':') {
        return host.len() <= 128
            && host
                .chars()
                .all(|character| character.is_ascii_hexdigit() || matches!(character, ':' | '.'));
    }
    let host = if let Some(suffix) = host.strip_prefix("*.") {
        if !allow_wildcards || suffix.is_empty() || suffix.contains('*') {
            return false;
        }
        suffix
    } else {
        if host.contains('*') {
            return false;
        }
        host
    };
    host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
}

fn site_pattern_matches(pattern: &str, url: &str) -> bool {
    let Ok(pattern) = parse_site_pattern(pattern) else {
        return false;
    };
    let Some(target) = parse_site_target(url) else {
        return false;
    };
    if pattern.scheme != SiteScheme::Any && pattern.scheme != target.scheme {
        return false;
    }
    let host_matches = if pattern.host == "*" {
        true
    } else if let Some(suffix) = pattern.host.strip_prefix("*.") {
        target.host == suffix || target.host.ends_with(&format!(".{suffix}"))
    } else {
        target.host == pattern.host
    };
    if !host_matches {
        return false;
    }
    let expected_port = pattern.port.unwrap_or(match target.scheme {
        SiteScheme::Http => 80,
        SiteScheme::Https => 443,
        SiteScheme::Any => return false,
    });
    target.port == expected_port && glob_matches(&pattern.path, &target.path)
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let (mut pattern_index, mut value_index) = (0, 0);
    let (mut star, mut retry) = (None, 0);
    while value_index < value.len() {
        if pattern_index < pattern.len() && pattern[pattern_index] == value[value_index] {
            pattern_index += 1;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star = Some(pattern_index);
            pattern_index += 1;
            retry = value_index;
        } else if let Some(star_index) = star {
            pattern_index = star_index + 1;
            retry += 1;
            value_index = retry;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

/// A user-maintained durable browsing context definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextDefinition {
    pub name: String,
    pub label: String,
    pub profile: String,
    #[serde(default)]
    pub sessions: Vec<String>,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default = "default_context_target")]
    pub default_target: String,
}

/// A context route from an explicit browser-controlled entry point.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextRoute {
    pub id: String,
    pub pattern: String,
    pub context: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default = "default_context_behavior")]
    pub behavior: String,
    #[serde(default)]
    pub entry_points: Vec<String>,
}

/// The separate user-maintained `contexts.toml` document.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextsConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u64,
    #[serde(default)]
    pub contexts: Vec<ContextDefinition>,
    #[serde(default)]
    pub routes: Vec<ContextRoute>,
}

/// Returns context routes that match one browser-controlled entry point.
///
/// Routes use the same deliberately narrow HTTP(S) match-pattern grammar as
/// CONFIG-006. Results are ordered by descending priority, then descending
/// pattern specificity, then authored order. The caller can show all results
/// in a preview while using the first result as the deterministic candidate.
#[must_use]
pub fn matching_context_routes<'a>(
    config: &'a ContextsConfig,
    url: &str,
    entry_point: &str,
) -> Vec<&'a ContextRoute> {
    if entry_point.is_empty() {
        return Vec::new();
    }
    let Some(url_parts) = parse_http_target(url) else {
        return Vec::new();
    };
    let mut routes = config
        .routes
        .iter()
        .enumerate()
        .filter_map(|(index, route)| {
            if !route.entry_points.iter().any(|entry| entry == entry_point) {
                return None;
            }
            let pattern = parse_context_pattern(&route.pattern)?;
            let specificity = pattern.specificity();
            pattern
                .matches(&url_parts)
                .then_some((route.priority, specificity, index, route))
        })
        .collect::<Vec<_>>();
    routes.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    routes.into_iter().map(|(_, _, _, route)| route).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HttpTarget {
    scheme: String,
    host: String,
    port: Option<u16>,
    path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContextPattern {
    scheme: String,
    host: String,
    port: Option<u16>,
    path: String,
}

impl ContextPattern {
    fn matches(&self, target: &HttpTarget) -> bool {
        if self.scheme != "*" && self.scheme != target.scheme {
            return false;
        }
        let host_matches = if self.host == "*" {
            true
        } else if let Some(suffix) = self.host.strip_prefix("*.") {
            target.host == suffix || target.host.ends_with(&format!(".{suffix}"))
        } else {
            target.host == self.host
        };
        if !host_matches {
            return false;
        }
        let target_port = target.port.or_else(|| default_http_port(&target.scheme));
        let pattern_port = self.port.or_else(|| {
            default_http_port(if self.scheme == "*" {
                &target.scheme
            } else {
                &self.scheme
            })
        });
        pattern_port == target_port && glob_matches(&self.path, &target.path)
    }

    fn specificity(&self) -> u16 {
        let scheme = u16::from(self.scheme != "*") * 100;
        let host = if self.host == "*" {
            0
        } else if self.host.starts_with("*.") {
            50
        } else {
            100
        };
        let port = u16::from(self.port.is_some()) * 10;
        let path: u16 = self
            .path
            .bytes()
            .filter(|byte| *byte != b'*')
            .count()
            .min(99)
            .try_into()
            .expect("specificity path length is bounded to 99");
        scheme + host + port + path
    }
}

fn parse_context_pattern(value: &str) -> Option<ContextPattern> {
    let target = parse_http_target(value)?;
    let (scheme, authority) = value.split_once("://")?;
    let authority_end = authority.find(['/', '?', '#']).unwrap_or(authority.len());
    let authority = &authority[..authority_end];
    if scheme != "*" && !matches!(scheme, "http" | "https")
        || value.contains(['?', '#'])
        || authority.is_empty()
        || authority.contains('@')
    {
        return None;
    }
    let host = target.host;
    if host != "*" && !host.starts_with("*.") && host.contains('*')
        || host.starts_with("*.") && host.len() <= 2
    {
        return None;
    }
    let path = value
        .split_once("://")
        .and_then(|(_, remainder)| remainder.split_once('/').map(|(_, path)| path))
        .map_or_else(|| "/*".to_owned(), |path| format!("/{path}"));
    if path.is_empty() || path.chars().any(char::is_control) {
        return None;
    }
    Some(ContextPattern {
        scheme: scheme.to_ascii_lowercase(),
        host,
        port: target.port,
        path,
    })
}

fn parse_http_target(value: &str) -> Option<HttpTarget> {
    let (scheme, remainder) = value.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https" | "*") || scheme == "*" && remainder.is_empty() {
        return None;
    }
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    let (host, port) = parse_pattern_authority(authority)?;
    let path = remainder[authority_end..]
        .split(['?', '#'])
        .next()
        .filter(|path| !path.is_empty())
        .unwrap_or("/");
    Some(HttpTarget {
        scheme,
        host,
        port,
        path: path.to_owned(),
    })
}

fn parse_pattern_authority(authority: &str) -> Option<(String, Option<u16>)> {
    if authority.is_empty()
        || authority
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return None;
    }
    if authority == "*" {
        return Some(("*".into(), None));
    }
    let (host, port) = if authority.starts_with('[') {
        let close = authority.find(']')?;
        let suffix = &authority[close + 1..];
        let port = if let Some(value) = suffix.strip_prefix(':') {
            Some(parse_port(value)?)
        } else {
            None
        };
        if !suffix.is_empty() && !suffix.starts_with(':') {
            return None;
        }
        (&authority[1..close], port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        if host.contains(':') {
            return None;
        }
        (host, Some(parse_port(port)?))
    } else {
        (authority, None)
    };
    if host.is_empty() {
        return None;
    }
    let host = host.to_ascii_lowercase();
    if host != "*" && host != "localhost" && !host.starts_with("*.") && host.contains('*')
        || host.starts_with("*.") && host.len() <= 2
    {
        return None;
    }
    Some((host, port))
}

fn parse_port(value: &str) -> Option<u16> {
    let port = value.parse::<u16>().ok()?;
    (port != 0).then_some(port)
}

fn default_http_port(scheme: &str) -> Option<u16> {
    match scheme {
        "http" => Some(80),
        "https" => Some(443),
        _ => None,
    }
}

impl Default for ContextsConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            contexts: Vec::new(),
            routes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_schema_version")]
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
    #[serde(default = "default_runtime_schema_version")]
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

#[allow(clippy::too_many_lines)]
fn validate(config: &Config) -> Result<(), ConfigError> {
    if config.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(ConfigError::Validation(format!(
            "unsupported schema_version {}",
            config.schema_version
        )));
    }
    if !matches!(config.ui.statusbar.as_str(), "always" | "command" | "never") {
        return Err(ConfigError::Validation(
            "ui.statusbar must be always, command, or never".into(),
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

fn validate_profile_definition(profile: &ProfileDefinition) -> Result<(), ConfigError> {
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

fn validate_profiles(config: &ProfilesConfig) -> Result<(), ConfigError> {
    if config.schema_version > CURRENT_SCHEMA_VERSION {
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
fn validate_contexts(config: &ContextsConfig) -> Result<(), ConfigError> {
    const ALLOWED_ROUTE_ENTRY_POINTS: [&str; 3] =
        ["external-open", "explicit-open", "typed-initial-url"];
    if config.schema_version > CURRENT_SCHEMA_VERSION {
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

fn validate_slug(value: &str, label: &str) -> Result<(), ConfigError> {
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

fn validate_runtime_key(key: &str) -> Result<(), ConfigError> {
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

fn validate_runtime_overrides(overrides: &RuntimeOverrides) -> Result<(), ConfigError> {
    if overrides.schema_version > RUNTIME_SCHEMA_VERSION {
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

fn validate_runtime_site_pattern(pattern: &str) -> Result<(), ConfigError> {
    parse_site_pattern(pattern).map_err(|_| {
        ConfigError::Validation("runtime site pattern must be a bounded HTTP(S) pattern".into())
    })?;
    Ok(())
}

fn runtime_site_rule_id(pattern: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in pattern.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("runtime-site-{hash:016x}")
}

fn validate_runtime_binding(mode: &str, keychain: &str, command: &str) -> Result<(), ConfigError> {
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

fn set_private_permissions(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn set_private_file_mode(options: &mut fs::OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
}

fn io_error(path: &Path, error: &std::io::Error) -> ConfigError {
    ConfigError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}
fn default_true() -> bool {
    true
}
fn default_schema_version() -> u64 {
    CURRENT_SCHEMA_VERSION
}
fn default_runtime_schema_version() -> u64 {
    RUNTIME_SCHEMA_VERSION
}
fn default_context_target() -> String {
    "reuse-or-window".into()
}
fn default_context_behavior() -> String {
    "prompt".into()
}
fn default_statusbar() -> String {
    "always".into()
}
fn default_tabs() -> String {
    "multiple".into()
}
fn default_tab_position() -> String {
    "top".into()
}
fn default_font_family() -> String {
    "monospace".into()
}
fn default_font_size() -> f64 {
    10.0
}
fn default_keychain_timeout() -> u64 {
    1_000
}
fn default_count_limit() -> u32 {
    9_999
}
fn default_overlay_delay() -> u64 {
    350
}
fn default_search_engine() -> String {
    "ddg".into()
}
fn default_start_pages() -> Vec<String> {
    vec!["rb://start".into()]
}
fn default_new_tab() -> String {
    "rb://blank".into()
}
fn default_builtin() -> String {
    "builtin".into()
}
fn default_related_position() -> String {
    "after-opener".into()
}
fn default_undo_limit() -> u32 {
    100
}
fn default_checkpoint() -> u64 {
    5
}
fn default_retention() -> u32 {
    90
}
fn default_command_limit() -> u32 {
    1_000
}
fn default_zoom() -> f64 {
    1.0
}
fn default_update_interval() -> u32 {
    24
}
fn default_lists() -> Vec<String> {
    vec!["easylist".into(), "easyprivacy".into()]
}
fn default_languages() -> Vec<String> {
    vec!["system".into()]
}
fn default_log_size() -> u32 {
    5
}
fn default_retained_files() -> u32 {
    3
}
fn default_search_engines() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("ddg".into(), "https://duckduckgo.com/?q={query}".into()),
        ("g".into(), "https://www.google.com/search?q={query}".into()),
    ])
}

fn valid_link_update_source(source: &str) -> bool {
    if !(10..=2048).contains(&source.len())
        || !source.is_ascii()
        || !source.bytes().all(|byte| byte.is_ascii_graphic())
        || !source.starts_with("https://")
        || source.contains(['?', '#', '@'])
    {
        return false;
    }
    let authority = source[8..].split('/').next().unwrap_or_default();
    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    !host.is_empty()
        && host.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'-'
        })
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !host.contains("..")
        && port.is_none_or(|value| {
            !value.is_empty()
                && value.len() <= 5
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && value.parse::<u16>().is_ok_and(|port| port != 0)
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte <= b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_starter_configuration() {
        let config = Config::default();
        validate(&config).expect("defaults validate");
        assert_eq!(config.navigation.default_search, "ddg");
        assert_eq!(config.tabs.undo_limit, 100);
    }

    #[test]
    fn remote_suggestions_are_rejected_without_a_provider() {
        let mut config = Config::default();
        config.privacy.remote_suggestions = true;
        let error = validate(&config).expect_err("unsupported remote suggestions rejected");
        assert!(error.to_string().contains("remote suggestion provider"));
    }

    #[test]
    fn rejects_unknown_keys_and_unsafe_search_templates() {
        let unknown = toml::from_str::<toml::Value>("[ui]\nnot_a_setting = true").expect("toml");
        let error = unknown
            .try_into::<Config>()
            .expect_err("unknown key rejected");
        assert!(error.to_string().contains("unknown field"));

        let mut config = Config::default();
        config
            .search_engines
            .insert("bad".into(), "https://example.test/{query}/{query}".into());
        assert!(validate(&config).is_err());
    }

    #[test]
    fn tools_editor_requires_one_complete_file_argument() {
        let mut config = Config::default();
        validate(&config).expect("empty editor is valid but unavailable");

        config.tools.editor = vec!["nvim".into(), "{file}".into()];
        validate(&config).expect("direct editor argv validates");

        config.tools.editor = vec!["nvim".into()];
        assert!(validate(&config).is_err());
        config.tools.editor = vec!["nvim".into(), "--cmd={file}".into()];
        assert!(validate(&config).is_err());
        config.tools.editor = vec!["-nvim".into(), "{file}".into()];
        assert!(validate(&config).is_err());
    }

    #[test]
    fn action_targets_are_typed_bounded_and_placeholder_safe() {
        let mut config = Config::default();
        config.action_targets.insert(
            "mpv".into(),
            ActionTargetConfig {
                subject_types: vec!["url".into(), "link".into()],
                executable: "mpv".into(),
                argv: vec!["--force-window".into(), "{url}".into()],
                detach: true,
                allow_private: false,
            },
        );
        validate(&config).expect("valid action target");

        config.action_targets.get_mut("mpv").unwrap().argv = vec!["--url={url}".into()];
        assert!(validate(&config).is_err());
        config.action_targets.get_mut("mpv").unwrap().argv = vec!["{selection}".into()];
        assert!(validate(&config).is_err());
        config.action_targets.get_mut("mpv").unwrap().argv = vec!["{url}".into()];
        config.action_targets.get_mut("mpv").unwrap().executable = "-sh".into();
        assert!(validate(&config).is_err());

        let mut tab_target = Config::default();
        tab_target.action_targets.insert(
            "tab-tool".into(),
            ActionTargetConfig {
                subject_types: vec!["tab".into()],
                executable: "tool".into(),
                argv: vec!["{title}".into(), "{url}".into()],
                detach: false,
                allow_private: true,
            },
        );
        validate(&tab_target).expect("tab targets may use tab URL/title fields");
    }

    #[test]
    fn validates_binding_layers_before_commit() {
        let mut config = Config::default();
        config
            .bindings
            .entry("normal".into())
            .or_default()
            .insert("g,t".into(), "tab-next".into());
        validate(&config).expect("valid binding layer");

        config
            .bindings
            .insert("unsupported".into(), BTreeMap::new());
        assert!(validate(&config).is_err());
    }

    #[test]
    fn validates_exact_profile_permission_rules() {
        let mut config = Config::default();
        config.permission_rules.push(PermissionRuleConfig {
            id: "mail-notifications".into(),
            profile: "work".into(),
            origin: "https://mail.google.com".into(),
            permission: "notifications".into(),
            decision: PermissionDecision::Allow,
        });
        validate(&config).expect("permission rule");

        config.permission_rules[0].origin = "http://example.test".into();
        assert!(validate(&config).is_err());
        config.permission_rules[0].origin = "https://mail.google.com".into();
        config.permission_rules[0].decision = PermissionDecision::Ask;
        validate(&config).expect("ask permission rule");
    }

    #[test]
    fn site_rules_match_in_priority_and_source_order() {
        let config = Config {
            site_rules: vec![
                SiteRule {
                    id: "later".into(),
                    pattern: "https://*.example.test/*".into(),
                    priority: 10,
                    set: BTreeMap::from([("content.zoom".into(), toml::Value::Float(1.25))]),
                },
                SiteRule {
                    id: "base".into(),
                    pattern: "https://example.test/*".into(),
                    priority: 0,
                    set: BTreeMap::from([("content.zoom".into(), toml::Value::Float(1.1))]),
                },
                SiteRule {
                    id: "override".into(),
                    pattern: "https://example.test/docs/*".into(),
                    priority: 0,
                    set: BTreeMap::from([("content.zoom".into(), toml::Value::Float(1.5))]),
                },
            ],
            ..Config::default()
        };
        validate(&config).expect("site rules validate");
        let rules = matching_site_rules(&config, "https://example.test/docs/start");
        assert_eq!(
            rules
                .iter()
                .map(|rule| rule.id.as_str())
                .collect::<Vec<_>>(),
            vec!["base", "override", "later"]
        );
        assert_eq!(
            rules
                .last()
                .and_then(|rule| rule.set.get("content.zoom"))
                .and_then(toml::Value::as_float),
            Some(1.25)
        );
        assert!(matching_site_rules(&config, "file:///tmp/page.html").is_empty());
    }

    #[test]
    fn site_patterns_normalize_scheme_host_port_and_path() {
        let mut config = Config {
            site_rules: vec![
                SiteRule {
                    id: "wildcard".into(),
                    pattern: "*://*.Example.test/*".into(),
                    priority: 0,
                    set: BTreeMap::new(),
                },
                SiteRule {
                    id: "port".into(),
                    pattern: "https://example.test:8443/docs/*".into(),
                    priority: 0,
                    set: BTreeMap::new(),
                },
            ],
            ..Config::default()
        };
        validate(&config).expect("site pattern syntax");
        assert_eq!(
            matching_site_rules(&config, "https://EXAMPLE.TEST/docs/start?x=1#part")
                .iter()
                .map(|rule| rule.id.as_str())
                .collect::<Vec<_>>(),
            vec!["wildcard"]
        );
        assert_eq!(
            matching_site_rules(&config, "https://sub.example.test/other")
                .iter()
                .map(|rule| rule.id.as_str())
                .collect::<Vec<_>>(),
            vec!["wildcard"]
        );
        assert!(matching_site_rules(&config, "https://badexample.test/other").is_empty());
        assert_eq!(
            matching_site_rules(&config, "https://example.test:8443/docs/page")
                .iter()
                .map(|rule| rule.id.as_str())
                .collect::<Vec<_>>(),
            vec!["port"]
        );
        assert!(matching_site_rules(&config, "https://example.test:9443/docs/page").is_empty());
        config.site_rules[0].pattern = "ftp://example.test/*".into();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn site_rules_reject_unsafe_or_unknown_settings() {
        let mut config = Config::default();
        config.site_rules.push(SiteRule {
            id: "docs".into(),
            pattern: "https://docs.example.test/*".into(),
            priority: 0,
            set: BTreeMap::from([("unknown.setting".into(), toml::Value::Boolean(true))]),
        });
        assert!(validate(&config).is_err());

        config.site_rules[0].set =
            BTreeMap::from([("content.zoom".into(), toml::Value::Float(9.0))]);
        assert!(validate(&config).is_err());
    }

    #[test]
    fn site_scope_capability_is_explicit_and_bounded() {
        assert_eq!(setting_supported_scopes("ui.reduced_motion"), &["global"]);
        assert_eq!(
            setting_supported_scopes("spellcheck.languages"),
            &["profile"]
        );
        assert_eq!(
            setting_supported_scopes("content.javascript"),
            &["global", "profile", "site"]
        );
        assert!(setting_supports_site_scope("content.javascript"));
        assert!(setting_supports_site_scope("content.zoom"));
        assert!(setting_supports_site_scope("input.entry_mode"));
        assert!(!setting_supports_site_scope("blocking.enabled"));
        assert!(!setting_supports_site_scope("ui.font_family"));
        assert!(!setting_supports_site_scope("proxy.global"));
    }

    #[test]
    fn spellcheck_languages_are_bounded_bcp47_tags() {
        let mut config = Config::default();
        validate(&config).expect("default system language");

        config.spellcheck.languages = vec![
            "en-US".into(),
            "zh-Hant-TW".into(),
            "en-US-u-hc-h12".into(),
            "de-1996".into(),
            "x-private".into(),
            "i-klingon".into(),
            "zh-min-nan".into(),
        ];
        validate(&config).expect("valid BCP 47 language tags");

        for invalid in ["", "e", "x", "en_US", "en--US", "en-", "en-a"] {
            config.spellcheck.languages = vec![invalid.into()];
            assert!(
                validate(&config).is_err(),
                "accepted invalid tag {invalid:?}"
            );
        }

        config.spellcheck.languages = vec!["en".into(), "EN".into()];
        assert!(
            validate(&config).is_err(),
            "accepted duplicate language tags"
        );
        config.spellcheck.languages = vec!["system".into(), "SYSTEM".into()];
        assert!(validate(&config).is_err(), "accepted duplicate system tags");
        config.spellcheck.languages = vec!["SYSTEM".into()];
        assert!(
            validate(&config).is_err(),
            "accepted an incorrectly cased system token"
        );
        config.spellcheck.languages = vec!["en".into(); 17];
        assert!(
            validate(&config).is_err(),
            "accepted an oversized language list"
        );
    }

    #[test]
    fn setting_registry_declares_metadata_for_static_and_dynamic_keys() {
        let metadata = setting_metadata("content.zoom").expect("content.zoom metadata");
        assert_eq!(metadata.value_type, "number");
        assert_eq!(metadata.default_value, "1.0");
        assert_eq!(metadata.supported_scopes, &["global", "profile", "site"]);
        assert_eq!(metadata.apply_time, "live");
        assert_eq!(metadata.prerequisite, "qt-webengine");

        let entry_mode = setting_metadata("input.entry_mode").expect("entry mode metadata");
        assert_eq!(entry_mode.apply_time, "navigation");
        assert_eq!(entry_mode.supported_scopes, &["global", "profile", "site"]);

        let dynamic = setting_metadata("search_engines.work").expect("dynamic metadata");
        assert_eq!(dynamic.value_type, "dynamic");
        assert_eq!(
            setting_supported_scopes("search_engines.work"),
            &["global", "profile"]
        );
        assert!(setting_metadata("not.a.setting").is_none());
    }

    #[test]
    fn setting_registry_entries_are_unique_and_complete() {
        let mut keys = BTreeSet::new();
        for metadata in SETTING_METADATA {
            assert!(keys.insert(metadata.key), "duplicate metadata key");
            assert!(!metadata.value_type.is_empty());
            assert!(!metadata.supported_scopes.is_empty());
            assert!(!metadata.apply_time.is_empty());
            assert!(!metadata.prerequisite.is_empty());
            assert!(!metadata.sensitivity.is_empty());
        }
        assert!(keys.len() >= 70);
    }

    #[test]
    fn reload_transaction_applies_immediate_values_and_reports_pending_values() {
        let current = toml::Value::try_from(Config::default()).expect("default TOML");
        let mut candidate = current.clone();
        candidate
            .get_mut("input")
            .and_then(toml::Value::as_table_mut)
            .expect("input table")
            .insert("entry_mode".into(), toml::Value::String("insert".into()));
        candidate
            .get_mut("session")
            .and_then(toml::Value::as_table_mut)
            .expect("session table")
            .insert("restore".into(), toml::Value::String("always".into()));
        candidate
            .get_mut("search_engines")
            .and_then(toml::Value::as_table_mut)
            .expect("search engines table")
            .remove("ddg");

        let active = apply_immediate_config_changes(&current, &candidate);
        assert_eq!(
            active
                .get("input")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("entry_mode")),
            Some(&toml::Value::String("insert".into()))
        );
        assert_eq!(
            active
                .get("session")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("restore")),
            current
                .get("session")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("restore"))
        );
        assert!(
            active
                .get("search_engines")
                .and_then(toml::Value::as_table)
                .is_some_and(|engines| !engines.contains_key("ddg"))
        );
        assert_eq!(
            pending_config_changes(&current, &candidate),
            vec![PendingSettingChange {
                key: "session.restore".into(),
                current: toml::Value::String("ask-after-crash".into()),
                pending: toml::Value::String("always".into()),
                apply_time: "startup",
            }]
        );
    }

    #[test]
    fn theme_palette_import_is_data_only_and_uses_deterministic_fallbacks() {
        let palette = parse_theme_palette(
            r##"
mode = "light"
accent = "#123456"
background = "#010203"
foreground = "#fefefe"
selection_background = "#223344"
red = "#aa0000"
"##,
            "omarchy",
            Path::new("/theme/colors.toml"),
        )
        .expect("palette");
        assert_eq!(palette.mode, "light");
        assert_eq!(palette.accent, "#123456");
        assert_eq!(palette.selection, "#223344");
        assert_eq!(palette.selection_foreground, "#fefefe");
        assert_eq!(palette.yellow, ThemePalette::default().yellow);
        assert_eq!(palette.green, ThemePalette::default().green);
    }

    #[test]
    fn omarchy_commit_metadata_accepts_only_bounded_git_refs() {
        let directory = std::env::temp_dir().join(format!(
            "rustbrowser-omarchy-git-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let git = directory.join(".git");
        fs::create_dir_all(git.join("refs/heads")).expect("git metadata directory");
        let commit = "0123456789abcdef0123456789abcdef01234567";
        fs::write(git.join("HEAD"), "ref: refs/heads/main\n").expect("symbolic HEAD");
        fs::write(git.join("refs/heads/main"), format!("{commit}\n")).expect("ref");
        assert_eq!(git_commit_at(&directory).as_deref(), Some(commit));

        fs::write(git.join("HEAD"), format!("{commit}\n")).expect("detached HEAD");
        assert_eq!(git_commit_at(&directory).as_deref(), Some(commit));

        fs::write(git.join("HEAD"), "ref: refs/../outside\n").expect("unsafe HEAD");
        assert!(git_commit_at(&directory).is_none());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn semantic_theme_tokens_use_spec_defaults_and_accept_alpha() {
        let defaults = ThemePalette::default();
        assert_eq!(defaults.background, "#1e1e2e");
        assert_eq!(defaults.foreground, "#cdd6f4");
        assert_eq!(defaults.accent, "#89b4fa");
        assert_eq!(defaults.error, "#f38ba8");
        assert_eq!(defaults.warning, "#f9e2af");
        assert_eq!(defaults.success, "#a6e3a1");

        let palette = parse_theme_palette(
            "surface = \"#313244cc\"\nborder = \"#585b70\"\nprivate = \"#cba6f7\"\nmode_insert = \"#f9e2af\"\n",
            "file",
            Path::new("/theme/colors.toml"),
        )
        .expect("semantic palette");
        assert_eq!(palette.surface, "#313244cc");
        assert_eq!(palette.border, "#585b70");
        assert_eq!(palette.private, "#cba6f7");
        assert_eq!(palette.mode_insert, "#f9e2af");
        assert_eq!(palette.lighter_background, "#313244cc");
    }

    #[test]
    fn builtin_theme_meets_wcag_aa_for_assessed_chrome_pairs() {
        let report = theme_contrast_report(&ThemePalette::default());
        assert_eq!(report.status, "pass");
        assert!(report.failing.is_empty());
        assert!(report.checks.iter().all(|check| check.passes_aa));
        assert!(contrast_ratio("#ffffff", "#000000").is_some_and(|ratio| ratio > 21.0 - 0.01));
    }

    #[test]
    fn imperfect_user_theme_is_reported_without_being_rejected() {
        let palette = ThemePalette {
            foreground: "#222222".into(),
            background: "#111111".into(),
            ..ThemePalette::default()
        };
        let report = theme_contrast_report(&palette);
        assert_eq!(report.status, "warning");
        assert!(
            report
                .failing
                .iter()
                .any(|name| name == "foreground-on-background")
        );
    }

    #[test]
    fn auto_theme_prefers_current_omarchy_path_and_explicit_path_wins() {
        let directory = std::env::temp_dir().join(format!(
            "rustbrowser-theme-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let current = directory.join(".local/state/omarchy/current/theme");
        let old = directory.join(".config/omarchy/current/theme");
        fs::create_dir_all(&current).expect("current theme directory");
        fs::create_dir_all(&old).expect("old theme directory");
        fs::write(current.join("colors.toml"), "accent = \"#112233\"\n").expect("current palette");
        fs::write(old.join("colors.toml"), "accent = \"#445566\"\n").expect("old palette");

        let auto = load_theme_palette_at(
            &ThemeConfig {
                source: ThemeSource::Auto,
                path: String::new(),
            },
            Some(&directory),
        )
        .expect("auto palette");
        assert_eq!(auto.accent, "#112233");
        assert_eq!(auto.source, "omarchy");
        assert_eq!(auto.provider_layout, "current");
        assert!(!auto.provider_version.contains('\n'));
        assert!(!auto.provider_commit.contains('\n'));

        let explicit = load_theme_palette_at(
            &ThemeConfig {
                source: ThemeSource::Builtin,
                path: old.join("colors.toml").to_string_lossy().into_owned(),
            },
            Some(&directory),
        )
        .expect("explicit palette");
        assert_eq!(explicit.accent, "#445566");
        assert_eq!(explicit.source, "file");
        assert!(explicit.provider_layout.is_empty());
        assert!(explicit.provider_version.is_empty());
        assert!(explicit.provider_commit.is_empty());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn theme_palette_rejects_invalid_colors() {
        let result = parse_theme_palette(
            "accent = \"javascript:bad\"\n",
            "file",
            Path::new("/theme/colors.toml"),
        );
        assert!(matches!(result, Err(ThemePaletteError::InvalidColor(_))));
    }

    #[test]
    fn reload_keeps_last_good_candidate_on_failure() {
        let directory =
            std::env::temp_dir().join(format!("rustbrowser-config-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("temp directory");
        let path = directory.join("config.toml");
        fs::write(&path, "[ui]\nfont_size_pt = 12.0\n").expect("valid config");
        let mut store = ConfigStore::new();
        store.reload(&path).expect("first reload");
        fs::write(&path, "[ui]\nfont_size_pt = 99.0\n").expect("invalid config");
        assert!(store.reload(&path).is_err());
        assert_eq!(store.revision(), 1);
        assert!(
            (store.current().expect("last good").config.ui.font_size_pt - 12.0).abs()
                < f64::EPSILON
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&directory);
    }

    #[test]
    fn contexts_save_round_trips_validated_routes_atomically() {
        let directory =
            std::env::temp_dir().join(format!("rustbrowser-context-save-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("temp directory");
        let path = directory.join("contexts.toml");
        let config = ContextsConfig {
            contexts: vec![ContextDefinition {
                name: "work".into(),
                label: "Work".into(),
                profile: "work".into(),
                sessions: Vec::new(),
                workspace: None,
                accent: None,
                default_target: "reuse-or-window".into(),
            }],
            routes: vec![ContextRoute {
                id: "company-work".into(),
                pattern: "https://*.company.test/*".into(),
                context: "work".into(),
                priority: 0,
                behavior: "prompt".into(),
                entry_points: vec!["explicit-open".into()],
            }],
            ..ContextsConfig::default()
        };
        save_contexts_atomic(&path, &config).expect("atomic contexts save");
        assert_eq!(load_contexts(&path).expect("saved contexts"), config);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&directory);
    }

    #[test]
    fn profiles_load_selects_one_default_and_applies_typed_overrides() {
        let directory = std::env::temp_dir().join(format!(
            "rustbrowser-config-profiles-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temp directory");
        let path = directory.join("profiles.toml");
        fs::write(
            &path,
            r#"
schema_version = 1

[[profiles]]
name = "work"
label = "Work"
default = true
overrides = { "content.zoom" = 1.25, "input.entry_mode" = "insert" }
"#,
        )
        .expect("profiles");
        let profiles = load_profiles(&path).expect("valid profiles");
        let profile = profiles.default_profile().expect("default profile");
        let layer = profile_override_layer(profile).expect("profile layer");
        let config = apply_runtime_overrides(&Config::default(), &layer).expect("apply profile");
        assert_eq!(profile.name, "work");
        assert!((config.content.zoom - 1.25).abs() < f64::EPSILON);
        assert_eq!(config.input.entry_mode, EntryMode::Insert);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&directory);
    }

    #[test]
    fn profiles_reject_duplicate_defaults_unknown_keys_and_site_rules() {
        let duplicate_defaults = ProfilesConfig {
            profiles: vec![
                ProfileDefinition {
                    name: "one".into(),
                    label: "One".into(),
                    default: true,
                    overrides: BTreeMap::new(),
                },
                ProfileDefinition {
                    name: "two".into(),
                    label: "Two".into(),
                    default: true,
                    overrides: BTreeMap::new(),
                },
            ],
            ..ProfilesConfig::default()
        };
        assert!(validate_profiles(&duplicate_defaults).is_err());

        let mut unknown = ProfileDefinition {
            name: "work".into(),
            label: "Work".into(),
            default: false,
            overrides: BTreeMap::new(),
        };
        unknown
            .overrides
            .insert("not.a_setting".into(), toml::Value::Boolean(true));
        assert!(validate_profile_definition(&unknown).is_err());
        unknown.overrides.clear();
        unknown
            .overrides
            .insert("site_rules".into(), toml::Value::Array(Vec::new()));
        assert!(validate_profile_definition(&unknown).is_err());
    }

    #[test]
    fn includes_merge_in_order_and_reject_cycles() {
        let directory =
            std::env::temp_dir().join(format!("rustbrowser-config-include-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("temp directory");
        fs::write(
            directory.join("base.toml"),
            "[ui]\nfont_size_pt = 11.0\nfont_family = \"serif\"\n",
        )
        .expect("base");
        fs::write(
            directory.join("config.toml"),
            "include = [\"base.toml\"]\n[ui]\nfont_size_pt = 13.0\n",
        )
        .expect("root");
        let loaded = load(directory.join("config.toml")).expect("include");
        assert!((loaded.config.ui.font_size_pt - 13.0).abs() < f64::EPSILON);
        assert_eq!(loaded.config.ui.font_family, "serif");
        fs::write(directory.join("base.toml"), "include = [\"config.toml\"]\n").expect("cycle");
        assert!(matches!(
            load(directory.join("config.toml")),
            Err(ConfigError::IncludeCycle { .. })
        ));
        let _ = fs::remove_file(directory.join("base.toml"));
        let _ = fs::remove_file(directory.join("config.toml"));
        let _ = fs::remove_dir(&directory);
    }

    #[test]
    fn runtime_overrides_round_trip_atomically_and_apply_by_key() {
        let directory = std::env::temp_dir().join(format!(
            "rustbrowser-runtime-overrides-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temp directory");
        let path = directory.join("runtime-overrides.toml");
        let mut overrides = RuntimeOverrides::default();
        overrides
            .set_literal("content.zoom", "1.25")
            .expect("numeric override");
        overrides
            .set_literal("input.entry_mode", "insert")
            .expect("string override");
        overrides
            .set_binding("normal", "g,g", "")
            .expect("explicit unbound binding");
        save_runtime_overrides_atomic(&path, &overrides).expect("atomic save");

        let loaded = load_runtime_overrides(&path).expect("load");
        assert_eq!(loaded.settings, overrides.settings);
        assert_eq!(loaded.bindings, overrides.bindings);
        assert_eq!(loaded.site_rules, overrides.site_rules);
        let config = apply_runtime_overrides(&Config::default(), &loaded).expect("apply");
        assert!((config.content.zoom - 1.25).abs() < f64::EPSILON);
        assert_eq!(config.input.entry_mode, EntryMode::Insert);
        assert_eq!(
            config
                .bindings
                .get("normal")
                .and_then(|bindings| bindings.get("g,g"))
                .map(String::as_str),
            Some("unbound")
        );

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&directory);
    }

    #[test]
    fn runtime_site_rules_round_trip_override_and_unset() {
        let mut overrides = RuntimeOverrides::default();
        overrides
            .set_site_literal("https://docs.example/*", "input.entry_mode", "insert")
            .expect("site setting");
        overrides
            .set_site_literal("https://docs.example/*", "content.zoom", "1.25")
            .expect("second site setting");
        let config = apply_runtime_overrides(&Config::default(), &overrides).expect("apply");
        let rules = matching_site_rules(&config, "https://docs.example/editor");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].priority, i32::MAX);
        assert_eq!(rules[0].set["input.entry_mode"].as_str(), Some("insert"));
        assert_eq!(rules[0].set["content.zoom"].as_float(), Some(1.25));

        assert!(
            overrides
                .unset_site_setting("https://docs.example/*", "input.entry_mode")
                .expect("unset site setting")
        );
        assert!(
            !overrides
                .unset_site_setting("https://docs.example/*", "input.entry_mode")
                .expect("repeat unset")
        );
        assert!(
            overrides
                .unset_site_setting("https://docs.example/*", "content.zoom")
                .expect("unset final site setting")
        );
        assert!(overrides.site_rules.is_empty());
        assert!(
            overrides
                .set_site_literal("https://docs.example/*", "ui.font_size_pt", "12")
                .is_err()
        );
    }

    #[test]
    fn runtime_override_unset_preserves_lower_layer_and_rejects_bad_types() {
        let mut overrides = RuntimeOverrides::default();
        overrides
            .set_literal("content.zoom", "1.5")
            .expect("override");
        assert!(overrides.unset_setting("content.zoom").expect("unset"));
        assert!(
            !overrides
                .unset_setting("content.zoom")
                .expect("second unset")
        );

        overrides
            .set_literal("content.zoom", "not-a-number")
            .expect("string is stored until typed application");
        let error = apply_runtime_overrides(&Config::default(), &overrides)
            .expect_err("wrong type rejected");
        assert!(error.to_string().contains("wrong type"));
    }

    #[test]
    fn blocking_bypass_sites_are_validated_and_runtime_overridable() {
        let mut config = Config::default();
        config.blocking.bypass_sites = vec!["example.test".into(), "*.sub.test".into()];
        validate(&config).expect("valid host bypasses");

        config.blocking.bypass_sites = vec!["https://example.test".into()];
        assert!(validate(&config).is_err());

        let mut overrides = RuntimeOverrides::default();
        overrides
            .set_literal("blocking.bypass_sites", "[\"example.test\"]")
            .expect("array override");
        let effective = apply_runtime_overrides(&Config::default(), &overrides).expect("apply");
        assert_eq!(effective.blocking.bypass_sites, ["example.test"]);
    }

    #[test]
    fn link_cleaning_updates_require_explicit_https_source_and_pinned_checksum() {
        let mut config = Config::default();
        config.links.cleaning.update_source =
            Some("https://updates.example.test/rustbrowser-links.toml".into());
        config.links.cleaning.update_sha256 = Some("a".repeat(64));
        validate(&config).expect("pinned HTTPS link source");

        config.links.cleaning.update_source = Some("http://updates.example.test/rules".into());
        assert!(validate(&config).is_err());

        config.links.cleaning.update_source = None;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn context_document_validates_metadata_and_route_references() {
        let directory =
            std::env::temp_dir().join(format!("rustbrowser-context-config-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("temp directory");
        let path = directory.join("contexts.toml");
        fs::write(
            &path,
            r##"schema_version = 1

[[contexts]]
name = "work"
label = "Work"
profile = "work"
workspace = "3"
accent = "#7aa2f7"

[[routes]]
id = "company-work"
pattern = "https://*.company.test/*"
context = "work"
entry_points = ["explicit-open"]
"##,
        )
        .expect("contexts");
        let loaded = load_contexts(&path).expect("valid contexts");
        assert_eq!(loaded.contexts[0].default_target, "reuse-or-window");
        assert_eq!(loaded.routes[0].behavior, "prompt");
        fs::write(
            &path,
            "[[routes]]\nid = \"bad\"\npattern = \"https://example.test\"\ncontext = \"missing\"\n",
        )
        .expect("invalid contexts");
        assert!(load_contexts(&path).is_err());
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&directory);
    }

    #[test]
    fn context_routes_use_narrow_host_port_path_and_entry_point_matching() {
        let config = ContextsConfig {
            routes: vec![
                ContextRoute {
                    id: "broad".into(),
                    pattern: "*://*.example.test/*".into(),
                    context: "broad".into(),
                    priority: 1,
                    behavior: "prompt".into(),
                    entry_points: vec!["explicit-open".into()],
                },
                ContextRoute {
                    id: "specific".into(),
                    pattern: "https://app.example.test:8443/docs/*".into(),
                    context: "specific".into(),
                    priority: 1,
                    behavior: "prompt".into(),
                    entry_points: vec!["explicit-open".into()],
                },
                ContextRoute {
                    id: "higher-priority".into(),
                    pattern: "https://app.example.test:8443/docs/*".into(),
                    context: "higher".into(),
                    priority: 4,
                    behavior: "suggest".into(),
                    entry_points: vec!["typed-initial-url".into()],
                },
            ],
            ..ContextsConfig::default()
        };
        let matches = matching_context_routes(
            &config,
            "https://app.example.test:8443/docs/start?token=ignored",
            "explicit-open",
        );
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "specific");
        assert!(
            matching_context_routes(
                &config,
                "https://badexample.test/docs/start",
                "explicit-open"
            )
            .is_empty()
        );
        assert!(
            matching_context_routes(
                &config,
                "https://app.example.test/docs/start",
                "explicit-open"
            )
            .iter()
            .any(|route| route.id == "broad")
        );
        assert!(
            matching_context_routes(
                &config,
                "https://app.example.test:8443/docs/start",
                "typed-initial-url"
            )
            .iter()
            .any(|route| route.id == "higher-priority")
        );
        assert!(
            matching_context_routes(&config, "file:///tmp/page.html", "explicit-open").is_empty()
        );
    }

    #[test]
    fn context_route_validation_rejects_ambiguous_patterns() {
        for pattern in [
            "ftp://example.test/*",
            "https://bad*example.test/*",
            "https://*.example.test/?token=secret",
            "https://example.test:0/*",
            "https://example.test/*#fragment",
        ] {
            let config = ContextsConfig {
                contexts: vec![ContextDefinition {
                    name: "work".into(),
                    label: "Work".into(),
                    profile: "work".into(),
                    sessions: Vec::new(),
                    workspace: None,
                    accent: None,
                    default_target: "reuse-or-window".into(),
                }],
                routes: vec![ContextRoute {
                    id: "route".into(),
                    pattern: pattern.into(),
                    context: "work".into(),
                    priority: 0,
                    behavior: "prompt".into(),
                    entry_points: vec!["explicit-open".into()],
                }],
                ..ContextsConfig::default()
            };
            assert!(validate_contexts(&config).is_err(), "pattern {pattern}");
        }
    }

    #[test]
    fn context_route_validation_rejects_unknown_entry_points_and_conflicts() {
        let context = |name: &str| ContextDefinition {
            name: name.into(),
            label: name.into(),
            profile: name.into(),
            sessions: Vec::new(),
            workspace: None,
            accent: None,
            default_target: "reuse-or-window".into(),
        };
        let route = |id: &str, target: &str| ContextRoute {
            id: id.into(),
            pattern: "https://example.test/*".into(),
            context: target.into(),
            priority: 2,
            behavior: "prompt".into(),
            entry_points: vec!["explicit-open".into()],
        };
        let mut invalid_entry_point = ContextsConfig {
            contexts: vec![context("work")],
            routes: vec![route("route", "work")],
            ..ContextsConfig::default()
        };
        invalid_entry_point.routes[0].entry_points = vec!["address-bar".into()];
        assert!(validate_contexts(&invalid_entry_point).is_err());

        let ambiguous = ContextsConfig {
            contexts: vec![context("work"), context("personal")],
            routes: vec![
                route("work-route", "work"),
                route("personal-route", "personal"),
            ],
            ..ContextsConfig::default()
        };
        assert!(validate_contexts(&ambiguous).is_err());
    }
}
