import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
import "../../qml/scripts/ChromePresentation.js" as ChromePresentation
import "../../qml/scripts/KeyboardPresentation.js" as KeyboardPresentation

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
        settingsScroll.contentItem.contentY = settingsScroll.contentHeight
                                             - settingsScroll.height
        tryVerify(function() {
            return settingsRows.mapToItem(settingsSurface, 0, 0).y
                   < settingsSurface.height
        })
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
}
