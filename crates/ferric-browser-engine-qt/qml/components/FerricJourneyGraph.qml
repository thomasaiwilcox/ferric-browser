import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ChromePresentation.js" as ChromePresentation
import "../scripts/ListNavigation.js" as ListNavigation

FerricManagerList {
    id: journeyGraph
    objectName: "journeyGraphList"
    required property var browserWindow
    required property var graphNodes
    required property var graphEntries
    required property var lineData
    property Component controlsHeader
    property Item navigationLayoutRoot: null
    readonly property real scale: browserWindow.chromeScale === undefined ? 1 : browserWindow.chromeScale
    readonly property real nodeWidth: Math.max(170 * scale, graphFont.averageCharacterWidth * 20)
    readonly property real nodeHeight: Math.max(48 * scale, graphFont.height * 2 + 14)
    readonly property int nodeCount: graphNodes.count === undefined ? graphNodes.length : graphNodes.count
    hostWindow: journeyGraph.Window.window
    model: graphEntries
    activeFocusOnTab: true
    currentIndex: -1
    keyNavigationEnabled: false
    Keys.onPressed: function(event) {
        var host = journeyGraph.Window.window
        var plain = event.modifiers === Qt.NoModifier
        var lastKey = event.key === Qt.Key_G && event.modifiers === Qt.ShiftModifier
        if (!host || host.activeFocusItem !== journeyGraph || (!plain && !lastKey)) return
        var delta = event.key === Qt.Key_J || event.key === Qt.Key_Down ? 40 : event.key === Qt.Key_K || event.key === Qt.Key_Up ? -40 : 0
        var page = event.key === Qt.Key_PageDown ? 1 : event.key === Qt.Key_PageUp ? -1 : 0
        var first = event.key === Qt.Key_Home || (event.key === Qt.Key_G && plain)
        var last = event.key === Qt.Key_End || lastKey
        if (!delta && !page && !first && !last) return
        var beginning = Math.min(originY, headerItem ? headerItem.y : originY)
        var atBeginning = beginningTimer.running || contentY <= beginning + 1
        beginningTimer.stop()
        ListNavigation.prepareViewport(journeyGraph, navigationLayoutRoot)
        if (atBeginning) positionViewAtBeginning()
        // A header finishing layout at the top may have queued a startup
        // reset. The user's navigation takes precedence over that reset.
        beginningTimer.stop()
        if (delta || page) ListNavigation.scrollTo(journeyGraph, contentY + (page ? page * height * 0.9 : delta))
        else if (first) positionViewAtBeginning()
        else if (count > 0) positionViewAtEnd()
        else ListNavigation.scrollTo(journeyGraph, Infinity)
        event.accepted = true
    }
    spacing: 8 * scale
    Accessible.name: "Journey relationships"
    function returnFromHeader(event) {
        var backtab = (event.key === Qt.Key_Backtab
            && (event.modifiers === Qt.NoModifier || event.modifiers === Qt.ShiftModifier))
            || (event.key === Qt.Key_Tab && event.modifiers === Qt.ShiftModifier)
        var host = journeyGraph.Window.window
        if (!backtab || !host || !headerItem
                || host.activeFocusItem !== headerItem.nextItemInFocusChain(true)) return false
        focusViewport(Qt.BacktabFocusReason)
        event.accepted = true
        return true
    }
    FontMetrics { id: graphFont; font: journeyGraph.browserWindow.font }
    function showBeginning() { beginningTimer.restart() }
    Timer { id: beginningTimer; interval: 0; onTriggered: journeyGraph.positionViewAtBeginning() }
    function requestPaint() { if (headerItem) headerItem.repaint() }
    onLineDataChanged: requestPaint()
    header: ColumnLayout {
        property real previousHeight: 0
        onHeightChanged: {
            // Preserve the top while the search header finishes laying out.
            if (journeyGraph.contentY <= -previousHeight + 1) journeyGraph.showBeginning()
            previousHeight = height
        }
        function repaint() { graphCanvas.requestPaint() }
        width: journeyGraph.width
        Loader {
            objectName: "journeyGraphControls"
            Layout.fillWidth: true
            Layout.preferredHeight: item ? item.implicitHeight : 0
            sourceComponent: journeyGraph.controlsHeader
        }
    Flickable {
        id: journeyGraphFlickable
        objectName: "journeyMapViewport"
        Layout.fillWidth: true
        Layout.preferredHeight: visible ? Math.min(220 * journeyGraph.scale,
            journeyGraphCanvas.height, Math.max(0, journeyGraph.height)) : 0
        visible: journeyGraph.nodeCount > 0
        clip: true
        contentWidth: journeyGraphCanvas.width
        contentHeight: journeyGraphCanvas.height
        Accessible.name: "Browsing journey map"
        activeFocusOnTab: true
        ScrollBar.horizontal: ScrollBar { active: true }
        ScrollBar.vertical: ScrollBar { active: true }
        Keys.onPressed: function(event) {
            var plain = event.modifiers === Qt.NoModifier
            var lastKey = event.key === Qt.Key_G && event.modifiers === Qt.ShiftModifier
            if (!plain && !lastKey) return
            var page = event.key === Qt.Key_PageDown ? 1 : event.key === Qt.Key_PageUp ? -1 : 0
            var first = event.key === Qt.Key_Home || (event.key === Qt.Key_G && plain)
            var last = event.key === Qt.Key_End || lastKey
            if (!page && !first && !last && [Qt.Key_Left, Qt.Key_Right, Qt.Key_Up, Qt.Key_Down].indexOf(event.key) < 0) return
            ListNavigation.prepareViewport(journeyGraph, journeyGraph.navigationLayoutRoot)
            journeyGraphFlickable.cancelFlick()
            if (event.key === Qt.Key_Left) contentX = Math.max(0, contentX - 80)
            else if (event.key === Qt.Key_Right) contentX = Math.min(Math.max(0, contentWidth - width), contentX + 80)
            else if (event.key === Qt.Key_Up) contentY = Math.max(0, contentY - 60)
            else if (event.key === Qt.Key_Down) contentY = Math.min(Math.max(0, contentHeight - height), contentY + 60)
            else if (page) contentY = Math.max(0, Math.min(Math.max(0, contentHeight - height), contentY + page * height * 0.9))
            else if (first) contentY = 0
            else if (last) contentY = Math.max(0, contentHeight - height)
            event.accepted = true
        }

        Item {
            id: journeyGraphCanvas
            width: Math.max(journeyGraphFlickable.width, Math.min(4, journeyGraph.nodeCount) * (journeyGraph.nodeWidth + 24))
            height: Math.max(120, Math.ceil(journeyGraph.nodeCount / 4) * (journeyGraph.nodeHeight + 18))

            Canvas {
                id: graphCanvas
                anchors.fill: parent
                z: 0
                onPaint: {
                    var context = getContext("2d")
                    context.clearRect(0, 0, width, height)
                    var colors = {
                        navigate: journeyGraph.browserWindow.borderColor,
                        redirect: journeyGraph.browserWindow.warningColor,
                        opener: journeyGraph.browserWindow.accentColor,
                        popup: journeyGraph.browserWindow.accentColor,
                        hint: journeyGraph.browserWindow.successColor,
                        "session-restore": journeyGraph.browserWindow.privateColor,
                        reopen: journeyGraph.browserWindow.warningColor,
                        "branch-after-back": journeyGraph.browserWindow.errorColor
                    }
                    var lines = journeyGraph.lineData
                    for (var i = 0; i < lines.length; ++i) {
                        var line = lines[i]
                        context.beginPath()
                        context.moveTo(line.x1, line.y1)
                        context.lineTo(line.x2, line.y2)
                        context.strokeStyle = colors[line.transition]
                                || journeyGraph.browserWindow.borderColor
                        context.lineWidth = 2
                        context.stroke()
                    }
                }
            }

            Repeater {
                model: journeyGraph.graphNodes
                delegate: Rectangle {
                    objectName: "journeyMapNode"
                    x: model.x
                    y: model.y
                    width: journeyGraph.nodeWidth
                    height: journeyGraph.nodeHeight
                    z: 1
                    color: journeyGraph.browserWindow.surfaceColor
                    border.color: journeyGraph.browserWindow.accentColor
                    Accessible.role: Accessible.ListItem
                    Accessible.name: model.label + ", " + ChromePresentation.journeyTransitionLabel(model.transition)

                    Column {
                        anchors.fill: parent
                        anchors.margins: 5
                        spacing: 2
                        Label {
                            width: parent.width
                            text: model.label
                            textFormat: Text.PlainText
                            color: journeyGraph.browserWindow.primaryTextColor
                            elide: Text.ElideRight
                        }
                        Label {
                            width: parent.width
                            text: ChromePresentation.journeyTransitionLabel(model.transition)
                            textFormat: Text.PlainText
                            color: journeyGraph.browserWindow.mutedTextColor
                            elide: Text.ElideRight
                        }
                    }
                }
            }
        }
    }

    }
    delegate: ColumnLayout {
        width: journeyGraph.width
        spacing: 4 * journeyGraph.scale
        Label {
            Layout.fillWidth: true
            text: model.label
            textFormat: Text.PlainText
            color: journeyGraph.browserWindow.primaryTextColor
            wrapMode: Text.Wrap
        }
        Label {
            Layout.fillWidth: true
            text: model.secondary
            textFormat: Text.PlainText
            color: journeyGraph.browserWindow.mutedTextColor
            wrapMode: Text.Wrap
        }
    }
}
