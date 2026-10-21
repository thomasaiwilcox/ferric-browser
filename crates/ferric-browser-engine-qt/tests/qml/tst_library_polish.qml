import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
TestCase {
    id: test
    name: "LibraryPolish"
    when: windowShown
    ApplicationWindow {
        id: host
        width: 900
        height: 700
        visible: true
        font.family: "monospace"
        font.pointSize: 10
        FontMetrics { id: actualFontMetrics; font: host.font }
        property real chromeScale: Math.max(1, actualFontMetrics.height / 14)
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
        property bool temporaryProfile: false
        property bool ephemeralProfile: false
        property color successColor: "#a6e3a1"
        property color privateColor: "#cba6f7"
        property bool libraryManagerVisible: true
        property bool privateHistoryTransferVisible: false
        property string privateHistoryTransferTitle: ""
        property string privateHistoryTransferUrl: ""
        property bool journeyExportPreviewVisible: false
        property string journeyExportPreviewText: ""
        readonly property var modalInputScope: FerricModalInputController { hostWindow: host }
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

    ListModel { id: records }
    ListModel { id: fixtureGraphNodes }
    ListModel { id: graphEdges }
    Component {
        id: fixture
        FerricLibraryManager {
            browserWindow: host
            managerVisible: host.libraryManagerVisible
            libraryKind: "bookmarks"
            graphMode: false
            totalEntries: records.count
            page: 0; pageCount: 1; pageSize: 100
            journeySearchText: ""; journeyCurrentOnly: false; pendingDelete: ""
            entries: records; graphNodes: fixtureGraphNodes; graphEntries: graphEdges; graphLineData: []
            profilesModel: []; profilesLoading: false
        }
    }
    Component {
        id: coveringDialogFixture
        FerricCommandDialog {
            browserWindow: host
            visible: true
            title: "Another operation"
            cancelAction: "back"
            actions: [{id: "back", key: "b", label: "Back", safe: true}]
            onActionRequested: visible = false
        }
    }
    Component { id: spyFixture; SignalSpy {} }
    Component { id: alternateRecordsFixture; ListModel {} }
    function visibleChild(item, name) {
        if (!item.visible) return null
        if (item.objectName === name) return item
        for (var child of item.children) {
            var found = visibleChild(child, name)
            if (found) return found
        }
        return null
    }
    function receipt(view, id, state) {
        view.feedbackId = id
        view.feedbackMessage = state === "failed" ? "This change couldn't be saved." : state === "pending" ? "Saving library change…" : "Library change saved"
        view.feedbackDetail = state === "failed" ? "<b>disk error</b>" : ""
        view.feedbackState = state
    }
    function createView() {
        var view = createTemporaryObject(fixture, host.contentItem)
        verify(view); verify(waitForPolish(host))
        tryVerify(function() { return findChild(view, "libraryEntryEditButton") !== null })
        return view
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width=900; host.height=700; host.font.pointSize=10; host.libraryManagerVisible=true
        host.temporaryProfile = false
        host.requestActivate(); tryCompare(host,"active",true)
        records.clear(); records.append({entryKind:"bookmark",entryId:"internal-record-123",nodeId:"",label:"<b>A bookmarked page</b>",secondary:"https://example.test/a/long/path",detail:""})
        fixtureGraphNodes.clear(); fixtureGraphNodes.append({nodeId:"internal-node-id",label:"<b>A page</b>",transition:"branch-after-back",source:"internal-source",x:0,y:0})
        graphEdges.clear(); graphEdges.append({label:"<b>A page</b> → Another page",secondary:"New path after going back"})
    }
    function edit(view, text) {
        var button=findChild(view,"libraryEntryEditButton")
        button.forceActiveFocus(); tryCompare(button,"activeFocus",true)
        keyClick(Qt.Key_Return)
        var input=findChild(view,"libraryEntryEdit")
        tryCompare(input,"visible",true); input.forceActiveFocus(); input.selectAll(); keyClick(Qt.Key_A, Qt.ControlModifier); for(var i=0;i<text.length;i++) keyClick(text.charAt(i))
        if (text.length === 0) keyClick(Qt.Key_Backspace)
        return input
    }
    function test_enter_activates_the_focused_edit_and_save_controls() {
        var view=createView()
        var opened=createTemporaryObject(spyFixture,view,{target:view,signalName:"entryOpenRequested"})
        var saved=createTemporaryObject(spyFixture,view,{target:view,signalName:"entryEditRequested"})
        var input=edit(view,"Edited title")
        compare(opened.count,0)
        keyClick(Qt.Key_Return)
        compare(saved.count,1); compare(saved.signalArguments[0][2],"Edited title"); compare(opened.count,0)
        verify(input.visible)
    }
    function test_only_its_own_success_closes_the_editor() {
        var view=createView(); var input=edit(view,"A draft")
        receipt(view,"one","pending"); view.mutationAccepted("one","bookmark","internal-record-123","A draft",false)
        verify(input.visible); verify(input.readOnly)
        receipt(view,"one","failed"); verify(input.visible); compare(input.text,"A draft"); verify(!input.readOnly)
        var feedback=findChild(view,"libraryOperationFeedback")
        verify(feedback.failed); verify(!findChild(feedback,"managerFeedbackDetails").visible)
        receipt(view,"two","pending"); view.mutationAccepted("two","bookmark","internal-record-123","A draft",false)
        receipt(view,"other","succeeded"); verify(input.visible)
        receipt(view,"two","succeeded"); tryCompare(input,"visible",false)
    }
    function test_background_row_update_preserves_edit_and_selection() {
        var view=createView(); var input=edit(view,"Keep this draft")
        var list=findChild(view,"libraryEntryList"); var row=list.currentItem
        records.setProperty(0,"label","A refreshed title")
        verify(waitForPolish(host)); compare(list.currentItem,row)
        compare(input.text,"Keep this draft"); verify(input.visible)
    }
    function test_backtab_from_first_action_returns_to_the_selected_entry_data() {
        var rows = []
        for (var kind of ["bookmark", "quickmark", "history"])
            for (var compact of [false, true])
                rows.push({tag: kind + (compact ? "-compact" : "-wide"), kind: kind,
                    width: compact ? 390 : 900, font: compact ? 20 : 12})
        return rows
    }
    function test_backtab_from_first_action_returns_to_the_selected_entry(data) {
        host.width = data.width
        host.font.pointSize = data.font
        records.clear()
        for (var i = 0; i < 30; ++i)
            records.append({entryKind: data.kind, entryId: "record-" + i, nodeId: "",
                label: "Saved page " + i, secondary: "https://example.test/" + i, detail: ""})
        var view = createView()
        view.libraryKind = data.kind === "bookmark" ? "bookmarks"
            : data.kind === "quickmark" ? "quickmarks" : "history"
        var list = findChild(view, "libraryEntryList")
        view.focusPrimaryControl()
        for (i = 0; i < 7; ++i) keyClick(Qt.Key_Down)
        compare(list.currentIndex, 7)
        verify(waitForPolish(host))
        var row = list.currentItem
        keyClick(Qt.Key_Tab)
        var action = host.activeFocusItem
        compare(action.text, "Open")
        var ancestor = action
        while (ancestor && ancestor !== row) ancestor = ancestor.parent
        verify(ancestor === row, "Tab enters the selected entry")
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        compare(list.currentIndex, 7)
        verify(host.activeFocusItem === list || host.activeFocusItem === row,
            "Shift+Tab returns to browsing the selected entry")
        keyClick(Qt.Key_Down)
        compare(list.currentIndex, 8, "Browsing resumes with a single arrow press")
    }
    function test_backtab_preserves_an_unfinished_edit() {
        var view = createView()
        var input = edit(view, "An unfinished title")
        var list = findChild(view, "libraryEntryList")
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        verify(host.activeFocusItem === list || host.activeFocusItem === list.currentItem)
        compare(input.text, "An unfinished title")
        verify(input.visible)
        keyClick(Qt.Key_Tab)
        compare(host.activeFocusItem, input)
        compare(input.text, "An unfinished title")
    }
    function test_backtab_reaches_scrolling_toolbar_and_returns_to_selection() {
        host.width = 390
        host.font.pointSize = 20
        records.clear()
        for (var i = 0; i < 30; ++i)
            records.append({entryKind: "bookmark", entryId: "saved-" + i, nodeId: "",
                label: "Saved page " + i, secondary: "https://example.test/" + i, detail: ""})
        var view = createView()
        view.focusPrimaryControl()
        var list = findChild(view, "libraryEntryList")
        for (i = 0; i < 7; ++i) keyClick(Qt.Key_Down)
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        compare(host.activeFocusItem.objectName, "libraryCloseButton")
        verify(waitForPolish(host))
        tryVerify(function() {
            var control = host.activeFocusItem
            var top = control.mapToItem(list, 0, 0).y
            return top >= -1 && top + control.height <= list.height + 1
        })
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        compare(host.activeFocusItem.objectName, "libraryRefreshButton")
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        verify(host.activeFocusItem === list || host.activeFocusItem === list.currentItem,
            "Header focus returns to list: " + JSON.stringify({actual: host.activeFocusItem.objectName,
                first: list.parent.headerControls().first ? list.parent.headerControls().first.objectName : "none",
                last: list.parent.headerControls().last ? list.parent.headerControls().last.objectName : "none"}))
        compare(list.currentIndex, 7)
        var opened = createTemporaryObject(spyFixture, view, {target: view, signalName: "entryOpenRequested"})
        keyClick(Qt.Key_Return)
        compare(opened.signalArguments[0][1], "saved-7")
        view.focusPrimaryControl()
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        compare(host.activeFocusItem.objectName, "libraryCloseButton")
        keyClick(Qt.Key_Tab)
        compare(host.activeFocusItem.text, "Open")
        keyClick(Qt.Key_Return)
        compare(opened.signalArguments[1][1], "saved-7")
    }
    function test_replacing_rows_preserves_header_control_focus() {
        var view = createView()
        view.focusPrimaryControl()
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        compare(host.activeFocusItem.objectName, "libraryRefreshButton")
        var refresh = host.activeFocusItem
        var refreshed = createTemporaryObject(spyFixture, view, {target: view, signalName: "refreshRequested"})
        var opened = createTemporaryObject(spyFixture, view, {target: view, signalName: "entryOpenRequested"})
        records.clear()
        for (var i = 0; i < 20; ++i)
            records.append({entryKind: "bookmark", entryId: "replacement-" + i, nodeId: "",
                label: "Replacement page " + i, secondary: "https://example.test/" + i, detail: ""})
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        compare(host.activeFocusItem, refresh, "The new current row does not steal header focus")
        keyClick(Qt.Key_Return)
        compare(refreshed.count, 1)
        compare(opened.count, 0)
    }
    function test_unfinished_edit_survives_scrolling_away_data() {
        return [{tag: "bookmark", kind: "bookmark", library: "bookmarks", draft: "An unfinished title"},
            {tag: "empty-bookmark", kind: "bookmark", library: "bookmarks", draft: ""},
            {tag: "quickmark", kind: "quickmark", library: "quickmarks", draft: "https://unsaved.test/draft"}]
    }
    function test_unfinished_edit_survives_scrolling_away(data) {
        records.setProperty(0, "entryKind", data.kind)
        for (var i = 1; i < 40; ++i)
            records.append({entryKind: data.kind, entryId: "record-" + i, nodeId: "",
                label: "Saved page " + i, secondary: "https://example.test/" + i, detail: ""})
        var view = createView()
        view.libraryKind = data.library
        var saved = createTemporaryObject(spyFixture, view, {target: view, signalName: "entryEditRequested"})
        edit(view, data.draft)
        var list = findChild(view, "libraryEntryList")
        list.cacheBuffer = 0
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        keyClick(Qt.Key_End)
        compare(list.currentIndex, 39)
        verify(waitForRendering(host.contentItem))
        tryVerify(function() { return list.itemAtIndex(0) === null }, 5000,
            "The original editor is outside the virtualized viewport")
        keyClick(Qt.Key_Home)
        compare(list.currentIndex, 0)
        verify(waitForPolish(host))
        var input = findChild(list.currentItem, "libraryEntryEdit")
        verify(input.visible, "Returning to an entry restores its unfinished edit")
        compare(input.text, data.draft)
        compare(saved.count, 0, "Scrolling never saves a draft")
        host.update()
        verify(waitForRendering(host.contentItem))
    }
    function test_save_receipt_applies_after_editor_recycling_data() {
        return [{tag: "success", state: "succeeded"}, {tag: "failure", state: "failed"}]
    }
    function test_save_receipt_applies_after_editor_recycling(data) {
        for (var i = 1; i < 40; ++i)
            records.append({entryKind: "bookmark", entryId: "record-" + i, nodeId: "",
                label: "Saved page " + i, secondary: "https://example.test/" + i, detail: ""})
        var view = createView()
        edit(view, "A submitted draft")
        keyClick(Qt.Key_Return)
        receipt(view, "save-away", "pending")
        view.mutationAccepted("save-away", "bookmark", "internal-record-123", "A submitted draft", false)
        var list = findChild(view, "libraryEntryList")
        list.cacheBuffer = 0
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        keyClick(Qt.Key_End)
        verify(waitForRendering(host.contentItem))
        tryVerify(function() { return list.itemAtIndex(0) === null })
        receipt(view, "save-away", data.state)
        keyClick(Qt.Key_Home)
        verify(waitForPolish(host))
        var input = findChild(list.currentItem, "libraryEntryEdit")
        compare(input.visible, data.state === "failed")
        if (input.visible) compare(input.text, "A submitted draft")
        verify(waitForRendering(host.contentItem))
    }
    function test_cancelled_edit_does_not_return_after_recycling() {
        for (var i = 1; i < 40; ++i)
            records.append({entryKind: "bookmark", entryId: "record-" + i, nodeId: "",
                label: "Saved page " + i, secondary: "https://example.test/" + i, detail: ""})
        var view = createView()
        edit(view, "Discard this draft")
        var list = findChild(view, "libraryEntryList")
        list.cacheBuffer = 0
        findChild(list.currentItem, "libraryEntryCancelEditButton").click()
        for (i = 0; i < 8 && host.activeFocusItem !== list && host.activeFocusItem !== list.currentItem; ++i)
            keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        keyClick(Qt.Key_End)
        verify(waitForRendering(host.contentItem))
        tryVerify(function() { return list.itemAtIndex(0) === null })
        keyClick(Qt.Key_Home)
        verify(waitForPolish(host))
        compare(findChild(list.currentItem, "libraryEntryEdit").visible, false)
        verify(waitForRendering(host.contentItem))
    }
    function test_drafts_follow_record_identity_across_catalog_pages() {
        var original = {entryKind: "bookmark", entryId: "shared-id", nodeId: "",
            label: "A bookmark", secondary: "https://example.test/a", detail: ""}
        records.set(0, original)
        var view = createView()
        edit(view, "Keep the bookmark title")
        records.clear()
        records.append({entryKind: "quickmark", entryId: "shared-id", nodeId: "",
            label: "A quickmark", secondary: "https://example.test/b", detail: ""})
        view.libraryKind = "quickmarks"
        verify(waitForPolish(host))
        var list = findChild(view, "libraryEntryList")
        compare(findChild(list.currentItem, "libraryEntryEdit").visible, false,
            "A different record kind does not inherit the editor")
        edit(view, "https://unsaved.test/quickmark")
        records.clear()
        records.append(original)
        view.libraryKind = "bookmarks"
        verify(waitForPolish(host))
        var input = findChild(list.currentItem, "libraryEntryEdit")
        verify(input.visible)
        compare(input.text, "Keep the bookmark title")
    }
    function test_replacing_the_catalog_owner_clears_drafts() {
        var view = createView()
        edit(view, "A draft in the first catalog")
        var other = createTemporaryObject(alternateRecordsFixture, view)
        other.append({entryKind: "bookmark", entryId: "internal-record-123", nodeId: "",
            label: "A different catalog", secondary: "https://example.test/other", detail: ""})
        view.entries = other
        verify(waitForPolish(host))
        var list = findChild(view, "libraryEntryList")
        compare(findChild(list.currentItem, "libraryEntryEdit").visible, false)
        view.entries = records
        verify(waitForPolish(host))
        compare(findChild(list.currentItem, "libraryEntryEdit").visible, false)
    }
    function test_an_older_save_does_not_clear_a_newer_draft() {
        var view = createView()
        var input = edit(view, "The submitted value")
        receipt(view, "older-save", "pending")
        view.mutationAccepted("older-save", "bookmark", "internal-record-123", "The submitted value", false)
        receipt(view, "another-operation", "succeeded")
        input.forceActiveFocus()
        keyClick(Qt.Key_A, Qt.ControlModifier)
        keyClick(Qt.Key_N)
        receipt(view, "older-save", "succeeded")
        verify(input.visible)
        compare(input.text, "n")
    }
    function test_draft_tracking_preserves_native_undo_and_redo() {
        var view = createView()
        var original = records.get(0).label
        var input = edit(view, "An undoable draft")
        keyClick(Qt.Key_Z, Qt.ControlModifier)
        compare(input.text, original)
        keyClick(Qt.Key_Y, Qt.ControlModifier)
        compare(input.text, "An undoable draft")
    }
    function test_backtab_from_reference_and_journey_controls_data() {
        return [{tag: "action", kind: "action", library: "actions"},
            {tag: "context-route", kind: "context-route", library: "context-routes"},
            {tag: "journey", kind: "journey", library: "journey"}]
    }
    function test_backtab_from_reference_and_journey_controls(data) {
        records.setProperty(0, "entryKind", data.kind)
        var view = createView()
        view.libraryKind = data.library
        var list = findChild(view, "libraryEntryList")
        view.focusPrimaryControl()
        keyClick(Qt.Key_Tab)
        var first = host.activeFocusItem
        verify(first !== list && first !== list.currentItem)
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        verify(host.activeFocusItem === list || host.activeFocusItem === list.currentItem)
    }
    function test_large_row_actions_fit_the_viewport_data() {
        return [{tag: "bookmark", kind: "bookmark", library: "bookmarks", privateHistory: false},
            {tag: "private-history", kind: "history", library: "history", privateHistory: true},
            {tag: "journey", kind: "journey", library: "journey", privateHistory: false}]
    }
    function test_large_row_actions_fit_the_viewport(data) {
        host.width = 390
        host.font.pointSize = 26
        host.temporaryProfile = data.privateHistory
        records.setProperty(0, "entryKind", data.kind)
        var view = createView()
        view.libraryKind = data.library
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        var list = findChild(view, "libraryEntryList")
        var keys = visibleChild(view, "dialogKeyboardHelp")
        verify(keys)
        verify(keys.parent.height <= keys.height + 1,
            "The compact keyboard hint leaves room for the library: "
                + JSON.stringify({height: keys.parent.height, keyHeight: keys.height,
                    width: keys.parent.width, keyWidth: keys.width,
                    labels: keys.parent.children.map(function(c) {return {text: c.text, width: c.width, height: c.height}})}))
        function checkButtons(item) {
            if (!item.visible) return
            if (typeof item.click === "function") {
                var point = item.mapToItem(list, 0, 0)
                verify(point.x >= -1 && point.x + item.width <= list.width + 1,
                    item.text + " fits inside the viewport")
                item.forceActiveFocus()
                verify(waitForPolish(host))
                if (item.height <= list.height)
                    tryVerify(function() {
                        var top = item.mapToItem(list, 0, 0).y
                        return top >= -1 && top + item.height <= list.height + 1
                    }, 5000, item.text + " remains visible when focused")
                if (testArtifactDirectory.length > 0
                        && (item.text === "Add to Favourites" || item.text === "Reopen in profile")) {
                    verify(waitForRendering(host.contentItem))
                    grabImage(host.contentItem).save(testArtifactDirectory + "/library-action-" + data.tag + ".png")
                }
            }
            for (var child of item.children) checkButtons(child)
        }
        checkButtons(list.currentItem)
    }
    function test_deletion_feedback_respects_modal_focus_data() {
        return [{tag: "frontmost", cover: "none"},
                {tag: "covered-before-feedback", cover: "before-feedback"},
                {tag: "covered-before-timer", cover: "before-timer"}]
    }
    function test_deletion_feedback_respects_modal_focus(row) {
        records.append({entryKind: "bookmark", entryId: "remaining-record", nodeId: "",
                        label: "Remaining bookmark", secondary: "https://example.test/b", detail: ""})
        var view = createView()
        var list = findChild(view, "libraryEntryList")
        var deleted = createTemporaryObject(spyFixture, view, {target: view, signalName: "entryDeleteRequested"})
        var button = findChild(list, "libraryEntryDeleteButton")
        button.forceActiveFocus()
        keyClick(Qt.Key_Return)
        compare(deleted.count, 1)
        compare(deleted.signalArguments[0][1], "internal-record-123")
        receipt(view,"delete-one", "pending")
        view.mutationAccepted("delete-one", "bookmark", "internal-record-123", "", true)
        var coveringDialog = null
        if (row.cover === "before-feedback")
            coveringDialog = createTemporaryObject(coveringDialogFixture, host.contentItem)
        records.remove(0)
        receipt(view,"delete-one", "succeeded")
        // No event processing between feedback and the new modal: ownership
        // must be checked when the queued restoration actually runs.
        if (row.cover === "before-timer")
            coveringDialog = createTemporaryObject(coveringDialogFixture, host.contentItem)
        verify(waitForPolish(host))
        if (coveringDialog) {
            verify(view.visible)
            compare(view.interactive, false)
            var back = findChild(coveringDialog, "commandDialogAction-back")
            verify(back)
            // Observe focus after the queued timers, rather than allowing a
            // transient initial focus assignment to satisfy the regression.
            wait(50)
            compare(back.activeFocus, true)
            compare(list.activeFocus, false)
            keyClick(Qt.Key_Escape)
            tryCompare(coveringDialog, "visible", false)
        }
        tryCompare(view, "interactive", true)
        tryCompare(list, "activeFocus", true)
        compare(list.currentIndex, 0)
        compare(records.get(list.currentIndex).entryId, "remaining-record")
    }
    function test_canonical_journey_target_and_list_activation() {
        records.set(0,{entryKind:"journey",entryId:"node-one",nodeId:"node-one",label:"Article",secondary:"https://example.test",detail:""})
        var view=createView(); view.libraryKind="journey"
        var spy=createTemporaryObject(spyFixture,view,{target:view,signalName:"journeyReopenRequested"})
        var list=findChild(view,"libraryEntryList"); wait(50); list.forceActiveFocus(); tryVerify(function(){return host.activeFocusItem === list || host.activeFocusItem === list.currentItem}); keyClick(Qt.Key_Return)
        compare(spy.count,1); compare(spy.signalArguments[0][0],"node-one"); compare(spy.signalArguments[0][1],"current")
        var row=list.currentItem; var selector=null
        function findSelector(item) { if (item.valueRole === "value") return item; for(var i=0;i<item.children.length;i++) { var result=findSelector(item.children[i]); if(result)return result } return null }
        selector=findSelector(row); verify(selector); selector.forceActiveFocus(); keyClick(Qt.Key_End)
        compare(selector.currentValue, "window")
        function findButton(item) { if(item.text === "Reopen" && typeof item.click === "function")return item; for(var i=0;i<item.children.length;i++){var result=findButton(item.children[i]);if(result)return result}return null }
        var button=findButton(row); verify(button); button.forceActiveFocus(); keyClick(Qt.Key_Return)
        compare(spy.count,2); compare(spy.signalArguments[1][1],"window")
    }
    function test_journey_list_enter_respects_the_selected_destination_data() {
        return [{tag: "new-tab", index: 1, target: "tab"},
                {tag: "new-window", index: 2, target: "window"}]
    }
    function test_journey_list_enter_respects_the_selected_destination(data) {
        records.set(0, {entryKind: "journey", entryId: "display-id", nodeId: "canonical-node",
            label: "A saved page", secondary: "https://example.test/saved", detail: ""})
        var view = createView()
        view.libraryKind = "journey"
        var reopened = createTemporaryObject(spyFixture, view, {target: view, signalName: "journeyReopenRequested"})
        var list = findChild(view, "libraryEntryList")
        view.focusPrimaryControl()
        keyClick(Qt.Key_Tab)
        var selector = host.activeFocusItem
        compare(selector.valueRole, "value")
        for (var i = 0; i < data.index; ++i) keyClick(Qt.Key_Down)
        compare(selector.currentValue, data.target)
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        verify(host.activeFocusItem === list || host.activeFocusItem === list.currentItem)
        keyClick(Qt.Key_Return)
        compare(reopened.count, 1)
        compare(reopened.signalArguments[0][0], "canonical-node")
        compare(reopened.signalArguments[0][1], data.target,
            "Enter uses the destination the user chose")
    }
    function test_journey_destination_survives_recycling_data() {
        return [{tag: "tab", index: 1, target: "tab"},
                {tag: "window", index: 2, target: "window"}]
    }
    function test_journey_destination_survives_recycling(data) {
        records.clear()
        for (var i = 0; i < 40; ++i)
            records.append({entryKind: "journey", entryId: "display-" + i, nodeId: "node-" + i,
                label: "Journey page " + i, secondary: "https://example.test/" + i, detail: ""})
        var view = createView()
        view.libraryKind = "journey"
        var list = findChild(view, "libraryEntryList")
        list.cacheBuffer = 0
        view.focusPrimaryControl()
        keyClick(Qt.Key_Tab)
        for (i = 0; i < data.index; ++i) keyClick(Qt.Key_Down)
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        keyClick(Qt.Key_End)
        verify(waitForRendering(host.contentItem))
        tryVerify(function() { return list.itemAtIndex(0) === null })
        keyClick(Qt.Key_Tab)
        compare(host.activeFocusItem.currentValue, "current", "Other nodes keep their own destination")
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        keyClick(Qt.Key_Home)
        verify(waitForPolish(host))
        keyClick(Qt.Key_Tab)
        compare(host.activeFocusItem.currentValue, data.target)
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        var reopened = createTemporaryObject(spyFixture, view, {target: view, signalName: "journeyReopenRequested"})
        keyClick(Qt.Key_Return)
        compare(reopened.signalArguments[0][0], "node-0")
        compare(reopened.signalArguments[0][1], data.target)
        verify(waitForRendering(host.contentItem))
    }
    function test_journey_popup_commit_controls_list_activation() {
        records.setProperty(0, "entryKind", "journey")
        records.setProperty(0, "nodeId", "canonical-node")
        var view = createView()
        view.libraryKind = "journey"
        view.focusPrimaryControl()
        keyClick(Qt.Key_Tab)
        var selector = host.activeFocusItem
        keyClick(Qt.Key_Space)
        tryCompare(selector.popup, "visible", true)
        keyClick(Qt.Key_End)
        keyClick(Qt.Key_Return)
        tryCompare(selector.popup, "visible", false)
        compare(selector.currentValue, "window")
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        var reopened = createTemporaryObject(spyFixture, view, {target: view, signalName: "journeyReopenRequested"})
        keyClick(Qt.Key_Return)
        compare(reopened.signalArguments[0][1], "window")
    }
    function test_journey_destination_survives_metadata_changes() {
        records.setProperty(0, "entryKind", "journey")
        records.setProperty(0, "nodeId", "canonical-node")
        var view = createView()
        view.libraryKind = "journey"
        view.focusPrimaryControl()
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_End)
        records.setProperty(0, "entryId", "a-new-display-id")
        records.setProperty(0, "label", "Refreshed page title")
        verify(waitForPolish(host))
        compare(host.activeFocusItem.currentValue, "window")
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        var reopened = createTemporaryObject(spyFixture, view, {target: view, signalName: "journeyReopenRequested"})
        keyClick(Qt.Key_Return)
        compare(reopened.signalArguments[0][0], "canonical-node")
        compare(reopened.signalArguments[0][1], "window")
    }
    function test_journey_destination_resets_with_catalog_owner() {
        records.setProperty(0, "entryKind", "journey")
        records.setProperty(0, "nodeId", "canonical-node")
        var view = createView()
        view.libraryKind = "journey"
        view.focusPrimaryControl()
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_End)
        var other = createTemporaryObject(alternateRecordsFixture, view)
        other.append({entryKind: "journey", entryId: "display-id", nodeId: "canonical-node",
            label: "A different catalog", secondary: "https://example.test/other", detail: ""})
        view.entries = other
        verify(waitForPolish(host))
        view.focusPrimaryControl()
        keyClick(Qt.Key_Tab)
        compare(host.activeFocusItem.currentValue, "current")
    }
    function test_journey_destinations_fit_large_text_data() {
        return [{tag: "20-point", points: 20}, {tag: "26-point", points: 26}]
    }
    function test_journey_destinations_fit_large_text(data) {
        host.width = 390
        host.font.pointSize = data.points
        records.setProperty(0, "entryKind", "journey")
        var view = createView()
        view.libraryKind = "journey"
        view.focusPrimaryControl()
        keyClick(Qt.Key_Tab)
        var selector = host.activeFocusItem
        compare(selector.objectName, "libraryJourneyDestination")
        for (var i = 0; i < 3; ++i) {
            if (i > 0) keyClick(Qt.Key_Down)
            verify(waitForPolish(host))
            var label = selector.contentItem
            verify(label.contentWidth <= label.width - label.leftPadding - label.rightPadding + 1,
                selector.currentText + " must be fully readable at " + data.points + " points")
            var list = findChild(view, "libraryEntryList")
            var point = selector.mapToItem(list, 0, 0)
            verify(point.x >= -1 && point.x + selector.width <= list.width + 1,
                "The destination selector fits within the viewport")
        }
    }
    function test_journey_destination_can_return_to_current_tab() {
        records.setProperty(0, "entryKind", "journey")
        var view = createView()
        view.libraryKind = "journey"
        view.focusPrimaryControl()
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_End)
        keyClick(Qt.Key_Home)
        compare(host.activeFocusItem.currentValue, "current")
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        var reopened = createTemporaryObject(spyFixture, view, {target: view, signalName: "journeyReopenRequested"})
        keyClick(Qt.Key_Return)
        compare(reopened.signalArguments[0][1], "current")
    }
    function test_read_only_action_rows_do_not_open_quickmarks() {
        records.setProperty(0,"entryKind","action")
        var view=createView(); view.libraryKind="actions"
        compare(view.commandText,":action-list")
        view.libraryKind="context-routes"; compare(view.commandText,":context-route list")
        view.libraryKind="actions"
        var opened=createTemporaryObject(spyFixture,view,{target:view,signalName:"entryOpenRequested"})
        findChild(view,"libraryEntryList").forceActiveFocus(); keyClick(Qt.Key_Return)
        compare(opened.count,0)
    }
    function test_modified_enter_does_not_open_an_entry() {
        var view=createView(); var opened=createTemporaryObject(spyFixture,view,{target:view,signalName:"entryOpenRequested"})
        findChild(view,"libraryEntryList").forceActiveFocus(); keyClick(Qt.Key_Return,Qt.ControlModifier)
        compare(opened.count,0)
    }
    function test_compact_controls_and_literal_details_data() {
        return [{tag:"wide",width:900,height:700,font:10},{tag:"compact",width:390,height:600,font:10},{tag:"large",width:390,height:700,font:16}]
    }
    function test_compact_controls_and_literal_details(row) {
        host.width=row.width;host.height=row.height;host.font.pointSize=row.font
        var view=createView(); var list=findChild(view,"libraryEntryList")
        var title=findChild(list,"libraryEntryTitle")
        compare(title.text,"<b>A bookmarked page</b>"); compare(title.textFormat,Text.PlainText)
        var button=findChild(list,"libraryEntryDeleteButton")
        verify(button.Accessible.name.indexOf("internal-record")<0)
        button.forceActiveFocus(); tryVerify(function(){var p=button.mapToItem(list,0,0);return p.x>=0&&p.x+button.width<=list.width&&p.y>=0&&p.y+button.height<=list.height+1})
        findChild(list,"libraryEntryDetailsButton").click()
        var details=findChild(list,"libraryEntryDetails")
        tryCompare(details,"visible",true); compare(details.textFormat,TextEdit.PlainText);verify(details.selectByMouse)
        button.forceActiveFocus(); wait(50)
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/library-"+row.tag+".png")
    }
    function test_graph_and_search_fit_large_text() {
        host.width=390;host.height=700;host.font.pointSize=16
        var view=createView();view.libraryKind="journey";view.graphMode=true
        verify(waitForPolish(host)); wait(50)
        var graph=findChild(view,"journeyGraphList"); var controls=findChild(graph,"journeyGraphControls")
        var search=findChild(graph,"journeySearchInput")
        verify(search.visible)
        tryVerify(function(){var point=search.mapToItem(graph,0,0);return point.y>=-1&&point.y+search.height<=graph.height+1})
        verify(view.graphNodeHeight >= 60)
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/library-graph-large.png")
    }
    function createLongGraph(withEdges) {
        host.width = 390
        host.font.pointSize = 20
        graphEdges.clear()
        if (withEdges) {
            for (var i = 0; i < 40; ++i)
                graphEdges.append({label: "Page " + i + " connects to another saved page",
                    secondary: "A navigation relationship with readable details"})
        }
        var view = createView()
        view.libraryKind = "journey"
        view.graphMode = true
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        var graph = findChild(view, "journeyGraphList")
        graph.forceActiveFocus()
        tryCompare(graph, "activeFocus", true)
        return {view: view, graph: graph}
    }
    function test_graph_pages_by_the_visible_viewport_data() {
        return [{tag: "relationships", edges: true}, {tag: "map-only", edges: false}]
    }
    function test_graph_pages_by_the_visible_viewport(data) {
        var fixture = createLongGraph(data.edges)
        var graph = fixture.graph
        var beginning = graph.contentY
        keyClick(Qt.Key_PageDown)
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        verify(graph.contentY > beginning, "Page Down moves through the relationships view")
        verify(graph.contentY - beginning <= graph.height + 1, "Paging keeps visible overlap")
        keyClick(Qt.Key_PageUp)
        verify(waitForPolish(host))
        tryVerify(function() { return Math.abs(graph.contentY - beginning) < 1 })
        verify(graph.activeFocus)
    }
    function test_graph_paging_reaches_the_final_row() {
        var fixture = createLongGraph(true)
        var graph = fixture.graph
        graphEdges.clear()
        for (var i = 0; i < 14; ++i)
            graphEdges.append({label: i === 0 ? "Owned child secret"
                    : "Owned child secret → Owned child secret", secondary: "Navigation"})
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        for (i = 0; i < 100; ++i) {
            var oldY = graph.contentY
            keyClick(Qt.Key_PageDown)
            verify(waitForPolish(host))
            verify(waitForRendering(host.contentItem))
            if (Math.abs(graph.contentY - oldY) < 1) break
        }
        verify(i < 100, "Paging reaches a stable boundary")
        var last = graph.itemAtIndex(graph.count - 1)
        verify(last !== null)
        var bottom = last.mapToItem(graph, 0, last.height).y
        verify(bottom > 0 && bottom <= graph.height + 1,
            "The final row fits at the boundary: " + bottom + " / " + graph.height)
    }
    function test_graph_home_and_end_reach_the_bounds_data() {
        return [{tag: "home", key: Qt.Key_Home, top: true},
            {tag: "end", key: Qt.Key_End, top: false},
            {tag: "g", key: Qt.Key_G, top: true},
            {tag: "G", key: Qt.Key_G, modifiers: Qt.ShiftModifier, top: false},
            {tag: "map-home", key: Qt.Key_Home, top: true, edges: false},
            {tag: "map-end", key: Qt.Key_End, top: false, edges: false}]
    }
    function test_graph_home_and_end_reach_the_bounds(data) {
        var fixture = createLongGraph(data.edges !== false)
        var graph = fixture.graph
        if (data.top) for (var i = 0; i < 12; ++i) keyClick(Qt.Key_Down)
        keyClick(data.key, data.modifiers || Qt.NoModifier)
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        if (data.top) {
            verify(graph.headerItem.mapToItem(graph, 0, 0).y >= -1,
                "Home returns to the search controls")
        } else if (graph.count > 0) {
            var last = graph.itemAtIndex(graph.count - 1)
            verify(last !== null, "End reaches the last relationship")
            var bottom = last.mapToItem(graph, 0, last.height).y
            verify(bottom <= graph.height + 1 && bottom > 0)
        } else {
            var headerBottom = graph.headerItem.mapToItem(graph, 0, graph.headerItem.height).y
            verify(headerBottom <= graph.height + 1 && headerBottom > 0,
                "End reaches the map even without relationship rows")
            var node = findChild(graph, "journeyMapNode")
            verify(node !== null)
            verify(node.mapToItem(graph, 0, 0).y >= -1
                && node.mapToItem(graph, 0, node.height).y <= graph.height + 1,
                "End shows the actual node, not unused map space")
        }
        verify(graph.activeFocus)
    }
    function test_graph_shortcuts_preserve_search_editing() {
        var fixture = createLongGraph(true)
        var input = findChild(fixture.graph, "journeySearchInput")
        input.forceActiveFocus()
        input.text = "saved browsing pages"
        input.cursorPosition = 5
        verify(waitForPolish(host))
        var before = fixture.graph.contentY
        keyClick(Qt.Key_Home)
        compare(input.cursorPosition, 0)
        keyClick(Qt.Key_End, Qt.ShiftModifier)
        compare(input.selectedText, input.text)
        keyClick(Qt.Key_PageDown)
        compare(fixture.graph.contentY, before)
        verify(input.activeFocus)
        verify(fixture.view.keyHelp.indexOf("PgUp/PgDn") >= 0)
        fixture.view.focusPrimaryControl()
        compare(host.activeFocusItem, fixture.graph, "Returning to browsing clears search subfocus")
        keyClick(Qt.Key_Tab)
        compare(host.activeFocusItem.objectName, "libraryGraphToggleButton")
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        compare(host.activeFocusItem, fixture.graph, "The first control returns to graph browsing")
        input.forceActiveFocus()
        fixture.view.graphMode = false
        verify(waitForRendering(host.contentItem))
        fixture.view.graphMode = true
        verify(waitForRendering(host.contentItem))
        compare(host.activeFocusItem, fixture.graph, "Returning to Relationships clears retained subfocus")
        keyClick(Qt.Key_PageDown)
        verify(waitForRendering(host.contentItem))
        verify(fixture.graph.headerItem.mapToItem(fixture.graph, 0, 0).y < -1)
    }
    function test_graph_map_arrows_pan_without_scrolling_relationships() {
        var fixture = createLongGraph(true)
        var map = findChild(fixture.graph, "journeyMapViewport")
        map.forceActiveFocus()
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        var before = fixture.graph.contentY
        keyClick(Qt.Key_Right)
        verify(map.contentX > 0)
        compare(fixture.graph.contentY, before)
        verify(map.activeFocus)
    }
    function test_graph_single_node_does_not_pan_into_empty_columns() {
        var fixture = createLongGraph(false)
        host.width = 900
        host.font.pointSize = 10
        var map = findChild(fixture.graph, "journeyMapViewport")
        map.forceActiveFocus()
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        verify(map.width > fixture.view.graphNodeWidth + 24)
        for (var i = 0; i < 10; ++i) keyClick(Qt.Key_Right)
        compare(map.contentX, 0, "A map that fits does not scroll into unused grid columns")
    }
    function test_graph_focused_map_pages_within_its_visible_bounds_data() {
        return [{tag: "20-point", points: 20}, {tag: "26-point", points: 26}]
    }
    function test_graph_focused_map_pages_within_its_visible_bounds(data) {
        var fixture = createLongGraph(true)
        host.font.pointSize = data.points
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        fixtureGraphNodes.clear()
        for (var i = 0; i < 20; ++i)
            fixtureGraphNodes.append({nodeId: "map-" + i, label: "Saved map page " + i,
                transition: "navigate", source: "", x: (i % 4) * (fixture.view.graphNodeWidth + 24),
                y: Math.floor(i / 4) * (fixture.view.graphNodeHeight + 18)})
        var map = findChild(fixture.graph, "journeyMapViewport")
        map.forceActiveFocus()
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        verify(map.height <= fixture.graph.height + 1, "The focused map fits inside the reading viewport")
        var outerY = fixture.graph.contentY
        keyClick(Qt.Key_Right)
        verify(map.contentX > 0)
        keyClick(Qt.Key_PageDown)
        verify(map.contentY > 0 && map.contentY <= map.height + 1)
        keyClick(Qt.Key_End)
        compare(map.contentY, Math.max(0, map.contentHeight - map.height))
        keyClick(Qt.Key_Home)
        compare(map.contentY, 0)
        keyClick(Qt.Key_G, Qt.ShiftModifier)
        compare(map.contentY, Math.max(0, map.contentHeight - map.height))
        keyClick(Qt.Key_G)
        compare(map.contentY, 0)
        compare(fixture.graph.contentY, outerY, "Map keys do not move the relationship outline")
        compare(host.activeFocusItem, map)
    }
    function test_graph_first_page_after_font_and_window_changes() {
        var fixture = createLongGraph(true)
        host.height = 600
        // QWindow geometry is asynchronous. Acknowledge the native resize,
        // then deliver the key while the new text layout is still pending.
        tryCompare(host.contentItem, "height", 600)
        host.font.pointSize = 26
        var requested = {window: host.height, content: host.contentItem.height,
            viewport: fixture.graph.height, panel: findChild(fixture.view, "ferricModalPanel").height}
        keyClick(Qt.Key_PageDown)
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        var top = fixture.graph.headerItem.mapToItem(fixture.graph, 0, 0).y
        verify(fixture.graph.height >= actualFontMetrics.height * 2,
            "Large text leaves a readable viewport after the toolbar scrolls")
        verify(top < -1, "The first page is not reset by the resized header")
        verify(-top <= fixture.graph.height + 1, "The first page uses the new viewport: "
            + JSON.stringify({top: top, height: fixture.graph.height, y: fixture.graph.contentY,
                headerY: fixture.graph.headerItem.y, headerHeight: fixture.graph.headerItem.height,
                origin: fixture.graph.originY, requested: requested}))
        verify(fixture.graph.activeFocus)
    }
    function test_large_toolbar_controls_scroll_with_the_results_data() {
        return [{tag: "outline", graph: false}, {tag: "relationships", graph: true}]
    }
    function test_large_toolbar_controls_scroll_with_the_results(data) {
        host.width = 390
        host.height = 600
        host.font.pointSize = 26
        records.setProperty(0, "entryKind", "journey")
        var view = createView()
        view.libraryKind = "journey"
        view.graphMode = data.graph
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        var list = visibleChild(view, data.graph ? "journeyGraphList" : "libraryEntryList")
        verify(list.height >= actualFontMetrics.height * 2, "The results retain room for readable text")
        var toolbar = visibleChild(view, "libraryToolbar")
        verify(toolbar)
        for (var control of toolbar.children) {
            if (!control.visible || typeof control.click !== "function") continue
            control.forceActiveFocus()
            verify(waitForPolish(host))
            var left = control.mapToItem(list, 0, 0).x
            verify(left >= -1 && left + control.width <= list.width + 1,
                control.text + " fits the narrow toolbar")
            tryVerify(function() {
                var top = control.mapToItem(list, 0, 0).y
                return top >= -1 && top + control.height <= list.height + 1
            })
            if (control.objectName === "libraryGraphToggleButton" && testArtifactDirectory.length > 0) {
                verify(waitForRendering(host.contentItem))
                grabImage(host.contentItem).save(testArtifactDirectory + "/library-toolbar-" + data.tag + ".png")
            }
        }
    }
    function test_empty_library_can_read_and_leave_tall_feedback() {
        records.clear()
        host.width = 390
        host.font.pointSize = 20
        var view = createTemporaryObject(fixture, host.contentItem)
        verify(view)
        view.rejectionMessage = "A detailed library operation message. ".repeat(40)
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        var list = findChild(view, "libraryEntryList")
        view.focusPrimaryControl()
        keyClick(Qt.Key_PageDown)
        verify(waitForPolish(host))
        verify(list.headerItem.mapToItem(list, 0, 0).y < -1)
        compare(list.currentIndex, -1)
        keyClick(Qt.Key_End)
        verify(waitForPolish(host))
        verify(list.headerItem.mapToItem(list, 0, list.headerItem.height).y <= list.height + 1)
        keyClick(Qt.Key_Home)
        verify(waitForPolish(host))
        verify(list.headerItem.mapToItem(list, 0, 0).y >= -1)
        compare(list.currentIndex, -1)
    }
    function test_journey_search_waits_for_the_accepted_query() {
        var view=createView(); view.libraryKind="journey"
        var input=findChild(view,"journeySearchInput")
        var spy=createTemporaryObject(spyFixture,view,{target:view,signalName:"journeySearchRequested"})
        view.listPending=true; verify(input.readOnly)
        input.forceActiveFocus(); keyClick(Qt.Key_Return); compare(spy.count,0)
        view.listPending=false; verify(!input.readOnly)
        keyClick(Qt.Key_Return); compare(spy.count,1)
    }
    function test_empty_and_failed_reads_are_distinct() {
        records.clear();var view=createTemporaryObject(fixture,host.contentItem);verify(view);verify(waitForPolish(host))
        var empty=findChild(view,"libraryEmptyState");verify(empty.visible);verify(empty.text.indexOf(":bookmark-add")>=0)
        view.listPending=true;verify(!empty.visible)
        view.listPending=false;view.listError="disk unavailable";verify(!empty.visible)
        verify(findChild(view,"libraryLoadingFeedback").failed)
    }
    function test_reference_enter_inspects_without_executing() {
        var view=createView();view.libraryKind="actions"
        records.setProperty(0,"entryKind","action")
        var opened=createTemporaryObject(spyFixture,view,{target:view,signalName:"entryOpenRequested"})
        var list=findChild(view,"libraryEntryList");list.forceActiveFocus()
        keyClick(Qt.Key_Return)
        tryCompare(findChild(view,"libraryEntryDetails"),"visible",true)
        compare(opened.count,0)
        verify(view.keyHelp.indexOf("enter details")>=0)
    }

}
