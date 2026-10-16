import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
TestCase {
    id: test
    name: "NavigationPolish"
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
        property bool diagnosticsVisible: true
        property string displayScaleLabel: "1×"
        property string chromeFontFamily: "monospace"
        property bool reducedMotionActive: true
        property string themeContrastWarning: ""
        function activeWebView() { return outside }
        TextField { id: outside; y: 10; text: "Outside draft" }
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


    property var searchRows: [{identity:"first",meaning:"a",kind:"link",text:"<b>Literal article title</b>"}]
    Component { id: spyFixture; SignalSpy {} }
    FerricFocusOverlayController { id: focusController; browserWindow: host }
    Component {
        id: searchFixture
        FerricTargetSearch {
            browserWindow: host
            searchVisible: true
            scope: "links"; query: ""
            rows: test.searchRows
            selectedIdentity: "first"; selectedMeaning: "a"
            onOpening: focusController.captureOverlayFocus(host,outside)
            onClosed: focusController.restoreOverlayFocus()
        }
    }
    Component {
        id: diagnosticsFixture
        FerricDiagnostics {
            browserWindow: host
            previewText: "{\"error\":\"<b>literal detail</b>\"}"
            summaryRows: ["Web engine: Unavailable", "Profile storage: Not checked"]
            reportAvailable: true
        }
    }
    Component {
        id: scrollFixture
        FerricScrollColumn {
            width: 300; height: 200; hostWindow: host
            Label { objectName:"shortContent"; text:"Short content" }
        }
    }
    function init() {
        failOnWarning(/invalid context|TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width=900;host.height=700;host.font.pointSize=10
        searchRows=[{identity:"first",meaning:"a",kind:"link",text:"<b>Literal article title</b>"}]
        host.requestActivate();tryCompare(host,"active",true)
        outside.forceActiveFocus()
    }
    function sizes() { return [{tag:"wide",width:900,height:700,font:10},{tag:"compact",width:640,height:480,font:16},{tag:"narrow",width:390,height:600,font:20}] }
    function test_search_readability_data() { return sizes() }
    function test_search_readability(data) {
        host.width=data.width;host.height=data.height;host.font.pointSize=data.font
        var view=createTemporaryObject(searchFixture,host.contentItem);verify(view);verify(waitForPolish(host))
        var input=findChild(view,"targetSearchInput");tryCompare(input,"activeFocus",true)
        var label=findChild(view,"targetSearchResultText");verify(label)
        compare(label.textFormat,Text.PlainText);compare(label.text,"<b>Literal article title</b>")
        verify(label.width>view.bodyItem.width-80)
        verify(label.height>=label.implicitHeight-1)
        var confirm=createTemporaryObject(spyFixture,view,{target:view,signalName:"confirmRequested"})
        keyClick(Qt.Key_Return);compare(confirm.count,1)
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/target-search-"+data.tag+".png")
        view.searchVisible=false;tryCompare(outside,"activeFocus",true)
    }
    function test_search_reveals_keyboard_selection() {
        var rows=[];for(var i=0;i<80;i++)rows.push({identity:"r"+i,meaning:"m"+i,kind:"button",text:"Control "+i})
        searchRows=rows
        var view=createTemporaryObject(searchFixture,host.contentItem);verify(view);verify(waitForPolish(host))
        view.selectedIdentity="r79";view.selectedMeaning="m79"
        var list=findChild(view,"targetSearchList")
        tryVerify(function(){return list.contentY>0})
        var confirm=createTemporaryObject(spyFixture,view,{target:view,signalName:"confirmRequested"})
        view.selectedIdentity="";keyClick(Qt.Key_Return);compare(confirm.count,0)
    }
    function test_diagnostics_readable_and_truthful_data() { return sizes() }
    function test_diagnostics_readable_and_truthful(data) {
        host.width=data.width;host.height=data.height;host.font.pointSize=data.font
        var view=createTemporaryObject(diagnosticsFixture,host.contentItem);verify(view);verify(waitForPolish(host))
        verify(view.bodyItem.height > 150)
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/diagnostics-"+data.tag+".png")
        var report=findChild(view,"diagnosticsReportText");verify(!report.visible)
        var toggle=findChild(view,"diagnosticsDetailsToggle");toggle.forceActiveFocus();keyClick(Qt.Key_Return)
        tryCompare(report,"visible",true);compare(report.textFormat,TextEdit.PlainText);verify(report.readOnly);verify(report.selectByKeyboard)
        view.reportAvailable=false
        compare(findChild(view,"diagnosticsCopy").enabled,false)
        compare(findChild(view,"diagnosticsSave").enabled,false)
        verify(findChild(view,"diagnosticsUnavailable").visible)
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/diagnostics-details-"+data.tag+".png")
    }
    function test_diagnostics_preparation_keeps_close_available_and_errors_literal_data() { return sizes() }
    function test_diagnostics_preparation_keeps_close_available_and_errors_literal(data) {
        host.width=data.width;host.height=data.height;host.font.pointSize=data.font
        var view=createTemporaryObject(diagnosticsFixture,host.contentItem);verify(view);verify(waitForPolish(host))
        view.reportAvailable=false;view.reportPending=true
        view.previewText="";view.summaryRows=[]
        verify(waitForPolish(host))
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/diagnostics-pending-"+data.tag+".png")
        compare(findChild(view,"diagnosticsRefresh").enabled,false)
        compare(findChild(view,"diagnosticsCopy").enabled,false)
        compare(findChild(view,"diagnosticsSave").enabled,false)
        compare(findChild(view,"diagnosticsUnavailable").visible,false)
        var close=findChild(view,"diagnosticsClose");verify(close.enabled)
        var closed=createTemporaryObject(spyFixture,view,{target:view,signalName:"closeRequested"})
        close.click();compare(closed.count,1)
        view.reportPending=false;view.reportError="<b>fixture collector failed</b>"
        var feedback=findChild(view,"diagnosticsCollectionFeedback");verify(feedback.visible);verify(feedback.failed)
        compare(findChild(view,"diagnosticsRefresh").enabled,true)
        compare(findChild(view,"diagnosticsUnavailable").visible,false)
        var details=findChild(feedback,"managerFeedbackDetails");verify(!details.visible)
        findChild(feedback,"managerFeedbackDetailsButton").click()
        tryCompare(details,"visible",true);compare(details.textFormat,TextEdit.PlainText)
        compare(details.text,"<b>fixture collector failed</b>");verify(details.selectByKeyboard)
        verify(waitForPolish(host))
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/diagnostics-error-"+data.tag+".png")
    }

    function test_short_forms_do_not_show_scroll_tracks() {
        var view=createTemporaryObject(scrollFixture,host.contentItem);verify(view);verify(waitForPolish(host))
        compare(view.ScrollBar.vertical.policy,ScrollBar.AlwaysOff)
        findChild(view,"shortContent").text="A line\n".repeat(100)
        tryCompare(view.ScrollBar.vertical,"policy",ScrollBar.AsNeeded)
    }
}
