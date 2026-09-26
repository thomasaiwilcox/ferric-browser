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
    visible: !!browserUi && browserUi.spatial_visible
    enabled: false

    readonly property real sx: browserUi && browserUi.spatial_root_width > 0
                               ? width / browserUi.spatial_root_width : 1
    readonly property real sy: browserUi && browserUi.spatial_root_height > 0
                               ? height / browserUi.spatial_root_height : 1
    readonly property real cx: browserUi ? browserUi.spatial_current_x * sx : 0
    readonly property real cy: browserUi ? browserUi.spatial_current_y * sy : 0
    readonly property real cw: browserUi ? browserUi.spatial_current_width * sx : 0
    readonly property real ch: browserUi ? browserUi.spatial_current_height * sy : 0
    readonly property var labels: {
        if (!browserUi || !browserUi.spatial_labels) {
            return ["1","2","3","4","5","6","7","8","9"]
        }
        try {
            return JSON.parse(browserUi.spatial_labels)
        } catch (error) {
            return ["—","—","—","—","—","—","—","—","—"]
        }
    }

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
            width: 22
            height: 18
            radius: 3
            color: root.labelBackground
            opacity: 0.9
            x: root.cx + (index % 3) * root.cw / 3 + 5
            y: root.cy + Math.floor(index / 3) * root.ch / 3 + 5
            Text {
                anchors.centerIn: parent
                text: root.labels[index] || "—"
                color: root.labelColor
                font.pixelSize: 12
            }
        }
    }

    Rectangle {
        x: (browserUi ? browserUi.spatial_crosshair_x : 0) * root.sx - 8
        y: (browserUi ? browserUi.spatial_crosshair_y : 0) * root.sy - 0.5
        width: 16
        height: 1
        color: root.crosshairColor
    }
    Rectangle {
        x: (browserUi ? browserUi.spatial_crosshair_x : 0) * root.sx - 0.5
        y: (browserUi ? browserUi.spatial_crosshair_y : 0) * root.sy - 8
        width: 1
        height: 16
        color: root.crosshairColor
    }

    Rectangle {
        x: 10
        y: 10
        width: statusLabel.implicitWidth + 16
        height: statusLabel.implicitHeight + 8
        radius: 4
        color: "#dd111820"
        Text {
            id: statusLabel
            anchors.centerIn: parent
            color: "white"
            font.pixelSize: 12
            text: browserUi && browserUi.spatial_help_visible
                  ? "1-9 refine  Enter click  Space hover  Backspace back  0 reset  Esc cancel"
                  : "Grid · depth " + (browserUi ? browserUi.spatial_depth : 0)
        }
    }
}
