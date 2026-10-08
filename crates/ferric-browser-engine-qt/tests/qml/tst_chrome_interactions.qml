import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
import "../../qml/scripts/ChromePresentation.js" as ChromePresentation
import "../../qml/scripts/KeyboardPresentation.js" as KeyboardPresentation
import "../../qml/scripts/VersionedRestorePresentation.js" as VersionedRestorePresentation
import "../../qml/scripts/NativePageCaptureCallbacks.js" as NativePageCaptureCallbacks

TestCase {
    id: chromeTest
    name: "ChromeInteractions"
    when: windowShown
    width: 900
    height: 700

    ApplicationWindow {
        id: testWindow
        width: 900
        height: 700
        visible: true
    }

    property int selectedTabs: 0
    property int closedTabs: 0
    property int activatedResults: 0
    property int invokedActions: 0
    property int activatedContextItems: 0
    property string activatedContextAction: ""
    property int dismissedContextMenus: 0
    property int keptShutdowns: 0
    property int proceededShutdowns: 0
    property int recoveredSessions: 0
    property int dismissedRecoveries: 0
    property int acceptedCommandDialogs: 0
    property int cancelledCommandDialogs: 0
    property int acceptedStashDisclosures: 0
    property int declinedStashDisclosures: 0
    property int acceptedPageCaptureDisclosures: 0
    property int declinedPageCaptureDisclosures: 0
    property string lastStashDisclosureOperation: ""
    property int closedLibraries: 0
    property int openedLibraryEntries: 0
    property string openedLibraryEntryId: ""
    property int leakedLibraryKeys: 0
    property string appliedSettingValue: ""
    property string submittedCommand: ""
    property alias settingsModelObject: settingsModel
    property alias tabsModelObject: tabsModel

    QtObject {
        id: theme
        property real width: testWindow.width
        property real height: testWindow.height
        property real chromeScale: 1
        property int chromeRowHeight: 24
        property int tabBarHeight: 32
        property int inputBarHeight: 32
        property int statusBarHeight: 32
        property real chromeOpacity: 1.0
        property font font: Qt.font({ family: "monospace", pointSize: 10 })
        property color backgroundColor: "#1e1e2e"
        property color surfaceColor: "#313244"
        property color panelColor: "#181825"
        property color primaryTextColor: "#cdd6f4"
        property color mutedTextColor: "#a6adc8"
        property color selectionColor: "#45475a"
        property color selectionTextColor: "#cdd6f4"
        property color borderColor: "#585b70"
        property color accentColor: "#89b4fa"
        property color secondaryTextColor: "#cdd6f4"
        property color errorColor: "#f38ba8"
        property color warningColor: "#f9e2af"
        property color successColor: "#a6e3a1"
        property string switcherScope: "all"
        property var contextMenuItems: []
        property bool temporaryProfile: false
        property bool bindingHelpVisible: true
        property bool recoveryAvailable: true
        property bool journeyExportPreviewVisible: false
        property string journeyExportPreviewText: ""
        property bool privateHistoryTransferVisible: false
        property string bindingHelpSearch: ""
        property var bindingHelpRows: [
            { kind: "binding", mode: "normal", command: ":open",
              keys: "o", description: "Open a URL or search query",
              source: "default", count: "1" }
        ]
        function readableTextColor(candidate, background) {
            return ChromePresentation.readableTextColor(candidate, background)
        }
        function contrastText(background) {
            return ChromePresentation.contrastText(background)
        }
        function addressPresentation(url, maximumCharacters) {
            return String(url).slice(0, maximumCharacters)
        }
    }

    ListModel { id: settingsModel }
    ListModel { id: tabsModel }

    Component {
        id: settingsFixture
        FerricSettingRows {
            width: 600
            height: 250
            browserWindow: theme
            settingsModel: chromeTest.settingsModelObject
            onApplyRequested: function(row, value) {
                chromeTest.appliedSettingValue = String(value)
            }
        }
    }

    Component {
        id: switcherFixture
        FerricSwitcherResults {
            width: 600
            height: 250
            browserWindow: theme
            results: [{ kind: "tab", label: "Example", profile: "default",
                        secondary: "https://example.org", actions: ["close"] }]
            onActivationRequested: chromeTest.activatedResults += 1
            onActionRequested: chromeTest.invokedActions += 1
        }
    }

    Component {
        id: tabFixture
        FerricTabStrip {
            width: 500
            height: 32
            browserWindow: theme
            tabsModel: chromeTest.tabsModelObject
            activeTabIndex: 0
            onSelectRequested: chromeTest.selectedTabs += 1
            onCloseRequested: chromeTest.closedTabs += 1
        }
    }

    Component {
        id: completionFixture
        Item {
            width: 600
            height: 300
            Rectangle { anchors.fill: parent; color: "#ffffff" }
            FerricCommandLine {
                browserWindow: theme
                commandVisible: true
                completionVisible: true
                completionText: "open\nopen-current"
                completionValues: "open\nopen-current"
                completionStart: 0
                completionEnd: 0
                completionSelected: 0
                onSubmitted: function(text) { chromeTest.submittedCommand = text }
            }
        }
    }

    Component {
        id: statusBarFixture
        FerricStatusBar {
            width: 600
            browserWindow: theme
            statusVisible: false
            mode: "normal"
            displayUrl: "https://example.test"
            statusText: "Startup status"
            contextName: ""
            contextColor: theme.primaryTextColor
            profileName: "default"
            temporaryProfile: false
            ephemeralProfile: false
            blockingSiteCount: 0
            activeTabIndex: 0
            tabCount: 1
            engineUpdateNotice: ""
            macroStatusText: ""
            activeView: null
            accessibleDetails: ""
        }
    }

    Component {
        id: contextMenuFixture
        FerricContextMenu {
            browserWindow: theme
            onItemActivated: function(item) {
                chromeTest.activatedContextItems += 1
                chromeTest.activatedContextAction = item.action
            }
            onDismissed: chromeTest.dismissedContextMenus += 1
        }
    }

    Component {
        id: libraryFixture
        Item {
            width: testWindow.width
            height: testWindow.height
            Keys.onPressed: function(event) {
                chromeTest.leakedLibraryKeys += 1
            }

            ListModel {
                id: fixtureHistoryEntries
                ListElement {
                    entryKind: "history"
                    entryId: "first"
                    label: "First page"
                    secondary: "https://first.test"
                    nodeId: ""
                }
                ListElement {
                    entryKind: "history"
                    entryId: "second"
                    label: "Second page"
                    secondary: "https://second.test"
                    nodeId: ""
                }
            }

            FerricLibraryManager {
                browserWindow: theme
                managerVisible: true
                libraryKind: "history"
                graphMode: false
                totalEntries: 2
                page: 0
                pageCount: 1
                pageSize: 100
                journeySearchText: ""
                journeyCurrentOnly: false
                pendingDelete: ""
                entries: fixtureHistoryEntries
                graphNodes: []
                graphEntries: []
                graphLineData: []
                profilesModel: []
                profilesLoading: false
                onCloseRequested: chromeTest.closedLibraries += 1
                onEntryOpenRequested: function(entryKind, entryId) {
                    chromeTest.openedLibraryEntries += 1
                    chromeTest.openedLibraryEntryId = entryId
                }
            }
        }
    }

    Component {
        id: focusOverlayFixture
        Item {
            width: testWindow.width
            height: testWindow.height
            property alias controller: focusController
            property alias pageItem: pageFocusTarget
            property alias commandItem: commandFocusTarget
            property alias overlayItem: overlayFocusTarget

            Item {
                id: pageFocusTarget
                visible: true
            }
            Item {
                id: commandFocusTarget
                visible: true
            }
            Item {
                id: overlayFocusTarget
                visible: true
            }
            FerricFocusOverlayController {
                id: focusController
                browserWindow: testWindow
            }
        }
    }

    Component {
        id: shutdownDecisionFixture
        FerricShutdownDecisionDialog {
            hostWindow: theme
            promptVisible: true
            title: "Close browser window?"
            message: "The browser is waiting for a keyboard decision."
            keepLabel: "Keep browser open"
            proceedLabel: "Close browser"
            onKeepRequested: chromeTest.keptShutdowns += 1
            onProceedRequested: chromeTest.proceededShutdowns += 1
        }
    }

    Component {
        id: recoveryBannerFixture
        FerricRecoveryBanner {
            browserWindow: theme
            onRecoveryRequested: chromeTest.recoveredSessions += 1
            onDismissalRequested: chromeTest.dismissedRecoveries += 1
        }
    }

    Component {
        id: commandDialogFixture
        FerricCommandDialog {
            browserWindow: theme
            visible: true
            commandText: ":test-dialog"
            title: "Keyboard contract"
            message: "The safe choice is selected first."
            actions: [
                { id: "cancel", key: "n", label: "Cancel", safe: true,
                  shortcuts: ["N"] },
                { id: "accept", key: "y", label: "Accept",
                  shortcuts: ["Y"] }
            ]
            onActionRequested: function(action) {
                if (action === "accept") {
                    chromeTest.acceptedCommandDialogs += 1
                } else {
                    chromeTest.cancelledCommandDialogs += 1
                }
            }
        }
    }

    Component {
        id: versioningConsentFixture
        FerricVersioningConsentDialog {
            browserWindow: theme
            operationId: "native-operation-1"
            disclosureText: "This live stash is volatile. Nothing is saved to disk."
            onAcceptRequested: function(operationId) {
                chromeTest.acceptedStashDisclosures += 1
                chromeTest.lastStashDisclosureOperation = operationId
            }
            onDeclineRequested: function(operationId) {
                chromeTest.declinedStashDisclosures += 1
                chromeTest.lastStashDisclosureOperation = operationId
            }
        }
    }

    Component {
        id: pageCaptureConsentFixture
        FerricVersioningConsentDialog {
            browserWindow: theme
            operationId: "native-page-operation-1"
            disclosureText: "This page archive may contain personal data and secrets."
            pageCaptureMode: true
            acceptEnabled: true
            onAcceptRequested: function(operationId) {
                chromeTest.acceptedPageCaptureDisclosures += 1
                chromeTest.lastStashDisclosureOperation = operationId
            }
            onDeclineRequested: function(operationId) {
                chromeTest.declinedPageCaptureDisclosures += 1
                chromeTest.lastStashDisclosureOperation = operationId
            }
        }
    }

    Component {
        id: largeFontHelpFixture
        Pane {
            width: 480
            height: 650
            font.pointSize: 40
            FerricBindingHelp {
                id: help
                browserWindow: theme
            }
        }
    }

    Component {
        id: largeFontInventoryFixture
        Pane {
            width: 480
            height: 400
            font.pointSize: 40
            FerricUserscriptInventory {
                id: inventory
                width: parent.width
                browserWindow: theme
                rows: [{ name: "Example script", enabled: true,
                         actions: 2, page_world: false }]
            }
        }
    }

    Component {
        id: largeFontSettingsFixture
        Pane {
            width: 480
            height: 650
            font.pointSize: 40
            FerricSettings {
                browserWindow: theme
                settingsVisible: true
                temporary: false
                searchText: ""
                notice: ""
                spellcheckStatus: "Available"
                spellcheckDictionariesMissing: false
                userscriptRows: [{ name: "Example script", enabled: true,
                                   actions: 2, page_world: false }]
                settingsModel: chromeTest.settingsModelObject
            }
        }
    }

    Component {
        id: spatialGridFixture
        Item {
            width: 300
            height: 240
            property alias gridItem: spatialGrid
            property alias gridModel: spatialGridModel
            QtObject {
                id: spatialGridModel
                property bool spatial_visible: true
                property bool spatial_selecting: true
                property real spatial_root_width: 300
                property real spatial_root_height: 240
                property real spatial_current_x: 0
                property real spatial_current_y: 0
                property real spatial_current_width: 300
                property real spatial_current_height: 240
                property real spatial_crosshair_x: 150
                property real spatial_crosshair_y: 120
                property int spatial_depth: 0
                property bool spatial_help_visible: false
                property var spatial_labels: ["1", "2", "3", "4", "5",
                                              "6", "7", "8", "9"]
            }
            FerricSpatialGridOverlay {
                id: spatialGrid
                anchors.fill: parent
                browserUi: spatialGridModel
            }
        }
    }

    function init() {
        settingsModel.clear()
        settingsModel.append({ key: "ui.reduced_motion", label: "Reduced motion",
                               type: "enum", scope: "global", apply: "live",
                               value: "on", options: [{ value: "system" },
                                                      { value: "on" },
                                                      { value: "off" }] })
        tabsModel.clear()
        for (var index = 0; index < 25; ++index) {
            tabsModel.append({ title: "Tab " + index, pinned: false, muted: false })
        }
        theme.switcherScope = "all"
        theme.contextMenuItems = []
        selectedTabs = 0
        closedTabs = 0
        activatedResults = 0
        invokedActions = 0
        activatedContextItems = 0
        activatedContextAction = ""
        dismissedContextMenus = 0
        keptShutdowns = 0
        proceededShutdowns = 0
        recoveredSessions = 0
        dismissedRecoveries = 0
        acceptedCommandDialogs = 0
        cancelledCommandDialogs = 0
        closedLibraries = 0
        openedLibraryEntries = 0
        openedLibraryEntryId = ""
        leakedLibraryKeys = 0
        appliedSettingValue = ""
        submittedCommand = ""
    }

    function test_enum_options_reach_the_editor() {
        var rows = createTemporaryObject(settingsFixture, testWindow.contentItem)
        verify(rows)
        var settingsList = findChild(rows, "settingsList")
        verify(settingsList)
        tryCompare(settingsList, "count", 1)
        var settingRow = settingsList.itemAtIndex(0)
        verify(settingRow)
        var combo = findChild(settingRow, "settingEnumEditor")
        verify(combo)
        compare(combo.count, 3)
        compare(combo.currentText, "on")
        mouseClick(combo)
        tryCompare(combo.popup, "visible", true)
        var option = combo.popup.contentItem.itemAtIndex(2)
        verify(option)
        mouseClick(option)
        compare(appliedSettingValue, "off")
    }

    function test_switcher_actions_do_not_activate_the_result() {
        var results = createTemporaryObject(switcherFixture, testWindow.contentItem)
        verify(results)
        wait(50)
        var button = findChild(results, "switcherActionButton")
        verify(button)
        verify(button.width > 0 && button.height > 0)
        mouseClick(button)
        compare(invokedActions, 1)
        compare(activatedResults, 0)

        var list = findChild(results, "switcherResultList")
        verify(list)
        var row = list.itemAtIndex(0)
        verify(row)
        mouseClick(row, 4, 4)
        compare(activatedResults, 1)
    }

    function test_active_tab_is_revealed_in_both_orientations() {
        var strip = createTemporaryObject(tabFixture, testWindow.contentItem)
        verify(strip)
        var list = findChild(strip, "tabList")
        verify(list)
        wait(50)
        strip.activeTabIndex = 24
        tryVerify(function() { return list.contentX > 0 })
        tryVerify(function() { return list.itemAtIndex(24) !== null })
        var active = list.itemAtIndex(24)
        verify(active.x >= list.contentX)
        verify(active.x + active.width <= list.contentX + list.width)
        var newButton = findChild(strip, "newTabButton")
        verify(newButton)
        verify(newButton.x + newButton.width <= strip.width)

        strip.vertical = true
        strip.width = 220
        strip.height = 240
        strip.activeTabIndex = 0
        strip.activeTabIndex = 24
        tryVerify(function() { return list.contentY > 0 })
        tryVerify(function() { return list.itemAtIndex(24) !== null })
        active = list.itemAtIndex(24)
        verify(active.y >= list.contentY)
        verify(active.y + active.height <= list.contentY + list.height)
        verify(newButton.y + newButton.height <= strip.height)
    }

    function test_few_tabs_use_available_strip_width() {
        tabsModel.clear()
        for (var index = 0; index < 4; ++index) {
            tabsModel.append({ title: "A readable tab title " + index,
                               pinned: false, muted: false })
        }
        var strip = createTemporaryObject(tabFixture, testWindow.contentItem)
        verify(strip)
        strip.width = 1200
        var list = findChild(strip, "tabList")
        verify(list)
        tryVerify(function() { return list.itemAtIndex(0) !== null })
        verify(list.itemAtIndex(0).width >= list.width / 4 - 1)
    }

    function test_tab_strip_updates_after_model_insert() {
        tabsModel.clear()
        tabsModel.append({ title: "First", pinned: false, muted: false })
        var strip = createTemporaryObject(tabFixture, testWindow.contentItem)
        verify(strip)
        var list = findChild(strip, "tabList")
        verify(list)
        tryCompare(list, "count", 1)
        tabsModel.append({ title: "Second", pinned: false, muted: false })
        tryCompare(list, "count", 2)
        tryVerify(function() { return list.itemAtIndex(1) !== null })
        compare(strip.Accessible.description, "2 browser tabs")
    }

    function test_tab_strip_keyboard_and_accessible_activation() {
        tabsModel.clear()
        tabsModel.append({ title: "First", pinned: false, muted: false })
        tabsModel.append({ title: "Second", pinned: false, muted: false })
        var strip = createTemporaryObject(tabFixture, testWindow.contentItem)
        verify(strip)
        var list = findChild(strip, "tabList")
        verify(list)
        tryVerify(function() { return list.itemAtIndex(1) !== null })
        var second = list.itemAtIndex(1)
        second.forceActiveFocus()
        tryCompare(second, "activeFocus", true)
        keyClick(Qt.Key_Return)
        compare(selectedTabs, 1)
        keyClick(Qt.Key_Space)
        compare(selectedTabs, 2)
        second.Accessible.pressAction()
        compare(selectedTabs, 3)
    }

    function test_mouse_wheel_on_tab_strip_requests_switch() {
        var strip = createTemporaryObject(tabFixture, testWindow.contentItem)
        verify(strip)
        var list = findChild(strip, "tabList")
        verify(list)
        mouseWheel(list, 20, 15, 0, -120)
        compare(selectedTabs, 1)
    }

    function test_completion_selection_replaces_only_the_active_token() {
        var surface = createTemporaryObject(completionFixture, testWindow.contentItem)
        verify(surface)
        var command = findChild(surface, "commandSurface")
        verify(command)
        command.commandText = "open ex --target tab"
        command.completionValues = "https://example.test"
        command.completionStart = 5
        command.completionEnd = 7
        command.completionSelected = -1
        compare(command.commandWithCompletion(-1), "open ex --target tab")
        compare(command.commandWithCompletion(0),
                "open https://example.test --target tab")
        command.applyCompletion(0)
        compare(command.commandText, "open https://example.test --target tab")
        command.commandText = "open ex --target tab"
        command.completionSelected = 0
        command.focusInput()
        var input = findChild(command, "commandLineInput")
        verify(input)
        input.accepted()
        compare(submittedCommand, "open https://example.test --target tab")
    }

    function test_command_feedback_is_visible_and_editing_clears_it() {
        var surface = createTemporaryObject(completionFixture,
                                            testWindow.contentItem)
        verify(surface)
        var command = findChild(surface, "commandSurface")
        var input = findChild(command, "commandLineInput")
        var completion = findChild(command, "commandCompletionPopup")
        var feedback = findChild(command, "commandFeedbackPopup")
        verify(command)
        verify(input)
        verify(completion)
        verify(feedback)

        command.showFeedback("Unknown command: nope", true)
        compare(command.feedbackText, "Unknown command: nope")
        compare(command.feedbackError, true)
        compare(feedback.visible, true)
        compare(completion.visible, false)

        command.commandText += "x"
        compare(command.feedbackText, "")
        compare(feedback.visible, false)
    }

    function test_library_escape_works_when_a_child_control_has_focus() {
        var manager = createTemporaryObject(libraryFixture,
                                            testWindow.contentItem)
        verify(manager)
        var closeButton = findChild(manager, "libraryCloseButton")
        verify(closeButton)
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        closeButton.forceActiveFocus()
        tryCompare(closeButton, "activeFocus", true)
        keyClick(Qt.Key_Escape)
        compare(closedLibraries, 1)
    }

    function test_history_list_owns_navigation_and_activates_selection() {
        var manager = createTemporaryObject(libraryFixture,
                                            testWindow.contentItem)
        verify(manager)
        var list = findChild(manager, "libraryEntryList")
        verify(list)
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        tryCompare(list, "activeFocus", true)
        tryCompare(list, "currentIndex", 0)

        keyClick(Qt.Key_Down)
        compare(list.currentIndex, 1)
        compare(leakedLibraryKeys, 0)

        keyClick(Qt.Key_Up)
        compare(list.currentIndex, 0)
        compare(leakedLibraryKeys, 0)

        keyClick(Qt.Key_J)
        compare(list.currentIndex, 1)
        compare(leakedLibraryKeys, 0)

        keyClick(Qt.Key_Return)
        compare(openedLibraryEntries, 1)
        compare(openedLibraryEntryId, "second")
        compare(leakedLibraryKeys, 0)
    }

    function test_overlay_focus_wins_deferred_command_mode_restore() {
        var fixture = createTemporaryObject(focusOverlayFixture,
                                            testWindow.contentItem)
        verify(fixture)
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)

        fixture.pageItem.forceActiveFocus()
        tryCompare(fixture.pageItem, "activeFocus", true)
        fixture.controller.captureModeFocus()

        fixture.commandItem.forceActiveFocus()
        tryCompare(fixture.commandItem, "activeFocus", true)
        fixture.controller.captureOverlayFocus(testWindow, fixture.pageItem)
        fixture.overlayItem.forceActiveFocus()
        tryCompare(fixture.overlayItem, "activeFocus", true)
        fixture.commandItem.visible = false

        fixture.controller.restoreModeFocus()
        wait(0)
        compare(fixture.overlayItem.activeFocus, true)

        fixture.controller.restoreOverlayFocus()
        tryCompare(fixture.pageItem, "activeFocus", true)
    }

    function test_close_button_retains_hover_and_does_not_select() {
        var strip = createTemporaryObject(tabFixture, testWindow.contentItem)
        verify(strip)
        var list = findChild(strip, "tabList")
        verify(list)
        wait(50)
        var row = list.itemAtIndex(1)
        verify(row)
        var button = findChild(row, "tabCloseButton")
        verify(button)
        mouseMove(row, row.width - 10, row.height / 2)
        tryCompare(button, "visible", true)
        mouseMove(button, button.width / 2, button.height / 2)
        compare(button.visible, true)
        mouseClick(button)
        compare(closedTabs, 1)
        compare(selectedTabs, 0)
    }

    function test_completion_surface_is_opaque_over_page_content() {
        var completion = createTemporaryObject(completionFixture,
                                               testWindow.contentItem)
        verify(completion)
        wait(50)
        var picture = grabImage(completion)
        compare(picture.red(400, 245), 24)
        compare(picture.green(400, 245), 24)
        compare(picture.blue(400, 245), 37)
    }

    function test_context_menu_is_compact_keyboard_navigable_and_selectable() {
        theme.contextMenuItems = [
            { label: "Open link", action: "open-link", value: "https://example.test" },
            { label: "Open link in new tab", action: "open-link-tab",
              value: "tab\thttps://example.test" },
            { label: "Copy link", action: "copy-link", value: "https://example.test" }
        ]
        var menu = createTemporaryObject(contextMenuFixture, testWindow.contentItem)
        verify(menu)
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        menu.openAt(testWindow, 100, 120)
        var popup = findChild(menu, "contextMenuPopup")
        verify(popup)
        if (typeof popup["popupType"] !== "undefined") {
            compare(popup["popupType"], 0)
        }
        tryCompare(popup, "visible", true)
        tryCompare(popup, "count", 3)
        tryCompare(popup, "currentIndex", 0)
        verify(popup.width <= 520)
        verify(popup.height < testWindow.height / 2)

        keyClick(Qt.Key_Down)
        compare(popup.currentIndex, 1)
        keyClick(Qt.Key_Return)
        tryCompare(chromeTest, "activatedContextItems", 1)
        compare(activatedContextAction, "open-link-tab")
        tryCompare(popup, "visible", false)
    }

    function test_large_font_rows_expand_to_fit_content() {
        var helpHost = createTemporaryObject(largeFontHelpFixture,
                                             testWindow.contentItem)
        verify(helpHost)
        var helpList = findChild(helpHost, "bindingHelpList")
        verify(helpList)
        tryVerify(function() { return helpList.itemAtIndex(0) !== null })
        var helpRow = helpList.itemAtIndex(0)
        verify(helpRow)
        verify(helpRow.height > 76)
        var details = findChild(helpRow, "bindingDetails")
        verify(details)
        verify(details.y + details.implicitHeight <= helpRow.height)

        var inventoryHost = createTemporaryObject(largeFontInventoryFixture,
                                                  testWindow.contentItem)
        verify(inventoryHost)
        var scripts = findChild(inventoryHost, "userscriptList")
        verify(scripts)
        tryVerify(function() { return scripts.itemAtIndex(0) !== null })
        var scriptRow = scripts.itemAtIndex(0)
        verify(scriptRow)
        verify(scriptRow.height > 42)

        var settingsHost = createTemporaryObject(largeFontSettingsFixture,
                                                 testWindow.contentItem)
        verify(settingsHost)
        var settingsSurface = findChild(settingsHost, "settingsSurface")
        verify(settingsSurface)
        var settingsRows = findChild(settingsSurface, "settingsList")
        verify(settingsRows)
        verify(settingsRows.height > 0)
        var settingsScroll = findChild(settingsSurface, "settingsScroll")
        verify(settingsScroll)
        verify(settingsScroll.contentHeight > settingsScroll.height)
        var settingsViewport = settingsScroll.contentItem
        verify(settingsViewport)
        settingsViewport.contentY = settingsViewport.contentHeight
                                    - settingsViewport.height
        tryVerify(function() {
            return settingsViewport.contentY + settingsViewport.height
                   >= settingsViewport.contentHeight - 1
        })
    }

    function test_spatial_grid_hides_labels_when_cells_become_too_small() {
        var fixture = createTemporaryObject(spatialGridFixture,
                                            testWindow.contentItem)
        verify(fixture)
        compare(fixture.gridItem.visible, true)
        compare(fixture.gridItem.labelsFit, true)
        verify(fixture.gridItem.labelFontSize > 12)
        verify(fixture.gridItem.reticleOuterRadius
               > fixture.gridItem.reticleGap)
        verify(fixture.gridItem.gridLineThickness
               > fixture.gridItem.gridLineCoreThickness)

        fixture.gridModel.spatial_current_width = 45
        fixture.gridModel.spatial_current_height = 30
        tryCompare(fixture.gridItem, "labelsFit", false)
        verify(fixture.gridItem.labelFontSize <= 12)

        fixture.gridModel.spatial_selecting = false
        tryCompare(fixture.gridItem, "visible", false)
    }

    function test_context_accent_matching_surface_is_readable() {
        var textColor = ChromePresentation.readableTextColor("#313244", "#313244")
        verify(ChromePresentation.contrastRatio(textColor, "#313244") >= 4.5)
    }

    function test_statusbar_visibility_matches_in_mode_policy() {
        compare(ChromePresentation.statusBarVisible("in-mode", "normal"), false)
        compare(ChromePresentation.statusBarVisible("in-mode", "command"), false)
        compare(ChromePresentation.statusBarVisible("in-mode", "search"), false)
        compare(ChromePresentation.statusBarVisible("in-mode", "insert"), true)
        compare(ChromePresentation.statusBarVisible("in-mode", "hint"), true)
        compare(ChromePresentation.statusBarVisible("in-mode", "caret"), true)
        compare(ChromePresentation.statusBarVisible("in-mode", "pass-through"), true)

        compare(ChromePresentation.statusBarVisible("always", "normal"), true)
        compare(ChromePresentation.statusBarVisible("always", "insert"), true)
        compare(ChromePresentation.statusBarVisible("always", "command"), false)
        compare(ChromePresentation.statusBarVisible("never", "normal"), false)
        compare(ChromePresentation.statusBarVisible("never", "insert"), false)
        compare(ChromePresentation.statusBarVisible("command", "hint"), true)
    }

    function test_hidden_statusbar_lazily_creates_content_when_shown() {
        var statusBar = createTemporaryObject(statusBarFixture,
                                              testWindow.contentItem)
        verify(statusBar)
        var contents = findChild(statusBar, "statusBarContents")
        verify(contents)
        compare(contents.item, null)

        statusBar.statusText = "Updated status"
        statusBar.statusVisible = true
        tryVerify(function() { return contents.item !== null })
        var message = findChild(statusBar, "statusMessageLabel")
        verify(message)
        compare(message.text, "Updated status")

        statusBar.statusVisible = false
        tryVerify(function() { return contents.item === null })
    }

    function test_shifted_ascii_key_normalization_preserves_H_binding() {
        compare(KeyboardPresentation.printableKey(
                    "h", Qt.Key_H, true, Qt.Key_A, Qt.Key_Z), "H")
        compare(KeyboardPresentation.printableKey(
                    "", Qt.Key_H, true, Qt.Key_A, Qt.Key_Z), "H")
        compare(KeyboardPresentation.printableKey(
                    "h", Qt.Key_H, false, Qt.Key_A, Qt.Key_Z), "h")
        compare(KeyboardPresentation.printableKey(
                    "λ", Qt.Key_unknown, false, Qt.Key_A, Qt.Key_Z), "λ")
    }

    function test_shutdown_decision_is_keyboard_operable() {
        var dialog = createTemporaryObject(shutdownDecisionFixture,
                                           testWindow.contentItem)
        verify(dialog)
        var keep = findChild(dialog, "shutdownKeepButton")
        var proceed = findChild(dialog, "shutdownProceedButton")
        verify(keep)
        verify(proceed)
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        dialog.focusSafeChoice()
        tryCompare(keep, "activeFocus", true)

        keyClick(Qt.Key_Return)
        compare(keptShutdowns, 1)

        dialog.decisionTaken = false
        keep.forceActiveFocus()
        keyClick(Qt.Key_Tab)
        tryCompare(proceed, "activeFocus", true)
        keyClick(Qt.Key_Return)
        compare(proceededShutdowns, 1)

        dialog.decisionTaken = false
        proceed.forceActiveFocus()
        keyClick(Qt.Key_Escape)
        compare(keptShutdowns, 2)

        dialog.decisionTaken = false
        keep.forceActiveFocus()
        keyClick(Qt.Key_Return, Qt.ControlModifier)
        compare(proceededShutdowns, 2)

        dialog.decisionTaken = false
        testWindow.contentItem.forceActiveFocus()
        keyClick(Qt.Key_Y)
        compare(proceededShutdowns, 3)

        dialog.decisionTaken = false
        testWindow.contentItem.forceActiveFocus()
        keyClick(Qt.Key_N)
        compare(keptShutdowns, 3)

        dialog.decisionTaken = false
        dialog.focusSafeChoice()
        tryCompare(keep, "activeFocus", true)
        keyClick(Qt.Key_J)
        tryCompare(dialog, "currentIndex", 1)
        tryCompare(proceed, "activeFocus", true)
        keyClick(Qt.Key_Return)
        compare(proceededShutdowns, 4)

        dialog.decisionTaken = false
        mouseClick(keep, keep.width / 2, keep.height / 2)
        compare(keptShutdowns, 4)
    }

    function test_recovery_banner_has_window_shortcuts() {
        var banner = createTemporaryObject(recoveryBannerFixture,
                                           testWindow.contentItem)
        verify(banner)
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        keyClick(Qt.Key_R, Qt.AltModifier)
        compare(recoveredSessions, 1)
        // A real recovery decision closes the surface. Reset the one-shot
        // guard so this fixture can exercise the second window shortcut too.
        banner.decisionTaken = false
        keyClick(Qt.Key_D, Qt.AltModifier)
        compare(dismissedRecoveries, 1)
    }

    function test_shared_command_dialog_supports_keyboard_and_mouse() {
        var dialog = createTemporaryObject(commandDialogFixture,
                                           testWindow.contentItem)
        verify(dialog)
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        tryCompare(dialog, "currentIndex", 0)

        keyClick(Qt.Key_J)
        compare(dialog.currentIndex, 1)
        keyClick(Qt.Key_Return)
        compare(acceptedCommandDialogs, 1)

        dialog.decisionTaken = false
        keyClick(Qt.Key_Escape)
        compare(cancelledCommandDialogs, 1)

        dialog.decisionTaken = false
        var accept = findChild(dialog, "commandDialogAction-accept")
        verify(accept)
        mouseClick(accept, accept.width / 2, accept.height / 2)
        compare(acceptedCommandDialogs, 2)
    }

    function test_versioning_consent_defaults_to_safe_one_use_decline() {
        var dialog = createTemporaryObject(versioningConsentFixture,
                                           testWindow.contentItem)
        verify(dialog)
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        tryCompare(dialog, "currentIndex", 0)
        compare(dialog.actionEnabled(1), false)
        keyClick(Qt.Key_Y)
        dialog.requestAction("accept")
        compare(acceptedStashDisclosures, 0)
        keyClick(Qt.Key_Escape)
        compare(declinedStashDisclosures, 1)
        compare(lastStashDisclosureOperation, "native-operation-1")
        dialog.requestAction("decline")
        compare(declinedStashDisclosures, 1)

        dialog.visible = false
        dialog.visible = true
        dialog.acceptEnabled = true
        tryCompare(dialog, "currentIndex", 0)
        compare(dialog.actionEnabled(1), true)
        dialog.requestAction("accept")
        compare(acceptedStashDisclosures, 1)
        compare(lastStashDisclosureOperation, "native-operation-1")
    }

    function test_page_capture_consent_uses_distinct_archive_copy_and_exact_operation() {
        var dialog = createTemporaryObject(pageCaptureConsentFixture,
                                           testWindow.contentItem)
        verify(dialog)
        compare(dialog.title, "Save a page archive?")
        compare(dialog.commandText, "archive page")
        compare(dialog.message, "This page archive may contain personal data and secrets.")
        keyClick(Qt.Key_Y)
        compare(acceptedPageCaptureDisclosures, 1)
        compare(lastStashDisclosureOperation, "native-page-operation-1")

        dialog.decisionTaken = false
        dialog.requestAction("decline")
        compare(declinedPageCaptureDisclosures, 1)
        compare(lastStashDisclosureOperation, "native-page-operation-1")
    }

    function test_versioned_restore_window_allocation_failure_preserves_source() {
        var allocations = 0
        var source = {
            tabCount: 3,
            activeUrl: "https://example.test/current",
            activeWindowId: "source-window",
            status_text: "before restore"
        }
        var factory = {
            createObject: function(parent, properties) {
                allocations += 1
                compare(parent, null)
                compare(properties.windowRestoreCommitId, "commit-id")
                return null
            }
        }
        var owner = { canCreate: function() { return true },
                      retain: function() { fail("Failed allocation must not be retained") } }
        var child = VersionedRestorePresentation.openRestoreWindow(factory, {
            windowStartupUrl: "about:blank",
            windowRestoreCommitId: "commit-id",
            windowProfileName: "default",
            windowPrivateProfile: false,
            windowEphemeralProfile: false
        }, source, owner)
        compare(child, null)
        compare(source.status_text,
                "Could not create versioned restore window; current work is intact")
        compare(source.tabCount, 3)
        compare(source.activeUrl, "https://example.test/current")
        compare(source.activeWindowId, "source-window")

        var throwingFactory = {
            createObject: function() { throw new Error("synthetic allocation failure") }
        }
        compare(VersionedRestorePresentation.openRestoreWindow(
                    throwingFactory, {}, source, owner), null)
        compare(source.tabCount, 3)
        compare(source.activeUrl, "https://example.test/current")
        compare(VersionedRestorePresentation.openRestoreWindow(null, {}, source, owner), null)
        compare(source.tabCount, 3)

        var childDestroyed = false
        var retainFailureFactory = {
            createObject: function() {
                return { destroy: function() { childDestroyed = true } }
            }
        }
        var refusingOwner = { canCreate: function() { return true },
                              retain: function() { return false } }
        compare(VersionedRestorePresentation.openRestoreWindow(
                    retainFailureFactory, {}, source, refusingOwner), null)
        verify(childDestroyed)
        compare(source.status_text,
                "Could not retain versioned restore window; current work is intact")
        compare(source.tabCount, 3)
        compare(source.activeUrl, "https://example.test/current")
        compare(source.activeWindowId, "source-window")

        childDestroyed = false
        var throwingRetainOwner = {
            canCreate: function() { return true },
            retain: function() { throw new Error("synthetic retain failure") }
        }
        compare(VersionedRestorePresentation.openRestoreWindow(
                    retainFailureFactory, {}, source, throwingRetainOwner), null)
        verify(childDestroyed)
        compare(source.status_text,
                "Could not retain versioned restore window; current work is intact")
        compare(source.tabCount, 3)
        compare(source.activeUrl, "https://example.test/current")
        compare(source.activeWindowId, "source-window")

        var allocationsBeforeOwnerFailure = allocations
        var throwingCapacityOwner = {
            canCreate: function() { throw new Error("synthetic owner failure") },
            retain: function() { fail("Unavailable owner must not retain a child") }
        }
        compare(VersionedRestorePresentation.openRestoreWindow(
                    factory, {}, source, throwingCapacityOwner), null)
        compare(allocations, allocationsBeforeOwnerFailure)
        compare(source.status_text,
                "Restore window limit reached or owner unavailable; current work is intact")
        compare(source.tabCount, 3)
        compare(source.activeUrl, "https://example.test/current")
        compare(source.activeWindowId, "source-window")
    }

    function test_native_page_capture_retirement_is_bounded_and_never_evicts_stale_paths() {
        var state = {
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: []
        }
        var limit = 3

        verify(NativePageCaptureCallbacks.retirementCapacityAvailable(state, limit))
        verify(NativePageCaptureCallbacks.retireDownload(state, "101", limit))
        verify(NativePageCaptureCallbacks.retireDownload(state, "101", limit))
        verify(!NativePageCaptureCallbacks.retireDownload(state, "bad-id", limit))
        verify(NativePageCaptureCallbacks.retirePath(state,
                                                     "/private/staging/one/capture.bin",
                                                     limit))
        verify(NativePageCaptureCallbacks.retirePath(state,
                                                     "/private/staging/one/capture.bin",
                                                     limit))
        verify(NativePageCaptureCallbacks.retirePath(state,
                                                     "/private/staging/two/capture.bin",
                                                     limit))
        verify(NativePageCaptureCallbacks.retirePath(state,
                                                     "/private/staging/three/capture.bin",
                                                     limit))
        verify(NativePageCaptureCallbacks.isRetiredPath(
                   state, "/private/staging/one/capture.bin"))
        verify(NativePageCaptureCallbacks.retireDownload(state, "102", limit))
        verify(NativePageCaptureCallbacks.retireDownload(state, "103", limit))
        compare(state.nativePageCaptureRetiredDownloadOrder.length, 3)
        verify(!NativePageCaptureCallbacks.retirementCapacityAvailable(state, limit))
        verify(!NativePageCaptureCallbacks.retireDownload(state, "104", limit))
        verify(!NativePageCaptureCallbacks.retirePath(state,
                                                      "/private/staging/four/capture.bin",
                                                      limit))
        compare(state.nativePageCaptureRetiredDownloadOrder.join(","), "101,102,103")
        compare(state.nativePageCaptureRetiredPathOrder.length, 3)
        verify(NativePageCaptureCallbacks.isRetiredPath(
                   state, "/private/staging/one/capture.bin"))

        verify(NativePageCaptureCallbacks.consumeDownload(state, "101"))
        verify(!NativePageCaptureCallbacks.consumeDownload(state, "101"))
        verify(!NativePageCaptureCallbacks.retirementCapacityAvailable(state, limit))
        verify(NativePageCaptureCallbacks.retireDownload(state, "104", limit))
        verify(NativePageCaptureCallbacks.isRetiredPath(
                   state, "/private/staging/one/capture.bin"))
    }

    function test_exhausted_page_capture_quarantine_stops_saves_for_owner_window() {
        var cancelledOperations = []
        var ui = {
            status_text: "",
            cancel_native_page_capture_request: function(operation) {
                cancelledOperations.push(operation)
            }
        }
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            nativePageCaptureRetiredDownloadLimit: 1,
            nativePageCaptureRetirementCapacityAvailable: function() {
                return NativePageCaptureCallbacks.retirementCapacityAvailable(
                            this, this.nativePageCaptureRetiredDownloadLimit)
            },
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(
                            this, path, this.nativePageCaptureRetiredDownloadLimit)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(
                            this, id, this.nativePageCaptureRetiredDownloadLimit)
            },
            discardNativePageCaptureMutationProbe: function() {},
            scheduleRuntimeWork: function() {}
        }
        verify(owner.retireNativePageCaptureDownload("990"))
        var timedOutDownload = {
            cancellations: 0,
            cancel: function() {
                compare(owner.nativePageCaptureSave, null)
                verify(owner.nativePageCaptureRetirementExhausted)
                this.cancellations++
            }
        }
        var timedOut = {
            operationId: "exhausted-timeout",
            path: "/private/staging/exhausted/timeout.mhtml",
            downloadId: "991",
            download: timedOutDownload,
            downloadFinished: false
        }
        owner.nativePageCaptureSave = timedOut
        verify(NativePageCaptureCallbacks.retireTimedOutCapture(owner, ui, timedOut))
        compare(owner.nativePageCaptureSave, null)
        verify(owner.nativePageCaptureRetirementExhausted)
        compare(timedOutDownload.cancellations, 1)
        compare(cancelledOperations.join(","), "exhausted-timeout")
        compare(ui.status_text,
                "Native page-save callback capacity is exhausted; window capture was stopped")
        verify(NativePageCaptureCallbacks.shouldIgnoreSavePageCompletion(
                   owner, { isSavePageDownload: true }))
        verify(!NativePageCaptureCallbacks.shouldIgnoreSavePageCompletion(
                   owner, { isSavePageDownload: false }))

        var rejectedRequest = {
            id: "992",
            isSavePageDownload: true,
            cancellations: 0,
            cancel: function() { this.cancellations++ }
        }
        verify(NativePageCaptureCallbacks.routeSaveRequest(owner, ui,
                                                            rejectedRequest, 7))
        compare(rejectedRequest.cancellations, 1)

        var ordinaryDownload = {
            isSavePageDownload: false,
            cancellations: 0,
            cancel: function() { this.cancellations++ }
        }
        verify(!NativePageCaptureCallbacks.routeSaveRequest(owner, ui,
                                                             ordinaryDownload, 7))
        compare(ordinaryDownload.cancellations, 0)
    }

    function test_approved_shutdown_quarantines_late_page_save_without_admission() {
        verify(NativePageCaptureCallbacks.shutdownApproved({ shutdownApproved: true }))
        verify(NativePageCaptureCallbacks.shutdownApproved({ windowShutdownApproved: true }))
        var ui = { status_text: "" }
        var owner = {
            shutdownApproved: true,
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            nativePageCaptureRetiredDownloadLimit: 8,
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            isRetiredNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.isRetiredPath(this, path)
            }
        }
        var request = {
            id: "904",
            isSavePageDownload: true,
            savePageFormat: 7,
            downloadDirectory: "/private/staging/late-shutdown",
            downloadFileName: "capture.mhtml",
            cancellations: 0,
            cancel: function() { this.cancellations++ }
        }
        Object.defineProperty(request, "view", { get: function() {
            throw new Error("late shutdown Save Page must not inspect Qt's view getter")
        } })

        verify(NativePageCaptureCallbacks.routeSaveRequest(owner, ui, request, 7, true))
        compare(request.cancellations, 1)
        verify(owner.nativePageCaptureRetiredDownloads["904"])
        verify(owner.nativePageCaptureRetiredPaths[
                   "/private/staging/late-shutdown/capture.mhtml"])
        compare(ui.status_text, "")
    }

    function test_shutdown_cancels_rust_owned_capture_before_qt_request() {
        var events = []
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureDisclosure: ["shutdown-operation", "fixed copy"],
            scheduleRuntimeWork: function(delay) {
                compare(delay, 0)
                events.push("schedule")
            }
        }
        var pending = true
        var ui = {
            status_text: "",
            cancel_pending_native_page_capture_for_shutdown: function() {
                events.push("cancel-rust-owner")
                if (!pending)
                    return false
                pending = false
                return true
            }
        }

        verify(NativePageCaptureCallbacks.cancelPendingCaptureForShutdown(owner, ui))
        compare(events.join(","), "cancel-rust-owner,schedule")
        compare(owner.nativePageCaptureDisclosure.length, 0)
        compare(ui.status_text, "Native page capture cancelled for shutdown")
        verify(!NativePageCaptureCallbacks.cancelPendingCaptureForShutdown(owner, ui))
        compare(events.length, 3)
    }

    function test_shutdown_cancels_native_capture_after_quarantining_exact_request() {
        var events = []
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            nativePageCaptureRetiredDownloadLimit: 8,
            nativePageCaptureRetirementCapacityAvailable: function() {
                return NativePageCaptureCallbacks.retirementCapacityAvailable(
                            this, this.nativePageCaptureRetiredDownloadLimit)
            },
            retireNativePageCapturePath: function(path) {
                events.push("retire-path")
                return NativePageCaptureCallbacks.retirePath(
                            this, path, this.nativePageCaptureRetiredDownloadLimit)
            },
            retireNativePageCaptureDownload: function(id) {
                events.push("retire-id")
                return NativePageCaptureCallbacks.retireDownload(
                            this, id, this.nativePageCaptureRetiredDownloadLimit)
            },
            discardNativePageCaptureMutationProbe: function() {
                events.push("discard-probe")
            },
            scheduleRuntimeWork: function(delay) {
                compare(delay, 0)
                events.push("schedule")
            }
        }
        var ui = {
            status_text: "",
            cancel_native_page_capture_request: function(operation) {
                compare(operation, "shutdown-capture")
                events.push("cancel-operation")
            }
        }
        var capture = {
            operationId: "shutdown-capture",
            tabId: "shutdown-tab",
            path: "/private/staging/shutdown/capture.mhtml",
            downloadId: "771",
            downloadFinished: false,
            mutationProbe: function() {},
            download: {
                cancellations: 0,
                cancel: function() {
                    compare(owner.nativePageCaptureSave, null)
                    verify(NativePageCaptureCallbacks.isRetiredPath(
                               owner, capture.path))
                    verify(owner.nativePageCaptureRetiredDownloads[capture.downloadId])
                    events.push("cancel-qt")
                    this.cancellations++
                }
            }
        }
        owner.nativePageCaptureSave = capture

        verify(NativePageCaptureCallbacks.cancelCaptureForShutdown(
                   owner, ui, capture))
        compare(owner.nativePageCaptureSave, null)
        compare(capture.download.cancellations, 1)
        verify(NativePageCaptureCallbacks.isRetiredPath(owner, capture.path))
        verify(owner.nativePageCaptureRetiredDownloads[capture.downloadId])
        compare(events.join(","),
                "retire-path,retire-id,discard-probe,cancel-qt,cancel-operation,schedule")
        compare(ui.status_text, "Native page capture cancelled for shutdown")
    }

    function test_shutdown_cancels_finished_qt_capture_without_recancelling_download() {
        var cancelledOperations = []
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            nativePageCaptureRetiredDownloadLimit: 8,
            nativePageCaptureRetirementCapacityAvailable: function() {
                return NativePageCaptureCallbacks.retirementCapacityAvailable(this, 8)
            },
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            discardNativePageCaptureMutationProbe: function() {},
            scheduleRuntimeWork: function() {}
        }
        var ui = {
            status_text: "",
            cancel_native_page_capture_request: function(operation) {
                cancelledOperations.push(operation)
            }
        }
        var capture = {
            operationId: "finished-shutdown-capture",
            path: "/private/staging/shutdown/finished.mhtml",
            downloadId: "772",
            downloadFinished: true,
            download: { cancellations: 0, cancel: function() { this.cancellations++ } }
        }
        owner.nativePageCaptureSave = capture

        verify(NativePageCaptureCallbacks.cancelCaptureForShutdown(owner, ui, capture))
        compare(capture.download.cancellations, 0)
        compare(owner.nativePageCaptureSave, null)
        verify(NativePageCaptureCallbacks.isRetiredPath(owner, capture.path))
        verify(owner.nativePageCaptureRetiredDownloads[capture.downloadId])
        compare(cancelledOperations.join(","), "finished-shutdown-capture")
    }

    function test_profile_admitted_unpermitted_save_page_skips_view_getter() {
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            nativePageCaptureRetirementExhausted: false,
            isRetiredNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.isRetiredPath(this, path)
            }
        }
        var ui = { status_text: "" }
        var request = {
            id: "998",
            isSavePageDownload: true,
            cancellations: 0,
            cancel: function() { this.cancellations++ }
        }
        Object.defineProperty(request, "view", { get: function() {
            throw new Error("profile-admitted Save Page must not inspect Qt's view getter")
        } })
        verify(!NativePageCaptureCallbacks.routeSaveRequest(
                   owner, ui, request, 7, true))
        compare(request.cancellations, 0)
    }

    function test_exhausted_terminal_retirement_cancels_window_without_advancing_partial_capture() {
        var failedPages = []
        var cancelledOperations = []
        var ui = {
            status_text: "",
            fail_native_window_capture_page: function() {
                failedPages.push(true)
                return true
            },
            cancel_native_page_capture_request: function(operation) {
                cancelledOperations.push(operation)
            }
        }
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            nativePageCaptureRetiredDownloadLimit: 1,
            nativePageCaptureRetirementCapacityAvailable: function() {
                return NativePageCaptureCallbacks.retirementCapacityAvailable(
                            this, this.nativePageCaptureRetiredDownloadLimit)
            },
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(
                            this, path, this.nativePageCaptureRetiredDownloadLimit)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(
                            this, id, this.nativePageCaptureRetiredDownloadLimit)
            },
            discardNativePageCaptureMutationProbe: function() {},
            scheduleRuntimeWork: function() {}
        }
        verify(owner.retireNativePageCaptureDownload("993"))
        var download = { id: "994", cancellations: 0, cancel: function() {
            this.cancellations++
        } }
        var capture = {
            operationId: "exhausted-terminal",
            tabId: "partial-page",
            path: "/private/staging/exhausted/terminal.mhtml",
            downloadId: "994",
            download: download,
            downloadFinished: false
        }
        owner.nativePageCaptureSave = capture
        verify(NativePageCaptureCallbacks.retireTerminalCapture(
                   owner, ui, capture, download, "interrupted"))
        compare(owner.nativePageCaptureSave, null)
        verify(owner.nativePageCaptureRetirementExhausted)
        compare(download.cancellations, 0)
        compare(failedPages.length, 0)
        compare(cancelledOperations.join(","), "exhausted-terminal")
        verify(NativePageCaptureCallbacks.shouldIgnoreSavePageCompletion(
                   owner, { isSavePageDownload: true }))
    }

    function test_interrupted_native_save_poisons_only_its_tab_for_this_window() {
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            nativePageCapturePoisonedTabIds: [],
            nativePageCaptureRetiredDownloadLimit: 8,
            nativePageCaptureRetirementCapacityAvailable: function() {
                return NativePageCaptureCallbacks.retirementCapacityAvailable(this, 8)
            },
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            isRetiredNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.isRetiredPath(this, path)
            },
            discardNativePageCaptureMutationProbe: function() {},
            scheduleRuntimeWork: function() {}
        }
        var ui = {
            status_text: "",
            fail_native_window_capture_page: function() { return true },
            cancel_native_page_capture_request: function() {}
        }
        var view = { url: "https://example.test/interrupted" }
        var path = "/private/staging/interrupted/capture.mhtml"
        var download = { id: "995" }
        var capture = {
            operationId: "interrupted-tab-operation",
            tabId: "interrupted-tab",
            path: path,
            view: view,
            downloadId: "995",
            download: download,
            downloadFinished: false
        }
        owner.nativePageCaptureSave = capture
        verify(NativePageCaptureCallbacks.retireTerminalCapture(
                   owner, ui, capture, download, "interrupted"))
        verify(NativePageCaptureCallbacks.captureTabIsPoisoned(
                   owner, "interrupted-tab"))
        verify(!NativePageCaptureCallbacks.captureTabIsPoisoned(
                   owner, "other-tab"))
        compare(owner.nativePageCapturePoisonedTabIds.join(","), "interrupted-tab")
        var repeatedRequest = {
            id: "995",
            isSavePageDownload: true,
            savePageFormat: 7,
            downloadDirectory: "/user/chosen",
            downloadFileName: "late.mhtml",
            cancellations: 0,
            cancel: function() { this.cancellations++ }
        }
        Object.defineProperty(repeatedRequest, "view", { get: function() {
            throw new Error("retired request must not inspect Qt's view getter")
        } })
        verify(NativePageCaptureCallbacks.routeSaveRequest(
                   owner, ui, repeatedRequest, 7))
        compare(repeatedRequest.cancellations, 1)
        verify(owner.nativePageCaptureRetiredDownloads["995"])

        var poisonedView = { url: "https://example.test/interrupted" }
        var laterPageSave = {
            id: "997",
            isSavePageDownload: true,
            savePageFormat: 7,
            downloadDirectory: "/user/chosen",
            downloadFileName: "page.mhtml",
            cancellations: 0,
            cancel: function() { this.cancellations++ }
        }
        Object.defineProperty(laterPageSave, "view", { get: function() {
            throw new Error("poisoned profile fallback must not inspect Qt's view getter")
        } })
        verify(NativePageCaptureCallbacks.routeSaveRequest(owner, ui,
                                                            laterPageSave, 7))
        compare(laterPageSave.cancellations, 1)
        verify(owner.nativePageCaptureRetiredDownloads["997"])
        compare(ui.status_text,
                "Page saves are unavailable after an interrupted native save")
        verify(!NativePageCaptureCallbacks.routeSaveRequest(owner, ui, {
            isSavePageDownload: false
        }, 7))

        var finishedCancellation = {
            tabId: "finished-cancel-tab",
            path: "/private/staging/finished-cancel/capture.mhtml",
            downloadFinished: true
        }
        verify(NativePageCaptureCallbacks.poisonCaptureTab(
                   owner, finishedCancellation, true))
        verify(NativePageCaptureCallbacks.captureTabIsPoisoned(
                   owner, "finished-cancel-tab"))

        var completed = {
            tabId: "completed-tab",
            path: "/private/staging/completed/capture.mhtml",
            downloadId: "996",
            downloadFinished: true
        }
        verify(NativePageCaptureCallbacks.retireCaptureIdentities(owner, completed, []))
        verify(!NativePageCaptureCallbacks.captureTabIsPoisoned(
                   owner, "completed-tab"))
    }

    function test_native_page_capture_request_route_rejects_duplicates_and_mismatches() {
        var cancelledOperations = []
        var admitted = []
        var ui = {
            status_text: "",
            note_native_page_capture_download_requested: function(operation, id, path,
                                                                   save, mhtml, permit) {
                admitted.push({ operation: operation, id: id, path: path,
                                save: save, mhtml: mhtml, permit: permit })
                return true
            },
            cancel_native_page_capture_request: function(operation) {
                cancelledOperations.push(operation)
            }
        }
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            isRetiredNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.isRetiredPath(this, path)
            },
            rejectNativePageCaptureRequest: function(capture, download, id) {
                return NativePageCaptureCallbacks.rejectCaptureRequest(
                            this, ui, capture, download, id)
            },
            discardNativePageCaptureMutationProbe: function() {},
            scheduleRuntimeWork: function() {},
            downloadStateName: function() { return "in-progress" }
        }
        var view = { url: "https://example.test/" }
        var path = "/private/staging/exact/capture.mhtml"
        function request(id, observedPath, observedView, throwViewGetter) {
            var slash = observedPath.lastIndexOf("/")
            var result = {
                id: id,
                isSavePageDownload: true,
                savePageFormat: 7,
                downloadDirectory: observedPath.slice(0, slash),
                downloadFileName: observedPath.slice(slash + 1),
                cancellations: 0,
                stateChanged: { connect: function() {} },
                cancel: function() { this.cancellations++ }
            }
            if (throwViewGetter) {
                Object.defineProperty(result, "view", { get: function() {
                    throw new Error("exact permit path must not inspect Qt's view getter")
                } })
            } else {
                result.view = observedView
            }
            return result
        }
        var capture = {
            operationId: "exact-operation", path: path, view: view,
            downloadId: "", download: null
        }
        owner.nativePageCaptureSave = capture
        var first = request("701", path, null, true)
        verify(NativePageCaptureCallbacks.routeSaveRequest(owner, ui, first, 7))
        compare(admitted.length, 1)
        compare(admitted[0].id, "701")
        compare(admitted[0].path, path)
        verify(admitted[0].save && admitted[0].mhtml && admitted[0].permit)
        compare(capture.download, first)
        var duplicate = request("702", path, null, true)
        verify(NativePageCaptureCallbacks.routeSaveRequest(owner, ui, duplicate, 7))
        compare(admitted.length, 1)
        compare(first.cancellations, 1)
        compare(duplicate.cancellations, 1)
        compare(cancelledOperations.join(","), "exact-operation")
        compare(owner.nativePageCaptureSave, null)
        verify(NativePageCaptureCallbacks.isRetiredPath(owner, path))
        var late = request("703", path, view)
        verify(NativePageCaptureCallbacks.routeSaveRequest(owner, ui, late, 7))
        compare(late.cancellations, 1)

        var nextPath = "/private/staging/next/capture.mhtml"
        var nextCapture = {
            operationId: "next-operation", path: nextPath, view: view,
            downloadId: "", download: null
        }
        owner.nativePageCaptureSave = nextCapture
        var mismatch = request("704", "/private/staging/wrong/capture.mhtml", null, true)
        verify(NativePageCaptureCallbacks.routeSaveRequest(owner, ui, mismatch, 7))
        compare(mismatch.cancellations, 1)
        compare(cancelledOperations.join(","), "exact-operation,next-operation")
        compare(admitted.length, 1)

        var formatMismatchPath = "/private/staging/format-mismatch/capture.mhtml"
        owner.nativePageCaptureSave = {
            operationId: "format-mismatch-operation", path: formatMismatchPath,
            view: view, downloadId: "", download: null
        }
        var formatMismatch = request("705", formatMismatchPath, null, true)
        formatMismatch.savePageFormat = 8
        verify(NativePageCaptureCallbacks.routeSaveRequest(owner, ui, formatMismatch, 7))
        compare(formatMismatch.cancellations, 1)
        compare(admitted.length, 1)
        compare(cancelledOperations.join(","),
                "exact-operation,next-operation,format-mismatch-operation")
        compare(owner.nativePageCaptureSave, null)
        verify(NativePageCaptureCallbacks.isRetiredPath(owner, formatMismatchPath))
    }

    function test_public_save_page_path_is_bounded_before_owner_matching() {
        compare(NativePageCaptureCallbacks.publicSavePagePath({
            isSavePageDownload: true,
            downloadDirectory: "/private/stage",
            downloadFileName: "capture.mhtml"
        }), "/private/stage/capture.mhtml")
        compare(NativePageCaptureCallbacks.publicSavePagePath({
            isSavePageDownload: true,
            downloadDirectory: "/private/" + Array(4090).join("x"),
            downloadFileName: "capture.mhtml"
        }), "")
        compare(NativePageCaptureCallbacks.publicSavePagePath({
            isSavePageDownload: true,
            downloadDirectory: "/private/stage",
            downloadFileName: Array(257).join("x")
        }), "")
    }

    function test_native_page_capture_cancels_at_first_observed_size_overshoot() {
        var failures = []
        var cancellations = []
        var admitted = []
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            nativePageCaptureRetirementCapacityAvailable: function() {
                return NativePageCaptureCallbacks.retirementCapacityAvailable(this, 8)
            },
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            isRetiredNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.isRetiredPath(this, path)
            },
            rejectNativePageCaptureRequest: function(capture, download, id) {
                return NativePageCaptureCallbacks.rejectCaptureRequest(
                            this, ui, capture, download, id)
            },
            discardNativePageCaptureMutationProbe: function() {},
            scheduleRuntimeWork: function() {},
            downloadStateName: function(download) {
                return download.cancellations > 0 ? "cancelled" : "in-progress"
            }
        }
        var ui = {
            status_text: "",
            note_native_page_capture_download_requested: function(
                    operation, id, path, save, mhtml, view) {
                admitted.push([operation, id, path, save, mhtml, view])
                return true
            },
            fail_native_window_capture_page: function(
                    operation, tab, reason, path, id) {
                failures.push([operation, tab, reason, path, id])
                return operation.indexOf("oversized-partial") === 0
            },
            cancel_native_page_capture_request: function(operation) {
                cancellations.push(operation)
            }
        }
        var view = { url: "https://example.test/large" }
        var path = "/private/staging/large/capture.mhtml"
        var bytesChanged = { callback: null, connect: function(callback) {
            this.callback = callback
        } }
        var request = {
            id: "805",
            isSavePageDownload: true,
            savePageFormat: 7,
            view: view,
            downloadDirectory: "/private/staging/large",
            downloadFileName: "capture.mhtml",
            receivedBytes: 0,
            receivedBytesChanged: bytesChanged,
            stateChanged: { connect: function() {} },
            cancellations: 0,
            cancel: function() {
                verify(NativePageCaptureCallbacks.isRetiredPath(owner, path))
                verify(owner.nativePageCaptureRetiredDownloads["805"] === true)
                this.cancellations++
            }
        }
        owner.nativePageCaptureSave = {
            operationId: "oversized-partial",
            tabId: "large-tab",
            maxArchiveBytes: 100,
            path: path,
            view: view,
            downloadId: "",
            download: null,
            downloadFinished: false
        }

        verify(NativePageCaptureCallbacks.routeSaveRequest(owner, ui, request, 7))
        compare(admitted.length, 1)
        verify(typeof bytesChanged.callback === "function")
        request.receivedBytes = 100
        bytesChanged.callback()
        compare(owner.nativePageCaptureSave.operationId, "oversized-partial")
        request.receivedBytes = 101
        bytesChanged.callback()

        compare(request.cancellations, 1)
        compare(failures.length, 1)
        compare(failures[0].join("|"),
                "oversized-partial|large-tab|invalid-archive|" + path + "|805")
        compare(owner.nativePageCaptureSave, null)
        compare(cancellations.length, 0)
        compare(ui.status_text,
                "An oversized page archive was discarded; continuing window capture")
        verify(NativePageCaptureCallbacks.isRetiredPath(owner, path))
        verify(owner.nativePageCaptureRetiredDownloads["805"] === true)
        bytesChanged.callback()
        compare(failures.length, 1)
        compare(request.cancellations, 1)

        var replacementCapture = {
            operationId: "next-partial-page",
            tabId: "next-tab",
            maxArchiveBytes: 100,
            path: "/private/staging/next/capture.mhtml",
            downloadId: "",
            download: null,
            downloadFinished: false
        }
        owner.nativePageCaptureSave = replacementCapture
        bytesChanged.callback()
        compare(owner.nativePageCaptureSave, replacementCapture)
        compare(failures.length, 1)
        compare(cancellations.length, 0)

        var strictPath = "/private/staging/strict-large/capture.mhtml"
        var strictView = { url: "https://example.test/strict-large" }
        var strictBytesChanged = { callback: null, connect: function(callback) {
            this.callback = callback
        } }
        var strictRequest = {
            id: "806",
            isSavePageDownload: true,
            savePageFormat: 7,
            view: strictView,
            downloadDirectory: "/private/staging/strict-large",
            downloadFileName: "capture.mhtml",
            receivedBytes: 0,
            receivedBytesChanged: strictBytesChanged,
            stateChanged: { connect: function() {} },
            cancellations: 0,
            cancel: function() {
                verify(NativePageCaptureCallbacks.isRetiredPath(owner, strictPath))
                verify(owner.nativePageCaptureRetiredDownloads["806"] === true)
                this.cancellations++
            }
        }
        owner.nativePageCaptureSave = {
            operationId: "oversized-strict",
            tabId: "strict-tab",
            maxArchiveBytes: 100,
            path: strictPath,
            view: strictView,
            downloadId: "",
            download: null,
            downloadFinished: false
        }
        verify(NativePageCaptureCallbacks.routeSaveRequest(
                   owner, ui, strictRequest, 7))
        strictRequest.receivedBytes = 101
        strictBytesChanged.callback()
        compare(strictRequest.cancellations, 1)
        compare(failures.length, 2)
        compare(failures[1].join("|"),
                "oversized-strict|strict-tab|invalid-archive|" + strictPath + "|806")
        compare(cancellations.join(","), "oversized-strict")
        compare(owner.nativePageCaptureSave, null)
        compare(ui.status_text, "Page archive exceeded its size limit and was discarded")

        var completedPath = "/private/staging/completed-large/capture.mhtml"
        var completedDownload = {
            receivedBytes: 101,
            cancellations: 0,
            cancel: function() { this.cancellations++ }
        }
        var completedCapture = {
            operationId: "oversized-partial-completed",
            tabId: "completed-large-tab",
            maxArchiveBytes: 100,
            path: completedPath,
            downloadId: "807",
            download: completedDownload,
            downloadFinished: true
        }
        owner.nativePageCaptureSave = completedCapture
        verify(NativePageCaptureCallbacks.retireOversizedCapture(
                   owner, ui, completedCapture, completedDownload))
        compare(completedDownload.cancellations, 0)
        compare(failures.length, 3)
        compare(failures[2].join("|"),
                "oversized-partial-completed|completed-large-tab|invalid-archive|"
                + completedPath + "|807")
        verify(NativePageCaptureCallbacks.isRetiredPath(owner, completedPath))
        verify(owner.nativePageCaptureRetiredDownloads["807"] === true)
    }

    function test_partial_window_navigation_before_save_request_is_page_local() {
        var failures = []
        var cancellations = []
        var ui = {
            status_text: "",
            note_native_page_capture_download_requested: function() { return false },
            fail_native_window_capture_page: function(operation, tab, reason, path, id) {
                failures.push([operation, tab, reason, path, id])
                return true
            },
            cancel_native_page_capture_request: function(operation) {
                cancellations.push(operation)
            }
        }
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            isRetiredNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.isRetiredPath(this, path)
            },
            rejectNativePageCaptureRequest: function(capture, download, id) {
                return NativePageCaptureCallbacks.rejectCaptureRequest(
                            this, ui, capture, download, id)
            },
            discardNativePageCaptureMutationProbe: function() {},
            scheduleRuntimeWork: function() {}
        }
        var path = "/private/staging/window/capture.bin"
        var view = { url: "https://example.test/new" }
        var capture = {
            operationId: "window-operation", tabId: "tab-one", path: path,
            view: view, capturedUrl: "https://example.test/original",
            downloadId: "", download: null
        }
        var request = {
            id: "803", isSavePageDownload: true, savePageFormat: 7,
            view: view, downloadDirectory: "/private/staging/window",
            downloadFileName: "capture.bin", cancellations: 0,
            cancel: function() { this.cancellations++ }
        }
        owner.nativePageCaptureSave = capture
        verify(NativePageCaptureCallbacks.routeSaveRequest(owner, ui, request, 7))
        compare(failures.length, 1)
        compare(failures[0].join("|"),
                "window-operation|tab-one|document-changed|" + path + "|")
        compare(cancellations.length, 0)
        compare(request.cancellations, 1)
        compare(owner.nativePageCaptureSave, null)
        verify(NativePageCaptureCallbacks.isRetiredPath(owner, path))
        verify(NativePageCaptureCallbacks.consumeDownload(owner, "803"))

        var unchangedPath = "/private/staging/window/unchanged.bin"
        var unchangedView = { url: "https://example.test/original" }
        owner.nativePageCaptureSave = {
            operationId: "unchanged-operation", tabId: "tab-two",
            path: unchangedPath, view: unchangedView,
            capturedUrl: unchangedView.url, downloadId: "", download: null
        }
        var unchangedRequest = {
            id: "804", isSavePageDownload: true, savePageFormat: 7,
            view: unchangedView, downloadDirectory: "/private/staging/window",
            downloadFileName: "unchanged.bin", cancellations: 0,
            cancel: function() { this.cancellations++ }
        }
        verify(NativePageCaptureCallbacks.routeSaveRequest(
                   owner, ui, unchangedRequest, 7))
        compare(failures.length, 1)
        compare(cancellations.join(","), "unchanged-operation")
        compare(unchangedRequest.cancellations, 1)
    }

    function test_closing_captured_view_quarantines_before_qt_destroys_it() {
        var failures = []
        var cancellations = []
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            discardedProbes: 0,
            scheduledWork: 0,
            nativePageCaptureRetirementCapacityAvailable: function() {
                return NativePageCaptureCallbacks.retirementCapacityAvailable(this, 8)
            },
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            discardNativePageCaptureMutationProbe: function() {
                this.discardedProbes++
            },
            scheduleRuntimeWork: function() { this.scheduledWork++ }
        }
        var ui = {
            status_text: "",
            fail_native_window_capture_page: function(operation, tab, reason, path, id) {
                failures.push([operation, tab, reason, path, id])
                return operation === "partial-close"
            },
            cancel_native_page_capture_request: function(operation) {
                cancellations.push(operation)
            }
        }
        var view = { url: "https://example.test/first" }
        var path = "/private/staging/window/capture.bin"
        var capture = {
            operationId: "partial-close", tabId: "first-tab",
            view: view, path: path, downloadId: "905",
            downloadFinished: false,
            download: {
                cancel: function() {
                    compare(owner.nativePageCaptureSave, null)
                    verify(NativePageCaptureCallbacks.isRetiredPath(owner, path))
                    verify(owner.nativePageCaptureRetiredDownloads["905"] === true)
                }
            }
        }
        owner.nativePageCaptureSave = capture
        verify(!NativePageCaptureCallbacks.retireClosingView(owner, ui, {}))
        compare(owner.nativePageCaptureSave, capture)
        verify(NativePageCaptureCallbacks.retireClosingView(owner, ui, view))
        compare(failures[0].join("|"),
                "partial-close|first-tab|native-view-lost|" + path + "|905")
        compare(cancellations.length, 0)
        compare(owner.discardedProbes, 1)
        compare(owner.scheduledWork, 1)
        compare(ui.status_text,
                "The closed page could not be archived; continuing the partial capture")
        verify(!NativePageCaptureCallbacks.retireClosingView(owner, ui, view))

        var finishedView = { url: "https://example.test/finished" }
        var finishedPath = "/private/staging/window/finished.bin"
        var finishedCapture = {
            operationId: "finished-close", tabId: "third-tab",
            view: finishedView, path: finishedPath, downloadId: "906",
            downloadFinished: true
        }
        owner.nativePageCaptureSave = finishedCapture
        verify(!NativePageCaptureCallbacks.retireClosingView(owner, ui, finishedView))
        compare(owner.nativePageCaptureSave, finishedCapture)
        compare(failures.length, 1)
        compare(cancellations.length, 0)
        verify(!owner.nativePageCaptureRetiredPaths[finishedPath])
        verify(!owner.nativePageCaptureRetiredDownloads["906"])

        var strictView = { url: "https://example.test/strict" }
        owner.nativePageCaptureSave = {
            operationId: "strict-close", tabId: "second-tab",
            view: strictView, path: "", downloadId: "",
            downloadFinished: false
        }
        verify(NativePageCaptureCallbacks.retireClosingView(owner, ui, strictView))
        compare(cancellations.join(","), "strict-close")
        compare(ui.status_text, "Window capture was cancelled because a page closed")
        compare(owner.nativePageCaptureSave, null)
    }

    function test_native_page_capture_deadline_retires_exact_save_before_qt_cancel() {
        var observations = []
        var cancellations = []
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            discardedProbes: 0,
            scheduledWork: 0,
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            discardNativePageCaptureMutationProbe: function() {
                this.discardedProbes++
            },
            scheduleRuntimeWork: function() { this.scheduledWork++ }
        }
        var ui = {
            status_text: "",
            fail_native_window_capture_page: function(operation, tab, reason, path, id) {
                observations.push([operation, tab, reason, path, id])
                return operation === "partial-operation"
            },
            cancel_native_page_capture_request: function(operation) {
                cancellations.push(operation)
            }
        }
        var path = "/private/staging/window/capture.bin"
        var download = {
            cancel: function() {
                verify(owner.nativePageCaptureSave === null)
                verify(NativePageCaptureCallbacks.isRetiredPath(owner, path))
                verify(owner.nativePageCaptureRetiredDownloads["901"] === true)
            }
        }
        var capture = {
            operationId: "partial-operation", tabId: "first-tab",
            path: path, downloadId: "901", download: download,
            downloadFinished: false
        }
        owner.nativePageCaptureSave = capture
        verify(NativePageCaptureCallbacks.retireTimedOutCapture(owner, ui, capture))
        compare(observations[0].join("|"),
                "partial-operation|first-tab|timeout|" + path + "|901")
        compare(cancellations.length, 0)
        compare(owner.discardedProbes, 1)
        compare(owner.scheduledWork, 1)
        verify(!NativePageCaptureCallbacks.retireTimedOutCapture(owner, ui, capture))

        var finished = {
            operationId: "finished-operation", tabId: "second-tab",
            path: "/private/staging/window/finished.bin",
            downloadId: "902", downloadFinished: true
        }
        owner.nativePageCaptureSave = finished
        verify(!NativePageCaptureCallbacks.retireTimedOutCapture(owner, ui, finished))
        compare(owner.nativePageCaptureSave, finished)

        var strict = {
            operationId: "strict-operation", tabId: "second-tab",
            path: "/private/staging/window/strict.bin",
            downloadId: "", downloadFinished: false
        }
        owner.nativePageCaptureSave = strict
        verify(NativePageCaptureCallbacks.retireTimedOutCapture(owner, ui, strict))
        compare(cancellations.join(","), "strict-operation")
        compare(owner.nativePageCaptureSave, null)
    }

    function test_native_page_capture_terminal_state_retires_only_exact_request_once() {
        var cancelledOperations = []
        var ui = {
            status_text: "",
            cancel_native_page_capture_request: function(operationId) {
                cancelledOperations.push(operationId)
            }
        }
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            discardedProbes: 0,
            scheduledWork: 0,
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            discardNativePageCaptureMutationProbe: function() {
                this.discardedProbes += 1
            },
            scheduleRuntimeWork: function() {
                this.scheduledWork += 1
            }
        }
        var download = { id: "501" }
        var capture = {
            operationId: "exact-operation",
            downloadId: "501",
            download: download,
            downloadFinished: false,
            path: "/private/staging/exact/capture.mhtml"
        }
        owner.nativePageCaptureSave = capture

        verify(!NativePageCaptureCallbacks.retireTerminalCapture(
                   owner, ui, capture, null, "cancelled"))
        verify(!NativePageCaptureCallbacks.retireTerminalCapture(
                   owner, ui, capture, { id: "501" }, "cancelled"))
        verify(!NativePageCaptureCallbacks.retireTerminalCapture(
                   owner, ui, capture, download, "completed"))
        compare(cancelledOperations.length, 0)
        compare(owner.nativePageCaptureRetiredPathOrder.length, 0)
        compare(owner.nativePageCaptureSave, capture)

        verify(NativePageCaptureCallbacks.retireTerminalCapture(
                   owner, ui, capture, download, "cancelled"))
        compare(cancelledOperations.join(","), "exact-operation")
        compare(owner.discardedProbes, 1)
        compare(owner.scheduledWork, 1)
        compare(owner.nativePageCaptureSave, null)
        compare(ui.status_text, "Page archive save was cancelled and discarded")
        verify(NativePageCaptureCallbacks.isRetiredPath(owner, capture.path))
        verify(NativePageCaptureCallbacks.consumeDownload(owner, "501"))
        verify(!NativePageCaptureCallbacks.retireTerminalCapture(
                   owner, ui, capture, download, "cancelled"))
        compare(cancelledOperations.length, 1)
        compare(owner.discardedProbes, 1)

        var nextDownload = { id: "502" }
        var nextCapture = {
            operationId: "next-operation",
            downloadId: "502",
            download: nextDownload,
            downloadFinished: true,
            path: "/private/staging/next/capture.mhtml"
        }
        owner.nativePageCaptureSave = nextCapture
        verify(!NativePageCaptureCallbacks.retireTerminalCapture(
                   owner, ui, nextCapture, nextDownload, "interrupted"))
        compare(owner.nativePageCaptureSave, nextCapture)
        nextCapture.downloadFinished = false
        verify(!NativePageCaptureCallbacks.retireTerminalCapture(
                   owner, ui, capture, download, "interrupted"))
        verify(NativePageCaptureCallbacks.retireTerminalCapture(
                   owner, ui, nextCapture, nextDownload, "interrupted"))
        compare(cancelledOperations.join(","), "exact-operation,next-operation")
        compare(ui.status_text, "Page archive save was interrupted and discarded")
        compare(owner.scheduledWork, 2)
    }

    function test_partial_window_terminal_state_advances_only_exact_page() {
        var failedPages = []
        var cancelledOperations = []
        var ui = {
            status_text: "",
            fail_native_window_capture_page: function(operation, tab, reason, path, id) {
                failedPages.push([operation, tab, reason, path, id])
                return operation === "window-operation" && tab === "tab-two"
            },
            cancel_native_page_capture_request: function(operation) {
                cancelledOperations.push(operation)
            }
        }
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            discardNativePageCaptureMutationProbe: function() {},
            scheduleRuntimeWork: function() {}
        }
        var download = { id: "602" }
        var capture = {
            operationId: "window-operation",
            tabId: "tab-two",
            downloadId: "602",
            download: download,
            downloadFinished: false,
            path: "/private/staging/window/capture-page-1.bin"
        }
        owner.nativePageCaptureSave = capture
        verify(NativePageCaptureCallbacks.retireTerminalCapture(
                   owner, ui, capture, download, "interrupted"))
        compare(failedPages.length, 1)
        compare(failedPages[0].join("|"),
                "window-operation|tab-two|interrupted|/private/staging/window/capture-page-1.bin|602")
        compare(cancelledOperations.length, 0)
        compare(owner.nativePageCaptureSave, null)
        verify(NativePageCaptureCallbacks.isRetiredPath(owner, capture.path))
        verify(NativePageCaptureCallbacks.consumeDownload(owner, "602"))
    }

    function test_page_capture_probe_refuses_changed_view_or_route() {
        var capturedView = { url: "https://example.test/original" }
        var otherView = { url: "https://example.test/original" }
        var capture = { view: capturedView, capturedUrl: "https://example.test/original" }
        var facts = { observed_url: "https://example.test/original" }
        compare(NativePageCaptureCallbacks.mutationEvidenceForCurrentView(
                    capture, facts, "observed-stable", capturedView), "observed-stable")
        compare(NativePageCaptureCallbacks.mutationEvidenceForCurrentView(
                    capture, facts, "invalid", capturedView), "not-observed")
        compare(NativePageCaptureCallbacks.mutationEvidenceForCurrentView(
                    capture, facts, "observed-stable", otherView), "document-changed")
        capturedView.url = "https://example.test/next"
        compare(NativePageCaptureCallbacks.mutationEvidenceForCurrentView(
                    capture, facts, "observed-stable", capturedView), "document-changed")
        facts.observed_url = "https://example.test/next"
        compare(NativePageCaptureCallbacks.mutationEvidenceForCurrentView(
                    capture, facts, "observed-stable", capturedView), "document-changed")
        capture.view = { get url() { throw new Error("destroyed view") } }
        compare(NativePageCaptureCallbacks.mutationEvidenceForCurrentView(
                    capture, facts, "not-observed", capture.view), "document-changed")
    }

    function test_page_capture_completion_requires_retained_request_and_live_view() {
        var view = { url: "https://example.test/original" }
        var download = { id: "705" }
        Object.defineProperty(download, "view", { get: function() {
            throw new Error("destroyed Qt view getter must not run")
        } })
        var capture = { view: view, download: download }
        var accepted = NativePageCaptureCallbacks.completionViewFacts(
                    capture, download, view)
        compare(accepted.request_matches, true)
        compare(accepted.view_matches, true)
        compare(accepted.observed_url, view.url)
        var wrongRequest = NativePageCaptureCallbacks.completionViewFacts(
                    capture, { id: "705" }, view)
        compare(wrongRequest.request_matches, false)
        compare(wrongRequest.view_matches, false)
        var closed = NativePageCaptureCallbacks.completionViewFacts(
                    capture, download, null)
        compare(closed.request_matches, true)
        compare(closed.view_matches, false)
        compare(closed.observed_url, "")
        var switched = NativePageCaptureCallbacks.completionViewFacts(
                    capture, download, { url: view.url })
        compare(switched.view_matches, false)
        capture.view = { get url() { throw new Error("destroyed captured view") } }
        var destroyed = NativePageCaptureCallbacks.completionViewFacts(
                    capture, download, capture.view)
        compare(destroyed.view_matches, false)
        compare(destroyed.observed_url, "")
    }

    function test_page_capture_is_revalidated_after_async_collection_before_save() {
        var view = { url: "https://example.test/original" }
        var capture = { view: view, capturedUrl: view.url }
        compare(NativePageCaptureCallbacks.preSaveCaptureStatus(capture, view),
                "current")
        compare(NativePageCaptureCallbacks.preSaveCaptureStatus(capture, null),
                "native-view-lost")
        compare(NativePageCaptureCallbacks.preSaveCaptureStatus(
                    capture, { url: view.url }), "native-view-lost")

        view.url = "https://example.test/next"
        compare(NativePageCaptureCallbacks.preSaveCaptureStatus(capture, view),
                "document-changed")
        view.url = ""
        compare(NativePageCaptureCallbacks.preSaveCaptureStatus(capture, view),
                "native-view-lost")
        view.url = "https://example.test/next"

        capture.view = { get url() { throw new Error("destroyed view") } }
        compare(NativePageCaptureCallbacks.preSaveCaptureStatus(
                    capture, capture.view), "native-view-lost")
        compare(NativePageCaptureCallbacks.preSaveCaptureStatus(
                    { view: view, capturedUrl: "" }, view), "native-view-lost")
    }

    function test_page_capture_completion_classifies_missing_view_separately() {
        var completed = {
            terminal_state: "completed",
            received_bytes: 64,
            view_matches: true,
            mutation_evidence: "observed-stable"
        }
        compare(NativePageCaptureCallbacks.completedArchiveFailure(
                    completed, 128, true), "")
        completed.view_matches = false
        completed.mutation_evidence = "document-changed"
        compare(NativePageCaptureCallbacks.completedArchiveFailure(
                    completed, 128, true), "native-view-lost")

        completed.view_matches = true
        compare(NativePageCaptureCallbacks.completedArchiveFailure(
                    completed, 128, true), "document-changed")
        completed.terminal_state = "cancelled"
        compare(NativePageCaptureCallbacks.completedArchiveFailure(
                    completed, 128, true), "cancelled")
        completed.terminal_state = "interrupted"
        compare(NativePageCaptureCallbacks.completedArchiveFailure(
                    completed, 128, true), "interrupted")
        completed.terminal_state = "completed"
        completed.received_bytes = 0
        compare(NativePageCaptureCallbacks.completedArchiveFailure(
                    completed, 128, true), "empty-archive")
        completed.received_bytes = 129
        compare(NativePageCaptureCallbacks.completedArchiveFailure(
                    completed, 128, true), "invalid-archive")
        completed.received_bytes = 64.5
        compare(NativePageCaptureCallbacks.completedArchiveFailure(
                    completed, 128, true), "invalid-archive")
        completed.received_bytes = 64
        compare(NativePageCaptureCallbacks.completedArchiveFailure(
                    completed, 128, false), "invalid-archive")
        compare(NativePageCaptureCallbacks.completedArchiveFailure(
                    null, 128, true), "invalid-archive")
    }

    function test_superseded_page_capture_retires_exact_native_request_before_replacement() {
        var cancellations = []
        var oldCapture = {
            operationId: "old-operation",
            downloadId: "601",
            path: "/private/staging/old/capture.mhtml",
            downloadFinished: false,
            download: {
                cancel: function() {
                    compare(owner.nativePageCaptureSave, null)
                    verify(NativePageCaptureCallbacks.isRetiredPath(
                               owner, oldCapture.path))
                    verify(owner.nativePageCaptureRetiredDownloads["601"] === true)
                    cancellations.push("native")
                }
            }
        }
        var owner = {
            nativePageCaptureSave: oldCapture,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            discardedProbes: 0,
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            discardNativePageCaptureMutationProbe: function() {
                this.discardedProbes += 1
            }
        }
        var ui = {
            cancel_native_page_capture_request: function(operation) {
                cancellations.push(operation)
            }
        }
        verify(!NativePageCaptureCallbacks.retireSupersededCapture(
                   owner, ui, oldCapture, "old-operation"))
        verify(!NativePageCaptureCallbacks.retireSupersededCapture(
                   owner, ui, oldCapture, ""))
        compare(owner.nativePageCaptureSave, oldCapture)
        verify(NativePageCaptureCallbacks.retireSupersededCapture(
                   owner, ui, oldCapture, "new-operation"))
        compare(owner.nativePageCaptureSave, null)
        compare(owner.discardedProbes, 1)
        compare(cancellations.join(","), "native,old-operation")
        verify(!NativePageCaptureCallbacks.retireSupersededCapture(
                   owner, ui, oldCapture, "next-operation"))
        compare(cancellations.length, 2)
    }

    function test_superseded_page_capture_fails_closed_if_exact_quarantine_is_full() {
        var nativeCancels = 0
        var rustCancels = 0
        var capture = {
            operationId: "old-operation",
            downloadId: "602",
            path: "/private/staging/old/capture.mhtml",
            downloadFinished: false,
            download: { cancel: function() { nativeCancels++ } }
        }
        var owner = {
            nativePageCaptureSave: capture,
            retiredPaths: 0,
            discardedProbes: 0,
            retireNativePageCapturePath: function() {
                this.retiredPaths++
                return false
            },
            retireNativePageCaptureDownload: function() {
                fail("download ID must not be retired after path quarantine refusal")
            },
            discardNativePageCaptureMutationProbe: function() {
                this.discardedProbes++
            }
        }
        var ui = {
            cancel_native_page_capture_request: function() { rustCancels++ }
        }
        verify(!NativePageCaptureCallbacks.retireSupersededCapture(
                   owner, ui, capture, "new-operation"))
        compare(owner.nativePageCaptureSave, null)
        verify(owner.nativePageCaptureRetirementExhausted)
        compare(owner.retiredPaths, 1)
        compare(owner.discardedProbes, 1)
        compare(nativeCancels, 1)
        compare(rustCancels, 1)
        verify(NativePageCaptureCallbacks.shouldIgnoreSavePageCompletion(
                   owner, { isSavePageDownload: true }))
    }

    function test_rejected_page_capture_quarantines_before_cancelling_both_saves() {
        var cancellations = []
        var cancelledOperations = []
        var owner = {
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredDownloadOrder: [],
            nativePageCaptureRetiredPaths: ({}),
            nativePageCaptureRetiredPathOrder: [],
            discardedProbes: 0,
            scheduledWork: 0,
            retireNativePageCapturePath: function(path) {
                return NativePageCaptureCallbacks.retirePath(this, path, 8)
            },
            retireNativePageCaptureDownload: function(id) {
                return NativePageCaptureCallbacks.retireDownload(this, id, 8)
            },
            discardNativePageCaptureMutationProbe: function() {
                compare(this.nativePageCaptureSave, null)
                this.discardedProbes += 1
            },
            scheduleRuntimeWork: function() {
                this.scheduledWork += 1
            }
        }
        var ui = {
            status_text: "",
            cancel_native_page_capture_request: function(operationId) {
                cancelledOperations.push(operationId)
            }
        }
        var original = {
            cancel: function() {
                cancellations.push({ id: "601",
                    pathRetired: NativePageCaptureCallbacks.isRetiredPath(
                        owner, "/private/staging/capture.mhtml"),
                    idRetired: owner.nativePageCaptureRetiredDownloads["601"] === true,
                    currentCleared: owner.nativePageCaptureSave === null })
            }
        }
        var duplicate = {
            cancel: function() {
                cancellations.push({ id: "602",
                    pathRetired: NativePageCaptureCallbacks.isRetiredPath(
                        owner, "/private/staging/capture.mhtml"),
                    idRetired: owner.nativePageCaptureRetiredDownloads["602"] === true,
                    currentCleared: owner.nativePageCaptureSave === null })
            }
        }
        var capture = {
            operationId: "duplicate-operation",
            downloadId: "601",
            download: original,
            path: "/private/staging/capture.mhtml"
        }
        owner.nativePageCaptureSave = capture
        verify(NativePageCaptureCallbacks.rejectCaptureRequest(
                   owner, ui, capture, duplicate, "602"))
        compare(cancellations.length, 2)
        compare(cancellations[0].id, "601")
        compare(cancellations[1].id, "602")
        for (var i = 0; i < cancellations.length; ++i) {
            verify(cancellations[i].pathRetired)
            verify(cancellations[i].idRetired)
            verify(cancellations[i].currentCleared)
        }
        compare(cancelledOperations.join(","), "duplicate-operation")
        compare(ui.status_text,
                "Page archive save did not match the captured page and was discarded")
        compare(owner.discardedProbes, 1)
        compare(owner.scheduledWork, 1)
        verify(!NativePageCaptureCallbacks.rejectCaptureRequest(
                   owner, ui, capture, duplicate, "602"))
        compare(cancellations.length, 2)

        var nextCapture = {
            operationId: "mismatched-operation",
            downloadId: "",
            download: null,
            path: "/private/staging/next.mhtml"
        }
        var throwingCancelInvoked = false
        var throwingDownload = {
            cancel: function() {
                throwingCancelInvoked = true
                throw new Error("synthetic Qt cancellation failure")
            }
        }
        owner.nativePageCaptureSave = nextCapture
        var staleRequestCancelled = false
        var staleRequest = { cancel: function() { staleRequestCancelled = true } }
        verify(!NativePageCaptureCallbacks.rejectCaptureRequest(
                   owner, ui, capture, staleRequest, "604"))
        verify(staleRequestCancelled)
        verify(owner.nativePageCaptureRetiredDownloads["604"] === true)
        compare(owner.nativePageCaptureSave, nextCapture)
        verify(NativePageCaptureCallbacks.rejectCaptureRequest(
                   owner, ui, nextCapture, throwingDownload, "603"))
        verify(throwingCancelInvoked)
        compare(cancellations.length, 2)
        compare(cancelledOperations.join(","),
                "duplicate-operation,mismatched-operation")
        verify(NativePageCaptureCallbacks.isRetiredPath(
                   owner, "/private/staging/next.mhtml"))
        verify(owner.nativePageCaptureRetiredDownloads["603"] === true)
    }
}
