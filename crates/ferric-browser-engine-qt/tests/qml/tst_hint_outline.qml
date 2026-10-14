import QtQuick
import QtTest
import "../../qml/components" as Ferric

TestCase {
    name: "HintOutline"
    when: windowShown
    width: 640
    height: 480
    QtObject {
        id: palette
        property color warningColor: "yellow"
        property color primaryTextColor: "white"
        property color backgroundColor: "black"
        function contrastText(color) { return "black" }
    }
    Ferric.FerricHintOverlay {
        id: overlay
        browserWindow: palette
        hintsVisible: true
        hintResults: []
        sourceViewport: ({width: 640, height: 480})
        hintState: ({mode: "label", matching_labels: [], active_label: ""})
        unmatchedPolicy: "show"
        markerScale: 1
    }
    function test_only_selected_target_has_outline() {
        var rows = []
        for (var index = 0; index < 500; ++index) {
            rows.push({label: String(index), text: "Target " + index,
                       x: index % 20 * 30, y: Math.floor(index / 20) * 16,
                       width: 20, height: 12})
        }
        overlay.hintResults = rows
        overlay.hintState = {mode: "label", matching_labels: ["0"], active_label: "0"}
        var outlines = findChild(overlay, "activeHintOutlineRepeater")
        verify(outlines !== null)
        tryCompare(outlines, "count", 1)
        compare(outlines.itemAt(0).x, -2)
        compare(outlines.itemAt(0).y, -2)
        overlay.hintState = {mode: "label", matching_labels: ["499"], active_label: "499"}
        tryCompare(outlines, "count", 1)
        tryCompare(outlines.itemAt(0), "x", rows[499].x - 2)
        tryCompare(outlines.itemAt(0), "y", rows[499].y - 2)
        overlay.hintState = {mode: "label", matching_labels: [], active_label: "absent"}
        tryCompare(outlines, "count", 0)
        overlay.hintResults = []
        tryCompare(outlines, "count", 0)
    }
}
