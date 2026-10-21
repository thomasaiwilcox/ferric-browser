import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: switcher
    required property bool switcherVisible
    required property var results
    property bool resultsCurrent: true
    property bool sourcesPending: false
    property bool sourcesFailed: false
    property bool sourcesRetryable: false
    property string sourceNotice: ""
    property string sourceDetail: ""
    readonly property bool inlineLoading: resultsCurrent && results.length > 0
        && sourcesPending && !sourcesFailed && !sourcesRetryable && sourceDetail.length === 0
    signal refreshRequested()
    property bool sourceDetailsVisible: false
    property string inspectedSourceDetail: ""
    function hideSourceDetails() {
        sourceDetailsVisible = false
        inspectedSourceDetail = ""
    }
    onQueryChanged: hideSourceDetails()
    onScopeChanged: hideSourceDetails()
    onSwitcherVisibleChanged: if (!switcherVisible) hideSourceDetails()
    dismissOnEscape: !sourceDetailsVisible
    property int maximumResults: 100
    property string scope: "all"
    readonly property string scopeTitle: {
        var titles = {tabs: "Tabs", windows: "Windows", contexts: "Contexts",
            commands: "Commands", history: "History", marks: "Bookmarks and quickmarks",
            sessions: "Saved sessions", downloads: "Downloads", closed: "Recently closed tabs",
            actions: "Browser actions"}
        return typeof titles[scope] === "string" ? titles[scope] : "Switcher"
    }
    property alias query: switcherInput.text
    signal queryChangeRequested(string query)
    signal closeRequested()
    signal activationRequested(int index)
    signal actionRequested(int index, string action)

    function focusInput() {
        switcherInput.forceActiveFocus()
    }

    function focusBeforeResults() {
        if (sourceRefreshButton.visible && sourceRefreshButton.enabled)
            sourceRefreshButton.forceActiveFocus(Qt.BacktabFocusReason)
        else if (sourceDetailsButton.visible && sourceDetailsButton.enabled)
            sourceDetailsButton.forceActiveFocus(Qt.BacktabFocusReason)
        else switcherInput.forceActiveFocus(Qt.BacktabFocusReason)
    }

    visible: switcherVisible
    commandText: ":switcher"
    title: scopeTitle
    keyHelp: "↑/↓ select  ·  PgUp/PgDn page  ·  Tab actions  ·  Enter open  ·  Esc close"
    compactKeyHelp: "Tab · Esc"
    dialogWidth: 860 * scale
    dialogHeight: 520 * scale
    stackingOrder: 60
    initialFocusItem: switcherInput
    onDismissRequested: closeRequested()
    Accessible.description: "Search up to " + maximumResults
        + " browser items; use arrow keys or Control+N/P, Page Up, Page Down, Control+Home/End, and Enter to choose. Tab enters the selected result's actions, after any search status controls; Shift+Tab returns. Home and End edit the search text."

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 0
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            spacing: 12 * switcher.scale
            FerricTextField {
                id: switcherInput
                objectName: "switcherSearchInput"
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                placeholderText: switcher.scopeTitle === "Switcher" ? "Search browser items" : "Search " + switcher.scopeTitle.toLowerCase()
                Accessible.name: switcher.scopeTitle + " search"
                Accessible.role: Accessible.EditableText
                Accessible.editable: true
                Accessible.description: switcher.inlineLoading ? switcher.sourceNotice : ""
                ProgressBar {
                    objectName: "switcherSearchProgress"
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.leftMargin: 2
                    anchors.rightMargin: 2
                    anchors.bottomMargin: 1
                    height: 4 * switcher.scale
                    padding: 0
                    visible: switcher.inlineLoading
                    indeterminate: visible
                    Accessible.name: switcher.sourceNotice || "Searching browser items"
                }
                onTextChanged: switcher.queryChangeRequested(text)
                Keys.onPressed: function(event) {
                    var plain = event.modifiers === Qt.NoModifier
                    var control = event.modifiers === Qt.ControlModifier
                    if (event.key === Qt.Key_Escape) {
                        switcher.closeRequested()
                        event.accepted = true
                    } else if (plain && event.key === Qt.Key_Tab
                            && !sourceDetailsButton.visible && !sourceRefreshButton.visible) {
                        event.accepted = switcherResultsList.focusCurrentAction()
                    } else if ((plain && event.key === Qt.Key_Down)
                            || (control && event.key === Qt.Key_N)) {
                        switcherResultsList.moveBy(1)
                        event.accepted = true
                    } else if ((plain && event.key === Qt.Key_Up)
                            || (control && event.key === Qt.Key_P)) {
                        switcherResultsList.moveBy(-1)
                        event.accepted = true
                    } else if (plain && event.key === Qt.Key_PageDown) {
                        switcherResultsList.movePage(1)
                        event.accepted = true
                    } else if (plain && event.key === Qt.Key_PageUp) {
                        switcherResultsList.movePage(-1)
                        event.accepted = true
                    } else if (control && event.key === Qt.Key_Home) {
                        switcherResultsList.moveToBeginning()
                        event.accepted = true
                    } else if (control && event.key === Qt.Key_End) {
                        switcherResultsList.moveToEnd()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        if (!event.isAutoRepeat && switcher.resultsCurrent)
                            switcher.activationRequested(switcherResultsList.currentIndex)
                        event.accepted = true
                    }
                }
            }
            Label {
                objectName: "switcherResultCount"
                textFormat: Text.PlainText
                text: !switcher.resultsCurrent ? "Searching…"
                    : switcher.sourcesPending && switcher.results.length === 0 ? "Loading…"
                    : switcher.results.length + (switcher.results.length === 1 ? " result" : " results")
                color: switcher.browserWindow.mutedTextColor
                font.pointSize: Math.max(10, switcher.browserWindow.font.pointSize - 2)
                Accessible.name: text
                Accessible.role: Accessible.StatusBar
                Accessible.description: switcher.sourcesPending
                    ? "Current result count; " + (switcher.sourceNotice || "Searching browser items")
                    : "Current result count"
            }
        }

        FerricScrollColumn {
            id: sourceStatus
            objectName: "switcherSourceStatus"
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentLayout.implicitHeight,
                switcher.bodyItem.height * (switcher.results.length > 0 ? 0.4 : 0.75))
            hostWindow: switcher.hostWindow
            visible: switcher.resultsCurrent && switcher.sourceNotice.length > 0 && !switcher.inlineLoading
            FerricManagerFeedback {
                id: sourceFeedback
                Layout.fillWidth: true
                browserWindow: switcher.browserWindow
                message: switcher.sourceNotice
                detail: ""
                failed: switcher.sourcesFailed
                pending: switcher.sourcesPending
            }
            Flow {
                Layout.fillWidth: true
                Layout.preferredHeight: childrenRect.height
                spacing: 6 * switcher.scale
                Button {
                    id: sourceDetailsButton
                    objectName: "switcherSourceDetailsButton"
                    text: "Show details"
                    visible: switcher.sourceDetail.length > 0
                    Keys.onPressed: function(event) {
                        if (event.key === Qt.Key_Tab && event.modifiers === Qt.NoModifier
                                && !sourceRefreshButton.visible)
                            event.accepted = switcherResultsList.focusCurrentAction()
                    }
                    onClicked: {
                        switcher.inspectedSourceDetail = switcher.sourceDetail
                        switcher.sourceDetailsVisible = true
                    }
                }
                Button {
                    id: sourceRefreshButton
                    objectName: "switcherSourceRefresh"
                    text: "Refresh"
                    visible: switcher.sourcesRetryable
                    Keys.onPressed: function(event) {
                        if (event.key === Qt.Key_Tab && event.modifiers === Qt.NoModifier)
                            event.accepted = switcherResultsList.focusCurrentAction()
                    }
                    onClicked: switcher.refreshRequested()
                }
            }
        }

        FerricSwitcherResults {
            id: switcherResultsList
            Layout.fillWidth: true
            Layout.fillHeight: true
            browserWindow: switcher.browserWindow
            hostWindow: switcher.hostWindow
            results: switcher.results
            resultsCurrent: switcher.resultsCurrent
            sourcesPending: switcher.sourcesPending
            sourcesFailed: switcher.sourcesFailed
            sourceNotice: switcher.sourceNotice
            onFocusSearchRequested: switcher.focusInput()
            onFocusPreviousRequested: switcher.focusBeforeResults()
            onActivationRequested: function(index) { switcher.activationRequested(index) }
            onActionRequested: function(index, action) { switcher.actionRequested(index, action) }
        }
    }
    FerricModalSurface {
        id: sourceDetails
        objectName: "switcherDetailsDialog"
        parent: switcher.parent
        browserWindow: switcher.browserWindow
        hostWindow: switcher.hostWindow
        visible: switcher.switcherVisible && switcher.sourceDetailsVisible
        commandText: ":switcher"
        title: "Search details"
        keyHelp: "ctrl+c copy selection  ·  esc back"
        compactKeyHelp: "esc back"
        dialogWidth: 760 * scale
        dialogHeight: 560 * scale
        stackingOrder: switcher.stackingOrder + 1
        initialFocusItem: sourceDetailsText
        onDismissRequested: switcher.hideSourceDetails()
        FerricScrollColumn {
            anchors.fill: parent
            hostWindow: switcher.hostWindow
            TextArea {
                id: sourceDetailsText
                objectName: "switcherSourceDetails"
                Layout.fillWidth: true
                text: switcher.inspectedSourceDetail
                textFormat: TextEdit.PlainText
                wrapMode: TextEdit.WrapAnywhere
                readOnly: true
                selectByMouse: true
                selectByKeyboard: true
                color: switcher.browserWindow.secondaryTextColor
                background: Rectangle { color: switcher.browserWindow.surfaceColor }
                Accessible.name: "Search error details"
            }
        }
    }

}
