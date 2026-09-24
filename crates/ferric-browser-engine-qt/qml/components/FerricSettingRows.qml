import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: settingRows
    required property var browserWindow
    required property var settingsModel
    implicitHeight: settingsList.contentHeight
    signal applyRequested(var row, var value)
    signal resetRequested(var row)

    ListView {
        id: settingsList
        objectName: "settingsList"
        anchors.fill: parent
        clip: true
        interactive: false
        spacing: 5
        model: settingRows.settingsModel
        delegate: Rectangle {
            id: settingRow
            readonly property var rowData: ({ key: key, label: label, type: type,
                                               scope: scope, apply: apply, value: value,
                                               options: options })
            width: settingsList.width
            height: Math.max(76, settingContent.implicitHeight + 16)
            color: settingRows.browserWindow.surfaceColor
            radius: 3
            Accessible.name: rowData.label + " " + rowData.key

            ColumnLayout {
                id: settingContent
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 8
                spacing: 4

                Label {
                    Layout.fillWidth: true
                    text: settingRow.rowData.label
                    color: settingRows.browserWindow.primaryTextColor
                    font.bold: true
                    wrapMode: Text.WordWrap
                }
                Label {
                    Layout.fillWidth: true
                    text: settingRow.rowData.key + " · " + settingRow.rowData.scope
                          + " · " + settingRow.rowData.apply
                    color: settingRows.browserWindow.mutedTextColor
                    wrapMode: Text.WordWrap
                }

                CheckBox {
                    id: settingBooleanEditor
                    visible: settingRow.rowData.type === "bool"
                    Layout.fillWidth: true
                    text: checked ? "On" : "Off"
                    checked: settingRow.rowData.value === "true"
                    Accessible.name: settingRow.rowData.label
                    onToggled: settingRows.applyRequested(settingRow.rowData, checked)
                }

                ComboBox {
                    id: settingEnumEditor
                    objectName: "settingEnumEditor"
                    visible: settingRow.rowData.type === "enum"
                    Layout.fillWidth: true
                    model: settingRow.rowData.options || []
                    textRole: settingRow.rowData.options
                              && typeof settingRow.rowData.options.get === "function"
                              ? "value" : ""
                    currentIndex: count > 0
                                  ? Math.max(0, find(String(settingRow.rowData.value)))
                                  : -1
                    Accessible.name: settingRow.rowData.label
                    onActivated: settingRows.applyRequested(settingRow.rowData, currentText)
                }

                TextField {
                    id: settingTextEditor
                    visible: settingRow.rowData.type === "text" || settingRow.rowData.type === "number"
                             || settingRow.rowData.type === "languages"
                    Layout.fillWidth: true
                    text: settingRow.rowData.value
                    Accessible.name: settingRow.rowData.label
                    Accessible.role: Accessible.EditableText
                    onAccepted: settingRows.applyRequested(settingRow.rowData, text)
                }

                Flow {
                    Layout.fillWidth: true
                    Layout.preferredHeight: childrenRect.height
                    spacing: 8
                    Button {
                        visible: settingRow.rowData.type === "text" || settingRow.rowData.type === "number"
                                 || settingRow.rowData.type === "languages"
                        text: "Apply"
                        Accessible.name: "Apply " + settingRow.rowData.label
                        onClicked: settingRows.applyRequested(settingRow.rowData, settingTextEditor.text)
                    }
                    Button {
                        text: "Reset"
                        Accessible.name: "Reset " + settingRow.rowData.label
                        onClicked: settingRows.resetRequested(settingRow.rowData)
                    }
                }
            }
        }
    }
}
