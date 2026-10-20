import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtTest
import QtWebEngine
import "../../qml/components"
import "../../qml/scripts/ChromePresentation.js" as ChromePresentation
TestCase {
    id: test
    name: "PromptPolish"
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
        property bool diagnosticsVisible: false
        property string displayScaleLabel: "100%"
        property string chromeFontFamily: font.family
        property bool reducedMotionActive: false
        property string themeContrastWarning: ""
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
        property string pageDialogTitle: "<b>A page message</b>"
        property string pageDialogMessage: "Review this request"
        property string pageDialogOrigin: "https://example.test"
        property string dialogKind: "confirm"
        property bool pageDialogSuppressChecked: false
        property var pendingPageDialogRequest: ({})
        property int acceptedDialogs: 0
        property int rejectedDialogs: 0
        function pageDialogKind() { return dialogKind }
        function acceptPageDialog() { acceptedDialogs++ }
        function rejectPageDialog() { rejectedDialogs++ }
        property int pendingWebAuthState: WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount
        property string pendingWebAuthRelyingParty: "example.test"
        property string webAuthStatusText: "Choose an account"
        property var pendingWebAuthUserNames: ["<b>literal account</b>","Another account"]
        property var pendingWebAuthRequest: ({pinRequest:{remainingAttempts:3,minPinLength:4}})
        property string pendingClientCertificateHost: "example.test"
        property var pendingClientCertificateOptions: [{index:0,subject:"<b>literal identity</b>",issuer:"Example issuer",selfSigned:false}]
        property bool contextRouteVisible: false
        property var contextRouteHost: host
        property var contextRouteUi: ({context_route_context:"Research",context_route_profile:"work",context_route_id:"internal-rule-id",context_route_behavior:"prompt",context_route_url:"https://example.test"})
        function acceptContextRoute() {}
        function dismissContextRoute() { contextRouteVisible=false }
        property bool rendererFailureVisible: false
        property var rendererFailureHost: host
        property var rendererFailureView: ({})
        property int rendererFailureCount: 1
        property string rendererFailureSafeUrl: "https://example.test"
        property string rendererFailureReason: "<b>engine reason</b>"
        property int rendererFailureExitCode: 12345
        property bool softwareRendering: true
        function activeWebView() { return rendererFailureView }
        property bool permissionPromptVisible: false
        property var pendingPermissionHost: host
        property string pendingPermissionOrigin: "https://example.test"
        property string pendingPermissionName: "camera"
        property string pendingPermissionPolicyNotice: ""
        property bool pendingPermissionPrivate: false
        property int pendingPermissionGroupCount: 1
        FerricRequestPresentationController { id: permissionPresentation; browserWindow: host }
        function permissionDisplayName(name) { return permissionPresentation.permissionDisplayName(name) }
        function permissionCanRememberForSite(origin,name) { return permissionPresentation.permissionCanRememberForSite(origin,name) }
        function permissionCanRemember(origin,name) { return permissionPresentation.permissionCanRemember(origin,name) }
        property bool contextMoveVisible: false
        property string contextMoveTabId: ""
        property var contextMoveChoices: []
        property string movedContext: ""
        function chooseContextMove(name) { movedContext=name }
        function restoreOverlayFocus() {}
        property string pendingCertificateErrorHost: "example.test"
        property string pendingCertificateErrorDescription: "<b>literal certificate error</b>"
        property bool desktopMediaPromptVisible: false
        property var pendingDesktopMediaHost: host
        property string desktopMediaOrigin: "https://example.test"
        property var pendingDesktopMediaRequest: ({screensModel:screenSources,windowsModel:windowSources})
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

    Component {
        id: fixture
        FerricCommandDialog {
            browserWindow: host
            title: "A page request"
            message: "Please review the request."
            actions: [{id:"accept",key:"y",label:"Continue",shortcuts:["Y"]},
                      {id:"disabled",key:"x",label:"Unavailable",enabled:false},
                      {id:"cancel",key:"n",label:"Cancel",safe:true}]
        }
    }
    Component {
        id: formFixture
        FerricCommandDialog {
            browserWindow: host
            actions: [{id:"cancel",label:"Cancel",safe:true},{id:"accept",key:"y",label:"Continue",shortcuts:["Y"]}]
            ColumnLayout {
                anchors.fill: parent
                TextField { objectName: "draftField"; Layout.fillWidth: true }
                CheckBox { objectName: "reviewConsent"; text: "Review choice" }
                Button { objectName: "inspectionButton"; text: "Inspect"; property int activations: 0; onClicked: activations++ }
            }
        }
    }
    ListModel { id: fixtureProfiles; ListElement { name: "work"; label: "<b>Work profile</b>" } }
    Component { id: transferFixture; FerricPrivateHistoryTransfer { browserWindow: host; profilesModel: fixtureProfiles; profilesLoading: false } }
    ListModel { id: screenSources; ListElement { display: "Display 1" } }
    ListModel { id: windowSources; ListElement { display: "<b>Editor window</b>" } }
    QtObject {
        id: externalUi
        property bool external_navigation_visible: false
        property string external_navigation_scheme: "mailto"
        property string external_navigation_uri: "mailto:person@example.test"
        function confirm_external_navigation() { return false }
        function cancel_external_navigation() { external_navigation_visible=false }
    }
    Component { id: externalFixture; FerricExternalNavigationDialog { browserWindow: host; browserUi: externalUi } }
    Component { id: profileNoticeFixture; FerricProfileNotice {
        anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
        browserWindow: host; hostWindow: host
        noticeText: "Some settings couldn't be loaded."
    } }
    Component { id: copyNoticeFixture; FerricCopyNotice {
        browserWindow: host; hostWindow: host; noticeVisible: true
        copiedText: "<b>Literal copied content</b>".repeat(50); sensitive: false
        property int repeatRequests: 0
        onCopyAgainRequested: repeatRequests += 1
        onDismissalRequested: noticeVisible=false
    } }
    Component { id: diagnosticsFixture; FerricDiagnostics { browserWindow: host; previewText: '{"profile_startup":{"status":"degraded"}}'; summaryRows: ["Some settings could not be loaded"]; reportAvailable: true; startupDetails: "Settings: <b>/private/config.toml</b>\n".repeat(50); onCloseRequested: host.diagnosticsVisible=false } }
    Component { id: routeFixture; FerricContextRouteDialog { browserWindow: host } }
    Component { id: crashFixture; FerricRendererFailurePrompt { browserWindow: host } }
    Component { id: permissionFixture; FerricPermissionPrompt { browserWindow: host } }
    Component { id: moveFixture; FerricContextMoveDialog { browserWindow: host } }
    Component { id: authFixture; FerricWebAuthPrompt { browserWindow: host } }
    Component { id: certificateFixture; FerricCertificatePrompts { browserWindow: host } }
    Component { id: sharingFixture; FerricDesktopMediaPrompt { browserWindow: host } }
    Component { id: pageFixture; FerricPageDialog { browserWindow: host } }
    Component { id: spyFixture; SignalSpy {} }
    Component { id: outsideFixture; TextField {} }
    Component { id: loaderFixture; Loader { sourceComponent: fixture } }

    function init() {
        failOnWarning(/invalid context|TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width=900; host.height=700; host.font.pointSize=10
        host.pendingWebAuthState=WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount
        host.pendingWebAuthRelyingParty="example.test"
        host.desktopMediaPromptVisible=false
        host.diagnosticsVisible=false
        host.contextRouteVisible=false;host.rendererFailureVisible=false;externalUi.external_navigation_visible=false
        host.contextRouteHost=host
        host.contextRouteUi={context_route_context:"Research",context_route_profile:"work",context_route_id:"internal-rule-id",context_route_behavior:"prompt",context_route_url:"https://example.test"}
        host.permissionPromptVisible=false
        host.pendingPermissionOrigin="https://example.test"
        host.pendingPermissionName="camera"
        host.pendingPermissionPolicyNotice=""
        host.pendingPermissionPrivate=false
        host.contextMoveVisible=false;host.movedContext=""
        host.privateHistoryTransferVisible=false
        host.privateHistoryTransferTitle="<b>A private page</b>"
        host.privateHistoryTransferUrl="https://example.test/"
        host.dialogKind="confirm"; host.acceptedDialogs=0; host.rejectedDialogs=0
        host.pageDialogMessage="Review this request"; host.pageDialogSuppressChecked=false
        host.requestActivate(); tryCompare(host,"active",true)
    }
    function create(component) {
        var view=createTemporaryObject(component,host.contentItem)
        verify(view); verify(waitForPolish(host)); return view
    }
    function inside(item, panel) {
        var point=item.mapToItem(panel,0,0)
        verify(point.x>=-1 && point.y>=-1,"Control starts outside its viewport")
        verify(point.x+item.width<=panel.width+1 && point.y+item.height<=panel.height+1,"Control "+item.objectName+" ends outside "+panel.objectName+": "+JSON.stringify({x:point.x,y:point.y,width:item.width,height:item.height,panelWidth:panel.width,panelHeight:panel.height}))
    }
    function waitInside(item, panel) {
        tryVerify(function(){var p=item.mapToItem(panel,0,0);return p.y>=-1 && p.y+item.height<=panel.height+1})
        inside(item,panel)
    }
    function capture(name) {
        if(testArtifactDirectory.length>0) grabImage(host.contentItem).save(testArtifactDirectory+"/"+name+".png")
    }
    function sizes() { return [{tag:"wide",width:900,height:700,font:10},
                              {tag:"compact",width:640,height:480,font:16},
                              {tag:"narrow-large",width:390,height:600,font:20}] }
    function resize(data) { host.width=data.width;host.height=data.height;host.font.pointSize=data.font }
    function test_long_content_data() { return sizes() }
    function test_long_content(data) {
        resize(data)
        var view=create(fixture)
        view.title="<b>Literal title</b> " + "long-title-".repeat(80)
        view.message="<b>Literal message</b> " + "longaddresswithoutbreaks".repeat(200)
        verify(waitForPolish(host))
        var header=findChild(view,"dialogHeaderScroll")
        verify(header.contentHeight>header.height)
        var scrollbar=header.ScrollBar.vertical
        verify(scrollbar.mapToItem(header,0,0).x>header.width/2)
        compare(findChild(view,"dialogTitleLabel").textFormat,Text.PlainText)
        var message=findChild(view,"dialogMessageLabel")
        compare(message.text,view.message); compare(message.textFormat,TextEdit.PlainText)
        verify(message.height>=message.implicitHeight-1)
        var cancel=findChild(view,"commandDialogAction-cancel")
        tryCompare(cancel,"activeFocus",true)
        var choices=findChild(view,"commandDialogActionScroll")
        inside(choices,view.panelItem); waitInside(cancel,choices)
        mouseWheel(header,header.width/2,header.height/2,0,-240)
        tryVerify(function(){return header.contentItem.contentY>0})
        inside(cancel,choices)
        capture("prompt-"+data.tag)
    }
    function test_many_choices_scroll_and_skip_disabled_rows() {
        host.width=390;host.height=600;host.font.pointSize=16
        var view=create(fixture)
        var actions=[{id:"cancel",label:"Keep current settings",safe:true},{id:"disabled",label:"Unavailable",enabled:false}]
        for(var i=0;i<10;i++)actions.push({id:"choice"+i,label:"A long literal choice <b>"+i+"</b> with a description"})
        view.actions=actions
        var cancel=findChild(view,"commandDialogAction-cancel")
        tryCompare(cancel,"activeFocus",true)
        keyClick(Qt.Key_Down); compare(view.currentIndex,2)
        for(var j=0;j<9;j++)keyClick(Qt.Key_Down)
        compare(view.currentIndex,11)
        var last=findChild(view,"commandDialogAction-choice9")
        tryCompare(last,"activeFocus",true)
        verify(waitForPolish(host))
        // Focus scrolling is deferred until after layout has settled.
        waitInside(last,findChild(view,"commandDialogActionScroll"))
        var spy=createTemporaryObject(spyFixture,view,{target:view,signalName:"actionRequested"})
        keyClick(Qt.Key_Return); compare(spy.count,1); compare(spy.signalArguments[0][0],"choice9")
        keyClick(Qt.Key_Return); compare(spy.count,1)
    }
    function test_detail_controls_keep_their_own_keys() {
        var view=create(formFixture)
        var spy=createTemporaryObject(spyFixture,view,{target:view,signalName:"actionRequested"})
        var input=findChild(view,"draftField"); input.forceActiveFocus();tryCompare(input,"activeFocus",true)
        keyClick("j");keyClick("k");keyClick("y");compare(input.text,"jky");compare(spy.count,0)
        keyClick(Qt.Key_Left);compare(view.currentIndex,0)
        var checkbox=findChild(view,"reviewConsent");checkbox.forceActiveFocus();keyClick(Qt.Key_Return)
        compare(checkbox.checked,true);compare(spy.count,0)
        var button=findChild(view,"inspectionButton");button.forceActiveFocus();keyClick(Qt.Key_Return)
        compare(button.activations,1);compare(spy.count,0)
        keyClick(Qt.Key_Escape);compare(spy.count,1);compare(spy.signalArguments[0][0],"cancel")
    }
    function test_keyboard_can_read_to_the_end_of_a_long_message() {
        host.width=390;host.height=600
        var view=create(fixture)
        view.message="A line of literal page text.\n".repeat(200)
        var message=findChild(view,"dialogMessageLabel")
        message.forceActiveFocus();tryCompare(message,"activeFocus",true)
        keyClick(Qt.Key_End,Qt.ControlModifier)
        tryCompare(message,"cursorPosition",message.length)
        var header=findChild(view,"dialogHeaderScroll")
        tryVerify(function(){var cursor=message.cursorRectangle;var point=message.mapToItem(header,cursor.x,cursor.y);return point.y>=-1 && point.y+cursor.height<=header.height+1})
    }
    function test_message_selection_does_not_confirm() {
        var view=create(fixture)
        var spy=createTemporaryObject(spyFixture,view,{target:view,signalName:"actionRequested"})
        var message=findChild(view,"dialogMessageLabel");message.forceActiveFocus();message.selectAll()
        keyClick(Qt.Key_Return);compare(spy.count,0)
        keyClick(Qt.Key_Escape);compare(spy.count,1);compare(spy.signalArguments[0][0],"cancel")
    }
    function test_private_transfer_data() { return sizes() }
    function test_private_transfer(data) {
        resize(data);host.privateHistoryTransferVisible=true
        host.privateHistoryTransferUrl="https://example.test/"+"long-path".repeat(500)
        var view=create(transferFixture)
        var cancelled=createTemporaryObject(spyFixture,view,{target:view,signalName:"cancelRequested"})
        var bookmarked=createTemporaryObject(spyFixture,view,{target:view,signalName:"bookmarkRequested"})
        var cancel=findChild(view,"privateTransferCancel");tryCompare(cancel,"activeFocus",true)
        inside(cancel,view.panelItem)
        compare(findChild(view,"privateTransferAddress").text,host.privateHistoryTransferUrl)
        var bookmark=findChild(view,"privateTransferBookmark");verify(bookmark)
        bookmark.forceActiveFocus();tryCompare(bookmark,"activeFocus",true)
        waitInside(bookmark,findChild(view,"privateTransferProfiles"))
        inside(bookmark,view.panelItem)
        keyClick(Qt.Key_Return);compare(bookmarked.count,1);compare(bookmarked.signalArguments[0][0],"work")
        cancel.forceActiveFocus();keyClick(Qt.Key_Return);compare(cancelled.count,1)
        capture("private-transfer-"+data.tag)
    }
    function test_page_safe_default_data() { return [{tag:"confirm",kind:"confirm"},{tag:"leave-page",kind:"beforeunload"}] }
    function test_page_safe_default(data) {
        host.dialogKind=data.kind
        var view=create(pageFixture);view.open()
        var cancel=findChild(view,"pageDialogCancel");tryCompare(cancel,"activeFocus",true)
        keyClick(Qt.Key_Return);compare(host.rejectedDialogs,1);compare(host.acceptedDialogs,0)
        var accept=findChild(view,"pageDialogAccept");accept.forceActiveFocus();keyClick(Qt.Key_Space)
        compare(host.acceptedDialogs,1)
    }
    function test_short_page_request_uses_its_content_height() {
        host.dialogKind="confirm"
        var view=create(pageFixture);view.open();verify(waitForPolish(host))
        verify(view.panelItem.height<host.height*0.6,"A short request should not leave a tall empty panel")
        verify(view.headerItem.contentLayout.implicitHeight<=view.headerItem.availableHeight+1,
               "A short page heading and origin should fit without a header scrollbar")
        var cancel=findChild(view,"pageDialogCancel")
        var position=cancel.mapToItem(view.panelItem,0,0)
        verify(position.y+cancel.height<view.panelItem.height)
    }
    function test_page_form_data() { return sizes() }
    function test_page_form(data) {
        resize(data);host.dialogKind="authentication"
        host.pageDialogMessage="<b>Literal authentication message</b> ".repeat(100)
        var view=create(pageFixture);view.open();verify(waitForPolish(host))
        var username=findChild(view,"pageDialogUsername");tryCompare(username,"activeFocus",true)
        keyClick("j");keyClick("k");compare(view.usernameText,"jk")
        var checkbox=findChild(view,"pageDialogSuppress");checkbox.forceActiveFocus();keyClick(Qt.Key_Return)
        compare(host.pageDialogSuppressChecked,true);compare(host.acceptedDialogs,0)
        var cancel=findChild(view,"pageDialogCancel");cancel.forceActiveFocus()
        tryVerify(function(){var p=cancel.mapToItem(view.panelItem,0,0);return p.y+cancel.height<view.panelItem.height})
        keyClick(Qt.Key_Return);compare(host.rejectedDialogs,1);compare(host.acceptedDialogs,0)
        username.forceActiveFocus();keyClick(Qt.Key_Return);compare(host.acceptedDialogs,1)
        capture("page-form-"+data.tag)
    }
    function test_account_cancel_is_safe_and_navigation_is_deliberate() {
        var view=create(authFixture);view.open()
        var cancelled=createTemporaryObject(spyFixture,view,{target:view,signalName:"cancelled"})
        var selected=createTemporaryObject(spyFixture,view,{target:view,signalName:"accountSelected"})
        var cancel=findChild(view,"webAuthCancel");tryCompare(cancel,"activeFocus",true)
        var accounts=findChild(view,"webAuthAccounts")
        tryVerify(function(){return accounts.currentItem!==null})
        compare(accounts.currentItem.selected,false);compare(accounts.currentItem.keyHint,"")
        keyClick(Qt.Key_Return);compare(cancelled.count,1);compare(selected.count,0)
        keyClick(Qt.Key_Down);keyClick(Qt.Key_Return)
        compare(selected.count,1);compare(selected.signalArguments[0][0],"Another account")
        host.pendingWebAuthState=WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin
        var pin=findChild(view,"webAuthPin");tryCompare(pin,"activeFocus",true)
        view.pin="1234"
        var submitted=createTemporaryObject(spyFixture,view,{target:view,signalName:"pinSubmitted"})
        keyClick(Qt.Key_Return);compare(submitted.count,1)
        view.close();compare(view.pin,"")
    }
    function test_security_prompts_data() { return sizes() }
    function test_security_prompts(data) {
        resize(data)
        host.pendingWebAuthRelyingParty="<b>literal party</b>"+"longhost".repeat(100)
        var view=create(authFixture);view.open();verify(waitForPolish(host))
        var cancel=findChild(view,"webAuthCancel");tryCompare(cancel,"activeFocus",true)
        waitInside(cancel,findChild(view,"ferricModalPanel"))
        capture("passkey-"+data.tag);view.close()
        var certificates=create(certificateFixture);certificates.openCertificateError()
        var back=findChild(certificates,"commandDialogAction-back");tryCompare(back,"activeFocus",true)
        compare(findChild(certificates,"certificateErrorDescription").textFormat,TextEdit.PlainText)
        waitInside(back,findChild(certificates,"commandDialogActionScroll"))
        certificates.closeCertificateError();certificates.openClientCertificate()
        var rejected=createTemporaryObject(spyFixture,certificates,{target:certificates,signalName:"clientCertificateRejected"})
        var accepted=createTemporaryObject(spyFixture,certificates,{target:certificates,signalName:"clientCertificateAccepted"})
        var cancelCertificate=findChild(certificates,"clientCertificateCancel");tryCompare(cancelCertificate,"activeFocus",true)
        inside(cancelCertificate,findChild(certificates,"clientCertificateChoices").parent.parent)
        keyClick(Qt.Key_Return);compare(rejected.count,1);compare(accepted.count,0)
        keyClick(Qt.Key_Down);keyClick(Qt.Key_Return);compare(accepted.count,1);compare(accepted.signalArguments[0][0],0)
        certificates.closeClientCertificate()
    }
    function test_permission_data() { return sizes() }
    function test_permission_context_and_actions_data() {
        var rows=[]
        var permissions=["camera","microphone","camera-and-microphone","screen-capture",
                         "notifications","geolocation","clipboard","local-fonts"]
        for(var size of sizes())for(var permission of permissions)for(var privateProfile of [false,true])
            rows.push(Object.assign({},size,{tag:size.tag+"-"+permission+(privateProfile?"-private":"-regular"),
                                           permission:permission,privateProfile:privateProfile}))
        return rows
    }
    function test_permission_context_and_actions(data) {
        resize(data)
        host.pendingPermissionName=data.permission
        host.pendingPermissionPrivate=data.privateProfile
        host.permissionPromptVisible=true
        var view=create(permissionFixture)
        var decided=createTemporaryObject(spyFixture,view,{target:view,signalName:"decisionRequested"})
        var deny=findChild(view,"commandDialogAction-deny");tryCompare(deny,"activeFocus",true)
        var message=findChild(view,"dialogMessageLabel")
        inside(message,view.headerItem)
        compare(findChild(deny,"commandActionLabel").lineCount,1,"Deny must remain a readable single-line action")
        var persistence=findChild(view,"permissionPersistenceLabel")
        if(data.privateProfile)compare(persistence.text,"Permissions are not saved in private profiles.")
        else if(data.permission==="screen-capture")compare(persistence.text,"This permission cannot be saved for future visits.")
        capture("permission-context-"+data.tag)
        var choices=findChild(view,"commandDialogActionScroll")
        for(var index=0;index<view.actions.length;++index) {
            var action=findChild(view,"commandDialogAction-"+view.actions[index].id)
            if(index>0)keyClick(Qt.Key_Down)
            tryCompare(action,"activeFocus",true)
            waitInside(action,choices)
            var label=findChild(action,"commandActionLabel")
            verify(label.height>=label.implicitHeight-1,"Action labels must not be vertically clipped")
        }
        keyClick(Qt.Key_Escape)
        compare(decided.count,1);compare(decided.signalArguments[0][0],false)
    }
    function test_permission_without_rememberable_origin_data() {
        return [{tag:"file",origin:"file://"},{tag:"opaque",origin:"null"}]
    }
    function test_permission_without_rememberable_origin(data) {
        host.pendingPermissionOrigin=data.origin
        host.permissionPromptVisible=true
        var view=create(permissionFixture)
        compare(view.actions.length,2)
        compare(view.actions[0].id,"deny")
        compare(view.actions[1].id,"once")
    }
    function test_permission_choices_fit_when_window_has_room_data() {
        return [{tag:"normal",unavailable:false},{tag:"saved-policy-unavailable",unavailable:true}]
    }
    function test_permission_choices_fit_when_window_has_room(data) {
        resize({width:900,height:700,font:12})
        host.pendingPermissionName="notifications"
        host.permissionPromptVisible=true
        host.pendingPermissionPolicyNotice=data.unavailable
            ? "Saved permissions couldn't be checked. Review this request before allowing access." : ""
        var view=create(permissionFixture)
        var choices=findChild(view,"commandDialogActionScroll")
        compare(view.actions.length,4)
        verify(waitForPolish(host))
        // Keep the initial safe choice focused: no scrolling or selection is
        // needed to expose the remaining choices at this ordinary window size.
        for(var action of view.actions)
            waitInside(findChild(view,"commandDialogAction-"+action.id),choices.contentItem)
    }
    function test_permission_policy_unavailable_notice() {
        host.permissionPromptVisible=true
        host.pendingPermissionPolicyNotice="Saved permissions couldn't be checked. Review this request before allowing access."
        var view=create(permissionFixture)
        var notice=findChild(view,"permissionPolicyNotice")
        verify(notice.visible)
        compare(notice.text,host.pendingPermissionPolicyNotice)
        compare(notice.textFormat,Text.PlainText)
    }
    function test_permission(data) {
        resize(data);host.permissionPromptVisible=true
        host.pendingPermissionOrigin="https://example.test/"+"long-origin".repeat(200)
        var view=create(permissionFixture)
        var decided=createTemporaryObject(spyFixture,view,{target:view,signalName:"decisionRequested"})
        var deny=findChild(view,"commandDialogAction-deny");tryCompare(deny,"activeFocus",true)
        waitInside(deny,findChild(view,"commandDialogActionScroll"))
        keyClick(Qt.Key_Return);compare(decided.count,1);compare(decided.signalArguments[0][0],false)
        view.visible=false;view.visible=true
        tryCompare(deny,"activeFocus",true)
        keyClick(Qt.Key_Down);keyClick(Qt.Key_Down);keyClick(Qt.Key_Return)
        compare(decided.count,2);compare(decided.signalArguments[1][0],true);compare(decided.signalArguments[1][1],"")
        capture("permission-"+data.tag)
    }
    function test_sharing_data() { return sizes() }
    function test_sharing(data) {
        resize(data);host.desktopMediaPromptVisible=true
        var view=create(sharingFixture)
        var cancelled=createTemporaryObject(spyFixture,view,{target:view,signalName:"cancellationRequested"})
        var windows=createTemporaryObject(spyFixture,view,{target:view,signalName:"windowRequested"})
        var screens=createTemporaryObject(spyFixture,view,{target:view,signalName:"screenRequested"})
        var cancel=findChild(view,"sharingCancel");tryCompare(cancel,"activeFocus",true)
        waitInside(cancel,view.panelItem)
        var initialSources=findChild(view,"sharingSources")
        tryVerify(function(){return initialSources.currentItem!==null})
        compare(initialSources.currentItem.selected,false);compare(initialSources.currentItem.keyHint,"")
        keyClick(Qt.Key_Return);compare(cancelled.count,1);compare(screens.count,0)
        var categories=findChild(view,"sharingWindows");categories.forceActiveFocus();keyClick(Qt.Key_Return)
        compare(view.showingWindows,true);compare(windows.count,0)
        var sources=findChild(view,"sharingSources");tryVerify(function(){return sources.currentItem!==null})
        compare(sources.currentItem.actionLabel,"<b>Editor window</b>")
        keyClick(Qt.Key_Return);compare(windows.count,1);compare(windows.signalArguments[0][0],0)
        capture("sharing-"+data.tag)
    }
    function test_external_address_is_complete_and_literal_data() { return sizes() }
    function test_external_address_is_complete_and_literal(data) {
        resize(data);externalUi.external_navigation_visible=true
        externalUi.external_navigation_uri="mailto:person@example.test?subject=<b>literal</b>"+"long-subject".repeat(400)
        var view=create(externalFixture)
        var address=findChild(view,"externalNavigationAddress")
        compare(address.text,externalUi.external_navigation_uri);compare(address.textFormat,TextEdit.PlainText)
        var cancel=findChild(view,"commandDialogAction-cancel");tryCompare(cancel,"activeFocus",true)
        waitInside(cancel,findChild(view,"commandDialogActionScroll"))
        address.forceActiveFocus();address.selectAll();keyClick(Qt.Key_Return)
        compare(externalUi.external_navigation_visible,true)
        keyClick(Qt.Key_Escape);compare(externalUi.external_navigation_visible,false)
    }
    function test_route_and_recovery_details_data() { return sizes() }
    function test_scale_metadata_uses_correct_density_units() {
        compare(ChromePresentation.displayScaleLabel(1,96/25.4), "100% (96 logical dpi)")
        compare(ChromePresentation.displayScaleLabel(2,144/25.4), "200% (144 logical dpi)")
        compare(ChromePresentation.displayScaleLabel(NaN,0), "Scale unavailable")
        compare(ChromePresentation.reducedMotion("on","available",false),true)
        compare(ChromePresentation.reducedMotion("off","available",true),false)
        compare(ChromePresentation.reducedMotion("system","unknown",false),true)
        compare(ChromePresentation.reducedMotion("system","available",false),false)
    }
    function test_profile_notice_survives_navigation_and_bounds_content_data() {
        return [{tag:"normal",width:900,height:700,font:10}, {tag:"compact",width:390,height:480,font:16},
                {tag:"large-text",width:640,height:500,font:20}, {tag:"largest-text",width:900,height:700,font:60}]
    }
    function test_profile_notice_survives_navigation_and_bounds_content(data) {
        resize(data)
        var view=create(profileNoticeFixture)
        verify(view.visible);verify(view.height>0 && view.height<=host.contentItem.height*0.4+1)
        var message=findChild(view,"profileHealthMessage")
        compare(message.textFormat,Text.PlainText)
        view.noticeText="<b>Literal settings failure</b> ".repeat(40)
        verify(waitForPolish(host))
        verify(view.height<=host.contentItem.height*0.4+1)
        var action=findChild(view,"profileHealthDetails")
        var requested=createTemporaryObject(spyFixture,view,{target:view,signalName:"detailsRequested"})
        action.forceActiveFocus()
        tryCompare(view,"inputActive",true)
        waitInside(action,view)
        keyClick(Qt.Key_Return);compare(requested.count,1)
        keyClick(Qt.Key_Space);compare(requested.count,2)
        capture("profile-notice-"+data.tag)
        view.noticeText=""
        tryCompare(view,"visible",false);tryCompare(view,"height",0);tryCompare(view,"inputActive",false)
    }
    function test_copy_feedback_names_its_destination() {
        var view=create(copyNoticeFixture)
        var title=findChild(view,"copyNoticeTitle")
        compare(title.text,"Copied to clipboard")
        view.primarySelection=true
        compare(title.text,"Copied to primary selection")
    }
    function test_copy_feedback_controls_remain_reachable_data() {
        return [{tag:"normal",width:900,height:700,font:10},
                {tag:"compact",width:390,height:480,font:10},
                {tag:"large-text",width:640,height:500,font:20},
                {tag:"largest-text",width:390,height:480,font:60}]
    }
    function test_copy_feedback_controls_remain_reachable(data) {
        host.width=data.width;host.height=data.height;host.font.pointSize=data.font
        var view=create(copyNoticeFixture)
        var text=findChild(view,"copyNoticeText")
        compare(text.textFormat,Text.PlainText);verify(text.text.indexOf("<b>")>=0)
        var again=findChild(view,"copyNoticeAgain")
        again.forceActiveFocus();tryCompare(again,"activeFocus",true);tryCompare(view,"inputActive",true)
        var body=findChild(view,"copyNoticeBody")
        body.scheduleFocus();verify(waitForPolish(host));wait(30)
        var point=again.mapToItem(body.contentItem,0,0)
        verify(point.x>=-1 && point.x+again.width<=body.contentItem.width+1,"Copy again must fit the viewport width")
        verify(point.y>=-1 && point.y+again.height<=body.contentItem.height+1,"Focused action must remain fully reachable")
        keyClick(Qt.Key_Return);compare(view.repeatRequests,1)
        capture("copy-feedback-"+data.tag)
        keyClick(Qt.Key_Escape);tryCompare(view,"visible",false);tryCompare(view,"inputActive",false)
    }
    function test_diagnostics_summary_keeps_natural_row_spacing() {
        host.diagnosticsVisible=true
        var view=create(diagnosticsFixture)
        var body=findChild(view,"diagnosticsBody")
        var content=findChild(view,"diagnosticsContent")
        verify(waitForPolish(host))
        verify(content.height <= content.implicitHeight + 1)
        verify(content.y <= 1)
        var originalHeight=content.height
        host.height += 300
        verify(waitForPolish(host))
        verify(Math.abs(content.height-originalHeight)<=1,"Extra window height must not spread the summary rows")
        verify(content.height < body.height)
    }
    function test_startup_details_are_local_literal_and_collapsed_data() {
        return [{tag:"normal",width:900,height:700,font:10}, {tag:"compact",width:390,height:480,font:10}, {tag:"large-text",width:640,height:500,font:16}]
    }
    function test_startup_details_are_local_literal_and_collapsed(data) {
        host.width=data.width;host.height=data.height;host.font.pointSize=data.font
        host.diagnosticsVisible=true
        var view=create(diagnosticsFixture)
        var detail=findChild(view,"diagnosticsStartupText")
        tryCompare(detail,"visible",false)
        var toggle=findChild(view,"diagnosticsStartupToggle")
        verify(toggle.visible)
        toggle.clicked()
        tryCompare(detail,"visible",true)
        compare(detail.textFormat,TextEdit.PlainText)
        verify(detail.text.indexOf("<b>/private/config.toml</b>")>=0)
        verify(view.previewText.indexOf("/private/")<0)
        detail.forceActiveFocus();keyClick(Qt.Key_End,Qt.ControlModifier)
        verify(waitForPolish(host))
        capture("startup-details-"+data.tag)
        keyClick(Qt.Key_Escape)
        tryCompare(host,"diagnosticsVisible",false)
    }
    function test_short_route_fits_its_content_data() {
        return [{tag:"default",width:900,height:700,font:10},
                {tag:"large-text",width:1200,height:900,font:16},
                {tag:"larger-text",width:1400,height:1100,font:20}]
    }
    function test_short_route_fits_its_content(data) {
        resize(data)
        host.contextRouteVisible=true
        var route=create(routeFixture)
        verify(waitForPolish(host))
        verify(route.panelItem.height<host.height*0.65,"A short route choice should not occupy a tall empty card")
        var detailScroll=findChild(route,"commandDialogDetailScroll")
        verify(detailScroll.contentLayout.implicitHeight<=detailScroll.availableHeight+1,
               JSON.stringify({contents:detailScroll.contentLayout.implicitHeight,viewport:detailScroll.availableHeight,
                               padding:detailScroll.topPadding+detailScroll.bottomPadding,panel:route.panelItem.height}))
        var safe=findChild(route,"commandDialogAction-normal")
        tryCompare(safe,"activeFocus",true)
        waitInside(safe,findChild(route,"commandDialogActionScroll"))
        var heightBefore=route.panelItem.height
        capture("context-route-short-"+data.tag)
        var button=findChild(route,"contextRouteDetailsButton")
        button.forceActiveFocus();keyClick(Qt.Key_Return)
        verify(waitForPolish(host))
        verify(route.panelItem.height>=heightBefore)
        keyClick(Qt.Key_Return)
        tryVerify(function(){return Math.abs(route.panelItem.height-heightBefore)<2})
        keyClick(Qt.Key_Escape);compare(host.contextRouteVisible,false)
    }
    function test_route_and_recovery_details(data) {
        resize(data);host.contextRouteVisible=true
        host.contextRouteUi={context_route_context:"<b>Research</b>",context_route_profile:"work",context_route_id:"internal-rule-id",context_route_behavior:"prompt",context_route_url:"https://example.test/"+"long-address".repeat(200)}
        var route=create(routeFixture)
        var address=findChild(route,"contextRouteAddress")
        compare(address.text,host.contextRouteUi.context_route_url);compare(address.textFormat,TextEdit.PlainText)
        var details=findChild(route,"contextRouteDetails");verify(!details.visible)
        var button=findChild(route,"contextRouteDetailsButton");button.forceActiveFocus();keyClick(Qt.Key_Return)
        verify(button.width<=findChild(route,"commandDialogDetailScroll").width)
        verify(button.contentItem.height>=button.contentItem.implicitHeight-1)
        compare(details.visible,true);verify(details.text.indexOf("internal-rule-id")>=0)
        compare(host.contextRouteVisible,true)
        capture("context-route-details-"+data.tag)
        keyClick(Qt.Key_Escape);compare(host.contextRouteVisible,false)
        host.rendererFailureVisible=true
        var crash=create(crashFixture)
        var technical=findChild(crash,"rendererFailureDetails");verify(!technical.visible)
        var more=findChild(crash,"rendererFailureDetailsButton");more.forceActiveFocus();keyClick(Qt.Key_Return)
        compare(technical.visible,true);compare(technical.textFormat,TextEdit.PlainText)
        verify(more.width<=findChild(crash,"commandDialogDetailScroll").width)
        verify(more.contentItem.height>=more.contentItem.implicitHeight-1)
        verify(technical.text.indexOf("12345")>=0)
        var dismiss=findChild(crash,"commandDialogAction-dismiss");dismiss.forceActiveFocus();tryCompare(dismiss,"activeFocus",true)
        waitInside(dismiss,findChild(crash,"commandDialogActionScroll"))
        capture("recovery-"+data.tag)
    }
    function test_compact_keyboard_reference_is_available_without_losing_actions() {
        host.width=390;host.height=600;host.font.pointSize=16
        var view=create(fixture)
        var spy=createTemporaryObject(spyFixture,view,{target:view,signalName:"actionRequested"})
        var help=findChild(view,"dialogKeyboardHelp");verify(help.visible)
        help.forceActiveFocus();keyClick(Qt.Key_Return);compare(help.checked,true);compare(spy.count,0)
        var header=findChild(view,"dialogHeaderScroll")
        var instructions=findChild(view,"dialogKeyboardInstructions")
        compare(instructions.visible,true);compare(instructions.text,view.keyHelp)
        tryVerify(function(){return header.contentItem.contentY+header.availableHeight>=header.contentHeight-1})
        keyClick(Qt.Key_Escape);compare(spy.count,1)
        view.visible=false;view.visible=true;compare(help.checked,false)
    }
    function test_context_navigation_reaches_uninstantiated_choices() {
        var choices=[{name:"unavailable",label:"Unavailable context",available:false}]
        for(var i=0;i<100;i++)choices.push({name:"context"+i,label:"Context "+i,available:true})
        host.contextMoveChoices=choices;host.contextMoveVisible=true
        var view=create(moveFixture)
        var cancel=findChild(view,"contextMoveCancel");tryCompare(cancel,"activeFocus",true)
        // The last row is outside ListView's instantiated delegate range.
        keyClick(Qt.Key_Up);keyClick(Qt.Key_Return);compare(host.movedContext,"context99")
    }
    function test_dismiss_restores_focus() {
        var outside=create(outsideFixture);outside.forceActiveFocus();tryCompare(outside,"activeFocus",true)
        var view=create(fixture);tryVerify(function(){return view.containsActiveFocus})
        view.visible=false;tryCompare(outside,"activeFocus",true)
    }
    function test_unload_cancels_deferred_focus_work() {
        var loader=create(loaderFixture);verify(loader.item);loader.active=false;wait(20)
    }
}
