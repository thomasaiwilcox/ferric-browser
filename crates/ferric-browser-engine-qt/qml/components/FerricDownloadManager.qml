import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ChromePresentation.js" as ChromePresentation
import "../scripts/ListNavigation.js" as ListNavigation

FerricModalSurface {
    id: manager
    FontMetrics { id: formFont; font: manager.browserWindow.font }
    required property var downloadsModel
    property bool sessionOnly: false
    property bool listPending: false
    property bool listFailed: false
    signal closeRequested()
    signal openRequested(string downloadId, bool reveal)
    signal actionRequested(string downloadId, string action)
    property bool preparingDownload: false
    signal cancelRequestRequested()
    signal refreshRequested()
    property var expandedDownloads: ({})
    property Item lastFocusedControl: null
    onDownloadsModelChanged: { expandedDownloads = ({}); lastFocusedControl = null }
    onBrowserWindowChanged: { expandedDownloads = ({}); lastFocusedControl = null }
    onPreparingDownloadChanged: scheduleFocusRecovery()
    onListPendingChanged: scheduleFocusRecovery()
    onVisibleChanged: {
        if (visible) { if (primaryFocusTimer) primaryFocusTimer.restart() }
        else { expandedDownloads = ({}); lastFocusedControl = null }
    }
    Component.onCompleted: if (visible) primaryFocusTimer.restart()
    readonly property string historyStatusDescription: preparingDownload ? "Preparing download…" : listFailed
        ? "Download history couldn't be updated. Refresh to try again."
        : listPending && downloadList.count > 0 ? "Updating saved history…" : ""

    function toggleDetails(id) {
        var next = Object.assign({}, expandedDownloads)
        if (next[id]) delete next[id]
        else next[id] = true
        expandedDownloads = next
    }
    function normalizeSelection() {
        if (!downloadList) return
        downloadList.currentIndex = downloadList.count > 0
            ? Math.max(0, Math.min(downloadList.count - 1, downloadList.currentIndex)) : -1
    }
    function focusList() {
        lastFocusedControl = null
        normalizeSelection()
        downloadList.focusViewport()
    }
    function focusCurrentAction() {
        var row = downloadList.currentItem
        if (!row) return false
        var control = row.nextItemInFocusChain(true)
        var ancestor = control
        while (ancestor && ancestor !== row) ancestor = ancestor.parent
        if (!ancestor) return false
        downloadList.focusControl(control, Qt.TabFocusReason)
        return true
    }
    function isBacktab(event) {
        return (event.key === Qt.Key_Backtab
            && (event.modifiers === Qt.NoModifier || event.modifiers === Qt.ShiftModifier))
            || (event.key === Qt.Key_Tab && event.modifiers === Qt.ShiftModifier)
    }
    function handleNavigationKey(event) {
        var host = manager.hostWindow
        if (!host) return false
        var plain = event.modifiers === Qt.NoModifier
        var backtab = isBacktab(event)
        if (host.activeFocusItem !== downloadList && host.activeFocusItem !== downloadList.currentItem) {
            var header = downloadList.headerItem
            if (!header) return false
            var first = header.cancelRequestButton.visible ? header.cancelRequestButton
                : header.refreshButton.enabled ? header.refreshButton : header.closeButton
            if (backtab && host.activeFocusItem === first) {
                focusList()
                if (downloadList.currentIndex >= 0)
                    downloadList.positionViewAtIndex(downloadList.currentIndex, ListView.Contain)
            } else if (plain && event.key === Qt.Key_Tab && host.activeFocusItem === header.closeButton) {
                if (!focusCurrentAction()) return false
            } else return false
            event.accepted = true
            return true
        }
        if (!plain && !backtab && !(event.key === Qt.Key_G && event.modifiers === Qt.ShiftModifier)) return false
        if (event.key === Qt.Key_Up || (plain && event.key === Qt.Key_K)
                || event.key === Qt.Key_Down || (plain && event.key === Qt.Key_J)) {
            var direction = event.key === Qt.Key_Up || event.key === Qt.Key_K ? -1 : 1
            focusList()
            if (downloadList.count > 0) {
                downloadList.currentIndex = Math.max(0, Math.min(downloadList.count - 1, downloadList.currentIndex + direction))
                downloadList.positionViewAtIndex(downloadList.currentIndex, ListView.Contain)
                downloadList.focusViewport()
            }
        } else if (event.key === Qt.Key_PageUp || event.key === Qt.Key_PageDown) {
            lastFocusedControl = null
            ListNavigation.page(downloadList, event.key === Qt.Key_PageDown ? 1 : -1, manager)
            downloadList.focusViewport()
        } else if (event.key === Qt.Key_Home || event.key === Qt.Key_End || event.key === Qt.Key_G) {
            var end = event.key === Qt.Key_End || (event.key === Qt.Key_G && event.modifiers === Qt.ShiftModifier)
            focusList()
            ListNavigation.prepareViewport(downloadList, manager)
            if (downloadList.count > 0) downloadList.currentIndex = end ? downloadList.count - 1 : 0
            if (end && downloadList.count > 0) downloadList.positionViewAtEnd()
            else if (end) ListNavigation.scrollTo(downloadList, Infinity)
            else downloadList.positionViewAtBeginning()
            downloadList.focusViewport()
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (!event.isAutoRepeat && downloadList.currentItem) {
                toggleDetails(downloadList.currentItem.id)
                ListNavigation.polishRow(downloadList.currentItem)
                downloadList.forceLayout()
                ListNavigation.revealSelection(downloadList, 0)
            }
        } else if (backtab) {
            if (!downloadList.headerItem) return false
            // After distant rows change height, the header can still have an
            // estimated position. Anchor it to the first real row before the
            // shared focus reveal scrolls to its controls.
            if (downloadList.count > 0) ListNavigation.rowAt(downloadList, 0)
            downloadList.headerItem.closeButton.forceActiveFocus(Qt.BacktabFocusReason)
        } else if (event.key === Qt.Key_Tab) {
            if (!focusCurrentAction()) return false
        } else return false
        event.accepted = true
        return true
    }
    function scheduleFocusRecovery() { if (focusRecoveryTimer) focusRecoveryTimer.restart() }
    function recoverActionFocus() {
        var control = lastFocusedControl
        if (!control || (control.visible && control.enabled) || !interactive) return
        var focused = hostWindow ? hostWindow.activeFocusItem : null
        if (focused && focused !== control && focused !== downloadList && focused !== downloadList.currentItem) return
        var ancestor = control
        while (ancestor && ancestor !== downloadList.headerItem) ancestor = ancestor.parent
        lastFocusedControl = null
        if (ancestor && downloadList.headerItem) downloadList.headerItem.closeButton.forceActiveFocus()
        else if (!focusCurrentAction()) focusList()
    }
    Timer { id: primaryFocusTimer; interval: 0; onTriggered: if (manager.interactive) manager.focusList() }
    Timer { id: focusRecoveryTimer; interval: 0; onTriggered: manager.recoverActionFocus() }

    visible: browserWindow.downloadManagerVisible
    commandText: ":downloads"
    title: "Downloads"
    message: sessionOnly ? "Downloads for this session. History is not saved."
                         : "Your saved and active downloads."
    keyHelp: "j/k or ↑/↓ browse  ·  PgUp/PgDn page  ·  Home/End or g/G top/bottom  ·  enter details  ·  tab actions  ·  shift+tab back  ·  esc close"
    compactKeyHelp: "Tab · Esc"
    dialogWidth: 800 * scale
    dialogHeight: 560 * scale
    minimumBodyHeight: Math.max(100 * scale, formFont.height * 2 + 24 * scale)
    stackingOrder: 40
    initialFocusItem: downloadList
    onDismissRequested: closeRequested()

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * manager.scale

        FerricManagerList {
            id: downloadList
            objectName: "downloadList"
            hostWindow: manager.hostWindow
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: -1
            activeFocusOnTab: true
            keyNavigationEnabled: false
            onCountChanged: manager.normalizeSelection()
            Keys.onPressed: function(event) { manager.handleNavigationKey(event) }
            Accessible.role: Accessible.List
            Accessible.name: "Downloads"
            spacing: 8 * manager.scale
            model: manager.downloadsModel
            header: Flow {
                property alias cancelRequestButton: cancelRequestControl
                property alias refreshButton: refreshControl
                property alias closeButton: closeControl
                width: downloadList.width
                height: implicitHeight
                spacing: 6 * manager.scale
                Label {
                    width: Math.min(implicitWidth, parent.width)
                    text: manager.preparingDownload ? "Preparing…" : manager.listFailed ? "History unavailable"
                        : manager.listPending ? "Updating history…" : downloadList.count === 1 ? "1 download" : downloadList.count + " downloads"
                    textFormat: Text.PlainText
                    Accessible.name: manager.historyStatusDescription || text
                    color: manager.listFailed ? manager.browserWindow.warningColor : manager.browserWindow.secondaryTextColor
                    elide: Text.ElideRight
                    HoverHandler { id: historyStatusHover }
                    ToolTip.visible: historyStatusHover.hovered && manager.historyStatusDescription.length > 0
                    ToolTip.text: manager.historyStatusDescription
                }
                FerricWrappingButton {
                    id: cancelRequestControl
                    objectName: "downloadCancelRequestButton"
                    text: "Cancel request"
                    visible: manager.preparingDownload
                    onActiveFocusChanged: if (activeFocus) manager.lastFocusedControl = this
                    Keys.onPressed: function(event) { manager.handleNavigationKey(event) }
                    onClicked: manager.cancelRequestRequested()
                }
                FerricWrappingButton {
                    id: refreshControl
                    objectName: "downloadRefreshButton"
                    text: "Refresh"
                    enabled: !manager.listPending
                    opacity: enabled ? 1 : 0.5
                    onActiveFocusChanged: if (activeFocus) manager.lastFocusedControl = this
                    Keys.onPressed: function(event) { manager.handleNavigationKey(event) }
                    onClicked: manager.refreshRequested()
                }
                FerricWrappingButton {
                    id: closeControl
                    objectName: "downloadCloseButton"
                    text: "Close"
                    Accessible.name: "Close downloads"
                    onActiveFocusChanged: if (activeFocus) manager.lastFocusedControl = this
                    Keys.onPressed: function(event) { manager.handleNavigationKey(event) }
                    onClicked: manager.closeRequested()
                }
            }
            delegate: Rectangle {
                id: downloadRow
                required property int index
                required property string id
                required property string state
                required property string bytes
                required property real total
                required property real speed
                required property string destination
                required property string reason
                required property string displayName
                required property bool retryAvailable
                required property bool retryPending
                required property bool active
                required property bool fileReady
                required property bool finishing
                readonly property bool detailsExpanded: manager.expandedDownloads[downloadRow.id] === true
                width: downloadList.width
                height: downloadContent.implicitHeight + 24 * manager.scale
                color: manager.browserWindow.surfaceColor
                radius: 4 * manager.scale
                border.width: index === downloadList.currentIndex ? 2 * manager.scale : 0
                border.color: manager.browserWindow.accentColor
                Accessible.role: Accessible.ListItem
                Accessible.name: displayName + ", " + ChromePresentation.downloadStateLabel(state)
                Accessible.selected: index === downloadList.currentIndex
                onStateChanged: manager.scheduleFocusRecovery()
                onActiveChanged: manager.scheduleFocusRecovery()
                onFileReadyChanged: manager.scheduleFocusRecovery()
                onFinishingChanged: manager.scheduleFocusRecovery()
                onRetryAvailableChanged: manager.scheduleFocusRecovery()
                onRetryPendingChanged: manager.scheduleFocusRecovery()

                function selectFromControl(control) {
                    if (!control.activeFocus) return
                    manager.lastFocusedControl = control
                    if (downloadList.currentIndex !== index) {
                        var reason = control.focusReason
                        downloadList.currentIndex = index
                        downloadList.focusControl(control, reason)
                    }
                    downloadList.revealFocusedControl()
                }
                function returnToBrowsing(event, control) {
                    if (!manager.isBacktab(event) || control !== nextItemInFocusChain(true)) return
                    downloadList.currentIndex = index
                    manager.focusList()
                    downloadList.positionViewAtIndex(index, ListView.Contain)
                    event.accepted = true
                }

                ColumnLayout {
                    id: downloadContent
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 12 * manager.scale
                    spacing: 6 * manager.scale

                    Label {
                        objectName: "downloadFileName"
                        Layout.fillWidth: true
                        text: downloadRow.displayName
                        textFormat: Text.PlainText
                        font.bold: true
                        color: manager.browserWindow.primaryTextColor
                        elide: Text.ElideMiddle
                        Accessible.name: "Download " + text
                    }
                    Label {
                        Layout.fillWidth: true
                        text: (downloadRow.finishing ? "Saving file…" : ChromePresentation.downloadStateLabel(downloadRow.state))
                              + " · " + ChromePresentation.downloadProgress(downloadRow.bytes,
                                  downloadRow.total, downloadRow.speed, downloadRow.state)
                        textFormat: Text.PlainText
                        color: downloadRow.state === "interrupted"
                               ? manager.browserWindow.warningColor
                               : manager.browserWindow.secondaryTextColor
                        wrapMode: Text.WordWrap
                    }
                    Flow {
                        Layout.fillWidth: true
                        Layout.preferredHeight: childrenRect.height
                        spacing: 6 * manager.scale
                        FerricWrappingButton {
                            id: openControl
                            objectName: "downloadOpenButton"
                            enabled: downloadRow.fileReady
                            opacity: enabled ? 1 : 0.5
                            visible: downloadRow.state === "completed"
                            text: "Open"
                            onActiveFocusChanged: downloadRow.selectFromControl(openControl)
                            Keys.onPressed: function(event) { downloadRow.returnToBrowsing(event, openControl) }
                            Accessible.name: "Open " + ChromePresentation.fileName(downloadRow.destination)
                            onClicked: manager.openRequested(downloadRow.id, false)
                        }
                        FerricWrappingButton {
                            visible: downloadRow.state === "completed"
                            id: folderControl
                            objectName: "downloadFolderButton"
                            text: "Show in folder"
                            onActiveFocusChanged: downloadRow.selectFromControl(folderControl)
                            Keys.onPressed: function(event) { downloadRow.returnToBrowsing(event, folderControl) }
                            enabled: downloadRow.fileReady
                            opacity: enabled ? 1 : 0.5
                            onClicked: manager.openRequested(downloadRow.id, true)
                        }
                        FerricWrappingButton {
                            visible: downloadRow.active && !downloadRow.finishing
                                     && (downloadRow.state === "in-progress" || downloadRow.state === "paused")
                            id: pauseControl
                            objectName: "downloadPauseButton"
                            onActiveFocusChanged: downloadRow.selectFromControl(pauseControl)
                            Keys.onPressed: function(event) { downloadRow.returnToBrowsing(event, pauseControl) }
                            text: downloadRow.state === "paused" ? "Resume" : "Pause"
                            onClicked: manager.actionRequested(downloadRow.id,
                                downloadRow.state === "paused" ? "resume" : "pause")
                        }
                        FerricWrappingButton {
                            id: cancelControl
                            objectName: "downloadCancelButton"
                            onActiveFocusChanged: downloadRow.selectFromControl(cancelControl)
                            Keys.onPressed: function(event) { downloadRow.returnToBrowsing(event, cancelControl) }
                            visible: downloadRow.active && (downloadRow.finishing
                                     || downloadRow.state === "offered" || downloadRow.state === "selecting-destination"
                                     || downloadRow.state === "in-progress" || downloadRow.state === "paused")
                            text: "Cancel"
                            onClicked: manager.actionRequested(downloadRow.id, "cancel")
                        }
                        FerricWrappingButton {
                            visible: downloadRow.state === "interrupted" || downloadRow.state === "cancelled"
                            text: downloadRow.retryPending ? "Retrying…" : "Retry"
                            id: retryControl
                            objectName: "downloadRetryButton"
                            onActiveFocusChanged: downloadRow.selectFromControl(retryControl)
                            Keys.onPressed: function(event) { downloadRow.returnToBrowsing(event, retryControl) }
                            enabled: downloadRow.retryAvailable && !manager.preparingDownload
                            opacity: enabled ? 1 : 0.5
                            onClicked: manager.actionRequested(downloadRow.id, "retry")
                        }
                        FerricWrappingButton {
                            id: detailsControl
                            objectName: "downloadDetailsButton"
                            flat: true
                            onActiveFocusChanged: downloadRow.selectFromControl(detailsControl)
                            Keys.onPressed: function(event) { downloadRow.returnToBrowsing(event, detailsControl) }
                            text: downloadRow.detailsExpanded ? "Hide details" : "Details"
                            onClicked: manager.toggleDetails(downloadRow.id)
                        }
                    }
                    ProgressBar {
                        objectName: "downloadProgressBar"
                        Layout.fillWidth: true
                        // Progress is an information signal, independent of
                        // the subdued colors used for selecting text.
                        palette.highlight: manager.browserWindow.accentColor
                        palette.dark: manager.browserWindow.accentColor
                        palette.base: manager.browserWindow.panelColor
                        palette.midlight: manager.browserWindow.panelColor
                        visible: !downloadRow.finishing && (downloadRow.state === "in-progress" || downloadRow.state === "paused")
                        from: 0
                        to: 1
                        value: downloadRow.total > 0
                               ? Math.max(0, Math.min(1, Number(downloadRow.bytes) / downloadRow.total)) : 0
                        indeterminate: downloadRow.state === "in-progress" && downloadRow.total <= 0
                        Accessible.name: "Download progress"
                    }
                    Label {
                        Layout.fillWidth: true
                        text: downloadRow.destination || "Choose a location to save this file."
                        textFormat: Text.PlainText
                        color: manager.browserWindow.mutedTextColor
                        elide: Text.ElideMiddle
                        Accessible.name: "Download location: " + text
                    }
                    Label {
                        objectName: "downloadDetails"
                        Layout.fillWidth: true
                        visible: downloadRow.detailsExpanded
                        text: downloadRow.displayName + "\n"
                              + (downloadRow.destination || "Choose a location to save this file.")
                              + ((downloadRow.state === "interrupted" || downloadRow.state === "cancelled")
                                 && !downloadRow.retryAvailable && !downloadRow.retryPending
                                 ? "\nStart this download again from its page; its saved address can't be retried." : "")
                              + ((downloadRow.state === "interrupted" || downloadRow.state === "cancelled")
                                 && downloadRow.reason.length > 0 ? "\n" + downloadRow.reason : "")
                              + "\nDownload ID: " + downloadRow.id
                        textFormat: Text.PlainText
                        color: manager.browserWindow.secondaryTextColor
                        wrapMode: Text.WrapAnywhere
                    }
                }
            }

            footer: Item {
                width: downloadList.width
                height: downloadList.count === 0 ? emptyExplanation.implicitHeight : 0
                Label {
                    id: emptyExplanation
                    objectName: "downloadsEmptyState"
                    width: parent.width
                    padding: 12 * manager.scale
                    visible: downloadList.count === 0
                    text: manager.listPending ? "Loading downloads…"
                          : manager.listFailed ? "Download history couldn't be loaded.\nRefresh to try again."
                          : "No downloads yet.\nFiles you save will appear here."
                    textFormat: Text.PlainText
                    color: manager.browserWindow.secondaryTextColor
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                }
            }
        }
    }
}
