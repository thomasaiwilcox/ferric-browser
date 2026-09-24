//! Typed, accessible chrome projection for a validated theme palette.

use ferric_browser_config::{ThemePalette, contrast_ratio, theme_contrast_report};

pub(super) struct ChromeTheme {
    pub(super) background: String,
    pub(super) surface: String,
    pub(super) panel: String,
    pub(super) primary_text: String,
    pub(super) secondary_text: String,
    pub(super) muted_text: String,
    pub(super) border: String,
    pub(super) accent: String,
    pub(super) warning: String,
    pub(super) error: String,
    pub(super) success: String,
    pub(super) private: String,
    pub(super) mode_insert: String,
    pub(super) selection: String,
    pub(super) selection_text: String,
    pub(super) contrast_status: String,
    pub(super) contrast_reason: String,
}

pub(super) fn project(palette: &ThemePalette) -> ChromeTheme {
    let background = opaque(&palette.background, "#1e1e2e");
    let surface = opaque(&palette.surface, "#313244");
    let panel = opaque(&palette.dark_background, "#181825");
    let selection = opaque(&palette.selection_background, "#45475a");
    let report = theme_contrast_report(palette);
    ChromeTheme {
        primary_text: readable(&palette.foreground, &background, "#cdd6f4"),
        secondary_text: readable(&palette.selection_foreground, &surface, "#cdd6f4"),
        muted_text: readable(&palette.muted, &panel, "#a6adc8"),
        border: opaque(&palette.border, "#585b70"),
        accent: readable(&palette.accent, &background, "#89b4fa"),
        warning: readable(&palette.warning, &background, "#f9e2af"),
        error: readable(&palette.error, &background, "#f38ba8"),
        success: readable(&palette.success, &background, "#a6e3a1"),
        private: readable(&palette.private, &background, "#cba6f7"),
        mode_insert: readable(&palette.mode_insert, &background, "#f9e2af"),
        selection_text: readable(&palette.selection_foreground, &selection, "#cdd6f4"),
        background,
        surface,
        panel,
        selection,
        contrast_status: report.status,
        contrast_reason: if report.reason.is_empty() {
            "Some theme colors may be difficult to read".into()
        } else {
            report.reason
        },
    }
}

fn opaque(value: &str, fallback: &str) -> String {
    let value = valid_color(value).unwrap_or(fallback);
    format!("{}ff", &value[..7])
}

fn readable(value: &str, background: &str, fallback: &str) -> String {
    let candidate = valid_color(value).unwrap_or(fallback);
    if contrast_ratio(candidate, background).is_some_and(|ratio| ratio >= 4.5) {
        candidate.to_owned()
    } else {
        contrast_text(background)
    }
}

fn contrast_text(background: &str) -> String {
    let black = contrast_ratio("#000000", background).unwrap_or(0.0);
    let white = contrast_ratio("#ffffff", background).unwrap_or(0.0);
    if black >= white {
        "#000000".into()
    } else {
        "#ffffff".into()
    }
}

fn valid_color(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    ((bytes.len() == 7 || bytes.len() == 9)
        && bytes.first() == Some(&b'#')
        && bytes[1..].iter().all(u8::is_ascii_hexdigit))
    .then_some(value)
}

#[cfg(test)]
mod tests {
    use super::project;
    use ferric_browser_config::ThemePalette;

    #[test]
    fn projects_opaque_surfaces_and_readable_semantic_text() {
        let palette = ThemePalette {
            foreground: "#202020".into(),
            background: "#1e1e2e80".into(),
            ..ThemePalette::default()
        };
        let theme = project(&palette);
        assert_eq!(theme.background, "#1e1e2eff");
        assert_eq!(theme.primary_text, "#ffffff");
        assert_eq!(theme.contrast_status, "warning");
    }
}
