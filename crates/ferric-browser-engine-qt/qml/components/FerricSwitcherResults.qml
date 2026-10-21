import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ChromePresentation.js" as ChromePresentation
import "../scripts/SwitcherResults.js" as SwitcherResults
import "../scripts/ListNavigation.js" as ListNavigation

Item {
    id: switcherResults
    required property var browserWindow
    required property var results
    property bool resultsCurrent: true
    property bool sourcesPending: false
    property bool sourcesFailed: false
    property string sourceNotice: ""
    property var hostWindow: browserWindow
    readonly property int resultCount: resultList.count
    readonly property int currentIndex: resultList.currentIndex
    signal activationRequested(int index)
    signal actionRequested(int index, string action)
    signal focusSearchRequested()
    signal focusPreviousRequested()

    function resultKey(value, index) {
        return SwitcherResults.resultKey(value, index)
    }
    function activationKey(value, index) {
        return SwitcherResults.activationKey(value, index)
    }
    function indexForKey(key) {
        for (var i = 0; i < resultRows.count; ++i)
            if (resultRows.get(i).rowKey === key) return i
        return -1
    }
    function actionIn(item, action) {
        if (!item) return null
        if (item.objectName === "switcherActionButton" && item.resultAction === action) return item
        for (var i = 0; i < item.children.length; ++i) {
            var found = actionIn(item.children[i], action)
            if (found) return found
        }
        return null
    }
    function reconcileResults() {
        if (!resultRows || !resultList) return
        var oldIndex = resultList.currentIndex
        var selected = oldIndex >= 0 && oldIndex < resultRows.count
            ? resultRows.get(oldIndex).rowKey : ""
        var focused = hostWindow ? hostWindow.activeFocusItem : null
        var focusedAction = focused && focused.objectName === "switcherActionButton"
            ? {key: focused.resultIdentity, action: focused.resultAction} : null
        var values = results || []
        for (var i = 0; i < values.length; ++i) {
            var value = values[i]
            var key = resultKey(value, i)
            var previous = i
            while (previous < resultRows.count && resultRows.get(previous).rowKey !== key)
                ++previous
            var actions = (value.actions || []).join("\t")
            if (previous < resultRows.count) {
                if (previous !== i) resultRows.move(previous, i, 1)
                var row = resultRows.get(i)
                // Keep action delegates when their actual actions did not
                // change. A metadata refresh must not discard keyboard focus.
                var actionsChanged = row.rowActions !== actions
                if (actionsChanged) resultRows.setProperty(i, "rowActions", actions)
                if (actionsChanged || !SwitcherResults.samePresentation(row.rowData, value))
                    resultRows.setProperty(i, "rowData", value)
            } else resultRows.insert(i, {rowKey: key, rowActions: actions, rowData: value})
        }
        if (resultRows.count > values.length)
            resultRows.remove(values.length, resultRows.count - values.length)
        var next = indexForKey(selected)
        if (next < 0) next = resultRows.count > 0 ? Math.min(Math.max(oldIndex, 0), resultRows.count - 1) : -1
        resultList.currentIndex = next
        if (focusedAction) {
            if (next < 0 || resultRows.get(next).rowKey !== focusedAction.key) {
                focusSearchRequested()
            } else {
                resultList.positionViewAtIndex(next, ListView.Contain)
                resultList.forceLayout()
                var action = actionIn(resultList.itemAtIndex(next), focusedAction.action)
                if (action) action.forceActiveFocus(Qt.TabFocusReason)
                else focusSearchRequested()
            }
        }
    }
    function activateCaptured(key, action) {
        if (!resultsCurrent || !key) return
        for (var i = 0; i < results.length; ++i) {
            if (activationKey(results[i], i) !== key) continue
            if (action === "") activationRequested(i)
            else if ((results[i].actions || []).indexOf(action) >= 0) actionRequested(i, action)
            return
        }
    }
    onResultsChanged: reconcileResults()
    Component.onCompleted: reconcileResults()
    ListModel { id: resultRows; dynamicRoles: true }

    function revealFocusedControl() {
        var control = switcherResults.hostWindow ? switcherResults.hostWindow.activeFocusItem : null
        var ancestor = control
        while (ancestor && ancestor !== resultList.contentItem) ancestor = ancestor.parent
        if (!ancestor || !control || control.height > resultList.height) return false
        var top = control.mapToItem(resultList.contentItem, 0, 0).y
        var bottom = top + control.height
        var next = resultList.contentY
        if (top < next) next = top
        else if (bottom > next + resultList.height) next = bottom - resultList.height
        resultList.contentY = Math.max(resultList.originY,
            Math.min(resultList.originY + Math.max(0, resultList.contentHeight - resultList.height), next))
        return true
    }
    Timer {
        id: revealTimer
        interval: 0
        onTriggered: {
            if (!switcherResults.revealFocusedControl() && resultList.currentIndex >= 0)
                ListNavigation.revealSelection(resultList, 0)
        }
    }

    function moveBy(delta) {
        if (resultList.count <= 0) {
            return
        }
        var next = Math.max(0, Math.min(resultList.count - 1,
                                        resultList.currentIndex + delta))
        resultList.currentIndex = next
        revealTimer.restart()
    }

    function movePage(direction) {
        revealTimer.stop()
        ListNavigation.page(resultList, direction)
    }

    function focusCurrentAction() {
        if (!resultsCurrent || resultList.currentIndex < 0
                || resultList.currentIndex >= resultRows.count) return false
        var actions = resultRows.get(resultList.currentIndex).rowActions.split("\t")
        var button = actionIn(resultList.currentItem, actions[0])
        if (!button || !button.visible || !button.enabled) return false
        button.forceActiveFocus(Qt.TabFocusReason)
        revealTimer.restart()
        return true
    }

    function moveToBeginning() {
        if (resultList.count > 0) {
            resultList.currentIndex = 0
            resultList.positionViewAtBeginning()
            revealTimer.restart()
        }
    }

    function moveToEnd() {
        if (resultList.count > 0) {
            resultList.currentIndex = resultList.count - 1
            resultList.positionViewAtEnd()
            revealTimer.restart()
        }
    }

    ListView {
        id: resultList
        objectName: "switcherResultList"
        anchors.fill: parent
        clip: true
        focus: true
        model: resultRows
        visible: switcherResults.resultsCurrent
        enabled: switcherResults.resultsCurrent
        currentIndex: -1
        Accessible.role: Accessible.List
        Accessible.name: "Switcher results"
        Accessible.description: count + (count === 1 ? " result; " : " results; ")
            + "selected result is announced with its kind and label"
        onCurrentIndexChanged: revealTimer.restart()
        onCurrentItemChanged: revealTimer.restart()
        onWidthChanged: revealTimer.restart()
        onHeightChanged: revealTimer.restart()
        ScrollBar.vertical: ScrollBar { id: resultScrollBar; policy: ScrollBar.AsNeeded }
        delegate: Rectangle {
            id: resultRow
            required property int index
            required property var rowData
            required property string rowKey
            required property string rowActions
            width: Math.max(0, resultList.width - (resultScrollBar.visible ? resultScrollBar.width + 4 : 0))
            readonly property real padding: 8
            height: resultColumn.implicitHeight + 2 * padding
            onHeightChanged: if (index === resultList.currentIndex) revealTimer.restart()
            readonly property var resultData: rowData
            readonly property int resultIndex: index
            readonly property string detailText: [resultData.profile || "", resultData.secondary || ""]
                .filter(function(value) { return value.length > 0 }).join(" · ")
            color: index === resultList.currentIndex
                   ? switcherResults.browserWindow.selectionColor : "transparent"
            Accessible.role: Accessible.ListItem
            Accessible.selected: index === resultList.currentIndex
            Accessible.name: ChromePresentation.switcherKindLabel(resultData.kind) + " " + resultData.label
            Accessible.description: detailText + (index === resultList.currentIndex
                ? (detailText.length > 0 ? " · selected" : "Selected") : "")

            MouseArea {
                anchors.fill: parent
                property string pressedKey: ""
                onPressed: pressedKey = switcherResults.activationKey(resultRow.resultData, resultRow.resultIndex)
                onCanceled: pressedKey = ""
                onClicked: {
                    switcherResults.activateCaptured(pressedKey, "")
                    pressedKey = ""
                }
            }

            ColumnLayout {
                id: resultColumn
                x: resultRow.padding
                y: resultRow.padding
                width: Math.max(0, resultRow.width - 2 * resultRow.padding)
                height: implicitHeight
                spacing: 6
                Label {
                    id: resultTitle
                    objectName: "switcherResultTitle"
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    color: switcherResults.browserWindow.primaryTextColor
                    text: ChromePresentation.switcherKindLabel(resultRow.resultData.kind) + " · " + resultRow.resultData.label
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    maximumLineCount: resultRow.index === resultList.currentIndex ? 2 : 1
                    elide: Text.ElideRight
                }
                Label {
                    id: resultDetail
                    objectName: "switcherResultDetail"
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    color: switcherResults.browserWindow.mutedTextColor
                    text: resultRow.detailText
                    textFormat: Text.PlainText
                    visible: text.length > 0
                    elide: Text.ElideRight
                }
                Flow {
                    id: actionFlow
                    Layout.fillWidth: true
                    Layout.preferredHeight: childrenRect.height
                    spacing: 4
                    Repeater {
                        model: resultRow.rowActions.length > 0 ? resultRow.rowActions.split("\t") : []
                        delegate: Button {
                            id: actionButton
                            required property int index
                            required property var modelData
                            objectName: "switcherActionButton"
                            readonly property string resultIdentity: resultRow.rowKey
                            readonly property string resultAction: String(modelData)
                            property string pressedKey: ""
                            property string pressedAction: ""
                            onPressed: {
                                pressedKey = switcherResults.activationKey(resultRow.resultData, resultRow.resultIndex)
                                pressedAction = resultAction
                            }
                            onCanceled: pressedKey = ""
                            Keys.onPressed: function(event) {
                                var backtab = (event.key === Qt.Key_Backtab
                                    && (event.modifiers === Qt.NoModifier
                                        || event.modifiers === Qt.ShiftModifier))
                                    || (event.key === Qt.Key_Tab && event.modifiers === Qt.ShiftModifier)
                                if (backtab && actionButton.index === 0) {
                                    switcherResults.focusPreviousRequested()
                                    event.accepted = true
                                }
                            }

                            text: ChromePresentation.switcherActionLabel(modelData)
                            width: Math.min(implicitWidth, actionFlow.width)
                            contentItem: Label {
                                text: actionButton.text
                                font: actionButton.font
                                color: actionButton.palette.buttonText
                                textFormat: Text.PlainText
                                horizontalAlignment: Text.AlignHCenter
                                verticalAlignment: Text.AlignVCenter
                                wrapMode: Text.WordWrap
                            }
                            Accessible.name: text + " · " + resultRow.resultData.label
                            onActiveFocusChanged: if (activeFocus) {
                                if (resultList.currentIndex !== resultRow.resultIndex) {
                                    resultList.currentIndex = resultRow.resultIndex
                                    // Selecting a new ListView row moves focus to its
                                    // delegate. Keep keyboard activation on this action.
                                    actionButton.forceActiveFocus(Qt.TabFocusReason)
                                }
                                revealTimer.restart()
                            }
                            onHeightChanged: if (activeFocus) revealTimer.restart()
                            onClicked: {
                                switcherResults.activateCaptured(pressedKey, pressedAction)
                                pressedKey = ""
                            }
                        }
                    }
                }
            }
            HoverHandler { id: rowHover }
            ToolTip {
                id: resultToolTip
                objectName: "switcherResultTooltip"
                visible: rowHover.hovered && (resultTitle.truncated || resultDetail.truncated)
                delay: 500
                font: switcherResults.browserWindow.font
                readonly property real maximumHeight: Math.max(1,
                    (switcherResults.hostWindow ? switcherResults.hostWindow.height : switcherResults.height) / 2 - 16)
                width: Math.min(implicitWidth, Math.max(0, switcherResults.width))
                height: Math.min(implicitHeight, maximumHeight)
                x: (parent.width - width) / 2
                FontMetrics { id: tooltipMetrics; font: resultToolTip.font }
                text: resultTitle.text + (resultDetail.text.length > 0 ? "\n" + resultDetail.text : "")
                contentItem: Text {
                    text: resultToolTip.text
                    font: resultToolTip.font
                    color: switcherResults.browserWindow.primaryTextColor
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    maximumLineCount: Math.max(1, Math.min(8, Math.floor((resultToolTip.maximumHeight
                        - resultToolTip.topPadding - resultToolTip.bottomPadding) / tooltipMetrics.height)))
                    elide: Text.ElideRight
                }
                background: Rectangle {
                    color: switcherResults.browserWindow.panelColor
                    border.color: switcherResults.browserWindow.borderColor
                    radius: 3
                }
            }
        }
    }
    Label {
        objectName: "switcherEmptyState"
        anchors.centerIn: parent
        width: Math.max(0, parent.width - 24)
        visible: !switcherResults.resultsCurrent
            || (resultList.count === 0 && switcherResults.sourceNotice.length === 0)
        text: !switcherResults.resultsCurrent ? "Searching…"
            : switcherResults.sourcesPending ? "Waiting for saved items…"
            : switcherResults.sourcesFailed ? "Search is incomplete."
            : switcherResults.sourceNotice.length > 0 ? "No matching items in this session."
            : "No matching items.\nTry a different search or switch to a broader scope."
        textFormat: Text.PlainText
        color: switcherResults.browserWindow.secondaryTextColor
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
    }
}
