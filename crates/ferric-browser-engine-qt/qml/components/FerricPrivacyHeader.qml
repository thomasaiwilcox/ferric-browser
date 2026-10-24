import QtQuick

// Window identity stays outside the page and does not participate in input.
Rectangle {
    id: identity
    objectName: "windowPrivacyHeader"
    required property var browserWindow
    required property bool privateProfile
    required property bool ephemeralProfile
    readonly property string label: ephemeralProfile ? "Temporary" : privateProfile ? "Private" : ""
    readonly property color markerColor: ephemeralProfile ? browserWindow.accentColor : browserWindow.privateColor
    visible: label.length > 0
    implicitHeight: label.length > 0 ? Math.ceil(metrics.height) + 8 : 0
    height: implicitHeight
    color: browserWindow.panelColor
    Accessible.name: label.length ? label + " window" : ""
    Accessible.role: Accessible.StaticText
    Accessible.ignored: !visible

    FontMetrics { id: metrics; font: identity.browserWindow.font }
    Text {
        objectName: "windowPrivacyLabel"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.leftMargin: 8
        anchors.rightMargin: 8
        height: metrics.height
        text: identity.label
        textFormat: Text.PlainText
        font: Qt.font({family: identity.browserWindow.font.family,
                       pointSize: identity.browserWindow.font.pointSize, bold: true})
        fontSizeMode: Text.Fit
        minimumPointSize: 8
        color: identity.browserWindow.readableTextColor(identity.markerColor, identity.color)
        Accessible.ignored: true
    }
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: identity.markerColor
    }
}
