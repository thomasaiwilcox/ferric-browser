import QtQuick
import QtTest
import QtWebEngine

TestCase {
    id: tests
    name: "FrameProviderProbes"
    when: windowShown
    width: 640
    height: 480

    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) {
            loaded = request.status === WebEngineView.LoadSucceededStatus
        }
    }

    property bool callbackDone: false
    property var callbackValue

    function evaluatePage(source, world) {
        callbackDone = false
        page.runJavaScript(source, world, function(value) {
            callbackValue = value
            callbackDone = true
        })
        tryVerify(function() { return callbackDone }, 5000)
        return callbackValue
    }

    function evaluateFrame(frame, source) {
        callbackDone = false
        frame.runJavaScript(source, WebEngineScript.ApplicationWorld, function(value) {
            callbackValue = value
            callbackDone = true
        })
        tryVerify(function() { return callbackDone }, 5000)
        return callbackValue
    }

    function directFrames() {
        return page.mainFrame.children
    }

    function frameByHtmlName(name) {
        var frames = directFrames()
        for (var i = 0; i < frames.length; i++) {
            if (frames[i].htmlName === name)
                return frames[i]
        }
        return null
    }

    function test_T035_T052_independent_frame_provider_boundary() {
        page.loaded = false
        page.url = "http://127.0.0.1:8766/frame-provider-parent.html"
        tryVerify(function() { return page.loaded }, 15000)
        verify(page.mainFrame.isValid, "public mainFrame should be valid")

        tryVerify(function() {
            var frames = directFrames()
            if (frames.length !== 6)
                return false
            for (var i = 0; i < frames.length; i++) {
                if (!frames[i].isValid || String(frames[i].url).indexOf("frame-provider-child.html") < 0)
                    return false
            }
            return true
        }, 15000, "QWebEngineFrame.children did not expose the six fixture children")

        var parentBoundary = evaluatePage(
            "(() => { const ids=['same','duplicate-a','duplicate-b','sandboxed','removable','navigating']; " +
            "const frames=ids.map(id=>document.getElementById(id)); " +
            "const rects=['duplicate-a','duplicate-b'].map(id=>{const r=document.getElementById(id).getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height};}); " +
            "return {sameOriginReadable:frames[0].contentDocument!==null,crossOriginHidden:frames[1].contentDocument===null&&frames[2].contentDocument===null,sandboxOriginHidden:frames[3].contentDocument===null,duplicateRectsDistinct:rects[0].x!==rects[1].x,rects}; })()",
            WebEngineScript.MainWorld)
        verify(parentBoundary.sameOriginReadable, "same-origin control should remain readable")
        verify(parentBoundary.crossOriginHidden && parentBoundary.sandboxOriginHidden,
               "parent world must not read cross-origin or opaque sandbox child DOM")
        verify(parentBoundary.duplicateRectsDistinct,
               "duplicate frames must have distinct outer rectangles")

        var duplicates = []
        var frames = directFrames()
        for (var j = 0; j < frames.length; j++) {
            if (frames[j].htmlName === "duplicate-frame")
                duplicates.push(frames[j])
        }
        compare(duplicates.length, 2)
        compare(String(duplicates[0].url), String(duplicates[1].url),
                "duplicate frame metadata must collide for the geometry test")
        compare(duplicates[0].size.width, duplicates[1].size.width)
        compare(duplicates[0].size.height, duplicates[1].size.height)
        var duplicateHandlesDistinct = duplicates[0] !== duplicates[1]
        verify(duplicateHandlesDistinct,
               "QWebEngineFrame value identity should distinguish duplicate children")
        var hasFramePosition = typeof duplicates[0].position !== "undefined"
        verify(!hasFramePosition,
               "the public webEngineFrame value type should not invent an offset property")

        var isolated = evaluateFrame(duplicates[0],
            "(() => { const cap=4; const buttons=document.querySelectorAll('button'); " +
            "return {accessible:document.documentElement!==null,worldIsolated:typeof window.__s23PageWorldMarker==='undefined', " +
            "crossOriginFrameElementUnavailable:window.frameElement===null, " +
            "count:Math.min(buttons.length,cap),truncated:buttons.length>cap}; })()")
        verify(isolated.accessible, "native-selected child frame should expose its own DOM in ApplicationWorld")
        verify(isolated.worldIsolated, "ApplicationWorld must not share page JavaScript globals")
        verify(isolated.crossOriginFrameElementUnavailable,
               "cross-origin child must not obtain its parent iframe element for geometry")
        compare(isolated.count, 4)
        verify(isolated.truncated, "provider result must demonstrate a bounded count")

        var sandboxFrame = frameByHtmlName("sandboxed-frame")
        verify(sandboxFrame !== null && String(sandboxFrame.url).indexOf("localhost") >= 0,
               "native frame tree should identify the opaque sandbox frame")
        var sandboxResult = evaluateFrame(sandboxFrame,
            "({accessible:document.documentElement!==null,worldIsolated:typeof window.__s23PageWorldMarker==='undefined'})")
        verify(sandboxResult.accessible && sandboxResult.worldIsolated,
               "browser-owned ApplicationWorld should be isolated and frame-local for opaque sandbox")

        var navigatingFrame = frameByHtmlName("navigating-frame")
        verify(navigatingFrame !== null, "navigation control frame missing")
        compare(evaluateFrame(navigatingFrame,
            "(globalThis.__s23DocumentEpoch='old-document')"), "old-document")
        evaluatePage("document.getElementById('navigating').src='http://localhost:8766/frame-provider-next.html'",
                     WebEngineScript.MainWorld)
        tryVerify(function() {
            return navigatingFrame.isValid && String(navigatingFrame.url).indexOf("frame-provider-next.html") >= 0
        }, 10000, "child navigation did not reach the same captured public frame handle")
        verify(evaluateFrame(navigatingFrame, "typeof globalThis.__s23DocumentEpoch") === "undefined",
               "the isolated-world document marker must reset after navigation")

        var removedFrame = frameByHtmlName("removable-frame")
        verify(removedFrame !== null && removedFrame.isValid, "removal control frame missing")
        evaluatePage("document.getElementById('removable').remove()", WebEngineScript.MainWorld)
        tryVerify(function() { return !removedFrame.isValid }, 10000,
                  "captured QWebEngineFrame did not become invalid after destruction")
        verify(!directFrames().some(function(frame) { return frame.htmlName === "removable-frame" }),
               "destroyed frame remained in a fresh child enumeration")

        console.log("S23_T035_T052_RESULT " + JSON.stringify({
            parentBoundary: true,
            independentApplicationWorld: true,
            sandboxFrameAccessibleByBrowserProvider: true,
            duplicateMetadata: true,
            duplicateHandlesDistinct: duplicateHandlesDistinct,
            duplicateOuterXDistinct: parentBoundary.duplicateRectsDistinct,
            framePositionAvailable: hasFramePosition,
            navigationEpochChanged: true,
            destroyedHandleInvalidated: true,
            callbackReturned: callbackDone
        }))
    }
}
