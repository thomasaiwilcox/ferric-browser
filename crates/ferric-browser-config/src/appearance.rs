//! Chrome and theme configuration, palette loading, and contrast projection.

use super::{
    Deserialize, Path, PathBuf, ReducedMotion, Serialize, ThemeSource, default_font_family,
    default_font_size, default_statusbar, default_tab_position, default_tabs, fs,
};

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

pub(super) fn load_theme_palette_at(
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

pub(super) fn git_commit_at(root: &Path) -> Option<String> {
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

pub(super) fn parse_theme_palette(
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
