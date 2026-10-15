import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FocusScope {
    id: libraryEntries
    required property var browserWindow
    required property var entries
    required property string libraryKind
    required property string pendingDelete
    property Component controlsHeader
    property bool operationPending: false
    property string completedEditValue: ""
    property int completedEditToken: 0
    property string completedEditKey: ""
    signal favouriteRequested(string id)
    signal openRequested(string entryKind, string entryId)
    signal privateHistoryTransferRequested(string entryId, string title, string url)
    signal editRequested(string entryKind, string entryId, string value)
    signal deleteRequested(string entryKind, string entryId)
    signal journeyReopenRequested(string nodeId, string target)
    signal journeyExpandRequested(string nodeId)

    readonly property alias currentIndex: libraryList.currentIndex

    function markEditSaved(entryKind, entryId, value) {
        completedEditValue = value
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
        } else if (["history", "bookmark", "quickmark"].indexOf(entry.entryKind) >= 0) {
            libraryEntries.openRequested(entry.entryKind, entry.entryId)
        }
    }

    function handleNavigationKey(event) {
        var host = libraryList.Window.window
        if (!host || (host.activeFocusItem !== libraryList && host.activeFocusItem !== libraryList.currentItem)) return false
        var plainKey = event.modifiers === Qt.NoModifier
        if (!plainKey && !(event.key === Qt.Key_G && event.modifiers === Qt.ShiftModifier)) return false
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
                   || (event.key === Qt.Key_G && event.modifiers === Qt.ShiftModifier)) {
            moveToEnd()
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (!event.isAutoRepeat) activateCurrent()
        } else {
            return false
        }
        event.accepted = true
        return true
    }

    FerricManagerList {
        id: libraryList
        hostWindow: libraryEntries.Window.window
        objectName: "libraryEntryList"
        anchors.fill: parent
        model: libraryEntries.entries
        header: libraryEntries.controlsHeader
        spacing: 8
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
            property bool detailsExpanded: false
            property string editValue: ""
            readonly property string entryKey: model.entryKind + "\t" + model.entryId
            Accessible.role: Accessible.ListItem
            Accessible.name: model.label + ", " + model.secondary
            Accessible.selected: index === libraryList.currentIndex

            Connections {
                target: libraryEntries
                function onCompletedEditTokenChanged() {
                    if (libraryEntries.completedEditKey === libraryRow.entryKey && libraryRow.editValue === libraryEntries.completedEditValue) {
                        var hadFocus = editInput.activeFocus
                        libraryRow.editing = false
                        if (hadFocus) editButton.forceActiveFocus()
                    }
                }
            }

            ColumnLayout {
                id: rowContent
                anchors.fill: parent
                anchors.margins: 4
                spacing: 2

                Label {
                    objectName: "libraryEntryTitle"
                    Layout.fillWidth: true
                    text: model.label || model.secondary || "Untitled item"
                    textFormat: Text.PlainText
                    color: libraryEntries.browserWindow.primaryTextColor
                    font.bold: true
                    wrapMode: Text.Wrap
                    maximumLineCount: 3
                    elide: Text.ElideRight
                }
                Label {
                    objectName: "libraryEntrySummary"
                    Layout.fillWidth: true
                    text: model.secondary
                    textFormat: Text.PlainText
                    color: libraryEntries.browserWindow.mutedTextColor
                    elide: Text.ElideMiddle
                }
                FerricTextField {
                    id: editInput
                    objectName: "libraryEntryEdit"
                    visible: libraryRow.editing
                    Layout.fillWidth: true
                    text: libraryRow.editValue
                    readOnly: libraryEntries.operationPending
                    Accessible.name: (model.entryKind === "quickmark" ? "Quickmark address for " : "Bookmark title for ") + model.label
                    onTextEdited: libraryRow.editValue = text
                    onAccepted: if (!libraryEntries.operationPending) libraryEntries.editRequested(model.entryKind, model.entryId, text)
                }
                Flow {
                    Layout.fillWidth: true
                    Layout.preferredHeight: childrenRect.height
                    spacing: 6
                    Button {
                        visible: model.entryKind === "history"
                                 || model.entryKind === "bookmark"
                                 || model.entryKind === "quickmark"
                        text: "Open"
                        Accessible.name: "Open " + model.label
                        onClicked: {
                            libraryList.currentIndex = index
                            libraryEntries.openRequested(model.entryKind, model.entryId)
                        }
                    }
                    Button {
                        visible: model.entryKind === "bookmark"
                        text: "Add to Favourites"
                        Accessible.name: "Choose a favourite slot for " + model.label
                        onClicked: libraryEntries.favouriteRequested(model.entryId)
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
                        id: editButton
                        objectName: "libraryEntryEditButton"
                        text: libraryRow.editing ? "Save" : "Edit"
                        enabled: !libraryEntries.operationPending
                        Accessible.name: (libraryRow.editing ? "Save " : "Edit ")
                                         + model.label
                        onClicked: {
                            if (libraryRow.editing) {
                                libraryEntries.editRequested(
                                            model.entryKind, model.entryId, libraryRow.editValue)
                            } else {
                                libraryRow.editValue = model.entryKind === "quickmark"
                                        ? model.secondary : model.label
                                libraryRow.editing = true
                                editInput.forceActiveFocus()
                            }
                        }
                    }
                    Button {
                        objectName: "libraryEntryDeleteButton"
                        enabled: !libraryEntries.operationPending
                        visible: (model.entryKind === "bookmark" || model.entryKind === "quickmark")
                                 && !libraryRow.editing
                        text: libraryEntries.pendingDelete === libraryRow.entryKey
                              ? "Confirm delete" : "Delete"
                        Accessible.name: (text === "Delete" ? "Delete " : "Confirm delete ")
                                         + model.label
                        onClicked: libraryEntries.deleteRequested(model.entryKind, model.entryId)
                    }
                    Button {
                        text: "Cancel edit"
                        visible: libraryRow.editing
                        enabled: !libraryEntries.operationPending
                        onClicked: { libraryRow.editing = false; editButton.forceActiveFocus() }
                    }
                    ComboBox {
                        id: journeyTargetSelector
                        visible: libraryEntries.libraryKind === "journey"
                        model: [{label: "Current tab", value: "current"}, {label: "New tab", value: "tab"}, {label: "New window", value: "window"}]
                        textRole: "label"
                        valueRole: "value"
                        Accessible.name: "Reopen destination"
                    }
                    Button {
                        visible: libraryEntries.libraryKind === "journey"
                        text: "Reopen"
                        Accessible.name: "Reopen " + model.label + " in " + journeyTargetSelector.currentText
                        onClicked: libraryEntries.journeyReopenRequested(
                                       model.nodeId, journeyTargetSelector.currentValue)
                    }
                    Button {
                        visible: libraryEntries.libraryKind === "journey"
                        text: "Nearby pages"
                        Accessible.name: "Show nearby pages for " + model.label
                        onClicked: libraryEntries.journeyExpandRequested(model.nodeId)
                    }
                    ToolButton {
                        objectName: "libraryEntryDetailsButton"
                        text: libraryRow.detailsExpanded ? "Hide details" : "Details"
                        onClicked: libraryRow.detailsExpanded = !libraryRow.detailsExpanded
                    }
                }
                TextArea {
                    objectName: "libraryEntryDetails"
                    Layout.fillWidth: true
                    visible: libraryRow.detailsExpanded
                    text: model.detail || (model.label + "\n" + model.secondary)
                    textFormat: TextEdit.PlainText
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    color: libraryEntries.browserWindow.secondaryTextColor
                    background: Rectangle { color: libraryEntries.browserWindow.surfaceColor }
                    Accessible.name: "Item details"
                }
            }
        }
    }
}
