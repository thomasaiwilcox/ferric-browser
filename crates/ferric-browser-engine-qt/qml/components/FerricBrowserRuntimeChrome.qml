import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as BrowserScripts
import "../scripts/SpellcheckPresentation.js" as SpellcheckPresentation

FerricBrowserRuntimeRequests {
    id: window

    popupWindowComponentObject: popupWindowComponent
    browserWindowComponentObject: browserWindowComponent
    webViewComponentObject: webViewComponent
    webViewsObject: webViews

    readonly property alias commandSurfaceObject: commandSurface
    readonly property alias searchSurfaceObject: searchSurface
    readonly property alias attachedDevToolsLoaderObject: attachedDevToolsLoader

    Component {
        id: popupWindowComponent

        FerricPopupWindow {
            rootWindow: window
        }
    }
    Component {
        id: browserWindowComponent

        FerricBrowserWindow {
            rootWindow: window
        }
    }

    FerricWebEngineSurfaceRecovery {
        hostWindow: window
        enabled: window.nativeWayland && !window.softwareRendering
        views: [
            window.activeWebView(),
            attachedDevToolsLoader.active ? attachedDevToolsLoader.item : null
        ]
    }

    header: ToolBar {
id: browserHeader
height: window.tabPosition === "top" && window.tabStripVisible
        ? window.tabBarHeight : 0
visible: height > 0

background: Rectangle {
    color: window.panelColor

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: window.borderColor
    }
}

GridLayout {
    id: headerColumn
    columns: 1
    anchors.fill: parent
    rowSpacing: 0

    FerricTabStrip {
        id: compactTabStrip
        browserWindow: window
        tabsModel: tabs
        activeTabIndex: browserUi.active_tab_index
        vertical: window.sideTabs
        parent: window.tabPosition === "top" ? headerColumn : window.contentItem
        x: parent === headerColumn ? 0
           : (window.tabPosition === "right" ? parent.width - width : 0)
        y: parent === headerColumn || vertical ? 0
           : parent.height - window.bottomChromeHeight
        width: parent === headerColumn ? headerColumn.width
               : (vertical ? window.sideTabWidth : parent.width)
        height: parent === headerColumn ? window.tabBarHeight
                : (vertical
                   ? parent.height
                     - window.bottomChromeHeight
                   : window.tabBarHeight)
        z: 9
        Layout.row: 0
        Layout.fillWidth: parent === headerColumn
        Layout.preferredHeight: window.tabBarHeight
        Layout.maximumHeight: visible && parent === headerColumn
                              ? window.tabBarHeight : 0
        visible: window.tabStripVisible

        onSelectRequested: function(tabIndex) {
            var tabId = browserUi.tab_id_for_index(tabIndex)
            if (tabId.length > 0
                    && browserUi.execute_ui_action("browser.tab.select", tabId)) {
                tabs.setProperty(tabIndex, "loaded", true)
                window.executePendingEngineAction()
            }
        }
        onCloseRequested: function(tabIndex) {
            if (window.closeTabAtIndex(tabIndex)) {
                window.executePendingEngineAction()
            }
        }
        onNewRequested: {
            var index = browserUi.new_tab()
            if (index >= 0) {
                tabs.append({ url: "about:blank", title: "New tab",
                    loaded: true, pinned: false, muted: false, zoom: 1.0,
                    suspended: false, discarded: false })
                window.syncTabModel()
            }
        }
    }

    RowLayout {
        Layout.row: 1
        Layout.fillWidth: true
        Layout.preferredHeight: 0
        Layout.maximumHeight: 0
        spacing: 8
        visible: false

        ToolButton {
            text: "‹"
            Accessible.name: "Back"
            onClicked: {
                var historyView = window.activeWebView()
                if (historyView && historyView.canGoBack) {
                    browserUi.back()
                    historyView.goBack()
                } else {
                    browserUi.status_text = "History boundary reached"
                }
            }
        }
        ToolButton {
            text: "›"
            Accessible.name: "Forward"
            onClicked: {
                var historyView = window.activeWebView()
                if (historyView && historyView.canGoForward) {
                    browserUi.forward()
                    historyView.goForward()
                } else {
                    browserUi.status_text = "History boundary reached"
                }
            }
        }
        TextField {
            id: address
            Layout.fillWidth: true
            property bool addressEditing: false
            function refreshAddressPresentation() {
                if (!addressEditing) {
                    text = window.addressPresentation(
                        browserUi.display_url,
                        width / Math.max(1, font.pixelSize * 0.56))
                }
            }
            text: window.addressPresentation(
                browserUi.display_url,
                width / Math.max(1, font.pixelSize * 0.56))
            placeholderText: "Enter a URL"
            Accessible.name: "Address"
            Accessible.description: browserUi.display_url
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
            selectByMouse: true
            onActiveFocusChanged: {
                addressEditing = activeFocus
                if (addressEditing) {
                    text = browserUi.display_url
                    cursorPosition = text.length
                } else {
                    refreshAddressPresentation()
                }
            }
            onWidthChanged: refreshAddressPresentation()
            Connections {
                target: browserUi
                function onDisplay_urlChanged() {
                    address.refreshAddressPresentation()
                }
            }
            onAccepted: {
                browserUi.navigate(text)
                window.updateActiveTabUrl()
            }
        }
        ToolButton {
            text: "⟳"
            Accessible.name: "Reload"
            onClicked: {
                browserUi.reload()
                window.activeWebView()?.reload()
            }
        }
        Label {
    text: browserUi.mode + " · " + browserUi.status_text
          + window.contextStatus(
              browserUi, window.temporaryProfile, window.profileName,
              window.ephemeralProfile)
          + window.siteDoctorBadge(browserUi)
          + (window.recoveryAvailable ? " · recovery available" : "")
            Accessible.name: "Browser status"
            color: window.contextStatusColor(
                       browserUi, window.primaryTextColor)
        }
        ToolButton {
            text: "Window"
            visible: false
            Accessible.name: "New profile window"
            onClicked: {
                window.browserWindowFactory.createObject(null, {
                    windowStartupUrl: browserUi.current_url,
                    windowProfileName: "secondary",
                    windowPrivateProfile: false
                })
            }
        }
        ToolButton {
            text: "Private"
            visible: false
            contentItem: Text {
                text: parent.text
                color: window.privateColor
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
            }
            Accessible.name: "New private window"
            onClicked: {
                window.browserWindowFactory.createObject(null, {
                    windowStartupUrl: "about:blank",
                    windowProfileName: "private",
                    windowPrivateProfile: true
                })
            }
        }
        ToolButton {
            text: "Sessions"
            visible: false
            Accessible.name: "Open session manager"
            onClicked: {
                window.refreshSessions()
                window.openInternalSurface()
                window.sessionManagerVisible = true
            }
        }
        ToolButton {
            text: "Profiles"
            visible: false
            Accessible.name: "Open profile manager"
            onClicked: {
                window.refreshProfiles()
                window.openInternalSurface()
                window.profileManagerVisible = true
            }
        }
        ToolButton {
            text: "Downloads"
            visible: false
            Accessible.name: "Open downloads manager"
            onClicked: {
                window.refreshDownloads()
                window.openInternalSurface()
                window.downloadManagerVisible = true
            }
        }
        ToolButton {
            text: "Diagnostics"
            visible: false
            Accessible.name: "Open diagnostics"
            onClicked: window.showDiagnostics()
        }
        ToolButton {
            text: "Help"
            visible: false
            Accessible.name: "Open binding help"
            onClicked: window.showBindingHelp()
        }
        ToolButton {
            text: "Settings"
            visible: false
            Accessible.name: "Open settings"
            onClicked: window.showSettings()
        }
        ToolButton {
            text: "Undo tab"
            visible: false
            Accessible.name: "Reopen most recently closed tab"
            onClicked: {
                if (browserUi.execute_ui_action("browser.tab.undo", "")) {
                    window.syncTabModel()
                    window.executePendingEngineAction()
                }
            }
        }
        ToolButton {
            text: "Clone tab"
            visible: false
            Accessible.name: "Clone current tab"
            onClicked: {
                if (browserUi.execute_command(":tab-clone")) {
                    window.syncTabModel()
                    window.executePendingEngineAction()
                }
            }
        }
        ToolButton {
            text: "Reopen window"
            visible: false
            Accessible.name: "Reopen tab in same-profile window; live state will be lost"
            onClicked: {
                if (browserUi.execute_ui_action("browser.tab.reopen-window", "")) {
                    window.executePendingEngineAction()
                }
            }
        }
        ToolButton {
            text: "−"
            visible: false
            Accessible.name: "Zoom out"
            onClicked: {
                if (browserUi.execute_ui_action("browser.tab.zoom", "out")) {
                    window.executePendingEngineAction()
                }
            }
        }
        ToolButton {
            text: "100%"
            visible: false
            Accessible.name: "Reset page zoom"
            onClicked: {
                if (browserUi.execute_ui_action("browser.tab.zoom", "reset")) {
                    window.executePendingEngineAction()
                }
            }
        }
        ToolButton {
            text: "+"
            visible: false
            Accessible.name: "Zoom in"
            onClicked: {
                if (browserUi.execute_ui_action("browser.tab.zoom", "in")) {
                    window.executePendingEngineAction()
                }
            }
        }
    }

    RowLayout {
        Layout.row: 2
        Accessible.role: Accessible.PageTabList
        Accessible.name: "Browser tabs"
        Layout.fillWidth: true
        spacing: 4
        visible: false

        Repeater {
            id: tabButtons
            model: tabs
            delegate: RowLayout {
                property int tabIndex: index
                spacing: 0
                Layout.preferredWidth: Math.min(400, Math.max(150, tabLabel.implicitWidth + pinTab.implicitWidth + muteTab.implicitWidth + lifecycleTab.implicitWidth + discardTab.implicitWidth + moveLeft.implicitWidth + moveRight.implicitWidth + closeTab.implicitWidth + 24))

                ToolButton {
                    id: tabLabel
                    Layout.fillWidth: true
                    text: (title || "New tab")
                          + (window.permissionPendingForTab(tabIndex)
                             ? " · permission"
                             : "")
                          + (tabViewAt(tabIndex)
                             && tabViewAt(tabIndex).rendererFailed
                             ? " · renderer failed"
                             : "")
                    highlighted: tabIndex === browserUi.active_tab_index
                    Accessible.role: Accessible.PageTab
                    Accessible.name: "Tab " + (tabIndex + 1) + ": " + text
                    Accessible.selected: tabIndex === browserUi.active_tab_index
                    Accessible.description: model.pinned
                          ? "Pinned tab"
                          : "Browser tab"
                    onClicked: {
                        var tabId = browserUi.tab_id_for_index(tabIndex)
                        if (tabId.length === 0
                                || !browserUi.execute_ui_action(
                                    "browser.tab.select", tabId)) {
                            return
                        }
                        if (browserUi.active_tab_index === tabIndex) {
                            tabs.setProperty(tabIndex, "loaded", true)
                            window.executePendingEngineAction()
                        }
                        window.showRendererFailureForTab(tabIndex)
                    }
                }
                ToolButton {
                    id: pinTab
                    text: model.pinned ? "📌" : "pin"
                    Accessible.name: model.pinned ? "Unpin tab " + (tabIndex + 1) : "Pin tab " + (tabIndex + 1)
                    onClicked: {
                        var nextPinned = !model.pinned
                        var tabId = browserUi.tab_id_for_index(tabIndex)
                        if (tabId.length === 0
                                || !browserUi.execute_ui_action("browser.tab.pin", tabId)) {
                            return
                        }
                        var newIndex = browserUi.tab_index_for_id(tabId)
                        if (newIndex >= 0 && newIndex !== tabIndex) {
                            tabs.move(tabIndex, newIndex, 1)
                        }
                        if (newIndex >= 0) {
                            tabs.setProperty(newIndex, "pinned", nextPinned)
                        }
                        window.syncTabModel()
                    }
                }
                ToolButton {
                    id: muteTab
                    text: model.muted ? "🔇" : "mute"
                    Accessible.name: model.muted ? "Unmute tab " + (tabIndex + 1) : "Mute tab " + (tabIndex + 1)
                    onClicked: {
                        var nextMuted = !model.muted
                        var tabId = browserUi.tab_id_for_index(tabIndex)
                        if (tabId.length === 0
                                || !browserUi.execute_ui_action("browser.tab.mute", tabId)) {
                            return
                        }
                        var newIndex = browserUi.tab_index_for_id(tabId)
                        if (newIndex >= 0) {
                            tabs.setProperty(newIndex, "muted", nextMuted)
                        }
                    }
                }
                ToolButton {
                    id: lifecycleTab
                    text: model.suspended || model.discarded ? "resume" : "freeze"
                    enabled: model.suspended || model.discarded || tabIndex !== browserUi.active_tab_index
                    Accessible.name: model.suspended || model.discarded
                                      ? "Resume tab " + (tabIndex + 1)
                                      : "Freeze hidden tab " + (tabIndex + 1)
                    onClicked: {
                        var tabId = browserUi.tab_id_for_index(tabIndex)
                        var action = model.suspended || model.discarded
                                ? "browser.tab.resume" : "browser.tab.suspend"
                        if (tabId.length === 0
                                || !browserUi.execute_ui_action(action, tabId)) {
                            return
                        }
                        window.executePendingEngineAction()
                    }
                }
                ToolButton {
                    id: discardTab
                    text: "discard"
                    enabled: !model.suspended && !model.discarded
                             && tabIndex !== browserUi.active_tab_index
                    Accessible.name: "Discard hidden tab " + (tabIndex + 1)
                    onClicked: {
                        var tabId = browserUi.tab_id_for_index(tabIndex)
                        if (tabId.length === 0
                                || !browserUi.execute_ui_action("browser.tab.discard", tabId)) {
                            return
                        }
                        window.executePendingEngineAction()
                    }
                }
                ToolButton {
                    id: moveContext
                    text: "context"
                    Accessible.name: "Move tab " + (tabIndex + 1) + " to another context window"
                    enabled: !model.suspended && !model.discarded
                    onClicked: {
                        var tabId = browserUi.tab_id_for_index(tabIndex)
                        if (tabId.length > 0) {
                            window.showContextMovePicker(tabId)
                        }
                    }
                }
                ToolButton {
                    id: moveLeft
                    text: "←"
                    Accessible.name: "Move tab left"
                    onClicked: {
                        var tabId = browserUi.tab_id_for_index(tabIndex)
                        if (tabId.length === 0
                                || !browserUi.execute_ui_action(
                                    "browser.tab.move", tabId + "\tleft")) {
                            return
                        }
                        var newIndex = browserUi.tab_index_for_id(tabId)
                        if (newIndex >= 0 && newIndex !== tabIndex) {
                            tabs.move(tabIndex, newIndex, 1)
                        }
                        window.syncTabModel()
                    }
                }
                ToolButton {
                    id: moveRight
                    text: "→"
                    Accessible.name: "Move tab right"
                    onClicked: {
                        var tabId = browserUi.tab_id_for_index(tabIndex)
                        if (tabId.length === 0
                                || !browserUi.execute_ui_action(
                                    "browser.tab.move", tabId + "\tright")) {
                            return
                        }
                        var newIndex = browserUi.tab_index_for_id(tabId)
                        if (newIndex >= 0 && newIndex !== tabIndex) {
                            tabs.move(tabIndex, newIndex, 1)
                        }
                        window.syncTabModel()
                    }
                }
                ToolButton {
                    id: closeTab
                    text: "×"
                    Accessible.name: "Close tab " + (tabIndex + 1)
                    onClicked: {
                        if (window.closeTabAtIndex(tabIndex)) {
                            window.executePendingEngineAction()
                        }
                    }
                }
            }
        }

        ToolButton {
            text: "+"
            Accessible.name: "New tab"
            onClicked: {
                var index = browserUi.new_tab()
                if (index >= 0) {
                    tabs.append({ url: "about:blank", title: "New tab", loaded: true, pinned: false, muted: false, zoom: 1.0, suspended: false, discarded: false })
                    window.syncTabModel()
                }
            }
        }
    }
}

}

    FerricContextMoveDialog {
        id: contextMovePopup
        browserWindow: window
    }

    FerricExternalNavigationDialog {
        id: externalNavigationPopup
        browserWindow: window
        browserUi: window.browserUi
    }

    FerricContextRouteDialog {
        id: contextRoutePopup
        browserWindow: window
    }

    FerricNavigationFailure {
        id: navigationFailureSurface
        browserWindow: window
        browserUi: window.browserUi
    }

    FerricRecoveryBanner {
        browserWindow: window
        onRecoveryRequested: {
            var restored = browserUi.recover_session()
            if (restored === "QUEUED") {
                window.sessionPreviewLoading = true
                return
            }
            if (restored.length === 0) {
                return
            }
            tabs.clear()
            window.applyRestorePayload(restored, false)
            window.recoveryAvailable = false
            window.syncTabModel()
            window.executePendingEngineAction()
        }
        onDismissalRequested: window.recoveryAvailable = false
    }

    FerricSiteLedger {
        browserWindow: window
        siteExperimentAvailable: !browserUi.site_experiment_active
        onCloseRequested: {
            window.siteLedgerVisible = false
            window.closeInternalSurface()
        }
        onRefreshRequested: window.showSiteLedger()
        onActiveOriginDataClearRequested: {
            if (browserUi.site_data_clear(window.siteLedgerData.origin, true)) {
                window.executePendingEngineAction()
            }
        }
        onSanitizedReportCopyRequested: function(includeHost) {
            var report = browserUi.site_report(includeHost)
            if (report && report.length > 0) {
                window.copyToClipboard(report, true)
                browserUi.status_text = "Sanitized site report copied"
            }
        }
        onSiteDoctorProposalApplyRequested: function(proposalId) {
            if (browserUi.apply_site_doctor_proposal(proposalId, true)) {
                window.showSiteLedger()
            }
        }
        onSiteDoctorExperimentRequested: function(kind) {
            var started = browserUi.begin_site_doctor_experiment(kind)
            if (started.length > 0) {
                window.executePendingEngineAction()
            }
        }
    }

    FerricDiagnostics {
        browserWindow: window
        onCloseRequested: window.closeDiagnostics()
        onRefreshRequested: window.refreshDiagnostics()
        onCopyRequested: window.copyToClipboard(window.diagnosticsText, true)
        onSaveRequested: {
            window.refreshDiagnostics()
            fileDialogSurfaces.diagnosticsExportCurrentFile = "ferric-browser-diagnostics.json"
            fileDialogSurfaces.openDiagnosticsExport()
        }
    }

    FerricBindingHelp {
        browserWindow: window
        onCloseRequested: window.closeBindingHelp()
        onRefreshRequested: window.refreshBindingHelp()
        onSearchChanged: function(text) {
            window.bindingHelpSearch = text
            window.refreshBindingHelp()
        }
    }

    FerricSettingsModel { id: settingsModel }

    FerricSettings {
        id: settingsSurface
        browserWindow: window
        settingsVisible: window.settingsVisible
        temporary: window.settingsTemporary
        searchText: window.settingsSearch
        notice: window.settingsNotice
        spellcheckStatus: window.spellcheckStatusText(browserUi)
        spellcheckDictionariesMissing: window.spellcheckDictionaryStatus(browserUi).missing.length > 0
        userscriptRows: window.userscriptInventoryRows()
        settingsModel: settingsModel
        onCloseRequested: window.closeSettings()
        onRefreshRequested: window.refreshSettings()
        onInstallUserscriptRequested: window.chooseUserscriptManifest()
        onTemporaryChangeRequested: function(temporary) {
            window.settingsTemporary = temporary
            window.settingsNotice = temporary
                    ? "Changes apply only to this window and session"
                    : "Changes use the profile runtime override layer"
        }
        onSearchChanged: function(text) {
            window.settingsSearch = text
            window.refreshSettings()
        }
        onUserscriptEnabledRequested: function(name, enabled) {
            if (browserUi.set_userscript_enabled(name, enabled)) {
                settingsSurface.setUserscriptEnabled(name, enabled)
            } else {
                settingsSurface.setUserscriptEnabled(name, !enabled)
            }
        }
        onUserscriptRemovalRequested: function(name) { window.confirmRemoveUserscript(name) }
        onSettingApplyRequested: function(row, value) { window.applySetting(row, value) }
        onSettingResetRequested: function(row) { window.resetSetting(row) }
    }

    FerricLibraryManager {
        id: libraryManager
        browserWindow: window
        managerVisible: window.libraryManagerVisible
        libraryKind: browserUi.library_kind
        graphMode: window.libraryJourneyGraphMode
        totalEntries: window.libraryTotalEntries
        page: window.libraryPage
        pageCount: window.libraryPageCount()
        pageSize: window.libraryPageSize
        journeySearchText: window.libraryJourneySearchText
        journeyCurrentOnly: window.libraryJourneyCurrentOnly
        pendingDelete: window.pendingLibraryDelete
        entries: libraryEntries
        graphNodes: libraryGraphNodes
        graphEntries: libraryGraphEntries
        graphLineData: window.libraryGraphLineData
        profilesModel: profiles
        profilesLoading: browserUi.profile_values_pending
        onCloseRequested: {
            window.libraryManagerVisible = false
            window.closeInternalSurface()
        }
        onGraphToggleRequested: window.libraryJourneyGraphMode = !window.libraryJourneyGraphMode
        onExportRequested: window.beginJourneyExport()
        onPageChangeRequested: function(delta) { window.changeLibraryPage(delta) }
        onJourneySearchRequested: function(text) {
            window.libraryJourneySearchText = text
            window.libraryJourneyExpandedNode = ""
            window.libraryJourneyCurrentOnly = false
            window.runJourneyQuery(window.journeySearchArgument(text))
        }
        onJourneyClearRequested: {
            window.libraryJourneySearchText = ""
            window.libraryJourneyExpandedNode = ""
            window.libraryJourneyCurrentOnly = false
            window.runJourneyQuery("")
        }
        onJourneyCurrentRequested: window.runJourneyCurrentQuery()
        onJourneyAllRequested: window.runJourneyAllQuery()
        onEntryOpenRequested: function(entryKind, entryId) {
            window.openLibraryEntry(entryKind, entryId)
        }
        onPrivateHistoryTransferRequested: function(entryId, title, url) {
            window.showPrivateHistoryTransfer(entryId, title, url)
        }
        onEntryEditRequested: function(entryKind, entryId, value) {
            if (window.editLibraryEntry(entryKind, entryId, value)) {
                libraryManager.markEditSaved(entryKind, entryId)
            }
        }
        onEntryDeleteRequested: function(entryKind, entryId) {
            window.deleteLibraryEntry(entryKind, entryId)
        }
        onJourneyReopenRequested: function(nodeId, target) {
            var command = ":journey-reopen " + window.libraryCommandArgument(nodeId)
                    + " --target " + target
            if (browserUi.execute_command(command)) {
                window.syncTabModel()
                window.executePendingEngineAction()
                window.libraryManagerVisible = false
                window.closeInternalSurface()
            }
        }
        onJourneyExpandRequested: function(nodeId) {
            window.libraryJourneyExpandedNode = nodeId
            window.libraryJourneySearchText = ""
            window.libraryJourneyCurrentOnly = false
            window.runJourneyQuery(window.journeyExpandArgument(nodeId))
        }
        onJourneyExportFileRequested: {
            fileDialogSurfaces.journeyExportCurrentFile = "journey-export.json"
            fileDialogSurfaces.openJourneyExport()
        }
        onJourneyExportCancelRequested: window.journeyExportPreviewVisible = false
        onPrivateHistoryReopenRequested: function(profileName) {
            window.confirmPrivateHistoryTransfer(profileName)
        }
        onPrivateHistoryBookmarkRequested: function(profileName, profileLabel) {
            window.confirmPrivateHistoryBookmark(profileName, profileLabel)
        }
        onPrivateHistoryCancelRequested: window.cancelPrivateHistoryTransfer()
    }

    FerricSwitcher {
        id: switcherSurface
        browserWindow: window
        switcherVisible: window.switcherVisible
        results: window.switcherResults
        onQueryChangeRequested: function(query) {
            if (window.switcherVisible) {
                window.cancelSwitcherBatch()
                switcherRefreshTimer.restart()
            }
        }
        onCloseRequested: window.closeSwitcher()
        onActivationRequested: function(index) { window.activateSwitcher(index) }
        onActionRequested: function(index, action) {
            window.activateSwitcherAction(index, action)
        }
    }

    FerricLinkPreview {
        browserWindow: window
        previewUi: browserUi
        onCloseRequested: window.closeLinkPreview()
        onNavigationConfirmed: {
            if (browserUi.confirm_link_navigation()) {
                window.executePendingEngineAction()
                window.closeLinkPreview()
                window.syncTabModel()
            }
        }
    }

    FerricDownloadManager {
        browserWindow: window
        downloadsModel: downloads
        onCloseRequested: {
            window.downloadManagerVisible = false
            window.closeInternalSurface()
        }
        onOpenRequested: function(downloadId, reveal) {
            window.openDownload(downloadId, reveal)
        }
        onActionRequested: function(downloadId, action) {
            window.requestDownloadAction(downloadId, action)
        }
    }

    Timer {
        id: downloadMetricsTimer
        interval: 1000
        repeat: true
        running: window.downloadManagerVisible
        onTriggered: window.refreshDownloads()
    }

    FerricProfileSessionSurfaces {
        id: profileSessionSurfaces
        anchors.fill: parent
        z: 50
        browserWindow: window
        browserUi: window.browserUi
        profilesModel: profiles
        namedSessionsModel: namedSessions
        onCreateProfileRequested: function(name, label) {
            if (browserUi.create_profile(name, label)) {
                profileSessionSurfaces.clearProfileCreateInputs()
                window.scheduleProfileRefresh()
            }
        }
        onRenameProfileRequested: function(name, label) {
            if (browserUi.rename_profile(name, label)) {
                profileSessionSurfaces.clearProfileRenameInputs()
                window.scheduleProfileRefresh()
            }
        }
        onOpenProfileRequested: function(name, label) {
            window.openProfile(name, label)
        }
        onDeleteProfileRequested: function(name) {
            if (window.profileDeletePreviewVisible) {
                if (browserUi.delete_profile(window.profileDeleteName, true)) {
                    window.profileDeletePreviewVisible = false
                    window.profileManagerVisible = false
                    window.closeInternalSurface()
                }
            } else {
                window.showProfileDeletePreview(name)
            }
        }
        onSaveSessionRequested: function(name) {
            if (browserUi.save_named_session(name)) {
                window.refreshSessions()
            }
        }
        onDeleteSessionRequested: function(name) {
            if (browserUi.delete_named_session(name, true)) {
                window.pendingDeleteName = ""
                window.refreshSessions()
            }
        }
    }

    StackLayout {
        id: webViews
        anchors.fill: parent
        anchors.leftMargin: window.tabStripVisible && window.tabPosition === "left"
                            ? window.sideTabWidth : 0
        anchors.rightMargin: window.tabStripVisible && window.tabPosition === "right"
                             ? window.sideTabWidth : 0
        anchors.bottomMargin: window.bottomChromeHeight
        currentIndex: browserUi.active_tab_index

        Component {
            id: webViewComponent

            WebEngineView {
                id: webView
                property int tabIndex: -1
                property string stableTabId: ""
                property var viewUi: browserUi
                property var viewHost: window
                property var viewTabs: tabs
                property var viewModel: ({})
                property var viewProfile: browserProfile
                property var viewInterceptor: window.primaryRequestInterceptor
                property bool viewTransientProfile: window.temporaryProfile
                property bool viewTransferred: false
                property var pageUserScriptNames: []
                property string pageUserScriptReloadUrl: ""
                property string pageDialogDocumentKey: ""
                property int pageDialogCount: 0
                property bool pageDialogSuppressed: false
                property bool rendererFailed: false
                property int rendererFailureCount: 0
                property double rendererFailureAt: 0
                property string rendererFailureSafeUrl: ""
                property string rendererFailureReason: ""
                property int rendererFailureExitCode: 0
                property bool scrollPositionRestored: false
                devToolsView: window.devToolsVisible
                        ? (window.devToolsDetached
                           ? window.devToolsExternalView : attachedDevToolsLoader.item)
                        : null
                // Navigation-scoped content settings are snapshotted per
                // document; live config changes apply to the next navigation.
                property var effectiveSiteSettings: ({ values: {}, matched_rules: [] })
                function refreshEffectiveSiteSettings() {
                    effectiveSiteSettings = window.siteRuleSettingsFor(
                        viewUi, webView.url.toString())
                }
                Connections {
                    target: viewUi
                    function onSite_experiment_kindChanged() {
                        webView.refreshEffectiveSiteSettings()
                    }
                    function onActive_tab_indexChanged() {
                        if (viewUi === browserUi
                                && tabIndex === viewUi.active_tab_index) {
                            window.refreshBlockingEvidence()
                            window.updateMprisForPrimaryView(webView)
                            window.scheduleFocusProbe(2)
                        }
                    }
                }
                property int blockedRequestCount: {
                    var host = webView.url && webView.url.host
                            ? String(webView.url.host) : ""
                    var counts = viewInterceptor.blockedSiteCounts || ({})
                    return host.length > 0 && counts[host] !== undefined ? Number(counts[host]) : 0
                }
                profile: viewProfile
                url: viewModel.loaded ? viewModel.url : "about:blank"
                audioMuted: !!viewModel.muted
                zoomFactor: Number(window.siteRuleValue(
                    effectiveSiteSettings, "content.zoom", viewModel.zoom))
                settings.javascriptEnabled: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.javascript", true)
                settings.autoLoadImages: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.images", true)
                settings.forceDarkMode: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.force_dark", false)
                settings.playbackRequiresUserGesture:
                    window.siteRuleValue(effectiveSiteSettings, "content.autoplay", "engine-default")
                    === "require-gesture"
                Accessible.name: "Web content for tab " + (tabIndex + 1)
                Component.onCompleted: {
                    refreshEffectiveSiteSettings()
                    window.installFocusObserver(webView)
                }
                onActiveFocusChanged: {
                    if (activeFocus && viewUi === browserUi
                            && tabIndex === viewUi.active_tab_index) {
                        window.scheduleFocusProbe(2)
                    }
                }
                onZoomFactorChanged: {
                    if (viewUi === browserUi && browserUi.mode === "grid") {
                        browserUi.spatial_invalidated("geometry-changed")
                    }
                }

                onUrlChanged: {
                    refreshEffectiveSiteSettings()
                    if (!viewModel.loaded) {
                        return
                    }
                    var changedUrl = url.toString()
                    if (viewModel.url !== changedUrl && viewTabs === tabs) {
                        tabs.setProperty(tabIndex, "url", changedUrl)
                    }
                    if (viewTabs === tabs) {
                        viewUi.navigation_url_changed_for(tabIndex, changedUrl)
                    } else {
                        viewUi.navigation_url_changed(changedUrl)
                    }
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        if (viewUi === browserUi) {
                            address.text = browserUi.display_url
                            window.updateMprisForPrimaryView(webView)
                            window.refreshBlockingEvidence()
                        }
                    }
                }
                onTitleChanged: {
                    if (viewModel.loaded && viewTabs === tabs) {
                        tabs.setProperty(tabIndex, "title", title || "New tab")
                        if (tabIndex === viewUi.active_tab_index && viewUi === browserUi) {
                            window.updateMprisForPrimaryView(webView)
                        }
                    }
                }
                onLoadingChanged: function(loadRequest) {
                    if (loadRequest.status === WebEngineView.LoadStartedStatus) {
                        webView.scrollPositionRestored = false
                        window.clearPageDialogForView(webView)
                        window.clearClientCertificateForView(webView)
                        window.clearCertificateErrorForView(webView)
                        window.clearWebAuthForView(webView)
                        window.clearContextMenuForView(webView)
                        window.clearDesktopMediaForView(webView)
                        window.clearSiteDataClearForView(webView)
                        window.noteCaptureNavigation(webView)
                        window.clearFileDialogForView(webView)
                        window.clearRendererFailureForView(webView)
                        window.resetPageDialogBudget(webView)
                        viewInterceptor.clearSiteEvidence(webView.url.host)
                        viewUi.clear_blocking_active_evidence()
                        if (viewTabs === tabs) {
                            window.cancelPermissionForTab(tabIndex)
                        } else {
                            window.cancelPermissionForUi(viewUi)
                        }
                        window.installPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile)
                        window.injectPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile,
                            "document_start")
                        window.injectCosmeticRules(viewUi, webView)
                        if (viewTabs === tabs) {
                            viewUi.navigation_started_for(tabIndex, loadRequest.url.toString())
                        } else {
                            viewUi.navigation_started(loadRequest.url.toString())
                        }
                    } else if (loadRequest.status === WebEngineView.LoadSucceededStatus) {
                        if (viewTabs === tabs) {
                            viewUi.navigation_committed_for(tabIndex, webView.url.toString(), webView.title)
                        } else {
                            viewUi.navigation_committed(webView.url.toString(), webView.title)
                        }
                        if (!webView.scrollPositionRestored) {
                            webView.scrollPositionRestored = true
                            window.restoreScrollPosition(webView, viewModel.scrollX, viewModel.scrollY)
                        }
                        window.injectPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile,
                            "document_end")
                        window.injectCosmeticRules(viewUi, webView)
                        Qt.callLater(function() {
                            window.injectPageUserscripts(
                                viewUi, webView, webView.url.toString(), viewTransientProfile,
                                "document_idle")
                        })
                        if (viewTabs === tabs) {
                            viewUi.navigation_completed_for(tabIndex)
                        } else {
                            viewUi.navigation_completed()
                        }
                        if (viewUi === browserUi
                                && tabIndex === viewUi.active_tab_index) {
                            window.scheduleFocusProbe(3)
                        }
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                            window.finishSiteDoctorAfterLoad(viewUi, true)
                        }
                    } else if (loadRequest.status === WebEngineView.LoadFailedStatus) {
                        if (viewTabs === tabs) {
                            viewUi.navigation_failed_with_details(
                                tabIndex,
                                loadRequest.url.toString(),
                                window.navigationFailureKind(loadRequest),
                                window.navigationFailureDetail(loadRequest))
                        } else {
                            viewUi.navigation_failed()
                        }
                        if (tabIndex === viewUi.active_tab_index) {
                            window.finishSiteDoctorAfterLoad(viewUi, false)
                        }
                    }
                }
                onPdfPrintingFinished: function(filePath, success) {
                    viewUi.finish_print_pdf(filePath, success)
                }
                onFileDialogRequested: function(request) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handleFileDialogRequested(request, webView, viewUi)
                    } else {
                        window.resolveQtRequest(viewUi, request, "file-dialog", "dialogReject", [])
                    }
                }
                onPermissionRequested: function(permissionRequest) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handlePermissionRequested(permissionRequest, tabIndex)
                    } else {
                        window.resolveQtRequest(viewUi, permissionRequest, "permission", "deny", [])
                    }
                }
                onJavaScriptDialogRequested: function(request) {
                    window.handleJavaScriptDialogRequested(
                        viewUi, webView, request, viewTransientProfile)
                }
                onAuthenticationDialogRequested: function(request) {
                    window.handleAuthenticationDialogRequested(
                        viewUi, webView, request, viewTransientProfile)
                }
                onSelectClientCertificate: function(selection) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handleClientCertificateRequested(
                            viewUi, webView, selection, viewTransientProfile, window)
                    } else {
                        window.resolveQtRequest(viewUi, selection, "client-certificate", "selectNone", [])
                    }
                }
                onCertificateError: function(error) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handleCertificateError(
                            viewUi, webView, error, window)
                    } else {
                        window.resolveQtRequest(viewUi, error, "client-certificate", "rejectCertificate", [])
                    }
                }
                onWebAuthUxRequested: function(request) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handleWebAuthRequested(
                            viewUi, webView, request, window)
                    } else {
                        window.resolveQtRequest(viewUi, request, "webauth", "cancel", [])
                    }
                }
                onContextMenuRequested: function(request) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handleContextMenuRequested(
                            viewUi, webView, request, window)
                    } else {
                        request.accepted = true
                    }
                }
                onDesktopMediaRequested: function(request) {
                    window.handleDesktopMediaRequested(
                        viewUi, webView, request, window)
                }
                onFullScreenRequested: function(request) {
                    if (viewUi === browserUi && browserUi.mode === "grid") {
                        browserUi.spatial_invalidated("view-changed")
                    }
                    request.accept()
                    if (request.toggleOn) {
                        window.showFullScreen()
                        viewUi.status_text = "Page fullscreen enabled"
                    } else {
                        window.showNormal()
                        viewUi.status_text = "Page fullscreen ended"
                    }
                }
                onRenderProcessTerminated: function(terminationStatus, exitCode) {
                    window.handleRendererProcessTerminated(
                        viewUi, webView, window, tabIndex,
                        terminationStatus, exitCode)
                }
                Component.onDestruction: {
                    window.clearPageDialogForView(webView)
                    window.clearClientCertificateForView(webView)
                    window.clearCertificateErrorForView(webView)
                    window.clearWebAuthForView(webView)
                    window.clearContextMenuForView(webView)
                    window.clearDesktopMediaForView(webView)
                    window.clearCaptureSessionForView(webView)
                    window.clearFileDialogForView(webView)
                    window.clearRendererFailureForView(webView)
                }
                onNewWindowRequested: function(request) {
                    var popupAllowed = viewTabs === tabs
                            ? viewUi.popup_allowed_for(
                                tabIndex, request.requestedUrl.toString(), request.userInitiated)
                            : viewUi.popup_allowed(
                                request.requestedUrl.toString(), request.userInitiated)
                    if (!popupAllowed) {
                        return
                    }
                    var popup = popupWindowComponent.createObject(null, {
                        popupRequest: request,
                        popupJourneyToken: viewUi.take_popup_journey_token(
                            request.requestedUrl.toString()),
                        popupProfile: viewProfile,
                        popupPermissionUi: viewUi,
                        popupContextName: viewUi.context_name,
                        popupContextLabel: viewUi.context_label,
                        popupPrivateProfile: viewTransientProfile,
                        popupEphemeralProfile: window.ephemeralProfile,
                        popupProfileName: window.profileName
                    })
                    if (!popup) {
                        if (viewTabs === tabs) {
                            viewUi.navigation_failed_for(tabIndex)
                        } else {
                            viewUi.navigation_failed()
                        }
                    }
                }
            }
        }
    }

    FerricPagePointerAdapter {
        id: spatialPointerAdapter
        enabled: browserUi.spatial_visible
        targetItem: browserUi.spatial_visible ? window.activeWebView() : null
        targetWindow: window
        inputBlocked: window.browserChromeInputActive
        onDispatchAcknowledged: function(requestId, sessionId, serial, revision, outcome) {
            browserUi.spatial_dispatch_ack(
                        requestId, sessionId, serial, revision, outcome)
        }
        onInvalidated: function(reason) {
            // Initial target binding can advance the adapter stamp before the
            // first geometry observation. Once a root is installed, every
            // later invalidation is authoritative.
            if (browserUi.spatial_visible && browserUi.spatial_root_width > 0) {
                browserUi.spatial_invalidated(reason)
            }
        }
        onPhysicalPointerDetected: function(reason) {
            if (browserUi.spatial_visible) {
                browserUi.spatial_invalidated(reason)
            }
        }
        onSurfaceChanged: {
            if (browserUi.spatial_visible && browserUi.spatial_root_width > 0) {
                browserUi.spatial_invalidated("geometry-changed")
            }
        }
    }

    Connections {
        target: browserUi
        function onSpatial_visibleChanged() {
            if (!browserUi.spatial_visible) {
                return
            }
            var sessionId = browserUi.spatial_session_id
            Qt.callLater(function() {
                if (!browserUi.spatial_visible
                        || browserUi.spatial_session_id !== sessionId) {
                    return
                }
                var view = window.activeWebView()
                if (view && view.width > 0 && view.height > 0) {
                    browserUi.spatial_surface_ready(
                                view.width, view.height,
                                spatialPointerAdapter.surfaceSerial,
                                spatialPointerAdapter.surfaceRevision)
                } else {
                    browserUi.spatial_invalidated("surface-unavailable")
                }
            })
        }
        function onSpatial_dispatch_requestChanged() {
            var payload = browserUi.spatial_dispatch_request
            if (!payload || payload.length === 0) {
                return
            }
            var request
            try {
                request = JSON.parse(payload)
            } catch (error) {
                browserUi.spatial_invalidated("dispatch-rejected")
                return
            }
            // `enabled` stays declaratively bound to `spatial_visible` so the
            // adapter always releases its session and physical-input filter
            // state when Grid exits. Never assign to it here: doing so would
            // replace the binding after the first dispatch.
            if (!spatialPointerAdapter.enabled) {
                browserUi.spatial_invalidated("dispatch-rejected")
                return
            }
            spatialPointerAdapter.dispatch(
                        request.request_id, request.session_id,
                        request.serial, request.revision,
                        Number(request.x), Number(request.y), request.action)
        }
    }

    FerricSpatialGridOverlay {
        id: spatialGridOverlay
        anchors.fill: webViews
        z: 31
        browserUi: window.browserUi
        chromeScale: window.chromeScale
        labelColor: window.selectionTextColor
        labelBackground: window.selectionColor
        crosshairColor: window.accentColor
    }

    Loader {
        id: attachedDevToolsLoader
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: Math.min(window.devToolsPanelHeight, parent.height * 0.55)
        active: window.devToolsVisible && !window.devToolsDetached
        visible: active
        z: 20
        sourceComponent: Component {
            WebEngineView {
                anchors.fill: parent
                profile: browserProfile
                inspectedView: window.activeWebView()
                Accessible.name: "Attached developer tools"
            }
        }
    }

    Timer {
        interval: 80
        repeat: true
        running: browserUi.hint_visible
        onTriggered: window.pollHintRefresh()
    }

    FerricHintOverlay {
        id: hintOverlaySurface
        anchors.fill: webViews
        browserWindow: window
        hintsVisible: browserUi.hint_visible
        hintResults: window.hintResults
        sourceViewport: window.hintViewport
        hintState: window.hintState
        unmatchedPolicy: browserUi.hint_unmatched_policy
        markerScale: browserUi.hint_marker_scale
        collisionRotation: window.hintCollisionRotation
        onActivationRequested: function(label) { window.activateHint(label) }
        onActionsRequested: function(label) { window.showHintActions(label) }
    }

    FerricRapidHintConfirmation {
        browserWindow: window
        onContinueRequested: {
            if (browserUi.confirm_rapid_hint_tabs()) {
                window.rapidHintConfirmationVisible = false
                Qt.callLater(window.startHintCollection)
            }
        }
        onCancelRequested: {
            window.rapidHintConfirmationVisible = false
            window.closeHints()
        }
    }

    FerricCopyNotice {
        browserWindow: window
        onCopyAgainRequested: window.copyToClipboard(
                                  window.copiedValue, !window.copyNoticeSensitive)
        onDismissalRequested: window.copyNoticeVisible = false
    }

    FerricStatusBar {
        browserWindow: window
        statusVisible: window.statusBarVisible
        mode: browserUi.mode
        displayUrl: browserUi.display_url
        statusText: window.commandNoticeVisible
                    ? window.commandNoticeText : browserUi.status_text
        statusError: window.commandNoticeVisible && window.commandNoticeError
        contextName: browserUi.context_name
        contextColor: window.contextStatusColor(browserUi, window.mutedTextColor)
        profileName: window.profileName
        temporaryProfile: window.temporaryProfile
        ephemeralProfile: window.ephemeralProfile
        blockingSiteCount: browserUi.blocking_active_site_count
        activeTabIndex: browserUi.active_tab_index
        tabCount: browserUi.tab_count
        engineUpdateNotice: window.engineUpdateNotice
        macroStatusText: browserUi.macro_status_text
        activeView: window.activeWebView()
        accessibleDetails: window.statusDetails(
                               browserUi, window, window.activeWebView(),
                               window.temporaryProfile, window.profileName,
                               window.ephemeralProfile)
    }

    function submitInteractiveCommand(text) {
        var focusedContextWindow = window.focusExistingContextWindow(text, browserUi)
        var succeeded = focusedContextWindow
                || browserUi.execute_interactive_command(text)
        if (!succeeded) {
            window.pendingInteractiveCommand = browserUi.command_retryable ? text : ""
            commandSurface.showFeedback(browserUi.status_text,
                                        !browserUi.command_retryable)
            commandSurface.selectAllInput()
            return
        }

        window.pendingInteractiveCommand = ""
        var commandStatus = browserUi.status_text
        var preview = browserUi.take_session_preview()
        if (preview.length > 0) {
            window.showCommandSessionPreview(preview)
        }
        if (browserUi.library_kind.length > 0) {
            window.libraryPage = 0
            window.openInternalSurface()
            window.libraryManagerVisible = true
            window.refreshLibraryManager()
        }
        if (browserUi.link_preview_visible) {
            window.showLinkPreview()
        }
        if (text.trim().indexOf("context-enter ") === 0) {
            window.routeContextWorkspace(browserUi)
        }
        window.syncTabModel()
        window.executePendingEngineAction()
        commandStatus = browserUi.status_text
        commandSurface.clearInput()
        if (browserUi.mode === "command") {
            browserUi.escape()
            browserUi.status_text = commandStatus
        }
        window.showCommandNotice(commandStatus, false)
    }

    FerricCommandLine {
        id: commandSurface
        browserWindow: window
        commandVisible: browserUi.mode === "command"
        completionVisible: browserUi.completion_visible
        completionText: browserUi.completion_text
        completionValues: browserUi.completion_values
        completionStart: browserUi.completion_start
        completionEnd: browserUi.completion_end
        completionSelected: browserUi.completion_selected
        onCompletionUpdateRequested: function(text, cursorPosition) {
            browserUi.update_completion(text, cursorPosition)
        }
        onSubmitted: function(text) { window.submitInteractiveCommand(text) }
        onInputEdited: window.pendingInteractiveCommand = ""
        onEscapeRequested: {
            window.pendingInteractiveCommand = ""
            browserUi.escape()
        }
        onCompletionMoveRequested: function(delta) { browserUi.completion_move(delta) }
        onCompletionSelectRequested: function(index) { browserUi.completion_select(index) }
        onHistoryMoveRequested: function(delta, current) {
            commandSurface.commandText = delta < 0
                ? browserUi.command_history_previous(current)
                : browserUi.command_history_next(current)
            commandSurface.cursorPosition = commandSurface.commandText.length
        }
    }

    FerricSearchBar {
        id: searchSurface
        browserWindow: window
        searchVisible: browserUi.mode === "search"
        searchBackward: browserUi.search_backward
        searchText: browserUi.search_text
        onSearchChanged: function(text) { browserUi.search_changed(text) }
        onAccepted: {
            browserUi.search_next(browserUi.search_backward)
            browserUi.accept_search()
            window.executePendingEngineAction()
        }
        onEscapeRequested: browserUi.escape()
        onNextRequested: function(backward) {
            browserUi.execute_ui_action(
                        "browser.tab.search-next", backward ? "backward" : "forward")
            window.executePendingEngineAction()
        }
    }

}
