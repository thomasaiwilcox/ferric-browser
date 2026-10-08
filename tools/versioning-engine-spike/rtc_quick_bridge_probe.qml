import QtQuick 2.15
import QtQuick.Window 2.15
import QtWebEngine 6.8

Window {
    id: root
    width: 640
    height: 480
    visible: true

    property int result: 2
    property int phase: 0
    property bool invalidationObserved: false
    property string failureReason: ""
    property bool cameraPollInFlight: false
    property bool cameraCleanupProbeInFlight: false
    property bool cameraPermissionObserved: false
    property int cameraFramePolls: 0
    property int cameraCleanupPolls: 0
    property bool lifecycleExitInvalidationObserved: false
    property bool lifecycleAcquireInvalidationObserved: false
    property bool lifecycleAcquireRefusalObserved: false
    property bool profileReplacementInvalidationObserved: false
    property int lifecyclePolls: 0
    property bool microphoneDispatchAcknowledged: false
    property bool microphonePermissionObserved: false
    property bool microphonePendingProbePassed: false
    property bool microphonePollInFlight: false
    property bool microphoneCleanupProbeInFlight: false
    property int microphonePolls: 0
    property int microphoneCleanupPolls: 0
    property var pendingMicrophonePermission: null
    property bool combinedDispatchAcknowledged: false
    property bool combinedPermissionObserved: false
    property bool combinedPendingProbePassed: false
    property bool combinedPollInFlight: false
    property bool combinedCleanupProbeInFlight: false
    property int combinedPolls: 0
    property int combinedCleanupPolls: 0
    property var pendingCombinedPermission: null
    property bool rendererTerminationSignalled: false
    property string rendererTerminationReason: ""
    property bool rendererTerminatedObserved: false
    property int rendererInvalidationCount: 0
    property bool foreignThreadGuardProbePassed: false
    property bool enableQuickMediaPermissionRoutes: false
    property int probeCompletionCount: 0
    property bool crossFramePeerPollInFlight: false
    property bool crossFrameCaptureInvalidationObserved: false
    property bool crossFrameCapturePollInFlight: false
    property bool crossFrameCaptureCleanupPollInFlight: false
    property bool crossFrameCapturePermissionObserved: false
    property bool crossFrameCaptureTrackObserved: false
    property string lastPermissionSignalTrace: ""
    property var crossFrameCaptureFrame: null
    property int crossFrameCapturePolls: 0
    property int certificateErrorCount: 0
    property string lastCertificateErrorUrl: ""
    property string lastCertificateErrorDetail: ""

    signal finished(int result)
    function grantMicrophoneAfterPendingProof() {
        if (root.phase !== 8 || !root.microphonePendingProbePassed
                || !root.microphoneDispatchAcknowledged
                || !root.pendingMicrophonePermission)
            return
        root.pendingMicrophonePermission.grant()
        root.pendingMicrophonePermission = null
        root.phase = 9
        root.microphonePolls = 0
        cameraWaitTimer.start()
    }

    function grantCombinedAfterPendingProof() {
        if (root.phase !== 23 || !root.combinedPendingProbePassed
                || !root.combinedDispatchAcknowledged
                || !root.pendingCombinedPermission)
            return
        root.pendingCombinedPermission.grant()
        root.pendingCombinedPermission = null
        root.phase = 24
        root.combinedPolls = 0
        cameraWaitTimer.start()
    }

    function startMicrophoneProbe() {
        root.phase = 7
        root.invalidationObserved = false
        root.microphoneDispatchAcknowledged = false
        root.microphonePermissionObserved = false
        root.microphonePendingProbePassed = false
        root.microphonePolls = 0
        cameraWaitTimer.start()
        view.visible = true
        Qt.callLater(function() {
            if (!root.invalidationObserved
                    || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
                root.finish(false, "visible Qt Quick transition did not revoke the held lease")
                return
            }
            view.runJavaScript(
                "navigator.mediaDevices.getUserMedia({audio:true,video:false})"
                + ".then(stream=>{window.ferricMicrophoneStream=stream;"
                + "window.ferricMicrophoneTrack=stream.getAudioTracks()[0];"
                + "window.ferricMicrophoneStarted=!!window.ferricMicrophoneTrack&&"
                + "window.ferricMicrophoneTrack.kind==='audio'&&"
                + "window.ferricMicrophoneTrack.readyState==='live';"
                + "}).catch(error=>{window.ferricMicrophoneError=String(error);});true",
                WebEngineScript.ApplicationWorld,
                function(value) {
                    if (value !== true) {
                        root.finish(false, "Qt Quick microphone script dispatch was not acknowledged")
                        return
                    }
                    root.microphoneDispatchAcknowledged = true
                    root.grantMicrophoneAfterPendingProof()
                })
        })
    }

    function startDataOnlyPeerProbe() {
        root.phase = 12
        root.lifecyclePolls = 0
        root.invalidationObserved = false
        cameraWaitTimer.start()
        view.runJavaScript(
            "window.peer = new RTCPeerConnection({iceServers: []});"
            + "window.channel = peer.createDataChannel('ferric-rtc-quick-probe');"
            + "peer.signalingState === 'stable' && channel.readyState === 'connecting'",
            WebEngineScript.ApplicationWorld,
            function(value) {
                if (value !== true || !root.invalidationObserved) {
                    root.finish(false, "Qt Quick peer dispatch did not revoke the held lease first")
                    return
                }
                root.phase = 13
                Qt.callLater(function() {
                    if (root.phase === 13 && !view.ferricAcquireRtcEmptyLeaseForProbe())
                        root.finish(false, "post-peer observation request was not admitted")
                })
            })
    }

    function startRendererLossProbe() {
        root.phase = 29
        root.rendererTerminationSignalled = false
        root.rendererTerminatedObserved = false
        root.rendererInvalidationCount = 0
        root.lifecyclePolls = 0
        cameraWaitTimer.start()
    }

    function finish(success, reason) {
        if (root.result !== 2)
            return
        root.failureReason = reason
        if (!success)
            console.error("RTC Qt Quick probe failed: " + reason)
        else
            console.log("RTC Qt Quick probe passed: " + reason)
        cameraWaitTimer.stop()
        if (root.pendingMicrophonePermission) {
            root.pendingMicrophonePermission.deny()
            root.pendingMicrophonePermission = null
        }
        if (root.pendingCombinedPermission) {
            root.pendingCombinedPermission.deny()
            root.pendingCombinedPermission = null
        }
        view.ferricEndChildFrameCaptureProbe()
        view.ferricReleaseRtcEmptyLeaseForProbe()
        root.result = success ? 0 : 1
        finishTimer.start()
    }

    Timer {
        id: finishTimer
        interval: 50
        onTriggered: {
            root.finished(root.result)
            root.close()
        }
    }

    Timer {
        id: cameraWaitTimer
        interval: 100
        repeat: true
        onTriggered: {
            if (root.phase === 2) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "visible Qt Quick owner did not revoke its Frozen lease")
                    return
                }
                if (view.visible && view.lifecycleState === WebEngineView.LifecycleState.Active
                        && root.lifecycleExitInvalidationObserved
                        && !view.ferricRtcEmptyLeaseValidForProbe()) {
                    cameraWaitTimer.stop()
                    root.phase = 14
                    root.lifecyclePolls = 0
                    view.visible = false
                    view.lifecycleState = WebEngineView.LifecycleState.Frozen
                    cameraWaitTimer.start()
                }
                return
            }

            if (root.phase === 30) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "foreign-thread bridge guard probe did not complete")
                    return
                }
                if (!root.foreignThreadGuardProbePassed)
                    return
                cameraWaitTimer.stop()
                root.phase = 2
                root.lifecycleExitInvalidationObserved = false
                root.lifecyclePolls = 0
                view.visible = true
                cameraWaitTimer.start()
                return
            }

            if (root.phase === 7) {
                if (++root.microphonePolls >= 200) {
                    root.microphonePolls = 0
                    view.runJavaScript(
                        "JSON.stringify({started:window.ferricMicrophoneStarted,"
                        + "permissionError:window.ferricMicrophoneError,"
                        + "track:window.ferricMicrophoneTrack&&window.ferricMicrophoneTrack.readyState})",
                        WebEngineScript.ApplicationWorld,
                        function(diagnostic) {
                            root.finish(false, "fake microphone permission did not arrive: "
                                        + diagnostic + " callback="
                                        + root.microphoneDispatchAcknowledged)
                        })
                }
                return
            }

            if (root.phase === 14) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "hidden Qt Quick owner did not return to Frozen")
                    return
                }
                if (!view.visible && view.lifecycleState === WebEngineView.LifecycleState.Frozen) {
                    cameraWaitTimer.stop()
                    root.phase = 15
                    if (!view.ferricAcquireRtcEmptyLeaseForProbe()) {
                        root.finish(false, "fresh post-lifecycle lease request was not admitted")
                        return
                    }
                }
                return
            }

            if (root.phase === 16) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "in-flight exact-view lease was not revoked on lifecycle exit")
                    return
                }
                if (view.visible && view.lifecycleState === WebEngineView.LifecycleState.Active
                        && (root.lifecycleAcquireInvalidationObserved
                            || root.lifecycleAcquireRefusalObserved)
                        && !view.ferricRtcEmptyLeaseValidForProbe()) {
                    cameraWaitTimer.stop()
                    root.phase = 17
                    root.lifecyclePolls = 0
                    view.visible = false
                    view.lifecycleState = WebEngineView.LifecycleState.Frozen
                    cameraWaitTimer.start()
                }
                return
            }

            if (root.phase === 17) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "exact view did not return to Frozen after racing acquisition")
                    return
                }
                if (!view.visible && view.lifecycleState === WebEngineView.LifecycleState.Frozen) {
                    cameraWaitTimer.stop()
                    root.phase = 18
                    if (!view.ferricAcquireRtcEmptyLeaseForProbe()) {
                        root.finish(false, "fresh post-race lease request was not admitted")
                        return
                    }
                }
                return
            }

            if (root.phase === 19) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "profile adapter replacement did not reload/revoke: profile="
                                + (view.profile === replacementProfile)
                                + " loading=" + view.loading + " url=" + view.url
                                + " invalidated=" + root.profileReplacementInvalidationObserved
                                + " valid=" + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                return
            }

            if (root.phase === 20) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "replacement-profile view did not return to Frozen")
                    return
                }
                if (view.profile === replacementProfile && !view.visible
                        && view.lifecycleState === WebEngineView.LifecycleState.Frozen) {
                    if (!root.profileReplacementInvalidationObserved
                            || view.ferricRtcEmptyLeaseValidForProbe()) {
                        root.finish(false, "profile replacement retained or failed to revoke its old lease")
                        return
                    }
                    cameraWaitTimer.stop()
                    root.phase = 21
                    if (!view.ferricAcquireRtcEmptyLeaseForProbe()) {
                        root.finish(false, "replacement-profile lease request was not admitted")
                        return
                    }
                }
                return
            }

            if (root.phase === 27) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "closed data-only peer did not release registry ownership")
                    return
                }
                if (!view.visible && view.lifecycleState === WebEngineView.LifecycleState.Frozen) {
                    cameraWaitTimer.stop()
                    root.phase = 28
                    if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                        root.finish(false, "post-peer-close exact-view lease request was not admitted")
                }
                return
            }

            if (root.phase === 32) {
                if (++root.lifecyclePolls >= 200)
                    root.finish(false, "same-view reload after peer close did not complete")
                return
            }

            if (root.phase === 33) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "reloaded exact view did not return to Frozen")
                    return
                }
                if (!view.visible && view.lifecycleState === WebEngineView.LifecycleState.Frozen) {
                    cameraWaitTimer.stop()
                    root.phase = 34
                    if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                        root.finish(false, "post-context-destruction lease request was not admitted")
                    else
                        cameraWaitTimer.start()
                }
                return
            }

            if (root.phase === 34) {
                if (++root.lifecyclePolls >= 200)
                    root.finish(false, "post-context-destruction registry probe did not complete")
                return
            }

            if (root.phase === 35) {
                if (++root.lifecyclePolls >= 50) {
                    var frameDetails = []
                    for (var index = 0; index < view.mainFrame.children.length; ++index)
                        frameDetails.push(String(view.mainFrame.children[index].url))
                    root.finish(false, "cross-origin child frame did not appear in the QML frame tree: "
                                + JSON.stringify(frameDetails)
                                + " certificateErrors=" + root.certificateErrorCount
                                + " lastCertificateError=" + root.lastCertificateErrorUrl
                                + " detail=" + root.lastCertificateErrorDetail)
                    return
                }
                if (root.crossFramePeerPollInFlight)
                    return
                var childFrames = view.mainFrame.children
                for (var childIndex = 0; childIndex < childFrames.length; ++childIndex) {
                    var childFrame = childFrames[childIndex]
                    var childUrl = String(childFrame.url)
                    if (!childFrame.isValid || childUrl.indexOf('https://top-a.localhost:') !== 0)
                        continue
                    root.crossFramePeerPollInFlight = true
                    childFrame.runJavaScript(
                        "try {"
                        + "window.ferricCrossFramePeer = new RTCPeerConnection({iceServers: []});"
                        + "window.ferricCrossFrameChannel = "
                        + "window.ferricCrossFramePeer.createDataChannel('ferric-cross-frame-application-probe');"
                        + "'ready';"
                        + "} catch (error) { 'error:' + String(error); }",
                        WebEngineScript.ApplicationWorld,
                        function(value) {
                            root.crossFramePeerPollInFlight = false
                            if (root.phase !== 35)
                                return
                            if (value !== 'ready') {
                                root.finish(false, "cross-origin ApplicationWorld peer creation failed: " + value)
                                return
                            }
                            cameraWaitTimer.stop()
                            root.phase = 36
                            root.lifecyclePolls = 0
                            view.visible = false
                            view.lifecycleState = WebEngineView.LifecycleState.Frozen
                            cameraWaitTimer.start()
                        })
                    return
                }
                return
            }

            if (root.phase === 36) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "cross-origin child view did not return to Frozen")
                    return
                }
                if (!view.visible && view.lifecycleState === WebEngineView.LifecycleState.Frozen) {
                    cameraWaitTimer.stop()
                    root.phase = 37
                    root.lifecyclePolls = 0
                    if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                        root.finish(false, "cross-origin child peer registry probe was not admitted")
                }
                return
            }

            if (root.phase === 37) {
                if (++root.lifecyclePolls >= 200)
                    root.finish(false, "cross-origin child peer registry probe did not complete")
                return
            }

            if (root.phase === 38) {
                if (++root.lifecyclePolls >= 200)
                    root.finish(false, "clean reload after cross-origin child peer did not complete")
                return
            }

            if (root.phase === 39) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "cleanly reloaded cross-origin view did not return to Frozen")
                    return
                }
                if (!view.visible && view.lifecycleState === WebEngineView.LifecycleState.Frozen) {
                    cameraWaitTimer.stop()
                    root.phase = 40
                    root.lifecyclePolls = 0
                    if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                        root.finish(false, "post-cross-frame context-destruction lease was not admitted")
                }
                return
            }

            if (root.phase === 47) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "cross-origin capture view did not become Active before navigation")
                    return
                }
                if (view.visible && view.lifecycleState === WebEngineView.LifecycleState.Active) {
                    cameraWaitTimer.stop()
                    root.requestActivate()
                    view.forceActiveFocus()
                    root.phase = 41
                    root.crossFrameCapturePolls = 0
                    view.url = ferricCrossCaptureFixtureUrl
                    cameraWaitTimer.start()
                }
                return
            }

            if (root.phase === 41) {
                if (++root.crossFrameCapturePolls >= 100) {
                    root.finish(false, "capture child frame did not appear for capture-state probe")
                    return
                }
                var captureFrames = view.mainFrame.children
                for (var captureIndex = 0; captureIndex < captureFrames.length; ++captureIndex) {
                    var captureFrame = captureFrames[captureIndex]
                    var expectedCaptureChildOrigin = ferricSameOriginCaptureProbe
                            ? 'https://cookie.localhost:' : 'https://top-a.localhost:'
                    if (!captureFrame.isValid
                            || String(captureFrame.url).indexOf(expectedCaptureChildOrigin) !== 0)
                        continue
                    cameraWaitTimer.stop()
                    root.crossFrameCaptureFrame = captureFrame
                    root.crossFrameCapturePermissionObserved = false
                    root.crossFrameCapturePolls = 0
                    root.phase = 42
                    cameraWaitTimer.start()
                    return
                }
                return
            }

            if (root.phase === 42) {
                if (root.crossFrameCapturePollInFlight)
                    return
                if (++root.crossFrameCapturePolls >= 200) {
                    root.finish(false, "active cross-origin child fake-camera track did not start; permission="
                                + root.crossFrameCapturePermissionObserved)
                    return
                }
                if (!root.crossFrameCaptureFrame || !root.crossFrameCaptureFrame.isValid) {
                    root.finish(false, "cross-origin capture frame became invalid during getUserMedia")
                    return
                }
                if (!view.visible || view.lifecycleState !== WebEngineView.LifecycleState.Active)
                    return
                root.crossFrameCapturePollInFlight = true
                root.crossFrameCaptureFrame.runJavaScript(
                    "(()=>{"
                    + "if(!window.ferricCrossCaptureRequestStarted){"
                    + "if(document.readyState!=='complete'||!isSecureContext"
                    + "||document.visibilityState!=='visible'||!navigator.mediaDevices)"
                    + "return 'waiting:'+document.readyState+':'+document.visibilityState"
                    + "+':secure='+isSecureContext+':media='+!!navigator.mediaDevices;"
                    + "window.ferricCrossCaptureRequestStarted=true;"
                    + "try{navigator.mediaDevices.getUserMedia({video:true,audio:false})"
                    + ".then(stream=>{window.ferricCrossCaptureStream=stream;"
                    + "window.ferricCrossCaptureTrack=stream.getVideoTracks()[0];"
                    + "window.ferricCrossCaptureStarted=true;})"
                    + ".catch(error=>{window.ferricCrossCaptureError=error.name+':'+error.message"
                    + "+' state='+document.readyState+' visibility='+document.visibilityState"
                    + "+' secure='+isSecureContext+' cameraPolicy='+"
                    + "!!((document.permissionsPolicy||document.featurePolicy)&&"
                    + "(document.permissionsPolicy||document.featurePolicy).allowsFeature('camera'));"
                    + "});return 'requested';}"
                    + "catch(error){window.ferricCrossCaptureError=String(error);"
                    + "return 'error:'+window.ferricCrossCaptureError;}}"
                    + "if(window.ferricCrossCaptureError)return 'error:'+window.ferricCrossCaptureError;"
                    + "return window.ferricCrossCaptureStarted===true&&window.ferricCrossCaptureTrack"
                    + "&&window.ferricCrossCaptureTrack.readyState==='live'?'live':'pending';})()",
                    WebEngineScript.ApplicationWorld,
                    function(value) {
                        root.crossFrameCapturePollInFlight = false
                        if (root.phase !== 42)
                            return
                        if (String(value).indexOf('error:') === 0) {
                            root.finish(false, "cross-origin child getUserMedia failed: " + value
                                        + " permission=" + root.crossFrameCapturePermissionObserved
                                        + " lastPermissionSignal=" + root.lastPermissionSignalTrace
                                        + " frames=" + view.ferricFrameActivationSnapshotForProbe())
                            return
                        }
                        if (value !== 'live')
                            return
                        root.crossFrameCaptureTrackObserved = true
                        if (!root.crossFrameCapturePermissionObserved
                                && !ferricForcedChildPermissionProbe) {
                            root.finish(false, "cross-origin fake camera bypassed the Qt permission callback")
                            return
                        }
                        if (!view.ferricHasFrameCaptureActivityForProbe()
                                || view.ferricHasNoMediaCaptureActivityForProbe())
                            return
                        cameraWaitTimer.stop()
                        root.phase = 43
                        root.crossFrameCapturePolls = 0
                        root.crossFrameCaptureFrame.runJavaScript(
                            "window.ferricCrossCaptureTrack.stop();'stopped';",
                            WebEngineScript.ApplicationWorld,
                            function(stopResult) {
                                if (root.phase !== 43)
                                    return
                                if (stopResult !== 'stopped') {
                                    root.finish(false, "cross-origin fake-camera track stop was not dispatched")
                                    return
                                }
                                cameraWaitTimer.start()
                            })
                    })
                return
            }

            if (root.phase === 43) {
                if (root.crossFrameCaptureCleanupPollInFlight)
                    return
                if (++root.crossFrameCapturePolls >= 200) {
                    root.finish(false, "stopped cross-origin fake-camera track did not clear capture state")
                    return
                }
                if (!root.crossFrameCaptureFrame || !root.crossFrameCaptureFrame.isValid) {
                    root.finish(false, "cross-origin capture frame became invalid during track cleanup")
                    return
                }
                root.crossFrameCaptureCleanupPollInFlight = true
                root.crossFrameCaptureFrame.runJavaScript(
                    "window.ferricCrossCaptureTrack&&window.ferricCrossCaptureTrack.readyState==='ended'",
                    WebEngineScript.ApplicationWorld,
                    function(ended) {
                        root.crossFrameCaptureCleanupPollInFlight = false
                        if (root.phase !== 43 || ended !== true)
                            return
                        if (view.ferricHasFrameCaptureActivityForProbe()
                                || !view.ferricHasNoMediaCaptureActivityForProbe())
                            return
                        cameraWaitTimer.stop()
                        root.phase = 44
                        root.lifecyclePolls = 0
                        view.visible = false
                        view.lifecycleState = WebEngineView.LifecycleState.Frozen
                        cameraWaitTimer.start()
                    })
                return
            }

            if (root.phase === 44) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "cleaned cross-origin capture view did not return to Frozen")
                    return
                }
                if (!view.visible && view.lifecycleState === WebEngineView.LifecycleState.Frozen) {
                    cameraWaitTimer.stop()
                    root.phase = 45
                    root.lifecyclePolls = 0
                    if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                        root.finish(false, "post-capture empty lease was not admitted")
                }
                return
            }

            if (root.phase === 45) {
                if (++root.lifecyclePolls >= 200)
                    root.finish(false, "post-capture empty lease did not complete")
                return
            }

            if (root.phase === 13) {
                if (++root.lifecyclePolls >= 500)
                    root.finish(false, "data-only peer admission probe did not complete; completions="
                                + root.probeCompletionCount)
                return
            }

            if (root.phase === 24) {
                if (root.combinedPollInFlight)
                    return
                root.combinedPollInFlight = true
                view.runJavaScript(
                    "window.ferricCombinedStarted===true&&"
                    + "window.ferricCombinedAudioTrack&&"
                    + "window.ferricCombinedAudioTrack.readyState==='live'&&"
                    + "window.ferricCombinedVideoTrack&&"
                    + "window.ferricCombinedVideoTrack.readyState==='live'&&"
                    + "window.ferricCombinedFrame===true",
                    function(value) {
                        root.combinedPollInFlight = false
                        if (root.phase !== 24)
                            return
                        if (value === true) {
                            if (!root.combinedPermissionObserved) {
                                root.finish(false, "combined fake capture bypassed the Qt Quick permission callback")
                                return
                            }
                            cameraWaitTimer.stop()
                            root.phase = 25
                            if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                                root.finish(false, "active combined-capture refusal probe was not admitted")
                        } else if (++root.combinedPolls >= 200) {
                            root.finish(false, "combined fake capture did not deliver live audio/video tracks and a video frame")
                        }
                    })
                return
            }

            if (root.phase === 26) {
                if (root.combinedCleanupProbeInFlight)
                    return
                if (++root.combinedCleanupPolls >= 200) {
                    root.finish(false, "stopped combined fake capture did not release its activity interlock")
                    return
                }
                root.combinedCleanupProbeInFlight = true
                if (!view.ferricAcquireRtcEmptyLeaseForProbe()) {
                    root.combinedCleanupProbeInFlight = false
                    root.finish(false, "combined-capture cleanup probe was not admitted")
                }
                return
            }

            if (root.phase === 29) {
                if (++root.lifecyclePolls >= 200) {
                    root.finish(false, "renderer loss did not invalidate the exact Qt Quick owner lease: signalled="
                                + root.rendererTerminationSignalled + " observed="
                                + root.rendererTerminatedObserved + " invalidations="
                                + root.rendererInvalidationCount + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe() + " refusal="
                                + root.rendererTerminationReason)
                    return
                }
                if (root.rendererInvalidationCount > 1) {
                    root.finish(false, "renderer loss invalidated the exact-view lease more than once")
                    return
                }
                if (root.rendererTerminationSignalled && root.rendererTerminatedObserved
                        && root.rendererInvalidationCount === 1
                        && !view.ferricRtcEmptyLeaseValidForProbe())
                    root.finish(true, "exact Qt Quick renderer loss invalidated its retained empty-registry lease; childCaptureTrack="
                                + root.crossFrameCaptureTrackObserved + " childPermissionCallback="
                                + root.crossFrameCapturePermissionObserved)
                return
            }

            if (root.phase === 3) {
                if (root.cameraPollInFlight)
                    return
                root.cameraPollInFlight = true
                view.runJavaScript(
                    "window.ferricCameraStarted===true&&"
                    + "window.ferricCameraTrack&&"
                    + "window.ferricCameraTrack.readyState==='live'&&"
                    + "window.ferricCameraFrame===true",
                    function(value) {
                        root.cameraPollInFlight = false
                        if (root.phase !== 3)
                            return
                        if (value === true) {
                            if (!root.cameraPermissionObserved) {
                                root.finish(false, "fake camera bypassed the Qt Quick permission callback")
                                return
                            }
                            cameraWaitTimer.stop()
                            root.phase = 4
                            if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                                root.finish(false, "active fake-camera refusal probe was not admitted")
                        } else if (++root.cameraFramePolls >= 200) {
                            view.runJavaScript(
                                "JSON.stringify({started:window.ferricCameraStarted,"
                                + "permissionError:window.ferricCameraError,"
                                + "frameError:window.ferricCameraFrameError,"
                                + "frame:window.ferricCameraFrame,"
                                + "track:window.ferricCameraTrack&&window.ferricCameraTrack.readyState})",
                                function(diagnostic) {
                                    root.finish(false, "fake camera did not deliver a track frame: "
                                                + diagnostic)
                                })
                        }
                    })
                return
            }

            if (root.phase === 9) {
                if (root.microphonePollInFlight)
                    return
                root.microphonePollInFlight = true
                view.runJavaScript(
                    "window.ferricMicrophoneStarted===true&&"
                    + "window.ferricMicrophoneTrack&&"
                    + "window.ferricMicrophoneTrack.readyState==='live'",
                    WebEngineScript.ApplicationWorld,
                    function(value) {
                        root.microphonePollInFlight = false
                        if (root.phase !== 9)
                            return
                        if (value === true) {
                            if (!root.microphonePermissionObserved) {
                                root.finish(false, "fake microphone bypassed the Qt Quick permission callback")
                                return
                            }
                            cameraWaitTimer.stop()
                            root.phase = 10
                            if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                                root.finish(false, "active fake-microphone refusal probe was not admitted")
                        } else if (++root.microphonePolls >= 200) {
                            root.finish(false, "fake microphone did not start a live track")
                        }
                    })
                return
            }

            if (root.phase === 5) {
                if (root.cameraCleanupProbeInFlight)
                    return
                if (++root.cameraCleanupPolls >= 200) {
                    root.finish(false, "stopped fake camera did not release its activity interlock")
                    return
                }
                root.cameraCleanupProbeInFlight = true
                if (!view.ferricAcquireRtcEmptyLeaseForProbe()) {
                    root.cameraCleanupProbeInFlight = false
                    root.finish(false, "camera cleanup probe was not admitted")
                }
                return
            }

            if (root.phase === 11) {
                if (root.microphoneCleanupProbeInFlight)
                    return
                if (++root.microphoneCleanupPolls >= 200) {
                    root.finish(false, "stopped fake microphone did not release its activity interlock")
                    return
                }
                root.microphoneCleanupProbeInFlight = true
                if (!view.ferricAcquireRtcEmptyLeaseForProbe()) {
                    root.microphoneCleanupProbeInFlight = false
                    root.finish(false, "microphone cleanup probe was not admitted")
                }
                return
            }

            cameraWaitTimer.stop()
        }
    }

    WebEngineProfile {
        id: disposableProfile
        offTheRecord: true
    }

    WebEngineProfile {
        id: replacementProfile
        offTheRecord: true
    }

    WebEngineView {
        id: view
        objectName: "ferricRtcQuickProbeView"
        anchors.fill: parent
        profile: disposableProfile
        url: ferricFixtureUrl

        onCertificateError: function(error) {
            root.certificateErrorCount += 1
            root.lastCertificateErrorUrl = String(error.url)
            root.lastCertificateErrorDetail = "type=" + error.type
                    + " overridable=" + error.overridable
                    + " main=" + error.isMainFrame
            error.acceptCertificate()
        }

        onPermissionRequested: function(permission) {
            root.lastPermissionSignalTrace = "phase=" + root.phase
                    + " type=" + permission.permissionType
                    + " videoEnum=" + WebEnginePermission.PermissionType.MediaVideoCapture
                    + " isVideo="
                    + (permission.permissionType === WebEnginePermission.PermissionType.MediaVideoCapture)
            if (permission.permissionType === WebEnginePermission.PermissionType.MediaAudioCapture) {
                if (root.phase !== 7 || root.pendingMicrophonePermission) {
                    permission.deny()
                    root.finish(false, "unexpected or duplicate fake-microphone permission")
                    return
                }
                root.microphonePermissionObserved = true
                root.pendingMicrophonePermission = permission
                root.phase = 8
                view.visible = false
                view.lifecycleState = WebEngineView.LifecycleState.Frozen
                if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                    root.finish(false, "pending microphone refusal probe was not admitted")
            } else if (permission.permissionType === WebEnginePermission.PermissionType.MediaAudioVideoCapture) {
                if (root.phase !== 22 || root.pendingCombinedPermission) {
                    permission.deny()
                    root.finish(false, "unexpected or duplicate combined fake-capture permission")
                    return
                }
                root.combinedPermissionObserved = true
                root.pendingCombinedPermission = permission
                root.phase = 23
                if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                    root.finish(false, "pending combined-capture refusal probe was not admitted")
            } else if (permission.permissionType === WebEnginePermission.PermissionType.MediaVideoCapture
                       && root.phase === 42 && !root.crossFrameCapturePermissionObserved) {
                root.crossFrameCapturePermissionObserved = true
                permission.grant()
            } else if (permission.permissionType === WebEnginePermission.PermissionType.MediaVideoCapture) {
                root.cameraPermissionObserved = true
                permission.grant()
            } else {
                permission.deny()
            }
        }

        onLoadingChanged: function(load) {
            if (load.status === WebEngineView.LoadSucceededStatus && root.phase === 0) {
                root.phase = 1
                view.visible = false
                Qt.callLater(function() {
                    if (!view.ferricAcquireRtcEmptyLeaseForProbe()) {
                        root.finish(false, "first exact-view lease request was not admitted")
                        return
                    }
                    if (view.ferricAcquireRtcEmptyLeaseForProbe()) {
                        root.finish(false, "overlapping exact-view lease request was admitted")
                    }
                })
            } else if (load.status === WebEngineView.LoadSucceededStatus && root.phase === 19) {
                root.phase = 20
                root.lifecyclePolls = 0
                view.visible = false
                view.lifecycleState = WebEngineView.LifecycleState.Frozen
                cameraWaitTimer.start()
            } else if (load.status === WebEngineView.LoadSucceededStatus && root.phase === 32) {
                root.phase = 33
                root.lifecyclePolls = 0
                view.visible = false
                view.lifecycleState = WebEngineView.LifecycleState.Frozen
                cameraWaitTimer.start()
            } else if (load.status === WebEngineView.LoadSucceededStatus && root.phase === 35) {
                root.lifecyclePolls = 0
                root.crossFramePeerPollInFlight = false
                cameraWaitTimer.start()
            } else if (load.status === WebEngineView.LoadSucceededStatus && root.phase === 38) {
                root.phase = 39
                root.lifecyclePolls = 0
                view.visible = false
                view.lifecycleState = WebEngineView.LifecycleState.Frozen
                cameraWaitTimer.start()
            } else if (load.status === WebEngineView.LoadSucceededStatus && root.phase === 41) {
                cameraWaitTimer.start()
            } else if (load.status === WebEngineView.LoadFailedStatus) {
                root.finish(false, "owned HTTPS page load failed")
            }
        }

        onRenderProcessTerminated: function(terminationStatus, exitCode) {
            if (root.phase === 29)
                root.rendererTerminatedObserved = true
        }

        onFerricRtcEmptyLeaseProbeCompleted: function(known, registered, frames, processes) {
            root.probeCompletionCount += 1
            console.log("Qt Quick lease completion phase=" + root.phase + " known=" + known
                        + " registered=" + registered + " frames=" + frames
                        + " processes=" + processes)
            if (root.phase === 1) {
                if (!known || registered || frames < 1 || processes < 1
                        || !view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Frozen) {
                    root.finish(false, "empty Qt Quick lease mismatch: known=" + known
                                + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState
                                + " visible=" + view.visible)
                    return
                }
                root.foreignThreadGuardProbePassed = false
                root.phase = 30
                root.lifecyclePolls = 0
                cameraWaitTimer.start()
                return
            }

            if (root.phase === 15) {
                if (!known || registered || frames < 1 || processes < 1
                        || !view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Frozen) {
                    root.finish(false, "fresh post-lifecycle lease mismatch: known=" + known
                                + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                cameraWaitTimer.stop()
                root.phase = 16
                root.lifecycleAcquireInvalidationObserved = false
                root.lifecycleAcquireRefusalObserved = false
                root.lifecyclePolls = 0
                view.ferricReleaseRtcEmptyLeaseForProbe()
                if (!view.ferricAcquireRtcEmptyLeaseForProbe()) {
                    root.finish(false, "in-flight lifecycle-race lease request was not admitted")
                    return
                }
                view.visible = true
                cameraWaitTimer.start()
                return
            }

            if (root.phase === 16) {
                if (known || registered || view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
                    root.finish(false, "lease completion escaped the visible-owner guard")
                    return
                }
                if (view.ferricAcquireRtcEmptyLeaseForProbe()) {
                    root.finish(false, "visible exact-view owner admitted another registry lease")
                    return
                }
                root.lifecycleAcquireRefusalObserved = true
                return
            }

            if (root.phase === 23) {
                if (known || registered || frames < 1 || processes < 1
                        || view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
                    root.finish(false, "pending combined fake capture was not fail-closed: known=" + known
                                + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                if (!root.pendingCombinedPermission || root.phase !== 23) {
                    root.finish(false, "pending combined-capture permission identity was lost")
                    return
                }
                root.combinedPendingProbePassed = true
                root.grantCombinedAfterPendingProof()
                return
            }

            if (root.phase === 18) {
                if (!known || registered || frames < 1 || processes < 1
                        || !view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Frozen) {
                    root.finish(false, "fresh post-race lease mismatch: known=" + known
                                + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                cameraWaitTimer.stop()
                root.startDataOnlyPeerProbe()
                return
            }

            if (root.phase === 21) {
                if (!known || registered || frames < 1 || processes < 1
                        || !view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Frozen) {
                    root.finish(false, "replacement-profile lease mismatch: known=" + known
                                + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                cameraWaitTimer.stop()
                root.phase = 35
                root.lifecyclePolls = 0
                root.crossFramePeerPollInFlight = false
                view.url = String(ferricFixtureUrl).replace("/rtc/empty", "/rtc/cross-empty")
                cameraWaitTimer.start()
                return
            }

            if (root.phase === 37) {
                if (!known || !registered || frames < 2
                        || processes < ferricExpectedQuickProcessCount
                        || view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
                    root.finish(false, "cross-origin child peer escaped exact-view ownership: known="
                                + known + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " expectedProcesses="
                                + ferricExpectedQuickProcessCount + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                cameraWaitTimer.stop()
                root.phase = 38
                root.lifecyclePolls = 0
                view.url = ferricFixtureUrl
                cameraWaitTimer.start()
                return
            }

            if (root.phase === 40) {
                if (!known || registered || frames < 1 || processes < 1
                        || !view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Frozen) {
                    root.finish(false, "post-cross-frame context-destruction lease mismatch: known="
                                + known + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                cameraWaitTimer.stop()
                if (!ferricRunCrossFrameCaptureProbe && !ferricForcedChildPermissionProbe) {
                    root.startRendererLossProbe()
                    return
                }
                view.ferricReleaseRtcEmptyLeaseForProbe()
                root.phase = 47
                root.lifecyclePolls = 0
                view.visible = true
                view.lifecycleState = WebEngineView.LifecycleState.Active
                cameraWaitTimer.start()
                return
            }

            if (root.phase === 45) {
                if (!known || registered || frames < 1 || processes < 1
                        || !view.ferricRtcEmptyLeaseValidForProbe()
                        || !view.ferricHasNoMediaCaptureActivityForProbe()
                        || view.ferricHasFrameCaptureActivityForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Frozen) {
                    root.finish(false, "real child-frame capture cleanup did not restore empty exact-view state")
                    return
                }
                cameraWaitTimer.stop()
                root.startRendererLossProbe()
                return
            }

            if (root.phase === 4) {
                if (known || registered || frames < 1 || processes < 1
                        || view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
                    root.finish(false, "active camera was not fail-closed: known=" + known
                                + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                root.phase = 5
                view.runJavaScript("window.ferricCameraTrack.stop();true", function(value) {
                    if (value !== true) {
                        root.finish(false, "fake-camera track stop was not dispatched")
                        return
                    }
                    root.cameraCleanupPolls = 0
                    cameraWaitTimer.start()
                })
                return
            }

            if (root.phase === 5) {
                root.cameraCleanupProbeInFlight = false
                if (!known || registered || frames < 1 || processes < 1
                        || !view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Frozen)
                    return

                cameraWaitTimer.stop()
                root.invalidationObserved = false
                root.phase = 7
                view.runJavaScript(
                    "navigator.mediaDevices.getUserMedia({audio:true,video:false})"
                    + ".then(stream=>{window.ferricMicrophoneStream=stream;"
                    + "window.ferricMicrophoneTrack=stream.getAudioTracks()[0];"
                    + "window.ferricMicrophoneStarted=!!window.ferricMicrophoneTrack&&"
                    + "window.ferricMicrophoneTrack.kind==='audio'&&"
                    + "window.ferricMicrophoneTrack.readyState==='live';"
                    + "}).catch(error=>{window.ferricMicrophoneError=String(error);});true",
                    WebEngineScript.ApplicationWorld,
                    function(value) {
                        if (value !== true || !root.invalidationObserved) {
                            root.finish(false, "Qt Quick microphone dispatch did not revoke the held lease first")
                            return
                        }
                        root.microphoneDispatchAcknowledged = true
                        root.grantMicrophoneAfterPendingProof()
                    })
                return
            }

            if (root.phase === 8) {
                if (known || registered || frames < 1 || processes < 1
                        || view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
                    root.finish(false, "pending fake microphone was not fail-closed: known=" + known
                                + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                if (!root.pendingMicrophonePermission
                        || root.phase !== 8) {
                    root.finish(false, "pending microphone permission identity was lost")
                    return
                }
                root.microphonePendingProbePassed = true
                root.grantMicrophoneAfterPendingProof()
                return
            }

            if (root.phase === 10) {
                if (known || registered || frames < 1 || processes < 1
                        || view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
                    root.finish(false, "active fake microphone was not fail-closed: known=" + known
                                + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                root.phase = 11
                view.runJavaScript("window.ferricMicrophoneTrack.stop();true",
                                   WebEngineScript.ApplicationWorld, function(value) {
                    if (value !== true) {
                        root.finish(false, "fake-microphone track stop was not dispatched")
                        return
                    }
                    root.microphoneCleanupPolls = 0
                    cameraWaitTimer.start()
                })
                return
            }

            if (root.phase === 11) {
                root.microphoneCleanupProbeInFlight = false
                if (!known || registered || frames < 1 || processes < 1
                        || !view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Frozen)
                    return

                cameraWaitTimer.stop()
                root.invalidationObserved = false
                view.runJavaScript(
                    "window.peer = new RTCPeerConnection({iceServers: []});"
                    + "window.channel = peer.createDataChannel('ferric-rtc-quick-probe');"
                    + "peer.signalingState === 'stable' && channel.readyState === 'connecting'",
                    WebEngineScript.ApplicationWorld,
                    function(value) {
                        if (value !== true || !root.invalidationObserved) {
                            root.finish(false, "Qt Quick peer dispatch did not revoke the held lease first")
                            return
                        }
                        root.phase = 13
                        if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                            root.finish(false, "post-peer observation request was not admitted")
                    })
                return
            }

            if (root.phase === 25) {
                if (known || registered || frames < 1 || processes < 1
                        || view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
                    root.finish(false, "active combined fake capture was not fail-closed: known=" + known
                                + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                root.phase = 26
                view.runJavaScript("window.ferricCombinedStream.getTracks().forEach(track=>track.stop());true", function(value) {
                    if (value !== true) {
                        root.finish(false, "combined fake-capture track stop was not dispatched")
                        return
                    }
                    root.combinedCleanupPolls = 0
                    cameraWaitTimer.start()
                })
                return
            }

            if (root.phase === 13) {
                if (known && registered && frames >= 1 && processes >= 1
                        && !view.ferricRtcEmptyLeaseValidForProbe()
                        && view.lifecycleState === WebEngineView.LifecycleState.Active) {
                    cameraWaitTimer.stop()
                    root.phase = 27
                    root.lifecyclePolls = 0
                    view.runJavaScript("window.peer.close();window.channel.close();true",
                                       WebEngineScript.ApplicationWorld, function(value) {
                        if (value !== true) {
                            root.finish(false, "data-only peer did not close before renderer-loss probe")
                            return
                        }
                        view.visible = false
                        view.lifecycleState = WebEngineView.LifecycleState.Frozen
                        cameraWaitTimer.start()
                    })
                } else {
                    root.finish(false, "Qt Quick peer observation mismatch: known=" + known
                                + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState
                                + " visible=" + view.visible)
                }
                return
            }

            if (root.phase === 28) {
                if (!known || !registered || view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
                    root.finish(false, "closing a live RTCPeerConnection did not conservatively retain registration: known="
                                + known + " registered=" + registered + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                cameraWaitTimer.stop()
                root.phase = 32
                root.lifecyclePolls = 0
                view.reload()
                cameraWaitTimer.start()
                return
            }

            if (root.phase === 34) {
                if (!known || registered || frames < 1 || processes < 1
                        || !view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Frozen) {
                    root.finish(false, "post-context-destruction empty lease mismatch: known=" + known
                                + " registered=" + registered + " frames=" + frames
                                + " processes=" + processes + " valid="
                                + view.ferricRtcEmptyLeaseValidForProbe()
                                + " lifecycle=" + view.lifecycleState)
                    return
                }
                cameraWaitTimer.stop()
                root.phase = 19
                root.profileReplacementInvalidationObserved = false
                root.lifecyclePolls = 0
                view.profile = replacementProfile
                cameraWaitTimer.start()
                return
            }

            if (root.phase === 26) {
                root.combinedCleanupProbeInFlight = false
                if (!known || registered || frames < 1 || processes < 1
                        || !view.ferricRtcEmptyLeaseValidForProbe()
                        || view.lifecycleState !== WebEngineView.LifecycleState.Frozen)
                    return

                cameraWaitTimer.stop()
                root.phase = 12
                root.invalidationObserved = false
                view.runJavaScript(
                    "window.peer = new RTCPeerConnection({iceServers: []});"
                    + "window.channel = peer.createDataChannel('ferric-rtc-quick-probe');"
                    + "peer.signalingState === 'stable' && channel.readyState === 'connecting'",
                    WebEngineScript.ApplicationWorld,
                    function(value) {
                        if (value !== true || !root.invalidationObserved) {
                            root.finish(false, "Qt Quick peer dispatch did not revoke the held lease first")
                            return
                        }
                        root.phase = 13
                        if (!view.ferricAcquireRtcEmptyLeaseForProbe())
                            root.finish(false, "post-peer observation request was not admitted")
                    })
                return
            }
        }

        onFerricRtcEmptyLeaseInvalidated: {
            root.invalidationObserved = true
            if (root.phase === 29)
                root.rendererInvalidationCount += 1
            if (root.phase === 2)
                root.lifecycleExitInvalidationObserved = true
            if (root.phase === 16)
                root.lifecycleAcquireInvalidationObserved = true
            if (root.phase === 19)
                root.profileReplacementInvalidationObserved = true
        }
    }

    Timer {
        interval: 120000
        running: true
        onTriggered: root.finish(false, "bounded Qt Quick probe timed out")
    }
}
