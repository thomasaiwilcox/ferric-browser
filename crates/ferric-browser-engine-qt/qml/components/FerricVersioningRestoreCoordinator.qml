import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import "../scripts/VersionedPageState.js" as VersionedPageState

Item {
    id: coordinator
    width: 0
    height: 0

    required property var hostWindow
    required property var browserUi
    property bool editableRestoreEnabled: false
    property string requestId: ""
    property string destinationTabId: ""
    property int attempts: 0
    property int generation: 0
    property bool captureRunning: false
    property bool suppressDialogRejection: false
    property var previewItems: []
    property int previewOmitted: 0
    property int previewRefillableCount: 0
    readonly property var editablePreviewList: editablePreviewRepeater

    function clearEditablePreview() {
        editablePreviewTimeout.stop()
        previewItems = []
        previewOmitted = 0
        previewRefillableCount = 0
        hostWindow.versioningRestoreEditableDialogVisible = false
        if (editableRestoreDialog.visible) {
            suppressDialogRejection = true
            editableRestoreDialog.close()
            suppressDialogRejection = false
        }
    }

    function clear(discardSidecar) {
        var oldRequestId = requestId
        readinessTimer.stop()
        clearEditablePreview()
        requestId = ""
        destinationTabId = ""
        attempts = 0
        captureRunning = false
        generation += 1
        if (discardSidecar && oldRequestId.length > 0) {
            browserUi.discard_versioned_restore_sidecar(oldRequestId)
        }
        return oldRequestId.length > 0
    }

    function discardStaleRequest(request) {
        if (requestId === request) {
            clear(true)
        } else if (request.length > 0) {
            // A late callback must not clear a newer restore that replaced it.
            // The application discard is scoped to the old one-use request ID.
            browserUi.discard_versioned_restore_sidecar(request)
        }
    }

    function startGeometry(request, tabId) {
        requestId = request
        destinationTabId = tabId
        attempts = 0
        captureRunning = false
        generation += 1
        readinessTimer.start()
    }

    function handleSidecarReady(action) {
        var allowEditableRestore = editableRestoreEnabled
        editableRestoreEnabled = false
        var fields = String(action || "").split("\t")
        if (fields.length !== 3 || fields[0] !== "versioning-sidecar-ready"
                || !/^\d{1,20}$/.test(fields[1]) || fields[1].length === 0
                || fields[2].length === 0 || fields[2].length > 64) {
            browserUi.status_text = "Restore sidecar request was invalid"
            return false
        }
        if (requestId.length > 0) {
            clear(true)
        }
        requestId = fields[1]
        destinationTabId = fields[2]
        if (!allowEditableRestore) {
            startGeometry(requestId, destinationTabId)
            return true
        }
        var previewJson = browserUi.versioned_restore_editable_preview(requestId)
        if (typeof previewJson !== "string" || previewJson.length === 0
                || previewJson.length > 20 * 1024) {
            browserUi.status_text = "Editable draft preview unavailable; restoring page structure only"
            startGeometry(requestId, destinationTabId)
            return true
        }
        var preview
        try {
            preview = JSON.parse(previewJson)
        } catch (error) {
            browserUi.status_text = "Editable draft preview unavailable; restoring page structure only"
            startGeometry(requestId, destinationTabId)
            return true
        }
        if (!preview || !Array.isArray(preview.items) || preview.items.length > 64
                || !Number.isFinite(Number(preview.omitted || 0))) {
            browserUi.status_text = "Editable draft preview invalid; restoring page structure only"
            startGeometry(requestId, destinationTabId)
            return true
        }
        var safeItems = []
        var usedBytes = 0
        for (var i = 0; i < preview.items.length; ++i) {
            var item = preview.items[i]
            if (!item || (item.kind !== "text" && item.kind !== "textarea"
                          && item.kind !== "checkbox" && item.kind !== "content-editable")) {
                browserUi.status_text = "Editable draft preview invalid; restoring page structure only"
                startGeometry(requestId, destinationTabId)
                return true
            }
            if (item.kind === "checkbox" && item.value === true) {
                safeItems.push({ kind: "checkbox", displayValue: "Checked" })
                continue
            }
            if ((item.kind !== "text" && item.kind !== "textarea"
                 && item.kind !== "content-editable")
                    || typeof item.value !== "string" || item.value.length === 0
                    || item.value.length > 4096) {
                browserUi.status_text = "Editable draft preview invalid; restoring page structure only"
                startGeometry(requestId, destinationTabId)
                return true
            }
            usedBytes += item.value.length * 2
            if (usedBytes > 16 * 1024) {
                browserUi.status_text = "Editable draft preview exceeded its safe limit; restoring page structure only"
                startGeometry(requestId, destinationTabId)
                return true
            }
            safeItems.push({ kind: item.kind, displayValue: item.value })
        }
        if (safeItems.length === 0) {
            if (Number(preview.omitted || 0) > 0) {
                browserUi.status_text = "Some editable drafts were not eligible for review"
            }
            startGeometry(requestId, destinationTabId)
            return true
        }
        previewItems = safeItems
        previewOmitted = Number(preview.omitted || 0)
        previewRefillableCount = safeItems.filter(function(item) {
            return item.kind !== "content-editable"
        }).length
        hostWindow.versioningRestoreEditableDialogVisible = true
        editableRestoreDialog.open()
        editablePreviewTimeout.restart()
        return true
    }

    function finishEditableReview(confirmed) {
        var pendingRequest = requestId
        var pendingTab = destinationTabId
        clearEditablePreview()
        if (pendingRequest.length === 0) {
            return
        }
        var refillConfirmed = !!confirmed
                && browserUi.confirm_versioned_restore_editable_preview(pendingRequest)
        if (confirmed && !refillConfirmed) {
            browserUi.status_text = "Editable draft refill was not authorized; restoring page structure only"
        } else if (!confirmed) {
            browserUi.status_text = "Editable draft refill skipped"
        }
        startGeometry(pendingRequest, pendingTab)
    }

    function contextCurrent(request, runGeneration, view, tabId, tabIndex, epoch, pageUrl) {
        return requestId === request
                && generation === runGeneration
                && browserUi.active_tab_index === tabIndex
                && browserUi.tab_index_for_id(tabId) === tabIndex
                && browserUi.tab_id_for_index(tabIndex) === tabId
                && hostWindow.activeWebView() === view
                && view.versioningNavigationEpoch === epoch
                && view.url.toString() === pageUrl
                && browserUi.current_url === pageUrl
                && view.profile === hostWindow.browserProfile
                && !hostWindow.temporaryProfile
                && !hostWindow.ephemeralProfile
                && view.lifecycleState === WebEngineView.LifecycleState.Active
    }

    function advanceGeometry() {
        var request = requestId
        if (request.length === 0) {
            readinessTimer.stop()
            return
        }
        attempts += 1
        if (attempts > 25) {
            clear(true)
            browserUi.status_text = "Geometry restore skipped: page was not ready in time"
            return
        }
        if (captureRunning) {
            return
        }
        var tabId = destinationTabId
        var tabIndex = browserUi.tab_index_for_id(tabId)
        if (tabIndex < 0 || tabIndex >= browserUi.tab_count
                || browserUi.tab_id_for_index(tabIndex) !== tabId) {
            clear(true)
            browserUi.status_text = "Geometry restore skipped: destination tab changed"
            return
        }
        if (browserUi.active_tab_index !== tabIndex) {
            clear(true)
            browserUi.status_text = "Geometry restore skipped: selected destination tab changed"
            return
        }
        var view = hostWindow.tabViewAt(tabIndex)
        if (!view || view.profile !== hostWindow.browserProfile
                || hostWindow.temporaryProfile || hostWindow.ephemeralProfile
                || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
            return
        }
        if (view.loading || !view.visible || !view.enabled) {
            return
        }
        var pageUrl = view.url.toString()
        if (!/^https?:\/\//i.test(pageUrl) || browserUi.current_url !== pageUrl) {
            return
        }
        var epoch = view.versioningNavigationEpoch
        var requestJson = browserUi.versioned_restore_geometry_capture_request(request, tabId)
        if (requestJson === "ferric-media-fingerprint-key-pending") {
            return
        }
        if (!requestJson || requestJson.length === 0) {
            clear(true)
            browserUi.status_text = "Geometry restore skipped: page qualification failed"
            return
        }
        var captureRequest
        try {
            captureRequest = JSON.parse(requestJson)
        } catch (error) {
            clear(true)
            browserUi.status_text = "Geometry restore skipped: capture request was invalid"
            return
        }
        var captureScript = VersionedPageState.geometryScript(captureRequest)
        if (typeof captureScript !== "string" || captureScript.length === 0) {
            clear(true)
            browserUi.status_text = "Geometry restore skipped: capture request was invalid"
            return
        }
        captureRunning = true
        attempts = 0
        var runGeneration = generation
        view.runJavaScript(captureScript, hostWindow.browserScriptWorld, function(currentJson) {
            if (!coordinator.contextCurrent(
                        request, runGeneration, view, tabId, tabIndex, epoch, pageUrl)) {
                coordinator.discardStaleRequest(request)
                return
            }
            if (typeof currentJson !== "string" || currentJson.length === 0
                    || currentJson.length > 262144) {
                coordinator.clear(true)
                browserUi.status_text = "Geometry restore skipped: live observation was unavailable"
                return
            }
            var geometryRequest = browserUi.versioned_restore_geometry_script(request, currentJson)
            if (!geometryRequest || geometryRequest.length === 0) {
                coordinator.clear(true)
                browserUi.status_text = "Geometry restore skipped: live observation did not qualify"
                return
            }
            var restoreScript = VersionedPageState.restoreGeometryScript(geometryRequest)
            if (typeof restoreScript !== "string" || restoreScript.length === 0) {
                coordinator.clear(true)
                browserUi.status_text = "Geometry restore skipped: plan was invalid"
                return
            }
            view.runJavaScript(restoreScript, hostWindow.browserScriptWorld, function(resultJson) {
                if (!coordinator.contextCurrent(
                            request, runGeneration, view, tabId, tabIndex, epoch, pageUrl)) {
                    coordinator.discardStaleRequest(request)
                    return
                }
                coordinator.clear(true)
                var result = null
                try {
                    result = typeof resultJson === "string" ? JSON.parse(resultJson) : null
                } catch (error) {
                    result = null
                }
                if (!result || result.ok !== true
                        || !Number.isFinite(Number(result.applied))
                        || !Number.isFinite(Number(result.skipped))) {
                    browserUi.status_text = "Structural restore completed; geometry was not applied"
                    return
                }
                var applied = Number(result.applied)
                var skipped = Number(result.skipped)
                var mediaApplied = Number(result.media_applied || 0)
                var mediaSkipped = Number(result.media_skipped || 0)
                var editableApplied = Number(result.editable_applied || 0)
                var editableSkipped = Number(result.editable_skipped || 0)
                var editableUnaccepted = Number(result.editable_unaccepted || 0)
                var mediaStatus = mediaApplied > 0 || mediaSkipped > 0
                        ? "; media positions " + mediaApplied + " applied, "
                          + mediaSkipped + " skipped" : ""
                var editableStatus = editableApplied > 0 || editableSkipped > 0
                        ? "; editable drafts " + editableApplied + " applied, "
                          + editableSkipped + " skipped (" + editableUnaccepted
                          + " not accepted by the page)" : ""
                browserUi.status_text = skipped > 0 || mediaSkipped > 0
                        || editableSkipped > 0
                        ? "Restore applied partially (" + applied + " geometry, "
                          + skipped + " skipped)" + mediaStatus + editableStatus
                        : "Restore applied (" + applied + " geometry)" + mediaStatus
                          + editableStatus
            })
        })
    }

    Component.onDestruction: clear(true)

    Timer {
        id: readinessTimer
        interval: 200
        repeat: true
        onTriggered: coordinator.advanceGeometry()
    }

    Timer {
        id: editablePreviewTimeout
        interval: 60000
        repeat: false
        onTriggered: coordinator.finishEditableReview(false)
    }

    Dialog {
        id: editableRestoreDialog
        modal: true
        focus: true
        title: "Review recovered form drafts"
        anchors.centerIn: Overlay.overlay
        width: Math.max(320, Math.min(720, coordinator.hostWindow.width - 48))
        height: Math.max(300, Math.min(560, coordinator.hostWindow.height - 48))
        padding: 20
        closePolicy: Popup.CloseOnEscape
        standardButtons: Dialog.NoButton

        onRejected: {
            if (!coordinator.suppressDialogRejection) {
                coordinator.finishEditableReview(false)
            }
        }
        onAccepted: coordinator.finishEditableReview(
                        coordinator.previewRefillableCount > 0)

        ColumnLayout {
            anchors.fill: parent
            spacing: 12

            Label {
                Layout.fillWidth: true
                text: "These saved values can be sensitive. Review them before allowing a one-time refill into matching blank native fields. Rich-text editor drafts are shown as plain text and are never refilled. Ferric will not submit the page or dispatch input/change events. The site may still replace a value."
                wrapMode: Text.WordWrap
            }

            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true

                ColumnLayout {
                    width: parent.width
                    spacing: 8

                    Repeater {
                        id: editablePreviewRepeater
                        model: coordinator.previewItems

                        delegate: Frame {
                            required property var modelData
                            property alias displayValueControl: previewValueControl
                            Layout.fillWidth: true

                            ColumnLayout {
                                anchors.fill: parent
                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.kind === "textarea" ? "Text area"
                                          : (modelData.kind === "checkbox" ? "Checkbox"
                                             : (modelData.kind === "content-editable"
                                                ? "Rich-text editor draft (text only; not refilled)"
                                                : "Text field"))
                                    font.bold: true
                                }
                                TextArea {
                                    id: previewValueControl
                                    Layout.fillWidth: true
                                    objectName: "versionedEditableDraftValue"
                                    text: String(modelData.displayValue || "")
                                    textFormat: TextEdit.PlainText
                                    readOnly: true
                                    selectByMouse: true
                                    wrapMode: TextEdit.WrapAnywhere
                                    Accessible.name: modelData.kind === "content-editable"
                                                      ? "Recovered rich-text editor draft as plain text; not refilled"
                                                      : "Recovered " + modelData.kind + " value"
                                }
                            }
                        }
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                visible: coordinator.previewOmitted > 0
                text: coordinator.previewOmitted
                      + " additional value(s) were omitted because they were unsupported or exceeded the preview limit."
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Skip draft refill"
                    Accessible.name: "Skip recovered draft refill"
                    onClicked: editableRestoreDialog.reject()
                }
                Button {
                    objectName: "versioningEditableRestoreConfirmButton"
                    text: coordinator.previewRefillableCount > 0
                          ? "Allow one-time refill" : "Continue without refill"
                    Accessible.name: coordinator.previewRefillableCount > 0
                                   ? "Allow one-time recovered native draft refill"
                                   : "Continue using the local text-only draft preview"
                    onClicked: editableRestoreDialog.accept()
                }
            }
        }
    }
}
