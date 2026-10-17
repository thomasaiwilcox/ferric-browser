import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: preview
    objectName: "linkPreviewSurface"
    required property var previewUi
    readonly property var currentView: hostWindow && hostWindow.activeWebView
        ? hostWindow.activeWebView() : null
    property var sourceView: null
    property string sourceWindowId: ""
    property string sourceUrl: ""
    readonly property bool ownerCurrent: !!previewUi && !!hostWindow && !!sourceView
        && hostWindow.runtimeReleased !== true && hostWindow.runtimeDisposing !== true
        && !previewUi.profile_bootstrap_pending
        && currentView === sourceView && String(previewUi.core_window_id) === sourceWindowId
    signal closeRequested()
    signal navigationConfirmed()

    visible: !!previewUi && previewUi.link_preview_visible
    restoreFocus: false
    enabled: !visible || ownerCurrent
    initialFocusItem: closeButton
    commandText: previewUi.link_preview_command === "open" ? ":open --clean-link"
                 : previewUi.link_preview_command === "url-explain"
                 ? ":url-explain" : ":url-clean"
    title: previewUi.link_preview_command === "url-explain"
           ? "URL explanation" : "Clean-link preview"
    message: "Sensitive URL details are hidden."
    keyHelp: previewUi.link_preview_requires_confirmation ? "y open cleaned address  ·  esc cancel" : "esc close"
    dialogWidth: 900 * scale
    dialogHeight: fittedDialogHeight(previewActions.implicitHeight + previewBody.contentLayout.implicitHeight + 8)
    compactKeyHelp: previewUi.link_preview_requires_confirmation ? "esc cancel" : "esc close"
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 70
    FerricFocusOverlayController { id: focusReturn; browserWindow: preview.hostWindow }
    onOpening: {
        // Use the modal frame's pre-focus capture. Enabling the owner can
        // synchronously focus a control in this surface during registration.
        var target = previousFocusItem
        var ancestor = target
        while (ancestor && ancestor !== preview) ancestor = ancestor.parent
        if (ancestor === preview || target === hostWindow.contentItem) target = null
        focusReturn.captureOverlayFocus(hostWindow, currentView, target)
        sourceView = currentView
        sourceWindowId = String(previewUi.core_window_id)
        sourceUrl = String(previewUi.current_url)
    }
    onVisibleChanged: if (!visible && focusReturn) {
        if (ownerCurrent) focusReturn.restoreOverlayFocus(hostWindow)
        else focusReturn.discardHost(hostWindow)
        sourceView = null; sourceWindowId = ""; sourceUrl = ""
    }
    onOwnerCurrentChanged: if (visible && !ownerCurrent && staleTimer) staleTimer.restart()
    Timer {
        id: staleTimer
        interval: 0
        onTriggered: if (preview.visible && !preview.ownerCurrent) preview.dismissPreview()
    }
    onDismissRequested: dismissPreview()
    function dismissPreview() {
        if (previewUi) previewUi.clear_link_preview()
        closeRequested()
    }
    function confirmPreview() {
        if (!ownerCurrent || !previewUi.link_preview_requires_confirmation) {
            dismissPreview()
            return
        }
        if (previewUi.confirm_link_navigation()) navigationConfirmed()
        else {
            var failure = previewUi.status_text
            dismissPreview()
            if (hostWindow && hostWindow.showCommandNotice) hostWindow.showCommandNotice(failure, true)
        }
    }
    Connections {
        target: preview.previewUi
        function onCurrent_urlChanged() {
            if (preview.visible && String(preview.previewUi.current_url) !== preview.sourceUrl)
                preview.dismissPreview()
        }
        function onProfile_bootstrap_pendingChanged() {
            if (preview.visible && preview.previewUi.profile_bootstrap_pending) preview.dismissPreview()
        }
    }
    Connections {
        target: preview.sourceView
        ignoreUnknownSignals: true
        function onLoadingChanged() {
            if (preview.visible && preview.sourceView && preview.sourceView.loading
                    && !preview.sourceView.nativeUrl)
                preview.dismissPreview()
        }
    }
    Connections {
        target: preview.hostWindow
        ignoreUnknownSignals: true
        function onRuntimeReleasedChanged() {
            if (preview.hostWindow.runtimeReleased) preview.dismissPreview()
        }
    }

    Shortcut {
        sequence: "Y"
        context: Qt.WindowShortcut
        enabled: preview.interactive
                 && preview.previewUi.link_preview_requires_confirmation
        onActivated: preview.confirmPreview()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 0
        spacing: 8

        Flow {
            id: previewActions
            Layout.fillWidth: true
            Layout.preferredHeight: childrenRect.height
            spacing: 8
            FerricWrappingButton {
                id: closeButton
                objectName: "linkPreviewClose"
                text: preview.previewUi.link_preview_requires_confirmation ? "Cancel" : "Close"
                onClicked: preview.dismissPreview()
            }
            FerricWrappingButton {
                objectName: "linkPreviewConfirm"
                text: "Open cleaned address"
                visible: preview.previewUi.link_preview_requires_confirmation
                enabled: preview.ownerCurrent
                Accessible.name: "Confirm cleaned URL navigation"
                onClicked: preview.confirmPreview()
            }
        }
        FerricScrollColumn {
            id: previewBody
            objectName: "linkPreviewBody"
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            hostWindow: preview.hostWindow
            Label {
                Layout.fillWidth: true
                Layout.maximumHeight: implicitHeight
                text: "Original: " + (preview.previewUi.link_preview_original || "")
                objectName: "linkPreviewOriginal"
                textFormat: Text.PlainText
                color: preview.browserWindow.primaryTextColor
                wrapMode: Text.WrapAnywhere
            }
            Label {
                Layout.fillWidth: true
                Layout.maximumHeight: implicitHeight
                text: "Result: " + (preview.previewUi.link_preview_cleaned || "")
                objectName: "linkPreviewResult"
                textFormat: Text.PlainText
                color: preview.browserWindow.primaryTextColor
                wrapMode: Text.WrapAnywhere
            }
            Label {
                Layout.fillWidth: true
                Layout.maximumHeight: implicitHeight
                text: "Rules: " + (preview.previewUi.link_preview_applied_rules.join(", ") || "none")
                textFormat: Text.PlainText
                color: preview.browserWindow.mutedTextColor
                wrapMode: Text.WordWrap
            }
            Label {
                Layout.fillWidth: true
                Layout.maximumHeight: implicitHeight
                text: "Removed: " + (preview.previewUi.link_preview_removed_parameters.join(", ") || "none")
                textFormat: Text.PlainText
                color: preview.browserWindow.mutedTextColor
                wrapMode: Text.WordWrap
            }
            Label {
                Layout.fillWidth: true
                Layout.maximumHeight: implicitHeight
                text: "Retained: " + (preview.previewUi.link_preview_retained_parameters.join(", ") || "none")
                textFormat: Text.PlainText
                color: preview.browserWindow.mutedTextColor
                wrapMode: Text.WordWrap
            }
            Label {
                Layout.fillWidth: true
                Layout.maximumHeight: implicitHeight
                text: preview.previewUi.link_preview_explanation || ""
                objectName: "linkPreviewExplanation"
                textFormat: Text.PlainText
                color: preview.browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
                verticalAlignment: Text.AlignTop
            }
            Item { Layout.fillHeight: true; Layout.minimumHeight: 0 }
        }
    }
}
