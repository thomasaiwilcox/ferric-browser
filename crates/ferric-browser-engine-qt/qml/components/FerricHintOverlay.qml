import QtQuick

// Native hint markers. Collection and activation remain behind the validated
// Rust/page-script boundary; this item only lays out and presents snapshots.
Item {
    id: hintOverlay
    required property var browserWindow
    required property bool hintsVisible
    required property var hintResults
    required property var sourceViewport
    required property var hintState
    required property string unmatchedPolicy
    required property real markerScale
    required property int collisionRotation
    signal activationRequested(string label)
    signal actionsRequested(string label)

    anchors.fill: parent
    z: 30
    visible: hintsVisible && hintResults.length > 0
    focus: visible

    function matching(label) {
        var labels = hintState && hintState.matching_labels
                ? hintState.matching_labels : []
        return labels.indexOf(String(label)) >= 0
    }

    function active(label) {
        return String(hintState && hintState.active_label || "") === String(label)
    }

    function escaped(value) {
        return String(value).replace(/&/g, "&amp;").replace(/</g, "&lt;")
                .replace(/>/g, "&gt;").replace(/\"/g, "&quot;")
    }

    function coordinateScale(renderedSize, sourceSize) {
        var source = Number(sourceSize || 0)
        if (!Number.isFinite(source) || source <= 0)
            return 1.0
        var ratio = renderedSize / source
        return Number.isFinite(ratio) && ratio >= 0.2 && ratio <= 5.0
                ? ratio : 1.0
    }

    function layoutHints(values, viewportWidth, viewportHeight,
                         sourceViewportWidth, sourceViewportHeight, scale) {
        var placed = []
        var cells = ({})
        var scaleX = coordinateScale(viewportWidth, sourceViewportWidth)
        var scaleY = coordinateScale(viewportHeight, sourceViewportHeight)
        var cellWidth = Math.max(36, 52 * scale)
        var cellHeight = Math.max(24, 30 * scale)
        function overlaps(x, y, w, h) {
            var minX = Math.floor(x / cellWidth), maxX = Math.floor((x + w) / cellWidth)
            var minY = Math.floor(y / cellHeight), maxY = Math.floor((y + h) / cellHeight)
            for (var cy = minY; cy <= maxY; ++cy) {
                for (var cx = minX; cx <= maxX; ++cx) {
                    var bucket = cells[cx + ":" + cy] || []
                    for (var i = 0; i < bucket.length; ++i) {
                        var p = bucket[i]
                        if (x < p.x + p.w && x + w > p.x
                                && y < p.y + p.h && y + h > p.y)
                            return true
                    }
                }
            }
            return false
        }
        function occupy(marker) {
            var minX = Math.floor(marker.marker_x / cellWidth)
            var maxX = Math.floor((marker.marker_x + marker.marker_width) / cellWidth)
            var minY = Math.floor(marker.marker_y / cellHeight)
            var maxY = Math.floor((marker.marker_y + marker.marker_height) / cellHeight)
            for (var cy = minY; cy <= maxY; ++cy) {
                for (var cx = minX; cx <= maxX; ++cx) {
                    var key = cx + ":" + cy
                    if (!cells[key]) cells[key] = []
                    cells[key].push({x: marker.marker_x, y: marker.marker_y,
                                     w: marker.marker_width, h: marker.marker_height})
                }
            }
        }
        for (var index = 0; index < values.length; ++index) {
            var source = values[index]
            var w = Math.max(24 * scale, (String(source.label).length * 9 + 10) * scale)
            var h = 22 * scale
            var targetX = source.x * scaleX
            var targetY = source.y * scaleY
            var targetWidth = source.width * scaleX
            var targetHeight = source.height * scaleY
            var targetRight = targetX + targetWidth
            var targetBottom = targetY + targetHeight
            // Keep the label visibly attached to its target whenever possible.
            // Outside-corner positions are only fallbacks for dense clusters.
            var options = [[targetX, targetY],
                           [targetRight - w, targetY],
                           [targetX, targetBottom - h],
                           [targetRight - w, targetBottom - h],
                           [targetX, targetY - h],
                           [targetRight - w, targetY - h],
                           [targetX, targetBottom],
                           [targetRight - w, targetBottom]]
            var chosen = null
            for (var option = 0; option < options.length && !chosen; ++option) {
                var x = Math.max(0, Math.min(viewportWidth - w, options[option][0]))
                var y = Math.max(0, Math.min(viewportHeight - h, options[option][1]))
                if (!overlaps(x, y, w, h)) chosen = [x, y, 0]
            }
            for (var offset = 1; offset <= 8 && !chosen; ++offset) {
                var dx = ((offset % 3) - 1) * 8 * scale
                var dy = Math.floor((offset + 1) / 3) * 7 * scale
                var ox = Math.max(0, Math.min(viewportWidth - w, targetX + dx))
                var oy = Math.max(0, Math.min(viewportHeight - h, targetY + dy))
                if (!overlaps(ox, oy, w, h)) chosen = [ox, oy, 0]
            }
            if (!chosen)
                chosen = [Math.max(0, Math.min(viewportWidth - w, targetX)),
                          Math.max(0, Math.min(viewportHeight - h, targetY)), 1]
            var marker = ({})
            for (var key in source) marker[key] = source[key]
            marker.x = targetX
            marker.y = targetY
            marker.width = targetWidth
            marker.height = targetHeight
            marker.marker_x = chosen[0]
            marker.marker_y = chosen[1]
            marker.marker_width = w
            marker.marker_height = h
            marker.collision_cluster = chosen[2]
            placed.push(marker)
            occupy(marker)
        }
        return placed
    }

    property var laidOutHints: layoutHints(hintResults, width, height,
                                           sourceViewport && sourceViewport.width,
                                           sourceViewport && sourceViewport.height,
                                           Math.max(0.75, Math.min(2.0, markerScale)))

    Repeater {
        model: hintOverlay.laidOutHints
        delegate: Rectangle {
            visible: hintOverlay.matching(modelData.label)
                     || hintOverlay.unmatchedPolicy !== "hide"
            opacity: hintOverlay.matching(modelData.label)
                     || hintOverlay.unmatchedPolicy === "show" ? 1.0 : 0.24
            x: modelData.marker_x
            y: modelData.marker_y
            width: modelData.marker_width
            height: modelData.marker_height
            color: hintOverlay.browserWindow.warningColor
            border.color: hintOverlay.active(modelData.label)
                          ? hintOverlay.browserWindow.primaryTextColor
                          : hintOverlay.browserWindow.backgroundColor
            border.width: hintOverlay.active(modelData.label) ? 3 : 1
            radius: 3 * hintOverlay.markerScale
            z: hintOverlay.active(modelData.label) ? 3 : 1

            Text {
                anchors.centerIn: parent
                textFormat: Text.RichText
                text: {
                    var label = String(modelData.label)
                    var prefix = hintOverlay.hintState.mode === "label"
                            ? String(hintOverlay.hintState.prefix || "") : ""
                    if (prefix.length === 0 || label.indexOf(prefix) !== 0)
                        return hintOverlay.escaped(label)
                    return "<u>" + hintOverlay.escaped(prefix) + "</u>"
                            + hintOverlay.escaped(label.slice(prefix.length))
                }
                color: hintOverlay.browserWindow.contrastText(parent.color)
                font.bold: true
                font.pixelSize: 13 * hintOverlay.markerScale
                Accessible.name: "Hint " + modelData.label + " " + modelData.text
            }

            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onClicked: function(mouse) {
                    if (mouse.button === Qt.RightButton)
                        hintOverlay.actionsRequested(modelData.label)
                    else
                        hintOverlay.activationRequested(modelData.label)
                }
            }
        }
    }

    Repeater {
        model: hintOverlay.laidOutHints
        delegate: Rectangle {
            visible: hintOverlay.active(modelData.label)
            x: modelData.x - 2
            y: modelData.y - 2
            width: modelData.width + 4
            height: modelData.height + 4
            color: "transparent"
            border.color: hintOverlay.browserWindow.warningColor
            border.width: 2
            radius: 2
            z: 0
            Accessible.name: "Active hint target " + modelData.text
        }
    }
}
