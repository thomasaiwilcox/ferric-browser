import QtQuick
import "../scripts/ChromePresentation.js" as ChromePresentation

// Read-only presentation of the owning profile's validated chrome values.
QtObject {
    id: appearance
    required property var browserUi
    required property var hostWindow
    required property var themeWindow
    readonly property real systemFontScale: browserUi.system_font_scale_status === "available" ? browserUi.system_font_scale : themeWindow.systemFontScale
    readonly property font font: Qt.font({
        family: browserUi.chrome_font_family,
        pointSize: Math.min(60, Math.max(6, browserUi.chrome_font_size_pt * systemFontScale))
    })
    property var metrics: FontMetrics {
        font: appearance.font
    }
    readonly property real chromeScale: Math.max(1, metrics.height / 14)
    readonly property real chromePadding: Math.max(4, Math.ceil(metrics.height * 0.25))
    readonly property real chromeRowHeight: Math.ceil(metrics.height + chromePadding * 1.25)
    readonly property real tabBarHeight: Math.ceil(metrics.height + chromePadding * 1.5)
    readonly property real inputBarHeight: Math.ceil(metrics.height + chromePadding * 2)
    readonly property real statusBarHeight: inputBarHeight
    readonly property real chromeOpacity: 1
    readonly property bool runtimeDisposing: hostWindow.runtimeReleased === true
    readonly property color backgroundColor: browserUi.theme_background_color
    readonly property color surfaceColor: browserUi.theme_surface_color
    readonly property color panelColor: browserUi.theme_panel_color
    readonly property color primaryTextColor: browserUi.theme_primary_text_color
    readonly property color secondaryTextColor: browserUi.theme_secondary_text_color
    readonly property color mutedTextColor: browserUi.theme_muted_text_color
    readonly property color borderColor: browserUi.theme_border_color
    readonly property color accentColor: browserUi.theme_accent_color
    readonly property color warningColor: browserUi.theme_warning_color
    readonly property color errorColor: browserUi.theme_error_color
    readonly property color successColor: browserUi.theme_success_color
    readonly property color privateColor: browserUi.theme_private_color
    readonly property color modeInsertColor: browserUi.theme_mode_insert_color
    readonly property color selectionColor: browserUi.theme_selection_color
    readonly property color selectionTextColor: browserUi.theme_selection_text_color
    function contrastText(background) {
        return ChromePresentation.contrastText(background);
    }
    function readableTextColor(candidate, background) {
        return ChromePresentation.readableTextColor(candidate, background);
    }
    function addressPresentation(value, limit) {
        return themeWindow.addressPresentation(value, limit);
    }
    function statusBarVisibleForMode(mode) {
        return ChromePresentation.statusBarVisible(browserUi.chrome_statusbar_mode, mode);
    }
}
