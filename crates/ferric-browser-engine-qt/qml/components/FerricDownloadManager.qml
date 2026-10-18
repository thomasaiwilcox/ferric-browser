import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ChromePresentation.js" as ChromePresentation

FerricModalSurface {
    id: manager
    required property var downloadsModel
    property bool sessionOnly: false
    property bool listPending: false
    property bool listFailed: false
    signal closeRequested()
    signal openRequested(string downloadId, bool reveal)
    signal actionRequested(string downloadId, string action)
    property bool preparingDownload: false
    signal cancelRequestRequested()
    signal refreshRequested()
    readonly property string historyStatusDescription: preparingDownload ? "Preparing download…" : listFailed
        ? "Download history couldn't be updated. Refresh to try again."
        : listPending && downloadList.count > 0 ? "Updating saved history…" : ""

    function revealControl(control) {
        if (!control || !control.activeFocus || downloadList.height <= 0) return
        var top = control.mapToItem(downloadList.contentItem, 0, 0).y
        var bottom = top + control.height
        var position = downloadList.contentY
        if (top < position) position = top
        else if (bottom > position + downloadList.height) position = bottom - downloadList.height
        downloadList.contentY = Math.max(downloadList.originY,
            Math.min(downloadList.originY + Math.max(0, downloadList.contentHeight - downloadList.height), position))
    }

    visible: browserWindow.downloadManagerVisible
    commandText: ":downloads"
    title: "Downloads"
    message: sessionOnly ? "Downloads for this session. History is not saved."
                         : "Your saved and active downloads."
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc close"
    dialogWidth: 800 * scale
    dialogHeight: 560 * scale
    stackingOrder: 40
    initialFocusItem: downloadList
    onDismissRequested: closeRequested()

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * manager.scale

        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                opacity: manager.historyStatusDescription.length > 0 ? 1 : 0
                Accessible.ignored: opacity === 0
                Accessible.name: manager.historyStatusDescription
                text: manager.preparingDownload ? "Preparing…" : manager.listFailed
                    ? "History unavailable" : "Updating history…"
                textFormat: Text.PlainText
                color: manager.listFailed ? manager.browserWindow.warningColor : manager.browserWindow.secondaryTextColor
                elide: Text.ElideRight
                HoverHandler { id: historyStatusHover }
                ToolTip.visible: historyStatusHover.hovered && opacity > 0
                ToolTip.text: manager.historyStatusDescription
            }
            Button {
                objectName: "downloadCancelRequestButton"
                text: "Cancel request"
                visible: manager.preparingDownload
                onClicked: manager.cancelRequestRequested()
            }
            Button {
                text: "Refresh"
                enabled: !manager.listPending
                opacity: enabled ? 1 : 0.5
                onClicked: manager.refreshRequested()
            }
            Button {
                text: "Close"
                Accessible.name: "Close downloads"
                onClicked: manager.closeRequested()
            }
        }

        ListView {
            id: downloadList
            objectName: "downloadList"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 8 * manager.scale
            model: manager.downloadsModel
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: downloadRow
                required property string id
                required property string state
                required property string bytes
                required property real total
                required property real speed
                required property string destination
                required property string reason
                required property string displayName
                required property bool retryAvailable
                required property bool retryPending
                required property bool active
                required property bool fileReady
                required property bool finishing
                property bool detailsExpanded: false
                width: downloadList.width
                height: downloadContent.implicitHeight + 24 * manager.scale
                color: manager.browserWindow.surfaceColor
                radius: 4 * manager.scale

                ColumnLayout {
                    id: downloadContent
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 12 * manager.scale
                    spacing: 6 * manager.scale

                    Label {
                        objectName: "downloadFileName"
                        Layout.fillWidth: true
                        text: downloadRow.displayName
                        textFormat: Text.PlainText
                        font.bold: true
                        color: manager.browserWindow.primaryTextColor
                        elide: Text.ElideMiddle
                        Accessible.name: "Download " + text
                    }
                    Label {
                        Layout.fillWidth: true
                        text: (downloadRow.finishing ? "Saving file…" : ChromePresentation.downloadStateLabel(downloadRow.state))
                              + " · " + ChromePresentation.downloadProgress(downloadRow.bytes,
                                  downloadRow.total, downloadRow.speed, downloadRow.state)
                        textFormat: Text.PlainText
                        color: downloadRow.state === "interrupted"
                               ? manager.browserWindow.warningColor
                               : manager.browserWindow.secondaryTextColor
                        wrapMode: Text.WordWrap
                    }
                    Flow {
                        Layout.fillWidth: true
                        Layout.preferredHeight: childrenRect.height
                        spacing: 6 * manager.scale
                        Button {
                            id: openControl
                            objectName: "downloadOpenButton"
                            enabled: downloadRow.fileReady
                            opacity: enabled ? 1 : 0.5
                            visible: downloadRow.state === "completed"
                            text: "Open"
                            onActiveFocusChanged: if (activeFocus) manager.revealControl(openControl)
                            Accessible.name: "Open " + ChromePresentation.fileName(downloadRow.destination)
                            onClicked: manager.openRequested(downloadRow.id, false)
                        }
                        Button {
                            visible: downloadRow.state === "completed"
                            id: folderControl
                            text: "Show in folder"
                            onActiveFocusChanged: if (activeFocus) manager.revealControl(folderControl)
                            enabled: downloadRow.fileReady
                            opacity: enabled ? 1 : 0.5
                            onClicked: manager.openRequested(downloadRow.id, true)
                        }
                        Button {
                            visible: downloadRow.active && !downloadRow.finishing
                                     && (downloadRow.state === "in-progress" || downloadRow.state === "paused")
                            id: pauseControl
                            objectName: "downloadPauseButton"
                            onActiveFocusChanged: if (activeFocus) manager.revealControl(pauseControl)
                            text: downloadRow.state === "paused" ? "Resume" : "Pause"
                            onClicked: manager.actionRequested(downloadRow.id,
                                downloadRow.state === "paused" ? "resume" : "pause")
                        }
                        Button {
                            id: cancelControl
                            objectName: "downloadCancelButton"
                            onActiveFocusChanged: if (activeFocus) manager.revealControl(cancelControl)
                            visible: downloadRow.active && (downloadRow.finishing
                                     || downloadRow.state === "offered" || downloadRow.state === "selecting-destination"
                                     || downloadRow.state === "in-progress" || downloadRow.state === "paused")
                            text: "Cancel"
                            onClicked: manager.actionRequested(downloadRow.id, "cancel")
                        }
                        Button {
                            visible: downloadRow.state === "interrupted" || downloadRow.state === "cancelled"
                            text: downloadRow.retryPending ? "Retrying…" : "Retry"
                            id: retryControl
                            objectName: "downloadRetryButton"
                            onActiveFocusChanged: if (activeFocus) manager.revealControl(retryControl)
                            enabled: downloadRow.retryAvailable && !manager.preparingDownload
                            opacity: enabled ? 1 : 0.5
                            onClicked: manager.actionRequested(downloadRow.id, "retry")
                        }
                        ToolButton {
                            id: detailsControl
                            objectName: "downloadDetailsButton"
                            onActiveFocusChanged: if (activeFocus) manager.revealControl(detailsControl)
                            text: downloadRow.detailsExpanded ? "Hide details" : "Details"
                            onClicked: downloadRow.detailsExpanded = !downloadRow.detailsExpanded
                        }
                    }
                    ProgressBar {
                        objectName: "downloadProgressBar"
                        Layout.fillWidth: true
                        // Progress is an information signal, independent of
                        // the subdued colors used for selecting text.
                        palette.highlight: manager.browserWindow.accentColor
                        palette.dark: manager.browserWindow.accentColor
                        palette.base: manager.browserWindow.panelColor
                        palette.midlight: manager.browserWindow.panelColor
                        visible: !downloadRow.finishing && (downloadRow.state === "in-progress" || downloadRow.state === "paused")
                        from: 0
                        to: 1
                        value: downloadRow.total > 0
                               ? Math.max(0, Math.min(1, Number(downloadRow.bytes) / downloadRow.total)) : 0
                        indeterminate: downloadRow.state === "in-progress" && downloadRow.total <= 0
                        Accessible.name: "Download progress"
                    }
                    Label {
                        Layout.fillWidth: true
                        text: downloadRow.destination || "Choose a location to save this file."
                        textFormat: Text.PlainText
                        color: manager.browserWindow.mutedTextColor
                        elide: Text.ElideMiddle
                        Accessible.name: "Download location: " + text
                    }
                    Label {
                        objectName: "downloadDetails"
                        Layout.fillWidth: true
                        visible: downloadRow.detailsExpanded
                        text: "Download ID: " + downloadRow.id
                              + ((downloadRow.state === "interrupted" || downloadRow.state === "cancelled")
                                 && !downloadRow.retryAvailable && !downloadRow.retryPending
                                 ? "\nStart this download again from its page; its saved address can't be retried." : "")
                              + ((downloadRow.state === "interrupted" || downloadRow.state === "cancelled")
                                 && downloadRow.reason.length > 0 ? "\n" + downloadRow.reason : "")
                        textFormat: Text.PlainText
                        color: manager.browserWindow.secondaryTextColor
                        wrapMode: Text.WrapAnywhere
                    }
                }
            }

            Label {
                objectName: "downloadsEmptyState"
                anchors.centerIn: parent
                width: Math.max(0, parent.width - 24 * manager.scale)
                visible: downloadList.count === 0
                text: manager.listPending ? "Loading downloads…"
                      : manager.listFailed ? "Download history couldn't be loaded.\nRefresh to try again."
                      : "No downloads yet.\nFiles you save will appear here."
                textFormat: Text.PlainText
                color: manager.browserWindow.secondaryTextColor
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
            }
        }
    }
}
