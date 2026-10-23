import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "SearchEditor"
    when: windowShown
    property int accepts: 0
    property int escapes: 0
    ApplicationWindow {
        id: host
        width: 720; height: 480; visible: true
        font.family: "monospace"
        font.pointSize: 12
        FontMetrics { id: metrics; font: host.font }
        property int inputBarHeight: Math.ceil(metrics.height + 8)
        property real chromeOpacity: 1
        property color surfaceColor: "#313244"
        property color panelColor: "#181825"
        property color primaryTextColor: "#cdd6f4"
        property color mutedTextColor: "#a6adc8"
        property color selectionColor: "#45475a"
        property color selectionTextColor: "#cdd6f4"
        property color warningColor: "#f9e2af"
        function contrastText(color) { return "#181825" }
    }
    Component {
        id: fixture
        FerricSearchBar {
            browserWindow: host
            searchVisible: true
            searchBackward: false
            searchText: "needle "
            onSearchChanged: function(text) { searchText = text }
            onAccepted: test.accepts++
            onEscapeRequested: test.escapes++
        }
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        accepts=0; escapes=0
        host.width=720;host.font.pointSize=12
        host.requestActivate()
        tryCompare(host,"active",true)
    }
    function test_preedit_owns_acceptance_and_escape_data() {
        return [{tag:"return",key:Qt.Key_Return}, {tag:"escape",key:Qt.Key_Escape},
            {tag:"keypad-enter",key:Qt.Key_Enter,modifiers:Qt.KeypadModifier}]
    }
    function test_navigation_symbols_follow_the_editor_font_data() {
        return [{tag:"regular",points:12,width:720},{tag:"large-narrow",points:26,width:320}]
    }
    function test_navigation_symbols_follow_the_editor_font(data) {
        host.width=data.width;host.font.pointSize=data.points
        var bar=createTemporaryObject(fixture,host.contentItem)
        verify(waitForPolish(host))
        var buttons=Array.from(bar.children).filter(function(child){return typeof child.clicked==="function" && !!child.contentItem})
        compare(buttons.length,2)
        for (var button of buttons) {
            compare(button.contentItem.font.pointSize,host.font.pointSize)
            verify(button.width>=host.inputBarHeight)
        }
    }
    function test_preedit_owns_acceptance_and_escape(data) {
        var bar=createTemporaryObject(fixture,host.contentItem)
        bar.focusInput()
        tryVerify(function(){return host.activeFocusItem
            && host.activeFocusItem.inputMethodComposing!==undefined})
        var input=host.activeFocusItem
        verify(input && input.inputMethodComposing!==undefined)
        input.cursorPosition=input.text.length
        testInputMethod.sendInputMethod(input,"にほ","")
        tryCompare(input,"inputMethodComposing",true)
        keyClick(data.key,data.modifiers || Qt.NoModifier)
        compare(accepts,0)
        compare(escapes,0)
        bar.focusInput()
        testInputMethod.sendInputMethod(input,"","日本")
        tryCompare(input,"inputMethodComposing",false)
        verify(bar.searchText.indexOf("日本")>=0)
        keyClick(data.key,data.modifiers || Qt.NoModifier)
        compare(accepts,data.key===Qt.Key_Escape?0:1)
        compare(escapes,data.key===Qt.Key_Escape?1:0)
    }
}
