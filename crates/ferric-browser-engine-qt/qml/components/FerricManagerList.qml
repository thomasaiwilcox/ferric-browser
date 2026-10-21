import QtQuick
import QtQuick.Controls

// Forms and rows share one viewport. Keep keyboard focus visible as feedback
// and optional details change the header's height.
ListView {
    id: view
    required property var hostWindow
    property Item headerFocusItem: null
    property int headerFocusReason: Qt.OtherFocusReason
    property bool movingFocus: false
    onModelChanged: headerFocusItem = null
    onHostWindowChanged: headerFocusItem = null
    onVisibleChanged: if (!visible) headerFocusItem = null
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {
        active: true
        policy: view.contentHeight > view.height + 1 ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
    }

    function focusViewport(reason) {
        movingFocus = true
        headerFocusItem = null
        // Reactivate retained subfocus first, including after the surface
        // was hidden, so returning to browsing can clear its actual path.
        if (reason === undefined) forceActiveFocus()
        else forceActiveFocus(reason)
        var item = hostWindow ? hostWindow.activeFocusItem : null
        var ancestor = item
        while (ancestor && ancestor !== view) ancestor = ancestor.parent
        // ListView is a focus scope. Focusing it alone retains a focused
        // header/editor; clear that subfocus when returning to browsing.
        if (ancestor) {
            while (item && item !== view) {
                item.focus = false
                item = item.parent
            }
        }
        if (reason === undefined) forceActiveFocus()
        else forceActiveFocus(reason)
        movingFocus = false
    }

    function focusControl(control, reason) {
        movingFocus = true
        headerFocusItem = null
        control.forceActiveFocus(reason === undefined ? Qt.OtherFocusReason : reason)
        movingFocus = false
        preserveHeaderFocus()
    }

    function preserveHeaderFocus() {
        if (movingFocus) return
        if (!visible) { headerFocusItem = null; return }
        var item = hostWindow ? hostWindow.activeFocusItem : null
        if (!item) return
        var ancestor = item
        while (ancestor && ancestor !== headerItem) ancestor = ancestor.parent
        if (headerItem && ancestor) {
            headerFocusItem = item
            headerFocusReason = typeof item.focusReason === "number" ? item.focusReason : Qt.OtherFocusReason
            return
        }
        // Publishing rows must not interrupt a header control the user is
        // operating. Explicit browsing/action navigation clears this target.
        if ((item === view || item === currentItem || item.parent === contentItem)
                && headerFocusItem && headerFocusItem.visible && headerFocusItem.enabled) {
            movingFocus = true
            headerFocusItem.forceActiveFocus(headerFocusReason)
            movingFocus = false
        } else headerFocusItem = null
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
        function onActiveFocusItemChanged() { view.preserveHeaderFocus(); view.scheduleFocus() }
    }
}
