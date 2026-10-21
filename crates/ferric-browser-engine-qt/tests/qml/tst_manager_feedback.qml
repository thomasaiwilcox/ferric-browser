import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
TestCase {
    id: test
    name: "ManagerFeedback"
    when: windowShown
    ApplicationWindow {
        id: host
        width: 900
        height: 700
        visible: true
        font.family: "monospace"
        font.pointSize: 10
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
        property bool downloadManagerVisible: true
        property bool sessionManagerVisible: true
        property bool temporaryProfile: false
        property bool ephemeralProfile: false
        property bool sessionPreviewVisible: true
        property bool sessionPreviewLoading: false
        property bool sessionPreviewError: false
        property bool sessionPreviewAppend: false
        property string sessionPreviewName: "Research"
        property string sessionPreviewText: ""
        property bool profileManagerVisible: true
        property bool profileDeletePreviewVisible: false
        property string profileDeleteName: "work"
        property string profileDeletePreviewText: "/disposable/profile/path"
        property string pendingDeleteName: ""
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

    ListModel { id: profiles; ListElement { name: "work"; label: "<b>Work</b>" } }
    ListModel { id: sessions; ListElement { name: "research" } }
    Component { id: profileFixture; FerricProfileManager { browserWindow: host; profilesModel: profiles } }
    Component { id: sessionFixture; FerricSessionManager { browserWindow: host; sessionsModel: sessions } }
    Component { id: spyFixture; SignalSpy {} }
    function receipt(id, state, message, detail) { return {id: id, state: state, message: message || "", detail: detail || ""} }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width = 900; host.height = 700; host.font.pointSize = 10
        host.profileManagerVisible = true; host.sessionManagerVisible = true
        host.temporaryProfile = false; host.ephemeralProfile = false
        profiles.clear(); profiles.append({name: "work", label: "<b>Work</b>"})
        sessions.clear(); sessions.append({name: "research"})
        host.requestActivate(); tryCompare(host, "active", true)
    }
    function profile() {
        var view = createTemporaryObject(profileFixture, host.contentItem)
        verify(view); verify(waitForPolish(host)); verify(view.createNameField)
        view.createNameField.text = "new-profile"; view.createLabelField.text = "New profile"
        return view
    }
    function session() {
        var view = createTemporaryObject(sessionFixture, host.contentItem)
        verify(view); verify(waitForPolish(host)); verify(view.nameField)
        view.nameField.text = "draft-session"
        return view
    }
    function activate(control) {
        verify(control && control.enabled)
        control.forceActiveFocus()
        verify(waitForPolish(host))
        compare(host.activeFocusItem, control)
        keyClick(Qt.Key_Return)
    }
    function test_session_return_submits_the_name() {
        var view = session()
        var saved = createTemporaryObject(spyFixture, view, {target: view, signalName: "saveRequested"})
        var accepted = createTemporaryObject(spyFixture, view, {target: view.nameField, signalName: "accepted"})
        var clicked = createTemporaryObject(spyFixture, view, {target: findChild(view, "sessionSaveButton"), signalName: "clicked"})
        view.nameField.forceActiveFocus()
        keyClick(Qt.Key_Return)
        compare(saved.count, 1, "accepted=" + accepted.count + ", clicked=" + clicked.count
            + ", focus=" + host.activeFocusItem.objectName)
        compare(saved.signalArguments[0][0], "draft-session")
    }
    function test_profile_return_advances_to_display_name() {
        var view = profile()
        view.createLabelField.text = ""
        var created = createTemporaryObject(spyFixture, view, {target: view, signalName: "createRequested"})
        view.createNameField.forceActiveFocus()
        keyClick(Qt.Key_Return)
        compare(host.activeFocusItem, view.createLabelField)
        compare(created.count, 0)
    }
    function test_profile_return_submits_the_display_name() {
        var view = profile()
        var created = createTemporaryObject(spyFixture, view, {target: view, signalName: "createRequested"})
        view.createLabelField.forceActiveFocus()
        keyClick(Qt.Key_Return)
        compare(created.count, 1)
        compare(created.signalArguments[0][0], "new-profile")
        compare(created.signalArguments[0][1], "New profile")
    }
    function test_profile_return_submits_the_rename() {
        var view = profile()
        var renamed = createTemporaryObject(spyFixture, view, {target: view, signalName: "renameRequested"})
        view.beginRename("work", "Work")
        tryCompare(view.renameField, "activeFocus", true)
        view.renameField.text = "Research"
        keyClick(Qt.Key_Return)
        compare(renamed.count, 1)
        compare(renamed.signalArguments[0][0], "work")
        compare(renamed.signalArguments[0][1], "Research")
    }
    function test_cancel_rename_returns_to_a_visible_control() {
        var view = profile()
        view.beginRename("work", "Work")
        tryCompare(view.renameField, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        compare(host.activeFocusItem.objectName, "profileRenameButton")
        keyClick(Qt.Key_Tab)
        compare(host.activeFocusItem.text, "Cancel rename")
        keyClick(Qt.Key_Return)
        compare(view.renameName, "")
        verify(host.activeFocusItem !== null && host.activeFocusItem.visible
            && host.activeFocusItem.enabled && host.activeFocusItem.activeFocusOnTab,
            "Closing the rename form leaves focus on an available control")
    }
    function test_cancel_rename_returns_to_its_invoking_control() {
        var view = profile()
        var action = findChild(view, "profileBeginRenameButton")
        verify(action)
        action.forceActiveFocus()
        keyClick(Qt.Key_Return)
        tryCompare(view.renameField, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_Return)
        compare(view.renameName, "")
        compare(host.activeFocusItem, action)
    }
    function test_successful_create_prepares_the_next_name_without_stealing_focus_data() {
        return [{tag: "form", inForm: true}, {tag: "other-control", inForm: false}]
    }
    function test_successful_create_prepares_the_next_name_without_stealing_focus(data) {
        var view = profile()
        var action = findChild(view, "profileBeginRenameButton")
        var target = data.inForm ? view.createLabelField : action
        target.forceActiveFocus()
        view.feedback = receipt("create", "pending")
        view.mutationAccepted("create", "create", "new-profile", "New profile")
        view.feedback = receipt("create", "succeeded")
        compare(view.createNameField.text, "")
        compare(view.createLabelField.text, "")
        compare(host.activeFocusItem, data.inForm ? view.createNameField : action)
    }
    function test_completed_rename_leaves_visible_focus() {
        var view = profile()
        view.beginRename("work", "Work")
        tryCompare(view.renameField, "activeFocus", true)
        view.renameField.text = "Research"
        view.feedback = receipt("rename", "pending")
        view.mutationAccepted("rename", "rename", "work", "Research")
        view.feedback = receipt("rename", "succeeded")
        compare(view.renameName, "")
        compare(host.activeFocusItem, view.createNameField)
    }
    function test_completed_rename_keeps_focus_after_catalog_publication() {
        var view = profile()
        var action = findChild(view, "profileBeginRenameButton")
        action.forceActiveFocus()
        keyClick(Qt.Key_Return)
        tryCompare(view.renameField, "activeFocus", true)
        view.renameField.text = "Research"
        view.feedback = receipt("rename", "pending")
        view.mutationAccepted("rename", "rename", "work", "Research")
        view.feedback = receipt("rename", "succeeded")
        profiles.clear(); profiles.append({name: "work", label: "Research"})
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        verify(host.activeFocusItem && host.activeFocusItem.visible && host.activeFocusItem.enabled
            && host.activeFocusItem.activeFocusOnTab,
            "Publishing the renamed catalog leaves a usable control focused")
    }
    function test_session_return_respects_submission_guards_data() {
        return [{tag: "blank"}, {tag: "saving"}, {tag: "deleting"},
            {tag: "temporary"}, {tag: "ephemeral"}]
    }
    function test_session_return_respects_submission_guards(data) {
        var view = session()
        var saved = createTemporaryObject(spyFixture, view, {target: view, signalName: "saveRequested"})
        if (data.tag === "blank") view.nameField.text = "   "
        if (data.tag === "saving") view.saveFeedback = receipt("pending", "pending")
        if (data.tag === "deleting") view.deleteFeedback = receipt("pending", "pending")
        host.temporaryProfile = data.tag === "temporary"
        host.ephemeralProfile = data.tag === "ephemeral"
        verify(!findChild(view, "sessionSaveButton").enabled)
        view.nameField.forceActiveFocus()
        keyClick(Qt.Key_Return)
        compare(saved.count, 0)
    }
    function test_profile_return_respects_submission_guards_data() {
        return [{tag: "blank-name"}, {tag: "blank-label"}, {tag: "creating"},
            {tag: "blank-rename", rename: true}, {tag: "renaming", rename: true}]
    }
    function test_profile_return_respects_submission_guards(data) {
        var view = profile()
        var created = createTemporaryObject(spyFixture, view, {target: view, signalName: "createRequested"})
        var renamed = createTemporaryObject(spyFixture, view, {target: view, signalName: "renameRequested"})
        if (data.rename) view.beginRename("work", "Work")
        var input = data.rename ? view.renameField : view.createLabelField
        if (data.tag === "blank-name") view.createNameField.text = " "
        if (data.tag === "blank-label" || data.tag === "blank-rename") input.text = " "
        if (data.tag === "creating" || data.tag === "renaming") view.feedback = receipt("pending", "pending")
        verify(!findChild(view, data.rename ? "profileRenameButton" : "profileCreateButton").enabled)
        input.forceActiveFocus()
        keyClick(Qt.Key_Return)
        compare(created.count, 0)
        compare(renamed.count, 0)
    }
    function test_refresh_preserves_form_focus_data() {
        return [{tag: "profile", profile: true}, {tag: "session", profile: false}]
    }
    function test_refresh_preserves_form_focus(data) {
        var view = data.profile ? profile() : session()
        var input = data.profile ? view.createLabelField : view.nameField
        var text = input.text
        input.forceActiveFocus()
        var model = data.profile ? profiles : sessions
        model.clear()
        model.append(data.profile ? {name: "another", label: "Another profile"} : {name: "another"})
        verify(waitForPolish(host))
        verify(waitForRendering(host.contentItem))
        compare(host.activeFocusItem, input)
        compare(input.text, text)
    }
    function test_tab_leaves_the_form_for_row_actions_data() {
        return [{tag: "profile", profile: true}, {tag: "session", profile: false}]
    }
    function test_tab_leaves_the_form_for_row_actions(data) {
        var view = data.profile ? profile() : session()
        var input = data.profile ? view.createLabelField : view.nameField
        input.forceActiveFocus()
        keyClick(Qt.Key_Tab)
        compare(host.activeFocusItem.objectName, data.profile ? "profileCreateButton" : "sessionSaveButton")
        keyClick(Qt.Key_Tab)
        compare(host.activeFocusItem.text, data.profile ? "Open window" : "Preview")
    }
    function test_profile_retains_failed_draft_and_clears_its_own_success() {
        var view = profile()
        view.feedback = receipt("one", "pending", "Creating profile…")
        view.mutationAccepted("one", "create", "new-profile", "New profile")
        compare(view.createNameField.text, "new-profile")
        verify(view.createNameField.readOnly); verify(!findChild(view, "profileCreateButton").enabled)
        view.feedback = receipt("one", "failed", "The profile change couldn't be completed.", "<b>filesystem error</b>")
        compare(view.createNameField.text, "new-profile"); verify(!view.createNameField.readOnly)
        var feedback = findChild(view, "profileFeedback")
        compare(feedback.message, "The profile change couldn't be completed.")
        var details = findChild(feedback, "managerFeedbackDetails")
        verify(!details.visible)
        activate(findChild(feedback, "managerFeedbackDetailsButton"))
        tryCompare(details, "visible", true)
        compare(details.textFormat, TextEdit.PlainText); verify(details.readOnly); verify(details.selectByMouse)
        view.feedback = receipt("two", "pending", "Creating profile…")
        view.mutationAccepted("two", "create", "new-profile", "New profile")
        view.feedback = receipt("two", "succeeded", "Profile created")
        compare(view.createNameField.text, ""); compare(view.createLabelField.text, "")
        verify(!details.visible)
    }
    function test_profile_unrelated_receipt_or_newer_draft_is_preserved() {
        var view = profile()
        view.feedback = receipt("one", "pending")
        view.mutationAccepted("one", "create", "new-profile", "New profile")
        view.feedback = receipt("other", "succeeded", "Profile created")
        compare(view.createNameField.text, "new-profile")
        view.createLabelField.text = "A newer draft"
        view.feedback = receipt("one", "succeeded", "Profile created")
        compare(view.createNameField.text, "new-profile"); compare(view.createLabelField.text, "A newer draft")
    }
    function test_profile_rename_form_tracks_only_its_submitted_change() {
        var view = profile()
        var form = findChild(view, "profileRenameForm")
        verify(!form.visible)
        view.beginRename("work", "Work"); tryCompare(form, "visible", true)
        view.renameField.text = "Research"
        view.feedback = receipt("rename", "pending", "Renaming profile…")
        view.mutationAccepted("rename", "rename", "work", "Research")
        view.feedback = receipt("rename", "failed", "Couldn't rename", "disk error")
        compare(view.renameField.text, "Research"); verify(form.visible)
        view.feedback = receipt("retry", "pending")
        view.mutationAccepted("retry", "rename", "work", "Research")
        view.feedback = receipt("retry", "succeeded", "Profile display name updated")
        verify(!form.visible); compare(view.createNameField.text, "new-profile")
    }
    function test_session_receipt_and_immediate_rejection_keep_drafts() {
        var view = session()
        view.requestRejected("Review this session name.", "invalid name")
        compare(view.nameField.text, "draft-session")
        compare(findChild(view, "sessionRequestFeedback").message, "Review this session name.")
        view.saveFeedback = receipt("one", "pending", "Saving session…")
        view.saveAccepted("one", "draft-session")
        verify(view.nameField.readOnly); verify(!findChild(view, "sessionSaveButton").enabled)
        compare(view.rejectionMessage, "")
        view.saveFeedback = receipt("one", "failed", "Session couldn't be saved.", "disk full")
        compare(view.nameField.text, "draft-session"); verify(!view.nameField.readOnly)
        view.saveFeedback = receipt("two", "pending")
        view.saveAccepted("two", "draft-session")
        view.saveFeedback = receipt("other", "succeeded")
        compare(view.nameField.text, "draft-session")
        view.nameField.text = "newer-draft"
        view.saveFeedback = receipt("two", "succeeded")
        compare(view.nameField.text, "newer-draft")
        view.saveFeedback = receipt("three", "pending")
        view.saveAccepted("three", "newer-draft")
        view.saveFeedback = receipt("three", "succeeded", "Session saved")
        compare(view.nameField.text, "")
    }
    function test_session_delete_confirmation_expires() {
        var view = session()
        view.pendingDeleteName = "research"
        view.deleteFeedback = receipt("one", "pending", "Deleting session…")
        compare(view.pendingDeleteName, "")
        view.deleteFeedback = receipt("one", "failed", "Couldn't delete")
        view.pendingDeleteName = "research"
        host.sessionManagerVisible = false
        compare(view.pendingDeleteName, "")
        host.sessionManagerVisible = true
        view.pendingDeleteName = "research"; activate(findChild(view, "sessionRefreshButton"))
        compare(view.pendingDeleteName, "")
    }
    function test_catalog_failure_is_distinct_from_empty() {
        var view = session()
        view.listPending = true
        compare(findChild(view, "sessionListFeedback").message, "Loading saved sessions…")
        view.listPending = false; view.listError = "permission denied"
        var feedback = findChild(view, "sessionListFeedback")
        verify(feedback.failed); verify(feedback.message.indexOf("Refresh") >= 0)
        compare(findChild(view, "sessionList").count, 1)
    }
    function test_compact_forms_remain_scrollable_data() {
        return [{tag: "profile", profile: true, font: 10}, {tag: "profile-large", profile: true, font: 16},
                {tag: "session", profile: false, font: 10}, {tag: "session-large", profile: false, font: 16},
                {tag: "profile-26", profile: true, font: 26, width: 320},
                {tag: "session-26", profile: false, font: 26, width: 320}]
    }
    function test_compact_forms_remain_scrollable(row) {
        host.width = row.width || 390; host.height = 600; host.font.pointSize = row.font
        var view = row.profile ? profile() : session()
        var list = findChild(view, row.profile ? "profileList" : "sessionList")
        if (row.profile) {
            view.beginRename("work", "A longer display name")
            tryCompare(view.renameField, "activeFocus", true)
            view.feedback = receipt("one", "failed", "The profile change couldn't be completed. Review the details and try again.", Array(20).join("A long technical detail.\n"))
        } else view.saveFeedback = receipt("one", "failed", "The session couldn't be saved. Review the details and try again.", Array(20).join("A long technical detail.\n"))
        var feedback = findChild(view, row.profile ? "profileFeedback" : "sessionSaveFeedback")
        activate(findChild(feedback, "managerFeedbackDetailsButton"))
        verify(waitForPolish(host)); wait(50)
        var input = row.profile ? view.renameField : view.nameField
        input.forceActiveFocus()
        tryVerify(function() { var point = input.mapToItem(list, 0, 0); return point.y >= -1 && point.y + input.height <= list.height + 1 })
        tryVerify(function() { return list.contentHeight > list.height })
        var button = findChild(view, row.profile ? "profileRenameButton" : "sessionSaveButton")
        button.forceActiveFocus()
        tryVerify(function() { var point = button.mapToItem(list, 0, 0); return point.y >= -1 && point.y + button.height <= list.height + 1 })
        wait(50)
        var finalPoint = button.mapToItem(list, 0, 0)
        verify(finalPoint.y >= -1 && finalPoint.y + button.height <= list.height + 1)
        compare(button.width > view.panelItem.width, false)
        function checkButtons(item) {
            if (!item.visible) return
            if (typeof item.text === "string" && typeof item.clicked === "function") {
                var x = item.mapToItem(view.panelItem, 0, 0).x
                verify(x >= -1 && x + item.width <= view.panelItem.width + 1,
                    item.text + " fits at " + host.width + "px and " + row.font + "pt")
            }
            for (var child of item.children) checkButtons(child)
        }
        checkButtons(view)
        if (testArtifactDirectory.length > 0) grabImage(host.contentItem).save(testArtifactDirectory + "/manager-" + row.tag + ".png")
    }
}
