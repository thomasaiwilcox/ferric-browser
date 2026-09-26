import QtQuick

// Passive presentation only. It deliberately has no focus, Keys handler,
// MouseArea, TapHandler, or pointer-accepting delegate. Native input remains
// owned by FerricPagePointerAdapter.
Item {
    id: root
    property var browserUi
    property color lineColor: "#d6e8ff"
    property color labelColor: "#111820"
    property color labelBackground: "#d6e8ff"
    property color crosshairColor: "#ffd166"
    property real chromeScale: 1.0
    visible: !!browserUi && browserUi.spatial_visible
             && browserUi.spatial_selecting
    enabled: false
    Accessible.role: Accessible.Indicator
    Accessible.name: "Grid mode, depth " + (browserUi ? browserUi.spatial_depth : 0)
    Accessible.description: browserUi && browserUi.spatial_help_visible
                            ? "Use the displayed cell keys to refine. Enter clicks, Space hovers, Backspace goes back, zero resets, and Escape cancels."
                            : "Keyboard-controlled spatial navigation. Press question mark for help or Escape to cancel."
    Accessible.ignored: !visible

    readonly property real sx: browserUi && browserUi.spatial_root_width > 0
                               ? width / browserUi.spatial_root_width : 1
    readonly property real sy: browserUi && browserUi.spatial_root_height > 0
                               ? height / browserUi.spatial_root_height : 1
    readonly property real cx: browserUi ? browserUi.spatial_current_x * sx : 0
    readonly property real cy: browserUi ? browserUi.spatial_current_y * sy : 0
    readonly property real cw: browserUi ? browserUi.spatial_current_width * sx : 0
    readonly property real ch: browserUi ? browserUi.spatial_current_height * sy : 0
    readonly property real cellWidth: cw / 3
    readonly property real cellHeight: ch / 3
    readonly property bool labelsFit: cellWidth >= 24 * chromeScale
                                      && cellHeight >= 20 * chromeScale
    readonly property real labelFontSize: Math.max(10 * chromeScale,
                                                   Math.min(42 * chromeScale,
                                                            Math.min(cellWidth * 0.28,
                                                                     cellHeight * 0.42)))
    readonly property real reticleOuterRadius: Math.max(0,
                                                        Math.min(30 * chromeScale,
                                                                 Math.min(cellWidth,
                                                                          cellHeight) / 2
                                                                 - 3 * chromeScale))
    readonly property real reticleGap: Math.min(16 * chromeScale,
                                                reticleOuterRadius * 0.58)
    readonly property real reticleStroke: Math.max(1, 2 * chromeScale)
    readonly property real gridLineThickness: Math.max(3,
                                                        Math.round(3 * chromeScale))
    readonly property real gridLineCoreThickness: Math.max(1,
                                                            Math.round(chromeScale))
    readonly property color gridLineHaloColor: "#b0000000"
    readonly property var labels: browserUi && browserUi.spatial_labels
                                  && browserUi.spatial_labels.length === 9
                                  ? browserUi.spatial_labels
                                  : ["—","—","—","—","—","—","—","—","—"]

    Repeater {
        model: 4
        delegate: Rectangle {
            required property int index
            x: root.cx + index * root.cellWidth - width / 2
            y: root.cy
            width: root.gridLineThickness
            height: root.ch
            color: root.gridLineHaloColor

            Rectangle {
                anchors.horizontalCenter: parent.horizontalCenter
                width: root.gridLineCoreThickness
                height: parent.height
                color: root.lineColor
            }
        }
    }

    Repeater {
        model: 4
        delegate: Rectangle {
            required property int index
            x: root.cx
            y: root.cy + index * root.cellHeight - height / 2
            width: root.cw
            height: root.gridLineThickness
            color: root.gridLineHaloColor

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width
                height: root.gridLineCoreThickness
                color: root.lineColor
            }
        }
    }

    Repeater {
        model: 9
        delegate: Item {
            required property int index
            x: root.cx + (index % 3) * root.cellWidth
            y: root.cy + Math.floor(index / 3) * root.cellHeight
            width: root.cellWidth
            height: root.cellHeight

            Rectangle {
                anchors.fill: parent
                anchors.margins: 3 * root.chromeScale
                visible: index === 4 && root.reticleOuterRadius > 0
                color: "transparent"
                border.color: root.crosshairColor
                border.width: root.reticleStroke
                opacity: 0.7
            }

            Rectangle {
                id: labelBadge
                anchors.centerIn: parent
                visible: root.labelsFit
                width: Math.max(0,
                                Math.min(parent.width - 8 * root.chromeScale,
                                         Math.max(30 * root.chromeScale,
                                                  cellLabel.implicitWidth
                                                  + 14 * root.chromeScale)))
                height: Math.max(0,
                                 Math.min(parent.height - 8 * root.chromeScale,
                                          Math.max(28 * root.chromeScale,
                                                   cellLabel.implicitHeight
                                                   + 8 * root.chromeScale)))
                radius: Math.min(8 * root.chromeScale, height / 3)
                color: root.labelBackground
                opacity: index === 4 ? 0.82 : 0.72
                Text {
                    id: cellLabel
                    anchors.fill: parent
                    anchors.margins: 3 * root.chromeScale
                    text: root.labels[index] || "—"
                    color: root.labelColor
                    font.pixelSize: root.labelFontSize
                    font.bold: true
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                }
            }
        }
    }

    Rectangle {
        x: (browserUi ? browserUi.spatial_crosshair_x : 0) * root.sx
           - root.reticleOuterRadius
        y: (browserUi ? browserUi.spatial_crosshair_y : 0) * root.sy
           - root.reticleStroke / 2
        width: Math.max(0, root.reticleOuterRadius - root.reticleGap)
        height: root.reticleStroke
        color: root.crosshairColor
    }
    Rectangle {
        x: (browserUi ? browserUi.spatial_crosshair_x : 0) * root.sx
           + root.reticleGap
        y: (browserUi ? browserUi.spatial_crosshair_y : 0) * root.sy
           - root.reticleStroke / 2
        width: Math.max(0, root.reticleOuterRadius - root.reticleGap)
        height: root.reticleStroke
        color: root.crosshairColor
    }
    Rectangle {
        x: (browserUi ? browserUi.spatial_crosshair_x : 0) * root.sx
           - root.reticleStroke / 2
        y: (browserUi ? browserUi.spatial_crosshair_y : 0) * root.sy
           - root.reticleOuterRadius
        width: root.reticleStroke
        height: Math.max(0, root.reticleOuterRadius - root.reticleGap)
        color: root.crosshairColor
    }
    Rectangle {
        x: (browserUi ? browserUi.spatial_crosshair_x : 0) * root.sx
           - root.reticleStroke / 2
        y: (browserUi ? browserUi.spatial_crosshair_y : 0) * root.sy
           + root.reticleGap
        width: root.reticleStroke
        height: Math.max(0, root.reticleOuterRadius - root.reticleGap)
        color: root.crosshairColor
    }

    Rectangle {
        x: 10 * root.chromeScale
        y: 10 * root.chromeScale
        width: Math.max(0, Math.min(root.width - 20 * root.chromeScale,
                                   statusLabel.implicitWidth + 16 * root.chromeScale))
        height: statusLabel.implicitHeight + 8 * root.chromeScale
        radius: 4 * root.chromeScale
        color: "#dd111820"
        Text {
            id: statusLabel
            anchors.fill: parent
            anchors.margins: 4 * root.chromeScale
            color: "white"
            font.pixelSize: 12 * root.chromeScale
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            text: browserUi && browserUi.spatial_help_visible
                  ? "1-9 refine  Enter click  Space hover  Backspace back  0 reset  Esc cancel"
                  : "Grid · depth " + (browserUi ? browserUi.spatial_depth : 0)
                    + (root.labelsFit ? "" : " · cells " + root.labels.join(" "))
        }
    }
}
