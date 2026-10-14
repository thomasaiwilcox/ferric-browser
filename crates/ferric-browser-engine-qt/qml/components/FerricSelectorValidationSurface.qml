import QtQuick
import QtWebEngine

Item {
    id: root
    objectName: "selectorValidationSurface"

    required property var browserUi
    property string activeGeneration: ""
    property var pendingRequest: null
    property bool documentReady: false
    visible: true
    opacity: 0
    width: 1
    height: 1

    function rejectActiveRequest(reason) {
        if (!root.browserUi || root.activeGeneration.length === 0) {
            return
        }
        var token = root.activeGeneration
        root.activeGeneration = ""
        selectorTimeout.stop()
        root.browserUi.complete_selector_validation(token, JSON.stringify({
            ok: false,
            reason: String(reason).slice(0, 192)
        }))
    }

    function validateRequest(request) {
        if (!root.browserUi || !request || !request.generation
                || !Array.isArray(request.items) || request.items.length === 0) {
            root.rejectActiveRequest("selector validation request is unavailable")
            return
        }
        if (!root.documentReady) {
            root.pendingRequest = request
            return
        }

        root.pendingRequest = null
        root.activeGeneration = String(request.generation)
        selectorTimeout.restart()
        var token = root.activeGeneration
        var itemJson = JSON.stringify(request.items)
        var script = "(function(){var items=" + itemJson + ";"
                + "for(var i=0;i<items.length;i++){var item=items[i];"
                + "try{document.querySelector(item.selector);}catch(error){"
                + "return JSON.stringify({ok:false,family:item.family,index:item.index,"
                + "rule:item.rule,reason:String(error).slice(0,192)});}}"
                + "return JSON.stringify({ok:true});})()"
        selectorDocument.runJavaScript(script, function(result) {
            if (root.activeGeneration !== token) {
                return
            }
            root.activeGeneration = ""
            selectorTimeout.stop()
            var response = typeof result === "string" ? result : JSON.stringify(result)
            root.browserUi.complete_selector_validation(token, response)
        })
    }

    WebEngineView {
        id: selectorDocument
        anchors.fill: parent
        opacity: 0
        visible: true
        url: "data:text/html;charset=utf-8,%3C!doctype%20html%3E%3Chtml%3E%3Chead%3E%3Cmeta%20charset%3D%22utf-8%22%3E%3C%2Fhead%3E%3Cbody%3E%3Cmain%20id%3D%22ferric-selector-parser-document%22%3E%3Ca%20href%3D%22%23local%22%3Elink%3C%2Fa%3E%3Cbutton%3Ebutton%3C%2Fbutton%3E%3C%2Fmain%3E%3C%2Fbody%3E%3C%2Fhtml%3E"

        onLoadingChanged: function(loadRequest) {
            if (loadRequest.status === WebEngineView.LoadSucceededStatus) {
                root.documentReady = true
                if (root.pendingRequest) {
                    root.validateRequest(root.pendingRequest)
                }
            } else if (loadRequest.status === WebEngineView.LoadFailedStatus) {
                root.documentReady = false
                root.rejectActiveRequest("browser-owned WebEngine parser document failed to load")
            }
        }
    }

    Timer {
        id: selectorTimeout
        interval: 5000
        repeat: false
        onTriggered: root.rejectActiveRequest("WebEngine selector validation timed out")
    }

    Connections {
        target: root.browserUi
        function onSelector_validation_requestChanged() {
            var raw = root.browserUi.selector_validation_request
            if (!raw || raw.length === 0) {
                root.activeGeneration = ""
                root.pendingRequest = null
                selectorTimeout.stop()
                return
            }
            var request = null
            try {
                request = JSON.parse(raw)
            } catch (error) {
                root.rejectActiveRequest("selector validation request could not be decoded")
                return
            }
            root.activeGeneration = String(request.generation || "")
            root.validateRequest(request)
        }
    }
}
