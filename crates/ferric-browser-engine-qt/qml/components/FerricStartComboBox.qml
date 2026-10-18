import QtQuick
import QtQuick.Controls

ComboBox {
    id: control
    required property var browserWindow
    implicitHeight: Math.max(40, contentItem.implicitHeight + topPadding + bottomPadding)
    topPadding: 8
    bottomPadding: 8
    leftPadding: 12
    rightPadding: 34
    contentItem: Label {
        text: control.displayText
        color: control.browserWindow.primaryTextColor
        font.pointSize: control.browserWindow.chromeFontPointSize || 10
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        textFormat: Text.PlainText
    }
    indicator: Label {
        textFormat: Text.PlainText
        x: control.width - width - 12
        anchors.verticalCenter: parent.verticalCenter
        text: "⌄"
        color: control.browserWindow.mutedTextColor
    }
    background: Rectangle {
        radius: 9
        color: control.browserWindow.backgroundColor
        border.width: control.activeFocus ? 2 : 1
        border.color: control.activeFocus ? control.browserWindow.accentColor : control.browserWindow.borderColor
    }
    delegate: ItemDelegate {
        required property var modelData
        required property int index
        width: control.width - 12
        height: Math.max(38, itemLabel.implicitHeight + 16)
        highlighted: index === control.highlightedIndex
        Accessible.name: String(modelData)
        contentItem: Label {
            id: itemLabel
            text: String(modelData)
            color: control.browserWindow.primaryTextColor
            font.pointSize: control.browserWindow.chromeFontPointSize || 10
            wrapMode: Text.WordWrap
            textFormat: Text.PlainText
        }
        background: Rectangle {
            radius: 6
            color: parent.highlighted ? control.browserWindow.surfaceColor : "transparent"
        }
    }
    popup: Popup {
        y: control.height + 6
        width: control.width
        height: Math.min(340, options.contentHeight + 12)
        padding: 6
        contentItem: ListView {
            id: options
            clip: true
            model: control.popup.visible ? control.delegateModel : null
            currentIndex: control.highlightedIndex
            ScrollBar.vertical: ScrollBar {}
        }
        background: Rectangle {
            color: control.browserWindow.backgroundColor
            border.color: control.browserWindow.borderColor
            radius: 10
        }
    }
}
