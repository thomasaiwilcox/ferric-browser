import QtQuick
import QtQuick.Controls
import "../scripts/HintPresentation.js" as HintPresentation

// Presentation snapshots only; selection and action authority remain in Rust.
Item {
    id: hintOverlay
    required property var browserWindow
    required property bool hintsVisible
    property bool interactionActive: hintsVisible
    required property var hintResults
    required property var sourceViewport
    required property var hintState
    required property string unmatchedPolicy
    required property real markerScale
    signal activationRequested(string label)
    signal actionsRequested(string label)
    signal selectionRequested(string label)

    anchors.fill: parent
    z: 30
    visible: hintsVisible && hintResults.length > 0
    // FerricBrowserKeyRouter owns hint gestures at the window boundary. Moving
    // Quick focus here would blur the page and invalidate focused-frame proof.
    focus: false

    property var laidOutHints: []
    property var placementCache: ({})
    property string hoveredLabel: ""
    property bool stackExpanded: false
    property string lastInteractionState: ""
    readonly property real clampedScale: Math.max(0.75, Math.min(2, markerScale))
    FontMetrics { id: labelMetrics; font.bold: true; font.pixelSize: 13 }

    function layoutHints(values, viewportWidth, viewportHeight, sourceWidth, sourceHeight, scale, previous) {
        values = Array.isArray(values) ? values : []
        var widths = ({})
        for (var i = 0; i < values.length; ++i)
            widths[String(values[i].label)] = labelMetrics.advanceWidth(String(values[i].label)) * scale
        return HintPresentation.layout(values,viewportWidth,viewportHeight,sourceWidth,sourceHeight,scale,widths,previous || null)
    }
    function rebuildLayout() {
        var values = layoutHints(hintResults,width,height,sourceViewport && sourceViewport.width,
                                 sourceViewport && sourceViewport.height,clampedScale,placementCache)
        for (var i = 0; i < values.length; ++i) {
            var row = values[i]
            placementCache[row.layout_key] = {dx:row.marker_x-row.anchor_x,dy:row.marker_y-row.anchor_y,
                                              width:row.marker_width,height:row.marker_height}
        }
        laidOutHints = values
        if (!values.some(function(row){return row.label===hoveredLabel})) hoveredLabel = ""
    }
    onHintResultsChanged: rebuildLayout()
    onSourceViewportChanged: rebuildLayout()
    onWidthChanged: rebuildLayout()
    onHeightChanged: rebuildLayout()
    onMarkerScaleChanged: rebuildLayout()
    onInteractionActiveChanged: {
        if (!interactionActive) { placementCache = ({}); hoveredLabel = ""; stackExpanded = false }
    }
    Component.onCompleted: rebuildLayout()
    onHintStateChanged: {
        var key=JSON.stringify([hintState && hintState.mode,hintState && hintState.prefix,
                               hintState && hintState.query,hintState && hintState.active_label])
        if (key!==lastInteractionState) { hoveredLabel="";stackExpanded=false }
        lastInteractionState=key
        connectors.requestPaint()
    }

    property var matchingLabelLookup: {
        var lookup = Object.create(null)
        var labels = hintState && hintState.matching_labels || []
        for (var i = 0; i < labels.length; ++i) lookup[String(labels[i])] = true
        return lookup
    }
    function matching(label) { return matchingLabelLookup[String(label)] === true }
    function active(label) { return String(hintState && hintState.active_label || "") === String(label) }
    function escaped(value) {
        return String(value).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/\"/g,"&quot;")
    }
    property var activeHint: {
        var label = String(hintState && hintState.active_label || "")
        for (var i = 0; i < laidOutHints.length; ++i) if (String(laidOutHints[i].label)===label) return laidOutHints[i]
        return null
    }
    readonly property string previewText: activeHint ? HintPresentation.description(activeHint)
        : (hintState && hintState.prefix || hintState && hintState.query || "")
          + " · " + (hintState && hintState.remaining || 0) + " matches · Tab to preview"
    property var filteredClusters: {
        var groups = Object.create(null)
        for (var i = 0; i < laidOutHints.length; ++i) {
            var row = laidOutHints[i]
            if (!matching(row.label)) continue
            if (!groups[row.cluster_key]) groups[row.cluster_key] = []
            groups[row.cluster_key].push(row)
        }
        return groups
    }
    property var visibleMarkerLookup: {
        var lookup = Object.create(null), seen = Object.create(null)
        for (var i = 0; i < laidOutHints.length; ++i) {
            var row = laidOutHints[i]
            if (seen[row.cluster_key]) continue
            seen[row.cluster_key] = true
            var members = filteredClusters[row.cluster_key] || []
            var selected = members[0] || null
            for (var j = 0; j < members.length; ++j)
                if (active(members[j].label)) { selected = members[j]; break }
            if (selected) lookup[selected.label] = true
            else if (unmatchedPolicy !== "hide") lookup[row.label] = true
        }
        return lookup
    }
    // Instantiate the displayed representative only; every member remains in
    // the session, keyboard traversal and virtualized expanded list.
    readonly property var displayedMarkers: laidOutHints.filter(function(row) {
        return visibleMarkerLookup[String(row.label)] === true
    })
    function clusterRows(row) { return row ? filteredClusters[row.cluster_key] || [] : [] }
    function collisionLabels(label) {
        for (var i = 0; i < laidOutHints.length; ++i)
            if (laidOutHints[i].label===label) return clusterRows(laidOutHints[i]).map(function(row){return row.label})
        return []
    }
    function markerVisible(row) { return visibleMarkerLookup[String(row.label)] === true }

    Canvas {
        id: connectors
        anchors.fill: parent
        z: 0
        onPaint: {
            var context = getContext("2d")
            context.clearRect(0,0,width,height)
            context.strokeStyle = hintOverlay.browserWindow.warningColor
            context.fillStyle = hintOverlay.browserWindow.warningColor
            context.lineWidth = 1.5 * hintOverlay.clampedScale
            for (var i = 0; i < hintOverlay.laidOutHints.length; ++i) {
                var row = hintOverlay.laidOutHints[i]
                if (!row.connected || !hintOverlay.markerVisible(row)) continue
                var x = Math.max(row.marker_x,Math.min(row.marker_x+row.marker_width,row.endpoint_x))
                var y = Math.max(row.marker_y,Math.min(row.marker_y+row.marker_height,row.endpoint_y))
                context.beginPath();context.moveTo(x,y);context.lineTo(row.endpoint_x,row.endpoint_y);context.stroke()
                context.beginPath();context.arc(row.endpoint_x,row.endpoint_y,2*hintOverlay.clampedScale,0,2*Math.PI);context.fill()
            }
        }
    }
    onLaidOutHintsChanged: connectors.requestPaint()
    onMatchingLabelLookupChanged: connectors.requestPaint()

    Repeater {
        objectName: "hintMarkerRepeater"
        model: hintOverlay.displayedMarkers
        delegate: Rectangle {
            id: badge
            visible: hintOverlay.markerVisible(modelData)
            opacity: hintOverlay.matching(modelData.label) || hintOverlay.unmatchedPolicy === "show" ? 1 : 0.24
            x: modelData.marker_x; y: modelData.marker_y
            width: modelData.marker_width; height: modelData.marker_height
            color: hintOverlay.browserWindow.warningColor
            border.color: hintOverlay.active(modelData.label) ? hintOverlay.browserWindow.primaryTextColor : hintOverlay.browserWindow.backgroundColor
            border.width: hintOverlay.active(modelData.label) ? 2 : 1
            radius: 3 * hintOverlay.clampedScale
            z: hintOverlay.active(modelData.label) ? 3 : 1
            Accessible.role: Accessible.Button
            Accessible.selectable: true
            Accessible.selected: hintOverlay.active(modelData.label)
            Accessible.description: hintOverlay.active(modelData.label) ? "Active hint target" : "Hint target"
            Accessible.name: HintPresentation.description(modelData)
                + (modelData.collision_cluster > 1 ? ". " + modelData.collision_cluster + " overlapping targets. Space cycles." : "")
            Accessible.onPressAction: hintOverlay.activationRequested(modelData.label)
            Text {
                anchors.centerIn: parent
                textFormat: Text.RichText
                text: {
                    var label = String(modelData.label)
                    var prefix = hintOverlay.hintState && hintOverlay.hintState.mode === "label" ? String(hintOverlay.hintState.prefix || "") : ""
                    var letters = prefix.length && label.indexOf(prefix)===0
                            ? "<u>"+hintOverlay.escaped(prefix)+"</u>"+hintOverlay.escaped(label.slice(prefix.length)) : hintOverlay.escaped(label)
                    return letters + (modelData.document_scroll ? " ↕ Page scroll" : "")
                }
                color: hintOverlay.browserWindow.contrastText(parent.color)
                font.bold: true; font.pixelSize: 13 * hintOverlay.clampedScale
                Accessible.ignored: true
            }
            Rectangle {
                visible: hintOverlay.clusterRows(modelData).length > 1
                z: 2
                Accessible.role: Accessible.Button
                Accessible.name: "Show " + hintOverlay.clusterRows(modelData).length + " overlapping targets"
                Accessible.onPressAction: { hintOverlay.selectionRequested(modelData.label); hintOverlay.stackExpanded = true }
                anchors.right: parent.right; anchors.top: parent.top
                anchors.rightMargin: -5; anchors.topMargin: -8
                width: countLabel.implicitWidth+6; height: 14 * hintOverlay.clampedScale
                radius: 3
                color: hintOverlay.browserWindow.backgroundColor
                Text { id:countLabel;anchors.centerIn:parent;text:"+"+(hintOverlay.clusterRows(modelData).length-1);color:hintOverlay.browserWindow.primaryTextColor;font.pixelSize:10*hintOverlay.clampedScale }
                MouseArea {
                    anchors.fill: parent
                    onClicked: { hintOverlay.selectionRequested(modelData.label); hintOverlay.stackExpanded = true }
                }
            }
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                hoverEnabled: true
                onEntered: hintOverlay.hoveredLabel = modelData.label
                onExited: if (hintOverlay.hoveredLabel === modelData.label) hintOverlay.hoveredLabel = ""
                onClicked: function(mouse) {
                    if (mouse.button === Qt.RightButton) hintOverlay.actionsRequested(modelData.label)
                    else hintOverlay.activationRequested(modelData.label)
                }
            }

        }
    }

    property var hoveredHint: {
        for (var i=0;i<laidOutHints.length;++i)
            if (laidOutHints[i].label===hoveredLabel) return laidOutHints[i]
        return null
    }
    ToolTip {
        id: hoverDescription
        parent: hintOverlay
        visible: hintOverlay.hintsVisible && !!hintOverlay.hoveredHint
        delay: 250
        x: hintOverlay.hoveredHint ? Math.max(0,Math.min(hintOverlay.width-width,hintOverlay.hoveredHint.marker_x)) : 0
        y: hintOverlay.hoveredHint ? Math.max(0,Math.min(hintOverlay.height-height,hintOverlay.hoveredHint.marker_y+hintOverlay.hoveredHint.marker_height+6)) : 0
        width: Math.min(360*hintOverlay.clampedScale,hintOverlay.width)
        text: HintPresentation.description(hintOverlay.hoveredHint)
        contentItem: Text {
            text: hoverDescription.text
            textFormat: Text.PlainText
            color: hintOverlay.browserWindow.primaryTextColor
            font.pixelSize: 13*hintOverlay.clampedScale
            wrapMode: Text.Wrap
        }
        background: Rectangle { color:hintOverlay.browserWindow.backgroundColor;border.color:hintOverlay.browserWindow.warningColor;radius:4 }
    }

    // The outline follows the keyboard selection; hover cannot redirect Enter.
    Repeater {
        objectName: "activeHintOutlineRepeater"
        model: hintOverlay.activeHint ? hintOverlay.activeHint.fragments : []
        delegate: Rectangle {
            x: modelData.x - 2; y: modelData.y - 2
            width: modelData.width + 4; height: modelData.height + 4
            color: "transparent"
            border.color: hintOverlay.browserWindow.warningColor; border.width: 2
            radius: 2; z: 0
            Accessible.ignored: true
        }
    }
    Rectangle {
        id: stackPanel
        objectName: "hintStackPanel"
        visible: hintOverlay.stackExpanded && hintOverlay.activeHint && hintOverlay.clusterRows(hintOverlay.activeHint).length > 1
        x: hintOverlay.activeHint ? Math.max(0,Math.min(hintOverlay.width-width,hintOverlay.activeHint.marker_x)) : 0
        y: hintOverlay.activeHint ? Math.max(0,Math.min(hintOverlay.height-height,hintOverlay.activeHint.marker_y+hintOverlay.activeHint.marker_height+8)) : 0
        width: Math.min(hintOverlay.width,320*hintOverlay.clampedScale)
        height: Math.min(hintOverlay.height,Math.min(6,stackList.count)*34*hintOverlay.clampedScale)
        color: hintOverlay.browserWindow.backgroundColor
        border.color: hintOverlay.browserWindow.warningColor; radius: 4
        z: 5
        ListView {
            id: stackList
            anchors.fill: parent; anchors.margins: 3; clip: true
            model: hintOverlay.clusterRows(hintOverlay.activeHint)
            delegate: ItemDelegate {
                width: stackList.width; height: 34*hintOverlay.clampedScale
                text: HintPresentation.description(modelData)
                contentItem: Text { text:HintPresentation.description(modelData);textFormat:Text.PlainText;elide:Text.ElideRight;color:hintOverlay.browserWindow.primaryTextColor;font.pixelSize:13*hintOverlay.clampedScale;verticalAlignment:Text.AlignVCenter }
                onClicked: hintOverlay.activationRequested(modelData.label)
                onPressAndHold: hintOverlay.actionsRequested(modelData.label)
            }
        }
    }
}
