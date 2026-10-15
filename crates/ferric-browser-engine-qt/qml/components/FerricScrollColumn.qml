import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// A form-sized viewport: text keeps its natural height and focused controls
// remain reachable when a narrow window or larger font needs more room.
ScrollView {
    id: scroll
    default property alias items: content.data
    property var hostWindow: null
    spacing: 8
    readonly property alias contentLayout: content
    implicitWidth: 0
    implicitHeight: 0
    clip: true
    contentWidth: Math.max(0, availableWidth - effectiveScrollBarWidth)
    contentHeight: Math.max(content.implicitHeight, availableHeight)
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    ScrollBar.vertical.active: true

    function reveal(item) {
        var ancestor = item;
        while (ancestor && ancestor !== content)
            ancestor = ancestor.parent;
        var viewport = contentItem;
        if (!ancestor || !viewport || viewport.height <= 0)
            return;
        var top = item.mapToItem(content, 0, 0).y;
        var targetHeight = item.height;
        // Text inspection can span many viewports. Follow the keyboard caret,
        // rather than treating the entire text area as the focused rectangle.
        var cursor = item.cursorRectangle;
        if (targetHeight > viewport.height && cursor && cursor.height > 0) {
            top += cursor.y;
            targetHeight = cursor.height;
        }
        var bottom = top + targetHeight;
        if (top >= viewport.contentY && bottom <= viewport.contentY + viewport.height)
            return;
        if (targetHeight > viewport.height && top < viewport.contentY && bottom > viewport.contentY
                + viewport.height)
            return;
        var next = top;
        if (targetHeight < viewport.height) {
            next = top < viewport.contentY ? top - 8 : bottom - viewport.height + 8;
        }
        viewport.contentY = Math.max(0, Math.min(Math.max(0, viewport.contentHeight - viewport.height),
                                                 next));
    }
    function scheduleFocus() {
        revealTimer.restart();
    }
    onAvailableHeightChanged: scheduleFocus()
    Timer {
        id: revealTimer
        interval: 0
        onTriggered: scroll.reveal(scroll.hostWindow ? scroll.hostWindow.activeFocusItem : null)
    }
    Connections {
        target: scroll.hostWindow
        ignoreUnknownSignals: true
        function onActiveFocusItemChanged() {
            scroll.scheduleFocus();
        }
    }
    Connections {
        target: scroll.hostWindow ? scroll.hostWindow.activeFocusItem : null
        ignoreUnknownSignals: true
        function onCursorRectangleChanged() { scroll.scheduleFocus(); }
    }
    ColumnLayout {
        id: content
        width: scroll.contentWidth
        height: scroll.contentHeight
        spacing: scroll.spacing
        onImplicitHeightChanged: scroll.scheduleFocus()
    }
}
