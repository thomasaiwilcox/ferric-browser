import QtQuick
import QtQuick.Window
import QtWebEngine

Window {
    id: root
    width: 320
    height: 240
    visible: true

    WebEngineProfile {
        id: disposableProfile
        storageName: "ferric-cookie-boundary-fixture"
        offTheRecord: false
        persistentCookiesPolicy: WebEngineProfile.OnlyPersistentCookies
    }

    property var probeProfile: disposableProfile
    property url probeUrl: ""

    WebEngineView {
        anchors.fill: parent
        profile: disposableProfile
        url: root.probeUrl
    }
}
