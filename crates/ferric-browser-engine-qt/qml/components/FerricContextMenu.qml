import QtQuick
import QtQuick.Controls

pragma ComponentBehavior: Bound

Item {
    id: root

    // The window creates safe menu entries and executes them. This component
    // only displays that model and reports the user's choice.
    required property var browserWindow
    Accessible.role: Accessible.PopupMenu
    Accessible.name: "Web content context menu"

    signal itemActivated(var item)
    signal dismissed()

    function openAt(hostWindow, x, y) {
        var parentItem = hostWindow && hostWindow.contentItem
                ? hostWindow.contentItem : root.parent
        popup.popup(parentItem, Number(x) || 0, Number(y) || 0)
        Qt.callLater(function() {
            if (popup.visible && popup.count > 0) {
                popup.currentIndex = 0
                popup.forceActiveFocus()
            }
        })
    }

    function open() {
        openAt(null, 12, 12)
    }

    function close() {
        popup.dismiss()
    }

    Menu {
        id: popup
        objectName: "contextMenuPopup"
        popupType: Popup.Item
        modal: true
        focus: true
        margins: 8
        padding: 6
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        width: Math.min(520 * root.browserWindow.chromeScale,
                        Math.max(260 * root.browserWindow.chromeScale,
                                 implicitContentWidth + leftPadding + rightPadding))
        onClosed: root.dismissed()

        background: Rectangle {
            color: root.browserWindow.panelColor
            border.color: root.browserWindow.borderColor
            radius: 5
        }

        Instantiator {
            model: root.browserWindow.contextMenuItems

            delegate: MenuItem {
                id: itemDelegate
                required property var modelData
                objectName: "contextMenuItem"
                width: popup.availableWidth
                implicitWidth: Math.max(
                                   248 * root.browserWindow.chromeScale,
                                   menuLabel.implicitWidth
                                   + 28 * root.browserWindow.chromeScale)
                implicitHeight: Math.max(
                                    32 * root.browserWindow.chromeScale,
                                    menuLabel.implicitHeight
                                    + 12 * root.browserWindow.chromeScale)
                font: root.browserWindow.font
                Accessible.role: Accessible.MenuItem
                Accessible.name: itemDelegate.modelData.label
                onTriggered: root.itemActivated(itemDelegate.modelData)

                contentItem: Label {
                    id: menuLabel
                    text: itemDelegate.modelData.label
                    color: itemDelegate.highlighted
                           ? root.browserWindow.selectionTextColor
                           : root.browserWindow.primaryTextColor
                    elide: Text.ElideRight
                    verticalAlignment: Text.AlignVCenter
                }

                background: Rectangle {
                    color: itemDelegate.highlighted
                           ? root.browserWindow.selectionColor : "transparent"
                    radius: 3
                }
            }

            onObjectAdded: function(index, object) {
                popup.insertItem(index, object)
            }
            onObjectRemoved: function(index, object) {
                popup.removeItem(object)
            }
        }
    }
}
