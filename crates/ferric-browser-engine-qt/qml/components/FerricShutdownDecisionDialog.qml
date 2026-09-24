import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Presentation-only shutdown decision surface. The owning window supplies its
// state and handles the emitted intents, so this component contains no browser
// lifecycle or storage policy.
Rectangle {
    id: dialog

    required property var hostWindow
    property bool promptVisible: false
    property string title: ""
    property string message: ""
    property string keepLabel: "Keep browser open"
    property string proceedLabel: "Continue"
    property string keepAccessibleName: keepLabel
    property string proceedAccessibleName: proceedLabel
    property color dialogBorderColor: hostWindow.warningColor
    property int dialogHeight: 220
    property int stackingOrder: 100

    signal keepRequested()
    signal proceedRequested()

    anchors.centerIn: parent
    width: Math.min(560, hostWindow.width - 80)
    height: Math.min(dialogHeight, hostWindow.height - 32)
    z: stackingOrder
    visible: promptVisible
    color: hostWindow.panelColor
    border.color: dialogBorderColor
    border.width: 2
    Accessible.role: Accessible.Dialog
    Accessible.name: title

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 12

        Label {
            Layout.fillWidth: true
            text: dialog.title
            color: dialog.hostWindow.primaryTextColor
            font.bold: true
        }
        Label {
            Layout.fillWidth: true
            text: dialog.message
            color: dialog.hostWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }
        RowLayout {
            Layout.alignment: Qt.AlignRight
            spacing: 8
            Button {
                text: dialog.keepLabel
                Accessible.name: dialog.keepAccessibleName
                onClicked: dialog.keepRequested()
            }
            Button {
                text: dialog.proceedLabel
                Accessible.name: dialog.proceedAccessibleName
                onClicked: dialog.proceedRequested()
            }
        }
    }
}
