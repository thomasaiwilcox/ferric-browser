import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: libraryEntries
    required property var browserWindow
    required property var entries
    required property string libraryKind
    required property string pendingDelete
    property int completedEditToken: 0
    property string completedEditKey: ""
    signal openRequested(string entryKind, string entryId)
    signal privateHistoryTransferRequested(string entryId, string title, string url)
    signal editRequested(string entryKind, string entryId, string value)
    signal deleteRequested(string entryKind, string entryId)
    signal journeyReopenRequested(string nodeId, string target)
    signal journeyExpandRequested(string nodeId)

    function markEditSaved(entryKind, entryId) {
        completedEditKey = entryKind + "\t" + entryId
        completedEditToken += 1
    }

    ListView {
        id: libraryList
        anchors.fill: parent
        clip: true
        model: libraryEntries.entries
        delegate: ColumnLayout {
            id: libraryRow
            width: libraryList.width
            spacing: 2
            property bool editing: false
            property bool journeyTargetPickerVisible: false
            property string editValue: model.entryKind === "quickmark"
                    ? model.secondary : model.label
            readonly property string entryKey: model.entryKind + "\t" + model.entryId

            Connections {
                target: libraryEntries
                function onCompletedEditTokenChanged() {
                    if (libraryEntries.completedEditKey === libraryRow.entryKey) {
                        libraryRow.editing = false
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: !libraryRow.editing
                    text: model.label
                    color: libraryEntries.browserWindow.primaryTextColor
                    elide: Text.ElideRight
                }
                TextField {
                    visible: libraryRow.editing
                    Layout.fillWidth: true
                    text: libraryRow.editValue
                    Accessible.name: "Edit " + model.entryKind + " " + model.entryId
                    onTextChanged: libraryRow.editValue = text
                    onAccepted: libraryEntries.editRequested(model.entryKind, model.entryId, text)
                }
                Button {
                    visible: model.entryKind === "history"
                             || model.entryKind === "bookmark"
                             || model.entryKind === "quickmark"
                    text: "Open"
                    Accessible.name: "Open " + model.entryKind + " " + model.entryId
                    onClicked: libraryEntries.openRequested(model.entryKind, model.entryId)
                }
                Button {
                    visible: libraryEntries.browserWindow.temporaryProfile
                             && model.entryKind === "history"
                    text: "Reopen in profile"
                    Accessible.name: "Reopen private history entry in a named profile"
                    onClicked: libraryEntries.privateHistoryTransferRequested(
                                   model.entryId, model.label, model.secondary)
                }
                Button {
                    visible: model.entryKind === "bookmark" || model.entryKind === "quickmark"
                    text: libraryRow.editing ? "Save" : "Edit"
                    Accessible.name: (libraryRow.editing ? "Save " : "Edit ")
                                     + model.entryKind + " " + model.entryId
                    onClicked: {
                        if (libraryRow.editing) {
                            libraryEntries.editRequested(
                                        model.entryKind, model.entryId, libraryRow.editValue)
                        } else {
                            libraryRow.editValue = model.entryKind === "quickmark"
                                    ? model.secondary : model.label
                            libraryRow.editing = true
                        }
                    }
                }
                Button {
                    visible: (model.entryKind === "bookmark" || model.entryKind === "quickmark")
                             && !libraryRow.editing
                    text: libraryEntries.pendingDelete === libraryRow.entryKey
                          ? "Confirm delete" : "Delete"
                    Accessible.name: (text === "Delete" ? "Delete " : "Confirm delete ")
                                     + model.entryKind + " " + model.entryId
                    onClicked: libraryEntries.deleteRequested(model.entryKind, model.entryId)
                }
                Button {
                    visible: libraryEntries.libraryKind === "journey"
                    text: "Reopen target"
                    Accessible.name: "Choose reopen target for journey node " + model.nodeId
                    onClicked: libraryRow.journeyTargetPickerVisible = !libraryRow.journeyTargetPickerVisible
                }
                ComboBox {
                    id: journeyTargetSelector
                    visible: libraryEntries.libraryKind === "journey"
                             && libraryRow.journeyTargetPickerVisible
                    model: ["current", "tab", "window"]
                    Accessible.name: "Journey reopen target"
                    ToolTip.visible: hovered
                    ToolTip.text: "Choose where this safe GET will open"
                }
                Button {
                    visible: libraryEntries.libraryKind === "journey"
                    text: "Reopen"
                    Accessible.name: "Reopen journey node " + model.nodeId
                            + " in " + journeyTargetSelector.currentText
                    onClicked: libraryEntries.journeyReopenRequested(
                                   model.nodeId, journeyTargetSelector.currentText)
                }
                Button {
                    visible: libraryEntries.libraryKind === "journey"
                    text: "Expand"
                    Accessible.name: "Expand journey node " + model.nodeId
                    onClicked: libraryEntries.journeyExpandRequested(model.nodeId)
                }
            }
            Label {
                Layout.fillWidth: true
                text: model.secondary
                color: libraryEntries.browserWindow.mutedTextColor
                elide: Text.ElideMiddle
            }
        }
    }
}
