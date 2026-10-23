import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "CommandCompletion"
    when: windowShown
    property int submissions: 0
    property string lastSubmission: ""
    property int historyRequests: 0
    property int completionMoves: 0
    property int escapes: 0

    Component {
        id: nativeEditorFixture
        FerricTextField { width: 600; height: 40 }
    }

    ApplicationWindow {
        id: host
        width: 720
        height: 480
        visible: true
        font.family: "monospace"
        font.pointSize: 12
        FontMetrics { id: metrics; font: host.font }
        property int chromeRowHeight: Math.ceil(metrics.height + 8)
        property int inputBarHeight: chromeRowHeight
        property real chromeOpacity: 1
        property color surfaceColor: "#313244"
        property color panelColor: "#181825"
        property color primaryTextColor: "#cdd6f4"
        property color mutedTextColor: "#a6adc8"
        property color selectionColor: "#45475a"
        property color selectionTextColor: "#cdd6f4"
        property color borderColor: "#585b70"
        property color accentColor: "#89b4fa"
        property color errorColor: "#f38ba8"
        property color warningColor: "#f9e2af"
        function contrastText(color) { return "#181825" }
        function readableTextColor(color, background) { return color }
    }
    Component {
        id: fixture
        FerricCommandLine {
            browserWindow: host
            commandVisible: true
            completionVisible: true
            completionText: ""
            completionValues: ""
            completionStart: 0
            completionEnd: 2
            completionSelected: -1
            property bool refreshAfterEdit: false
            property string historyResponse: ""
            onHistoryMoveRequested: {
                test.historyRequests++
                recallHistory(historyResponse)
            }
            onSubmitted: function(text) { test.submissions++; test.lastSubmission = text }
            onEscapeRequested: test.escapes++
            onCompletionUpdateRequested: function(text, cursor) {
                if (!refreshAfterEdit) return
                // The production bridge replaces candidates and the token
                // range synchronously when applying a completion edits text.
                completionValues = "refreshed"
                completionText = "value\tRefreshed\tNew candidate set"
                completionStart = 0
                completionEnd = cursor
                completionSelected = -1
            }
            onCompletionMoveRequested: function(delta) {
                test.completionMoves++
                var count = completionValues.split("\n").length
                completionSelected = completionSelected < 0
                    ? (delta < 0 ? count - 1 : 0)
                    : (completionSelected + delta + count) % count
            }
            onCompletionSelectRequested: function(index) { completionSelected = index }
        }
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width = 720
        host.height = 480
        host.font.pointSize = 12
        submissions = 0
        lastSubmission = ""
        historyRequests = 0
        completionMoves = 0
        escapes = 0
        host.requestActivate()
        tryCompare(host, "active", true)
    }
    function create() {
        var command = createTemporaryObject(fixture, host.contentItem)
        verify(command)
        var labels = [], values = []
        for (var i = 0; i < 32; ++i) {
            labels.push("command\tCommand " + i + "\tDescription for command " + i)
            values.push("command-" + i)
        }
        command.completionText = labels.join("\n")
        command.completionValues = values.join("\n")
        command.commandText = "co"
        command.focusInput()
        tryCompare(findChild(command, "commandLineInput"), "activeFocus", true)
        verify(waitForPolish(host))
        return command
    }
    function listFor(command) {
        return findChild(command, "commandCompletionList")
    }
    function selectedIsVisible(command) {
        var list = listFor(command)
        var row = list.itemAtIndex(command.completionSelected)
        if (!row) return false
        var top = row.mapToItem(list, 0, 0).y
        return top >= -1 && top + row.height <= list.height + 1
    }
    function test_keyboard_selection_scrolls_and_wraps() {
        var command = create()
        for (var i = 0; i < 25; ++i) keyClick(Qt.Key_Tab)
        compare(command.completionSelected, 24)
        tryVerify(function() { return selectedIsVisible(command) })
        compare(findChild(command, "commandLineInput").activeFocus, true)
        compare(command.commandText, "co")
        for (i = 0; i < 8; ++i) keyClick(Qt.Key_N, Qt.ControlModifier)
        compare(command.completionSelected, 0)
        tryVerify(function() { return selectedIsVisible(command) })
        keyClick(Qt.Key_P, Qt.ControlModifier)
        compare(command.completionSelected, 31)
        tryVerify(function() { return selectedIsVisible(command) })
        host.height = 320
        tryVerify(function() { return selectedIsVisible(command) })
        if (testArtifactDirectory.length > 0) {
            wait(100)
            verify(waitForRendering(host.contentItem))
            grabImage(host.contentItem).save(testArtifactDirectory + "/command-completion.png")
        }
    }
    function test_selection_survives_a_same_count_completion_refresh() {
        var command = create()
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        compare(command.completionSelected, 31)
        tryVerify(function() { return selectedIsVisible(command) })
        command.completionText = command.completionText.replace(/Description/g, "Updated description")
        verify(waitForPolish(host))
        tryCompare(listFor(command), "currentIndex", 31)
        tryVerify(function() { return selectedIsVisible(command) })
    }
    function test_backtab_selects_previous_without_leaving_input_data() {
        return [{tag: "backtab", key: Qt.Key_Backtab}, {tag: "shift-tab", key: Qt.Key_Tab}]
    }
    function test_backtab_selects_previous_without_leaving_input(data) {
        var command = create()
        keyClick(data.key, Qt.ShiftModifier)
        compare(command.completionSelected, 31)
        compare(findChild(command, "commandLineInput").activeFocus, true)
        tryVerify(function() { return selectedIsVisible(command) })
    }
    function test_completion_respects_configured_font_without_clipping() {
        host.font.pointSize = 26
        var command = create()
        var list = listFor(command)
        tryVerify(function() { return list.itemAtIndex(0) !== null })
        var row = list.itemAtIndex(0)
        var textItems = row.children.filter(function(child) { return child.textFormat !== undefined })
        compare(textItems.length, 2)
        for (var i = 0; i < textItems.length; ++i) {
            compare(textItems[i].font.pointSize, host.font.pointSize)
            verify(textItems[i].y + textItems[i].implicitHeight <= row.height,
                "Completion text extends below its row")
        }
        verify(textItems[0].y + textItems[0].implicitHeight <= textItems[1].y,
            "Completion title overlaps its description")
    }
    function test_completion_edit_survives_a_synchronous_candidate_refresh() {
        var command = create()
        command.commandText = "open ex --target tab"
        command.completionValues = "https://first.test\nhttps://example.test/🦀"
        command.completionStart = 5
        command.completionEnd = 7
        command.completionSelected = 1
        command.refreshAfterEdit = true
        command.applyCompletion(1)
        compare(command.commandText, "open https://example.test/🦀 --target tab")
        compare(command.cursorPosition, "open https://example.test/🦀".length)
    }
    function test_mouse_completion_survives_delegate_refresh() {
        var command = create()
        var row = listFor(command).itemAtIndex(1)
        verify(row)
        host.contentItem.forceActiveFocus()
        compare(findChild(command, "commandLineInput").activeFocus, false)
        command.refreshAfterEdit = true
        mouseClick(row, row.width / 2, row.height / 2)
        compare(command.commandText, "command-1")
        compare(command.cursorPosition, "command-1".length)
        compare(findChild(command, "commandLineInput").activeFocus, true)
    }
    function test_keyboard_inserts_completion_without_running_data() {
        return [{tag: "right", key: Qt.Key_Right, suffix: ""},
                {tag: "space", key: Qt.Key_Space, suffix: " "}]
    }
    function test_keyboard_inserts_completion_without_running(data) {
        var command = create()
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_Tab)
        compare(command.completionSelected, 1)
        command.refreshAfterEdit = true
        keyClick(data.key)
        compare(command.commandText, "command-1" + data.suffix)
        compare(command.cursorPosition, command.commandText.length)
        compare(submissions, 0)
        if (data.key === Qt.Key_Right) keyClick(Qt.Key_Space)
        keyClick(Qt.Key_X)
        keyClick(Qt.Key_Return)
        compare(submissions, 1)
        compare(lastSubmission, "command-1 x")
    }
    function test_completion_keeps_ordinary_cursor_editing_data() {
        return [{tag: "middle", cursor: 0, key: Qt.Key_Right, modifiers: Qt.NoModifier, position: 1},
                {tag: "selection", cursor: 2, select: true, key: Qt.Key_Right, modifiers: Qt.NoModifier, position: 2},
                {tag: "shift", cursor: 0, key: Qt.Key_Right, modifiers: Qt.ShiftModifier, position: 1},
                {tag: "space-in-middle", cursor: 1, key: Qt.Key_Space, modifiers: Qt.NoModifier, position: 2, text: "c o"}]
    }
    function test_completion_keeps_ordinary_cursor_editing(data) {
        var command = create()
        var input = findChild(command, "commandLineInput")
        command.completionSelected = 1
        input.cursorPosition = data.cursor
        if (data.select) input.selectAll()
        keyClick(data.key, data.modifiers)
        compare(command.commandText, data.text || "co")
        compare(input.cursorPosition, data.position)
        compare(submissions, 0)
    }
    function test_modified_arrows_follow_the_native_editor_data() {
        var rows = []
        var modifiers = [{name: "shift", value: Qt.ShiftModifier},
            {name: "control", value: Qt.ControlModifier},
            {name: "control-shift", value: Qt.ControlModifier | Qt.ShiftModifier},
            {name: "alt", value: Qt.AltModifier},
            {name: "keypad-shift", value: Qt.KeypadModifier | Qt.ShiftModifier}]
        for (var i = 0; i < modifiers.length; ++i)
            for (var direction = 0; direction < 2; ++direction)
                rows.push({tag: modifiers[i].name + (direction ? "-down" : "-up"),
                    key: direction ? Qt.Key_Down : Qt.Key_Up, modifiers: modifiers[i].value})
        return rows
    }
    function test_modified_arrows_follow_the_native_editor(data) {
        var native = createTemporaryObject(nativeEditorFixture, host.contentItem)
        verify(native)
        native.text = "open two words"
        native.cursorPosition = 7
        native.forceActiveFocus()
        tryCompare(native, "activeFocus", true)
        keyClick(data.key, data.modifiers)
        var expected = {text: native.text, cursor: native.cursorPosition,
            selected: native.selectedText}

        var command = create()
        var input = findChild(command, "commandLineInput")
        command.commandText = "open two words"
        command.cursorPosition = 7
        command.historyResponse = "zoom 1.2"
        keyClick(data.key, data.modifiers)
        compare(command.commandText, expected.text)
        compare(command.cursorPosition, expected.cursor)
        compare(input.selectedText, expected.selected)
        compare(historyRequests, 0)
        compare(submissions, 0)
    }
    function test_other_modified_keys_do_not_cycle_completions_data() {
        return [{tag: "control-shift-n", key: Qt.Key_N, modifiers: Qt.ControlModifier | Qt.ShiftModifier},
            {tag: "control-shift-p", key: Qt.Key_P, modifiers: Qt.ControlModifier | Qt.ShiftModifier},
            {tag: "control-alt-n", key: Qt.Key_N, modifiers: Qt.ControlModifier | Qt.AltModifier},
            {tag: "control-alt-p", key: Qt.Key_P, modifiers: Qt.ControlModifier | Qt.AltModifier},
            {tag: "control-tab", key: Qt.Key_Tab, modifiers: Qt.ControlModifier},
            {tag: "control-shift-tab", key: Qt.Key_Backtab, modifiers: Qt.ControlModifier | Qt.ShiftModifier}]
    }
    function test_other_modified_keys_do_not_cycle_completions(data) {
        var command = create()
        command.completionSelected = 1
        keyClick(data.key, data.modifiers)
        compare(completionMoves, 0)
        compare(historyRequests, 0)
        compare(submissions, 0)
    }
    function test_keypad_arrows_keep_browser_navigation_data() {
        return [{tag: "up", key: Qt.Key_Up}, {tag: "down", key: Qt.Key_Down},
            {tag: "right", key: Qt.Key_Right}]
    }
    function test_keypad_arrows_keep_browser_navigation(data) {
        var command = create()
        command.completionSelected = 1
        command.historyResponse = "zoom 1.2"
        keyClick(data.key, Qt.KeypadModifier)
        compare(command.commandText, data.key === Qt.Key_Right ? "command-1" : "zoom 1.2")
        compare(historyRequests, data.key === Qt.Key_Right ? 0 : 1)
        compare(submissions, 0)
    }
    function test_preedit_keeps_browser_actions_inactive_data() {
        return [{tag: "right", key: Qt.Key_Right}, {tag: "space", key: Qt.Key_Space},
            {tag: "up", key: Qt.Key_Up}, {tag: "down", key: Qt.Key_Down},
            {tag: "tab", key: Qt.Key_Tab},
            {tag: "backtab", key: Qt.Key_Backtab, modifiers: Qt.ShiftModifier},
            {tag: "control-n", key: Qt.Key_N, modifiers: Qt.ControlModifier},
            {tag: "control-p", key: Qt.Key_P, modifiers: Qt.ControlModifier},
            {tag: "escape", key: Qt.Key_Escape}, {tag: "return", key: Qt.Key_Return}]
    }
    function test_preedit_keeps_browser_actions_inactive(data) {
        var command = create()
        var input = findChild(command, "commandLineInput")
        command.completionSelected = 1
        command.historyResponse = "zoom 1.2"
        testInputMethod.sendInputMethod(input, "にほ", "")
        tryCompare(input, "inputMethodComposing", true)
        keyClick(data.key, data.modifiers || Qt.NoModifier)
        compare(historyRequests, 0)
        compare(completionMoves, 0)
        compare(submissions, 0)
        compare(escapes, 0)
        command.focusInput()
        testInputMethod.sendInputMethod(input, "", "日本")
        tryCompare(input, "inputMethodComposing", false)
        verify(command.commandText.indexOf("日本") >= 0)
        command.completionSelected = -1
        var committed = command.commandText
        keyClick(Qt.Key_Return)
        compare(submissions, 1)
        compare(lastSubmission, committed)
    }
    function test_completion_is_one_undoable_edit_data() {
        return [{tag: "typed-command", typed: "co", start: 0, end: 2, expected: "command-1"},
                {tag: "empty-argument", typed: "open ", start: 5, end: 5, expected: "open command-1"},
                {tag: "cleared-input", typed: "co", cleared: true, start: 0, end: 0, expected: "command-1"}]
    }
    function test_completion_is_one_undoable_edit(data) {
        var command = create()
        command.clearInput()
        for (var i = 0; i < data.typed.length; ++i) keyClick(data.typed[i])
        if (data.cleared) {
            keyClick(Qt.Key_Backspace)
            keyClick(Qt.Key_Backspace)
        }
        var original = command.commandText
        command.completionStart = data.start
        command.completionEnd = data.end
        command.completionSelected = 1
        command.refreshAfterEdit = true
        keyClick(Qt.Key_Right)
        compare(command.commandText, data.expected)
        keyClick(Qt.Key_Z, Qt.ControlModifier)
        compare(command.commandText, original)
        compare(command.cursorPosition, original.length)
        compare(findChild(command, "commandLineInput").selectedText, "")
        keyClick(Qt.Key_Z, Qt.ControlModifier | Qt.ShiftModifier)
        compare(command.commandText, data.expected)
    }
    function test_feedback_prevents_inserting_a_hidden_suggestion() {
        var command = create()
        command.completionSelected = 1
        command.showFeedback("Check this command", true)
        keyClick(Qt.Key_Right)
        compare(command.commandText, "co")
        compare(command.feedbackText, "Check this command")
    }
    function test_history_is_one_undoable_edit_data() {
        return [{tag: "draft", typed: "open draft", recalled: "zoom 120"},
                {tag: "empty-draft", typed: "", recalled: "zoom 120"},
                {tag: "return-to-empty-draft", typed: "zoom 120", recalled: ""},
                {tag: "unicode-history", typed: "open draft", recalled: "open https://example.test/🦀/λ"},
                {tag: "selected-draft", typed: "open draft", recalled: "zoom 120", selected: true}]
    }
    function test_history_is_one_undoable_edit(data) {
        var command = create()
        command.clearInput()
        for (var i = 0; i < data.typed.length; ++i) keyClick(data.typed[i])
        var input = findChild(command, "commandLineInput")
        if (data.selected) input.selectAll()
        command.historyResponse = data.recalled
        command.refreshAfterEdit = true
        keyClick(Qt.Key_Up)
        compare(command.commandText, data.recalled)
        compare(command.cursorPosition, data.recalled.length)
        compare(submissions, 0)
        keyClick(Qt.Key_Z, Qt.ControlModifier)
        compare(command.commandText, data.typed)
        keyClick(Qt.Key_Z, Qt.ControlModifier | Qt.ShiftModifier)
        compare(command.commandText, data.recalled)
        compare(input.activeFocus, true)
    }
    function test_history_boundary_preserves_selection_and_undo() {
        var command = create()
        command.clearInput()
        keyClick("d"); keyClick("r"); keyClick("a"); keyClick("f"); keyClick("t")
        command.historyResponse = command.commandText
        var input = findChild(command, "commandLineInput")
        input.select(1, 4)
        keyClick(Qt.Key_Down)
        compare(command.commandText, "draft")
        compare(input.selectedText, "raf")
        compare(input.cursorPosition, 4)
        keyClick(Qt.Key_Z, Qt.ControlModifier)
        verify(command.commandText !== "draft")
    }
    function test_help_fits_narrow_large_font_popup() {
        host.width = 390
        host.height = 320
        host.font.pointSize = 26
        var command = create()
        command.completionSelected = 31
        tryVerify(function() { return selectedIsVisible(command) })
        var help = findChild(command, "commandCompletionHelp")
        verify(help)
        compare(help.visible, true)
        compare(help.font.pointSize, 26)
        compare(help.truncated, false)
        compare(help.textFormat, Text.PlainText)
        var title = findChild(listFor(command).currentItem, "commandCompletionTitle")
        verify(title)
        compare(title.text, "Command 31")
        compare(title.truncated, false)
        if (testArtifactDirectory.length > 0) {
            wait(100)
            verify(waitForRendering(host.contentItem))
            grabImage(host.contentItem).save(testArtifactDirectory + "/completion-help-large.png")
        }
    }
}
