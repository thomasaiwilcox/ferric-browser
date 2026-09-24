import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: journeyGraph
    required property var browserWindow
    required property var graphNodes
    required property var graphEntries
    required property var lineData
    spacing: 6

    function requestPaint() {
        graphCanvas.requestPaint()
    }

    onLineDataChanged: requestPaint()

    Flickable {
        id: journeyGraphFlickable
        Layout.fillWidth: true
        Layout.preferredHeight: 220
        Layout.minimumHeight: 120
        clip: true
        contentWidth: journeyGraphCanvas.width
        contentHeight: journeyGraphCanvas.height
        Accessible.name: "Bounded journey graph layout"

        Item {
            id: journeyGraphCanvas
            width: Math.max(journeyGraphFlickable.width, 4 * (170 + 24))
            height: Math.max(120, Math.ceil(journeyGraph.graphNodes.count / 4) * (48 + 18))

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
                    width: 170
                    height: 48
                    z: 1
                    color: journeyGraph.browserWindow.surfaceColor
                    border.color: journeyGraph.browserWindow.accentColor
                    Accessible.role: Accessible.ListItem
                    Accessible.name: model.label + ", transition " + model.transition
                    Accessible.description: "Journey node " + model.nodeId
                            + (model.source.length > 0 ? ", source " + model.source : "")

                    Column {
                        anchors.fill: parent
                        anchors.margins: 5
                        spacing: 2
                        Label {
                            width: parent.width
                            text: model.label
                            color: journeyGraph.browserWindow.primaryTextColor
                            elide: Text.ElideRight
                        }
                        Label {
                            width: parent.width
                            text: model.transition
                                    + (model.source.length > 0 ? " · " + model.source : "")
                            color: journeyGraph.browserWindow.mutedTextColor
                            elide: Text.ElideRight
                        }
                    }
                }
            }
        }
    }

    ListView {
        id: libraryGraphList
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        model: journeyGraph.graphEntries
        Accessible.name: "Journey relationships"
        delegate: ColumnLayout {
            width: libraryGraphList.width
            spacing: 2
            Label {
                Layout.fillWidth: true
                text: model.label
                color: journeyGraph.browserWindow.primaryTextColor
                elide: Text.ElideRight
            }
            Label {
                Layout.fillWidth: true
                text: model.secondary
                color: journeyGraph.browserWindow.mutedTextColor
                elide: Text.ElideMiddle
            }
        }
    }
}
