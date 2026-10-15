import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
import "../../qml/scripts/ChromePresentation.js" as ChromePresentation

TestCase {
    id: test
    name: "ProductPolish"
    when: windowShown

    property string downloadActionId: ""
    property string switcherAction: ""
    ApplicationWindow {
        id: host
        width: 900
        height: 700
        visible: true
        font.family: "monospace"
        font.pointSize: 10
        property real chromeScale: 1
        property real chromeRowHeight: 24
        property color backgroundColor: "#181825"
        property color surfaceColor: "#313244"
        property color panelColor: "#1e1e2e"
        property color primaryTextColor: "#cdd6f4"
        property color secondaryTextColor: "#bac2de"
        property color mutedTextColor: "#a6adc8"
        property color borderColor: "#585b70"
        property color accentColor: "#89b4fa"
        property color warningColor: "#f9e2af"
        property color errorColor: "#f38ba8"
        property color selectionColor: "#45475a"
        property color selectionTextColor: "#cdd6f4"
        property bool downloadManagerVisible: true
        property bool sessionManagerVisible: true
        property bool temporaryProfile: false
        property bool ephemeralProfile: false
        color: backgroundColor
        palette.window: backgroundColor
        palette.windowText: primaryTextColor
        palette.base: panelColor
        palette.text: primaryTextColor
        palette.placeholderText: mutedTextColor
        palette.button: surfaceColor
        palette.buttonText: primaryTextColor
        palette.highlight: accentColor
        palette.highlightedText: backgroundColor
    }
    QtObject {
        id: navigationUi
        property bool navigation_failure_visible: true
        property string navigation_failure_kind: "dns"
        property string navigation_failure_url: "https://unreachable.example/"
        property string navigation_failure_requested_url: "https://unreachable.example/"
        property string navigation_failure_detail: "net::ERR_NAME_NOT_RESOLVED"
        function clear_navigation_failure() { navigation_failure_visible = false }
    }
    ListModel { id: downloads }
    Component {
        id: downloadFixture
        FerricDownloadManager {
            browserWindow: host
            downloadsModel: downloads
            onOpenRequested: function(downloadId, reveal) { test.downloadActionId = downloadId }
        }
    }
    Component {
        id: navigationFixture
        FerricNavigationFailure { browserWindow: host; browserUi: navigationUi }
    }
    Component {
        id: sessionsFixture
        FerricSessionManager { browserWindow: host; sessionsModel: [] }
    }
    Component {
        id: plainWindowFixture
        ApplicationWindow { width: 390; height: 600; visible: true; font: host.font }
    }
    Component {
        id: shutdownFixture
        FerricShutdownDecisionDialog { promptVisible: true }
    }
    Component {
        id: switcherFixture
        FerricSwitcherResults {
            width: 350
            height: 280
            browserWindow: host
            results: [{ kind: "session", label: "<b>Research</b>", profile: "Personal",
                        secondary: "3 tabs", actions: ["load-preview", "load"] }]
            onActionRequested: function(index, action) { test.switcherAction = action }
        }
    }

    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width = 900
        host.height = 700
        host.font.pointSize = 10
        host.requestActivate()
        tryCompare(host, "active", true)
        navigationUi.navigation_failure_visible = true
        host.temporaryProfile = false
        host.ephemeralProfile = false
        downloadActionId = ""
        switcherAction = ""
        downloads.clear()
    }
    function create(fixture) {
        var item = createTemporaryObject(fixture, host.contentItem)
        verify(item !== null)
        verify(waitForPolish(host))
        return item
    }
    function capture(name) {
        if (testArtifactDirectory.length > 0) {
            verify(waitForPolish(host))
            wait(50)
            grabImage(host.contentItem).save(testArtifactDirectory + "/" + name + ".png")
        }
    }
    function inside(item, panel) {
        var position = item.mapToItem(panel, 0, 0)
        verify(position.x >= -1 && position.y >= -1)
        verify(position.x + item.width <= panel.width + 1)
        verify(position.y + item.height <= panel.height + 1)
    }
    function test_readable_progress_handles_unknown_sizes() {
        compare(ChromePresentation.downloadProgress(1572864, 3145728, 1048576, "in-progress"),
                "1.5 MiB of 3.0 MiB · 1.0 MiB/s")
        compare(ChromePresentation.downloadProgress(0, -1, 0, "in-progress"), "0 B")
        compare(ChromePresentation.downloadProgress(1024, 2048, 1024, "paused"), "1.0 KiB of 2.0 KiB")
        compare(ChromePresentation.byteSize(NaN), "Size unknown")
        compare(ChromePresentation.byteSize(-1), "Size unknown")
        compare(ChromePresentation.downloadStateLabel("constructor"), "Status unavailable")
    }
    function test_shutdown_has_themed_safe_actions_with_long_text() {
        host.font.pointSize = 20
        var child = createTemporaryObject(plainWindowFixture)
        verify(child)
        var surface = createTemporaryObject(shutdownFixture, child.contentItem, {
            hostWindow: child, themeWindow: host,
            title: "<b>Close this window?</b>",
            message: "This page has unsaved form or editor state. ".repeat(12),
            proceedLabel: "Cancel active work and close this window"
        })
        verify(surface)
        child.requestActivate()
        tryCompare(child, "active", true)
        var keep = findChild(surface, "shutdownKeepButton")
        var proceed = findChild(surface, "shutdownProceedButton")
        var panel = findChild(surface, "shutdownDecisionPanel")
        compare(panel.color, host.panelColor)
        compare(findChild(surface, "shutdownTitle").textFormat, Text.PlainText)
        tryVerify(function() { return keep.activeFocus })
        verify(waitForPolish(child))
        inside(keep, panel)
        inside(proceed, panel)
        var scroll = findChild(surface, "shutdownMessageScroll")
        verify(scroll.contentHeight > scroll.height)
        var title = findChild(surface, "shutdownTitle")
        var message = findChild(surface, "shutdownMessage")
        verify(title.height >= title.implicitHeight - 1)
        verify(message.height >= message.implicitHeight - 1)
        verify(message.y >= title.y + title.height)
        var scrollbar = findChild(surface, "shutdownMessageScrollbar")
        verify(scrollbar)
        var scrollbarPosition = scrollbar.mapToItem(scroll, 0, 0)
        verify(scrollbarPosition.x > scroll.width / 2)
        if (testArtifactDirectory.length > 0)
            grabImage(child.contentItem).save(testArtifactDirectory + "/shutdown-narrow-large-font.png")
        mouseWheel(scroll, scroll.width / 2, scroll.height / 2, 0, -240)
        tryVerify(function() { return scroll.contentItem.contentY > 0 })
        inside(keep, panel)
        inside(proceed, panel)
        var spy = createTemporaryObject(signalSpyFixture, surface, {target: surface, signalName: "keepRequested"})
        keyClick(Qt.Key_Escape)
        compare(spy.count, 1)
    }
    Component { id: signalSpyFixture; SignalSpy {} }
    function test_navigation_failure_data() {
        return [{tag: "wide", width: 900, height: 700},
                {tag: "compact", width: 640, height: 480},
                {tag: "narrow", width: 390, height: 600}]
    }
    function test_navigation_failure(data) {
        host.width = data.width
        host.height = data.height
        var surface = create(navigationFixture)
        verify(surface.message.indexOf("address") >= 0)
        var details = findChild(surface, "navigationFailureDetails")
        var button = findChild(surface, "navigationFailureDetailsButton")
        verify(!details.visible)
        inside(button, surface.panelItem)
        inside(findChild(surface, "commandDialogAction-cancel"), surface.panelItem)
        capture("navigation-" + data.tag)
        mouseClick(button)
        tryVerify(function() { return details.visible })
        verify(details.text.indexOf("ERR_NAME_NOT_RESOLVED") >= 0)
        compare(details.textFormat, TextEdit.PlainText)
        capture("navigation-details-" + data.tag)
        host.requestActivate()
        tryCompare(host, "active", true)
        keyClick(Qt.Key_Escape)
        tryCompare(navigationUi, "navigation_failure_visible", false)
    }
    function test_downloads_use_filename_and_keep_command_id_in_details() {
        downloads.append({ id: "download-command-id", state: "completed", bytes: "1572864",
                           total: 1572864, speed: 0, destination: "/home/test/Downloads/archive-λ.tar.gz",
                           reason: "No reason" })
        var surface = create(downloadFixture)
        var list = findChild(surface, "downloadList")
        tryCompare(list, "count", 1)
        var row = list.itemAtIndex(0)
        verify(row)
        compare(findChild(row, "downloadFileName").text, "archive-λ.tar.gz")
        var details = findChild(row, "downloadDetails")
        verify(!details.visible)
        capture("downloads-complete")
        mouseClick(findChild(row, "downloadDetailsButton"))
        tryVerify(function() { return details.visible })
        verify(details.text.indexOf("download-command-id") >= 0)
        verify(details.text.indexOf("No reason") < 0)
        var open = findChild(row, "downloadOpenButton")
        verify(open)
        mouseClick(open)
        compare(downloadActionId, "download-command-id")
    }
    function test_unknown_download_size_stops_animating_when_paused() {
        downloads.append({ id: "active", state: "in-progress", bytes: "4096", total: -1,
                           speed: 1024, destination: "/tmp/archive.zip", reason: "" })
        var surface = create(downloadFixture)
        var list = findChild(surface, "downloadList")
        tryCompare(list, "count", 1)
        var progress = findChild(list.itemAtIndex(0), "downloadProgressBar")
        verify(progress.visible && progress.indeterminate)
        downloads.setProperty(0, "state", "paused")
        tryCompare(progress, "indeterminate", false)
        capture("downloads-paused")
    }
    function test_download_failure_details_remain_available() {
        downloads.append({ id: "interrupted", state: "interrupted", bytes: "4096", total: 8192,
                           speed: 0, destination: "/tmp/archive.zip", reason: "Destination is not writable" })
        var surface = create(downloadFixture)
        var list = findChild(surface, "downloadList")
        tryCompare(list, "count", 1)
        var row = list.itemAtIndex(0)
        var details = findChild(row, "downloadDetails")
        verify(!details.visible)
        mouseClick(findChild(row, "downloadDetailsButton"))
        tryVerify(function() { return details.visible })
        verify(details.text.indexOf("Destination is not writable") >= 0)
    }
    function test_empty_downloads_have_an_explanation() {
        var surface = create(downloadFixture)
        verify(findChild(surface, "downloadsEmptyState").visible)
        capture("downloads-empty")
    }
    function test_private_sessions_explain_why_saving_is_unavailable() {
        host.temporaryProfile = true
        var surface = create(sessionsFixture)
        verify(!findChild(surface, "sessionSaveButton").enabled)
        verify(surface.message.indexOf("regular profile") >= 0)
    }
    function test_switcher_readable_actions_retain_canonical_intents() {
        var surface = create(switcherFixture)
        var list = findChild(surface, "switcherResultList")
        tryCompare(list, "count", 1)
        var row = list.itemAtIndex(0)
        verify(row)
        var action = findChild(row, "switcherActionButton")
        compare(action.text, "Preview session")
        mouseClick(action)
        compare(switcherAction, "load-preview")
        verify(row.Accessible.name.indexOf("<b>Research</b>") >= 0)
        surface.results = []
        tryCompare(list, "count", 0)
        verify(findChild(surface, "switcherEmptyState").visible)
    }
    function test_shared_headers_render_page_text_literally() {
        var surface = create(navigationFixture)
        surface.title = "<b>Page title</b>"
        surface.message = "<img src='https://example.invalid/tracker'>"
        compare(findChild(surface, "dialogTitleLabel").textFormat, Text.PlainText)
        compare(findChild(surface, "dialogMessageLabel").textFormat, Text.PlainText)
    }
}
