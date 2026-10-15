import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: testCase
    name: "ClipboardFields"
    when: windowShown
    visible: true
    width: 500
    height: 250
    FerricTextField { id: first; width: 250; y: 0 }
    FerricTextField { id: second; width: 250; y: 60 }

    function test_plainPasteFollowsNativeFocus() {
        first.forceActiveFocus()
        first.text = "plain-paste-fixture"
        first.selectAll()
        first.copy()
        first.text = ""
        keyClick(Qt.Key_V, Qt.ControlModifier | Qt.ShiftModifier)
        compare(first.text, "plain-paste-fixture")
        second.forceActiveFocus()
        second.text = ""
        keyClick(Qt.Key_V, Qt.ControlModifier | Qt.ShiftModifier)
        compare(second.text, "plain-paste-fixture")
        compare(first.text, "plain-paste-fixture")
    }
    function test_readOnlyDoesNotPaste() {
        first.forceActiveFocus()
        first.text = "sentinel"
        first.readOnly = true
        keyClick(Qt.Key_V, Qt.ControlModifier | Qt.ShiftModifier)
        compare(first.text, "sentinel")
        first.readOnly = false
    }
}
