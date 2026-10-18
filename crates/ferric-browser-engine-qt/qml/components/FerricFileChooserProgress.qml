import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Own input as soon as preparation starts. Delay painting so a fast native
// picker opens without flashing another dialog. Never restore page focus
// while the native picker is taking ownership of the request.
Item {
    id: chooserProgress
    property var chooser: null
    property var ownerWindow: null
    readonly property var appearance: ownerWindow && ownerWindow.chromeAppearance
            ? ownerWindow.chromeAppearance
            : ownerWindow && ownerWindow.accentColor !== undefined ? ownerWindow : null
    readonly property bool preparing: !!chooser && chooser.visible && chooser.backend.length === 0
    property bool revealed: false
    onPreparingChanged: if (!preparing) revealed = false

    Timer {
        interval: 200
        running: chooserProgress.preparing
        onTriggered: chooserProgress.revealed = true
    }
    Loader {
        parent: chooserProgress.ownerWindow ? chooserProgress.ownerWindow.contentItem : null
        anchors.fill: parent
        z: 80
        active: !!chooserProgress.appearance
        sourceComponent: FerricModalSurface {
            objectName: "fileChooserOpening"
            browserWindow: chooserProgress.appearance
            hostWindow: chooserProgress.ownerWindow
            visible: chooserProgress.preparing
            opacity: chooserProgress.revealed ? 1 : 0
            restoreFocus: !chooserProgress.chooser || !chooserProgress.chooser.visible
            commandText: "File chooser"
            title: "Opening the file chooser…"
            message: "You can cancel while the file chooser opens."
            keyHelp: "esc cancel"
            compactKeyHelp: keyHelp
            dialogWidth: 480 * scale
            minimumBodyHeight: cancelButton.implicitHeight
            dialogHeight: fittedDialogHeight(cancelButton.implicitHeight)
            initialFocusItem: cancelButton
            onDismissRequested: if (chooserProgress.preparing) chooserProgress.chooser.reject()

            RowLayout {
                anchors.fill: parent
                BusyIndicator { running: chooserProgress.revealed && chooserProgress.preparing }
                Item { Layout.fillWidth: true }
                Button {
                    id: cancelButton
                    objectName: "cancelFileChooserOpening"
                    text: "Cancel"
                    font: chooserProgress.appearance.font
                    onClicked: if (chooserProgress.preparing) chooserProgress.chooser.reject()
                }
            }
        }
    }
}
