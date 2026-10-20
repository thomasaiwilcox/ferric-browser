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
        selector=findSelector(row); verify(selector); selector.currentIndex=2
        function findButton(item) { if(item.text === "Reopen" && typeof item.click === "function")return item; for(var i=0;i<item.children.length;i++){var result=findButton(item.children[i]);if(result)return result}return null }
        var button=findButton(row); verify(button); button.forceActiveFocus(); keyClick(Qt.Key_Return)
        compare(spy.count,2); compare(spy.signalArguments[1][1],"window")
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
