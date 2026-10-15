import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "SettingsWorkspace"
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
    function init() {
        failOnWarning(/TypeError|ReferenceError|Cannot assign|Unable to assign|Binding loop/)
        host.runtimeMetrics=false
        host.width=900;host.height=700;host.font.pointSize=10
        host.requestActivate();tryCompare(host,"active",true)
    }
    function workspace(owner, extra) {
        var properties=Object.assign({browserUi:owner},extra || {})
        var view=createTemporaryObject(workspaceFixture,host.contentItem,properties)
        verify(view);return view
    }
    function visibleCommandSurface(item, command) {
        if (item.commandText===command && item.visible) return item
        var children=item.children || []
        for(var i=0;i<children.length;i++) {
            var found=visibleCommandSurface(children[i],command)
            if(found) return found
        }
        return null
    }
    function test_remove_confirmation_owns_pointer_and_escape_data() {
        return [{tag:"pointer",pointer:true},{tag:"escape",pointer:false}]
    }
    function test_remove_confirmation_owns_pointer_and_escape(data) {
        var owner=createTemporaryObject(ownerFixture,test,{
            userscript_names:["Example"],userscript_enabled_values:["false"],
            userscript_page_world_values:["false"],userscript_action_counts:["0"]})
        var view=workspace(owner);view.show();verify(waitForPolish(host))
        view.confirmRemoveUserscript("Example");verify(waitForPolish(host))
        var confirmation=visibleCommandSurface(view,":userscript-remove");verify(confirmation)
        if(data.pointer && testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/settings-script-removal.png")
        if(data.pointer) mouseClick(findChild(confirmation,"commandDialogAction-cancel"))
        else keyClick(Qt.Key_Escape)
        tryCompare(confirmation,"visible",false)
        compare(view.settingsVisible,true);compare(view.pendingUserscriptRemoval,"")
    }
    function test_window_owners_keep_independent_drafts_and_mutations() {
        var first=createTemporaryObject(ownerFixture,test)
        var second=createTemporaryObject(ownerFixture,test)
        var a=workspace(first), b=workspace(second)
        a.show("font");verify(waitForPolish(host))
        compare(a.settingsSearch,"font");compare(b.settingsVisible,false)
        compare(findChild(a,"dialogMessageLabel").textFormat,TextEdit.PlainText)
        verify(findChild(a,"dialogMessageLabel").text.indexOf("<b>profile</b>")>=0)
        verify(a.applySetting(first.settings_rows[0],"14"))
        compare(first.calls.length,1);compare(second.calls.length,0)
        a.close();b.show("other")
        compare(a.settingsSearch,"font");compare(b.settingsSearch,"other")
        verify(b.resetSetting(second.settings_rows[0]))
        compare(first.calls.length,1);compare(second.calls.length,1)
        b.close();a.show();compare(a.settingsSearch,"font")
    }
    function test_rejected_change_retains_draft_and_discloses_literal_details() {
        var owner=createTemporaryObject(ownerFixture,test,{accepted:false})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor");verify(editor)
        editor.text="unsaved value"
        compare(view.applySetting(owner.settings_rows[0],editor.text),false)
        compare(editor.text,"unsaved value");verify(view.noticeFailed)
        compare(view.settingsNotice.indexOf("<b>"),-1)
        var details=findChild(view,"managerFeedbackDetails");verify(!details.visible)
        findChild(view,"managerFeedbackDetailsButton").click();tryCompare(details,"visible",true)
        compare(details.text,"<b>literal failure details</b>")
        compare(details.textFormat,TextEdit.PlainText);verify(details.selectByKeyboard)
    }
    function test_private_profile_uses_memory_scope_and_disables_script_changes() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner,{transientProfile:true});view.show("font");verify(waitForPolish(host))
        compare(findChild(view,"settingsInstallScript").enabled,false)
        verify(view.applySetting(owner.settings_rows[0],"14"))
        compare(owner.calls[0].temporary,true)
        verify(view.settingsNotice.indexOf("for this session")>=0)
    }
    function test_enable_write_failure_restores_saved_value_and_retry_recovers() {
        var owner=createTemporaryObject(ownerFixture,test,{
            userscript_names:["Example"],userscript_enabled_values:["false"],
            userscript_page_world_values:["false"],userscript_action_counts:["0"]})
        var view=workspace(owner);view.show();verify(waitForPolish(host))
        findChild(view,"settingsScriptsTab").click();verify(waitForPolish(host))
        var toggle=findChild(view,"userscriptEnableToggle");verify(toggle);compare(toggle.checked,false)
        toggle.click();compare(toggle.checked,true);verify(view.noticePending);compare(toggle.enabled,false)
        owner.userscript_install_state="error:fixture write failed"
        tryCompare(toggle,"checked",false);verify(view.noticeFailed);compare(toggle.enabled,true)
        toggle.click();verify(view.noticePending)
        owner.userscript_enabled_values=["true"]
        owner.userscript_install_state="enabled"
        verify(waitForPolish(host));toggle=findChild(view,"userscriptEnableToggle")
        verify(toggle);tryCompare(toggle,"checked",true);compare(view.noticePending,false)
        compare(view.noticeFailed,false);compare(view.noticeDetail,"")
    }
    function test_sections_and_empty_search_explain_available_actions() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("missing setting");verify(waitForPolish(host))
        var empty=findChild(view,"settingsEmptyResults");verify(empty.visible)
        verify(empty.text.indexOf("No settings match")>=0)
        var scripts=findChild(view,"settingsScriptsTab")
        mouseClick(scripts);verify(waitForPolish(host))
        var install=findChild(view,"settingsInstallScript")
        verify(install.visible);verify(install.enabled)
        var search=findChild(view,"settingsSearchInput")
        verify(!search.visible)
        view.close();view.show();verify(waitForPolish(host))
        tryCompare(install,"activeFocus",true)
        // Help for a configuration key always returns to the matching settings.
        view.show("font");verify(waitForPolish(host))
        tryCompare(search,"activeFocus",true)
        verify(!empty.visible)
    }
    function test_configured_font_keeps_results_reachable_data() {
        return [{tag:"compact",width:640,height:480,font:14},{tag:"narrow",width:390,height:600,font:20}]
    }
    function test_configured_font_keeps_results_reachable(data) {
        host.runtimeMetrics=true
        host.width=data.width;host.height=data.height;host.font.pointSize=data.font
        var owner=createTemporaryObject(ownerFixture,test,{accepted:false})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor");verify(editor)
        var scroll=findChild(view,"settingsScroll")
        // Filtering should show the first matching setting, rather than a
        // page of unrelated guidance and script controls above the results.
        var list=findChild(view,"settingsList");verify(list)
        var search=findChild(view,"settingsSearchInput")
        tryCompare(search,"activeFocus",true)
        tryVerify(function() {
            var y=search.mapToItem(scroll,0,0).y
            return y>=0 && y+search.height<=scroll.height+1
        })
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/settings-configured-font-"+data.tag+".png")
        verify(list.mapToItem(scroll,0,0).y < scroll.height, "Matching settings must start within the visible form")
        editor.forceActiveFocus()
        tryVerify(function() {
            var y=editor.mapToItem(scroll,0,0).y
            return y>=0 && y+editor.height<=scroll.height+1
        })
        editor.text="0";editor.accepted();verify(waitForPolish(host))
        verify(view.noticeFailed)
        tryVerify(function() {
            var y=editor.mapToItem(scroll,0,0).y
            return y>=0 && y+editor.height<=scroll.height+1
        })
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/settings-configured-font-error-"+data.tag+".png")
    }
    function test_worker_error_has_readable_feedback_and_close_remains_available_data() {
        return [{tag:"wide",width:900,height:700,font:10},{tag:"compact",width:640,height:480,font:16},{tag:"narrow",width:390,height:600,font:20}]
    }
    function test_worker_error_has_readable_feedback_and_close_remains_available(data) {
        host.width=data.width;host.height=data.height;host.font.pointSize=data.font
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font")
        owner.userscript_install_state="error:<b>private implementation details</b>"
        verify(view.noticeFailed);compare(view.settingsNotice.indexOf("implementation"),-1)
        verify(waitForPolish(host))
        var install=findChild(view,"settingsInstallScript")
        verify(install.width<=install.parent.width+1)
        verify(install.parent.width<=view.width-50)
        verify(install.contentItem.height>=install.contentItem.contentHeight-1)
        var scopeLabel=findChild(view,"settingsSessionScopeLabel")
        verify(scopeLabel.height>=scopeLabel.contentHeight-1)
        verify(scopeLabel.width<=install.parent.width+1)
        var search=findChild(view,"settingsSearchInput")
        var scroll=findChild(view,"settingsScroll")
        var feedbackViewport=findChild(view,"settingsFeedbackViewport")
        verify(feedbackViewport.visible && feedbackViewport.height>0)
        var surface=findChild(view,"settingsSurface")
        verify(feedbackViewport.y>=0)
        verify(feedbackViewport.y+feedbackViewport.height<=surface.bodyItem.height+1)
        tryCompare(search,"activeFocus",true)
        tryVerify(function() {
            var y=search.mapToItem(scroll,0,0).y
            return y>=0 && y+search.height<=scroll.height+1
        })
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/settings-workspace-error-"+data.tag+".png")
        keyClick(Qt.Key_Escape);tryCompare(view,"settingsVisible",false)
    }
}
