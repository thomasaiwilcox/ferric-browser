import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtTest
import "../../qml/components"
import "../../qml/scripts/ChromePresentation.js" as ChromePresentation
import "../../qml/scripts/KeyboardPresentation.js" as KeyboardPresentation
import "../../qml/scripts/TargetSearchPresentation.js" as TargetSearchPresentation

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
    property int acceptedEngineFileDialogs: 0
    property int rejectedEngineFileDialogs: 0
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
        property color modeInsertColor: "#a6e3a1"
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
        property var primaryBrowserUi: helpUi
        property bool bindingHelpVisible: true
        property bool recoveryAvailable: true
        property bool journeyExportPreviewVisible: false
        property string journeyExportPreviewText: ""
        property bool privateHistoryTransferVisible: false
        property bool siteLedgerVisible: true
        property bool siteDataClearPending: false
        property var siteLedgerData: ({
            private: true,
            origin: "",
            facts: [],
            blocking: ({ active_site_decisions: [] }),
            site_doctor: ({ active: ({ kind: "" }), experiments: [] }),
            interaction: ({})
        })
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
        function addressPresentation(value, count) { return value }
    }

    QtObject {
        id: helpUi
        property string config_json: "{}"
        property var guide_categories: ["Opening Pages and Search"]
        property var guide_titles: ["Open a page"]
        property var guide_keys: ["o"]
        property var guide_modes: ["normal"]
        property var guide_commands: ["open"]
        property var guide_summaries: ["Open a URL or search query"]
        property var guide_details: ["Enter a URL and press Enter. Escape cancels."]
        function refresh_guide(search) {}
    }

    ListModel { id: settingsModel }
    ListModel { id: tabsModel }

    Component {
        id: primaryStatusFixture
        FerricStatusBar {
            width: 900
            browserWindow: theme
            statusVisible: true
            mode: "pass-through"
            displayUrl: "https://example.test"
            statusText: "Ready"
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
        id: secondaryStatusFixture
        FerricWindowStatusBar {
            width: 900
            browserWindow: theme
            statusVisible: true
            mode: "pass-through"
            displayUrl: "https://example.test"
            statusText: "Ready"
            profileLabel: "default"
            statusColor: theme.primaryTextColor
            profileColor: theme.primaryTextColor
            accessibleDetails: ""
        }
    }

    Component {
        id: siteLedgerFixture
        FerricSiteLedger {
            width: 900
            height: 660
            browserWindow: theme
            siteExperimentAvailable: false
        }
    }

    function test_passthrough_status_feedback_in_both_windows() {
        for (var fixture of [primaryStatusFixture, secondaryStatusFixture]) {
            var bar = createTemporaryObject(fixture, testWindow.contentItem)
            verify(bar !== null)
            var label = findChild(bar, "inputModeLabel")
            var hint = findChild(bar, "inputModeHint")
            compare(label.text, "PASS THROUGH")
            compare(hint.text, "Ctrl+Esc: Normal")
            compare(label.parent.color, theme.accentColor)
            verify(bar.Accessible.name.indexOf("Escape") >= 0)
            bar.mode = "pass-through-once"
            compare(label.text, "PASS NEXT KEY")
            verify(hint.visible)
            verify(bar.Accessible.name.indexOf("Next key") >= 0)
            bar.mode = "normal"
            verify(!hint.visible)
        }
        compare(ChromePresentation.statusBarVisible("in-mode", "pass-through-once"), true)
        compare(ChromePresentation.statusBarVisible("never", "pass-through-once"), false)
    }

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
        id: fileDialogSurfacesFixture
        FerricFileDialogSurfaces {
            onEngineFileAccepted: chromeTest.acceptedEngineFileDialogs++
            onEngineFileRejected: chromeTest.rejectedEngineFileDialogs++
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
        id: guideHelpFixture
        FerricBindingHelp {
            property int closeRequests: 0
            browserWindow: theme
            onCloseRequested: closeRequests++
        }
    }

    function test_guide_help_escape_returns_focus_before_closing() {
        var help = createTemporaryObject(guideHelpFixture, testWindow.contentItem)
        verify(help)
        wait(0)
        var guide = findChild(help, "ferricGuide")
        guide.searchInput.text = "open"
        guide.searchInput.forceActiveFocus()
        verify(!help.dismissOnEscape)
        keyClick(Qt.Key_Escape)
        compare(help.closeRequests, 0)
        compare(guide.searchText, "")
        guide.focusReader()
        wait(0)
        verify(!help.dismissOnEscape)
        keyClick(Qt.Key_Escape)
        tryVerify(function() { return guide.entriesList.activeFocus })
        verify(guide.articleOpen)
        compare(help.closeRequests, 0)
        verify(help.dismissOnEscape)
        keyClick(Qt.Key_Escape)
        tryCompare(help, "closeRequests", 1)
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
                property real spatial_viewport_width: 300
                property real spatial_viewport_height: 240
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

    function test_native_prompt_file_chooser_accept_and_cancel_by_keyboard() {
        var accepted = createTemporaryObject(fileDialogSurfacesFixture,
                                             testWindow.contentItem, {
                                                 engineFileOptions: FileDialog.DontUseNativeDialog,
                                                 engineFileMode: FileDialog.SaveFile,
                                                 engineFileCurrentFolder: "file:///tmp/"
                                             })
        verify(accepted)
        accepted.openEngineFile()
        tryCompare(accepted, "engineFileVisible", true)
        var acceptName = findChild(testWindow, "fileNameTextField")
        verify(acceptName)
        tryCompare(acceptName, "visible", true)
        acceptName.forceActiveFocus()
        tryCompare(acceptName, "activeFocus", true)
        acceptName.text = "ferric-keyboard-accept.txt"
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_Space)
        tryCompare(chromeTest, "acceptedEngineFileDialogs", 1, 3000)
        tryCompare(accepted, "engineFileVisible", false)
        accepted.openEngineFile()
        tryCompare(accepted, "engineFileVisible", true)
        acceptName.forceActiveFocus()
        tryCompare(acceptName, "activeFocus", true)
        keyClick(Qt.Key_Escape)
        tryCompare(chromeTest, "rejectedEngineFileDialogs", 1, 3000)
    }

    function test_large_font_rows_expand_to_fit_content() {
        var helpHost = createTemporaryObject(largeFontHelpFixture,
                                             testWindow.contentItem)
        verify(helpHost)
        var helpList = findChild(helpHost, "ferricGuideEntries")
        verify(helpList)
        tryVerify(function() { return helpList.itemAtIndex(1) !== null })
        var helpRow = helpList.itemAtIndex(1)
        verify(helpRow)
        verify(helpRow.height > 76)
        var details = findChild(helpRow, "guideFeatureContent")
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

    function test_spatial_grid_maps_nonzero_scope_into_full_view() {
        var fixture = createTemporaryObject(spatialGridFixture,
                                            testWindow.contentItem)
        verify(fixture)
        fixture.gridModel.spatial_viewport_width = 600
        fixture.gridModel.spatial_viewport_height = 480
        fixture.gridModel.spatial_current_x = 120
        fixture.gridModel.spatial_current_y = 72
        fixture.gridModel.spatial_current_width = 240
        fixture.gridModel.spatial_current_height = 192
        compare(fixture.gridItem.cx, 60)
        compare(fixture.gridItem.cy, 36)
        compare(fixture.gridItem.cw, 120)
        compare(fixture.gridItem.ch, 96)
    }

    function test_spatial_grid_discloses_unqualified_double_click_gate() {
        var fixture = createTemporaryObject(spatialGridFixture,
                                           testWindow.contentItem)
        verify(fixture)
        fixture.gridModel.spatial_help_visible = true
        compare(fixture.gridItem.doubleClickEnabled, false)
        verify(fixture.gridItem.Accessible.description.indexOf("unavailable") >= 0)
    }

    function test_context_accent_matching_surface_is_readable() {
        var textColor = ChromePresentation.readableTextColor("#313244", "#313244")
        verify(ChromePresentation.contrastRatio(textColor, "#313244") >= 4.5)
    }

    function test_siteLedgerInteractionStatusIsOnDemandAndRedacted() {
        theme.siteLedgerData = {
            private: true,
            origin: "",
            facts: [],
            blocking: ({ active_site_decisions: [] }),
            site_doctor: ({ active: ({ kind: "" }), experiments: [] }),
            interaction: {
                mode: "normal",
                focus: { category: "editable", frame_count: 2 },
                shortcut_policy: {
                    value: "mixed",
                    source: "site",
                    page_key_count: 4,
                    policy_site_rule: "docs-shortcuts"
                },
                native_prompt: {
                    active: false,
                    category: "none",
                    last_outcome: "awaiting-focus"
                },
                scroll_policy: { status: "unknown" },
                recent_action_errors: {
                    categories: [{ category: "stale-target", count: 2 }]
                },
                private_url_forbidden: "https://private.test/doc?token=secret"
            }
        }
        var ledger = createTemporaryObject(siteLedgerFixture, testWindow.contentItem)
        verify(ledger !== null)
        var summary = findChild(ledger, "siteLedgerInteractionSummary")
        verify(summary !== null)
        verify(summary.text.indexOf("focus editable") >= 0)
        verify(summary.text.indexOf("mixed (site)") >= 0)
        verify(summary.text.indexOf("policy rule docs-shortcuts") >= 0)
        verify(summary.text.indexOf("prompt outcome awaiting-focus") >= 0)
        verify(summary.text.indexOf("stale-target ×2") >= 0)
        verify(summary.text.indexOf("private.test") < 0)
        verify(summary.text.indexOf("token=secret") < 0)
    }

    function test_statusbar_visibility_matches_in_mode_policy() {
        compare(ChromePresentation.statusBarVisible("in-mode", "normal"), false)
        compare(ChromePresentation.statusBarVisible("in-mode", "command"), false)
        compare(ChromePresentation.statusBarVisible("in-mode", "search"), false)
        compare(ChromePresentation.statusBarVisible("in-mode", "insert"), true)
        compare(ChromePresentation.statusBarVisible("in-mode", "insert-locked"), true)
        compare(ChromePresentation.statusBarVisible("in-mode", "hint"), true)
        compare(ChromePresentation.statusBarVisible("in-mode", "caret"), true)
        compare(ChromePresentation.statusBarVisible("in-mode", "pass-through"), true)

        compare(ChromePresentation.statusBarVisible("always", "normal"), true)
        compare(ChromePresentation.statusBarVisible("always", "insert"), true)
        compare(ChromePresentation.statusBarVisible("always", "insert-locked"), true)
        compare(ChromePresentation.statusBarVisible("always", "command"), false)
        compare(ChromePresentation.statusBarVisible("never", "normal"), false)
        compare(ChromePresentation.statusBarVisible("never", "insert"), false)
        compare(ChromePresentation.statusBarVisible("never", "insert-locked"), false)
        compare(ChromePresentation.statusBarVisible("command", "hint"), true)
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

    function test_insert_entry_keys_remain_distinct_on_webengine_paths() {
        compare(KeyboardPresentation.printableKey(
                    "i", Qt.Key_I, false, Qt.Key_A, Qt.Key_Z), "i")
        compare(KeyboardPresentation.printableKey(
                    "i", Qt.Key_I, true, Qt.Key_A, Qt.Key_Z), "I")
        compare(KeyboardPresentation.printableKey(
                    "", Qt.Key_I, true, Qt.Key_A, Qt.Key_Z), "I")
    }

    function test_start_page_cycle_key_keeps_both_modifiers() {
        compare(KeyboardPresentation.logicalNormalKeyText({
            key: Qt.Key_Space, text: " ",
            modifiers: Qt.ControlModifier | Qt.ShiftModifier
        }), "Ctrl+Shift+Space")
        compare(KeyboardPresentation.logicalNormalKeyText({
            key: Qt.Key_Space, text: "", modifiers: Qt.ControlModifier
        }), "Ctrl+Space")
        compare(KeyboardPresentation.logicalNormalKeyText({
            key: Qt.Key_Space, text: " ", modifiers: Qt.NoModifier
        }), " ")
    }

    function test_modified_space_does_not_claim_other_shortcuts() {
        for (var extra of [Qt.AltModifier, Qt.MetaModifier]) {
            compare(KeyboardPresentation.logicalNormalKeyText({
                key: Qt.Key_Space, text: " ",
                modifiers: Qt.ControlModifier | Qt.ShiftModifier | extra
            }), "")
        }
        compare(KeyboardPresentation.logicalNormalKeyText({
            key: Qt.Key_V, text: "", modifiers: Qt.ControlModifier | Qt.ShiftModifier
        }), "Ctrl+Shift+v")
    }

    function test_grid_double_click_shortcut_is_explicit_and_mode_local() {
        compare(KeyboardPresentation.logicalGridKeyText({
            key: Qt.Key_Enter, text: "\r", modifiers: Qt.AltModifier
        }), "Alt+Enter")
        compare(KeyboardPresentation.logicalGridKeyText({
            key: Qt.Key_Enter, text: "\r", modifiers: Qt.NoModifier
        }), "Enter")
        compare(KeyboardPresentation.logicalGridKeyText({
            key: Qt.Key_Enter, text: "\r", modifiers: Qt.ShiftModifier
        }), "Shift+Enter")
        compare(KeyboardPresentation.logicalGridKeyText({
            key: Qt.Key_Enter, text: "\r", modifiers: Qt.ControlModifier
        }), "Ctrl+Enter")
        compare(KeyboardPresentation.logicalGridKeyText({
            key: Qt.Key_Enter, text: "\r",
            modifiers: Qt.AltModifier | Qt.ShiftModifier
        }), "")
        compare(KeyboardPresentation.logicalGridKeyText({
            key: Qt.Key_A, text: "a", modifiers: Qt.NoModifier
        }), "")
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

    function test_target_search_selection_tracks_identity_and_action_meaning() {
        var hints = [
            { label: "a", element_id: 11, related_element_id: null,
              frame_path: "0", kind: "link", text: "Documentation",
              href: "https://example.test/docs?token=private" },
            { label: "b", element_id: 22, related_element_id: null,
              frame_path: "0", kind: "button", text: "Open menu", href: null }
        ]
        var rows = TargetSearchPresentation.rows(hints, ["b", "a"])
        compare(rows[0].label, "b", "the shared matcher ranking controls result order")
        verify(rows[0].text === "Open menu")
        verify(!("href" in rows[1]), "page URLs are not projected into result rows")

        var selection = TargetSearchPresentation.reconcile(rows, {
            identity: "", meaning: "", initialized: false
        })
        compare(selection.identity, rows[0].identity)
        selection = TargetSearchPresentation.move(rows, selection, 1)
        compare(selection.identity, rows[1].identity)
        verify(!selection.meaning.includes("private"),
               "the selection signature does not retain a tokenised URL")

        var refreshed = TargetSearchPresentation.rows([
            { label: "x", element_id: 22, related_element_id: null,
              frame_path: "0", kind: "button", text: "Open menu", href: null },
            { label: "y", element_id: 11, related_element_id: null,
              frame_path: "0", kind: "link", text: "Documentation",
              href: "https://example.test/docs?token=private" }
        ], ["x", "y"])
        var preserved = TargetSearchPresentation.reconcile(refreshed, selection)
        compare(preserved.identity, selection.identity)
        compare(TargetSearchPresentation.selectedRow(refreshed, preserved).label, "y")

        var virtualReplacement = TargetSearchPresentation.rows([
            { label: "y", element_id: 99, related_element_id: null,
              frame_path: "0", kind: "link", text: "Documentation",
              href: "https://example.test/docs?token=private" }
        ], ["y"])
        var cleared = TargetSearchPresentation.reconcile(virtualReplacement, preserved)
        compare(cleared.identity, "", "a replacement at the same list position cannot inherit selection")
        verify(TargetSearchPresentation.selectedRow(virtualReplacement, cleared) === null)

        var changedMeaning = TargetSearchPresentation.rows([
            { label: "y", element_id: 11, related_element_id: null,
              frame_path: "0", kind: "link", text: "Documentation",
              href: "https://example.test/other" }
        ], ["y"])
        cleared = TargetSearchPresentation.reconcile(changedMeaning, preserved)
        compare(cleared.identity, "", "changed action meaning clears the selection")
        cleared = TargetSearchPresentation.reconcile([], preserved)
        compare(cleared.identity, "", "a filtered-out target cannot remain selected")
    }
}
