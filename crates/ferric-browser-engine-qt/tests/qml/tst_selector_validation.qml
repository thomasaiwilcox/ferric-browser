import QtQuick
import QtTest
import QtWebEngine

TestCase {
    id: testCase
    name: "SelectorSyntaxValidation"
    when: windowShown
    width: 320
    height: 240

    property var javascriptResult: undefined
    property bool documentReady: false

    WebEngineView {
        id: parserView
        anchors.fill: parent
        url: "data:text/html;charset=utf-8,%3C!doctype%20html%3E%3Chtml%3E%3Cbody%3E%3Cbutton%3Eok%3C%2Fbutton%3E%3C%2Fbody%3E%3C%2Fhtml%3E"
        onLoadingChanged: function(loadRequest) {
            if (loadRequest.status === WebEngineView.LoadSucceededStatus) {
                testCase.documentReady = true
            }
        }
    }

    function evaluateSelector(selector) {
        javascriptResult = undefined
        parserView.runJavaScript("(function(){try{document.querySelector("
                                 + JSON.stringify(selector)
                                 + ");return JSON.stringify({ok:true});}catch(e){return JSON.stringify({ok:false,reason:String(e)});}})()",
                                 function(value) { javascriptResult = value })
        tryVerify(function() { return javascriptResult !== undefined }, 5000)
        return JSON.parse(javascriptResult)
    }

    function test_webengine_accepts_valid_css_selector() {
        tryVerify(function() { return testCase.documentReady }, 10000)
        compare(evaluateSelector("button:first-child").ok, true)
    }

    function test_webengine_rejects_malformed_css_selector() {
        tryVerify(function() { return testCase.documentReady }, 10000)
        var result = evaluateSelector("button[")
        compare(result.ok, false)
        verify(result.reason.length > 0)
    }
}
