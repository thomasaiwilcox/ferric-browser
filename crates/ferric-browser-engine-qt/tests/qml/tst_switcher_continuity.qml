import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "SwitcherContinuity"
    when: windowShown
    property string lastAction: ""
    property int lastIndex: -1
    property int activationCount: 0
    property int refreshCount: 0

    ApplicationWindow {
        id: host
        width: 900
        height: 700
        visible: true
        font.family: "monospace"
        font.pointSize: 20
        FontMetrics { id: metrics; font: host.font }
        property real chromeScale: Math.max(1, metrics.height / 14)
        property real chromeRowHeight: Math.ceil(metrics.height + Math.max(4, Math.ceil(metrics.height * 0.25)) * 1.25)
        property color panelColor: "#181825"
        property color primaryTextColor: "#cdd6f4"
        property color secondaryTextColor: "#bac2de"
        property color mutedTextColor: "#a6adc8"
        property color errorColor: "#f38ba8"
        property color surfaceColor: "#1e1e2e"
        property color borderColor: "#585b70"
        property color accentColor: "#89b4fa"
        property color selectionColor: "#45475a"
        function switcherMaxResults() { return 200 }
        color: panelColor
        palette.window: panelColor
        palette.windowText: primaryTextColor
        palette.text: primaryTextColor
        palette.base: panelColor
        palette.button: panelColor
        palette.buttonText: primaryTextColor
        palette.placeholderText: mutedTextColor
        palette.highlight: accentColor
    }
    Component {
        id: fixture
        FerricSwitcher {
            browserWindow: host
            switcherVisible: true
            scope: "contexts"
            results: [{kind: "context",
                label: "Before work <b>Research</b> - notes and reference material with a long label",
                profile: "default", secondary: "work", actions: ["enter"]}]
            onRefreshRequested: test.refreshCount++
            onActionRequested: function(index, action) {
                test.lastIndex = index
                test.lastAction = action
            }
            onActivationRequested: function(index) {
                test.lastIndex = index
                test.activationCount++
            }
        }
    }

    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width = 900
        host.height = 700
        host.panelColor = "#181825"
        host.primaryTextColor = "#cdd6f4"
        host.borderColor = "#585b70"
        host.requestActivate()
        tryCompare(host, "active", true)
        lastAction = ""
        lastIndex = -1
        activationCount = 0
        refreshCount = 0
    }
    function create(properties) {
        var surface = createTemporaryObject(fixture, host.contentItem, properties || {})
        verify(surface)
        verify(waitForPolish(host))
        wait(0)
        tryVerify(function() { return findChild(surface, "switcherSearchInput").activeFocus })
        return surface
    }
    function inside(item, viewport) {
        var position = item.mapToItem(viewport, 0, 0)
        return position.x >= -1 && position.y >= -1
            && position.x + item.width <= viewport.width + 1
            && position.y + item.height <= viewport.height + 1
    }
    function buttons(row) {
        var matches = []
        function visit(item) {
            if (item.objectName === "switcherActionButton") matches.push(item)
            for (var i = 0; i < item.children.length; ++i) visit(item.children[i])
        }
        visit(row)
        return matches
    }
    function row(id, label) {
        return {kind: "tab", id: id, generation: "1", owner_token: "owned-window",
            label: label, profile: "default", actions: ["focus", "open"]}
    }
    function test_keyboard_selection_tracks_the_same_result_after_refresh() {
        var alpha = row("alpha", "Alpha")
        var beta = row("beta", "Beta")
        var surface = create({scope: "tabs", results: [alpha, beta]})
        var list = findChild(surface, "switcherResultList")
        keyClick(Qt.Key_Down)
        tryCompare(list, "currentIndex", 1)
        surface.results = [row("new", "New result"), alpha, beta]
        tryCompare(list, "currentIndex", 2)
        keyClick(Qt.Key_Return)
        compare(lastIndex, 2)
        compare(activationCount, 1)
    }
    function test_focused_action_tracks_identity_after_refresh() {
        var alpha = row("alpha", "Alpha")
        var beta = row("beta", "Beta")
        var surface = create({scope: "tabs", results: [alpha, beta]})
        var list = findChild(surface, "switcherResultList")
        tryVerify(function() { return list.itemAtIndex(1) !== null })
        var action = buttons(list.itemAtIndex(1))[1]
        action.forceActiveFocus(Qt.TabFocusReason)
        tryCompare(action, "activeFocus", true)
        surface.results = [row("new", "New result"), alpha, beta]
        tryVerify(function() {
            var focus = host.activeFocusItem
            return list.currentIndex === 2 && focus
                && focus.objectName === "switcherActionButton"
                && focus.Accessible.name === "Open · Beta"
        })
        keyClick(Qt.Key_Return)
        compare(lastAction, "open")
        compare(lastIndex, 2)
        compare(activationCount, 0)
    }
    function test_pointer_press_cannot_retarget_after_replacement() {
        var surface = create({scope: "tabs", results: [row("alpha", "Alpha")]})
        var list = findChild(surface, "switcherResultList")
        tryVerify(function() { return list.itemAtIndex(0) !== null })
        var action = buttons(list.itemAtIndex(0))[1]
        var point = action.mapToItem(surface, action.width / 2, action.height / 2)
        mousePress(surface, point.x, point.y)
        surface.results = [row("beta", "Beta")]
        verify(waitForPolish(host))
        mouseRelease(surface, point.x, point.y)
        compare(lastAction, "", "a replaced target must not inherit a pending click")
        compare(activationCount, 0)
    }
    function test_pressed_action_cannot_cross_a_result_generation_data() {
        return [{tag: "mouse", keyboard: false}, {tag: "keyboard", keyboard: true}]
    }
    function test_pressed_action_cannot_cross_a_result_generation(data) {
        var surface = create({scope: "tabs", results: [row("alpha", "Alpha")]})
        var list = findChild(surface, "switcherResultList")
        tryVerify(function() { return list.itemAtIndex(0) !== null })
        var action = buttons(list.itemAtIndex(0))[1]
        action.forceActiveFocus(Qt.TabFocusReason)
        tryCompare(action, "activeFocus", true)
        var point = action.mapToItem(surface, action.width / 2, action.height / 2)
        if (data.keyboard) keyPress(Qt.Key_Space)
        else mousePress(surface, point.x, point.y)
        tryCompare(action, "down", true)
        var replacement = row("alpha", "Alpha changed")
        replacement.generation = "2"
        surface.results = [replacement]
        verify(waitForPolish(host))
        // The action delegate stays alive for the same identity. Its pending
        // activation must still retain the version captured at press time.
        compare(buttons(list.itemAtIndex(0))[1], action)
        if (data.keyboard) keyRelease(Qt.Key_Space)
        else mouseRelease(surface, point.x, point.y)
        compare(lastAction, "")
        compare(activationCount, 0)
        action.forceActiveFocus(Qt.TabFocusReason)
        keyClick(Qt.Key_Return)
        compare(lastAction, "open")
        compare(lastIndex, 0)
    }

}
