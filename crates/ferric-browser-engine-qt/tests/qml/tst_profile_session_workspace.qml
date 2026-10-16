import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "ProfileSessionWorkspace"
    when: windowShown
    ApplicationWindow {
        id: host
        width: 900; height: 700; visible: true
        font.family: "monospace"; font.pointSize: 10
        property bool runtimeMetrics: false
        FontMetrics { id: runtimeFontMetrics; font: host.font }
        property real chromeScale: runtimeMetrics ? Math.max(1, runtimeFontMetrics.height / 14) : 1
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
        color: backgroundColor
        palette.window: backgroundColor; palette.base: panelColor
        palette.text: primaryTextColor; palette.button: surfaceColor
        palette.buttonText: primaryTextColor; palette.windowText: primaryTextColor
    }
    Component {
        id: ownerFixture
        QtObject {
            property var calls: []
            property bool accepted: true
            property string status_text: "private worker details"
            property string profile_values: "work\tWork profile\tuuid"
            property bool profile_values_pending: false
            property string profile_list_error: ""
            property string profile_preview_text: "Profile folders"
            property bool profile_preview_pending: false
            property string profile_preview_error: ""
            property string profile_action_id: ""
            property string profile_action_state: "idle"
            property string profile_action_message: ""
            property string profile_action_detail: ""
            property var session_names_values: ["saved"]
            property bool session_names_pending: false
            property string session_names_error: ""
            property string session_save_id: ""
            property string session_save_state: "idle"
            property string session_save_message: ""
            property string session_save_detail: ""
            property string session_delete_id: ""
            property string session_delete_state: "idle"
            property string session_delete_message: ""
            property string session_delete_detail: ""
            property var session_preview_labels: ["Saved page"]
            property var session_preview_urls: ["https://example.test/"]
            property var session_preview_pinned: ["false"]
            property var session_preview_muted: ["false"]
            property var previewRequest: []
            property string previewResult: ""
            function request_profile_list() { return accepted }
            function request_profile_window(name) { calls=calls.concat([{kind:"open-profile",name:name}]);return accepted }
            function list_named_sessions() { return accepted }
            function create_profile(name,label) { calls=calls.concat([{kind:"create",name:name,label:label}]);return accepted }
            function rename_profile(name,label) { calls=calls.concat([{kind:"rename",name:name,label:label}]);return accepted }
            function delete_profile(name,confirmed) { calls=calls.concat([{kind:"delete",name:name,confirmed:confirmed}]);return accepted }
            function save_named_session(name) { calls=calls.concat([{kind:"save",name:name}]);return accepted }
            function delete_named_session(name,confirmed) { calls=calls.concat([{kind:"delete-session",name:name,confirmed:confirmed}]);return accepted }
            function request_profile_delete_preview(name) { return accepted }
            function request_session_preview(name) { calls=calls.concat([{kind:"preview",name:name}]);return accepted }
            function request_named_session_load(name,append) { calls=calls.concat([{kind:"load",name:name,append:append}]);return accepted }
            function take_session_preview_request() { var value=previewRequest;previewRequest=[];return value }
            function take_session_preview() { var value=previewResult;previewResult="";return value }
        }
    }
    Component {
        id: workspaceFixture
        FerricProfileSessionWorkspace {
            browserWindow: host
            hostWindow: host
        }
    }
    function workspace(owner) {
        var view=createTemporaryObject(workspaceFixture,host.contentItem,{browserUi:owner})
        verify(view);return view
    }
    function init() { failOnWarning(/TypeError|ReferenceError|Cannot assign|Unable to assign|Binding loop/) }
    function test_owners_keep_independent_models_and_visibility() {
        var first=createTemporaryObject(ownerFixture,test)
        var second=createTemporaryObject(ownerFixture,test,{profile_values:"other\tOther profile\tother-id"})
        var a=workspace(first), b=workspace(second)
        a.showProfiles();verify(waitForPolish(host))
        compare(a.profileManagerVisible,true);compare(b.profileManagerVisible,false)
        compare(a.profilesModel.get(0).name,"work")
        a.closeProfiles();b.showProfiles();verify(waitForPolish(host))
        compare(b.profilesModel.get(0).name,"other")
        compare(a.profilesModel.get(0).name,"work")
        b.closeProfiles();a.showSessions();verify(waitForPolish(host))
        compare(a.namedSessionsModel.get(0).name,"saved")
        compare(b.sessionManagerVisible,false)
    }
    function test_command_request_is_drained_once_and_result_stays_separate() {
        var owner=createTemporaryObject(ownerFixture,test,{previewRequest:["saved","true"]})
        var view=workspace(owner)
        view.presentCommandRequest()
        compare(owner.previewRequest.length,0)
        compare(view.sessionPreviewName,"saved");compare(view.sessionPreviewAppend,true)
        compare(view.sessionPreviewLoading,true)
        // A restore for another workflow cannot dismiss a pending preview read.
        view.finishSessionRestore();compare(view.sessionPreviewVisible,true)
        owner.previewResult="Saved page details"
        tryCompare(view,"sessionPreviewLoading",false)
        compare(view.sessionPreviewText,"Saved page details")
        view.presentCommandRequest();compare(view.sessionPreviewName,"saved")
        verify(view.loadPreviewedSession())
        compare(owner.calls[owner.calls.length-1].append,true)
        view.finishSessionRestore();compare(view.sessionPreviewVisible,false)
        compare(view.sessionPreviewLoading,false)
    }
    function test_failed_preview_remains_local_and_does_not_offer_restore() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);verify(view.showSessionPreview("saved",false))
        owner.previewResult="ERROR\t<b>literal unavailable detail</b>"
        tryCompare(view,"sessionPreviewLoading",false)
        compare(view.sessionPreviewError,true)
        compare(view.sessionPreviewText,"<b>literal unavailable detail</b>")
        compare(view.loadPreviewedSession(),false)
        compare(owner.calls.length,1)
        view.closeSessions();compare(view.sessionPreviewVisible,false)
    }
    function test_cancelled_folder_preview_cannot_reopen_the_manager() {
        var owner=createTemporaryObject(ownerFixture,test,{profile_preview_pending:true})
        var view=workspace(owner);view.showProfiles()
        verify(view.showProfileDeletePreview("work"))
        view.closeProfiles()
        owner.profile_preview_text="";owner.profile_preview_pending=false
        wait(150)
        compare(view.profileManagerVisible,false);compare(view.profileDeletePreviewVisible,false)
    }
    function test_manager_fills_its_window_data() {
        var rows=[]
        for (var surface of ["profiles","sessions"])
            for (var size of [{width:900,height:700,font:10},{width:640,height:480,font:16},{width:390,height:600,font:20}])
                rows.push({tag:surface+"-"+size.width,surface:surface,width:size.width,height:size.height,font:size.font})
        return rows
    }
    function test_manager_fills_its_window(row) {
        host.width=row.width;host.height=row.height;host.font.pointSize=row.font
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner)
        if(row.surface==="profiles")view.showProfiles();else view.showSessions()
        verify(waitForPolish(host))
        var panel=findChild(view,row.surface==="profiles"?"profileList":"sessionList")
        while(panel && panel.objectName!=="ferricModalPanel")panel=panel.parent
        verify(panel)
        verify(panel.width>host.width/2,"Manager panel must occupy its window")
        verify(panel.height>host.height/2,"Manager body must have usable height")
        compare(panel.parent.width,host.contentItem.width)
        compare(panel.parent.height,host.contentItem.height)
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/workspace-"+row.tag+".png")
        host.width=900;host.height=700;host.font.pointSize=10
    }

}
