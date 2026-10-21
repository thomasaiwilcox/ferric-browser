import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
import "../../qml/scripts/ListNavigation.js" as ListNavigation

TestCase {
    id: test
    name: "ListPaging"
    when: windowShown
    property int actionIndex: -1
    property string lastAction: ""
    property int activationCount: 0

    ApplicationWindow {
        id: host
        width: 900
        height: 700
        visible: true
        font.family: "monospace"
        font.pointSize: 12
        property real chromeScale: 1
        property color panelColor: "#181825"
        property color primaryTextColor: "#cdd6f4"
        property color secondaryTextColor: "#bac2de"
        property color mutedTextColor: "#a6adc8"
        property color errorColor: "#f38ba8"
        property color surfaceColor: "#313244"
        property color borderColor: "#585b70"
        property color accentColor: "#89b4fa"
        property color selectionColor: "#45475a"
        property bool temporaryProfile: false
        color: panelColor
        palette.buttonText: primaryTextColor
        palette.text: primaryTextColor
        palette.windowText: primaryTextColor
        palette.placeholderText: mutedTextColor
        palette.button: surfaceColor
        palette.base: panelColor
    }
    ListModel { id: pagingRecords }
    Component {
        id: switcherFixture
        FerricSwitcher {
            browserWindow: host
            switcherVisible: true
            scope: "tabs"
            results: []
            onActionRequested: function(index, action) {
                test.actionIndex = index
                test.lastAction = action
            }
            onActivationRequested: test.activationCount++
        }
    }
    Component {
        id: libraryFixture
        FerricLibraryEntries {
            anchors.fill: parent
            anchors.margins: 16
            browserWindow: host
            entries: pagingRecords
            libraryKind: "bookmarks"
            pendingDelete: ""
        }
    }
    Component {
        id: tallHeader
        Rectangle { width: 500; height: 600; color: "#313244" }
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.requestActivate()
        tryCompare(host, "active", true)
        pagingRecords.clear()
        actionIndex = -1
        lastAction = ""
        activationCount = 0
    }
    function create(data, count) {
        host.width = data.width
        host.height = data.height
        host.font.pointSize = data.font
        var rows = []
        for (var i = 0; i < count; ++i) {
            var label = "Result " + i + (i % 3 === 0
                ? " — a longer page title that needs room to remain readable" : "")
            rows.push({kind: "tab", id: String(i), label: label,
                secondary: "https://example.test/page/" + i,
                actions: i % 3 === 0 ? ["open", "focus", "close"] : ["open"]})
            pagingRecords.append({entryKind: "bookmark", entryId: String(i), nodeId: "",
                label: label, secondary: "https://example.test/page/" + i, detail: ""})
        }
        var surface = createTemporaryObject(data.kind === "switcher"
            ? switcherFixture : libraryFixture, host.contentItem)
        verify(surface)
        if (data.kind === "switcher") surface.results = rows
        else surface.focusList()
        verify(waitForPolish(host))
        var list = findChild(surface, data.kind === "switcher"
            ? "switcherResultList" : "libraryEntryList")
        verify(list)
        tryCompare(list, "count", count)
        var input = data.kind === "switcher" ? findChild(surface, "switcherSearchInput") : list
        input.forceActiveFocus()
        tryCompare(input, "activeFocus", true)
        wait(0)
        return {surface: surface, list: list, input: input}
    }
    function visibleRange(list) {
        var first = list.count, last = -1
        for (var i = 0; i < list.count; ++i) {
            var row = list.itemAtIndex(i)
            if (!row) continue
            var top = row.mapToItem(list, 0, 0).y
            if (top + row.height > 1 && top < list.height - 1) {
                first = Math.min(first, i)
                last = Math.max(last, i)
            }
        }
        verify(last >= first, "A page of results is visible")
        return {first: first, last: last}
    }
    function test_pages_do_not_skip_unseen_rows_data() {
        var cases = []
        for (var kind of ["switcher", "library"]) {
            cases.push({tag: kind + "-wide", kind: kind, width: 1000, height: 800, font: 12})
            cases.push({tag: kind + "-narrow", kind: kind, width: 390, height: 600, font: 20})
            cases.push({tag: kind + "-short", kind: kind, width: 700, height: 420, font: 26})
        }
        return cases
    }
    function test_pages_do_not_skip_unseen_rows(data) {
        var fixture = create(data, 30)
        var list = fixture.list
        for (var direction of [1, -1]) {
            var complete = false
            for (var step = 0; step < 150; ++step) {
                var before = visibleRange(list)
                var oldIndex = list.currentIndex, oldY = list.contentY
                keyClick(direction > 0 ? Qt.Key_PageDown : Qt.Key_PageUp)
                verify(waitForPolish(host))
                wait(0)
                var after = visibleRange(list)
                verify(after.first <= before.last + 1 && after.last >= before.first - 1,
                    "Paging skipped unseen rows: " + JSON.stringify(before) + " → " + JSON.stringify(after))
                compare(fixture.input.activeFocus, true)
                if (step === 0 && direction > 0 && data.width === 390
                        && testArtifactDirectory.length > 0) {
                    wait(100)
                    verify(waitForRendering(host.contentItem))
                    grabImage(host.contentItem).save(testArtifactDirectory
                        + "/paging-" + data.kind + ".png")
                }
                if (oldIndex === list.currentIndex && Math.abs(oldY - list.contentY) < 1) {
                    compare(list.currentIndex, direction > 0 ? list.count - 1 : 0)
                    complete = true
                    break
                }
            }
            verify(complete, "Paging reaches the end without stalling")
        }
    }
    function test_page_down_reads_expanded_row_before_leaving_it() {
        var fixture = create({kind: "library", width: 700, height: 400, font: 16}, 4)
        var list = fixture.list
        pagingRecords.setProperty(0, "detail", "Line of saved page details\n".repeat(50))
        list.currentItem.detailsExpanded = true
        verify(waitForPolish(host))
        verify(list.currentItem.height > list.height * 2)
        var start = list.contentY
        keyClick(Qt.Key_PageDown)
        verify(waitForPolish(host))
        compare(list.currentIndex, 0, "Read the current row before selecting another")
        verify(list.contentY > start, "Page Down scrolls through the expanded details")
        var after = list.contentY
        keyClick(Qt.Key_PageUp)
        verify(waitForPolish(host))
        compare(list.currentIndex, 0)
        verify(list.contentY < after, "Page Up returns through the same details")
    }
    function test_estimated_bounds_do_not_hide_a_measured_row_data() {
        return [{tag: "underestimated-end", origin: 0, position: 450},
                {tag: "overestimated-origin", origin: 500, position: 400}]
    }
    function test_estimated_bounds_do_not_hide_a_measured_row(data) {
        // ListView can revise these estimates while neighboring delegates
        // finish layout. The selected row's actual coordinates are known.
        var view = {originY: data.origin, contentHeight: 200, height: 100,
            contentY: 400, contentItem: {}, headerItem: null,
            currentItem: {height: 200, mapToItem: function() { return {y: 400} }}}
        ListNavigation.scrollTo(view, data.position)
        compare(view.contentY, data.position, "A valid position inside the selected row remains readable")
    }
    function test_page_up_from_tall_row_does_not_skip_its_neighbor_data() {
        return [{tag:"wide",width:700,height:500,font:16},
                {tag:"wide-short",width:700,height:350,font:16},
                {tag:"wide-large",width:700,height:350,font:26},
                {tag:"narrow",width:390,height:600,font:20},
                {tag:"narrow-short",width:390,height:480,font:20},
                {tag:"narrow-small",width:390,height:320,font:20}]
    }
    function test_page_up_from_tall_row_does_not_skip_its_neighbor(data) {
        data.kind = "library"
        var fixture = create(data, 40)
        var list = fixture.list
        list.cacheBuffer = 0
        list.currentIndex = 27
        list.positionViewAtIndex(27, ListView.Beginning)
        verify(waitForPolish(host))
        pagingRecords.setProperty(27, "detail", "Expanded saved page details\n".repeat(40))
        list.currentItem.detailsExpanded = true
        verify(waitForPolish(host))
        list.positionViewAtIndex(27, ListView.Beginning)
        verify(waitForPolish(host))
        verify(list.currentItem.height > list.height)
        verify(waitForRendering(host.contentItem))
        compare(visibleRange(list).first, 27)
        compare(visibleRange(list).last, 27)
        keyClick(Qt.Key_PageUp)
        verify(waitForPolish(host))
        var after = visibleRange(list)
        verify(list.currentIndex < 27)
        verify(after.last >= 26, "Page Up must show the result immediately before the tall row; got "
            + JSON.stringify(after))
    }
    function test_page_distance_adapts_to_window_height_data() {
        return [{tag: "switcher", kind: "switcher"}, {tag: "library", kind: "library"}]
    }
    function test_page_distance_adapts_to_window_height(data) {
        var fixture = create({kind: data.kind, width: 1000, height: 800, font: 12}, 30)
        var list = fixture.list
        var before = visibleRange(list)
        verify(before.last >= 3, "Several results fit in the large viewport")
        keyClick(Qt.Key_PageDown)
        verify(waitForPolish(host))
        var largeTarget = list.currentIndex
        verify(largeTarget >= before.last - 1, "Page Down uses the available viewport")
        if (data.kind === "switcher") keyClick(Qt.Key_Home, Qt.ControlModifier)
        else keyClick(Qt.Key_Home)
        tryCompare(list, "currentIndex", 0)
        // Leave enough room between sizes to change the page by at least one
        // row even with the smaller default fonts used by CI.
        host.height = 350
        verify(waitForPolish(host))
        wait(0)
        before = visibleRange(list)
        keyClick(Qt.Key_PageDown)
        verify(waitForPolish(host))
        verify(list.currentIndex < largeTarget, "Shorter windows produce smaller pages")
        verify(list.currentIndex >= before.last - 1, "The smaller page still uses its viewport")
    }
    function test_empty_lists_leave_focus_in_place_data() {
        return [{tag: "switcher", kind: "switcher"}, {tag: "library", kind: "library"}]
    }
    function test_empty_lists_leave_focus_in_place(data) {
        var fixture = create({kind: data.kind, width: 700, height: 600, font: 12}, 0)
        keyClick(Qt.Key_PageDown)
        keyClick(Qt.Key_PageUp)
        compare(fixture.list.currentIndex, -1)
        compare(fixture.input.activeFocus, true)
    }
    function test_page_up_reaches_controls_above_first_row() {
        var fixture = create({kind: "library", width: 700, height: 400, font: 16}, 12)
        var list = fixture.list
        fixture.surface.controlsHeader = tallHeader
        verify(waitForPolish(host))
        keyClick(Qt.Key_End)
        verify(waitForPolish(host))
        var complete = false
        for (var step = 0; step < 100; ++step) {
            var oldY = list.contentY, oldIndex = list.currentIndex
            keyClick(Qt.Key_PageUp)
            verify(waitForPolish(host))
            wait(0)
            if (oldIndex === list.currentIndex && Math.abs(oldY - list.contentY) < 1) {
                compare(list.currentIndex, 0)
                verify(list.headerItem.mapToItem(list, 0, 0).y >= -1,
                    "Page Up reaches the beginning of the controls above the results")
                complete = true
                break
            }
        }
        verify(complete, "Paging through a tall header does not loop")
        var beginning = list.contentY
        keyClick(Qt.Key_PageDown)
        verify(waitForPolish(host))
        compare(list.currentIndex, 0, "Page Down reads the header before browsing results")
        verify(list.contentY > beginning && list.contentY - beginning <= list.height + 1)
    }
    function test_tab_after_paging_reaches_selected_row_actions_data() {
        return [{tag: "result-page", oversized: false}, {tag: "expanded-details", oversized: true}]
    }
    function test_tab_from_switcher_search_reaches_selected_result_data() {
        return [{tag: "arrows", key: Qt.Key_Down, times: 7, modifiers: Qt.NoModifier},
                {tag: "pages", key: Qt.Key_PageDown, times: 3, modifiers: Qt.NoModifier},
                {tag: "last", key: Qt.Key_End, times: 1, modifiers: Qt.ControlModifier},
                {tag: "details", key: Qt.Key_Down, times: 7, modifiers: Qt.NoModifier,
                    controls: ["switcherSourceDetailsButton"]},
                {tag: "refresh", key: Qt.Key_Down, times: 7, modifiers: Qt.NoModifier,
                    controls: ["switcherSourceRefresh"]},
                {tag: "details-and-refresh", key: Qt.Key_Down, times: 7, modifiers: Qt.NoModifier,
                    controls: ["switcherSourceDetailsButton", "switcherSourceRefresh"]}]
    }
    function test_tab_from_switcher_search_reaches_selected_result(data) {
        var fixture = create({kind: "switcher", width: 390, height: 600, font: 20}, 30)
        var controls = data.controls || []
        if (controls.length) {
            fixture.surface.sourceNotice = "Some saved items could not be loaded."
            fixture.surface.sourceDetail = controls.indexOf("switcherSourceDetailsButton") >= 0
                ? "Saved library details" : ""
            fixture.surface.sourcesRetryable = controls.indexOf("switcherSourceRefresh") >= 0
            verify(waitForPolish(host))
        }
        for (var i = 0; i < data.times; ++i) {
            keyClick(data.key, data.modifiers)
            verify(waitForPolish(host))
        }
        var selected = fixture.list.currentIndex
        verify(selected > 0)
        var row = fixture.list.currentItem
        for (var name of controls) {
            keyClick(Qt.Key_Tab)
            compare(host.activeFocusItem.objectName, name, "Search controls remain in the tab order")
        }
        keyClick(Qt.Key_Tab)
        verify(waitForPolish(host))
        compare(fixture.list.currentIndex, selected, "Tab retains the selected result")
        var focused = host.activeFocusItem
        compare(focused.objectName, "switcherActionButton")
        while (focused && focused !== row) focused = focused.parent
        verify(focused === row, "Tab enters an action for the selected result")
        if (testArtifactDirectory.length > 0 && data.tag === "arrows") {
            verify(waitForRendering(host.contentItem))
            grabImage(host.contentItem).save(testArtifactDirectory + "/switcher-selected-action.png")
        }
        if (fixture.surface.results[selected].actions.length > 1) {
            keyClick(Qt.Key_Tab)
            compare(host.activeFocusItem.resultAction, "focus")
            compare(fixture.list.currentIndex, selected)
            keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
            compare(host.activeFocusItem.resultAction, "open")
        }
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        compare(host.activeFocusItem.objectName, controls.length
            ? controls[controls.length - 1] : "switcherSearchInput")
        compare(fixture.list.currentIndex, selected)
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_Return)
        compare(actionIndex, selected, "Enter acts on the selected result")
        compare(lastAction, "open")
        compare(activationCount, 0, "Action activation does not submit the search field")
    }
    function test_tab_after_paging_reaches_selected_row_actions(data) {
        var fixture = create({kind: "library", width: 700, height: 600, font: 16}, 30)
        if (data.oversized) {
            pagingRecords.setProperty(0, "detail", "Expanded page details\n".repeat(50))
            fixture.list.currentItem.detailsExpanded = true
            verify(waitForPolish(host))
        }
        keyClick(Qt.Key_PageDown)
        verify(waitForPolish(host))
        if (data.oversized) compare(fixture.list.currentIndex, 0)
        else verify(fixture.list.currentIndex > 0)
        keyClick(Qt.Key_Tab)
        var item = host.activeFocusItem
        while (item && item !== fixture.list.currentItem) item = item.parent
        verify(item === fixture.list.currentItem, "Tab reaches an action for the selected result")
    }
    function test_large_switcher_keeps_a_compact_keyboard_footer() {
        var fixture = create({kind: "switcher", width: 390, height: 600, font: 26}, 30)
        function visibleKeys(item) {
            if (!item.visible) return null
            if (item.objectName === "dialogKeyboardHelp") return item
            for (var child of item.children) {
                var found = visibleKeys(child)
                if (found) return found
            }
            return null
        }
        var keys = visibleKeys(fixture.surface)
        verify(keys)
        verify(keys.parent.height <= keys.height + 1,
            "The compact keyboard hint leaves room for search results")
    }
    function test_page_uses_the_completed_search_layout_data() {
        return [{tag: "20pt", font: 20, height: 600},
                {tag: "26pt", font: 26, height: 700}]
    }
    function test_page_uses_the_completed_search_layout(data) {
        var fixture = create({kind: "switcher", width: 390, height: data.height, font: data.font}, 30)
        fixture.surface.results = fixture.surface.results.map(function(row) {
            return Object.assign({}, row, {actions: ["open"]})
        })
        fixture.surface.sourcesPending = true
        fixture.surface.sourceDetail = "Waiting for source details"
        fixture.surface.sourceNotice = "Searching other browser windows…"
        verify(waitForPolish(host))
        // Font metrics differ across desktops. Create the constrained loading
        // viewport from the actual row instead of assuming a particular font.
        for (var attempt = 0; attempt < 5
                && fixture.list.height >= fixture.list.currentItem.height; ++attempt) {
            fixture.surface.dialogHeight = fixture.surface.panelItem.height
                - (fixture.list.height - fixture.list.currentItem.height) - 20
            verify(waitForPolish(host))
        }
        var oldHeight = fixture.list.height
        verify(fixture.list.currentItem.height > oldHeight,
            "The loading status temporarily leaves less than a full row")
        fixture.surface.sourcesPending = false
        fixture.surface.sourceNotice = ""
        fixture.surface.sourceDetail = ""
        keyClick(Qt.Key_PageDown)
        verify(waitForPolish(host))
        verify(fixture.list.height > oldHeight)
        verify(fixture.list.currentItem.height <= fixture.list.height,
            "The completed search has room for the selected row")
        verify(fixture.list.currentIndex > 0,
            "The first Page Down uses the completed layout without losing progress")
    }
    function test_background_search_keeps_the_results_viewport_data() {
        return [{tag: "20pt", font: 20, height: 600},
                {tag: "26pt", font: 26, height: 700}]
    }
    function test_background_search_keeps_the_results_viewport(data) {
        var fixture = create({kind: "switcher", width: 390, height: data.height, font: data.font}, 30)
        fixture.surface.results = fixture.surface.results.map(function(row) {
            return Object.assign({}, row, {actions: ["open"]})
        })
        verify(waitForPolish(host))
        var height = fixture.list.height
        fixture.surface.sourcesPending = true
        fixture.surface.sourceNotice = "Searching other browser windows…"
        verify(waitForPolish(host))
        compare(fixture.list.height, height, "Background search does not shrink usable results")
        var progress = findChild(fixture.surface, "switcherSearchProgress")
        verify(progress && progress.visible)
        compare(progress.Accessible.name, fixture.surface.sourceNotice)
        compare(host.activeFocusItem, fixture.input)
        keyClick(Qt.Key_PageDown)
        verify(waitForPolish(host))
        verify(fixture.list.currentIndex > 0)
        var selected = fixture.list.currentIndex
        keyClick(Qt.Key_Tab)
        compare(host.activeFocusItem.objectName, "switcherActionButton")
        compare(fixture.list.currentIndex, selected)
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        compare(host.activeFocusItem, fixture.input)
        fixture.surface.sourcesPending = false
        fixture.surface.sourceNotice = ""
        verify(waitForPolish(host))
        compare(fixture.list.height, height)
        compare(progress.visible, false)
    }
}
