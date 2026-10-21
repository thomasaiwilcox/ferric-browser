import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "DownloadKeyboard"
    when: windowShown
    ApplicationWindow {
        id: host
        width: 900; height: 700; visible: true
        font.family: "monospace"; font.pointSize: 12
        FontMetrics { id: metrics; font: host.font }
        property real chromeScale: Math.max(1, metrics.height / 14)
        property color backgroundColor: "#181825"
        property color panelColor: "#1e1e2e"
        property color surfaceColor: "#313244"
        property color primaryTextColor: "#cdd6f4"
        property color secondaryTextColor: "#bac2de"
        property color mutedTextColor: "#a6adc8"
        property color accentColor: "#89b4fa"
        property color warningColor: "#f9e2af"
        property color borderColor: "#585b70"
        property color selectionColor: "#45475a"
        property bool downloadManagerVisible: true
        color: backgroundColor
        palette.window: backgroundColor; palette.base: panelColor
        palette.text: primaryTextColor; palette.windowText: primaryTextColor
        palette.button: surfaceColor; palette.buttonText: primaryTextColor
        palette.placeholderText: mutedTextColor; palette.highlight: accentColor
    }
    ListModel { id: records }
    Component { id: fixture; FerricDownloadManager { browserWindow: host; downloadsModel: records } }
    Component { id: spyFixture; SignalSpy {} }
    function record(index, state) {
        return {id: "download-" + index, state: state || "completed", bytes: "4096", total: 8192,
            speed: 0, destination: "/owned/Research archives/2026/report-" + index + "-δ.zip",
            displayName: "Research <final> report " + index + " — δ.zip", reason: "",
            active: state === "in-progress", fileReady: !state || state === "completed",
            retryAvailable: false, retryPending: false, finishing: false}
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width = 900; host.height = 700; host.font.pointSize = 12
        host.downloadManagerVisible = true
        host.requestActivate(); tryCompare(host, "active", true)
        records.clear()
    }
    function create(count, state) {
        for (var i = 0; i < count; ++i) records.append(record(i, state))
        var view = createTemporaryObject(fixture, host.contentItem)
        verify(view)
        verify(waitForPolish(host)); verify(waitForRendering(host.contentItem))
        var list = findChild(view, "downloadList")
        compare(list.count, count)
        return {view: view, list: list}
    }
    function browse(list) {
        var item = host.activeFocusItem
        while (item && item !== list) { item.focus = false; item = item.parent }
        list.forceActiveFocus()
    }
    function test_j_and_k_browse_downloads() {
        var f = create(20)
        browse(f.list)
        keyClick(Qt.Key_J)
        compare(f.list.currentIndex, 1)
        verify(f.list.currentItem.border.width > 0, "The browsing selection is visible")
        keyClick(Qt.Key_K)
        compare(f.list.currentIndex, 0)
    }
    function test_page_down_reads_a_visible_page() {
        var f = create(40)
        browse(f.list)
        var before = f.list.contentY
        keyClick(Qt.Key_PageDown)
        verify(waitForPolish(host)); verify(waitForRendering(host.contentItem))
        verify(f.list.contentY > before, "Page Down moves through the downloads")
        verify(f.list.contentY - before <= f.list.height + 1, "Paging retains visible overlap")
    }
    function test_tab_after_end_reaches_selected_download() {
        var f = create(20)
        browse(f.list)
        keyClick(Qt.Key_End)
        compare(f.list.currentIndex, 19)
        keyClick(Qt.Key_Tab)
        var item = host.activeFocusItem
        while (item && item.id !== "download-19") item = item.parent
        verify(item !== null, "Tab enters the selected download's actions")
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        verify(host.activeFocusItem === f.list || host.activeFocusItem === f.list.currentItem,
            "Shift+Tab from the first action returns to browsing")
    }
    function test_toolbar_return_materializes_the_first_visible_download() {
        host.width = 390; host.height = 700; host.font.pointSize = 20
        var f = create(30)
        for (var page = 0; page < 100 && f.list.currentIndex < 29; ++page) {
            keyClick(Qt.Key_PageDown)
            verify(waitForRendering(host.contentItem))
        }
        for (page = 0; page < 100 && f.list.currentIndex > 0; ++page) {
            keyClick(Qt.Key_PageUp)
            verify(waitForRendering(host.contentItem))
        }
        keyClick(Qt.Key_End)
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        keyClick(Qt.Key_Return)
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_Return)
        for (var step = 0; step < 4; ++step) keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        tryCompare(host, "activeFocusItem", findChild(f.view, "downloadCloseButton"))
        tryVerify(function() {
            var first = f.list.itemAtIndex(0)
            return first && first.mapToItem(f.list, 0, 0).y < f.list.height
        }, 1000, "Returning to the toolbar leaves the downloads visible")
    }
    function test_details_reveal_full_filename_and_location() {
        var f = create(1)
        var button = findChild(f.list.currentItem, "downloadDetailsButton")
        button.forceActiveFocus(); keyClick(Qt.Key_Return)
        var details = findChild(f.list.currentItem, "downloadDetails")
        verify(details.visible)
        verify(details.text.indexOf(records.get(0).displayName) >= 0)
        verify(details.text.indexOf(records.get(0).destination) >= 0)
        compare(details.textFormat, Text.PlainText)
    }
    function test_disclosure_survives_delegate_recycling() {
        var f = create(40)
        f.list.cacheBuffer = 0
        var first = f.list.currentItem
        var button = findChild(first, "downloadDetailsButton")
        button.forceActiveFocus(); keyClick(Qt.Key_Return)
        verify(first.detailsExpanded)
        browse(f.list)
        f.list.currentIndex = 39; f.list.positionViewAtEnd()
        verify(waitForPolish(host)); verify(waitForRendering(host.contentItem))
        tryVerify(function() { return f.list.itemAtIndex(0) === null })
        f.list.currentIndex = 0; f.list.positionViewAtBeginning()
        verify(waitForPolish(host)); verify(waitForRendering(host.contentItem))
        verify(f.list.currentItem.detailsExpanded, "Details belong to the download, not its recycled widget")
    }
    function test_completion_keeps_focus_on_an_available_control() {
        var f = create(1, "in-progress")
        var pause = findChild(f.list.currentItem, "downloadPauseButton")
        pause.forceActiveFocus()
        compare(host.activeFocusItem, pause)
        records.setProperty(0, "state", "completed")
        records.setProperty(0, "active", false)
        records.setProperty(0, "fileReady", true)
        verify(waitForPolish(host)); verify(waitForRendering(host.contentItem))
        verify(host.activeFocusItem && host.activeFocusItem.visible && host.activeFocusItem.enabled
            && host.activeFocusItem.activeFocusOnTab,
            "Finishing a transfer does not strand focus on hidden controls")
    }
    function test_retry_becoming_unavailable_keeps_keyboard_actions_reachable() {
        var f = create(1, "interrupted")
        records.setProperty(0, "retryAvailable", true)
        var retry = findChild(f.list.currentItem, "downloadRetryButton")
        retry.forceActiveFocus()
        f.view.preparingDownload = true
        tryCompare(host, "activeFocusItem", findChild(f.list.currentItem, "downloadDetailsButton"))
        verify(!retry.enabled)
    }
    function test_header_focus_recovers_when_refresh_starts_or_preparation_finishes() {
        var f = create(1)
        var close = findChild(f.view, "downloadCloseButton")
        findChild(f.view, "downloadRefreshButton").forceActiveFocus()
        f.view.listPending = true
        tryCompare(host, "activeFocusItem", close)
        f.view.preparingDownload = true
        findChild(f.view, "downloadCancelRequestButton").forceActiveFocus()
        f.view.preparingDownload = false
        tryCompare(host, "activeFocusItem", close)
    }
    function test_transfer_updates_do_not_interrupt_toolbar_focus() {
        var f = create(1, "in-progress")
        findChild(f.list.currentItem, "downloadPauseButton").forceActiveFocus()
        var close = findChild(f.view, "downloadCloseButton")
        close.forceActiveFocus()
        records.setProperty(0, "state", "completed")
        records.setProperty(0, "active", false)
        records.setProperty(0, "fileReady", true)
        verify(waitForRendering(host.contentItem))
        compare(host.activeFocusItem, close)
    }
    function test_enter_reads_details_without_opening_the_file() {
        var f = create(1)
        var opened = createTemporaryObject(spyFixture, test, {target: f.view, signalName: "openRequested"})
        keyClick(Qt.Key_Return)
        verify(f.list.currentItem.detailsExpanded)
        compare(opened.count, 0)
        keyClick(Qt.Key_Return)
        verify(!f.list.currentItem.detailsExpanded)
        compare(opened.count, 0)
    }
    function test_disclosure_tracks_identity_and_clears_when_closed() {
        var f = create(2)
        keyClick(Qt.Key_Return)
        verify(f.list.currentItem.detailsExpanded)
        records.move(0, 1, 1)
        verify(waitForPolish(host))
        verify(f.list.itemAtIndex(1).detailsExpanded)
        verify(!f.list.itemAtIndex(0).detailsExpanded)
        host.downloadManagerVisible = false
        host.downloadManagerVisible = true
        tryVerify(function() { return host.activeFocusItem === f.list || host.activeFocusItem === f.list.currentItem })
        verify(!f.list.itemAtIndex(1).detailsExpanded)
    }
    function test_empty_toolbar_round_trip() {
        var f = create(0)
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        compare(host.activeFocusItem, findChild(f.view, "downloadCloseButton"))
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        compare(host.activeFocusItem, findChild(f.view, "downloadRefreshButton"))
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        verify(host.activeFocusItem === f.list || host.activeFocusItem === f.list.currentItem)
        keyClick(Qt.Key_PageDown)
        keyClick(Qt.Key_End)
        keyClick(Qt.Key_Home)
        compare(f.list.currentIndex, -1)
    }
    function test_paging_reads_long_failure_details_before_the_next_download() {
        host.width = 390; host.height = 700; host.font.pointSize = 20
        var f = create(3, "interrupted")
        records.setProperty(0, "reason", Array(80).join("A long failure explanation with a useful recovery step. "))
        keyClick(Qt.Key_Return)
        verify(waitForPolish(host))
        var row = f.list.currentItem
        verify(row.height > f.list.height * 2)
        var readToEnd = false
        for (var page = 0; page < 100; ++page) {
            if (row.mapToItem(f.list, 0, row.height).y <= f.list.height + 1) {
                readToEnd = true
                break
            }
            var before = f.list.contentY
            keyClick(Qt.Key_PageDown)
            verify(waitForPolish(host))
            compare(f.list.currentIndex, 0, "Read the expanded download before moving on")
            verify(f.list.contentY > before)
            verify(f.list.contentY - before <= f.list.height + 1)
        }
        verify(readToEnd)
        keyClick(Qt.Key_PageDown)
        verify(f.list.currentIndex > 0)
        keyClick(Qt.Key_G, Qt.ShiftModifier)
        compare(f.list.currentIndex, 2)
        keyClick(Qt.Key_G)
        compare(f.list.currentIndex, 0)
    }
    function test_controls_fit_large_text_data() {
        return [{tag: "390-20", width: 390, font: 20}, {tag: "320-26", width: 320, font: 26}]
    }
    function test_controls_fit_large_text(data) {
        host.width = data.width; host.height = 600; host.font.pointSize = data.font
        var f = create(1)
        f.view.preparingDownload = true
        verify(waitForPolish(host)); verify(waitForRendering(host.contentItem))
        verify(f.list.height >= metrics.height * 2, "Large text leaves usable reading space")
        function check(item) {
            if (!item.visible) return
            if (typeof item.clicked === "function" && typeof item.text === "string") {
                var x = item.mapToItem(f.view.panelItem, 0, 0).x
                verify(x >= -1 && x + item.width <= f.view.panelItem.width + 1,
                    item.text + " fits the panel")
            }
            for (var child of item.children) check(child)
        }
        check(f.view)
        if (testArtifactDirectory.length > 0)
            grabImage(host.contentItem).save(testArtifactDirectory + "/download-controls-" + data.tag + ".png")
    }
}
