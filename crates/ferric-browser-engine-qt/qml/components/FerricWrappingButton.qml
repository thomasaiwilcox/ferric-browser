import QtQuick
import QtQuick.Controls

Button {
    id: control
    width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
    contentItem: Label {
        text: control.text
        font: control.font
        color: control.palette.buttonText
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }
}
