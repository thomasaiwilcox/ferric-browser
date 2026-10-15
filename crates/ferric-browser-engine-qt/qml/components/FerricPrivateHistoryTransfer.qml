import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: transfer
    required property var profilesModel
    required property bool profilesLoading
    property bool transferPending: false
    property string transferMessage: ""
    property string transferDetail: ""
    property bool transferFailed: false
    signal reopenRequested(string profileName)
    signal bookmarkRequested(string profileName, string profileLabel)
    signal cancelRequested()
    visible: browserWindow.privateHistoryTransferVisible
    commandText: "Private page"
    title: "Use this page in a named profile"
    message: "The destination uses normal browsing and storage. Only this page’s address and title are copied; private cookies, permissions and browsing history stay private."
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc close"
    compactKeyHelp: "esc close"
    dialogWidth: 720 * scale
    dialogHeight: 500 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 82
    initialFocusItem: cancelButton
    onDismissRequested: cancelRequested()

    ColumnLayout {
        anchors.fill: parent
        spacing: 10 * transfer.scale

        FerricManagerFeedback {
            Layout.fillWidth: true
            browserWindow: transfer.browserWindow
            message: transfer.transferMessage
            detail: transfer.transferDetail
            pending: transfer.transferPending
            failed: transfer.transferFailed
        }
        FerricManagerList {
            id: profileList
            objectName: "privateTransferProfiles"
            Layout.fillWidth: true
            Layout.fillHeight: true
            hostWindow: transfer.hostWindow
            model: transfer.profilesModel
            spacing: 10 * transfer.scale
            header: ColumnLayout {
                width: profileList.width - 12 * transfer.scale
                spacing: 8 * transfer.scale
                FerricScrollColumn {
                    objectName: "privateTransferPageReview"
                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.min(contentLayout.implicitHeight, 140
                                                     * transfer.scale)
                    hostWindow: transfer.hostWindow
                    TextArea {
                        Layout.fillWidth: true
                        text: transfer.browserWindow.privateHistoryTransferTitle || "Untitled page"
                        textFormat: TextEdit.PlainText
                        readOnly: true
                        selectByMouse: true
                        selectByKeyboard: true
                        wrapMode: TextEdit.Wrap
                        padding: 0
                        background: null
                        color: transfer.browserWindow.primaryTextColor
                        Accessible.name: "Private page title"
                    }
                    TextArea {
                        objectName: "privateTransferAddress"
                        Layout.fillWidth: true
                        text: transfer.browserWindow.privateHistoryTransferUrl
                              || "Address unavailable"
                        textFormat: TextEdit.PlainText
                        readOnly: true
                        selectByMouse: true
                        selectByKeyboard: true
                        wrapMode: TextEdit.Wrap
                        padding: 0
                        background: null
                        color: transfer.browserWindow.secondaryTextColor
                        Accessible.name: "Address to use in the destination profile"
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: "Choose a destination profile"
                    color: transfer.browserWindow.primaryTextColor
                    wrapMode: Text.Wrap
                }
                Label {
                    Layout.fillWidth: true
                    visible: profileList.count === 0
                    text: transfer.profilesLoading ? "Loading named profiles…" :
                                                     "No named profiles available. Create one in Profiles first."
                    color: transfer.browserWindow.mutedTextColor
                    wrapMode: Text.Wrap
                }
            }
            delegate: ColumnLayout {
                required property string name
                required property string label
                width: profileList.width - 12 * transfer.scale
                spacing: 6 * transfer.scale
                Label {
                    Layout.fillWidth: true
                    text: label || name
                    textFormat: Text.PlainText
                    color: transfer.browserWindow.primaryTextColor
                    wrapMode: Text.Wrap
                }
                Label {
                    Layout.fillWidth: true
                    visible: label.length > 0 && label !== name
                    text: name
                    textFormat: Text.PlainText
                    color: transfer.browserWindow.mutedTextColor
                    wrapMode: Text.Wrap
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 8 * transfer.scale
                    Button {
                        objectName: "privateTransferOpen"
                        text: "Open page"
                        enabled: !transfer.transferPending
                        width: Math.min(implicitWidth, parent.width)
                        Accessible.name: "Open private page in " + (label || name)
                        onClicked: transfer.reopenRequested(name)
                    }
                    Button {
                        objectName: "privateTransferBookmark"
                        text: "Add bookmark"
                        enabled: !transfer.transferPending
                        width: Math.min(implicitWidth, parent.width)
                        Accessible.name: "Save private page as a bookmark in " + (label || name)
                        onClicked: transfer.bookmarkRequested(name, label)
                    }
                }
            }
        }
        Button {
            id: cancelButton
            objectName: "privateTransferCancel"
            Layout.alignment: Qt.AlignRight
            text: "Close"
            Accessible.name: "Cancel private page transfer"
            onClicked: transfer.cancelRequested()
        }
    }
}
