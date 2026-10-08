import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: panel
    required property var browserUi
    readonly property string panelKind: String(browserUi.versioning_panel_kind || "none")
    readonly property var payload: {
        try {
            return JSON.parse(String(browserUi.versioning_panel_data || "{}"))
        } catch (error) {
            return ({ message: "Versioning results could not be displayed." })
        }
    }
    property string selectedLeft: ""
    property string selectedRight: ""
    property string openLivePreviewCommitId: ""
    property bool includePausedMediaPosition: false
    property bool includeEditableDraftRestore: false
    property int listLimit: 100
    property int lastListOffset: 0
    property var commitRows: []
    // VB-057 must pass the release qualification before this UI action is
    // enabled. Its implementation is present behind this fail-closed gate.
    readonly property bool pageCaptureReleaseEnabled: false
    // VB-057 keeps historical rendering disabled until the offline boundary
    // and archive object materialization path are qualified together.
    readonly property bool archiveOfflineReleaseEnabled: false
    function boundedRows(value) {
        return Array.isArray(value) ? value.slice(0, listLimit) : []
    }
    function unsupportedCategorySummary(value) {
        if (!Array.isArray(value) || value.length === 0)
            return ""
        var categories = []
        var count = Math.min(value.length, 16)
        for (var i = 0; i < count; ++i) {
            if (typeof value[i] === "string")
                categories.push(value[i].slice(0, 96))
        }
        var suffix = value.length > count ? " · additional unsupported categories omitted" : ""
        var summary = categories.join(" · ").slice(0, 1536)
        return summary.length > 0 ? "Unsupported: " + summary + suffix : ""
    }
    onPayloadChanged: {
        commitRows = boundedRows(payload.commits)
        if (commitsList) {
            commitsList.model = null
            commitsList.model = commitRows
        }
        if (panelKind === "list") {
            lastListOffset = Math.max(0, Number(payload.offset || 0))
        }
    }
    onPanelKindChanged: Qt.callLater(function() {
        if (!panel.visible)
            return
        if (panel.panelKind === "list" && panel.commitRows.length > 0)
            commitsList.forceActiveFocus()
        else if (panel.panelKind === "info"
                 && panel.openLivePreviewCommitId.length === 0
                 && openLivePreviewButton.enabled)
            openLivePreviewButton.forceActiveFocus()
    })
    onOpenLivePreviewCommitIdChanged: Qt.callLater(function() {
        if (!panel.visible || panel.panelKind !== "info")
            return
        if (panel.openLivePreviewCommitId.length > 0)
            openLiveCancelButton.forceActiveFocus()
        else if (openLivePreviewButton.enabled)
            openLivePreviewButton.forceActiveFocus()
    })
    Component.onCompleted: {
        commitRows = boundedRows(payload.commits)
        commitsList.model = commitRows
    }

    visible: browserUi.versioning_panel_visible
             && String(browserUi.versioning_archive_view_url || "").length === 0
    commandText: ":commit-list"
    title: "Versioning"
    keyHelp: "Tab navigate · Esc close"
    dialogWidth: 920 * scale
    dialogHeight: 660 * scale
    stackingOrder: 72
    initialFocusItem: panel.panelKind === "list" && panel.commitRows.length > 0
                      ? commitsList : closeButton
    onDismissRequested: closePanel()
    Accessible.description: "Inspect saved structural commits and bounded differences. Sensitive page archives are not currently available and are never described as sanitized."

    function closePanel() {
        if (panel.panelKind === "delete-preview"
                && String(panel.payload.requestId || "").length > 0) {
            browserUi.cancel_versioned_commit_delete(
                String(panel.payload.requestId))
        }
        if (panel.panelKind === "policy-preview"
                && String(panel.payload.requestId || "").length > 0) {
            browserUi.cancel_versioned_policy_restore(
                String(panel.payload.requestId))
        }
        openLivePreviewCommitId = ""
        includePausedMediaPosition = false
        includeEditableDraftRestore = false
        panel.browserWindow.versioningOpenRestoreCommitId = ""
        panel.browserWindow.versioningOpenRestoreOptionsAt = 0
        panel.browserWindow.versioningOpenRestoreMediaPositionsRequested = false
        panel.browserWindow.versioningOpenRestoreEditableDraftsRequested = false
        browserUi.versioning_panel_visible = false
    }

    function run(command) {
        if (browserUi.execute_command(command)) {
            return true
        }
        return false
    }

    function inspectSelectedCommit() {
        var index = commitsList.currentIndex
        if (index < 0 || index >= commitRows.length) {
            return false
        }
        var commitId = String(commitRows[index].id || "")
        if (!exactCommitId(commitId)) {
            return false
        }
        return panel.run("commit-info " + commitId)
    }

    function handleFocusedButtonKey(button, event, action) {
        if (!button.activeFocus || !button.enabled
                || [Qt.Key_Space, Qt.Key_Return, Qt.Key_Enter].indexOf(event.key) < 0) {
            return
        }
        event.accepted = true
        if (!event.isAutoRepeat)
            action()
    }

    function exactCommitId(value) {
        return /^commit:[0-9a-f]{8}-[0-9a-f]{4}-[4][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(String(value || ""))
    }

    function runDiff(offset, requestedKind) {
        if (!exactCommitId(selectedLeft) || !exactCommitId(selectedRight)) {
            return
        }
        var kind = String(requestedKind || payload.kind || "browser")
        if (["browser", "page", "cookies"].indexOf(kind) < 0) {
            kind = "browser"
        }
        run("diff " + selectedLeft + " " + selectedRight
            + " --kind=" + kind + " --offset=" + Math.max(0, Number(offset) || 0))
    }

    function pageList(offset) {
        run("commit-list --limit=" + listLimit + " --offset=" + Math.max(0, Number(offset) || 0))
    }

    function beginOpenLivePreview(commitId) {
        var id = String(commitId || "")
        if (exactCommitId(id)) {
            panel.browserWindow.versioningOpenRestoreCommitId = ""
            panel.browserWindow.versioningOpenRestoreOptionsAt = 0
            panel.browserWindow.versioningOpenRestoreMediaPositionsRequested = false
            panel.browserWindow.versioningOpenRestoreEditableDraftsRequested = false
            includePausedMediaPosition = false
            includeEditableDraftRestore = false
            openLivePreviewCommitId = id
        }
    }

    function confirmOpenLive() {
        var id = openLivePreviewCommitId
        if (!exactCommitId(id))
            return
        panel.browserWindow.versioningOpenRestoreCommitId = id
        panel.browserWindow.versioningOpenRestoreOptionsAt = Date.now()
        panel.browserWindow.versioningOpenRestoreMediaPositionsRequested =
                includePausedMediaPosition
        panel.browserWindow.versioningOpenRestoreEditableDraftsRequested =
                includeEditableDraftRestore
        if (run("restore " + id)) {
            openLivePreviewCommitId = ""
        } else {
            panel.browserWindow.versioningOpenRestoreCommitId = ""
            panel.browserWindow.versioningOpenRestoreOptionsAt = 0
            panel.browserWindow.versioningOpenRestoreMediaPositionsRequested = false
            panel.browserWindow.versioningOpenRestoreEditableDraftsRequested = false
        }
    }

    function coverageSummary(value) {
        var counts = value || ({})
        return "unknown " + Number(counts.unknown || 0)
            + " · not requested " + Number(counts.notRequested || 0)
            + " · unavailable " + Number(counts.unavailable || 0)
            + " · partial " + Number(counts.partial || 0)
            + " · complete " + Number(counts.completeForRequest || 0)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 10

        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: panel.panelKind === "list" ? "Saved commits"
                    : panel.panelKind === "info" && panel.openLivePreviewCommitId.length > 0
                        ? "Open live pages"
                    : panel.panelKind === "info" ? "Commit details"
                    : panel.panelKind === "diff" ? "Structural difference"
                    : panel.panelKind === "error" ? "Versioning result"
                    : "Versioning"
                font.bold: true
                Accessible.role: Accessible.Heading
            }
            Label {
                text: panel.keyHelp
                color: panel.browserWindow.mutedTextColor
            }
            Button {
                id: closeButton
                objectName: "versioningPanelCloseButton"
                text: "Close"
                Accessible.name: "Close versioning panel"
                onClicked: panel.closePanel()
            }
        }

        Label {
            objectName: "versioningCaptureWarning"
            Layout.fillWidth: true
            visible: ["none", "list", "info"].indexOf(panel.panelKind) >= 0
            text: "Use the universal switcher to run :commit-list. Select a commit to inspect or restore it. Page archives are not released yet. When enabled, each explicit request will require one-use native confirmation. An archive may contain personal data, secrets, and resource URLs; it is not sanitized, may omit resources, and is not guaranteed to be an atomic snapshot. Bounded media position and paused state may also be stored and are never resumed automatically. For supported media, sidecar metadata stores a profile-keyed HMAC of the source URL for matching. This is not encryption; matching sources remain correlatable within a profile, and a process with access to the unlocked system key store may recover the same URL equality information. If the system key store is unavailable, media positions are omitted. Live, DRM-protected, adaptive-streaming, and device-backed media positions are unsupported and are not captured or sought. Files use restrictive local permissions, but are not encrypted at rest and may be readable by the same OS account or backups. Editable capture is a separate opt-in. Private/ephemeral captures are not persisted, and automatic checkpoints never collect page archives."
            wrapMode: Text.Wrap
            color: panel.browserWindow.warningColor
        }

        CheckBox {
            id: includeEditableDrafts
            objectName: "versioningEditableCaptureOptIn"
            Layout.alignment: Qt.AlignLeft
            enabled: panel.pageCaptureReleaseEnabled
                     && !panel.browserWindow.temporaryProfile
            checked: false
            text: "Also include visible editable drafts (may contain secrets)"
            Accessible.name: "Separately opt in to capture visible editable drafts"
            Accessible.description: "Off by default. Editable text and checkbox state require separate one-use native confirmation and remain unavailable until release qualification."
        }

        CheckBox {
            id: includeSelectionPosition
            objectName: "versioningSelectionCaptureOptIn"
            Layout.alignment: Qt.AlignLeft
            enabled: panel.pageCaptureReleaseEnabled
                     && !panel.browserWindow.temporaryProfile
            checked: false
            text: "Also include current text-selection position (not selected text)"
            Accessible.name: "Separately opt in to capture the current text-selection position"
            Accessible.description: "Off by default. Selection text is not stored. A one-use native confirmation is required."
        }

        Button {
            objectName: "versioningPageArchiveRequestButton"
            Layout.alignment: Qt.AlignLeft
            text: panel.pageCaptureReleaseEnabled
                  ? "Archive current page…" : "Page archive capture is not released"
            enabled: panel.pageCaptureReleaseEnabled
                     && !panel.browserWindow.temporaryProfile
                     && !panel.browserUi.has_native_page_capture_request()
                     && !panel.browserUi.has_active_versioning_operations()
            Accessible.name: "Request an explicit archive of the current page"
            Accessible.description: (includeEditableDrafts.checked || includeSelectionPosition.checked)
                ? "Archives may contain secrets and resource URLs, may omit resources, are not guaranteed to be atomic snapshots, and are not sanitized. Selected optional page-state categories are also requested. A native confirmation is required."
                : "Archives may contain secrets and resource URLs, may omit resources, are not guaranteed to be atomic snapshots, and are not sanitized. A native confirmation is required."
            onClicked: {
                if (panel.browserUi.request_native_page_capture_consent(
                            includeEditableDrafts.checked,
                            includeSelectionPosition.checked)) {
                    panel.browserWindow.scheduleRuntimeWork(0)
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: panel.panelKind === "policy-preview"
            spacing: 10
            Label {
                text: "Review configuration policy"
                font.bold: true
                Accessible.role: Accessible.Heading
            }
            Label {
                Layout.fillWidth: true
                text: "Commit " + String(panel.payload.commit || "")
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
            }
            Label {
                Layout.fillWidth: true
                text: "Configuration revision " + Number(panel.payload.configurationRevision || 0)
                    + " · " + String(panel.payload.key || "content.javascript")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
            }
            Label {
                objectName: "versioningPolicyDiff"
                Layout.fillWidth: true
                text: "Current: " + (panel.payload.currentEnabled ? "enabled" : "disabled")
                    + " · Captured: " + (panel.payload.capturedEnabled ? "enabled" : "disabled")
                wrapMode: Text.Wrap
            }
            Label {
                Layout.fillWidth: true
                text: "Only the listed setting is eligible. This explicit action does not restore permission grants, trust exceptions, external command paths, passwords, or other security policy. The current configuration revision is rechecked before applying."
                wrapMode: Text.Wrap
                color: panel.browserWindow.warningColor
            }
            Item { Layout.fillHeight: true }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Button {
                    objectName: "versioningPolicyCancelButton"
                    text: "Cancel"
                    onClicked: panel.closePanel()
                }
                Button {
                    objectName: "versioningPolicyConfirmButton"
                    text: "Apply setting and restore"
                    Accessible.name: "Apply the displayed policy setting and continue restore"
                    onClicked: browserUi.confirm_versioned_policy_restore(
                                   String(panel.payload.requestId || ""))
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: panel.panelKind === "policy-applied"
            Label {
                Layout.fillWidth: true
                text: "The allowlisted policy setting was applied through current configuration validation. The structural restore is being opened in a new window."
                wrapMode: Text.Wrap
                Accessible.role: Accessible.AlertMessage
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: panel.panelKind === "delete-preview"
            spacing: 10
            Label {
                text: "Review commit deletion"
                font.bold: true
                Accessible.role: Accessible.Heading
            }
            Label {
                Layout.fillWidth: true
                text: "Commit " + String(panel.payload.id || "")
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
            }
            Label {
                Layout.fillWidth: true
                text: (panel.payload.pinned ? "Pinned commit — deletion blocked. " : "")
                    + Number(panel.payload.childCommits || 0) + " child commits · "
                    + Number(panel.payload.namedReferences || 0) + " named references · "
                    + Number(panel.payload.activeOperations || 0) + " active operations"
                wrapMode: Text.Wrap
            }
            Label {
                Layout.fillWidth: true
                text: Number(panel.payload.objectLinks || 0) + " object links · "
                    + Number(panel.payload.sharedObjects || 0) + " shared objects remain · "
                    + Number(panel.payload.reclaimableObjects || 0) + " objects / "
                    + Number(panel.payload.reclaimableBytes || 0) + " bytes may become reclaimable"
                wrapMode: Text.Wrap
            }
            Label {
                Layout.fillWidth: true
                text: "Deleting removes this commit from Ferric's versioned history. It is not secure erasure; filesystem snapshots, backups, and other copies may remain."
                wrapMode: Text.Wrap
                color: panel.browserWindow.warningColor
            }
            Item { Layout.fillHeight: true }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Button {
                    text: "Cancel"
                    onClicked: {
                        browserUi.cancel_versioned_commit_delete(
                            String(panel.payload.requestId || ""))
                        panel.pageList(panel.lastListOffset)
                    }
                }
                Button {
                    text: "Delete commit"
                    enabled: panel.payload.canDelete === true
                    Accessible.name: "Confirm deletion of commit " + String(panel.payload.id || "")
                    onClicked: browserUi.confirm_versioned_commit_delete(
                                   String(panel.payload.requestId || ""))
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: panel.panelKind === "delete-pending"
            Label {
                Layout.fillWidth: true
                text: "Revalidating the exact profile, commit, references, and active operations before deletion."
                wrapMode: Text.Wrap
                Accessible.role: Accessible.AlertMessage
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: panel.panelKind === "delete-complete"
            Label {
                Layout.fillWidth: true
                text: "Commit removed from versioned history."
                font.bold: true
                Accessible.role: Accessible.AlertMessage
            }
            Label {
                Layout.fillWidth: true
                text: Number(panel.payload.reclaimableObjects || 0) + " objects / "
                    + Number(panel.payload.reclaimableBytes || 0) + " bytes no longer have commit references. "
                    + (panel.payload.cleanupPending === true
                        ? "Safe quarantine needs a retry. "
                        : Number(panel.payload.quarantinedOrphanObjects || 0)
                            + " orphan objects and "
                            + Number(panel.payload.quarantinedCorruptObjects || 0)
                            + " corrupt objects were moved to private quarantine. ")
                    + "This is not forensic erasure."
                wrapMode: Text.Wrap
            }
            Button {
                text: "Back to commits"
                onClicked: panel.pageList(panel.lastListOffset)
            }
        }

        RowLayout {
            Layout.fillWidth: true
            visible: panel.panelKind === "list"
            Label {
                Layout.fillWidth: true
                text: "Page starting at " + Number(panel.payload.offset || 0)
                    + " · " + Number(panel.payload.returned || 0) + " commits"
                Accessible.role: Accessible.AlertMessage
            }
            Button {
                text: "Previous commits"
                enabled: Number(panel.payload.offset || 0) > 0
                onClicked: panel.pageList(Math.max(0, Number(panel.payload.offset || 0) - panel.listLimit))
            }
            Button {
                text: "Next commits"
                enabled: panel.payload.hasNext === true
                onClicked: panel.pageList(Number(panel.payload.offset || 0) + panel.listLimit)
            }
        }

        ListView {
            id: commitsList
            objectName: "versioningCommitList"
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: panel.panelKind === "list"
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            keyNavigationEnabled: true
            activeFocusOnTab: true
            Accessible.name: "Saved commits"
            Keys.onReturnPressed: function(event) {
                if (panel.inspectSelectedCommit()) {
                    event.accepted = true
                }
            }
            Keys.onEnterPressed: function(event) {
                if (panel.inspectSelectedCommit()) {
                    event.accepted = true
                }
            }
            Accessible.description: "Use Up and Down to select a saved commit. Each row has detail, comparison, and restore actions."
            delegate: Rectangle {
                required property var modelData
                required property int index
                width: commitsList.width
                height: Math.max(92, actionRow.implicitHeight + 22)
                color: index % 2 === 0 ? panel.browserWindow.panelColor : panel.browserWindow.backgroundColor
                border.color: panel.browserWindow.borderColor
                radius: 4

                ColumnLayout {
                    id: actionRow
                    anchors.fill: parent
                    anchors.margins: 8
                    spacing: 5
                    RowLayout {
                        Layout.fillWidth: true
                        Label {
                            objectName: "versioningCommitId"
                            Layout.fillWidth: true
                            text: String(modelData.id || "")
                            textFormat: Text.PlainText
                            elide: Text.ElideMiddle
                            font.family: "monospace"
                            Accessible.name: "Commit identifier " + String(modelData.id || "")
                        }
                        Label {
                            text: Number(modelData.createdAtMs || 0) > 0
                                ? new Date(Number(modelData.createdAtMs)).toLocaleString() : "Time unavailable"
                            color: panel.browserWindow.mutedTextColor
                        }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Label {
                            Layout.fillWidth: true
                            text: (modelData.pinned ? "Pinned" : "Not pinned")
                                + " · " + Number(modelData.objectLinks || 0) + " stored objects"
                            color: panel.browserWindow.mutedTextColor
                        }
                        Button {
                            objectName: "versioningCommitDetailsButton"
                            text: "Details"
                            Accessible.name: "Inspect commit " + String(modelData.id || "")
                            enabled: panel.exactCommitId(modelData.id)
                            onClicked: panel.run("commit-info " + String(modelData.id || ""))
                        }
                        Button {
                            text: "Use as left"
                            onClicked: panel.selectedLeft = String(modelData.id || "")
                            Accessible.name: "Select commit as left side of comparison"
                        }
                        Button {
                            text: "Use as right"
                            onClicked: panel.selectedRight = String(modelData.id || "")
                            Accessible.name: "Select commit as right side of comparison"
                        }
                        Button {
                            text: "Delete"
                            Accessible.name: "Review deletion of commit " + String(modelData.id || "")
                            enabled: panel.exactCommitId(modelData.id)
                            onClicked: panel.run("commit-delete " + String(modelData.id || ""))
                        }
                        Button {
                            objectName: "versioningRestoreButton"
                            text: "Review restore options"
                            Accessible.name: "Review live restore options for this commit"
                            enabled: panel.exactCommitId(modelData.id)
                            onClicked: panel.run("commit-info " + String(modelData.id || ""))
                        }
                    }
                }
            }
        }

        Label {
            id: emptyCommitListState
            objectName: "versioningCommitListEmptyState"
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: panel.panelKind === "list" && panel.commitRows.length === 0
            text: Number(panel.payload.offset || 0) > 0
                  ? "No commits on this page. Go to the previous page or refresh the list."
                  : "No saved commits yet. Run :commit from a normal profile to save a structural snapshot."
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            verticalAlignment: Text.AlignVCenter
            Accessible.name: Number(panel.payload.offset || 0) > 0
                              ? "No commits on this page" : "No saved commits"
            Accessible.description: Number(panel.payload.offset || 0) > 0
                                    ? "Use Previous commits or refresh the list."
                                    : "A structural snapshot can be saved from a normal profile with the commit command."
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: panel.panelKind === "info" && panel.openLivePreviewCommitId.length === 0
            spacing: 8
            Label {
                text: "Commit: " + String(panel.payload.id || "")
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
            }
            Label {
                text: Number(panel.payload.createdAtMs || 0) > 0
                    ? "Recorded " + new Date(Number(panel.payload.createdAtMs)).toLocaleString()
                    : "Recording time unavailable"
                textFormat: Text.PlainText
            }
            Label {
                objectName: "versioningCommitScope"
                text: "Scope: " + String(panel.payload.scope || "unknown")
                textFormat: Text.PlainText
            }
            Label { text: Number(panel.payload.windows || 0) + " windows · "
                    + Number(panel.payload.pages || 0) + " pages · "
                    + Number(panel.payload.replayablePages || 0) + " replayable URLs" }
            Label { text: Number(panel.payload.unavailableUrls || 0) + " URLs unavailable · "
                    + Number(panel.payload.objectLinks || 0) + " object links" }
            Label { text: "Archive objects: " + Number(panel.payload.archiveObjects || 0)
                    + " · page sidecars: " + Number(panel.payload.sidecarObjects || 0) }
            Label {
                visible: Number(panel.payload.missingObjects || 0) > 0
                    || Number(panel.payload.quarantinedObjects || 0) > 0
                text: Number(panel.payload.missingObjects || 0) + " missing objects · "
                    + Number(panel.payload.quarantinedObjects || 0) + " quarantined objects"
                color: panel.browserWindow.warningColor
            }
            Label {
                Layout.fillWidth: true
                text: "Archive coverage — " + panel.coverageSummary(panel.payload.coverage && panel.payload.coverage.archive)
                wrapMode: Text.Wrap
            }
            Label {
                Layout.fillWidth: true
                text: "Page capture coverage — " + panel.coverageSummary(panel.payload.coverage && panel.payload.coverage.page)
                wrapMode: Text.Wrap
            }
            Label {
                Layout.fillWidth: true
                text: "Cookie capture coverage — " + panel.coverageSummary(panel.payload.coverage && panel.payload.coverage.cookies)
                wrapMode: Text.Wrap
            }
            Label {
                Layout.fillWidth: true
                text: "Unsupported: policy comparison, archived-document comparison, DOM content, form values, and cookie identities/values are not shown here."
                wrapMode: Text.Wrap
                color: panel.browserWindow.warningColor
            }
            Label { text: Number(panel.payload.warningCount || 0) + " warnings recorded" }
            Label {
                Layout.fillWidth: true
                Layout.fillHeight: true
                verticalAlignment: Text.AlignTop
                wrapMode: Text.Wrap
                text: "Messages, titles, URLs, page bodies, and cookie values are omitted from this detail view."
                color: panel.browserWindow.mutedTextColor
            }
            Button {
                objectName: "versioningOpenLivePreviewButton"
                text: "Open live pages…"
                enabled: panel.exactCommitId(panel.payload.id)
                Accessible.name: "Preview opening current live pages from this saved commit"
                onClicked: panel.beginOpenLivePreview(panel.payload.id)
                Keys.priority: Keys.BeforeItem
                Keys.onPressed: function(event) {
                    panel.handleFocusedButtonKey(openLivePreviewButton, event, function() {
                        panel.beginOpenLivePreview(panel.payload.id)
                    })
                }
                id: openLivePreviewButton
            }
            Button {
                objectName: "versioningOpenArchiveButton"
                text: panel.archiveOfflineReleaseEnabled
                    ? "View stored page archive" : "Stored page viewing is not released"
                enabled: panel.archiveOfflineReleaseEnabled
                    && Number(panel.payload.archiveObjects || 0) > 0
                    && Number(panel.payload.missingObjects || 0) === 0
                    && Number(panel.payload.quarantinedObjects || 0) === 0
                Accessible.name: "View the stored historical page archive"
                Accessible.description: "Uses a separate off-the-record profile and never navigates the normal browser profile."
                onClicked: {
                    if (panel.browserUi.request_versioned_archive_view(panel.payload.id))
                        panel.browserWindow.scheduleRuntimeWork(0)
                }
            }
            Button {
                text: "Back to commits"
                onClicked: panel.pageList(panel.lastListOffset)
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            // Long wrapped copy must not expand the confirmation controls
            // beyond the dialog on compact native windows.
            width: parent.width
            visible: panel.panelKind === "info" && panel.openLivePreviewCommitId.length > 0
            spacing: 10
            Label {
                text: "Confirm opening live pages"
                font.bold: true
                Accessible.role: Accessible.Heading
            }
            Label {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                text: panel.includeEditableDraftRestore
                    ? "This opens current live pages in a new normal-profile window and may send network requests. Separately opted-in saved drafts may be assigned only to unique matching, still-blank visible text fields or unchecked checkboxes. It does not restore page contents or submit forms."
                    : "This opens current live pages in a new normal-profile window and may send network requests. It does not display the stored archive or restore page contents, forms, or secrets."
                wrapMode: Text.Wrap
                color: panel.browserWindow.warningColor
            }
            CheckBox {
                id: restorePausedMediaPosition
                objectName: "versioningRestoreMediaPositionOptIn"
                Layout.fillWidth: true
                checked: panel.includePausedMediaPosition
                text: "Also restore compatible paused media positions"
                Accessible.name: "Opt in to restoring compatible paused media positions"
                Accessible.description: "Off by default. Only uniquely matched media paused both when saved and now may be seeked once. Saved media matching uses a profile-keyed HMAC of supported source URLs in sidecar metadata; it is not encryption, and matching sources remain correlatable within a profile or to a process with access to the unlocked system key store. If the profile key is unavailable, media positions are omitted or skipped. Live, DRM-protected, adaptive-streaming, and device-backed media positions are unsupported and are not captured or sought. Seeking supported media can request network segments; Ferric will never start or resume playback."
                onToggled: panel.includePausedMediaPosition = checked
            }
            CheckBox {
                id: restoreEditableDraftValues
                objectName: "versioningRestoreEditableDraftsOptIn"
                Layout.fillWidth: true
                checked: panel.includeEditableDraftRestore
                text: "Also refill saved drafts into matching blank fields"
                Accessible.name: "Opt in to refilling saved editable drafts"
                Accessible.description: "Off by default. Only unique visible text/search inputs, textareas and unchecked checkboxes qualify. Password, hidden/token, radio, rich text and nonblank fields are not changed. Ferric sets native properties only, dispatches no input/change/submit events, and verifies each assignment once."
                onToggled: panel.includeEditableDraftRestore = checked
            }
            Label {
                Layout.fillWidth: true
                text: "Commit " + panel.openLivePreviewCommitId
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
            }
            Item { Layout.fillHeight: true }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Button {
                    objectName: "versioningOpenLiveCancelButton"
                    text: "Cancel"
                    onClicked: {
                        panel.includePausedMediaPosition = false
                        panel.includeEditableDraftRestore = false
                        panel.openLivePreviewCommitId = ""
                    }
                    Keys.priority: Keys.BeforeItem
                    Keys.onPressed: function(event) {
                        panel.handleFocusedButtonKey(openLiveCancelButton, event, function() {
                            panel.includePausedMediaPosition = false
                            panel.includeEditableDraftRestore = false
                            panel.openLivePreviewCommitId = ""
                        })
                    }
                    id: openLiveCancelButton
                }
                Button {
                    objectName: "versioningOpenLiveConfirmButton"
                    text: "Open live pages in new window"
                    Accessible.name: "Confirm opening current live pages in a new normal-profile window"
                    Accessible.description: {
                        var disclosures = []
                        if (panel.includePausedMediaPosition) {
                            disclosures.push("Also allow a one-shot position seek for uniquely matched media paused both at capture and now. No playback is started or resumed. Matching uses a profile-keyed HMAC of supported source URLs, not encryption; source equality remains correlatable within a profile or to a process with access to the unlocked system key store. Media positions are skipped when the key is unavailable.")
                        }
                        if (panel.includeEditableDraftRestore) {
                            disclosures.push("Also permit a bounded native-property refill into unique, matching blank controls; no events are dispatched.")
                        }
                        return disclosures.length > 0
                            ? disclosures.join(" ")
                            : "Open current live pages only; do not apply saved media positions or editable drafts."
                    }
                    enabled: panel.exactCommitId(panel.openLivePreviewCommitId)
                    onClicked: panel.confirmOpenLive()
                    Keys.priority: Keys.BeforeItem
                    Keys.onPressed: function(event) {
                        panel.handleFocusedButtonKey(openLiveConfirmButton, event, function() {
                            panel.confirmOpenLive()
                        })
                    }
                    id: openLiveConfirmButton
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: panel.panelKind === "diff"
            spacing: 8
            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: "Category: " + String(panel.payload.kind || "structural")
                    textFormat: Text.PlainText
                }
                Label { text: Number(panel.payload.page && panel.payload.page.total_events || 0) + " changes" }
                Button { text: "Browser"; onClicked: panel.runDiff(0, "browser") }
                Button { text: "Page"; onClicked: panel.runDiff(0, "page") }
                Button { text: "Cookies"; onClicked: panel.runDiff(0, "cookies") }
                Button { text: "Back to commits"; onClicked: panel.pageList(panel.lastListOffset) }
                Button {
                    text: "Previous changes"
                    enabled: Number(panel.payload.offset || 0) > 0
                    onClicked: panel.runDiff(Math.max(0, Number(panel.payload.offset || 0) - panel.listLimit))
                }
                Button {
                    text: "Next changes"
                    enabled: Boolean(panel.payload.page
                                     && panel.payload.page.next_offset !== null)
                    onClicked: panel.runDiff(Number(panel.payload.page.next_offset || 0))
                }
            }
            Label {
                objectName: "versioningUnsupportedCategories"
                Layout.fillWidth: true
                visible: panel.unsupportedCategorySummary(panel.payload.unsupportedCategories).length > 0
                text: panel.unsupportedCategorySummary(panel.payload.unsupportedCategories)
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: panel.browserWindow.warningColor
                Accessible.role: Accessible.AlertMessage
            }
            ListView {
                id: changesList
                objectName: "versioningChangesList"
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: panel.boundedRows(panel.payload.page && panel.payload.page.events)
                Accessible.name: "Structural changes"
                keyNavigationEnabled: true
                activeFocusOnTab: true
                delegate: Rectangle {
                    required property var modelData
                    required property int index
                    width: parent ? parent.width : 0
                    height: eventLayout.implicitHeight + 12
                    color: index % 2 === 0 ? panel.browserWindow.panelColor : panel.browserWindow.backgroundColor
                    border.color: panel.browserWindow.borderColor
                    ColumnLayout {
                        id: eventLayout
                        anchors.fill: parent
                        anchors.margins: 6
                        Label {
                            text: String(modelData.change || "change unavailable")
                            textFormat: Text.PlainText
                            font.bold: true
                            Accessible.name: "Change type " + text
                        }
                        Label {
                            text: "Window " + String(modelData.window || "")
                                + (modelData.page ? " · Page " + String(modelData.page) : "")
                            textFormat: Text.PlainText
                            font.family: "monospace"
                            elide: Text.ElideMiddle
                        }
                        Label {
                            visible: modelData.before_availability !== undefined
                                || modelData.after_availability !== undefined
                            text: "Availability: " + JSON.stringify(modelData.before_availability || null)
                                + " → " + JSON.stringify(modelData.after_availability || null)
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: panel.browserWindow.mutedTextColor
                        }
                    }
                }
            }
        }

        Label {
            objectName: "versioningErrorMessage"
            Layout.fillWidth: true
            visible: panel.panelKind === "error"
            text: String(panel.payload.message || "Versioning request could not be completed.")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: panel.browserWindow.errorColor
            Accessible.role: Accessible.AlertMessage
        }

        Label {
            Layout.fillWidth: true
            visible: panel.panelKind === "list" && (panel.selectedLeft.length > 0 || panel.selectedRight.length > 0)
            textFormat: Text.PlainText
            text: "Left: " + (panel.selectedLeft || "not selected") + " · Right: "
                + (panel.selectedRight || "not selected")
            elide: Text.ElideMiddle
        }
        RowLayout {
            Layout.fillWidth: true
            visible: panel.panelKind === "list"
            Item { Layout.fillWidth: true }
            Button {
                text: "Compare selected"
                enabled: panel.selectedLeft.length > 0 && panel.selectedRight.length > 0
                onClicked: panel.runDiff(0)
            }
            Button {
                text: "Refresh list"
                onClicked: panel.pageList(panel.lastListOffset)
            }
        }
    }
}
