import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricCommandDialog {
    id: preview
    signal closeRequested()
    signal loadRequested(bool append)
    property var savedTabs: []
    readonly property bool canLoad: !browserWindow.sessionPreviewLoading
        && !browserWindow.sessionPreviewError && savedTabs.length > 0
    onVisibleChanged: detailsButton.checked = false

    visible: browserWindow.sessionPreviewVisible
    commandText: ":session-load"
    title: "Load session: " + browserWindow.sessionPreviewName
    message: browserWindow.sessionPreviewLoading ? "Reading saved tabs…"
        : !canLoad ? "The saved tabs couldn't be previewed. Close this preview and try again."
        : savedTabs.length + (savedTabs.length === 1 ? " saved tab. " : " saved tabs. ")
          + (browserWindow.sessionPreviewAppend ? "Add these to your current window?"
             : "Replacing current tabs may discard unsaved work.")
    dialogWidth: 680 * scale
    dialogHeight: 540 * scale
    stackingOrder: 50
    actions: [
        { id: "cancel", key: "esc", label: "Cancel", safe: true },
        { id: "append", key: "a", shortcuts: ["A"], label: "Append tabs", enabled: canLoad },
        { id: "replace", key: "r", shortcuts: ["R"], label: "Replace current tabs",
          destructive: true, enabled: canLoad && !browserWindow.sessionPreviewAppend }
    ]
    onActionRequested: function(action) {
        if (action === "append") preview.loadRequested(true)
        else if (action === "replace") preview.loadRequested(false)
        else preview.closeRequested()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * preview.scale
        BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            visible: preview.browserWindow.sessionPreviewLoading
            running: visible
        }
        ListView {
            id: tabs
            objectName: "sessionPreviewTabs"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: preview.savedTabs
            spacing: 8 * preview.scale
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                required property var modelData
                width: tabs.width
                implicitHeight: row.implicitHeight + 16 * preview.scale
                color: preview.browserWindow.surfaceColor
                radius: 4 * preview.scale
                ColumnLayout {
                    id: row
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 8 * preview.scale
                    spacing: 4 * preview.scale
                    Label {
                        objectName: "savedTabTitle"
                        Layout.fillWidth: true
                        text: modelData.label || modelData.url || "Saved tab"
                        textFormat: Text.PlainText
                        color: preview.browserWindow.primaryTextColor
                        font.bold: true
                        wrapMode: Text.Wrap
                    }
                    Label {
                        objectName: "savedTabAddress"
                        Layout.fillWidth: true
                        text: modelData.url || "This tab needs to be opened manually; its navigation won't be replayed."
                        textFormat: Text.PlainText
                        color: preview.browserWindow.secondaryTextColor
                        wrapMode: Text.Wrap
                    }
                    Label {
                        Layout.fillWidth: true
                        visible: !!modelData.pinned || !!modelData.muted
                        text: [modelData.pinned ? "Pinned" : "", modelData.muted ? "Muted" : ""].filter(function(label) { return label.length > 0 }).join(" · ")
                        color: preview.browserWindow.mutedTextColor
                        wrapMode: Text.Wrap
                    }
                }
            }
        }
        Button {
            id: detailsButton
            visible: !preview.browserWindow.sessionPreviewLoading && !preview.canLoad
            text: checked ? "Hide details" : "Show details"
            checkable: true
        }
        ScrollView {
            Layout.fillWidth: true
            Layout.preferredHeight: 100 * preview.scale
            visible: detailsButton.visible && detailsButton.checked
            clip: true
            TextArea {
                text: preview.browserWindow.sessionPreviewText
                textFormat: TextEdit.PlainText
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.Wrap
                color: preview.browserWindow.secondaryTextColor
                background: Rectangle { color: preview.browserWindow.surfaceColor }
            }
        }
    }
}
