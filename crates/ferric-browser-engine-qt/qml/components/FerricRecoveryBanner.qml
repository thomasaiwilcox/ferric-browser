import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Session-recovery presentation. The composition root owns recovery policy.
Rectangle {
    id: banner
    objectName: "recoveryBanner"
    required property var browserWindow

    signal recoveryRequested()
    signal dismissalRequested()

    anchors.top: parent.top
    anchors.topMargin: 76
    anchors.left: parent.left
    anchors.right: parent.right
    height: Math.max(42, browserWindow.chromeRowHeight * 2.75)
    z: 20
    visible: browserWindow.recoveryAvailable
    color: Qt.darker(browserWindow.warningColor, 2.2)

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.rightMargin: 10
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: "The previous browser run ended unexpectedly. Recover the last safe session?"
            color: browserWindow.primaryTextColor
            Accessible.name: "Session recovery notice"
        }
        Button {
            objectName: "recoveryButton"
            text: "Recover (Alt+R)"
            Accessible.name: "Recover last session"
            onClicked: recoveryRequested()
        }
        Button {
            objectName: "recoveryDismissButton"
            text: "Dismiss (Alt+D)"
            Accessible.name: "Dismiss session recovery"
            onClicked: dismissalRequested()
        }
    }

    Shortcut {
        sequence: "Alt+R"
        context: Qt.WindowShortcut
        enabled: banner.visible
        onActivated: banner.recoveryRequested()
    }

    Shortcut {
        sequence: "Alt+D"
        context: Qt.WindowShortcut
        enabled: banner.visible
        onActivated: banner.dismissalRequested()
    }
}
