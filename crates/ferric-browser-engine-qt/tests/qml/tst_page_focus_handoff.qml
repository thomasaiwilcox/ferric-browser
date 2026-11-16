import QtQuick
import QtTest
import QtWebEngine
import "../../qml/components" as Components

TestCase {
    id: testCase
    name: "PageFocusHandoff"
    when: windowShown
    visible: true
    width: 800
    height: 600
    property bool done: false
    property var result
    property bool pageKeysActive: true
    property bool hostActive: false
    QtObject {
        id: host
        property bool browserKeyFocusActive: testCase.pageKeysActive
        property bool active: testCase.hostActive
        property var modalInputScope: null
        function activeWebView() { return page }
    }
    Components.FerricFocusOverlayController {
        browserWindow: host
        followTabFocus: true
    }
    WebEngineView {
        id: page
        anchors.fill: parent
        property string nativeUrl: "rb://start"
        TextInput {
            id: nativeInput
            visible: page.nativeUrl.length > 0
            text: "Start page search"
        }
    }
    TextInput { id: chromeInput; text: "Browser command" }
    function evaluate(script) {
        done = false
        page.runJavaScript(script, function(value) { result = value; done = true })
        tryVerify(function() { return done }, 5000)
        return result
    }
    function test_navigation_data() {
        return [{tag:"page-owns-input", pageKeys:true}, {tag:"chrome-owns-input", pageKeys:false}]
    }
    function test_activation_data() { return test_navigation_data() }
    function test_activation(data) {
        hostActive = false
        pageKeysActive = data.pageKeys
        page.nativeUrl = ""
        page.loadHtml("<!doctype html><button>Page control</button>")
        tryVerify(function() { return !page.loading && evaluate("document.readyState==='complete'") }, 10000)
        wait(0)
        chromeInput.forceActiveFocus()
        compare(evaluate("document.hasFocus()"), false)
        hostActive = true
        if (data.pageKeys) {
            tryVerify(function() { return evaluate("document.hasFocus()") === true }, 2000)
        } else {
            verify(chromeInput.activeFocus)
            compare(evaluate("document.hasFocus()"), false)
        }
    }
    function test_navigation(data) {
        pageKeysActive = data.pageKeys
        page.nativeUrl = "rb://start"
        nativeInput.forceActiveFocus()
        verify(nativeInput.activeFocus)
        if (!data.pageKeys) chromeInput.forceActiveFocus()
        page.nativeUrl = ""
        page.loadHtml("<!doctype html><button>Page control</button>")
        tryVerify(function() { return !page.loading && evaluate("document.readyState==='complete'") }, 10000)
        if (data.pageKeys) {
            tryVerify(function() { return evaluate("document.hasFocus()") === true }, 2000)
        } else {
            verify(chromeInput.activeFocus)
            compare(evaluate("document.hasFocus()"), false)
        }
    }
}
