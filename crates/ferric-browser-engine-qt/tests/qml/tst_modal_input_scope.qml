import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "ModalInputScope"
    when: windowShown
    ApplicationWindow {
        id: host
        width: 900; height: 700; visible: true
        font.family: "monospace"; font.pointSize: 10
        property bool runtimeReleased: false
        function activeWebView() { return null }
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
        readonly property var modalInputScope: FerricModalInputController { hostWindow: host }
        property bool permissionPromptVisible: false
        property var pendingPermissionHost: host
        property string pendingPermissionOrigin: "https://permission.test"
        property string pendingPermissionName: "geolocation"
        property bool pendingPermissionPrivate: false
        property int pendingPermissionGroupCount: 1
        function permissionDisplayName(name) { return "use location" }
        function permissionCanRememberForSite(origin,name) { return false }
        function permissionCanRemember(origin,name) { return false }
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
            id: owner
            property var calls: []
            property bool accepted: true
            property string runtime_setting_error: "<b>literal failure details</b>"
            property string status_text: "<b>literal worker details</b>"
            property var settings_rows: [{key:"ui.font_size_pt",label:"Interface font size",type:"number",scope:"profile",apply:"immediate",value:"10",options:testEmptySettingsOptions}]
            property var userscript_names: []
            property var userscript_enabled_values: []
            property var userscript_page_world_values: []
            property var userscript_action_counts: []
            property string userscript_install_state: "idle"
            property bool userscript_inventory_ready: true
            property bool userscript_inventory_pending: false
            property string userscript_inventory_error: ""
            function refresh_userscript_inventory() {}
            function set_runtime_setting(key, value, temporary) {
                calls = calls.concat([{key:key, value:value, temporary:temporary}])
                return accepted
            }
            function unset_runtime_setting(key, temporary) {
                calls = calls.concat([{key:key, temporary:temporary}]); return accepted
            }
            function set_userscript_enabled(name, enabled) { return accepted }
            function remove_userscript(name) { return accepted }
        }
    }
    Component {
        id: workspaceFixture
        FerricSettingsWorkspace {
            browserWindow: host
            profileLabel: "Work <b>profile</b>"
            spellcheckStatus: "Spellcheck not checked"
        }
    }
    Component {
        id: incomingDialogFixture
        FerricPermissionPrompt {
            browserWindow: host
            hostWindow: host
            onDecisionRequested: host.permissionPromptVisible=false
        }
    }
    Component {
        id: genericDialogFixture
        FerricCommandDialog {
            browserWindow: host
            hostWindow: host
            visible: true
            commandText: "Browser panel"
            title: "Panel"
            cancelAction: "back"
            actions: [{id:"back",key:"b",label:"Back",safe:true}]
            onActionRequested: visible=false
        }
    }
    Component { id: layerFixture; Item { anchors.fill: parent } }
    function test_paint_ancestry_orders_input_above_a_higher_local_z() {
        var layer=createTemporaryObject(layerFixture,host.contentItem,{z:30})
        var child=createTemporaryObject(genericDialogFixture,layer,{stackingOrder:1000})
        var top=createTemporaryObject(genericDialogFixture,host.contentItem,{stackingOrder:80})
        verify(waitForPolish(host))
        verify(host.modalInputScope.activeSurface===top)
        compare(child.interactive,false);compare(top.interactive,true)
        keyClick(Qt.Key_Escape);tryCompare(top,"visible",false)
        compare(child.visible,true);verify(host.modalInputScope.activeSurface===child)
        keyClick(Qt.Key_Escape);tryCompare(child,"visible",false)
    }
    function test_paint_and_enabled_changes_update_the_input_owner() {
        var first=createTemporaryObject(genericDialogFixture,host.contentItem,{stackingOrder:60})
        var second=createTemporaryObject(genericDialogFixture,host.contentItem,{stackingOrder:80})
        verify(waitForPolish(host));verify(host.modalInputScope.activeSurface===second)
        first.stackingOrder=100
        tryVerify(function(){return host.modalInputScope.activeSurface===first})
        first.enabled=false
        tryVerify(function(){return host.modalInputScope.activeSurface===second})
        first.enabled=true
        tryVerify(function(){return host.modalInputScope.activeSurface===first})
    }
    function test_background_focus_work_cannot_steal_the_front_dialog() {
        var first=createTemporaryObject(genericDialogFixture,host.contentItem,{stackingOrder:60})
        var second=createTemporaryObject(genericDialogFixture,host.contentItem,{stackingOrder:80})
        verify(waitForPolish(host))
        var back=findChild(second,"commandDialogAction-back");verify(back)
        tryCompare(back,"activeFocus",true)
        first.focusInitialControl();wait(20)
        compare(back.activeFocus,true)
        first.visible=false;wait(20)
        compare(back.activeFocus,true);compare(second.visible,true)
        second.visible=false;wait(0)
        compare(host.modalInputScope.surfaces.length,0)
    }
    Component { id: focusControllerFixture; FerricFocusOverlayController { browserWindow: host } }
    Component { id: backgroundFieldFixture; TextField { text:"Background draft" } }
    function test_queued_focus_returns_keep_the_front_dialog_data() {
        return [{tag:"mode",mode:true},{tag:"overlay",mode:false}]
    }
    function test_queued_focus_returns_keep_the_front_dialog(data) {
        var field=createTemporaryObject(backgroundFieldFixture,host.contentItem)
        field.forceActiveFocus();tryCompare(field,"activeFocus",true)
        var controller=createTemporaryObject(focusControllerFixture,test)
        if(data.mode)controller.captureModeFocus()
        else controller.captureOverlayFocus(host,field)
        var dialog=createTemporaryObject(genericDialogFixture,host.contentItem,{stackingOrder:80})
        var back=findChild(dialog,"commandDialogAction-back");verify(back)
        tryCompare(back,"activeFocus",true)
        if(data.mode)controller.restoreModeFocus()
        else controller.restoreOverlayFocus(host)
        wait(20)
        compare(back.activeFocus,true)
        compare(field.text,"Background draft")
    }
    Component {
        id: anotherWindowFixture
        ApplicationWindow {
            id: another
            width:600; height:400; visible:true
            property bool runtimeReleased:false
            readonly property var modalInputScope: FerricModalInputController { hostWindow: another }
        }
    }
    function test_closing_another_window_preserves_primary_modal_input() {
        var another=createTemporaryObject(anotherWindowFixture,test)
        var primary=createTemporaryObject(genericDialogFixture,host.contentItem)
        var secondary=createTemporaryObject(genericDialogFixture,another.contentItem,{hostWindow:another})
        verify(waitForPolish(host))
        verify(host.modalInputScope.activeSurface===primary)
        verify(another.modalInputScope.activeSurface===secondary)
        compare(primary.interactive,true);compare(secondary.interactive,true)
        another.runtimeReleased=true
        tryCompare(secondary,"interactive",false)
        compare(another.modalInputScope.activeSurface,null)
        verify(host.modalInputScope.activeSurface===primary)
        another.destroy();wait(0)
        host.requestActivate();tryCompare(host,"active",true)
        keyClick(Qt.Key_Escape);tryCompare(primary,"visible",false)
    }
    function test_destroying_windows_with_visible_modals_retires_callbacks() {
        for(var index=0;index<6;++index) {
            var another=createTemporaryObject(anotherWindowFixture,test)
            var modal=createTemporaryObject(genericDialogFixture,another.contentItem,{hostWindow:another})
            verify(waitForPolish(another))
            verify(another.modalInputScope.activeSurface===modal)
            another.destroy();wait(0)
        }
        compare(host.modalInputScope.surfaces.length,0)
    }
    function test_other_window_modal_allows_primary_focus_return_data() {
        return [{tag:"mode",mode:true},{tag:"overlay",mode:false}]
    }
    function test_other_window_modal_allows_primary_focus_return(data) {
        var field=createTemporaryObject(backgroundFieldFixture,host.contentItem)
        field.forceActiveFocus();tryCompare(field,"activeFocus",true)
        var controller=createTemporaryObject(focusControllerFixture,test)
        if(data.mode)controller.captureModeFocus()
        else controller.captureOverlayFocus(host,field)
        var another=createTemporaryObject(anotherWindowFixture,test)
        var modal=createTemporaryObject(genericDialogFixture,another.contentItem,{hostWindow:another})
        host.requestActivate();tryCompare(host,"active",true)
        if(data.mode)controller.restoreModeFocus()
        else controller.restoreOverlayFocus(host)
        tryCompare(field,"activeFocus",true)
        verify(another.modalInputScope.activeSurface===modal)
        compare(modal.interactive,true)
    }
    function test_incoming_dialog_keeps_underlying_settings_open_data() {
        return [{tag:"escape",key:Qt.Key_Escape},{tag:"enter",key:Qt.Key_Return}]
    }
    function test_incoming_dialog_keeps_underlying_settings_open(data) {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor");verify(editor)
        editor.text="14";editor.forceActiveFocus();tryCompare(editor,"activeFocus",true)
        var dialog=createTemporaryObject(incomingDialogFixture,host.contentItem)
        verify(dialog)
        host.permissionPromptVisible=true
        verify(waitForPolish(host))
        var deny=findChild(dialog,"commandDialogAction-deny");verify(deny)
        tryCompare(deny,"activeFocus",true)
        keyClick(data.key)
        tryCompare(dialog,"visible",false)
        compare(view.settingsVisible,true)
        compare(owner.calls.length,0)
        compare(editor.text,"14")
        tryCompare(editor,"activeFocus",true)
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Cannot assign|Unable to assign|Binding loop/)
        host.permissionPromptVisible=false
        compare(host.modalInputScope.surfaces.length,0)
        host.runtimeReleased=false
        host.width=900;host.height=700;host.font.pointSize=10
        host.requestActivate();tryCompare(host,"active",true)
    }
    function workspace(owner, extra) {
        var properties=Object.assign({browserUi:owner},extra || {})
        var view=createTemporaryObject(workspaceFixture,host.contentItem,properties)
        verify(view);return view
    }
}
