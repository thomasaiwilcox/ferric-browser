import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ListNavigation.js" as ListNavigation

FocusScope {
    id: libraryEntries
    required property var browserWindow
    required property var entries
    required property string libraryKind
    required property string pendingDelete
    property Component controlsHeader
    property Item navigationLayoutRoot: null
    property bool operationPending: false
    property string completedEditValue: ""
    property int completedEditToken: 0
    property string completedEditKey: ""
    // Drafts belong to this window's library, not its recycled row widgets.
    property var editDrafts: ({})
    property var journeyDestinations: ({})
    onEntriesChanged: { editDrafts = ({}); journeyDestinations = ({}); if (libraryList) libraryList.headerFocusItem = null }
    onBrowserWindowChanged: { editDrafts = ({}); journeyDestinations = ({}); if (libraryList) libraryList.headerFocusItem = null }
    signal favouriteRequested(string id)
    signal openRequested(string entryKind, string entryId)
    signal privateHistoryTransferRequested(string entryId, string title, string url)
    signal editRequested(string entryKind, string entryId, string value)
    signal deleteRequested(string entryKind, string entryId)
    signal journeyReopenRequested(string nodeId, string target)
    signal journeyExpandRequested(string nodeId)

    readonly property alias currentIndex: libraryList.currentIndex

    function setEditDraft(key, value) {
        if (editDrafts[key] === value) return
        var next = Object.assign({}, editDrafts)
        next[key] = value
        editDrafts = next
    }

    function discardEditDraft(key) {
        if (editDrafts[key] === undefined) return
        var next = Object.assign({}, editDrafts)
        delete next[key]
        editDrafts = next
    }

    function journeyDestination(nodeId) {
        return journeyDestinations["journey\t" + nodeId] || "current"
    }

    function setJourneyDestination(nodeId, target) {
        if (["current", "tab", "window"].indexOf(target) < 0
                || journeyDestination(nodeId) === target) return
        var next = Object.assign({}, journeyDestinations)
        var key = "journey\t" + nodeId
        if (target === "current") delete next[key]
        else next[key] = target
        journeyDestinations = next
    }

    function markEditSaved(entryKind, entryId, value) {
        var key = entryKind + "\t" + entryId
        completedEditValue = value
        completedEditKey = key
        completedEditToken += 1
        if (editDrafts[key] === value) discardEditDraft(key)
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
        libraryList.focusViewport()
    }

    function focusSelection() {
        focusList()
        if (libraryList.currentIndex >= 0)
            libraryList.positionViewAtIndex(libraryList.currentIndex, ListView.Contain)
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

    function movePage(direction) {
        ListNavigation.page(libraryList, direction, navigationLayoutRoot)
        // The list owns page navigation while header controls or expanded
        // details scroll past the selected row. Delegate focus would pull the
        // viewport back to that row through the manager's focus revealer.
        if (libraryList.currentItem) libraryList.currentItem.focus = false
        libraryList.forceActiveFocus()
    }

    function focusCurrentAction() {
        var row = libraryList.currentItem
        if (!row) return false
        var control = row.nextItemInFocusChain(true)
        var ancestor = control
        while (ancestor && ancestor !== row) ancestor = ancestor.parent
        if (!ancestor) return false
        libraryList.focusControl(control, Qt.TabFocusReason)
        return true
    }

    function headerControls() {
        var header = libraryList.headerItem
        if (!header) return {first: null, last: null}
        var control = header.nextItemInFocusChain(true)
        var first = null
        var last = null
        while (control) {
            var ancestor = control
            while (ancestor && ancestor !== header) ancestor = ancestor.parent
            if (!ancestor || control === last) break
            if (!first) first = control
            last = control
            control = control.nextItemInFocusChain(true)
        }
        return {first: first, last: last}
    }

    function focusHeaderControls() {
        var control = headerControls().last
        if (!control) return false
        control.forceActiveFocus(Qt.BacktabFocusReason)
        return true
    }

    function isBacktab(event) {
        return (event.key === Qt.Key_Backtab
                && (event.modifiers === Qt.NoModifier || event.modifiers === Qt.ShiftModifier))
            || (event.key === Qt.Key_Tab && event.modifiers === Qt.ShiftModifier)
    }

    function moveToBeginning() {
        ListNavigation.prepareViewport(libraryList, navigationLayoutRoot)
        if (libraryList.count > 0) libraryList.currentIndex = 0
        libraryList.positionViewAtBeginning()
        libraryList.forceActiveFocus()
    }

    function moveToEnd() {
        ListNavigation.prepareViewport(libraryList, navigationLayoutRoot)
        if (libraryList.count > 0) {
            libraryList.currentIndex = libraryList.count - 1
            libraryList.positionViewAtEnd()
        } else ListNavigation.scrollTo(libraryList, Infinity)
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
            libraryEntries.journeyReopenRequested(entry.nodeId, journeyDestination(entry.nodeId))
        } else if (["history", "bookmark", "quickmark"].indexOf(entry.entryKind) >= 0) {
            libraryEntries.openRequested(entry.entryKind, entry.entryId)
        } else if (libraryList.currentItem) {
            libraryList.currentItem.detailsExpanded = !libraryList.currentItem.detailsExpanded
        }
    }

    function handleNavigationKey(event) {
        var host = libraryList.Window.window
        if (!host) return false
        var plainKey = event.modifiers === Qt.NoModifier
        var backtab = isBacktab(event)
        if (host.activeFocusItem !== libraryList && host.activeFocusItem !== libraryList.currentItem) {
            if (!backtab && !(plainKey && event.key === Qt.Key_Tab)) return false
            var controls = headerControls()
            if (backtab && host.activeFocusItem === controls.first) focusSelection()
            else if (plainKey && event.key === Qt.Key_Tab && host.activeFocusItem === controls.last) {
                if (!focusCurrentAction()) focusSelection()
            } else return false
            event.accepted = true
            return true
        }
        if (!plainKey && !backtab && !(event.key === Qt.Key_G && event.modifiers === Qt.ShiftModifier)) return false
        if (event.key === Qt.Key_Up || (plainKey && event.key === Qt.Key_K)) {
            moveSelection(-1)
        } else if (event.key === Qt.Key_Down || (plainKey && event.key === Qt.Key_J)) {
            moveSelection(1)
        } else if (event.key === Qt.Key_PageUp) {
            movePage(-1)
        } else if (event.key === Qt.Key_PageDown) {
            movePage(1)
        } else if (event.key === Qt.Key_Home
                   || (plainKey && event.key === Qt.Key_G
                       && !(event.modifiers & Qt.ShiftModifier))) {
            moveToBeginning()
        } else if (event.key === Qt.Key_End
                   || (event.key === Qt.Key_G && event.modifiers === Qt.ShiftModifier)) {
            moveToEnd()
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (!event.isAutoRepeat) activateCurrent()
        } else if (backtab) {
            if (!focusHeaderControls()) return false
        } else if (event.key === Qt.Key_Tab) {
            if (!focusCurrentAction()) return false
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
            readonly property bool editing: libraryEntries.editDrafts[entryKey] !== undefined
            property bool detailsExpanded: false
            readonly property string editValue: libraryEntries.editDrafts[entryKey] || ""
            readonly property string entryKey: model.entryKind + "\t" + model.entryId
            readonly property string journeyNodeId: model.nodeId
            Accessible.role: Accessible.ListItem
            Accessible.name: model.label + ", " + model.secondary
            Accessible.selected: index === libraryList.currentIndex

            function returnToList(event, control) {
                if (!libraryEntries.isBacktab(event) || libraryRow.nextItemInFocusChain(true) !== control) return
                libraryList.currentIndex = index
                libraryList.focusViewport(Qt.BacktabFocusReason)
                event.accepted = true
            }

            Connections {
                target: libraryEntries
                function onCompletedEditTokenChanged() {
                    if (libraryEntries.completedEditKey === libraryRow.entryKey && libraryRow.editValue === libraryEntries.completedEditValue) {
                        if (editInput.activeFocus) editButton.forceActiveFocus()
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
                    onTextEdited: libraryEntries.setEditDraft(libraryRow.entryKey, text)
                    Keys.onPressed: function(event) { libraryRow.returnToList(event, editInput) }
                    onAccepted: if (!libraryEntries.operationPending) libraryEntries.editRequested(model.entryKind, model.entryId, text)
                }
                Flow {
                    Layout.fillWidth: true
                    Layout.preferredHeight: childrenRect.height
                    spacing: 6
                    FerricWrappingButton {
                        id: openButton
                        visible: model.entryKind === "history"
                                 || model.entryKind === "bookmark"
                                 || model.entryKind === "quickmark"
                        text: "Open"
                        Accessible.name: "Open " + model.label
                        Keys.onPressed: function(event) { libraryRow.returnToList(event, openButton) }
                        onClicked: {
                            libraryList.currentIndex = index
                            libraryEntries.openRequested(model.entryKind, model.entryId)
                        }
                    }
                    FerricWrappingButton {
                        visible: model.entryKind === "bookmark"
                        text: "Add to Favourites"
                        Accessible.name: "Choose a favourite slot for " + model.label
                        onClicked: libraryEntries.favouriteRequested(model.entryId)
                    }
                    FerricWrappingButton {
                        visible: libraryEntries.browserWindow.temporaryProfile
                                 && model.entryKind === "history"
                        text: "Reopen in profile"
                        Accessible.name: "Reopen private history entry in a named profile"
                        onClicked: libraryEntries.privateHistoryTransferRequested(
                                       model.entryId, model.label, model.secondary)
                    }
                    FerricWrappingButton {
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
                                libraryEntries.setEditDraft(libraryRow.entryKey,
                                    model.entryKind === "quickmark" ? model.secondary : model.label)
                                editInput.forceActiveFocus()
                            }
                        }
                    }
                    FerricWrappingButton {
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
                    FerricWrappingButton {
                        objectName: "libraryEntryCancelEditButton"
                        text: "Cancel edit"
                        visible: libraryRow.editing
                        enabled: !libraryEntries.operationPending
                        onClicked: {
                            editButton.forceActiveFocus()
                            libraryEntries.discardEditDraft(libraryRow.entryKey)
                        }
                    }
                    ComboBox {
                        id: journeyTargetSelector
                        objectName: "libraryJourneyDestination"
                        visible: libraryEntries.libraryKind === "journey"
                        model: [{label: "Current tab", value: "current"}, {label: "New tab", value: "tab"}, {label: "New window", value: "window"}]
                        textRole: "label"
                        valueRole: "value"
                        implicitContentWidthPolicy: visible ? ComboBox.WidestText : ComboBox.ContentItemImplicitWidth
                        width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                        currentIndex: ["current", "tab", "window"].indexOf(
                            libraryEntries.journeyDestination(libraryRow.journeyNodeId))
                        onActivated: libraryEntries.setJourneyDestination(libraryRow.journeyNodeId, currentValue)
                        Accessible.name: "Reopen destination"
                        Keys.onPressed: function(event) { libraryRow.returnToList(event, journeyTargetSelector) }
                    }
                    FerricWrappingButton {
                        objectName: "libraryJourneyReopenButton"
                        visible: libraryEntries.libraryKind === "journey"
                        text: "Reopen"
                        Accessible.name: "Reopen " + model.label + " in " + journeyTargetSelector.currentText
                        onClicked: libraryEntries.journeyReopenRequested(
                            libraryRow.journeyNodeId, libraryEntries.journeyDestination(libraryRow.journeyNodeId))
                    }
                    FerricWrappingButton {
                        visible: libraryEntries.libraryKind === "journey"
                        text: "Nearby pages"
                        Accessible.name: "Show nearby pages for " + model.label
                        onClicked: libraryEntries.journeyExpandRequested(model.nodeId)
                    }
                    ToolButton {
                        id: detailsButton
                        objectName: "libraryEntryDetailsButton"
                        text: libraryRow.detailsExpanded ? "Hide details" : "Details"
                        Keys.onPressed: function(event) { libraryRow.returnToList(event, detailsButton) }
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
                    selectByKeyboard: true
                    wrapMode: TextEdit.Wrap
                    color: libraryEntries.browserWindow.secondaryTextColor
                    background: Rectangle { color: libraryEntries.browserWindow.surfaceColor }
                    Accessible.name: "Item details"
                }
            }
        }
    }
}
