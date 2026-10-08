import QtQuick
import QtQuick.Controls
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts

// This probe uses real WebEngineView instances and an off-the-record profile.
// It is a primitive qualification probe, not a production stash implementation.
TestCase {
    id: probe
    name: "VersioningLifecycleProbe"
    when: windowShown
    width: 500
    height: 350

    ApplicationWindow {
        id: testWindow
        width: 500
        height: 350
        visible: true
    }

    WebEngineProfilePrototype {
        id: disposableProfilePrototype
        storageName: ""
        persistentPermissionsPolicy: WebEngineProfile.AskEveryTime
    }

    property var disposableProfile: null

    function initTestCase() {
        disposableProfile = disposableProfilePrototype.instance()
        verify(disposableProfile, "the disposable profile instance must exist")
    }

    Component {
        id: hostFactory
        ApplicationWindow {
            width: 500
            height: 350
            visible: true
        }
    }

    Component {
        id: viewFactory
        WebEngineView {
            profile: probe.disposableProfile
            width: 480
            height: 320
            visible: true
            property bool loaded: false
            property string loadFailure: ""
            property int permissionRequestCount: 0
            onLoadingChanged: function(request) {
                if (request.status === WebEngineView.LoadSucceededStatus) {
                    loaded = true
                } else if (request.status === WebEngineView.LoadFailedStatus) {
                    loadFailure = request.errorString
                }
            }
            onPermissionRequested: function(permissionRequest) {
                permissionRequestCount += 1
                permissionRequest.deny()
            }
        }
    }

    function fixtureUrl() {
        var html = "<!doctype html><html><body>"
                + "<textarea id='draft'>initial</textarea>"
                + "<script>window.fixtureLoaded = true;</script>"
                + "</body></html>"
        return "data:text/html;charset=utf-8," + encodeURIComponent(html)
    }

    function loadView(parent) {
        var view = createTemporaryObject(viewFactory, parent)
        verify(view, "a real WebEngineView must be created")
        compare(view.profile, disposableProfile)
        view.url = fixtureUrl()
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        return view
    }

    function evaluate(view, script) {
        var answer = { done: false, value: null }
        view.runJavaScript(script, function(value) {
            answer.value = value
            answer.done = true
        })
        tryVerify(function() { return answer.done }, 10000)
        return answer.value
    }

    function test_retain_same_instance_across_host_close_and_resume() {
        var source = createTemporaryObject(hostFactory, null)
        var retention = createTemporaryObject(hostFactory, null)
        verify(source && retention)
        // Construct under the long-lived retention owner, then display in
        // the source host. A view created under source is destroyed with that
        // window even if its visual parent is later moved.
        var view = loadView(retention.contentItem)
        view.parent = source.contentItem
        compare(evaluate(view,
            "window.sentinel={id:'synthetic',freezes:0,resumes:0,ticks:0};"
            + "window.sameSentinel=window.sentinel;"
            + "document.querySelector('#draft').value='unsaved synthetic text';"
            + "document.addEventListener('freeze',()=>{window.sentinel.freezes++});"
            + "document.addEventListener('resume',()=>{window.sentinel.resumes++});"
            + "window.timer=setInterval(()=>{window.sentinel.ticks++},50);"
            + "window.sentinel.id"), "synthetic")
        compare(evaluate(view, BrowserScripts.formStateProbe()), "dirty")
        wait(120)
        verify(evaluate(view, "window.sentinel.ticks") > 0)

        view.visible = false
        tryCompare(view, "recommendedState", WebEngineView.LifecycleState.Frozen, 5000)
        view.lifecycleState = WebEngineView.LifecycleState.Frozen
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Frozen, 5000)

        // A QML variable alone cannot keep a QObject alive. Move native
        // parentage before destroying the original host.
        view.parent = retention.contentItem
        source.destroy()
        wait(100)
        verify(view)
        view.lifecycleState = WebEngineView.LifecycleState.Active
        view.visible = true
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)

        var result = evaluate(view,
            "JSON.stringify({same:window.sentinel===window.sameSentinel,"
            + "id:window.sentinel.id,freezes:window.sentinel.freezes,"
            + "resumes:window.sentinel.resumes,ticks:window.sentinel.ticks,"
            + "draft:document.querySelector('#draft').value})")
        var state = JSON.parse(result)
        verify(state.same)
        compare(state.id, "synthetic")
        compare(state.draft, "unsaved synthetic text")
        verify(state.freezes > 0, "freeze handler must be able to mutate state")
        verify(state.resumes > 0, "resume handler must be able to mutate state")

        var replacement = loadView(retention.contentItem)
        compare(evaluate(replacement, "typeof window.sentinel"), "undefined")
        // Resume continues the same instance. It does not restore a fixed
        // historical state or guarantee that time stopped while frozen.
        wait(120)
        verify(evaluate(view, "window.sentinel.ticks") > state.ticks)
    }

    function test_visible_view_refuses_frozen_lifecycle() {
        var view = loadView(testWindow.contentItem)
        compare(view.recommendedState, WebEngineView.LifecycleState.Active)
        view.lifecycleState = WebEngineView.LifecycleState.Frozen
        wait(100)
        compare(view.lifecycleState, WebEngineView.LifecycleState.Active)
    }

    function test_unknown_editor_state_is_not_classified_safe() {
        var view = loadView(testWindow.contentItem)
        compare(evaluate(view,
            "var editor=document.createElement('div');"
            + "editor.contentEditable='true';document.body.appendChild(editor);"
            + "editor.textContent='synthetic edit';'ready'"), "ready")
        compare(evaluate(view, BrowserScripts.formStateProbe()), "unknown")
    }

    function test_connected_data_channel_does_not_get_assumed_idle() {
        var view = loadView(testWindow.contentItem)
        var setup = "window.rtcState='starting';"
                + "window.rtcA=new RTCPeerConnection({iceServers:[]});"
                + "window.rtcB=new RTCPeerConnection({iceServers:[]});"
                + "window.rtcA.onicecandidate=e=>{if(e.candidate)window.rtcB.addIceCandidate(e.candidate)};"
                + "window.rtcB.onicecandidate=e=>{if(e.candidate)window.rtcA.addIceCandidate(e.candidate)};"
                + "window.rtcChannel=window.rtcA.createDataChannel('synthetic');"
                + "window.rtcChannel.onopen=()=>{window.rtcState='open'};"
                + "window.rtcB.ondatachannel=e=>{window.rtcRemote=e.channel};"
                + "(async()=>{let offer=await rtcA.createOffer();"
                + "await rtcA.setLocalDescription(offer);await rtcB.setRemoteDescription(offer);"
                + "let answer=await rtcB.createAnswer();await rtcB.setLocalDescription(answer);"
                + "await rtcA.setRemoteDescription(answer)})().catch(e=>window.rtcState='error');"
                + "window.rtcState"
        compare(evaluate(view, setup), "starting")
        tryVerify(function() {
            return evaluate(view, "window.rtcState") === "open"
        }, 15000)
        view.visible = false
        // Qt's aggregate lifecycle recommendation remains Frozen even with a
        // connected WebRTC data channel. It cannot prove live-call absence.
        tryCompare(view, "recommendedState", WebEngineView.LifecycleState.Frozen, 5000)
        compare(evaluate(view, "window.rtcChannel.readyState"), "open")
        compare(view.permissionRequestCount, 0,
                "a connected data-only channel gives no permission-request witness")
        evaluate(view, "rtcA.close();rtcB.close();'closed'")
    }
}
