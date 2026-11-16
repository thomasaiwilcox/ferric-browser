import QtQuick
import QtTest
import QtWebEngine
import "../../qml/components"

TestCase {
    id: testCase
    name: "SelectorSyntaxValidation"
    when: windowShown
    width: 320
    height: 240

    property var javascriptResult: undefined
    property bool documentReady: false
    property int requestGeneration: 0

    QtObject {
        id: validationUi
        property string selector_validation_request: ""
        property var completion: null
        function complete_selector_validation(generation, response) {
            completion = {generation: generation, result: JSON.parse(response)}
        }
    }
    QtObject {
        id: validationWindow
        function runBrowserScript(view, script, callback, world) {
            view.runJavaScript(script, world, callback)
        }
    }
    FerricSelectorValidationSurface {
        id: validationSurface
        browserUi: validationUi
        browserWindow: validationWindow
    }

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

    function submit(items) {
        validationUi.completion = null
        var generation = String(++requestGeneration)
        validationUi.selector_validation_request = JSON.stringify({generation: generation, items: items})
        tryVerify(function() { return validationUi.completion !== null }, 10000)
        compare(validationUi.completion.generation, generation)
        return validationUi.completion.result
    }
    function test_production_surface_accepts_interaction_selector_items() {
        var items = []
        var selectors = ["nav-shell", "slot.nav", ".nav-item", ".nav-scope", "a.nav-target",
                         "iframe.cards", ".cards", "card-item", "slot.primary", ".primary-item",
                         "button.primary", "#\\31 23", ".foo\\:bar[data\\:id=\\31 23]", "[data-é='привет']"]
        for (var i = 0; i < selectors.length; ++i)
            items.push({family: "interaction", index: i, selector: selectors[i], rule: "base site rule fixture interaction"})
        compare(submit(items).ok, true)
    }
    function test_production_surface_reports_interaction_error_provenance() {
        var result = submit([{family: "interaction", index: 3, selector: "button[",
                              rule: "temporary site rule fixture interaction collections[0].primary_selector"}])
        compare(result.ok, false)
        compare(result.family, "interaction")
        compare(result.index, 3)
        compare(result.rule, "temporary site rule fixture interaction collections[0].primary_selector")
        verify(result.reason.length > 0)
    }
    function cleanup() {
        validationUi.selector_validation_request = ""
    }
}
