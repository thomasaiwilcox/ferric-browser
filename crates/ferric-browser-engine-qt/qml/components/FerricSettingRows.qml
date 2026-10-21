import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ChromePresentation.js" as ChromePresentation

Item {
    id: settingRows
    required property var browserWindow
    required property var settingsModel
    property var hostWindow: browserWindow
    property bool interactive: true
    property bool operationPending: false
    property string pendingSettingKey: ""
    property Item retainedFocus: null
    property int retainedFocusReason: Qt.OtherFocusReason
    // These values outlive filtered row widgets. A submitted snapshot may
    // acknowledge its own draft, but must leave a newer edit alone.
    property var editDrafts: ({})
    property var submittedEdits: ({})
    onBrowserWindowChanged: clearDrafts()
    onSettingsModelChanged: clearDrafts()
    readonly property int count: settingsList.count
    implicitHeight: settingsList.contentHeight
    signal applyRequested(var row, var value)
    signal resetRequested(var row)

    function clearDrafts() { editDrafts = ({}); submittedEdits = ({}); retainedFocus = null }
    function rememberFocus() {
        var item = hostWindow ? hostWindow.activeFocusItem : null
        var ancestor = item
        while (ancestor && ancestor !== settingRows) ancestor = ancestor.parent
        if (!ancestor) { retainedFocus = null; return }
        if (item.activeFocusOnTab) {
            retainedFocus = item
            retainedFocusReason = typeof item.focusReason === "number" ? item.focusReason : Qt.OtherFocusReason
        } else if (retainedFocus) focusTimer.restart()
    }
    function restoreFocus() {
        var target = retainedFocus
        if (!target || !visible || !interactive || !hostWindow) return
        var focused = hostWindow.activeFocusItem
        if (focused && focused !== target && focused.activeFocusOnTab) return
        if (target.visible && target.enabled) target.forceActiveFocus(retainedFocusReason)
    }
    function discardDraft(key) {
        if (key !== pendingSettingKey) forgetSubmission(key)
        if (editDrafts[key] === undefined) return
        var next = Object.assign({}, editDrafts)
        delete next[key]
        editDrafts = next
    }
    function rememberDraft(row, value) {
        if (value === row.value && submittedEdits[row.key] === undefined && row.key !== pendingSettingKey) { discardDraft(row.key); return }
        if (editDrafts[row.key] === value) return
        var next = Object.assign({}, editDrafts)
        next[row.key] = value
        editDrafts = next
    }
    function reconcileDraft(row) {
        var submitted = submittedEdits[row.key]
        if (submitted !== undefined) {
            if (!matchesPublished(row, submitted)) return
            forgetSubmission(row.key)
            if (editDrafts[row.key] === submitted) { discardDraft(row.key); return }
        }
        if (editDrafts[row.key] === row.value) discardDraft(row.key)
    }
    function matchesPublished(row, value) {
        if (row.type === "number") {
            var number = String(value).trim().replace(/_/g, "")
            return number.length > 0 && isFinite(Number(number))
                && Number(number) === Number(row.value)
        }
        if (row.type === "languages") {
            function normalized(text) {
                var values = String(text).split(",").slice(0, 16).map(function(value) { return value.trim() })
                return values.filter(function(value, index) { return value.length > 0 && values.indexOf(value) === index }).join(",")
            }
            return normalized(value) === normalized(row.value)
        }
        return String(value) === row.value
    }
    function rememberSubmission(row, value) {
        if (editDrafts[row.key] === undefined) return
        var next = Object.assign({}, submittedEdits)
        next[row.key] = String(value)
        submittedEdits = next
    }
    function forgetSubmission(key) {
        if (submittedEdits[key] === undefined) return
        var next = Object.assign({}, submittedEdits)
        delete next[key]
        submittedEdits = next
    }
    function finishRequest(key, value, accepted) {
        if (!accepted && submittedEdits[key] === String(value)) forgetSubmission(key)
        for (var index = 0; index < settingsList.count; ++index) {
            var row = settingsList.itemAtIndex(index)
            if (row && row.rowData.key === key) { row.restoreImmediateEditors(); break }
        }
    }
    function draftValue(key) { return editDrafts[key] }
    function finishOperation(key, value, reset, snapshot, succeeded) {
        if (reset || submittedEdits[key] === String(value)) forgetSubmission(key)
        if (succeeded && editDrafts[key] === (reset ? snapshot : String(value))) discardDraft(key)
        finishRequest(key, value, true)
        for (var index = 0; index < settingsList.count; ++index) {
            var row = settingsList.itemAtIndex(index)
            if (row && row.rowData.key === key) { reconcileDraft(row.rowData); break }
        }
    }
    Timer { id: focusTimer; interval: 0; onTriggered: settingRows.restoreFocus() }

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
            readonly property bool editing: settingRows.editDrafts[rowData.key] !== undefined
            onRowDataChanged: settingRows.reconcileDraft(rowData)
            function submitValue(value) {
                if (settingRows.operationPending) return
                settingRows.rememberSubmission(rowData, value)
                settingRows.applyRequested(rowData, value)
                settingRows.reconcileDraft(rowData)
            }
            function restoreImmediateEditors() {
                settingBooleanEditor.checked = Qt.binding(function() { return settingRow.rowData.value === "true" })
                settingEnumEditor.currentIndex = Qt.binding(function() {
                    return settingEnumEditor.count > 0 ? Math.max(0, settingEnumEditor.find(String(settingRow.rowData.value))) : -1
                })
            }
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
                    objectName: "settingNameLabel"
                    Layout.fillWidth: true
                    text: settingRow.rowData.label
                    textFormat: Text.PlainText
                    color: settingRows.browserWindow.primaryTextColor
                    font.bold: true
                    wrapMode: Text.WordWrap
                }
                Label {
                    Layout.fillWidth: true
                    text: settingRow.rowData.key + " · "
                          + ChromePresentation.settingApplyLabel(settingRow.rowData.apply)
                    textFormat: Text.PlainText
                    color: settingRows.browserWindow.mutedTextColor
                    wrapMode: Text.WordWrap
                }

                CheckBox {
                    id: settingBooleanEditor
                    objectName: "settingBooleanEditor"
                    visible: settingRow.rowData.type === "bool"
                    enabled: !settingRows.operationPending
                    Layout.fillWidth: true
                    text: checked ? "On" : "Off"
                    checked: settingRow.rowData.value === "true"
                    Accessible.name: settingRow.rowData.label
                    onToggled: settingRow.submitValue(checked)
                }

                ComboBox {
                    id: settingEnumEditor
                    objectName: "settingEnumEditor"
                    visible: settingRow.rowData.type === "enum"
                    enabled: !settingRows.operationPending
                    Layout.fillWidth: true
                    model: settingRow.rowData.options || []
                    textRole: settingRow.rowData.options
                              && typeof settingRow.rowData.options.get === "function"
                              ? "value" : ""
                    currentIndex: count > 0
                                  ? Math.max(0, find(String(settingRow.rowData.value)))
                                  : -1
                    displayText: ChromePresentation.settingValueLabel(settingRow.rowData.key, currentText)
                    delegate: ItemDelegate {
                        required property int index
                        width: settingEnumEditor.width
                        font: settingEnumEditor.font
                        palette: settingEnumEditor.palette
                        text: ChromePresentation.settingValueLabel(settingRow.rowData.key, settingEnumEditor.textAt(index))
                        highlighted: settingEnumEditor.highlightedIndex === index
                    }
                    Accessible.name: settingRow.rowData.label
                    onActivated: settingRow.submitValue(currentText)
                }

                FerricTextField {
                    id: settingTextEditor
                    objectName: "settingTextEditor"
                    visible: settingRow.rowData.type === "text" || settingRow.rowData.type === "number"
                             || settingRow.rowData.type === "languages"
                    Layout.fillWidth: true
                    text: settingRow.editing ? settingRows.editDrafts[settingRow.rowData.key] : settingRow.rowData.value
                    Accessible.name: settingRow.rowData.label
                    Accessible.role: Accessible.EditableText
                    onTextEdited: settingRows.rememberDraft(settingRow.rowData, text)
                    onAccepted: settingRow.submitValue(text)
                    Keys.onPressed: function(event) {
                        if (event.isAutoRepeat && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter))
                            event.accepted = true
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    Layout.preferredHeight: childrenRect.height
                    spacing: 8
                    FerricWrappingButton {
                        objectName: "settingApplyButton"
                        visible: settingRow.rowData.type === "text" || settingRow.rowData.type === "number"
                                 || settingRow.rowData.type === "languages"
                        text: "Apply"
                        enabled: !settingRows.operationPending
                        Accessible.name: "Apply " + settingRow.rowData.label
                        onClicked: settingRow.submitValue(settingTextEditor.text)
                    }
                    FerricWrappingButton {
                        objectName: "settingResetButton"
                        text: "Reset"
                        enabled: !settingRows.operationPending
                        Accessible.name: "Reset " + settingRow.rowData.label
                        onClicked: settingRows.resetRequested(settingRow.rowData)
                    }
                    FerricWrappingButton {
                        objectName: "settingDiscardButton"
                        visible: settingRow.editing
                        text: "Discard edit"
                        Accessible.name: "Discard edit to " + settingRow.rowData.label
                        onClicked: {
                            settingRows.discardDraft(settingRow.rowData.key)
                            settingTextEditor.forceActiveFocus()
                        }
                    }
                }
                Label {
                    objectName: "settingDraftNotice"
                    Layout.fillWidth: true
                    visible: settingRow.editing
                    text: "Unapplied edit"
                    textFormat: Text.PlainText
                    color: settingRows.browserWindow.secondaryTextColor
                    wrapMode: Text.Wrap
                }
            }
        }
    }
    Connections {
        target: settingRows.hostWindow
        ignoreUnknownSignals: true
        function onActiveFocusItemChanged() { settingRows.rememberFocus() }
    }
}
