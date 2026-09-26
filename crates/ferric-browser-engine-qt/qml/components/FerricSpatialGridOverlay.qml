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
    readonly property bool labelsFit: cw / 3 >= 36 * chromeScale
                                      && ch / 3 >= 26 * chromeScale
    readonly property var labels: browserUi && browserUi.spatial_labels
                                  && browserUi.spatial_labels.length === 9
                                  ? browserUi.spatial_labels
                                  : ["—","—","—","—","—","—","—","—","—"]

    Rectangle { x: root.cx; y: root.cy; width: root.cw; height: 1; color: root.lineColor }
    Rectangle { x: root.cx; y: root.cy + root.ch - 1; width: root.cw; height: 1; color: root.lineColor }
    Rectangle { x: root.cx; y: root.cy; width: 1; height: root.ch; color: root.lineColor }
    Rectangle { x: root.cx + root.cw - 1; y: root.cy; width: 1; height: root.ch; color: root.lineColor }
    Rectangle { x: root.cx + root.cw / 3; y: root.cy; width: 1; height: root.ch; color: root.lineColor }
    Rectangle { x: root.cx + 2 * root.cw / 3; y: root.cy; width: 1; height: root.ch; color: root.lineColor }
    Rectangle { x: root.cx; y: root.cy + root.ch / 3; width: root.cw; height: 1; color: root.lineColor }
    Rectangle { x: root.cx; y: root.cy + 2 * root.ch / 3; width: root.cw; height: 1; color: root.lineColor }

    Repeater {
        model: 9
        delegate: Rectangle {
            required property int index
            visible: root.labelsFit
            width: Math.max(22 * root.chromeScale,
                            Math.min(cellLabel.implicitWidth + 8 * root.chromeScale,
                                     root.cw / 3 - 10 * root.chromeScale))
            height: 18 * root.chromeScale
            radius: 3 * root.chromeScale
            color: root.labelBackground
            opacity: 0.9
            x: root.cx + (index % 3) * root.cw / 3 + 5 * root.chromeScale
            y: root.cy + Math.floor(index / 3) * root.ch / 3 + 5 * root.chromeScale
            Text {
                id: cellLabel
                anchors.fill: parent
                anchors.margins: 3 * root.chromeScale
                text: root.labels[index] || "—"
                color: root.labelColor
                font.pixelSize: 12 * root.chromeScale
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
            }
        }
    }

    Rectangle {
        x: (browserUi ? browserUi.spatial_crosshair_x : 0) * root.sx - 8
        y: (browserUi ? browserUi.spatial_crosshair_y : 0) * root.sy - 0.5
        width: 5
        height: 1
        color: root.crosshairColor
    }
    Rectangle {
        x: (browserUi ? browserUi.spatial_crosshair_x : 0) * root.sx + 3
        y: (browserUi ? browserUi.spatial_crosshair_y : 0) * root.sy - 0.5
        width: 5
        height: 1
        color: root.crosshairColor
    }
    Rectangle {
        x: (browserUi ? browserUi.spatial_crosshair_x : 0) * root.sx - 0.5
        y: (browserUi ? browserUi.spatial_crosshair_y : 0) * root.sy - 8
        width: 1
        height: 5
        color: root.crosshairColor
    }
    Rectangle {
        x: (browserUi ? browserUi.spatial_crosshair_x : 0) * root.sx - 0.5
        y: (browserUi ? browserUi.spatial_crosshair_y : 0) * root.sy + 3
        width: 1
        height: 5
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
