import QtQuick
import QtWebEngine
import "../scripts/DownloadController.js" as DownloadController

// A fixed, isolated document uses the captured browser profile. Its lifetime
// extends through the stream, without navigating or activating a visible tab.
Item {
    id: initiator
    required property var rootWindow
    required property var hostWindow
    required property var browserUi
    property var activeHelper: null
    property var helpers: []
    property var ownedHelpers: []
    property bool stopped: false
    readonly property bool busy: activeHelper !== null
    function publishCapacity() {
        if (browserUi && typeof browserUi.set_download_controller_available === "function")
            browserUi.set_download_controller_available(!stopped && !busy && ownedHelpers.length < 8)
    }
    onBusyChanged: publishCapacity()
    onOwnedHelpersChanged: publishCapacity()
    onStoppedChanged: publishCapacity()
    Component.onCompleted: publishCapacity()

    function liveTransfer(helper) {
        if (!helper || stopped || !browserUi || helper.viewRetired || !helper.adopted
                || rootWindow.runtimeDisposing || hostWindow.runtimeReleased || hostWindow.runtimeDisposing) return false
        var group = rootWindow.profileResourceRegistry.forProfile(helper.profile)
        var invoking = rootWindow.profileResourceRegistry.downloadInvocation(browserUi)
        return !!group && !!invoking && invoking.profile === helper.profile
                && group.lifetime.ownerUi(helper) === browserUi
                && group.lifetime.ownerHost(helper) === hostWindow
    }
    function current(helper) {
        return liveTransfer(helper) && helper.phase !== "retiring" && !!helper.sourceView
                && helper.sourceView.viewRetired !== true && helper.sourceView.profile === helper.profile
                && browserUi.download_request_source_index(helper.requestToken) >= 0
    }
    function cleanPrompts(helper) {
        if (helper.authentication && rootWindow.pendingAuthenticationRequest === helper.authentication)
            rootWindow.clearPageDialog(true)
        if (helper.certificate && rootWindow.pendingCertificateError === helper.certificate)
            rootWindow.clearCertificateErrorRequest(true)
        if (helper.clientCertificate && rootWindow.pendingClientCertificateSelection === helper.clientCertificate)
            rootWindow.clearClientCertificateRequest(true)
    }
    function disposeHelper(helper) {
        if (!helper || ownedHelpers.indexOf(helper) < 0) return
        ownedHelpers = ownedHelpers.filter(function(view) { return view !== helper })
        helper.stop()
        if (helper.adopted) rootWindow.profileResourceRegistry.releaseView(helper)
        else helper.destroy()
    }
    function release(helper) {
        if (!helper || ownedHelpers.indexOf(helper) < 0) return
        if (helper.phase === "retiring") {
            // A retired owner suppresses its pending script callback. Release
            // the admission slot here instead of waiting for that callback.
            if (helper.viewRetired) disposeHelper(helper)
            return
        }
        helper.phase = "retiring"
        helpers = helpers.filter(function(view) { return view !== helper })
        if (activeHelper === helper) activeHelper = null
        var group = rootWindow.profileResourceRegistry.forProfile(helper.profile)
        if (group) group.interceptor.revokeDownloadController(helper.controllerUrl)
        cleanPrompts(helper)
        rootWindow.cancelScriptsForView(helper)
        // Abort first: stop()/destroy() alone does not reliably cancel a fetch
        // that is waiting for response headers in a surviving profile.
        if (helper.viewRetired || !rootWindow.runBrowserScript(helper, DownloadController.abort(), function() {
            initiator.disposeHelper(helper)
        })) disposeHelper(helper)
    }
    function fail(helper) {
        if (!helper || helper.phase === "retiring") return
        browserUi.fail_download_request(helper.requestToken)
        release(helper)
    }
    function cancel() {
        var helper = activeHelper
        if (!helper) return
        browserUi.cancel_download_request(helper.requestToken)
        release(helper)
    }
    function shutdown() {
        if (stopped) return
        cancel()
        stopped = true
        var copy = ownedHelpers.slice(0)
        for (var i = 0; i < copy.length; ++i) {
            if (copy[i].offeredDownload && !copy[i].offeredDownload.isFinished) copy[i].offeredDownload.cancel()
            release(copy[i])
        }
    }
    function origin(url) { return String(browserUi.canonical_download_origin(String(url)) || "") }
    function isControllerDocument(helper, url) {
        var match = String(url).match(/^data:text\/html;charset=utf-8,(.*)$/i)
        if (!match) return false
        try { return decodeURIComponent(match[1]) === helper.documentHtml }
        catch (error) { return false }
    }
    function loadController(helper, url) {
        var site = origin(url)
        var group = rootWindow.profileResourceRegistry.forProfile(helper.profile)
        if (!current(helper) || !site || !group) { fail(helper); return }
        if (helper.controllerUrl) group.interceptor.revokeDownloadController(helper.controllerUrl)
        rootWindow.cancelScriptsForView(helper)
        ++helper.epoch
        helper.controllerUrl = site + "/.ferric-download-controller/" + helper.requestToken + "/" + helper.epoch
        helper.requestUrl = String(url)
        helper.documentHtml = "<!doctype html><meta http-equiv='Content-Security-Policy' content=\"default-src 'none'; base-uri 'none'; form-action 'none'; connect-src http: https:\"><title>Ferric download " + helper.epoch + "</title>"
        if (!group.interceptor.registerDownloadController(helper.controllerUrl, helper.requestUrl)) { fail(helper); return }
        helper.phase = "loading"
        helper.loadHtml(helper.documentHtml, helper.controllerUrl)
    }
    function activate(helper) {
        if (!current(helper)) { fail(helper); return }
        helper.phase = "canonicalizing"
        var controller = helper.controllerUrl
        var started = rootWindow.runBrowserScript(helper, DownloadController.canonicalTarget(controller, helper.requestUrl), function(value) {
            if (!initiator.current(helper) || helper.controllerUrl !== controller || helper.phase !== "canonicalizing") return
            var group = rootWindow.profileResourceRegistry.forProfile(helper.profile)
            if (!value || !group) { initiator.fail(helper); return }
            if (!browserUi.admit_download_stream_target(helper.requestToken, String(value))) {
                var notice = browserUi.status_text
                initiator.fail(helper)
                browserUi.status_text = notice
                return
            }
            if (!group.interceptor.registerDownloadController(controller, String(value))) { initiator.fail(helper); return }
            helper.requestUrl = String(value)
            helper.phase = "requesting"
            if (!rootWindow.runBrowserScript(helper, DownloadController.start(controller, helper.requestUrl), function(ok) {
                if (initiator.current(helper) && helper.controllerUrl === controller && ok !== true) initiator.fail(helper)
            })) initiator.fail(helper)
        })
        if (!started) fail(helper)
    }
    function pollHeaders(helper) {
        if (!current(helper) || Date.now() >= helper.deadline) { fail(helper); return }
        if (helper.phase !== "requesting" || helper.polling) return
        helper.polling = true
        var controller = helper.controllerUrl
        var started = rootWindow.runBrowserScript(helper, DownloadController.metadata(controller), function(value) {
            helper.polling = false
            if (!initiator.current(helper) || helper.controllerUrl !== controller || helper.phase !== "requesting") return
            var group = rootWindow.profileResourceRegistry.forProfile(helper.profile)
            var redirect = group ? String(group.interceptor.takeDownloadControllerTarget(controller)) : ""
            if (redirect) { initiator.loadController(helper, redirect); return }
            if (!value || value.phase === "error") {
                var blocked = group && group.interceptor.downloadControllerBlocked(controller)
                initiator.fail(helper)
                if (blocked)
                    browserUi.status_text = "This download was blocked by the current site policy. Review Site Doctor before retrying."
                else if (value && Number(value.status) >= 400)
                    browserUi.status_text = "The server couldn't provide this file (HTTP " + Number(value.status) + "). Open the link to check access, then retry."
                return
            }
            if (value.phase !== "headers") return
            helper.phase = "headers"
            var transfer = streamFactory.createObject(initiator.rootWindow, {
                rootWindow:initiator.rootWindow, browserUi:initiator.browserUi, initiator:initiator,
                helper:helper, profileGroup:group, url:helper.originalUrl, totalBytes:Number(value.total),
                suggestedFileName:String(browserUi.download_stream_name(helper.requestUrl, String(value.disposition || ""), String(value.mime || "")))
            })
            if (!transfer) { initiator.fail(helper); return }
            if (!group || !rootWindow.profileResourceRegistry.handleStreamDownloadRequested(group, helper, transfer)) {
                transfer.cancel()
                initiator.fail(helper)
            }
        })
        if (!started) { helper.polling = false; fail(helper) }
    }
    function start(token, url) {
        var registry = rootWindow.profileResourceRegistry
        if (!registry || !registry.downloadSource || !registry.downloadSource.supported) {
            browserUi.fail_download_request(token)
            browserUi.status_text = "Downloads are unavailable in this build. Update or rebuild Ferric Browser, then try again."
            return false
        }
        var index = browserUi.download_request_source_index(token)
        var source = index >= 0 ? hostWindow.tabViewAt(index) : null
        if (stopped || busy || ownedHelpers.length >= 8 || !source || !source.profile || !origin(url)) {
            browserUi.fail_download_request(token)
            return false
        }
        var helper = helperFactory.createObject(initiator, {
            profile:source.profile, sourceView:source, requestToken:String(token), originalUrl:String(url), deadline:Date.now() + 30000
        })
        if (!helper) { browserUi.fail_download_request(token); return false }
        ownedHelpers = ownedHelpers.concat([helper])
        helpers = helpers.concat([helper])
        activeHelper = helper
        var group = registry.forProfile(source.profile)
        helper.adopted = !!group && group.lifetime.adoptView(helper, hostWindow, browserUi)
        if (!helper.adopted || !current(helper)) { fail(helper); return false }
        loadController(helper, url)
        return true
    }
    function permitsOffer(view) { return view === activeHelper && view.phase === "headers" && current(view) }
    function sourceForOffer(view) { return permitsOffer(view) ? view.originalUrl : "" }
    function acceptOffer(view, download, id) {
        if (!permitsOffer(view) || !browserUi.reserve_stream_download(id)
                || !browserUi.accept_download_request(view.requestToken, id)) {
            browserUi.release_stream_download(id)
            fail(view)
            return false
        }
        view.phase = "offered"
        view.offeredDownload = download
        activeHelper = null
        return true
    }
    function transferStarted(helper) {
        if (!helper || helper.phase !== "offered") return
        helper.phase = "streaming"
        helpers = helpers.filter(function(view) { return view !== helper })
    }
    Timer {
        interval: 25
        repeat: true
        running: initiator.busy
        onTriggered: initiator.pollHeaders(initiator.activeHelper)
    }
    Component { id: streamFactory; FerricStreamDownload {} }
    Component {
        id: helperFactory
        WebEngineView {
            id: helper
            width: 1
            height: 1
            visible: false
            enabled: false
            required property var sourceView
            required property string requestToken
            required property string originalUrl
            required property real deadline
            property var downloadInitiator: initiator
            property var viewHost: initiator.hostWindow
            property bool viewRetired: false
            property bool adopted: false
            property string phase: "preparing"
            property string requestUrl: ""
            property string controllerUrl: ""
            property string documentHtml: ""
            property int epoch: 0
            property bool polling: false
            property var offeredDownload: null
            property var authentication: null
            property var certificate: null
            property var clientCertificate: null
            settings.focusOnNavigationEnabled: false
            settings.javascriptCanOpenWindows: false
            onViewRetiredChanged: {
                if (!viewRetired) return
                if (offeredDownload && !offeredDownload.isFinished) offeredDownload.cancel()
                initiator.release(helper)
            }
            onNavigationRequested: function(request) {
                if (phase === "loading" && request.isMainFrame && initiator.isControllerDocument(helper, String(request.url))) request.accept()
                else request.reject()
            }
            onLoadingChanged: function(request) {
                if (phase !== "loading") return
                if (request.status === WebEngineView.LoadSucceededStatus && String(request.url) === controllerUrl) initiator.activate(helper)
                else if (request.status === WebEngineView.LoadFailedStatus
                         && (String(request.url) === controllerUrl || initiator.isControllerDocument(helper, String(request.url)))) initiator.fail(helper)
            }
            onRenderProcessTerminated: function(status) {
                if (status === WebEngineView.NormalTerminationStatus) return
                if (offeredDownload && !offeredDownload.isFinished) offeredDownload.interrupt("The download process stopped. Retry it from Downloads.", WebEngineDownloadRequest.NetworkFailed)
                else initiator.fail(helper)
            }
            onAuthenticationDialogRequested: function(request) {
                if (!initiator.current(helper)) { request.accepted = true; request.dialogReject(); return }
                authentication = request
                deadline = Math.max(deadline, Date.now() + 120000)
                initiator.rootWindow.handleAuthenticationDialogRequested(initiator.browserUi, sourceView,
                    request, profile.offTheRecord, initiator.hostWindow)
            }
            onSelectClientCertificate: function(selection) {
                if (!initiator.current(helper)) { selection.selectNone(); return }
                clientCertificate = selection
                deadline = Math.max(deadline, Date.now() + 120000)
                initiator.rootWindow.handleClientCertificateRequested(initiator.browserUi, sourceView,
                    selection, profile.offTheRecord, initiator.hostWindow)
            }
            onCertificateError: function(error) {
                var wasCurrent = initiator.current(helper)
                // A fetch is a subresource. Keep the existing rule that only
                // a deliberate top-level page visit can review an exception.
                error.rejectCertificate()
                initiator.fail(helper)
                if (wasCurrent) initiator.browserUi.status_text = "The download's secure connection couldn't be verified. Open the link to review its certificate, then retry."
            }
            onJavaScriptDialogRequested: function(request) {
                request.accepted = true
                request.dialogReject()
                initiator.fail(helper)
            }
            onPermissionRequested: function(permission) { permission.deny() }
            onFileDialogRequested: function(request) { request.accepted = true; request.dialogReject() }
        }
    }
    Component.onDestruction: shutdown()
}
