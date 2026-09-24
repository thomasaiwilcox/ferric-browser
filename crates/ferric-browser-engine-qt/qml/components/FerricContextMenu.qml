import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

pragma ComponentBehavior: Bound

Item {
    id: root

    // The window creates safe menu entries and executes them. This component
    // only displays that model and reports the user's choice.
    required property var browserWindow

    signal scopeSelected(string scope)
    signal itemActivated(var item)
    signal dismissed()

    function open() {
        popup.open()
    }

    function close() {
        popup.close()
    }

    Popup {
        id: popup
        parent: Overlay.overlay
        modal: true
        focus: true
        closePolicy: Popup.NoAutoClose
        width: Math.min(420 * root.browserWindow.chromeScale, root.browserWindow.width - 48)
        height: Math.min(560 * root.browserWindow.chromeScale, root.browserWindow.height - 32)
        padding: 8
        x: Math.round((root.browserWindow.width - width) / 2)
        y: Math.round((root.browserWindow.height - height) / 2)

        background: Rectangle {
            color: root.browserWindow.panelColor
            border.color: root.browserWindow.accentColor
            radius: 4
        }

        contentItem: ColumnLayout {
            focus: true
            Accessible.role: Accessible.PopupMenu
            Accessible.name: "Web content context menu"

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    root.dismissed()
                    event.accepted = true
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 4
                Accessible.role: Accessible.List
                Accessible.name: "Switcher scopes"
                Repeater {
                    model: ["all", "tabs", "windows", "contexts", "commands",
                        "actions", "history", "marks", "sessions", "downloads", "closed"]
                    delegate: Button {
                        id: scopeDelegate
                        required property string modelData
                        text: scopeDelegate.modelData
                        checkable: true
                        checked: scopeDelegate.modelData === root.browserWindow.switcherScope
                        Accessible.role: Accessible.PageTab
                        Accessible.name: "Switcher scope " + scopeDelegate.modelData
                        Accessible.selected: checked
                        onClicked: root.scopeSelected(scopeDelegate.modelData)
                    }
                }
            }

            ListView {
                id: itemList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: root.browserWindow.contextMenuItems
                Accessible.role: Accessible.List
                Accessible.name: "Context menu actions"

                delegate: Button {
                    id: itemDelegate
                    required property var modelData
                    width: itemList.width
                    text: itemDelegate.modelData.label
                    Accessible.role: Accessible.MenuItem
                    Accessible.name: itemDelegate.modelData.label
                    onClicked: root.itemActivated(itemDelegate.modelData)
                }
            }

            Button {
                Layout.fillWidth: true
                text: "Close"
                Accessible.name: "Close context menu"
                onClicked: root.dismissed()
            }
        }
    }
}
