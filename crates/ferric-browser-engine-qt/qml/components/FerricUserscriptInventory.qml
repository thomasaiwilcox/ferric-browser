import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: inventory
    required property var browserWindow
    required property var rows
    property var enabledOverrides: ({})
    signal enabledRequested(string name, bool enabled)
    signal removalRequested(string name)

    function enabledFor(name, fallback) {
        return Object.prototype.hasOwnProperty.call(enabledOverrides, name)
               ? enabledOverrides[name] : fallback
    }

    function setEnabled(name, enabled) {
        const next = Object.assign({}, enabledOverrides)
        next[name] = enabled
        enabledOverrides = next
    }

    implicitHeight: Math.min(Math.max(150, browserWindow.chromeRowHeight * 4),
                             Math.max(42, userscriptList.contentHeight))
    visible: !browserWindow.temporaryProfile

    ListView {
        id: userscriptList
        objectName: "userscriptList"
        anchors.fill: parent
        clip: true
        spacing: 3
        model: inventory.rows
        delegate: Rectangle {
            id: userscriptRow
            objectName: "userscriptRow"
            required property var modelData
            readonly property string scriptName: String(modelData.name || "")
            width: userscriptList.width
            height: userscriptContent.implicitHeight + 12
            color: inventory.browserWindow.surfaceColor
            radius: 3
            Accessible.name: scriptName.length > 0 ? scriptName : "userscript"

            ColumnLayout {
                id: userscriptContent
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 6
                spacing: 2
                Label {
                    Layout.fillWidth: true
                    text: userscriptRow.scriptName
                          + " · " + Number(userscriptRow.modelData.actions || 0) + " action(s)"
                          + (userscriptRow.modelData.page_world ? " · page world" : "")
                    color: inventory.browserWindow.secondaryTextColor
                    wrapMode: Text.WordWrap
                }
                RowLayout {
                    Layout.fillWidth: true
                    CheckBox {
                        id: enabledToggle
                        checked: inventory.enabledFor(userscriptRow.scriptName,
                                                    !!userscriptRow.modelData.enabled)
                        text: checked ? "Enabled" : "Disabled"
                        Accessible.name: "Enable userscript " + userscriptRow.scriptName
                        onToggled: inventory.enabledRequested(userscriptRow.scriptName, checked)
                    }
                    Item { Layout.fillWidth: true }
                    Button {
                        text: "Remove"
                        Accessible.name: "Remove userscript " + userscriptRow.scriptName
                        onClicked: inventory.removalRequested(userscriptRow.scriptName)
                    }
                }
            }
        }
    }
}
