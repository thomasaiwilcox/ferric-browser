import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: settingRows
    required property var browserWindow
    required property var settingsModel
    signal applyRequested(var row, var value)
    signal resetRequested(var row)

    ListView {
        id: settingsList
        anchors.fill: parent
        clip: true
        spacing: 5
        model: settingRows.settingsModel
        delegate: Rectangle {
            id: settingRow
            readonly property var rowData: ({ key: key, label: label, type: type,
                                               scope: scope, apply: apply, value: value })
            width: settingsList.width
            height: Math.max(76, settingRows.browserWindow.chromeRowHeight * 5)
            color: settingRows.browserWindow.surfaceColor
            radius: 3
            Accessible.name: rowData.label + " " + rowData.key

            RowLayout {
                anchors.fill: parent
                anchors.margins: 8
                spacing: 8

                ColumnLayout {
                    Layout.preferredWidth: 300
                    Layout.minimumWidth: 210
                    spacing: 2
                    Label {
                        Layout.fillWidth: true
                        text: settingRow.rowData.label
                        color: settingRows.browserWindow.primaryTextColor
                        font.bold: true
                        elide: Text.ElideRight
                    }
                    Label {
                        Layout.fillWidth: true
                        text: settingRow.rowData.key + " · " + settingRow.rowData.scope
                              + " · " + settingRow.rowData.apply
                        color: settingRows.browserWindow.mutedTextColor
                        elide: Text.ElideRight
                    }
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
                    visible: settingRow.rowData.type === "enum"
                    Layout.fillWidth: true
                    model: settingRow.rowData.options || []
                    currentIndex: Math.max(0, (settingRow.rowData.options || [])
                                           .indexOf(String(settingRow.rowData.value)))
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
