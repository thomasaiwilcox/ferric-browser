import QtQuick
import QtQuick.Controls

Rectangle {
    color: "#252a34"
    implicitHeight: 40
    TextInput {
        objectName: "chromeInput"
        anchors.fill: parent
        anchors.margins: 8
        color: "white"
        focus: false
        selectByMouse: true
        text: "chrome-ready"
    }
}
