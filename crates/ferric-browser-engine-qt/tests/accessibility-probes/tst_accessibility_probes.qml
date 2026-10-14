import QtQuick
import QtTest
import QtWebEngine

TestCase {
    id: tests
    name: "AccessibilityProviderProbes"
    when: windowShown
    width: 800
    height: 600

    property int qmlActionCount: 0
    property bool loaded: false
    property bool inspected: false
    property var qmlActionObject
    property string fixtureHeading: ""

    Component {
        id: actionComponent
        Rectangle {
            x: 24
            y: 24
            width: 220
            height: 48
            color: "#7090c0"
            Accessible.role: Accessible.Button
            Accessible.name: "S24 QML safe action"
            Accessible.description: "Local probe action; increments a test counter"
            Accessible.focusable: true
            Accessible.onPressAction: tests.qmlActionCount += 1
            Text { anchors.centerIn: parent; text: "QML probe action" }
        }
    }
    Component.onCompleted: qmlActionObject = actionComponent.createObject(tests)

    WebEngineView {
        id: page
        x: 24
        y: 96
        width: 720
        height: 460
        url: "http://127.0.0.1:8766/accessibility-provider.html"
        onLoadingChanged: function(request) {
            if (request.status === WebEngineView.LoadSucceededStatus) tests.loaded = true
        }
    }

    Timer {
        interval: 12000
        running: true
        repeat: false
        onTriggered: qmlActionObject.destroy()
    }

    function test_public_provider_window() {
        tryVerify(function() { return loaded }, 10000)
        page.runJavaScript("document.querySelector('h1')?.textContent || ''", function(value) {
            fixtureHeading = value
            inspected = true
        })
        tryVerify(function() { return inspected }, 5000)
        compare(fixtureHeading, "Provider fixture")
        wait(20000)
        compare(qmlActionCount, 1,
                "AT-SPI should perform the local QML action exactly once, including after target removal")
    }
}
