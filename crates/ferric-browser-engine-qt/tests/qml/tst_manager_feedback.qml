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
        findChild(feedback, "managerFeedbackDetailsButton").click()
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
        view.pendingDeleteName = "research"; findChild(view, "sessionRefreshButton").click()
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
                {tag: "session", profile: false, font: 10}, {tag: "session-large", profile: false, font: 16}]
    }
    function test_compact_forms_remain_scrollable(row) {
        host.width = 390; host.height = 600; host.font.pointSize = row.font
        var view = row.profile ? profile() : session()
        var list = findChild(view, row.profile ? "profileList" : "sessionList")
        if (row.profile) {
            view.beginRename("work", "A longer display name")
            view.feedback = receipt("one", "failed", "The profile change couldn't be completed. Review the details and try again.", Array(20).join("A long technical detail.\n"))
        } else view.saveFeedback = receipt("one", "failed", "The session couldn't be saved. Review the details and try again.", Array(20).join("A long technical detail.\n"))
        var feedback = findChild(view, row.profile ? "profileFeedback" : "sessionSaveFeedback")
        findChild(feedback, "managerFeedbackDetailsButton").click()
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
        if (testArtifactDirectory.length > 0) grabImage(host.contentItem).save(testArtifactDirectory + "/manager-" + row.tag + ".png")
    }
}
