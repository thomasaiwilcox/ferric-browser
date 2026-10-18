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

    visible: switcherVisible
    commandText: ":switcher"
    title: scopeTitle
    keyHelp: "↑/↓ select  ·  pgup/pgdn page  ·  enter open  ·  esc close"
    dialogWidth: 860 * scale
    dialogHeight: 520 * scale
    stackingOrder: 60
    initialFocusItem: switcherInput
    onDismissRequested: closeRequested()
    Accessible.description: "Search up to " + maximumResults
        + " browser items; use arrow keys, Page Up, Page Down, Home, End, and Enter to choose"

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
                onTextChanged: switcher.queryChangeRequested(text)
                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Escape) {
                        switcher.closeRequested()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Down) {
                        switcherResultsList.moveBy(1)
                        event.accepted = true
                    } else if (event.key === Qt.Key_Up) {
                        switcherResultsList.moveBy(-1)
                        event.accepted = true
                    } else if (event.key === Qt.Key_PageDown) {
                        switcherResultsList.moveBy(8)
                        event.accepted = true
                    } else if (event.key === Qt.Key_PageUp) {
                        switcherResultsList.moveBy(-8)
                        event.accepted = true
                    } else if (event.key === Qt.Key_Home) {
                        switcherResultsList.moveToBeginning()
                        event.accepted = true
                    } else if (event.key === Qt.Key_End) {
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
                Accessible.description: "Current result count"
            }
        }

        FerricScrollColumn {
            id: sourceStatus
            objectName: "switcherSourceStatus"
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentLayout.implicitHeight,
                switcher.bodyItem.height * (switcher.results.length > 0 ? 0.4 : 0.75))
            hostWindow: switcher.hostWindow
            visible: switcher.resultsCurrent && switcher.sourceNotice.length > 0
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
                    onClicked: {
                        switcher.inspectedSourceDetail = switcher.sourceDetail
                        switcher.sourceDetailsVisible = true
                    }
                }
                Button {
                    objectName: "switcherSourceRefresh"
                    text: "Refresh"
                    visible: switcher.sourcesRetryable
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
