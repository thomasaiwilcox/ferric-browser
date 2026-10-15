import QtQuick
import QtQuick.Controls

// TextField is already plain text. Qt's standard Paste binding omits
// Ctrl+Shift+V, so let the focused native control own that equivalent gesture.
TextField {
    id: control
    Shortcut {
        sequence: "Ctrl+Shift+V"
        context: Qt.WindowShortcut
        enabled: control.activeFocus && control.visible && control.enabled && !control.readOnly
        onActivated: control.paste()
    }
}
