import QtQuick
import QtQuick.Controls
import QtQuick.Window
import QtTest
import "../../qml/components"

TestCase {
    id: probe
    name: "VersioningPanel"
    when: windowShown
    width: 1000
    height: 760

    ApplicationWindow {
        id: testWindow
        width: probe.width
        height: probe.height
        visible: true
        font: Qt.font({ family: "Sans Serif", pointSize: 10 })
        property bool temporaryProfile: false
        property real chromeScale: 1
        property color panelColor: "#181825"
        property color backgroundColor: "#1e1e2e"
        property color borderColor: "#585b70"
        property color mutedTextColor: "#a6adc8"
        property color warningColor: "#f9e2af"
        property color errorColor: "#f38ba8"
        property color accentColor: "#89b4fa"
        property color primaryTextColor: "#cdd6f4"
        property color secondaryTextColor: "#bac2de"
        property string versioningOpenRestoreCommitId: ""
        property double versioningOpenRestoreOptionsAt: 0
        property bool versioningOpenRestoreMediaPositionsRequested: false
        property bool versioningOpenRestoreEditableDraftsRequested: false
    }

    function cleanup() {
        testWindow.width = probe.width
        testWindow.height = probe.height
    }

        QtObject {
            id: fakeUi
            property bool versioning_panel_visible: true
            property string versioning_panel_kind: "list"
            property string versioning_panel_data: "{}"
            property string versioning_archive_view_url: ""
            property string lastCommand: ""
            property string cancelledPolicyRequest: ""
            property string confirmedPolicyRequest: ""
            function execute_command(command) {
                lastCommand = String(command)
                return true
            }
            function cancel_versioned_policy_restore(requestId) {
                cancelledPolicyRequest = String(requestId)
                return true
            }
            function confirm_versioned_policy_restore(requestId) {
                confirmedPolicyRequest = String(requestId)
                return true
            }
            function has_native_page_capture_request() {
                return false
            }
            function has_active_versioning_operations() {
                return false
            }
            function request_native_page_capture_consent(editable, selection) {
                return true
            }
        }

        Component {
            id: panelFactory
            FerricVersioningPanel {
                browserWindow: testWindow
                browserUi: fakeUi
            }
        }

        function test_panel_close_clears_visibility_and_restore_state() {
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "list"
            fakeUi.versioning_panel_data = "{\"commits\":[]}"
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            tryVerify(function () {
                return panel.visible
            })
            panel.closePanel()
            tryVerify(function () {
                return fakeUi.versioning_panel_visible === false
            })
            verify(!panel.visible)
        }

        function test_capture_warning_describes_full_archive_sensitivity_without_sanitization_claim() {
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "none"
            fakeUi.versioning_panel_data = "{}"
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            var warning = findChild(panel, "versioningCaptureWarning")
            verify(warning)
            verify(warning.text.indexOf("personal data, secrets, and resource URLs") >= 0)
            verify(warning.text.indexOf("not sanitized") >= 0)
            verify(warning.text.indexOf("may omit resources") >= 0)
            verify(warning.text.indexOf("not guaranteed to be an atomic snapshot") >= 0)
            verify(warning.text.indexOf("not encrypted at rest") >= 0)
            verify(warning.text.indexOf("restrictive local permissions") >= 0)
            verify(warning.text.indexOf("same OS account or backups") >= 0)
            verify(warning.text.indexOf("one-use native confirmation") >= 0)
            verify(warning.text.indexOf("profile-keyed HMAC") >= 0)
            verify(warning.text.indexOf("not encryption") >= 0)
            verify(warning.text.indexOf("correlatable within a profile") >= 0)
            verify(warning.text.indexOf("unlocked system key store") >= 0)
            verify(warning.text.indexOf("media positions are omitted") >= 0)
            verify(warning.text.indexOf("separate opt-in") >= 0)
            verify(warning.text.indexOf("Live, DRM-protected, adaptive-streaming, and device-backed media positions are unsupported") >= 0)
            verify(warning.text.indexOf("automatic checkpoints never collect page archives") >= 0)
        }

        function test_page_archive_action_stays_disabled_before_release_qualification() {
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "none"
            fakeUi.versioning_panel_data = "{}"
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            var button = findChild(panel, "versioningPageArchiveRequestButton")
            verify(button)
            compare(panel.pageCaptureReleaseEnabled, false)
            compare(button.enabled, false)
            verify(button.text.indexOf("not released") >= 0)
            verify(button.Accessible.description.indexOf("may omit resources") >= 0)
            verify(button.Accessible.description.indexOf("atomic snapshots") >= 0)
            var editableOptIn = findChild(panel, "versioningEditableCaptureOptIn")
            var selectionOptIn = findChild(panel, "versioningSelectionCaptureOptIn")
            verify(editableOptIn)
            verify(selectionOptIn)
            compare(editableOptIn.checked, false)
            compare(editableOptIn.enabled, false)
            compare(selectionOptIn.checked, false)
            compare(selectionOptIn.enabled, false)
        }

        function test_panel_initial_focus_target_and_list_selection() {
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "list"
            fakeUi.versioning_panel_data = JSON.stringify({
                commits: [
                    {
                        id: "commit:12345678-1234-4234-8234-123456789abc",
                        createdAtMs: 0
                    },
                    {
                        id: "commit:abcdefab-cdef-4abc-8def-abcdefabcdef",
                        createdAtMs: 0
                    }
                ],
                offset: 0,
                limit: 100,
                returned: 2,
                hasNext: false
            })
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            var closeButton = findChild(panel, "versioningPanelCloseButton")
            var list = findChild(panel, "versioningCommitList")
            verify(closeButton)
            verify(list)
            compare(panel.initialFocusItem, list)
            compare(list.currentIndex, 0)
            list.forceActiveFocus()
            keyClick(Qt.Key_Down)
            tryCompare(list, "currentIndex", 1)
            compare(list.currentIndex, 1)
            list.currentIndex = 0
            compare(list.currentIndex, 0)
        }

        function test_empty_commit_list_explains_how_to_start() {
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "list"
            fakeUi.versioning_panel_data = JSON.stringify({
                commits: [],
                offset: 0,
                limit: 100,
                returned: 0,
                hasNext: false
            })
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            var emptyState = findChild(panel, "versioningCommitListEmptyState")
            verify(emptyState)
            tryVerify(function() {
                return emptyState.visible
            })
            compare(emptyState.textFormat, Text.PlainText)
            verify(emptyState.text.indexOf("No saved commits yet") >= 0)
            verify(emptyState.text.indexOf(":commit") >= 0)
            compare(emptyState.Accessible.name, "No saved commits")
            compare(panel.initialFocusItem, findChild(panel, "versioningPanelCloseButton"))

            fakeUi.versioning_panel_data = JSON.stringify({
                commits: [],
                offset: 100,
                limit: 100,
                returned: 0,
                hasNext: false
            })
            tryCompare(emptyState, "text", "No commits on this page. Go to the previous page or refresh the list.")
            compare(emptyState.Accessible.name, "No commits on this page")
        }

        function test_ambiguous_commit_prefix_cannot_dispatch_or_preview_restore() {
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "list"
            fakeUi.lastCommand = ""
            var prefix = "commit:12345678"
            fakeUi.versioning_panel_data = JSON.stringify({
                commits: [{ id: prefix, createdAtMs: 0 }],
                offset: 0, limit: 100, returned: 1, hasNext: false
            })
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            var detailsButton = findChild(panel, "versioningCommitDetailsButton")
            var restoreButton = findChild(panel, "versioningRestoreButton")
            verify(detailsButton)
            verify(restoreButton)
            compare(detailsButton.enabled, false)
            compare(restoreButton.enabled, false)
            compare(panel.inspectSelectedCommit(), false)
            panel.beginOpenLivePreview(prefix)
            compare(panel.openLivePreviewCommitId, "")
            compare(fakeUi.lastCommand, "")
        }

        function test_async_list_result_renders_saved_commit_after_panel_creation() {
            fakeUi.versioning_panel_visible = false
            fakeUi.versioning_panel_kind = "none"
            fakeUi.versioning_panel_data = "{}"
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            var list = findChild(panel, "versioningCommitList")
            verify(list)
            compare(list.count, 0)

            var id = "commit:12345678-1234-4234-8234-123456789abc"
            fakeUi.versioning_panel_kind = "list"
            fakeUi.versioning_panel_data = JSON.stringify({
                commits: [{ id: id, createdAtMs: 123456789, pinned: false, objectLinks: 0 }],
                returned: 1,
                offset: 0,
                limit: 10,
                hasNext: false
            })
            fakeUi.versioning_panel_visible = true

            tryCompare(list, "count", 1)
            tryVerify(function () { return panel.visible })
            compare(panel.commitRows[0].id, id)
            var label = findChild(panel, "versioningCommitId")
            verify(label)
            compare(label.text, id)
            compare(label.Accessible.name, "Commit identifier " + id)
            compare(panel.initialFocusItem, list)
        }

        function test_live_restore_buttons_activate_from_keyboard_focus() {
            fakeUi.versioning_panel_visible = true
            var id = "commit:12345678-1234-4234-8234-123456789abc"
            fakeUi.versioning_panel_kind = "list"
            fakeUi.versioning_panel_data = JSON.stringify({
                commits: [{
                    id: id,
                    createdAtMs: 1,
                    pinned: false,
                    objectLinks: 0
                }],
                offset: 0,
                limit: 100,
                returned: 1,
                hasNext: false
            })
            fakeUi.lastCommand = ""
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            var list = findChild(panel, "versioningCommitList")
            verify(list)

            testWindow.requestActivate()
            tryCompare(testWindow, "active", true)
            list.forceActiveFocus()
            tryVerify(function () {
                return list.activeFocus
            })
            keyClick(Qt.Key_Return)
            tryCompare(fakeUi, "lastCommand", "commit-info " + id)

            fakeUi.versioning_panel_kind = "info"
            fakeUi.versioning_panel_data = JSON.stringify({
                id: id,
                createdAtMs: 1,
                scope: "profile",
                pages: 1,
                replayablePages: 1,
                archiveObjects: 0,
                missingObjects: 0,
                quarantinedObjects: 0
            })
            fakeUi.lastCommand = ""
            tryCompare(panel, "panelKind", "info")
            var openLive = findChild(panel, "versioningOpenLivePreviewButton")
            verify(openLive)

            openLive.forceActiveFocus()
            tryVerify(function () {
                return openLive.activeFocus
            })
            keyClick(Qt.Key_Space)

            tryCompare(panel, "openLivePreviewCommitId", id)
            var confirm = findChild(panel, "versioningOpenLiveConfirmButton")
            verify(confirm)
            confirm.forceActiveFocus()
            tryVerify(function () {
                return confirm.activeFocus
            })
            keyClick(Qt.Key_Return)
            tryCompare(fakeUi, "lastCommand", "restore " + id)
        }

        function test_focus_follows_list_details_and_restore_review() {
            var id = "commit:12345678-1234-4234-8234-123456789abc"
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "list"
            fakeUi.versioning_panel_data = JSON.stringify({
                commits: [{ id: id, createdAtMs: 1 }],
                offset: 0,
                limit: 100,
                returned: 1,
                hasNext: false
            })
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            testWindow.requestActivate()
            tryCompare(testWindow, "active", true)
            var list = findChild(panel, "versioningCommitList")
            verify(list)
            tryVerify(function() { return list.activeFocus })

            fakeUi.versioning_panel_kind = "info"
            fakeUi.versioning_panel_data = JSON.stringify({ id: id, pages: 1 })
            var preview = findChild(panel, "versioningOpenLivePreviewButton")
            verify(preview)
            tryVerify(function() { return preview.activeFocus })

            panel.beginOpenLivePreview(id)
            var cancel = findChild(panel, "versioningOpenLiveCancelButton")
            verify(cancel)
            tryVerify(function() { return cancel.activeFocus })
            keyClick(Qt.Key_Return)
            tryCompare(panel, "openLivePreviewCommitId", "")
            tryVerify(function() { return preview.activeFocus })

            fakeUi.versioning_panel_kind = "list"
            fakeUi.versioning_panel_data = JSON.stringify({
                commits: [{ id: id, createdAtMs: 1 }],
                offset: 0,
                limit: 100,
                returned: 1,
                hasNext: false
            })
            tryVerify(function() { return list.activeFocus })
        }

        function test_live_restore_confirmation_fits_compact_window() {
            var id = "commit:12345678-1234-4234-8234-123456789abc"
            testWindow.width = 627
            testWindow.height = 694
            tryCompare(testWindow.contentItem, "width", 627)
            tryCompare(testWindow.contentItem, "height", 694)
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "info"
            fakeUi.versioning_panel_data = JSON.stringify({
                id: id,
                createdAtMs: 1,
                scope: "profile",
                pages: 1,
                replayablePages: 1,
                archiveObjects: 0,
                missingObjects: 0,
                quarantinedObjects: 0
            })
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            panel.beginOpenLivePreview(id)
            var confirm = findChild(panel, "versioningOpenLiveConfirmButton")
            var cancel = findChild(panel, "versioningOpenLiveCancelButton")
            verify(confirm)
            verify(cancel)
            tryVerify(function() { return confirm.visible && confirm.width > 0 })
            var position = confirm.mapToItem(testWindow.contentItem, 0, 0)
            var cancelPosition = cancel.mapToItem(testWindow.contentItem, 0, 0)
            var dialogPosition = panel.panelItem.mapToItem(testWindow.contentItem, 0, 0)
            verify(cancelPosition.x >= dialogPosition.x)
            verify(position.x >= dialogPosition.x)
            verify(position.x + confirm.width <= dialogPosition.x + panel.panelItem.width,
                   "restore confirmation must fit inside a compact dialog: x="
                   + position.x + " width=" + confirm.width
                   + " window=" + testWindow.width
                   + " body=" + panel.bodyItem.width
                   + " row=" + confirm.parent.width
                   + " column=" + confirm.parent.parent.width
                   + " root=" + confirm.parent.parent.parent.width)
            verify(position.y >= dialogPosition.y)
            verify(position.y + confirm.height <= dialogPosition.y + panel.panelItem.height)
            verify(cancelPosition.x + cancel.width <= dialogPosition.x + panel.panelItem.width)
        }

        function test_long_untrusted_label_is_plain_text_and_list_restore_routes_to_details() {
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "list"
            var id = "commit:12345678-1234-4234-8234-123456789abc"
            var hostile = new Array(401).join("<img src=x onerror=bad()>")
            fakeUi.versioning_panel_data = JSON.stringify({
                commits: [
                    {
                        id: hostile,
                        createdAtMs: 0,
                        pinned: false,
                        objectLinks: 0
                    }
                ],
                offset: 0,
                limit: 100,
                returned: 1,
                hasNext: false
            })
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            tryVerify(function () {
                return panel.visible
            })
            var idLabel = findChild(panel, "versioningCommitId")
            var restoreButton = findChild(panel, "versioningRestoreButton")
            verify(idLabel)
            verify(restoreButton)
            compare(idLabel.textFormat, Text.PlainText)
            compare(idLabel.text, hostile)
            compare(restoreButton.enabled, false)
            compare(panel.exactCommitId(id), true)
            compare(panel.exactCommitId(hostile), false)
            fakeUi.versioning_panel_data = JSON.stringify({
                commits: [
                    {
                        id: id,
                        createdAtMs: 0,
                        pinned: false,
                        objectLinks: 0
                    }
                ],
                offset: 0,
                limit: 100,
                returned: 1,
                hasNext: false
            })
            tryVerify(function () {
                var currentIdLabel = findChild(panel, "versioningCommitId")
                var currentRestoreButton = findChild(panel, "versioningRestoreButton")
                return currentIdLabel && currentIdLabel.text === id && currentRestoreButton && currentRestoreButton.enabled
            })
            idLabel = findChild(panel, "versioningCommitId")
            restoreButton = findChild(panel, "versioningRestoreButton")
            restoreButton.click()
            compare(fakeUi.lastCommand, "commit-info " + id)
            verify(fakeUi.lastCommand.indexOf("--into=current") < 0)
        }

        function test_open_live_is_separate_confirmed_normal_profile_action() {
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "info"
            var id = "commit:12345678-1234-4234-8234-123456789abc"
            fakeUi.versioning_panel_data = JSON.stringify({
                id: id,
                createdAtMs: 1,
                scope: "<img src=x onerror=bad()>",
                pages: 1,
                replayablePages: 1,
                archiveObjects: 1,
                missingObjects: 0,
                quarantinedObjects: 0
            })
            fakeUi.lastCommand = ""
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            var openLive = findChild(panel, "versioningOpenLivePreviewButton")
            var archive = findChild(panel, "versioningOpenArchiveButton")
            verify(openLive)
            verify(archive)
            compare(archive.enabled, false)
            var scope = findChild(panel, "versioningCommitScope")
            verify(scope)
            compare(scope.textFormat, Text.PlainText)
            compare(scope.text, "Scope: <img src=x onerror=bad()>")
            compare(archive.Accessible.name, "View the stored historical page archive")
            verify(archive.Accessible.description.indexOf("separate off-the-record profile") >= 0)
            mouseClick(openLive)
            compare(panel.openLivePreviewCommitId, id)
            compare(fakeUi.lastCommand, "")
            var mediaOptIn = findChild(panel, "versioningRestoreMediaPositionOptIn")
            verify(mediaOptIn)
            compare(mediaOptIn.checked, false)
            verify(mediaOptIn.Accessible.description.indexOf(
                       "Live, DRM-protected, adaptive-streaming, and device-backed media positions are unsupported") >= 0)
            verify(mediaOptIn.Accessible.description.indexOf("profile-keyed HMAC") >= 0)
            verify(mediaOptIn.Accessible.description.indexOf("not encryption") >= 0)
            verify(mediaOptIn.Accessible.description.indexOf("correlatable within a profile") >= 0)
            verify(mediaOptIn.Accessible.description.indexOf("key is unavailable") >= 0)
            mouseClick(mediaOptIn)
            var editableRestoreOptIn = findChild(panel, "versioningRestoreEditableDraftsOptIn")
            verify(editableRestoreOptIn)
            compare(editableRestoreOptIn.checked, false)
            mouseClick(editableRestoreOptIn)

            var openLiveConfirm = findChild(panel, "versioningOpenLiveConfirmButton")
            verify(openLiveConfirm)
            verify(openLiveConfirm.Accessible.description.indexOf("profile-keyed HMAC") >= 0)
            verify(openLiveConfirm.Accessible.description.indexOf("correlatable within a profile") >= 0)
            verify(openLiveConfirm.Accessible.description.indexOf("native-property refill") >= 0)

            var cancel = findChild(panel, "versioningOpenLiveCancelButton")
            verify(cancel)
            mouseClick(cancel)
            compare(panel.openLivePreviewCommitId, "")
            compare(panel.includePausedMediaPosition, false)
            compare(panel.includeEditableDraftRestore, false)
            compare(testWindow.versioningOpenRestoreMediaPositionsRequested, false)
            compare(testWindow.versioningOpenRestoreEditableDraftsRequested, false)
            compare(testWindow.versioningOpenRestoreCommitId, "")
            compare(testWindow.versioningOpenRestoreOptionsAt, 0)
            compare(fakeUi.lastCommand, "")

            mouseClick(openLive)
            var confirm = findChild(panel, "versioningOpenLiveConfirmButton")
            verify(confirm)
            verify(confirm.enabled)
            compare(mediaOptIn.checked, false)
            compare(editableRestoreOptIn.checked, false)
            mouseClick(mediaOptIn)
            mouseClick(editableRestoreOptIn)
            testWindow.requestActivate()
            tryCompare(testWindow, "active", true)
            confirm.forceActiveFocus()
            tryCompare(confirm, "activeFocus", true)
            mouseClick(confirm)
            compare(fakeUi.lastCommand, "restore " + id)
            compare(testWindow.versioningOpenRestoreMediaPositionsRequested, true)
            compare(testWindow.versioningOpenRestoreEditableDraftsRequested, true)
            compare(testWindow.versioningOpenRestoreCommitId, id)
            verify(testWindow.versioningOpenRestoreOptionsAt > 0)
            compare(panel.openLivePreviewCommitId, "")
        }

        function test_diff_pagination_uses_exact_ids_and_escapes_untrusted_identifier() {
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "diff"
            fakeUi.versioning_panel_data = JSON.stringify({
                kind: "page",
                page: {
                    next_offset: 100
                }
            })
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            panel.selectedLeft = "commit:12345678-1234-4234-8234-123456789abc"
            panel.selectedRight = "commit:abcdefab-cdef-4abc-8def-abcdefabcdef"
            panel.runDiff(100)
            compare(fakeUi.lastCommand, "diff " + panel.selectedLeft + " " + panel.selectedRight + " --kind=page --offset=100")
            panel.selectedLeft = "commit:x;restore commit:12345678-1234-4234-8234-123456789abc"
            fakeUi.lastCommand = ""
            panel.runDiff(0)
            compare(fakeUi.lastCommand, "")
        }

        function test_policy_restore_shows_only_allowlisted_diff_and_requires_confirmation() {
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "policy-preview"
            fakeUi.versioning_panel_data = JSON.stringify({
                requestId: "42",
                commit: "commit:12345678-1234-4234-8234-123456789abc",
                configurationRevision: 7,
                key: "content.javascript",
                currentEnabled: true,
                capturedEnabled: false
            })
            fakeUi.cancelledPolicyRequest = ""
            fakeUi.confirmedPolicyRequest = ""
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            tryVerify(function () {
                return panel.visible
            })
            var diff = findChild(panel, "versioningPolicyDiff")
            var confirmButton = findChild(panel, "versioningPolicyConfirmButton")
            verify(diff)
            verify(confirmButton)
            verify(diff.text.indexOf("Current: enabled") >= 0)
            verify(diff.text.indexOf("Captured: disabled") >= 0)
            mouseClick(confirmButton)
            compare(fakeUi.confirmedPolicyRequest, "42")
            fakeUi.versioning_panel_kind = "policy-applied"
            fakeUi.versioning_panel_data = "{}"
            fakeUi.versioning_panel_visible = false
            tryVerify(function () {
                return !panel.visible
            })
            fakeUi.versioning_panel_kind = "policy-preview"
            fakeUi.versioning_panel_data = JSON.stringify({
                requestId: "43"
            })
            fakeUi.versioning_panel_visible = true
            tryVerify(function () {
                return panel.visible
            })
            panel.closePanel()
            compare(fakeUi.cancelledPolicyRequest, "43")
            verify(!fakeUi.versioning_panel_visible)
        }

        function test_keyboard_focus_exposes_inspect_restore_and_escape_controls() {
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "list"
            var id = "commit:12345678-1234-4234-8234-123456789abc"
            fakeUi.versioning_panel_data = JSON.stringify({
                commits: [
                    {
                        id: id,
                        createdAtMs: 0
                    }
                ],
                offset: 0,
                limit: 100,
                returned: 1,
                hasNext: false
            })
            fakeUi.lastCommand = ""
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            tryVerify(function () {
                return panel.visible
            })
            testWindow.requestActivate()
            tryCompare(testWindow, "active", true)

            var detailsButton = findChild(panel, "versioningCommitDetailsButton")
            verify(detailsButton)
            verify(detailsButton.enabled)
            verify(detailsButton.visible)
            verify(detailsButton.activeFocusOnTab)
            compare(detailsButton.Accessible.name, "Inspect commit " + id)
            detailsButton.forceActiveFocus()
            tryVerify(function () {
                return detailsButton.activeFocus
            })
            compare(testWindow.activeFocusItem, detailsButton)

            fakeUi.versioning_panel_kind = "info"
            fakeUi.versioning_panel_data = JSON.stringify({
                id: id,
                pages: 1
            })
            var openLive = findChild(panel, "versioningOpenLivePreviewButton")
            tryVerify(function () {
                return openLive && openLive.visible && openLive.enabled
            })
            verify(openLive.activeFocusOnTab)
            compare(openLive.Accessible.name, "Preview opening current live pages from this saved commit")
            openLive.forceActiveFocus()
            tryVerify(function () {
                return openLive.activeFocus
            })
            panel.beginOpenLivePreview(id)
            var confirm = findChild(panel, "versioningOpenLiveConfirmButton")
            tryVerify(function () {
                return confirm && confirm.visible && confirm.enabled
            })
            verify(confirm.activeFocusOnTab)
            confirm.forceActiveFocus()
            tryVerify(function () {
                return confirm.activeFocus
            })

            keyClick(Qt.Key_Escape)
            tryVerify(function () {
                return !fakeUi.versioning_panel_visible
            })
        }

        function test_untrusted_payload_text_is_plain_and_display_rows_are_bounded() {
            var hostile = "<img src=x onerror=bad()>"
            fakeUi.versioning_panel_visible = true
            fakeUi.versioning_panel_kind = "error"
            fakeUi.versioning_panel_data = JSON.stringify({
                message: hostile
            })
            var panel = createTemporaryObject(panelFactory, testWindow)
            verify(panel)
            var errorMessage = findChild(panel, "versioningErrorMessage")
            verify(errorMessage)
            compare(errorMessage.textFormat, Text.PlainText)
            compare(errorMessage.text, hostile)

            var commits = []
            var events = []
            for (var i = 0; i < 150; ++i) {
                commits.push({
                    id: hostile + i,
                    createdAtMs: 0
                })
                events.push({
                    change: hostile + i,
                    window: "window-" + i
                })
            }
            fakeUi.versioning_panel_kind = "diff"
            fakeUi.versioning_panel_data = JSON.stringify({
                kind: hostile,
                unsupportedCategories: [hostile],
                page: {
                    events: events,
                    total_events: events.length,
                    next_offset: null
                }
            })
            tryVerify(function () {
                return panel.panelKind === "diff"
            })
            var categoryLabel = findChild(panel, "versioningUnsupportedCategories")
            var changesList = findChild(panel, "versioningChangesList")
            verify(categoryLabel)
            verify(changesList)
            compare(categoryLabel.textFormat, Text.PlainText)
            verify(categoryLabel.text.indexOf("Unsupported: ") === 0)
            verify(categoryLabel.text.indexOf(hostile) > 0)
            compare(changesList.count, 100)
            compare(panel.boundedRows(commits).length, 100)
            compare(panel.boundedRows({
                length: 1000
            }).length, 0)
        }
}
