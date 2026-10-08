import QtQuick
import QtQuick.Window
import QtWebEngine

Window {
    id: root
    width: 320
    height: 240
    visible: true
    WebEngineProfile { id: disposableProfile; offTheRecord: true }
    property var probeProfile: disposableProfile
    property url probeUrl: "data:text/html;charset=utf-8,<title>synthetic cookie probe</title>"
    WebEngineView {
        anchors.fill: parent
        profile: disposableProfile
        url: root.probeUrl
    }
}
