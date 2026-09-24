import QtQuick
import "../scripts/ChromePresentation.js" as ChromePresentation

// Projects already-validated BrowserUi chrome facts into the QML window. It
// has no navigation, persistence, or page authority.
QtObject {
    id: controller

    required property var browserWindow
    required property var browserUi

    function contrastText(background) {
        return ChromePresentation.contrastText(background)
    }

    function readableTextColor(candidate, background) {
        return ChromePresentation.readableTextColor(candidate, background)
    }

    function renderedContrastReport() {
        var window = controller.browserWindow
        return ChromePresentation.contrastReport({
            primaryText: window.primaryTextColor,
            background: window.backgroundColor,
            secondaryText: window.secondaryTextColor,
            surface: window.surfaceColor,
            mutedText: window.mutedTextColor,
            panel: window.panelColor,
            accent: window.accentColor
        })
    }

    function refreshChromeAppearance() {
        var window = controller.browserWindow
        var ui = controller.browserUi
        window.backgroundColor = ui.theme_background_color
        window.surfaceColor = ui.theme_surface_color
        window.panelColor = ui.theme_panel_color
        window.primaryTextColor = ui.theme_primary_text_color
        window.secondaryTextColor = ui.theme_secondary_text_color
        window.mutedTextColor = ui.theme_muted_text_color
        window.borderColor = ui.theme_border_color
        window.accentColor = ui.theme_accent_color
        window.warningColor = ui.theme_warning_color
        window.errorColor = ui.theme_error_color
        window.successColor = ui.theme_success_color
        window.privateColor = ui.theme_private_color
        window.modeInsertColor = ui.theme_mode_insert_color
        window.selectionColor = ui.theme_selection_color
        window.selectionTextColor = ui.theme_selection_text_color
        var renderedContrast = controller.renderedContrastReport()
        window.renderedThemeContrastStatus = renderedContrast.status
        window.chromeFontFamily = ui.chrome_font_family
        window.systemFontScale = ui.system_font_scale_status === "available"
                ? ui.system_font_scale : 1.0
        window.chromeFontPointSize = Math.min(60, Math.max(6, ui.chrome_font_size_pt
                                                           * window.systemFontScale))
        window.statusbarMode = ui.chrome_statusbar_mode
        window.tabsMode = ui.chrome_tabs_mode
        window.tabPosition = ui.chrome_tab_position
        window.systemReducedMotionStatus = ui.system_reduced_motion_status
        if (ui.chrome_reduced_motion === "on") {
            window.reducedMotionActive = true
        } else if (ui.chrome_reduced_motion === "off") {
            window.reducedMotionActive = false
        } else {
            window.reducedMotionActive = ui.system_reduced_motion_status === "available"
                    ? ui.system_reduced_motion_enabled : true
        }
        window.themeContrastStatus = ui.theme_contrast_status
        window.themeContrastWarning = (window.themeContrastStatus === "warning"
                || window.renderedThemeContrastStatus === "warning")
                ? ui.theme_contrast_reason : ""
    }
}
