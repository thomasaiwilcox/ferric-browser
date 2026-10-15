import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: feedback
    required property var browserWindow
    property string message: ""
    property string detail: ""
    property bool failed: false
    property bool pending: false
    readonly property real scale: browserWindow.chromeScale === undefined ? 1 : browserWindow.chromeScale
    visible: message.length > 0
    spacing: 6 * scale
    onMessageChanged: detailsButton.checked = false
    onDetailChanged: detailsButton.checked = false

    RowLayout {
        Layout.fillWidth: true
        BusyIndicator {
            visible: feedback.pending
            running: visible
            Layout.preferredWidth: 24 * feedback.scale
            Layout.preferredHeight: 24 * feedback.scale
        }
        Label {
            objectName: "managerFeedbackMessage"
            Layout.fillWidth: true
            text: feedback.message
            textFormat: Text.PlainText
            color: feedback.failed ? feedback.browserWindow.errorColor : feedback.browserWindow.secondaryTextColor
            wrapMode: Text.Wrap
            Accessible.name: text
        }
    }
    Button {
        id: detailsButton
        objectName: "managerFeedbackDetailsButton"
        visible: feedback.detail.length > 0
        text: checked ? "Hide details" : "Show details"
        checkable: true
    }
    TextArea {
        objectName: "managerFeedbackDetails"
        Layout.fillWidth: true
        visible: detailsButton.visible && detailsButton.checked
        text: feedback.detail
        textFormat: TextEdit.PlainText
        readOnly: true
        selectByMouse: true
        selectByKeyboard: true
        wrapMode: TextEdit.Wrap
        color: feedback.browserWindow.secondaryTextColor
        background: Rectangle { color: feedback.browserWindow.surfaceColor }
        Accessible.name: "Operation details"
    }
}
