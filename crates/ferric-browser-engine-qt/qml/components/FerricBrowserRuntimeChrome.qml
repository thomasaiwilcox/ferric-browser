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

    copyNoticeSurfaceObject: copyNoticeSurface
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
        tooltipsEnabled: window.active && !window.browserChromeInputActive
                         && browserUi.mode !== "hint" && browserUi.mode !== "grid"
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
            if (browserUi.execute_command(":tab-new")) window.executePendingEngineAction()
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
                if (historyView && historyView.contentCanGoBack) {
                    browserUi.back()
                    window.executePendingEngineAction()
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
                if (historyView && historyView.contentCanGoForward) {
                    browserUi.forward()
                    window.executePendingEngineAction()
                } else {
                    browserUi.status_text = "History boundary reached"
                }
            }
        }
        FerricTextField {
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
                window.executePendingEngineAction()
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
                if (browserUi.execute_command(":tab-new")) window.executePendingEngineAction()
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

    Connections {
        target: browserUi
        function onSite_experiment_activeChanged() {
            Qt.callLater(window.refreshVisibleSiteLedger)
        }
    }

    FerricSiteLedger {
        id: siteLedgerSurface
        browserWindow: window
        siteExperimentAvailable: !browserUi.site_experiment_active
        experimentRemainingSeconds: browserUi.site_experiment_remaining_seconds
        onCloseRequested: {
            window.siteLedgerVisible = false
            window.closeInternalSurface()
        }
        onRefreshRequested: window.showSiteLedger()
        onActiveOriginDataClearRequested: {
            if (!window.siteLedgerSnapshotIsCurrent()) {
                siteLedgerSurface.showNotice(browserUi.status_text, true)
                return
            }
            var accepted = browserUi.site_data_clear(window.siteLedgerData.origin, true)
            if (accepted) window.executePendingEngineAction()
            siteLedgerSurface.showNotice(browserUi.status_text, !accepted)
        }
        onSanitizedReportCopyRequested: function(includeHost) {
            if (!window.siteLedgerSnapshotIsCurrent()) {
                siteLedgerSurface.showNotice(browserUi.status_text, true)
                return
            }
            var report = browserUi.site_report(includeHost)
            if (report && report.length > 0) {
                window.copyToClipboard(report, true)
                browserUi.status_text = "Sanitized site report copied"
            }
            siteLedgerSurface.showNotice(browserUi.status_text, !report || report.length === 0)
        }
        onSiteDoctorProposalApplyRequested: function(proposalId) {
            if (!window.siteLedgerSnapshotIsCurrent()) {
                siteLedgerSurface.showNotice(browserUi.status_text, true)
                return
            }
            var accepted = browserUi.apply_site_doctor_proposal(proposalId, true)
            if (accepted) window.showSiteLedger()
            siteLedgerSurface.showNotice(browserUi.status_text, !accepted)
        }
        onSiteDoctorExperimentRequested: function(kind) {
            if (!window.siteLedgerSnapshotIsCurrent()) {
                siteLedgerSurface.showNotice(browserUi.status_text, true)
                return
            }
            var started = browserUi.begin_site_doctor_experiment(kind)
            if (started.length > 0) {
                window.executePendingEngineAction()
            }
            if (window.siteLedgerVisible) window.showSiteLedger()
            siteLedgerSurface.showNotice(browserUi.status_text, started.length === 0)
        }
    }

    profileNoticeHeight: profileNotice.height
    profileNoticeInputActive: profileNotice.inputActive
    FerricProfileNotice {
        id: profileNotice
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: window.bottomControlsHeight
        browserWindow: window
        hostWindow: window
        noticeText: browserUi.profile_startup_notice
        onDetailsRequested: window.showDiagnostics(browserUi, window)
    }

    FerricDiagnostics {
        hostWindow: window.diagnosticsHost || window
        parent: hostWindow && hostWindow.contentItem ? hostWindow.contentItem : null
        startupDetails: window.diagnosticsStartupDetails
        previewText: window.diagnosticsText
        summaryRows: (window.diagnosticsUi ? window.diagnosticsUi.diagnostics_summary : [])
        reportAvailable: (window.diagnosticsUi ? window.diagnosticsUi.diagnostics_available : false)
        reportPending: (window.diagnosticsUi ? window.diagnosticsUi.diagnostics_pending : false)
        reportError: (window.diagnosticsUi ? window.diagnosticsUi.diagnostics_error : "")
        exportNotice: window.diagnosticsExportNotice
        exportDetail: window.diagnosticsExportDetail
        visible: window.diagnosticsVisible
        browserWindow: window.diagnosticsAppearance
        onCloseRequested: window.closeDiagnostics()
        onRefreshRequested: window.refreshDiagnostics()
        onCopyRequested: {
            var ui = window.diagnosticsUi
            if (!ui || !ui.diagnostics_available) return
            if (ui.write_clipboard(window.diagnosticsText, false)) {
                window.diagnosticsExportNotice = "Report preview copied"
                window.diagnosticsExportDetail = ""
            } else {
                window.diagnosticsExportNotice = "The report preview couldn't be copied."
                window.diagnosticsExportDetail = String(ui.status_text)
            }
        }
        onSaveRequested: {
            if (!(window.diagnosticsUi ? window.diagnosticsUi.diagnostics_available : false)) return
            window.diagnosticsExportUi = window.diagnosticsUi
            fileDialogSurfaces.diagnosticsExportCurrentFile = "ferric-browser-diagnostics.json"
            fileDialogSurfaces.openDiagnosticsExport()
        }
    }

    FerricBindingHelp {
        id: keyboardHelpSurface
        browserWindow: window
        onCloseRequested: window.closeBindingHelp()
        onRefreshRequested: window.refreshBindingHelp()
        onSearchChanged: function(text) {
            window.bindingHelpSearch = text
            window.refreshBindingHelp()
        }
    }

    FerricSettingsWorkspace {
        id: settingsWorkspace
        objectName: "primarySettingsWorkspace"
        browserWindow: window
        browserUi: window.browserUi
        state: window
        profileLabel: window.profileLabel
        transientProfile: window.temporaryProfile || window.ephemeralProfile
        spellcheckStatus: window.spellcheckStatusText(browserUi)
        spellcheckDictionariesMissing: window.spellcheckDictionaryStatus(browserUi).missing.length > 0
        onOpening: window.openInternalSurface()
        onClosed: window.closeInternalSurface()
        onInstallManifestRequested: window.chooseUserscriptManifest()
    }

    FerricLibraryManager {
        id: libraryManager
        browserWindow: window
        managerVisible: window.libraryManagerVisible
        listPending: ["actions", "context-routes"].indexOf(browserUi.library_kind) < 0 && browserUi.library_pending
        listError: ["actions", "context-routes"].indexOf(browserUi.library_kind) < 0 ? browserUi.library_error : ""
        feedback: ["bookmarks", "quickmarks"].indexOf(browserUi.library_kind) >= 0
            ? ({id: browserUi.mark_action_id, state: browserUi.mark_action_state,
                message: browserUi.mark_action_message, detail: browserUi.mark_action_detail})
            : ({id: "", state: "idle", message: "", detail: ""})
        onRefreshRequested: {
            if (window.refreshActiveLibrary() === false)
                libraryManager.requestRejected("This view couldn't be refreshed. Try again.", browserUi.status_text)
        }
        onDeleteCancelled: window.pendingLibraryDelete = ""
        onGraphLayoutRequested: if (window.libraryManagerVisible) window.refreshLibraryManager()
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
            if (!window.openLibraryEntry(entryKind, entryId))
                libraryManager.requestRejected("This saved page couldn't be opened. Refresh and try again.", browserUi.status_text)
        }
        onPrivateHistoryTransferRequested: function(entryId, title, url) {
            window.showPrivateHistoryTransfer(entryId, title, url)
        }
        onEntryEditRequested: function(entryKind, entryId, value) {
            if (window.editLibraryEntry(entryKind, entryId, value)) {
                libraryManager.mutationAccepted(browserUi.mark_action_id, entryKind, entryId, String(value), false)
            } else {
                libraryManager.requestRejected("This edit couldn't be saved. Review the value and try again.", browserUi.status_text)
            }
        }
        onEntryDeleteRequested: function(entryKind, entryId) {
            var confirmed = window.pendingLibraryDelete === entryKind + "\t" + entryId
            if (window.deleteLibraryEntry(entryKind, entryId))
                libraryManager.mutationAccepted(browserUi.mark_action_id, entryKind, entryId, "", true)
            else if (confirmed)
                libraryManager.requestRejected("This item couldn't be deleted. Try again.", browserUi.status_text)
        }
        onJourneyReopenRequested: function(nodeId, target) {
            var command = ":journey-reopen " + window.libraryCommandArgument(nodeId)
                    + " --target " + target
            if (browserUi.execute_command(command)) {
                window.syncTabModel()
                window.executePendingEngineAction()
                window.libraryManagerVisible = false
                window.closeInternalSurface()
            } else {
                libraryManager.requestRejected("This page couldn't be reopened. Refresh and try again.", browserUi.status_text)
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

    readonly property alias profileSessionWorkspaceObject: profileSessionWorkspace
    FerricProfileSessionWorkspace {
        id: profileSessionWorkspace
        browserWindow: window
        hostWindow: window
        browserUi: window.primaryBrowserUi
        state: window
        profilesModel: profiles
        namedSessionsModel: namedSessions
        transientProfile: window.temporaryProfile
        onOpening: window.openInternalSurface()
        onClosed: window.closeInternalSurface()
    }

    StackLayout {
        id: webViews
        anchors.fill: parent
        anchors.leftMargin: window.tabStripVisible && window.tabPosition === "left"
                            ? window.sideTabWidth : 0
        anchors.rightMargin: window.tabStripVisible && window.tabPosition === "right"
                             ? window.sideTabWidth : 0
        anchors.bottomMargin: window.bottomChromeHeight
        currentIndex: {
            var selected = window.activeWebView()
            return selected ? selected.StackLayout.index : -1
        }

        Component {
            id: webViewComponent

            WebEngineView {
                id: webView
                readonly property string promptLeaseIdentity: browserUi.new_prompt_view_identity()
                property var promptIdentityUi: null
                property int promptIdentityTabIndex: -1
                function refreshPromptViewIdentity() {
                    if (promptIdentityUi && promptIdentityTabIndex >= 0
                            && promptIdentityUi.unregister_prompt_view_identity) {
                        promptIdentityUi.unregister_prompt_view_identity(
                            promptIdentityTabIndex, promptLeaseIdentity)
                    }
                    promptIdentityUi = viewUi
                    promptIdentityTabIndex = tabIndex
                    if (promptIdentityUi && promptIdentityTabIndex >= 0
                            && promptIdentityUi.register_prompt_view_identity) {
                        promptIdentityUi.register_prompt_view_identity(
                            promptIdentityTabIndex, promptLeaseIdentity)
                    }
                }
                onTabIndexChanged: refreshPromptViewIdentity()
                onViewUiChanged: {
                    refreshPromptViewIdentity()
                    if (resourceTracked) window.profileResourceRegistry.trackView(webView,viewHost,viewUi)
                }
                onViewHostChanged: {
                    if (resourceTracked) window.profileResourceRegistry.trackView(webView,viewHost,viewUi)
                }
                property bool resourceTracked: false
                property alias nativeUrl: nativeContent.nativeUrl
                property alias contentTraversalPending: nativeContent.traversalPending
                readonly property var startPage: nativeContent.startPage
                readonly property int engineHistoryIndex: nativeContent.engineHistoryIndex
                readonly property bool contentCanGoBack: nativeContent.canNavigateBack
                readonly property bool contentCanGoForward: nativeContent.canNavigateForward
                property bool contentInitialized: false
                readonly property string modelContentUrl: viewModel.loaded ? viewModel.url : ""
                function setContentUrl(destination, traversal, engineMoves) { contentInitialized = true; nativeContent.setContentUrl(destination, traversal, engineMoves) }
                function contentEngineOffset(destination, expected, backwards) { return nativeContent.engineOffset(destination, expected, backwards) }
                function synchronizeContentHistory() { nativeContent.synchronizeHistory() }
                onModelContentUrlChanged: {
                    if (modelContentUrl.length > 0 && (modelContentUrl === "rb://start" || modelContentUrl === "rb://blank" || nativeUrl.length > 0 || !contentInitialized)) setContentUrl(modelContentUrl, false)
                }
                FerricNativeTabContent {
                    id: nativeContent
                    anchors.fill: parent
                    webView: webView
                    browserUi: webView.viewUi
                    browserWindow: window
                    browserHost: webView.viewHost
                    tabsModel: webView.viewTabs
                }
                property int tabIndex: -1
                property string stableTabId: ""
                property var viewUi: browserUi
        property var selectionSource: null
                property var viewHost: window
                property var viewTabs: tabs
                property var viewModel: ({})
                property var viewProfile: browserProfile
                property var viewInterceptor: window.primaryRequestInterceptor
                property bool viewTransientProfile: window.temporaryProfile
                property bool viewTransferred: false
                property bool viewRetired: false
                function ownsTab() {
                    return !viewRetired && !!viewUi && tabIndex >= 0 && stableTabId.length > 0
                        && viewUi.tab_id_for_index(tabIndex) === stableTabId
                }
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
                devToolsView: viewHost && viewHost !== window
                        ? (viewHost.devToolsVisible ? viewHost.devToolsAttachedView : null)
                        : window.devToolsVisible
                          ? (window.devToolsDetached ? window.devToolsExternalView : attachedDevToolsLoader.item)
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
                url: "about:blank"
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
                    resourceTracked=window.profileResourceRegistry.trackView(webView,viewHost,viewUi)
                    if (!resourceTracked) {
                        viewRetired=true
                        viewUi.status_text="This page couldn't be initialized. Try reopening the window."
                        return
                    }
                    refreshPromptViewIdentity()
                    if (viewModel.loaded && !contentInitialized) setContentUrl(viewModel.url || "about:blank", false)
                    refreshEffectiveSiteSettings()
                    if (nativeUrl.length > 0) {
                        viewUi.content_navigated(tabIndex, nativeUrl, engineHistoryIndex, false)
                        if (nativeUrl === "rb://start") Qt.callLater(nativeContent.focusContent)
                    } else if (viewModel.loaded) window.installFocusObserver(webView)
                }
                onJavaScriptConsoleMessage: function(level, message, lineNumber, sourceID) {
                    window.probeFocusNotification(viewUi, viewHost, webView, tabIndex, message)
                }
                onActiveFocusChanged: {
                    if (activeFocus && nativeUrl === "rb://start") { Qt.callLater(nativeContent.focusContent); return }
                    if (activeFocus && viewUi === browserUi
                            && tabIndex === viewUi.active_tab_index) {
                        window.scheduleFocusProbe(2)
                    }
                }
                onRecentlyAudibleChanged: {
                    if (ownsTab() && viewUi === browserUi && tabIndex === viewUi.active_tab_index)
                        window.updateMprisForPrimaryView(webView)
                }
                onAudioMutedChanged: {
                    if (ownsTab() && viewUi === browserUi && tabIndex === viewUi.active_tab_index)
                        window.updateMprisForPrimaryView(webView)
                }
                onZoomFactorChanged: {
                    if (viewUi === browserUi && browserUi.mode === "grid") {
                        browserUi.spatial_invalidated("geometry-changed")
                    }
                }

                onNavigationRequested: function(request) {
                    if (request.isMainFrame) nativeContent.beginNavigation(request.navigationType === WebEngineView.BackForwardNavigation)
                }
                onUrlChanged: {
                    if (nativeUrl.length > 0) return
                    if (!ownsTab()) return
                    nativeContent.noteSameDocument(url.toString())
                    refreshEffectiveSiteSettings()
                    if (!viewModel.loaded) {
                        return
                    }
                    var changedUrl = url.toString()
                    if (viewModel.url !== changedUrl && viewTabs && tabIndex >= 0) {
                        viewTabs.setProperty(tabIndex, "url", changedUrl)
                    }
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        if (viewUi === browserUi) {
                            address.text = browserUi.display_url
                            window.updateMprisForPrimaryView(webView)
                            window.refreshBlockingEvidence()
                        }
                    }
                }
                onTitleChanged: {
                    if (nativeUrl.length > 0) return
                    if (!ownsTab()) return
                    if (viewModel.loaded && viewTabs && tabIndex >= 0) {
                        viewTabs.setProperty(tabIndex, "title", title || "New tab")
                        if (tabIndex === viewUi.active_tab_index && viewUi === browserUi) {
                            window.updateMprisForPrimaryView(webView)
                        }
                    }
                }
                onLoadingChanged: function(loadRequest) {
                    if (!viewModel.loaded || nativeUrl.length > 0 || !ownsTab()) return
                    if (loadRequest.status === WebEngineView.LoadStartedStatus) {
                        viewUi.invalidate_native_prompt_leases()
                        webView.scrollPositionRestored = false
                        window.clearPageDialogForView(webView)
                        window.clearClientCertificateForView(webView)
                        window.clearCertificateErrorForView(webView)
                        window.clearWebAuthForView(webView)
                        window.clearContextMenuForView(webView)
                        window.clearDesktopMediaForView(webView)
                        window.clearSiteDataClearForView(webView)
                        window.noteCaptureNavigation(webView)
                        if (viewHost && viewHost.clearFileDialogForView) viewHost.clearFileDialogForView(webView)
                        window.clearRendererFailureForView(webView)
                        window.resetPageDialogBudget(webView)
                        viewInterceptor.clearSiteEvidence(webView.url.host)
                        viewUi.clear_blocking_active_evidence()
                        if (viewTabs && tabIndex >= 0) {
                            window.removePermissionGroups(function(group) {
                                return group.ui === webView.viewUi && group.tabIndex === webView.tabIndex
                            }, true)
                        } else {
                            window.cancelPermissionForUi(viewUi)
                        }
                        window.installPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile)
                        window.injectPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile,
                            "document_start")
                        window.injectCosmeticRules(viewUi, webView)
                        if (viewTabs && tabIndex >= 0) {
                            viewUi.navigation_started_for(tabIndex, loadRequest.url.toString())
                        } else {
                            viewUi.navigation_started(loadRequest.url.toString())
                        }
                    } else if (loadRequest.status === WebEngineView.LoadSucceededStatus) {
                        nativeContent.noteCommit()
                        if (viewTabs && tabIndex >= 0) {
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
                        if (viewTabs && tabIndex >= 0) {
                            viewUi.navigation_completed_for(tabIndex)
                        } else {
                            viewUi.navigation_completed()
                        }
                        if (viewUi === browserUi
                                && tabIndex === viewUi.active_tab_index) {
                            window.scheduleFocusProbe(3)
                        }
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                            window.finishSiteDoctorAfterLoad(viewUi, true)
                        }
                    } else if (loadRequest.status === WebEngineView.LoadFailedStatus) {
                        nativeContent.noteCommit()
                        if (viewTabs && tabIndex >= 0) {
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
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        window.handleFileDialogRequested(request, webView, viewUi)
                    } else {
                        window.resolveQtRequest(viewUi, request, "file-dialog", "dialogReject", [])
                    }
                }
                onPermissionRequested: function(permissionRequest) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        window.handlePermissionRequested(permissionRequest, tabIndex,
                            viewUi, webView, viewTransientProfile, viewHost)
                    } else {
                        window.resolveQtRequest(viewUi, permissionRequest, "permission", "deny", [])
                    }
                }
                onJavaScriptDialogRequested: function(request) {
                    window.handleJavaScriptDialogRequested(
                        viewUi, webView, request, viewTransientProfile, viewHost)
                }
                onAuthenticationDialogRequested: function(request) {
                    window.handleAuthenticationDialogRequested(
                        viewUi, webView, request, viewTransientProfile, viewHost)
                }
                onSelectClientCertificate: function(selection) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        window.handleClientCertificateRequested(
                            viewUi, webView, selection, viewTransientProfile, viewHost)
                    } else {
                        window.resolveQtRequest(viewUi, selection, "client-certificate", "selectNone", [])
                    }
                }
                onCertificateError: function(error) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        window.handleCertificateError(
                            viewUi, webView, error, viewHost)
                    } else {
                        window.resolveQtRequest(viewUi, error, "client-certificate", "rejectCertificate", [])
                    }
                }
                onWebAuthUxRequested: function(request) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        window.handleWebAuthRequested(
                            viewUi, webView, request, viewHost)
                    } else {
                        window.resolveQtRequest(viewUi, request, "webauth", "cancel", [])
                    }
                }
                onContextMenuRequested: function(request) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        window.handleContextMenuRequested(
                            viewUi, webView, request, viewHost)
                    } else {
                        request.accepted = true
                    }
                }
                onDesktopMediaRequested: function(request) {
                    window.handleDesktopMediaRequested(
                        viewUi, webView, request, viewHost)
                }
                onFullScreenRequested: function(request) {
                    if (viewUi === browserUi && browserUi.mode === "grid") {
                        browserUi.spatial_invalidated("view-changed")
                    }
                    request.accept()
                    if (request.toggleOn) {
                        viewHost.showFullScreen()
                        viewUi.status_text = "Page fullscreen enabled"
                    } else {
                        viewHost.showNormal()
                        viewUi.status_text = "Page fullscreen ended"
                    }
                }
                onRenderProcessTerminated: function(terminationStatus, exitCode) {
                    if (nativeUrl.length > 0) return
                    window.handleRendererProcessTerminated(
                        viewUi, webView, viewHost, tabIndex,
                        terminationStatus, exitCode)
                }
                Component.onDestruction: {
                    if (!(viewHost && viewHost.runtimeReleased) && !window.runtimeDisposing) {
                        window.cancelScriptsForView(webView)
                        window.clearPageDialogForView(webView)
                        window.clearClientCertificateForView(webView)
                        window.clearCertificateErrorForView(webView)
                        window.clearWebAuthForView(webView)
                        window.clearContextMenuForView(webView)
                        window.clearDesktopMediaForView(webView)
                        window.clearCaptureSessionForView(webView)
                        if (viewHost && viewHost.clearFileDialogForView) viewHost.clearFileDialogForView(webView)
                        window.clearRendererFailureForView(webView)
                    }
                    if (promptIdentityUi && promptIdentityTabIndex >= 0
                            && promptIdentityUi.unregister_prompt_view_identity) {
                        promptIdentityUi.unregister_prompt_view_identity(
                            promptIdentityTabIndex, promptLeaseIdentity)
                    }
                }
                onNewWindowRequested: function(request) {
                    var popupAllowed = viewTabs && tabIndex >= 0
                            ? viewUi.popup_allowed_for(
                                tabIndex, request.requestedUrl.toString(), request.userInitiated)
                            : viewUi.popup_allowed(
                                request.requestedUrl.toString(), request.userInitiated)
                    if (!popupAllowed) {
                        return
                    }
                    var popupToken=viewUi.take_popup_journey_token(request.requestedUrl.toString())
                    var popup = popupWindowComponent.createObject(null, {
                        popupRequest: request,
                        popupJourneyToken: popupToken,
                        popupProfile: viewProfile,
                        popupOpenerUi: viewUi,
                        popupContextName: viewUi.context_name,
                        popupContextLabel: viewUi.context_label,
                        popupPrivateProfile: viewTransientProfile,
                        popupEphemeralProfile: viewHost === window ? window.ephemeralProfile : viewHost.windowEphemeralProfile,
                        popupEphemeralInvocationToken: viewHost === window ? window.ephemeralInvocationToken : viewHost.windowEphemeralInvocationToken,
                        popupProfileName: viewHost === window ? window.profileName : viewHost.windowProfileName,
                        popupStorageBasePath: viewHost === window ? window.storageBasePath : viewHost.windowStorageBasePath
                    })
                    if (!popup || !popup.profileBootstrapComplete) {
                        viewUi.close_popup_tab(popupToken)
                        viewUi.status_text="The popup could not be opened. Try opening the link again."
                    }
                }
            }
        }
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
                id: primaryInspector
                property var viewHost: window
                property var viewUi: browserUi
                anchors.fill: parent
                profile: browserProfile
                inspectedView: window.activeWebView()
                Accessible.name: "Attached developer tools"
                Component.onCompleted: window.profileResourceRegistry.trackView(primaryInspector,viewHost,viewUi)
            }
        }
    }

    FerricCopyNotice {
        id: copyNoticeSurface
        parent: window.copyNoticeView || (window.copyNoticeHost || window).contentItem
        browserWindow: window.copyNoticeAppearance
        hostWindow: window.copyNoticeHost || window
        noticeVisible: window.copyNoticeVisible && window.copyNoticeOwnerCurrent
        copiedText: window.copiedText
        sensitive: window.copyNoticeSensitive
        primarySelection: window.copyNoticePrimary
        onCopyAgainRequested: window.repeatClipboardCopy()
        onDismissalRequested: window.dismissCopyNotice()
    }

    FerricStatusBar {
        browserWindow: window
        statusVisible: window.statusBarVisible
        mode: browserUi.mode
        displayUrl: browserUi.display_url
        hintPreviewText: browserUi.mode === "hint" && !browserUi.target_search_active ? window.hintWorkspace.previewText : ""
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
        window.profileSessionWorkspaceObject.presentCommandRequest()
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
