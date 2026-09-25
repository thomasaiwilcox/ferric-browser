import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Presentation-only shutdown decision surface. The owning window supplies its
// state and handles the emitted intents, so this component contains no browser
// lifecycle or storage policy.
Rectangle {
    id: dialog
    objectName: "shutdownDecisionDialog"

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
    height: Math.min(Math.max(dialogHeight, contentLayout.implicitHeight + 32),
                     hostWindow.height - 32)
    z: stackingOrder
    visible: promptVisible
    focus: visible
    color: hostWindow.panelColor
    border.color: dialogBorderColor
    border.width: 2
    Accessible.role: Accessible.Dialog
    Accessible.name: title

    function focusSafeChoice() {
        Qt.callLater(function() {
            if (dialog.visible) {
                keepButton.forceActiveFocus()
            }
        })
    }

    onVisibleChanged: if (visible) focusSafeChoice()
    Component.onCompleted: if (visible) focusSafeChoice()

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            dialog.keepRequested()
            event.accepted = true
            return
        }
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if ((event.modifiers & Qt.ControlModifier)
                    || proceedButton.activeFocus) {
                dialog.proceedRequested()
            } else {
                dialog.keepRequested()
            }
            event.accepted = true
        }
    }

    ColumnLayout {
        id: contentLayout
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
        Label {
            Layout.fillWidth: true
            text: "Esc: " + dialog.keepLabel
                  + "  ·  Tab: choose  ·  Enter: select  ·  Ctrl+Enter: "
                  + dialog.proceedLabel
            color: dialog.hostWindow.mutedTextColor
            wrapMode: Text.WordWrap
            Accessible.name: text
        }
        RowLayout {
            Layout.alignment: Qt.AlignRight
            spacing: 8
            Button {
                id: keepButton
                objectName: "shutdownKeepButton"
                text: dialog.keepLabel
                Accessible.name: dialog.keepAccessibleName
                KeyNavigation.tab: proceedButton
                KeyNavigation.backtab: proceedButton
                onClicked: dialog.keepRequested()
            }
            Button {
                id: proceedButton
                objectName: "shutdownProceedButton"
                text: dialog.proceedLabel
                Accessible.name: dialog.proceedAccessibleName
                KeyNavigation.tab: keepButton
                KeyNavigation.backtab: keepButton
                onClicked: dialog.proceedRequested()
            }
        }
    }
}
