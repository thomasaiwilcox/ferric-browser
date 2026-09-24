import QtQuick

// Renders root-supplied, validated hint candidates. Hint collection and
// activation policy remain at the composition root.
Item {
    id: hintOverlay
    required property var browserWindow
    required property bool hintsVisible
    required property var hintResults
    signal activationRequested(string label)
    signal actionsRequested(string label)

    anchors.fill: parent
    z: 30
    visible: hintsVisible && hintResults.length > 0
    focus: visible

    Repeater {
        model: hintOverlay.hintResults
        delegate: Rectangle {
            x: modelData.x
            y: modelData.y
            width: Math.max(24, hintLabel.implicitWidth + 10)
            height: Math.max(22, hintLabel.implicitHeight + 6)
            color: hintOverlay.browserWindow.warningColor
            border.color: hintOverlay.browserWindow.backgroundColor
            border.width: 1
            radius: 3

            Text {
                id: hintLabel
                anchors.centerIn: parent
                text: modelData.label
                color: hintOverlay.browserWindow.contrastText(parent.color)
                font.bold: true
                Accessible.name: "Hint " + modelData.label + " " + modelData.text
            }

            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onClicked: function(mouse) {
                    if (mouse.button === Qt.RightButton) {
                        hintOverlay.actionsRequested(modelData.label)
                    } else {
                        hintOverlay.activationRequested(modelData.label)
                    }
                }
            }
        }
    }
}
