import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "SwitcherPolish"
    when: windowShown
    property string lastAction: ""
    property int lastIndex: -1
    property int activationCount: 0
    property int refreshCount: 0

    ApplicationWindow {
        id: host
        width: 900
        height: 700
        visible: true
        font.family: "monospace"
        font.pointSize: 20
        FontMetrics { id: metrics; font: host.font }
        property real chromeScale: Math.max(1, metrics.height / 14)
        property real chromeRowHeight: Math.ceil(metrics.height + Math.max(4, Math.ceil(metrics.height * 0.25)) * 1.25)
        property color panelColor: "#181825"
        property color primaryTextColor: "#cdd6f4"
        property color secondaryTextColor: "#bac2de"
        property color mutedTextColor: "#a6adc8"
        property color errorColor: "#f38ba8"
        property color surfaceColor: "#1e1e2e"
        property color borderColor: "#585b70"
        property color accentColor: "#89b4fa"
        property color selectionColor: "#45475a"
        function switcherMaxResults() { return 200 }
        color: panelColor
        palette.window: panelColor
        palette.windowText: primaryTextColor
        palette.text: primaryTextColor
        palette.base: panelColor
        palette.button: panelColor
        palette.buttonText: primaryTextColor
        palette.placeholderText: mutedTextColor
        palette.highlight: accentColor
    }
    Component {
        id: fixture
        FerricSwitcher {
            browserWindow: host
            switcherVisible: true
            scope: "contexts"
            results: [{kind: "context",
                label: "Before work <b>Research</b> - notes and reference material with a long label",
                profile: "default", secondary: "work", actions: ["enter"]}]
            onRefreshRequested: test.refreshCount++
            onActionRequested: function(index, action) {
                test.lastIndex = index
                test.lastAction = action
            }
            onActivationRequested: function(index) {
                test.lastIndex = index
                test.activationCount++
            }
        }
    }

    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width = 900
        host.height = 700
        host.panelColor = "#181825"
        host.primaryTextColor = "#cdd6f4"
        host.borderColor = "#585b70"
        host.requestActivate()
        tryCompare(host, "active", true)
        lastAction = ""
        lastIndex = -1
        activationCount = 0
        refreshCount = 0
    }
    function create(properties) {
        var surface = createTemporaryObject(fixture, host.contentItem, properties || {})
        verify(surface)
        verify(waitForPolish(host))
        wait(0)
        tryVerify(function() { return findChild(surface, "switcherSearchInput").activeFocus })
        return surface
    }
    function inside(item, viewport) {
        var position = item.mapToItem(viewport, 0, 0)
        return position.x >= -1 && position.y >= -1
            && position.x + item.width <= viewport.width + 1
            && position.y + item.height <= viewport.height + 1
    }
    function buttons(row) {
        var matches = []
        function visit(item) {
            if (item.objectName === "switcherActionButton") matches.push(item)
            for (var i = 0; i < item.children.length; ++i) visit(item.children[i])
        }
        visit(row)
        return matches
    }
    function test_tooltip_stays_in_window_data() {
        return [{tag:"dark",panel:"#181825",foreground:"#cdd6f4",border:"#585b70"},
            {tag:"light",panel:"#ffffff",foreground:"#17202a",border:"#ccd3dc"}]
    }
    function test_tooltip_stays_in_window(data) {
        host.panelColor = data.panel
        host.primaryTextColor = data.foreground
        host.borderColor = data.border
        host.width = 390
        host.height = 600
        var surface = create({scope:"history",results:[{kind:"history",label:"Literal <b>page</b> title. ".repeat(100), secondary:"https://example.test/", actions:["open"]}]})
        var list = findChild(surface, "switcherResultList")
        tryVerify(function() { return list.itemAtIndex(0) !== null })
        var row = list.itemAtIndex(0)
        var tooltip = findChild(row, "switcherResultTooltip")
        mouseMove(row, 5, 5)
        tryCompare(tooltip, "opened", true)
        verify(inside(tooltip.contentItem, host.contentItem), "hover details fit inside window")
        compare(tooltip.contentItem.textFormat, Text.PlainText)
        compare(findChild(row, "switcherResultDetail").text, "https://example.test/")
        compare(tooltip.background.color, host.panelColor)
        compare(tooltip.contentItem.color, host.primaryTextColor)
        verify(tooltip.contentItem.paintedHeight <= tooltip.contentItem.height + 1, "hover text is bounded, not painted outside its popup")
        if (testArtifactDirectory.length > 0)
            grabImage(host.contentItem).save(testArtifactDirectory + "/switcher-hover-narrow-" + data.tag + ".png")
    }
    function test_scoped_layout_data() {
        return [{tag: "wide", width: 900, height: 700},
            {tag: "short", width: 640, height: 480},
            {tag: "narrow", width: 390, height: 600}]
    }
    function test_scoped_layout(data) {
        host.width = data.width
        host.height = data.height
        var surface = create()
        var input = findChild(surface, "switcherSearchInput")
        var count = findChild(surface, "switcherResultCount")
        var list = findChild(surface, "switcherResultList")
        compare(surface.title, "Contexts")
        compare(input.placeholderText, "Search contexts")
        compare(count.text, "1 result")
        compare(count.Accessible.name, "1 result")
        compare(count.Accessible.description, "Current result count")
        verify(inside(input, surface.bodyItem), "search fits inside dialog body")
        verify(inside(count, surface.bodyItem), "result count fits inside dialog body")
        verify(input.x + input.width <= count.x, "search and count do not overlap")
        tryCompare(list, "count", 1)
        var row = list.itemAtIndex(0)
        var title = findChild(row, "switcherResultTitle")
        compare(title.textFormat, Text.PlainText)
        verify(title.text.indexOf("<b>Research</b>") >= 0)
        var tooltip = findChild(row, "switcherResultTooltip")
        compare(tooltip.contentItem.textFormat, Text.PlainText)
        verify(tooltip.text.indexOf("<b>Research</b>") >= 0)
        var action = buttons(row)[0]
        tryVerify(function() { return inside(action, list) }, 2000,
                  "primary context action is fully visible")
        compare(action.text, "Enter")
        mouseClick(action)
        compare(lastAction, "enter")
        compare(lastIndex, 0)
        compare(activationCount, 0)
        surface.results = []
        tryCompare(list, "count", 0)
        var empty = findChild(surface, "switcherEmptyState")
        verify(empty.visible)
        verify(inside(empty, list), "empty state stays inside results viewport")
    }
    function test_optional_details_data() {
        return [{tag:"both",profile:"Work",secondary:"Research",expected:"Work · Research"},
            {tag:"profile",profile:"Work",secondary:"",expected:"Work"},
            {tag:"secondary",profile:"",secondary:"Research",expected:"Research"},
            {tag:"neither",profile:"",secondary:"",expected:""}]
    }
    function test_optional_details(data) {
        var surface = create({results:[{kind:"context",label:"Research",profile:data.profile,
            secondary:data.secondary,actions:["enter"]}]})
        var list = findChild(surface,"switcherResultList")
        tryVerify(function() { return list.itemAtIndex(0) !== null })
        var detail = findChild(list.itemAtIndex(0),"switcherResultDetail")
        compare(detail.text,data.expected)
        compare(detail.visible,data.expected.length > 0)
        verify(list.itemAtIndex(0).Accessible.description.indexOf(" · ") !== 0)
    }
    function test_unknown_scope_is_safe() {
        var surface = create({scope: "__proto__"})
        compare(surface.title, "Switcher")
        compare(findChild(surface, "switcherSearchInput").placeholderText, "Search browser items")
    }
    function test_pending_query_cannot_open_previous_results() {
        var surface = create({scope: "tabs", results: [
            {kind: "tab", label: "Alpha", actions: ["open"]}]})
        surface.resultsCurrent = false
        var list = findChild(surface, "switcherResultList")
        verify(!list.visible)
        verify(!list.enabled)
        compare(findChild(surface, "switcherResultCount").text, "Searching…")
        compare(findChild(surface, "switcherEmptyState").text, "Searching…")
        keyClick(Qt.Key_Return)
        compare(activationCount, 0)
        surface.results = [{kind: "tab", label: "Beta", actions: ["open"]}]
        surface.resultsCurrent = true
        tryCompare(list, "visible", true)
        tryCompare(list, "currentIndex", 0)
        keyClick(Qt.Key_Return)
        compare(activationCount, 1)
        compare(lastIndex, 0)
    }
    function test_keyboard_moves_and_opens_selected_result() {
        host.width = 390
        host.height = 600
        var results = []
        for (var i = 0; i < 24; ++i)
            results.push({kind: "tab", label: "Research tab " + i,
                secondary: "https://example.test/research/" + i, actions: ["open"]})
        var surface = create({scope: "tabs", results: results})
        var list = findChild(surface, "switcherResultList")
        keyClick(Qt.Key_Down)
        tryCompare(list, "currentIndex", 1)
        keyClick(Qt.Key_PageDown)
        tryCompare(list, "currentIndex", 9)
        keyClick(Qt.Key_End)
        tryCompare(list, "currentIndex", 23)
        tryVerify(function() {
            var row = list.itemAtIndex(23)
            return row && inside(row, list)
        })
        keyClick(Qt.Key_Return)
        compare(lastIndex, 23)
        compare(activationCount, 1)
        keyClick(Qt.Key_Home)
        tryCompare(list, "currentIndex", 0)
        tryVerify(function() {
            var row = list.itemAtIndex(0)
            return row && inside(row, list)
        })
    }
    function test_keyboard_focus_reveals_wrapped_actions() {
        host.width = 390
        host.height = 480
        var surface = create({scope: "sessions", results: [{kind: "session",
            label: "A long session title that wraps onto two lines", profile: "Personal",
            secondary: "12 tabs", actions: ["load-preview", "load", "delete"]}]})
        var list = findChild(surface, "switcherResultList")
        tryVerify(function() { return list.itemAtIndex(0) !== null })
        var actions = buttons(list.itemAtIndex(0))
        compare(actions.length, 3)
        var last = actions[2]
        for (var i = 0; i < 5 && !last.activeFocus; ++i) keyClick(Qt.Key_Tab)
        tryVerify(function() { return last.activeFocus && inside(last, list) })
        verify(list.contentY > list.originY, "wrapped action row scrolls to focused control")
        keyClick(Qt.Key_Return)
        compare(lastAction, "delete")
        compare(lastIndex, 0)
        compare(activationCount, 0)
    }
    function test_focus_and_selection_stay_together() {
        var surface = create({results: [
            {kind: "context", label: "Work", actions: ["enter"]},
            {kind: "context", label: "Personal", actions: ["enter"]}]})
        var list = findChild(surface, "switcherResultList")
        tryVerify(function() { return list.itemAtIndex(1) !== null })
        var secondAction = buttons(list.itemAtIndex(1))[0]
        secondAction.forceActiveFocus(Qt.TabFocusReason)
        tryCompare(secondAction, "activeFocus", true)
        tryCompare(list, "currentIndex", 1)
        keyClick(Qt.Key_Return)
        compare(lastAction, "enter")
        compare(lastIndex, 1)
        compare(activationCount, 0)
    }
    function test_saved_source_states_data() {
        return [
            {tag: "waiting", pending: true, failed: false, retryable: false,
                notice: "Loading bookmarks and quickmarks…", detail: "", count: 0},
            {tag: "partial", pending: true, failed: false, retryable: false,
                notice: "Loading saved items…", detail: "", count: 1},
            {tag: "failed", pending: false, failed: true, retryable: true,
                notice: "Some saved items couldn't be loaded. Refresh to try again.",
                detail: "Saved library: literal <b>diagnostic</b>\n".repeat(100), count: 0},
            {tag: "failed-with-rows", pending: false, failed: true, retryable: true,
                notice: "Some saved items couldn't be loaded. Refresh to try again.",
                detail: "Saved library: literal <b>diagnostic</b>", count: 1},
            {tag: "private", pending: false, failed: false, retryable: false,
                notice: "Bookmarks and quickmarks aren't available in this private session.",
                detail: "", count: 0}
        ]
    }
    function test_saved_source_states(data) {
        host.width = 390
        host.height = 480
        var surface = create({scope: "marks", sourcesPending: data.pending,
            sourcesFailed: data.failed, sourcesRetryable: data.retryable,
            sourceNotice: data.notice, sourceDetail: data.detail,
            results: data.count ? [{kind: "bookmark", id: "held", label: "A saved bookmark",
                actions: ["open"]}] : []})
        var status = findChild(surface, "switcherSourceStatus")
        tryCompare(status, "visible", true)
        verify(inside(status, surface.bodyItem), "source state stays in the body")
        var notice = findChild(status, "managerFeedbackMessage")
        compare(notice.text, data.notice)
        compare(notice.textFormat, Text.PlainText)
        var refresh = findChild(status, "switcherSourceRefresh")
        compare(refresh.visible, data.retryable)
        if (data.pending && !data.count)
            compare(findChild(surface, "switcherResultCount").text, "Loading…")
        var empty = findChild(surface, "switcherEmptyState")
        verify(!empty.visible, "source notice replaces a misleading empty search")
        if (testArtifactDirectory.length > 0)
            grabImage(host.contentItem).save(testArtifactDirectory + "/switcher-source-initial-" + data.tag + ".png")
        if (data.detail.length > 0) {
            var disclosure = findChild(status, "switcherSourceDetailsButton")
            disclosure.forceActiveFocus(Qt.TabFocusReason)
            tryVerify(function() { return disclosure.activeFocus && inside(disclosure, status) })
            keyClick(Qt.Key_Space)
            tryCompare(surface, "sourceDetailsVisible", true)
            var details = findChild(host.contentItem, "switcherSourceDetails")
            verify(details)
            tryCompare(details, "activeFocus", true)
            compare(details.text, data.detail)
            compare(details.textFormat, TextEdit.PlainText)
            var dialog = findChild(host.contentItem, "switcherDetailsDialog")
            verify(inside(dialog.panelItem, host.contentItem), "details use the full bounded dialog")
            keyClick(Qt.Key_End, Qt.ControlModifier)
            if (testArtifactDirectory.length > 0)
                grabImage(host.contentItem).save(testArtifactDirectory + "/switcher-details-" + data.tag + ".png")
            keyClick(Qt.Key_Escape)
            tryCompare(surface, "sourceDetailsVisible", false)
            tryCompare(disclosure, "activeFocus", true)
            refresh.forceActiveFocus(Qt.TabFocusReason)
            tryVerify(function() { return refresh.activeFocus && inside(refresh, status) })
            keyClick(Qt.Key_Return)
            compare(refreshCount, 1)
        }
        if (data.count) {
            surface.focusInput()
            keyClick(Qt.Key_Return)
            compare(activationCount, 1, "current rows remain usable during saved-source work")
        }
        if (testArtifactDirectory.length > 0)
            grabImage(host.contentItem).save(testArtifactDirectory + "/switcher-source-" + data.tag + ".png")
    }

}
