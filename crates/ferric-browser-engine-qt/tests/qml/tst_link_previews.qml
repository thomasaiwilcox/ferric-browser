import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "LinkPreviews"
    when: windowShown
    Component {
        id: hostFixture
        ApplicationWindow {
            id: host
            width: 900; height: 700; visible: true
            font.family: "monospace"; font.pointSize: 10
            FontMetrics { id: metrics; font: host.font }
            property real chromeScale: Math.max(1,metrics.height/14)
            readonly property real lineHeight: metrics.height
            readonly property var modalInputScope: FerricModalInputController { hostWindow: host }
            property bool runtimeDisposing: false
            property bool runtimeReleased: false
            property var currentView: page
            property alias pageItem: page
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
            property string notice: ""
            color: backgroundColor
            function activeWebView() { return currentView }
            function showCommandNotice(text,error) { notice=text }
            Item { id: page; anchors.fill: parent; property bool loading: false; property string nativeUrl: "" }
        }
    }
    Component {
        id: ownerFixture
        QtObject {
            id: owner
            property string core_window_id: "window-one"
            property string current_url: "https://origin.test/"
            property bool profile_bootstrap_pending: false
            property bool link_preview_visible: false
            property bool link_preview_requires_confirmation: false
            property string link_preview_command: "url-clean"
            property string link_preview_original: "<b>literal original</b>"
            property string link_preview_cleaned: "<b>literal result</b>"
            property var link_preview_applied_rules: ["<b>literal rule</b>"]
            property var link_preview_removed_parameters: ["tracking"]
            property var link_preview_retained_parameters: ["signed"]
            property string link_preview_explanation: "<b>literal explanation</b>"
            property string status_text: "This preview expired. Run the command again."
            property bool reject: false
            property int clearCalls: 0
            property int confirmCalls: 0
            function clear_link_preview() {
                clearCalls++;link_preview_visible=false;link_preview_requires_confirmation=false
            }
            function confirm_link_navigation() {
                confirmCalls++
                if(reject) return false
                link_preview_visible=false
                return true
            }
        }
    }
    Component { id: previewFixture; FerricLinkPreview {} }
    Component { id: pageFixture; Item {} }
    Component { id: spyFixture; SignalSpy {} }

    function init() {
        failOnWarning(/TypeError|ReferenceError|Cannot assign|Unable to assign|Binding loop|invalid context/)
    }
    function setup(extra) {
        var host=createTemporaryObject(hostFixture,test)
        verify(host)
        var owner=createTemporaryObject(ownerFixture,test,extra || {})
        verify(owner)
        var preview=createTemporaryObject(previewFixture,host.contentItem,{
            browserWindow:host,hostWindow:host,previewUi:owner})
        verify(preview)
        host.requestActivate();tryCompare(host,"active",true)
        host.pageItem.forceActiveFocus()
        return {host:host,owner:owner,preview:preview}
    }
    function show(state) {
        state.host.requestActivate();tryCompare(state.host,"active",true)
        state.host.pageItem.forceActiveFocus()
        state.owner.link_preview_visible=true
        tryCompare(state.preview,"visible",true)
        tryCompare(state.preview,"ownerCurrent",true)
        verify(waitForPolish(state.host))
    }
    function test_independent_windows_cancel_only_their_own_preview() {
        var a=setup(),b=setup({core_window_id:"window-two"})
        show(a);show(b)
        a.host.requestActivate();tryCompare(a.host,"active",true)
        verify(waitForPolish(a.host))
        mouseClick(findChild(a.preview,"linkPreviewClose"))
        tryCompare(a.owner,"link_preview_visible",false)
        compare(a.owner.clearCalls,1);compare(a.owner.confirmCalls,0)
        verify(b.preview.visible);compare(b.owner.clearCalls,0)
        tryCompare(a.host.pageItem,"activeFocus",true)
    }
    function test_confirmation_uses_only_the_requesting_owner() {
        var a=setup({link_preview_command:"open",link_preview_requires_confirmation:true})
        var b=setup({core_window_id:"window-two",link_preview_command:"open",link_preview_requires_confirmation:true})
        show(a);show(b)
        var spy=createTemporaryObject(spyFixture,test,{target:a.preview,signalName:"navigationConfirmed"})
        mouseClick(findChild(a.preview,"linkPreviewConfirm"))
        tryCompare(a.owner,"confirmCalls",1);compare(spy.count,1)
        compare(b.owner.confirmCalls,0);verify(b.preview.visible)
    }
    function test_keyboard_dismissal_and_confirmation() {
        var state=setup({link_preview_command:"open",link_preview_requires_confirmation:true})
        show(state)
        keyClick(Qt.Key_Escape)
        tryCompare(state.owner,"link_preview_visible",false)
        compare(state.owner.confirmCalls,0)
        tryCompare(state.host.pageItem,"activeFocus",true)
        state.owner.link_preview_requires_confirmation=true
        show(state)
        keyClick(Qt.Key_Y)
        tryCompare(state.owner,"confirmCalls",1)
        tryCompare(state.owner,"link_preview_visible",false)
    }
    function test_changed_owner_cancels_without_navigation_data() {
        return [{tag:"tab",kind:"tab"},{tag:"profile",kind:"profile"},
                {tag:"url",kind:"url"},{tag:"reload",kind:"reload"},{tag:"bootstrap",kind:"bootstrap"}]
    }
    function test_changed_owner_cancels_without_navigation(data) {
        var state=setup({link_preview_requires_confirmation:true})
        show(state)
        if(data.kind==="tab") state.host.currentView=createTemporaryObject(pageFixture,state.host.contentItem)
        else if(data.kind==="profile") state.owner.core_window_id="replacement-window"
        else if(data.kind==="url") state.owner.current_url="https://changed.test/"
        else if(data.kind==="reload") state.host.pageItem.loading=true
        else state.owner.profile_bootstrap_pending=true
        tryCompare(state.owner,"link_preview_visible",false)
        compare(state.owner.confirmCalls,0)
        compare(state.owner.clearCalls,1)
    }
    function test_initial_window_container_returns_focus_to_page_content() {
        var state=setup({current_url:"rb://blank"})
        var root=state.host.contentItem
        while(root.parent) root=root.parent
        state.host.pageItem.focus=false
        root.forceActiveFocus()
        tryCompare(state.host,"activeFocusItem",root)
        state.owner.link_preview_visible=true
        tryCompare(state.preview,"ownerCurrent",true)
        verify(waitForPolish(state.host))
        keyClick(Qt.Key_Escape)
        tryCompare(state.owner,"link_preview_visible",false)
        tryCompare(state.host.pageItem,"activeFocus",true)
    }
    function test_native_placeholder_initialization_preserves_the_preview() {
        var state=setup({link_preview_requires_confirmation:true,current_url:"rb://blank"})
        state.host.pageItem.nativeUrl="rb://blank"
        show(state)
        state.host.pageItem.loading=true
        wait(0)
        verify(state.preview.visible)
        compare(state.owner.clearCalls,0)
        state.owner.current_url="https://changed.test/"
        tryCompare(state.owner,"link_preview_visible",false)
        compare(state.owner.confirmCalls,0)
    }
    function test_rejected_confirmation_closes_and_reports_to_its_host() {
        var state=setup({link_preview_requires_confirmation:true,reject:true})
        show(state)
        keyClick(Qt.Key_Y)
        tryCompare(state.owner,"link_preview_visible",false)
        compare(state.owner.confirmCalls,1);compare(state.owner.clearCalls,1)
        compare(state.host.notice,state.owner.status_text)
    }
    function test_short_previews_keep_related_rows_together() {
        var state=setup({link_preview_command:"open",link_preview_requires_confirmation:true})
        show(state)
        var rows=findChild(state.preview,"linkPreviewBody").contentLayout.children
        var labels=[]
        for(var row of rows) if(typeof row.text === "string") labels.push(row)
        compare(labels.length,6)
        for(var index=0;index<labels.length;index++) {
            verify(labels[index].height<=labels[index].implicitHeight+1,"Short content should retain its natural height")
            if(index>0) verify(labels[index].y-labels[index-1].y-labels[index-1].height<=12,"Related details should stay together")
        }
        verify(state.preview.panelItem.height<state.host.height*0.8,"Short previews should fit their content")
        if(testArtifactDirectory.length>0) grabImage(state.host.contentItem).save(testArtifactDirectory+"/link-preview-short.png")
    }
    function test_literal_readable_compact_preview_data() {
        return [{tag:"normal",width:900,height:700,font:10},
                {tag:"compact",width:640,height:480,font:14},
                {tag:"narrow",width:390,height:600,font:20}]
    }
    function test_literal_readable_compact_preview(data) {
        var state=setup({link_preview_command:"open",link_preview_requires_confirmation:true})
        state.host.width=data.width;state.host.height=data.height;state.host.font.pointSize=data.font
        state.owner.link_preview_explanation="<b>Literal, long explanation</b> ".repeat(50)
        show(state)
        for(var name of ["linkPreviewOriginal","linkPreviewResult","linkPreviewExplanation"])
            compare(findChild(state.preview,name).textFormat,Text.PlainText)
        compare(findChild(state.preview,"linkPreviewResult").text,"Result: <b>literal result</b>")
        verify(findChild(state.preview,"linkPreviewBody").height >= state.host.lineHeight*2,
               "The preview must retain readable address space at larger fonts")
        var close=findChild(state.preview,"linkPreviewClose")
        var confirm=findChild(state.preview,"linkPreviewConfirm")
        for(var control of [close,confirm]) {
            var point=control.mapToItem(state.host.contentItem,0,0)
            verify(point.x>=0 && point.y>=0)
            verify(point.x+control.width<=state.host.width+1)
            verify(point.y+control.height<=state.host.height+1)
        }
        if(testArtifactDirectory.length>0)
            grabImage(state.host.contentItem).save(testArtifactDirectory+"/link-preview-"+data.tag+".png")
        mouseClick(close)
        tryCompare(state.owner,"link_preview_visible",false)
        compare(state.owner.confirmCalls,0)
    }
}
