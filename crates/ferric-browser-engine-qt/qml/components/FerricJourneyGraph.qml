import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ChromePresentation.js" as ChromePresentation

FerricManagerList {
    id: journeyGraph
    objectName: "journeyGraphList"
    required property var browserWindow
    required property var graphNodes
    required property var graphEntries
    required property var lineData
    property Component controlsHeader
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
        if (!host || host.activeFocusItem !== journeyGraph || event.modifiers !== Qt.NoModifier) return
        var delta = event.key === Qt.Key_J || event.key === Qt.Key_Down ? 40 : event.key === Qt.Key_K || event.key === Qt.Key_Up ? -40 : 0
        if (!delta) return
        var beginning = headerItem ? headerItem.y : originY
        contentY = Math.max(beginning, Math.min(Math.max(beginning, beginning + contentHeight - height), contentY + delta))
        event.accepted = true
    }
    spacing: 8 * scale
    Accessible.name: "Journey relationships"
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
        Layout.fillWidth: true
        Layout.preferredHeight: visible ? 220 * journeyGraph.scale : 0
        visible: journeyGraph.nodeCount > 0
        clip: true
        contentWidth: journeyGraphCanvas.width
        contentHeight: journeyGraphCanvas.height
        Accessible.name: "Browsing journey map"
        activeFocusOnTab: true
        ScrollBar.horizontal: ScrollBar { active: true }
        ScrollBar.vertical: ScrollBar { active: true }
        Keys.onPressed: function(event) {
            if (event.modifiers !== Qt.NoModifier) return
            if (event.key === Qt.Key_Left) contentX = Math.max(0, contentX - 80)
            else if (event.key === Qt.Key_Right) contentX = Math.min(Math.max(0, contentWidth - width), contentX + 80)
            else if (event.key === Qt.Key_Up) contentY = Math.max(0, contentY - 60)
            else if (event.key === Qt.Key_Down) contentY = Math.min(Math.max(0, contentHeight - height), contentY + 60)
            else return
            event.accepted = true
        }

        Item {
            id: journeyGraphCanvas
            width: Math.max(journeyGraphFlickable.width, 4 * (journeyGraph.nodeWidth + 24))
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
