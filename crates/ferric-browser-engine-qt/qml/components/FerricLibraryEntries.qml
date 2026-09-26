import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FocusScope {
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

    readonly property alias currentIndex: libraryList.currentIndex

    function markEditSaved(entryKind, entryId) {
        completedEditKey = entryKind + "\t" + entryId
        completedEditToken += 1
    }

    function normalizeSelection() {
        if (libraryList.count === 0) {
            libraryList.currentIndex = -1
        } else if (libraryList.currentIndex < 0) {
            libraryList.currentIndex = 0
        } else if (libraryList.currentIndex >= libraryList.count) {
            libraryList.currentIndex = libraryList.count - 1
        }
    }

    function focusList() {
        normalizeSelection()
        libraryList.forceActiveFocus()
    }

    function moveSelection(delta) {
        normalizeSelection()
        if (libraryList.count === 0)
            return
        libraryList.currentIndex = Math.max(
                    0, Math.min(libraryList.count - 1,
                                libraryList.currentIndex + delta))
        libraryList.positionViewAtIndex(libraryList.currentIndex, ListView.Contain)
        libraryList.forceActiveFocus()
    }

    function moveToBeginning() {
        if (libraryList.count === 0)
            return
        libraryList.currentIndex = 0
        libraryList.positionViewAtBeginning()
        libraryList.forceActiveFocus()
    }

    function moveToEnd() {
        if (libraryList.count === 0)
            return
        libraryList.currentIndex = libraryList.count - 1
        libraryList.positionViewAtEnd()
        libraryList.forceActiveFocus()
    }

    function entryAt(index) {
        if (index < 0 || index >= libraryList.count)
            return null
        if (libraryEntries.entries && typeof libraryEntries.entries.get === "function")
            return libraryEntries.entries.get(index)
        return libraryEntries.entries[index]
    }

    function activateCurrent() {
        var entry = entryAt(libraryList.currentIndex)
        if (!entry)
            return
        if (entry.entryKind === "journey") {
            libraryEntries.journeyReopenRequested(entry.nodeId, "current")
        } else {
            libraryEntries.openRequested(entry.entryKind, entry.entryId)
        }
    }

    function handleNavigationKey(event) {
        var plainKey = !(event.modifiers & (Qt.ControlModifier
                                             | Qt.AltModifier
                                             | Qt.MetaModifier))
        if (event.key === Qt.Key_Up || (plainKey && event.key === Qt.Key_K)) {
            moveSelection(-1)
        } else if (event.key === Qt.Key_Down || (plainKey && event.key === Qt.Key_J)) {
            moveSelection(1)
        } else if (event.key === Qt.Key_PageUp) {
            moveSelection(-Math.max(1, Math.floor(libraryList.height / 54)))
        } else if (event.key === Qt.Key_PageDown) {
            moveSelection(Math.max(1, Math.floor(libraryList.height / 54)))
        } else if (event.key === Qt.Key_Home
                   || (plainKey && event.key === Qt.Key_G
                       && !(event.modifiers & Qt.ShiftModifier))) {
            moveToBeginning()
        } else if (event.key === Qt.Key_End
                   || (plainKey && event.key === Qt.Key_G
                       && (event.modifiers & Qt.ShiftModifier))) {
            moveToEnd()
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            activateCurrent()
        } else {
            return false
        }
        event.accepted = true
        return true
    }

    ListView {
        id: libraryList
        objectName: "libraryEntryList"
        anchors.fill: parent
        clip: true
        model: libraryEntries.entries
        currentIndex: -1
        focus: visible
        activeFocusOnTab: true
        keyNavigationEnabled: false
        Accessible.role: Accessible.List
        Accessible.name: libraryEntries.libraryKind + " entries"
        onCountChanged: libraryEntries.normalizeSelection()
        Keys.onPressed: function(event) {
            libraryEntries.handleNavigationKey(event)
        }

        delegate: Rectangle {
            id: libraryRow
            width: libraryList.width
            implicitHeight: rowContent.implicitHeight + 8
            height: implicitHeight
            color: index === libraryList.currentIndex
                   ? libraryEntries.browserWindow.selectionColor : "transparent"
            radius: 3
            property bool editing: false
            property bool journeyTargetPickerVisible: false
            property string editValue: model.entryKind === "quickmark"
                    ? model.secondary : model.label
            readonly property string entryKey: model.entryKind + "\t" + model.entryId
            Accessible.role: Accessible.ListItem
            Accessible.name: model.label + ", " + model.secondary
            Accessible.selected: index === libraryList.currentIndex

            Connections {
                target: libraryEntries
                function onCompletedEditTokenChanged() {
                    if (libraryEntries.completedEditKey === libraryRow.entryKey) {
                        libraryRow.editing = false
                    }
                }
            }

            ColumnLayout {
                id: rowContent
                anchors.fill: parent
                anchors.margins: 4
                spacing: 2

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
                        onClicked: {
                            libraryList.currentIndex = index
                            libraryEntries.openRequested(model.entryKind, model.entryId)
                        }
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
}
