import QtQuick
import QtQuick.Controls

// Forms and rows share one viewport. Keep keyboard focus visible as feedback
// and optional details change the header's height.
ListView {
    id: view
    required property var hostWindow
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {
        active: true
        policy: view.contentHeight > view.height + 1 ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
    }

    function revealFocusedControl() {
        if (!visible || height <= 0) return
        var item = hostWindow ? hostWindow.activeFocusItem : null
        var ancestor = item
        while (ancestor && ancestor !== contentItem) ancestor = ancestor.parent
        if (!ancestor) return
        var top = item.mapToItem(contentItem, 0, 0).y
        if (top >= contentY && top + item.height <= contentY + height) return
        if (item.height > height && top < contentY && top + item.height > contentY + height) return
        var next = item.height >= height ? top : top < contentY ? top - 8 : top + item.height - height + 8
        var beginning = Math.min(originY, headerItem ? headerItem.y : originY)
        contentY = Math.max(beginning, Math.min(Math.max(beginning, beginning + contentHeight - height), next))
    }
    function scheduleFocus() { if (focusTimer) focusTimer.restart() }
    onContentHeightChanged: scheduleFocus()
    onHeightChanged: scheduleFocus()
    Timer { id: focusTimer; interval: 0; onTriggered: view.revealFocusedControl() }
    Connections {
        target: view.hostWindow
        ignoreUnknownSignals: true
        function onActiveFocusItemChanged() { view.scheduleFocus() }
    }
}
