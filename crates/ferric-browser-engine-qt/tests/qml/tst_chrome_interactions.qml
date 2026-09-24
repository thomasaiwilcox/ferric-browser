import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
import "../../qml/scripts/ChromePresentation.js" as ChromePresentation

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
    property string appliedSettingValue: ""
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
                completionSelected: 0
            }
        }
    }

    Component {
        id: contextMenuFixture
        FerricContextMenu {
            browserWindow: theme
            onScopeSelected: function(scope) { theme.switcherScope = scope }
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
        selectedTabs = 0
        closedTabs = 0
        activatedResults = 0
        invokedActions = 0
        appliedSettingValue = ""
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

    function test_context_scope_fits_and_selects() {
        var menu = createTemporaryObject(contextMenuFixture, testWindow.contentItem)
        verify(menu)
        menu.open()
        var selector = findChild(menu, "scopeSelector")
        verify(selector)
        compare(selector.count, 11)
        compare(selector.currentText, "all")
        verify(selector.width <= 420)
        mouseClick(selector)
        tryCompare(selector.popup, "visible", true)
        var scopeRow = selector.popup.contentItem.itemAtIndex(1)
        verify(scopeRow)
        mouseClick(scopeRow)
        tryCompare(theme, "switcherScope", "tabs")
        menu.close()
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
}
