import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "SettingsWorkspace"
    when: windowShown
    property list<string> enumOptions: ["always", "never"]
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
            property bool receiptMode: false
            property string immediateReceiptState: ""
            property int receiptSequence: 0
            property string setting_action_id: ""
            property string setting_action_state: "idle"
            property string setting_action_message: ""
            property string setting_action_detail: ""
            property string runtime_setting_error: "<b>literal failure details</b>"
            property bool profile_bootstrap_pending: false
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
            function startSettingReceipt() {
                if (!receiptMode) return
                setting_action_id="setting-"+(++receiptSequence)
                setting_action_message="Saving setting…"
                setting_action_detail=""
                setting_action_state="pending"
                if (!accepted || immediateReceiptState) {
                    setting_action_message=accepted ? "Applied setting for this session" : "Couldn't apply setting"
                    setting_action_detail=accepted ? "" : runtime_setting_error
                    setting_action_state=accepted ? immediateReceiptState : "failed"
                }
            }
            function set_runtime_setting(key, value, temporary) {
                calls = calls.concat([{key:key, value:value, temporary:temporary}])
                startSettingReceipt()
                return accepted
            }
            function unset_runtime_setting(key, temporary) {
                calls = calls.concat([{key:key, temporary:temporary}]); startSettingReceipt(); return accepted
            }
            function set_userscript_enabled(name, enabled) {
                if (!accepted) userscript_install_state="error:<b>private script failure</b>"
                return accepted
            }
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
        failOnWarning(/TypeError|ReferenceError|Cannot assign|Unable to assign|Binding loop|Detected function/)
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
        var feedback=findChild(view,"settingsOperationFeedback")
        var details=findChild(feedback,"managerFeedbackDetails");verify(!details.visible)
        // The feedback was just made visible; operate its laid-out control.
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        var button=findChild(feedback,"managerFeedbackDetailsButton")
        button.forceActiveFocus();tryCompare(button,"activeFocus",true)
        keyClick(Qt.Key_Space);tryCompare(details,"visible",true)
        compare(details.text,"<b>literal failure details</b>")
        compare(details.textFormat,TextEdit.PlainText);verify(details.selectByKeyboard)
    }
    function test_setting_draft_survives_filtering_and_return() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        compare(editor.text,"17")
        var search=findChild(view,"settingsSearchInput")
        search.forceActiveFocus();search.selectAll()
        for (var letter of "missing") keyClick(letter.toUpperCase().charCodeAt(0))
        tryCompare(findChild(view,"settingsList"),"count",0)
        search.selectAll()
        for (letter of "font") keyClick(letter.toUpperCase().charCodeAt(0))
        tryCompare(findChild(view,"settingsList"),"count",1)
        verify(waitForPolish(host))
        compare(findChild(view,"settingTextEditor").text,"17","Filtering must not discard an unfinished setting")
        compare(owner.calls.length,0)
    }
    function test_unchanged_refresh_preserves_editor_focus_selection_and_draft() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        keyClick(Qt.Key_Left,Qt.ShiftModifier)
        compare(editor.selectedText,"7")
        view.refreshSettings();verify(waitForPolish(host))
        var current=findChild(view,"settingTextEditor")
        compare(current.text,"17")
        compare(host.activeFocusItem,current)
        compare(current.selectedText,"7")
    }
    function test_external_value_update_does_not_overwrite_an_unapplied_edit() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        owner.settings_rows=[{key:"ui.font_size_pt",label:"Interface font size",type:"number",scope:"profile",apply:"immediate",value:"14",options:testEmptySettingsOptions}]
        view.refreshSettings();verify(waitForPolish(host))
        var current=findChild(view,"settingTextEditor")
        compare(current.text,"17")
        compare(host.activeFocusItem,current)
    }
    function test_published_values_update_without_a_manual_refresh() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        owner.settings_rows=[{key:"ui.font_size_pt",label:"Interface font size",type:"number",scope:"profile",apply:"immediate",value:"14",options:testEmptySettingsOptions}]
        tryCompare(editor,"text","14")
    }
    function test_background_save_has_pending_and_terminal_feedback() {
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        verify(view.applySetting(owner.settings_rows[0],"17"))
        verify(view.noticePending,"An accepted background write remains visibly pending")
        owner.setting_action_message="Saved ui.font_size_pt for this profile"
        owner.setting_action_state="succeeded"
        tryCompare(view,"noticePending",false)
        compare(view.settingsNotice,owner.setting_action_message)
        verify(!view.noticeFailed)
    }
    function test_background_failure_preserves_the_edit_and_discloses_worker_details() {
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7);keyClick(Qt.Key_Return)
        owner.setting_action_message="Couldn't save this setting"
        owner.setting_action_detail="<b>Owned disk failure</b>"
        owner.setting_action_state="failed"
        tryCompare(view,"noticeFailed",true)
        compare(view.noticeDetail,"<b>Owned disk failure</b>")
        compare(editor.text,"17");verify(findChild(view,"settingDraftNotice").visible)
        compare(host.activeFocusItem,editor)
    }
    function test_reopening_during_a_save_restores_its_feedback() {
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        verify(view.applySetting(owner.settings_rows[0],"17"))
        view.close();view.show();verify(waitForPolish(host))
        verify(view.noticePending)
        compare(view.settingsNotice,owner.setting_action_message)
    }
    function test_failed_reset_keeps_the_unapplied_draft() {
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        verify(view.resetSetting(owner.settings_rows[0]))
        compare(editor.text,"17","An accepted reset must not discard a draft before commit")
        owner.setting_action_message="Couldn't reset this setting"
        owner.setting_action_detail="Owned reset failure"
        owner.setting_action_state="failed"
        tryCompare(view,"noticeFailed",true)
        compare(editor.text,"17")
    }
    function editSetting(editor, value) {
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier)
        for (var index=0;index<value.length;++index) keyClick(value.charCodeAt(index))
    }
    function test_pending_save_does_not_resubmit_and_newer_text_survives_success() {
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editSetting(editor,"17");keyClick(Qt.Key_Return)
        verify(!findChild(view,"settingApplyButton").enabled)
        verify(!findChild(view,"settingResetButton").enabled)
        editSetting(editor,"10");keyClick(Qt.Key_Return)
        compare(owner.calls.length,1)
        owner.settings_rows=[Object.assign({},owner.settings_rows[0],{value:"17"})]
        owner.setting_action_message="Saved setting";owner.setting_action_state="succeeded"
        verify(waitForPolish(host));compare(editor.text,"10")
        verify(findChild(view,"settingDraftNotice").visible)
        verify(findChild(view,"settingApplyButton").enabled)
    }
    function test_failure_retry_gets_a_new_receipt_and_clears_only_the_saved_draft() {
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editSetting(editor,"17");keyClick(Qt.Key_Return)
        var failed=owner.setting_action_id
        owner.setting_action_message="Couldn't save setting";owner.setting_action_state="failed"
        verify(view.noticeFailed);compare(editor.text,"17")
        keyClick(Qt.Key_Return);verify(owner.setting_action_id!==failed);verify(view.noticePending)
        owner.settings_rows=[Object.assign({},owner.settings_rows[0],{value:"17"})]
        owner.setting_action_message="Saved setting";owner.setting_action_state="succeeded"
        verify(waitForPolish(host));verify(!findChild(view,"settingDraftNotice").visible)
        compare(editor.text,"17");verify(!view.noticeFailed);verify(!view.noticePending)
    }
    function test_successful_reset_discards_only_its_original_draft_data() {
        return [{tag:"original",newer:false},{tag:"newer",newer:true}]
    }
    function test_successful_reset_discards_only_its_original_draft(data) {
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editSetting(editor,"17");verify(view.resetSetting(owner.settings_rows[0]))
        if(data.newer) editSetting(editor,"19")
        owner.settings_rows=[Object.assign({},owner.settings_rows[0],{value:"12"})]
        owner.setting_action_message="Reset setting";owner.setting_action_state="succeeded"
        verify(waitForPolish(host));compare(editor.text,data.newer?"19":"12")
        compare(findChild(view,"settingDraftNotice").visible,data.newer)
    }
    function test_discard_during_save_does_not_lose_a_later_edit_back_to_old_value() {
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editSetting(editor,"17");keyClick(Qt.Key_Return)
        findChild(view,"settingDiscardButton").forceActiveFocus();keyClick(Qt.Key_Return);compare(editor.text,"10")
        editSetting(editor,"10")
        owner.settings_rows=[Object.assign({},owner.settings_rows[0],{value:"17"})]
        owner.setting_action_message="Saved setting";owner.setting_action_state="succeeded"
        verify(waitForPolish(host));compare(editor.text,"10")
        verify(findChild(view,"settingDraftNotice").visible)
    }
    function test_scope_and_unrelated_script_completion_do_not_hide_save_progress() {
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        verify(view.applySetting(owner.settings_rows[0],"17"))
        var surface=findChild(view,"settingsSurface")
        surface.temporaryChangeRequested(true)
        owner.userscript_install_state="installed"
        compare(view.settingsNotice,owner.setting_action_message);verify(view.noticePending)
        owner.setting_action_message="Saved for this profile";owner.setting_action_state="succeeded"
        compare(view.settingsNotice,"Saved for this profile")
        verify(view.settingsTemporary)
    }
    function test_profile_handoff_ignores_the_previous_receipt() {
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        verify(view.applySetting(owner.settings_rows[0],"17"))
        owner.profile_bootstrap_pending=true
        owner.setting_action_message="Old profile failed";owner.setting_action_state="failed"
        owner.profile_bootstrap_pending=false;view.show("font")
        compare(view.settingsNotice,"");verify(!view.noticePending);verify(!view.noticeFailed)
    }
    function test_immediate_receipt_is_processed_after_the_request_returns_data() {
        return [{tag:"success",accepted:true},{tag:"failure",accepted:false}]
    }
    function test_immediate_receipt_is_processed_after_the_request_returns(data) {
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true,accepted:data.accepted,immediateReceiptState:"succeeded"})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        compare(view.applySetting(owner.settings_rows[0],"17"),data.accepted)
        compare(view.settingsNotice,owner.setting_action_message)
        compare(view.noticeFailed,!data.accepted);verify(!view.noticePending)
    }
    function test_keyboard_toggle_remains_usable_after_a_background_save() {
        var rows=[{key:"content.fullscreen",label:"Allow fullscreen",type:"bool",scope:"profile",apply:"immediate",value:"false",options:testEmptySettingsOptions}]
        var owner=createTemporaryObject(ownerFixture,test,{receiptMode:true,settings_rows:rows})
        var view=workspace(owner);view.show();verify(waitForPolish(host))
        var toggle=findChild(view,"settingBooleanEditor")
        toggle.forceActiveFocus();keyClick(Qt.Key_Space)
        verify(view.noticePending);verify(!toggle.enabled)
        owner.settings_rows=[Object.assign({},rows[0],{value:"true"})]
        owner.setting_action_message="Saved setting";owner.setting_action_state="succeeded"
        verify(waitForPolish(host));verify(toggle.enabled)
        compare(host.activeFocusItem,toggle)
        keyClick(Qt.Key_Space);compare(owner.calls.length,2)
    }
    function test_applying_another_setting_preserves_an_unfinished_edit() {
        var rows=[{key:"ui.font_size_pt",label:"Interface font size",type:"number",scope:"profile",apply:"immediate",value:"10",options:testEmptySettingsOptions},
                  {key:"content.zoom",label:"Default zoom",type:"number",scope:"profile",apply:"immediate",value:"1",options:testEmptySettingsOptions}]
        var owner=createTemporaryObject(ownerFixture,test,{settings_rows:rows})
        var view=workspace(owner);view.show();verify(waitForPolish(host))
        var list=findChild(view,"settingsList")
        var first=list.itemAtIndex(0)
        var editor=findChild(first,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        verify(view.applySetting(rows[1],"1.5"))
        verify(waitForPolish(host))
        compare(findChild(list.itemAtIndex(0),"settingTextEditor").text,"17")
        compare(owner.calls[0].key,"content.zoom")
    }
    function test_row_reordering_preserves_the_focused_draft_and_native_undo() {
        var rows=[{key:"ui.font_size_pt",label:"Interface font size",type:"number",scope:"profile",apply:"immediate",value:"10",options:testEmptySettingsOptions},
                  {key:"content.zoom",label:"Default zoom",type:"number",scope:"profile",apply:"immediate",value:"1",options:testEmptySettingsOptions}]
        var owner=createTemporaryObject(ownerFixture,test,{settings_rows:rows})
        var view=workspace(owner);view.show();verify(waitForPolish(host))
        var list=findChild(view,"settingsList"), row=list.itemAtIndex(0)
        var editor=findChild(row,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        owner.settings_rows=[rows[1],rows[0]];view.refreshSettings();verify(waitForPolish(host))
        compare(list.itemAtIndex(1),row)
        tryCompare(host,"activeFocusItem",editor);compare(editor.text,"17")
        keyClick(Qt.Key_Z,Qt.ControlModifier)
        verify(editor.text!=="17","Refreshing must keep native undo available")
    }
    function test_discard_restores_the_latest_effective_value_and_editor_focus() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        owner.settings_rows=[{key:"ui.font_size_pt",label:"Interface font size",type:"number",scope:"profile",apply:"immediate",value:"14",options:testEmptySettingsOptions}]
        view.refreshSettings();verify(waitForPolish(host))
        var discard=findChild(view,"settingDiscardButton")
        verify(discard.visible);discard.forceActiveFocus();keyClick(Qt.Key_Return)
        compare(editor.text,"14");compare(host.activeFocusItem,editor)
        verify(!discard.visible);compare(owner.calls.length,0)
    }
    function test_profile_handoff_clears_unapplied_values() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        owner.profile_bootstrap_pending=true
        compare(view.settingsVisible,false)
        owner.settings_rows=[{key:"ui.font_size_pt",label:"Interface font size",type:"number",scope:"profile",apply:"immediate",value:"14",options:testEmptySettingsOptions}]
        owner.profile_bootstrap_pending=false;view.show("font");verify(waitForPolish(host))
        compare(findChild(view,"settingTextEditor").text,"14")
        verify(!findChild(view,"settingDraftNotice").visible)
    }
    function test_two_owners_keep_their_actual_field_drafts() {
        var first=createTemporaryObject(ownerFixture,test), second=createTemporaryObject(ownerFixture,test)
        var a=workspace(first), b=workspace(second)
        a.show("font");verify(waitForPolish(host))
        var editor=findChild(a,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        a.close();b.show("font");verify(waitForPolish(host))
        editor=findChild(b,"settingTextEditor");compare(editor.text,"10")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_2);keyClick(Qt.Key_3)
        b.close();a.show("font");verify(waitForPolish(host))
        compare(findChild(a,"settingTextEditor").text,"17")
        a.browserUi=second;a.refreshSettings();verify(waitForPolish(host))
        compare(findChild(a,"settingTextEditor").text,"10")
        a.browserUi=first;a.close();b.close()
    }
    function test_invalid_duplicate_rows_preserve_the_current_form() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        owner.settings_rows=[owner.settings_rows[0],owner.settings_rows[0]]
        view.refreshSettings();verify(waitForPolish(host))
        verify(view.noticeFailed)
        compare(findChild(view,"settingsList").count,1)
        compare(findChild(view,"settingTextEditor"),editor)
        compare(editor.text,"17");compare(host.activeFocusItem,editor)
    }
    function test_acknowledging_an_old_value_keeps_a_newer_edit() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        keyClick(Qt.Key_Return);compare(owner.calls[0].value,"17")
        keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_9)
        owner.settings_rows=[{key:"ui.font_size_pt",label:"Interface font size",type:"number",scope:"profile",apply:"immediate",value:"17",options:testEmptySettingsOptions}]
        view.refreshSettings();verify(waitForPolish(host))
        compare(editor.text,"19");verify(findChild(view,"settingDraftNotice").visible)
        compare(host.activeFocusItem,editor)
    }
    function test_matching_published_value_clears_the_unapplied_marker() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7)
        keyClick(Qt.Key_Return)
        owner.settings_rows=[{key:"ui.font_size_pt",label:"Interface font size",type:"number",scope:"profile",apply:"immediate",value:"17",options:testEmptySettingsOptions}]
        view.refreshSettings();verify(waitForPolish(host))
        compare(editor.text,"17");verify(!findChild(view,"settingDraftNotice").visible)
        compare(host.activeFocusItem,editor)
    }
    function test_canonical_published_values_acknowledge_only_the_submitted_edit_data() {
        return [{tag:"number",type:"number",initial:"10",typed:"17",published:"17.0"},
                {tag:"number-underscores",type:"number",initial:"10",typed:"1_7",published:"17.0"},
                {tag:"languages",type:"languages",initial:"en",typed:"en,de",published:"en, de"}]
    }
    function test_canonical_published_values_acknowledge_only_the_submitted_edit(data) {
        var row={key:"fixture.value",label:"Fixture",type:data.type,scope:"profile",apply:"immediate",value:data.initial,options:testEmptySettingsOptions}
        var owner=createTemporaryObject(ownerFixture,test,{settings_rows:[row]})
        var view=workspace(owner);view.show();verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier)
        for (var letter of data.typed) keyClick(letter.toUpperCase().charCodeAt(0))
        keyClick(Qt.Key_Return)
        row.value=data.published;owner.settings_rows=[row]
        view.refreshSettings();verify(waitForPolish(host))
        verify(!findChild(view,"settingDraftNotice").visible,"Published canonical values acknowledge the matching submission")
        compare(editor.text,data.published)
    }
    function test_editing_back_to_the_old_value_survives_an_inflight_acknowledgment() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7);keyClick(Qt.Key_Return)
        keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_0)
        owner.settings_rows=[{key:"ui.font_size_pt",label:"Interface font size",type:"number",scope:"profile",apply:"immediate",value:"17",options:testEmptySettingsOptions}]
        view.refreshSettings();verify(waitForPolish(host))
        compare(editor.text,"10");verify(findChild(view,"settingDraftNotice").visible)
    }
    function test_rejected_toggle_restores_its_effective_value() {
        var owner=createTemporaryObject(ownerFixture,test,{accepted:false,
            settings_rows:[{key:"content.images",label:"Images",type:"bool",scope:"profile",apply:"immediate",value:"true",options:testEmptySettingsOptions}]})
        var view=workspace(owner);view.show();verify(waitForPolish(host))
        var toggle=findChild(view,"settingBooleanEditor")
        verify(toggle);toggle.forceActiveFocus();keyClick(Qt.Key_Space)
        compare(owner.calls.length,1);verify(view.noticeFailed)
        compare(toggle.checked,true,"A rejected toggle must show the effective value")
    }
    function test_rejected_enum_restores_its_effective_value() {
        var owner=createTemporaryObject(ownerFixture,test,{accepted:false,
            settings_rows:[{key:"ui.statusbar",label:"Status bar",type:"enum",scope:"profile",apply:"immediate",value:"always",options:test.enumOptions}]})
        var view=workspace(owner);view.show();verify(waitForPolish(host))
        var editor=findChild(view,"settingEnumEditor")
        verify(editor);compare(editor.count,2);compare(editor.currentText,"always")
        editor.forceActiveFocus();keyClick(Qt.Key_Down)
        compare(owner.calls.length,1);verify(view.noticeFailed)
        compare(editor.currentText,"always");compare(host.activeFocusItem,editor)
    }
    function test_draft_controls_fit_large_fonts_data() {
        return [{tag:"390-20",width:390,font:20},{tag:"320-26",width:320,font:26}]
    }
    function test_draft_controls_fit_large_fonts(data) {
        host.runtimeMetrics=true;host.width=data.width;host.height=600;host.font.pointSize=data.font
        var owner=createTemporaryObject(ownerFixture,test,{accepted:false})
        var view=workspace(owner);view.show("font");verify(waitForPolish(host))
        var editor=findChild(view,"settingTextEditor")
        editor.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);keyClick(Qt.Key_1);keyClick(Qt.Key_7);keyClick(Qt.Key_Return)
        verify(waitForPolish(host));verify(waitForRendering(host.contentItem))
        var discard=findChild(view,"settingDiscardButton"), scroll=findChild(view,"settingsScroll")
        verify(discard.visible)
        verify(scroll.height>=Math.max(editor.height,discard.height),"The viewport must fit the focused editor and wrapping draft actions")
        discard.forceActiveFocus()
        tryVerify(function(){ var p=discard.mapToItem(scroll,0,0);return p.y>=-1 && p.y+discard.height<=scroll.height+1 })
        function check(item) {
            if (!item.visible) return
            if (typeof item.clicked==="function" && typeof item.text==="string") {
                var p=item.mapToItem(view,0,0)
                verify(p.x>=-1 && p.x+item.width<=view.width+1,item.text+" must fit the settings surface")
            }
            for (var child of item.children) check(child)
        }
        check(view)
        if (testArtifactDirectory.length>0) grabImage(host.contentItem).save(testArtifactDirectory+"/settings-draft-"+data.tag+".png")
    }
    function test_private_profile_uses_memory_scope_and_disables_script_changes() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner,{transientProfile:true});view.show("font");verify(waitForPolish(host))
        compare(findChild(view,"settingsInstallScript").enabled,false)
        verify(view.applySetting(owner.settings_rows[0],"14"))
        compare(owner.calls[0].temporary,true)
        verify(view.settingsNotice.indexOf("for this window and session")>=0)
    }
    function test_shared_save_scope_is_disclosed_before_filtering() {
        var owner=createTemporaryObject(ownerFixture,test)
        var view=workspace(owner);view.show();verify(waitForPolish(host))
        var scope=findChild(view,"settingsScopeDescription")
        verify(scope.visible);verify(scope.text.indexOf("all regular profiles")>=0)
        view.show("font");verify(waitForPolish(host))
        verify(!scope.visible)
        verify(findChild(view,"settingsScopeControl").Accessible.description.indexOf("all regular profiles")>=0)
        findChild(view,"settingsSurface").temporaryChangeRequested(true)
        verify(view.settingsNotice.indexOf("only to this window")>=0)
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
        var row=list.itemAtIndex(0);verify(row)
        var name=findChild(row,"settingNameLabel");verify(name)
        var position=name.mapToItem(scroll,0,0)
        verify(position.y>=0 && position.y+name.height<=scroll.height+1,"The first matching setting's full name must be readable before entering its editor")
        editor.forceActiveFocus()
        tryVerify(function() {
            var y=editor.mapToItem(scroll,0,0).y
            return y>=0 && y+editor.height<=scroll.height+1
        })
        keyClick(Qt.Key_A, Qt.ControlModifier)
        keyClick(Qt.Key_0)
        keyClick(Qt.Key_Return)
        verify(waitForPolish(host))
        tryCompare(view,"noticeFailed",true)
        compare(owner.calls[owner.calls.length-1].value,"0")
        compare(editor.text,"0")
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

    function scripts(view) {
        view.show();verify(waitForPolish(host))
        findChild(view,"settingsScriptsTab").click();verify(waitForPolish(host))
    }
    function test_inventory_error_loading_and_empty_states() {
        var owner=createTemporaryObject(ownerFixture,test,{userscript_inventory_ready:false})
        var view=workspace(owner);scripts(view)
        var feedback=findChild(view,"userscriptInventoryFeedback")
        var empty=findChild(view,"userscriptInventoryEmpty")
        verify(feedback.visible);verify(!empty.visible)
        verify(feedback.message.indexOf("Refresh to load")>=0)
        owner.userscript_inventory_pending=true
        tryCompare(feedback,"pending",true);verify(!empty.visible)
        owner.userscript_inventory_error="<b>read /private/fixture/userscripts: Not a directory</b>"
        owner.userscript_inventory_pending=false
        tryCompare(feedback,"failed",true);verify(!empty.visible)
        compare(feedback.message.indexOf("/private"),-1)
        var details=findChild(feedback,"managerFeedbackDetails")
        verify(!details.visible)
        findChild(feedback,"managerFeedbackDetailsButton").click()
        tryCompare(details,"visible",true)
        compare(details.text,owner.userscript_inventory_error)
        compare(details.textFormat,TextEdit.PlainText);verify(details.selectByKeyboard)
        owner.userscript_inventory_error="";owner.userscript_inventory_ready=true
        tryCompare(empty,"visible",true);verify(!feedback.visible)
    }
    function test_inventory_failure_retains_previous_rows_and_recovers() {
        var owner=createTemporaryObject(ownerFixture,test,{
            userscript_names:["Example"],userscript_enabled_values:["true"],
            userscript_page_world_values:["false"],userscript_action_counts:["0"]})
        var view=workspace(owner);scripts(view)
        var toggle=findChild(view,"userscriptEnableToggle");verify(toggle.enabled)
        owner.userscript_inventory_pending=true
        tryCompare(toggle,"enabled",false)
        owner.userscript_inventory_error="couldn't read /private/fixture/userscripts"
        owner.userscript_inventory_pending=false
        compare(view.userscriptRows().length,1);verify(!toggle.enabled)
        verify(findChild(view,"userscriptInventoryPreviousResults").visible)
        verify(!findChild(view,"userscriptInventoryEmpty").visible)
        owner.userscript_inventory_error=""
        tryCompare(toggle,"enabled",true)
        verify(!findChild(view,"userscriptInventoryPreviousResults").visible)
    }
    function test_rejected_script_change_keeps_error_detail() {
        var owner=createTemporaryObject(ownerFixture,test,{accepted:false,
            userscript_names:["Example"],userscript_enabled_values:["true"],
            userscript_page_world_values:["false"],userscript_action_counts:["0"]})
        var view=workspace(owner);scripts(view)
        findChild(view,"userscriptEnableToggle").click()
        compare(view.noticeDetail,"<b>private script failure</b>")
        verify(view.noticeFailed);compare(view.settingsNotice.indexOf("private"),-1)
        view.close();view.show()
        compare(view.noticeDetail,"<b>private script failure</b>")
        verify(view.noticeFailed)
        owner.userscript_install_state="idle"
        compare(view.settingsNotice,"");compare(view.noticeDetail,"")
    }
    function test_applied_change_with_inventory_failure_keeps_its_success() {
        var owner=createTemporaryObject(ownerFixture,test,{
            userscript_names:["Example"],userscript_enabled_values:["false"],
            userscript_page_world_values:["false"],userscript_action_counts:["0"]})
        var view=workspace(owner);scripts(view)
        owner.userscript_inventory_error="An unrelated manifest couldn't be read."
        owner.userscript_install_state="enabled"
        compare(view.noticeFailed,false)
        verify(view.settingsNotice.indexOf("enabled")>=0)
        verify(findChild(view,"userscriptInventoryFeedback").failed)
        verify(!findChild(view,"userscriptEnableToggle").enabled)
    }
    function test_inventory_error_is_reachable_with_configured_fonts_data() {
        return [{tag:"compact",width:640,height:480,font:16},{tag:"narrow",width:390,height:600,font:20}]
    }
    function test_inventory_error_is_reachable_with_configured_fonts(data) {
        host.runtimeMetrics=true;host.width=data.width;host.height=data.height;host.font.pointSize=data.font
        var owner=createTemporaryObject(ownerFixture,test,{
            userscript_inventory_error:"Unable to read /private/fixture/userscripts: "+Array(10).join("long diagnostic detail ")})
        var view=workspace(owner);scripts(view)
        var feedback=findChild(view,"userscriptInventoryFeedback")
        var button=findChild(feedback,"managerFeedbackDetailsButton")
        button.forceActiveFocus()
        var scroll=findChild(view,"settingsScriptsScroll")
        tryVerify(function() {
            var y=button.mapToItem(scroll,0,0).y
            return y>=0 && y+button.height<=scroll.height+1
        })
        keyClick(Qt.Key_Space)
        var details=findChild(feedback,"managerFeedbackDetails")
        tryCompare(details,"visible",true)
        verify(waitForPolish(host))
        details.forceActiveFocus()
        tryCompare(details,"activeFocus",true)
        tryVerify(function() {
            var caret=details.cursorRectangle
            var y=details.mapToItem(scroll,caret.x,caret.y).y
            return y>=0 && y+caret.height<=scroll.height+1
        })
        verify(waitForRendering(host.contentItem))
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/settings-inventory-error-"+data.tag+".png")
        keyClick(Qt.Key_Escape);tryCompare(view,"settingsVisible",false)
    }
}
