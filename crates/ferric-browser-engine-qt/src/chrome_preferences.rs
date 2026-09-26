//! Typed chrome-preference projection for the QML presentation boundary.
//!
//! QML receives only the values it needs to render the browser chrome.  It
//! must not parse the complete configuration snapshot to decide layout or
//! accessibility behavior.

use ferric_browser_config::{Config, ReducedMotion};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ChromePreferences {
    pub font_family: String,
    pub font_size_pt: f64,
    pub statusbar_mode: &'static str,
    pub tabs_mode: &'static str,
    pub tab_position: &'static str,
    pub reduced_motion: &'static str,
}

#[must_use]
pub(super) fn project(config: &Config) -> ChromePreferences {
    ChromePreferences {
        font_family: nonempty_font_family(&config.ui.font_family),
        font_size_pt: config.ui.font_size_pt.clamp(6.0, 40.0),
        statusbar_mode: statusbar_mode(&config.ui.statusbar),
        tabs_mode: tabs_mode(&config.ui.tabs),
        tab_position: tab_position(&config.ui.tab_position),
        reduced_motion: reduced_motion(&config.ui.reduced_motion),
    }
}

fn nonempty_font_family(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "monospace".into()
    } else {
        value.into()
    }
}

fn statusbar_mode(value: &str) -> &'static str {
    match value {
        "always" => "always",
        "in-mode" | "command" => "in-mode",
        "never" => "never",
        _ => "in-mode",
    }
}

fn tabs_mode(value: &str) -> &'static str {
    match value {
        "always" => "always",
        "multiple" => "multiple",
        "switching" => "switching",
        "never" => "never",
        _ => "multiple",
    }
}

fn tab_position(value: &str) -> &'static str {
    match value {
        "top" => "top",
        "bottom" => "bottom",
        "left" => "left",
        "right" => "right",
        _ => "top",
    }
}

fn reduced_motion(value: &ReducedMotion) -> &'static str {
    match value {
        ReducedMotion::System => "system",
        ReducedMotion::On => "on",
        ReducedMotion::Off => "off",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_keeps_chrome_values_in_the_valid_presentation_domain() {
        let mut config = Config::default();
        config.ui.font_family = "  ".into();
        config.ui.font_size_pt = 100.0;
        config.ui.statusbar = "unknown".into();
        config.ui.tabs = "unknown".into();
        config.ui.tab_position = "unknown".into();
        config.ui.reduced_motion = ReducedMotion::On;

        assert_eq!(
            project(&config),
            ChromePreferences {
                font_family: "monospace".into(),
                font_size_pt: 40.0,
                statusbar_mode: "in-mode",
                tabs_mode: "multiple",
                tab_position: "top",
                reduced_motion: "on",
            }
        );
    }

    #[test]
    fn statusbar_projection_uses_in_mode_and_normalizes_the_legacy_alias() {
        let mut config = Config::default();
        assert_eq!(project(&config).statusbar_mode, "in-mode");

        config.ui.statusbar = "command".into();
        assert_eq!(project(&config).statusbar_mode, "in-mode");

        config.ui.statusbar = "always".into();
        assert_eq!(project(&config).statusbar_mode, "always");

        config.ui.statusbar = "never".into();
        assert_eq!(project(&config).statusbar_mode, "never");
    }
}
