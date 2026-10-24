import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as BrowserScripts
import "../scripts/ChromePresentation.js" as ChromePresentation
import "../scripts/SpellcheckPresentation.js" as SpellcheckPresentation
import "../scripts/SwitcherSources.js" as SwitcherSources
import "../scripts/SwitcherResults.js" as SwitcherResults
import "../scripts/CloseRequests.js" as CloseRequests
import "../scripts/TargetSearchPresentation.js" as TargetSearchPresentation
import "../scripts/EngineRequests.js" as EngineRequests
import "../scripts/KeyboardPresentation.js" as KeyboardPresentation

// Owns window behavior through an explicit host; no inherited creation scope.
QtObject {
    required property var host
    readonly property var window: host

    function activateSwitcher(index) {
        if (!window.switcherResultsCurrent()) return
        var result = window.switcherResults[index]
        if (!result) {
            return
        }
        var owner = window.switcherOwner(result)
        if (!owner) {
            var ui = window.switcherUi || window.browserUi
            ui.status_text = "That window is no longer available. Refresh the Switcher and choose again."
            window.refreshSwitcher()
            return
        }
        var generation = result.generation === null || result.generation === undefined
                ? "" : String(result.generation)
        if (owner.ui.activate_switcher_result(result.kind, result.id, generation)) {
            if (owner.host === window) {
                window.syncTabModel()
                window.executePendingEngineAction()
            } else if (owner.host.applySwitcherEngineAction) {
                owner.host.applySwitcherEngineAction()
            }
            window.closeSwitcher()
            window.activateBrowserWindow(owner.host, owner.ui)
            if (result.workspace) {
                window.routeContextWorkspace(owner.ui)
            }
        }
    }

    function activateSwitcherAction(index, action) {
        if (!window.switcherResultsCurrent()) return
        var result = window.switcherResults[index]
        if (!result || !action) {
            return
        }
        var owner = window.switcherOwner(result)
        if (!owner) {
            var ui = window.switcherUi || window.browserUi
            ui.status_text = "That window is no longer available. Refresh the Switcher and choose again."
            window.refreshSwitcher()
            return
        }
        var generation = result.generation === null || result.generation === undefined
                ? "" : String(result.generation)
        if (owner.ui.activate_switcher_action(result.kind, result.id, action, generation)) {
            if (owner.host === window) {
                window.syncTabModel()
                window.executePendingEngineAction()
            } else if (owner.host.applySwitcherEngineAction) {
                owner.host.applySwitcherEngineAction()
            }
            window.closeSwitcher()
            window.activateBrowserWindow(owner.host, owner.ui)
            if (result.workspace) {
                window.routeContextWorkspace(owner.ui)
            }
        }
    }

    function downloadStateName(download) {
        if (download.isPaused) {
            return "paused"
        }
        if (download.state === WebEngineDownloadRequest.DownloadInProgress) {
            return "in-progress"
        }
        if (download.state === WebEngineDownloadRequest.DownloadCompleted) {
            return "completed"
        }
        if (download.state === WebEngineDownloadRequest.DownloadCancelled) {
            return "cancelled"
        }
        if (download.state === WebEngineDownloadRequest.DownloadInterrupted) {
            return "interrupted"
        }
        return "offered"
    }

    function navigationFailureKind(loadRequest) {
        if (loadRequest.isDownload === true) {
            return "download"
        }
        var domain = String(loadRequest.errorDomain)
        if (domain === "6" || domain.indexOf("Dns") >= 0) {
            return "dns"
        }
        if (domain === "3" || domain.indexOf("Certificate") >= 0) {
            return "tls"
        }
        if (domain === "4" || domain === "7"
                || domain.indexOf("Http") >= 0) {
            return "http"
        }
        if (domain === "2" || domain.indexOf("Connection") >= 0) {
            return "network"
        }
        return "unknown"
    }

    function navigationFailureDetail(loadRequest) {
        var detail = String(loadRequest.errorString || "")
        var code = Number(loadRequest.errorCode)
        if (Number.isFinite(code) && code !== 0) {
            detail = detail.length > 0 ? detail + " (code " + code + ")" : "code " + code
        }
        return detail
    }

    function downloadsAskDestination(ui) {
        return (ui || window.browserUi).feature_downloads_ask_destination
    }

    function permissionTypeName(permissionType) {
        return window.requestPresentationController.permissionTypeName(permissionType)
    }

    function permissionDisplayName(permissionName) {
        return window.requestPresentationController.permissionDisplayName(permissionName)
    }

    function permissionCanRemember(origin, permissionName) {
        return window.requestPresentationController.permissionCanRemember(origin, permissionName)
    }

    function permissionCanRememberForSite(origin, permissionName) {
        return window.requestPresentationController.permissionCanRememberForSite(origin, permissionName)
    }

    function boundedPageDialogText(value) {
        return window.requestPresentationController.boundedPageDialogText(value)
    }

    function pageDialogType(request) {
        return window.requestPresentationController.pageDialogType(request)
    }

    function pageDialogKind() {
        return window.requestPresentationController.pageDialogKind()
    }

    function clearPageDialog(rejectRequest) {
        var host = window.runtime_pageDialogPopup.hostWindow
        var request = window.pendingAuthenticationRequest || window.pendingPageDialogRequest
        var hadRequest = !!request
        var ui = window.pendingPageDialogUi || window.browserUi
        var view = window.pendingPageDialogView
        var leaseToken = window.pendingPageDialogLeaseToken
        var authentication = window.pageDialogIsAuthentication
        window.pendingPageDialogRequest = null
        window.pendingAuthenticationRequest = null
        window.pendingPageDialogUi = null
        window.pendingPageDialogView = null
        window.pendingPageDialogLeaseToken = ""
        window.pageDialogIsAuthentication = false
        window.pageDialogMessage = ""
        window.pageDialogTitle = "Page dialog"
        window.pageDialogOrigin = ""
        window.pageDialogDefaultText = ""
        window.pageDialogSuppressChecked = false
        window.runtime_pageDialogPopup.usernameText = ""
        window.runtime_pageDialogPopup.passwordText = ""
        window.runtime_pageDialogPopup.inputText = ""
        window.runtime_pageDialogPopup.close()
        if (hadRequest) {
            window.restoreOverlayFocus(host)
        }
        window.completeNativePromptLease(ui, leaseToken, view)
        if (rejectRequest && request) {
            if (!authentication && window.pageDialogType(request) === "beforeunload"
                    && view && typeof view.cancelPendingContentReload === "function")
                view.cancelPendingContentReload()
            window.resolveQtRequest(ui, request, authentication ? "authentication" : "page-dialog", "dialogReject", [])
            if (ui) {
                ui.status_text = authentication
                        ? "Authentication prompt dismissed"
                        : "Page dialog dismissed"
            }
        }
    }

    function clearPageDialogForView(view) {
        if (window.pendingPageDialogView === view) {
            window.clearPageDialog(true)
        }
    }

    function resetPageDialogBudget(view) {
        if (!view) {
            return
        }
        view.pageDialogDocumentKey = ""
        view.pageDialogCount = 0
        view.pageDialogSuppressed = false
    }

    function pageDialogDocumentKey(view) {
        var url = view && view.url ? view.url.toString() : ""
        var fragment = url.indexOf("#")
        return fragment >= 0 ? url.slice(0, fragment) : url
    }

    function handleJavaScriptDialogRequested(ui, view, request, privateProfile, hostWindow) {
        if (!request) {
            return
        }
        // Claim presentation synchronously. This prevents Qt's default dialog;
        // the user's decision still resolves through dialogAccept/dialogReject.
        request.accepted = true
        if (window.pendingPageDialogRequest || window.pendingAuthenticationRequest) {
            window.clearPageDialog(true)
        }
        var kind = window.pageDialogType(request)
        if (kind === "unknown") {
            window.resolveQtRequest(ui, request, "page-dialog", "dialogReject", [])
            ui.status_text = "Blocked unsupported page dialog"
            return
        }
        var documentKey = window.pageDialogDocumentKey(view)
        if (view.pageDialogDocumentKey !== documentKey) {
            window.resetPageDialogBudget(view)
            view.pageDialogDocumentKey = documentKey
        }
        if (view.pageDialogSuppressed || view.pageDialogCount >= window.pageDialogLimit) {
            window.resolveQtRequest(ui, request, "page-dialog", "dialogReject", [])
            ui.status_text = view.pageDialogSuppressed
                    ? "Suppressed repeated page dialog"
                    : "Page dialog limit reached for this page"
            return
        }
        view.pageDialogCount += 1
        window.pendingPageDialogRequest = request
        window.pendingPageDialogUi = ui
        window.pendingPageDialogView = view
        window.pendingPageDialogLeaseToken = window.beginNativePromptLease(
                    ui, view, "page-dialog")
        window.captureOverlayFocus(hostWindow || window, view)
        window.pageDialogMessage = window.boundedPageDialogText(request.message)
        var title = window.boundedPageDialogText(request.title)
        var fallbackTitle = kind === "confirm" ? "Confirm this page's request"
                : kind === "prompt" ? "Reply to this page"
                : kind === "beforeunload" ? "Leave this page?"
                : "Message from this page"
        window.pageDialogTitle = title.length > 0 ? title : fallbackTitle
        window.pageDialogOrigin = ui.prompt_origin(String(request.securityOrigin || ""))
                || "Origin unavailable"
        window.pageDialogDefaultText = window.boundedPageDialogText(request.defaultText)
        window.pageDialogSuppressChecked = false
        window.runtime_pageDialogPopup.inputText = window.pageDialogDefaultText
        window.runtime_pageDialogPopup.open(hostWindow || window)
        ui.status_text = privateProfile ? "Private page dialog" : "Page dialog"
    }

    function handleAuthenticationDialogRequested(ui, view, request, privateProfile, hostWindow) {
        if (!request) {
            return
        }
        request.accepted = true
        if (window.pendingPageDialogRequest || window.pendingAuthenticationRequest) {
            window.clearPageDialog(true)
        }
        var documentKey = window.pageDialogDocumentKey(view)
        if (view.pageDialogDocumentKey !== documentKey) {
            window.resetPageDialogBudget(view)
            view.pageDialogDocumentKey = documentKey
        }
        if (view.pageDialogSuppressed || view.pageDialogCount >= window.pageDialogLimit) {
            window.resolveQtRequest(ui, request, "authentication", "dialogReject", [])
            ui.status_text = view.pageDialogSuppressed
                    ? "Suppressed repeated authentication prompt"
                    : "Authentication prompt limit reached for this page"
            return
        }
        view.pageDialogCount += 1
        window.pendingAuthenticationRequest = request
        window.pendingPageDialogUi = ui
        window.pendingPageDialogView = view
        window.pendingPageDialogLeaseToken = window.beginNativePromptLease(
                    ui, view, "authentication")
        window.captureOverlayFocus(hostWindow || window, view)
        window.pageDialogIsAuthentication = true
        var proxyAuthentication = request.type === AuthenticationDialogRequest.AuthenticationTypeProxy
        window.pageDialogTitle = proxyAuthentication
                ? "Sign in to the proxy"
                : "Sign in to this site"
        window.pageDialogOrigin = proxyAuthentication
                ? window.boundedPageDialogText(request.proxyHost)
                : (ui.prompt_origin(String(request.url || "")) || "Origin unavailable")
        var realm = window.boundedPageDialogText(request.realm)
        window.pageDialogMessage = realm.length > 0
                ? (proxyAuthentication ? "Proxy message: " : "Site message: ") + realm
                : "Enter your username and password to continue."
        window.pageDialogSuppressChecked = false
        window.runtime_pageDialogPopup.usernameText = ""
        window.runtime_pageDialogPopup.passwordText = ""
        window.runtime_pageDialogPopup.open(hostWindow || window)
        ui.status_text = privateProfile ? "Private authentication prompt" : "Authentication prompt"
    }

    function clearClientCertificateRequest(selectNone) {
        var host = window.certificatePrompts.hostWindow
        var selection = window.pendingClientCertificateSelection
        var hadSelection = !!selection
        var ui = window.pendingClientCertificateUi || window.browserUi
        var view = window.pendingClientCertificateView
        var leaseToken = window.pendingClientCertificateLeaseToken
        window.pendingClientCertificateSelection = null
        window.pendingClientCertificateUi = null
        window.pendingClientCertificateView = null
        window.pendingClientCertificateLeaseToken = ""
        window.pendingClientCertificateOptions = []
        window.pendingClientCertificateHost = ""
        window.certificatePrompts.closeClientCertificate()
        if (hadSelection && selectNone) {
            window.resolveQtRequest(ui, selection, "client-certificate", "selectNone", [])
        }
        if (hadSelection) {
            window.restoreOverlayFocus(host)
        }
        window.completeNativePromptLease(ui, leaseToken, view)
        return hadSelection ? ui : null
    }

    function clearClientCertificateForView(view) {
        if (window.pendingClientCertificateView === view) {
            window.clearClientCertificateRequest(true)
        }
    }

    function handleClientCertificateRequested(ui, view, selection, privateProfile, hostWindow) {
        if (!selection) {
            return
        }
        if (window.pendingPageDialogRequest || window.pendingAuthenticationRequest) {
            window.clearPageDialog(true)
        }
        if (window.pendingClientCertificateSelection) {
            window.clearClientCertificateRequest(true)
        }
        var offered = selection.certificates || []
        if (offered.length === 0) {
            window.resolveQtRequest(ui, selection, "client-certificate", "selectNone", [])
            ui.status_text = "No client certificates available; request cancelled"
            return
        }
        var options = []
        for (var i = 0; i < offered.length && i < 32; ++i) {
            var option = offered[i]
            options.push({
                index: i,
                subject: window.boundedPageDialogText(option.subject || "Unnamed certificate"),
                issuer: window.boundedPageDialogText(option.issuer || "Issuer unavailable"),
                selfSigned: !!option.isSelfSigned
            })
        }
        if (options.length === 0) {
            window.resolveQtRequest(ui, selection, "client-certificate", "selectNone", [])
            ui.status_text = "Client certificate choices unavailable; request cancelled"
            return
        }
        window.pendingClientCertificateSelection = selection
        window.pendingClientCertificateUi = ui
        window.pendingClientCertificateView = view
        window.pendingClientCertificateLeaseToken = window.beginNativePromptLease(
                    ui, view, "client-certificate")
        window.pendingClientCertificateOptions = options
        window.pendingClientCertificateHost = ui.client_certificate_origin(String(selection.host || ""))
                || "Host unavailable"
        // Capture the native frame before transferring focus to the menu.
        window.captureOverlayFocus(hostWindow || window, view)
        window.certificatePrompts.openClientCertificate(hostWindow || window)
        ui.status_text = privateProfile
                ? "Private client certificate selection"
                : "Client certificate selection"
    }

    function acceptClientCertificate(index) {
        var selection = window.pendingClientCertificateSelection
        var ui = window.pendingClientCertificateUi || window.browserUi
        var options = window.pendingClientCertificateOptions || []
        var valid = false
        for (var i = 0; i < options.length; ++i) {
            if (options[i] && options[i].index === index) {
                valid = true
                break
            }
        }
        if (!selection || !valid) {
            return
        }
        window.resolveQtRequest(ui, selection, "client-certificate", "select", [index])
        window.clearClientCertificateRequest(false)
        ui.status_text = "Client certificate selected"
    }

    function rejectClientCertificate() {
        var ui = window.clearClientCertificateRequest(true) || window.browserUi
        ui.status_text = "Client certificate request cancelled"
    }

    function googleCertificateHost(host) {
        var value = String(host || "").toLowerCase()
        return value === "google.com" || value.indexOf("google.") === 0
                || value.indexOf(".google.") >= 0 || value.endsWith(".google")
    }

    function clearCertificateErrorRequest(rejectRequest) {
        var host = window.certificatePrompts.hostWindow
        var error = window.pendingCertificateError
        var hadError = !!error
        var ui = window.pendingCertificateErrorUi || window.browserUi
        var view = window.pendingCertificateErrorView
        var leaseToken = window.pendingCertificateErrorLeaseToken
        window.pendingCertificateError = null
        window.pendingCertificateErrorUi = null
        window.pendingCertificateErrorView = null
        window.pendingCertificateErrorLeaseToken = ""
        window.pendingCertificateErrorHost = ""
        window.pendingCertificateErrorDescription = ""
        window.certificatePrompts.closeCertificateError()
        if (hadError && rejectRequest) {
            window.resolveQtRequest(ui, error, "client-certificate", "rejectCertificate", [])
        }
        if (hadError) {
            window.restoreOverlayFocus(host)
        }
        window.completeNativePromptLease(ui, leaseToken, view)
        return hadError ? ui : null
    }

    function clearCertificateErrorForView(view) {
        if (window.pendingCertificateErrorView === view) {
            window.clearCertificateErrorRequest(true)
        }
    }

    function handleCertificateError(ui, view, error, hostWindow) {
        if (!error) {
            return
        }
        if (window.pendingCertificateError) {
            window.clearCertificateErrorRequest(true)
        }
        if (window.pendingPageDialogRequest || window.pendingAuthenticationRequest) {
            window.clearPageDialog(true)
        }
        if (window.pendingClientCertificateSelection) {
            window.clearClientCertificateRequest(true)
        }
        var origin = ui.prompt_origin(String(error.url || ""))
        var host = ui.prompt_host(String(error.url || ""))
        if (!error.overridable || !error.isMainFrame) {
            window.resolveQtRequest(ui, error, "client-certificate", "rejectCertificate", [])
            ui.status_text = error.isMainFrame
                    ? "TLS certificate error blocked"
                    : "TLS certificate error blocked for a subresource"
            return
        }
        if (!host || !origin || window.googleCertificateHost(host)) {
            window.resolveQtRequest(ui, error, "client-certificate", "rejectCertificate", [])
            ui.status_text = "TLS certificate error blocked for protected host"
            return
        }
        // Qt rejects automatically when the signal handler returns unless the
        // decision is deferred. This only pauses; it never grants an exception.
        error.defer()
        window.pendingCertificateError = error
        window.pendingCertificateErrorUi = ui
        window.pendingCertificateErrorView = view
        window.pendingCertificateErrorLeaseToken = window.beginNativePromptLease(
                    ui, view, "certificate-error")
        window.pendingCertificateErrorHost = origin
        window.pendingCertificateErrorDescription = window.boundedPageDialogText(
                    error.description || "The certificate could not be verified")
        // Capture the native frame before transferring focus to the menu.
        window.captureOverlayFocus(hostWindow || window, view)
        window.certificatePrompts.openCertificateError(hostWindow || window)
        ui.status_text = "TLS certificate confirmation required"
    }

    function acceptCertificateError() {
        var error = window.pendingCertificateError
        var ui = window.pendingCertificateErrorUi || window.browserUi
        if (!error) {
            return
        }
        window.resolveQtRequest(ui, error, "client-certificate", "acceptCertificate", [])
        window.clearCertificateErrorRequest(false)
        ui.status_text = "Certificate exception allowed for this browser session"
    }

    function rejectCertificateError() {
        var ui = window.clearCertificateErrorRequest(true) || window.browserUi
        ui.status_text = "TLS certificate rejected"
    }

    function webAuthStateLabel(state) {
        if (state === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount) {
            return "Select an account"
        }
        if (state === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin) {
            return "Enter your security-key PIN"
        }
        if (state === WebEngineWebAuthUxRequest.WebAuthUxState.FinishTokenCollection) {
            return "Touch or confirm your security key"
        }
        if (state === WebEngineWebAuthUxRequest.WebAuthUxState.RequestFailed) {
            return "Security-key request failed"
        }
        if (state === WebEngineWebAuthUxRequest.WebAuthUxState.Completed) {
            return "Security-key request completed"
        }
        if (state === WebEngineWebAuthUxRequest.WebAuthUxState.Cancelled) {
            return "Security-key request cancelled"
        }
        return "Preparing security-key request"
    }

    function updateWebAuthState(request, state) {
        if (window.pendingWebAuthRequest !== request) {
            return
        }
        window.pendingWebAuthState = state
        window.webAuthStatusText = window.webAuthStateLabel(state)
        if (state === WebEngineWebAuthUxRequest.WebAuthUxState.Completed) {
            var completedUi = window.clearWebAuthRequest(false) || window.browserUi
            completedUi.status_text = "WebAuthn completed"
        } else if (state === WebEngineWebAuthUxRequest.WebAuthUxState.Cancelled) {
            var cancelledUi = window.clearWebAuthRequest(false) || window.browserUi
            cancelledUi.status_text = "WebAuthn cancelled"
        }
    }

    function clearWebAuthRequest(cancelRequest) {
        var host = window.webAuthPrompt.hostWindow
        var request = window.pendingWebAuthRequest
        var hadRequest = !!request
        var ui = window.pendingWebAuthUi || window.browserUi
        var view = window.pendingWebAuthView
        var leaseToken = window.pendingWebAuthLeaseToken
        window.pendingWebAuthRequest = null
        window.pendingWebAuthUi = null
        window.pendingWebAuthView = null
        window.pendingWebAuthLeaseToken = ""
        window.pendingWebAuthUserNames = []
        window.pendingWebAuthState = null
        window.pendingWebAuthRelyingParty = ""
        window.webAuthStatusText = ""
        window.webAuthPrompt.pin = ""
        window.webAuthPrompt.close()
        if (hadRequest && cancelRequest
                && request.state !== WebEngineWebAuthUxRequest.WebAuthUxState.Completed
                && request.state !== WebEngineWebAuthUxRequest.WebAuthUxState.Cancelled) {
            window.resolveQtRequest(ui, request, "webauth", "cancel", [])
        }
        if (hadRequest) {
            window.restoreOverlayFocus(host)
        }
        window.completeNativePromptLease(ui, leaseToken, view)
        return hadRequest ? ui : null
    }

    function clearWebAuthForView(view) {
        if (window.pendingWebAuthView === view) {
            window.clearWebAuthRequest(true)
        }
    }

    function handleWebAuthRequested(ui, view, request, hostWindow) {
        if (!request) {
            return
        }
        if (window.pendingWebAuthRequest) {
            window.clearWebAuthRequest(true)
        }
        if (window.pendingPageDialogRequest || window.pendingAuthenticationRequest) {
            window.clearPageDialog(true)
        }
        if (window.pendingClientCertificateSelection) {
            window.clearClientCertificateRequest(true)
        }
        if (window.pendingCertificateError) {
            window.clearCertificateErrorRequest(true)
        }
        var names = []
        var offered = request.userNames || []
        for (var i = 0; i < offered.length && i < 16; ++i) {
            names.push(window.boundedPageDialogText(offered[i]))
        }
        window.pendingWebAuthRequest = request
        window.pendingWebAuthUi = ui
        window.pendingWebAuthView = view
        window.pendingWebAuthLeaseToken = window.beginNativePromptLease(
                    ui, view, "webauthn")
        window.pendingWebAuthUserNames = names
        window.pendingWebAuthState = request.state
        window.pendingWebAuthRelyingParty = window.boundedPageDialogText(
                    request.relyingPartyId || "opaque or unavailable relying party")
        window.webAuthStatusText = window.webAuthStateLabel(request.state)
        request.stateChanged.connect(function(state) {
            window.updateWebAuthState(request, state)
        })
        // Capture the native frame before transferring focus to the menu.
        window.captureOverlayFocus(hostWindow || window, view)
        window.webAuthPrompt.open(hostWindow || window)
        ui.status_text = "WebAuthn security-key prompt"
    }

    function selectWebAuthAccount(account) {
        var request = window.pendingWebAuthRequest
        if (!request || window.pendingWebAuthState !== WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount) {
            return
        }
        request.setSelectedAccount(window.boundedPageDialogText(account))
    }

    function submitWebAuthPin() {
        var request = window.pendingWebAuthRequest
        if (!request || window.pendingWebAuthState !== WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin) {
            return
        }
        var pin = window.boundedPageDialogText(window.webAuthPrompt.pin)
        window.webAuthPrompt.pin = ""
        request.setPin(pin)
        pin = ""
    }

    function retryWebAuth() {
        var request = window.pendingWebAuthRequest
        if (request && window.pendingWebAuthState === WebEngineWebAuthUxRequest.WebAuthUxState.RequestFailed) {
            request.retry()
        }
    }

    function cancelWebAuth() {
        var ui = window.clearWebAuthRequest(true) || window.browserUi
        ui.status_text = "WebAuthn cancelled"
    }

    function safeContextUrl(value) {
        var result = String(value || "")
        var fragment = result.indexOf("#")
        if (fragment >= 0) {
            result = result.slice(0, fragment)
        }
        result = result.replace(/^(https?:\/\/)[^\/?#@]*@/i, "$1")
        var question = result.indexOf("?")
        if (question < 0) {
            return result
        }
        var base = result.slice(0, question)
        var queryParts = result.slice(question + 1).split("&")
        var kept = []
        for (var i = 0; i < queryParts.length; ++i) {
            var part = queryParts[i]
            var key = part.split("=", 1)[0].toLowerCase()
            var decodedKey = key
            try {
                decodedKey = decodeURIComponent(key).toLowerCase()
            } catch (error) {
                continue
            }
            if (/(pass|token|secret|auth|session|signature|sig|api[_-]?key|private[_-]?key|client[_-]?secret|credential|jwt|bearer|nonce|^code$|^key$)/.test(decodedKey)) {
                continue
            }
            if (part.length > 0) {
                kept.push(part)
            }
        }
        return kept.length > 0 ? base + "?" + kept.join("&") : base
    }

    function clearContextMenuRequest(preserveHintPalettePermit) {
        var host = window.pendingContextMenuHost
        var request = window.pendingContextMenuRequest
        var hadRequest = !!request
        var view = window.pendingContextMenuView
        var requestUi = window.pendingContextMenuUi
        var leaseToken = window.pendingContextMenuLeaseToken
        if (request && request.hintAction && !preserveHintPalettePermit && requestUi) {
            requestUi.cancel_hint_palette()
        }
        window.pendingContextMenuRequest = null
        window.pendingContextMenuView = null
        window.pendingContextMenuSelection = null
        window.pendingContextMenuUi = null
        window.pendingContextMenuLeaseToken = ""
        window.pendingContextMenuHost = null
        window.contextMenuItems = []
        window.contextMenu.close()
        if (hadRequest) {
            window.restoreOverlayFocus(host)
        }
        window.completeNativePromptLease(requestUi || window.browserUi, leaseToken, view)
    }

    function clearContextMenuForView(view) {
        if (window.pendingContextMenuView === view) {
            window.clearContextMenuRequest()
        }
    }

    function contextMenuItem(label, action, value, actionId, enabled) {
        return { label: label, action: action, value: value || "",
                 actionId: actionId || "", enabled: enabled !== false }
    }

    function appendExternalActions(items, ui, subject, value) {
        if (ui.select_external_action_subject(subject)) {
            var ids = ui.external_action_ids
            var labels = ui.external_action_labels
            var availability = ui.external_action_availability
            if (ids.length !== labels.length || ids.length !== availability.length) {
                return
            }
            for (var i = 0; i < ids.length; ++i) {
                if (String(availability[i]) === "true" && String(ids[i]).length > 0
                        && String(labels[i]).length > 0) {
                    items.push(window.contextMenuItem(
                                  String(labels[i]),
                                  "external-" + subject + "-send",
                                  value || "",
                                  String(ids[i])))
                }
            }
        }
    }

    function appendUserscriptActions(items, ui, subject, value) {
        if (ui.select_userscript_action_subject(subject)) {
            var ids = ui.userscript_action_ids
            var labels = ui.userscript_action_labels
            var availability = ui.userscript_action_availability
            if (ids.length !== labels.length || ids.length !== availability.length) {
                return
            }
            for (var i = 0; i < ids.length; ++i) {
                if (String(availability[i]) === "true" && String(ids[i]).length > 0
                        && String(labels[i]).length > 0) {
                    var item = window.contextMenuItem(String(labels[i]), "userscript-action",
                                                       subject === "selection" ? "" : value, String(ids[i]))
                    item.selectionSubject = subject === "selection"
                    items.push(item)
                }
            }
        }
    }

    function handleContextMenuRequested(ui, view, request, hostWindow) {
        if (!request || !view) {
            return
        }
        if (window.pendingContextMenuRequest) {
            window.clearContextMenuRequest()
        }
        var items = []
        var link = request.linkUrl ? request.linkUrl.toString() : ""
        var safeLink = window.safeContextUrl(link)
        if (safeLink.length > 0) {
            items.push(window.contextMenuItem("Open link", "open-link", safeLink,
                                              "browser.link.open"))
            if (!hostWindow || hostWindow === window) {
                items.push(window.contextMenuItem(
                              "Open link in new tab", "open-link-tab",
                              "tab\t" + safeLink, "browser.link.open"))
                items.push(window.contextMenuItem(
                              "Open link in background tab", "open-link-tab-bg",
                              "tab-bg\t" + safeLink, "browser.link.open"))
            }
            items.push(window.contextMenuItem(
                          "Open link in new window", "open-link-window",
                          "window\t" + safeLink, "browser.link.open"))
            items.push(window.contextMenuItem("Copy link", "copy-link", safeLink,
                                              "browser.link.copy"))
            items.push(window.contextMenuItem("Clean-copy link", "copy-link", safeLink,
                                              "browser.link.clean-copy"))
            items.push(window.contextMenuItem("Download link", "download-link", safeLink,
                                              "browser.link.download"))
            window.appendExternalActions(items, ui, "link", safeLink)
            window.appendUserscriptActions(items, ui, "link", safeLink)
        }
        var media = request.mediaUrl ? request.mediaUrl.toString() : ""
        if (media.length > 0 && Number(request.mediaType) !== 0) {
            items.push(window.contextMenuItem("Copy media URL", "copy-link",
                                               window.safeContextUrl(media),
                                               "browser.link.copy"))
            items.push(window.contextMenuItem("Download media", "download-link",
                                               window.safeContextUrl(media),
                                               "browser.link.download"))
        }
        var editFlags = Number(request.editFlags || 0)
        if (editFlags & 8 || String(request.selectedText || "").length > 0) {
            items.push(window.contextMenuItem("Copy selection", "web-copy", "",
                                              "browser.edit.copy"))
            items.push(window.contextMenuItem("Search selection", "selection-search", "",
                                              "browser.selection.search"))
            window.appendExternalActions(items, ui, "selection", "")
            window.appendUserscriptActions(items, ui, "selection",
                                           String(request.selectedText || ""))
        }
        if (editFlags & 1) {
            items.push(window.contextMenuItem("Undo", "web-undo"))
        }
        if (editFlags & 2) {
            items.push(window.contextMenuItem("Redo", "web-redo"))
        }
        if (editFlags & 4) {
            items.push(window.contextMenuItem("Cut", "web-cut", "", "browser.edit.cut"))
        }
        if (editFlags & 16) {
            items.push(window.contextMenuItem("Paste", "web-paste", "", "browser.edit.paste"))
            items.push(window.contextMenuItem("Paste plain text", "web-paste-plain", "", "browser.edit.paste-plain"))
        }
        if (editFlags & 32) {
            items.push(window.contextMenuItem("Delete", "web-delete"))
        }
        if (editFlags & 64) {
            items.push(window.contextMenuItem("Select all", "web-select-all"))
        }
        var suggestions = request.spellCheckerSuggestions || []
        if (String(request.misspelledWord || "").length > 0 && suggestions.length > 0) {
            for (var suggestionIndex = 0; suggestionIndex < suggestions.length
                 && suggestionIndex < 8; ++suggestionIndex) {
                var suggestion = window.boundedPageDialogText(suggestions[suggestionIndex])
                if (suggestion.length > 0) {
                    items.push(window.contextMenuItem(
                                  "Replace with " + suggestion, "spell", suggestion))
                }
            }
        }
        if (window.safeContextUrl(ui.current_url || "").length > 0) {
            items.push(window.contextMenuItem("Add page to Favourites", "favourite-add", "", "browser.tab.favourite"))
        }
        items.push(window.contextMenuItem("Inspect element", "inspect"))
        window.pendingContextMenuRequest = request
        window.pendingContextMenuView = view
        window.pendingContextMenuUi = ui
        window.pendingContextMenuLeaseToken = window.beginNativePromptLease(
                    ui, view, "context-menu")
        window.pendingContextMenuHost = hostWindow || window
        window.contextMenuItems = items
        request.accepted = true
        // Capture the native frame before transferring focus to the menu.
        var menuHost = hostWindow || window
        var menuX = Math.round(menuHost.width / 2)
        var menuY = Math.round(menuHost.height / 2)
        if (request.position && menuHost.contentItem && view.mapToItem) {
            var mappedPosition = view.mapToItem(
                        menuHost.contentItem,
                        Number(request.position.x), Number(request.position.y))
            menuX = mappedPosition.x
            menuY = mappedPosition.y
        }
        window.selectionProviderFor(view).capture(function(captured) {
            if (window.pendingContextMenuRequest !== request || window.pendingContextMenuView !== view) return
            window.pendingContextMenuSelection = captured
            window.captureOverlayFocus(hostWindow || window, view)
            window.contextMenu.openAt(menuHost, menuX, menuY)
            ui.status_text = "Context menu"
        })
    }

    function activateContextMenuItem(item) {
        var view = window.pendingContextMenuView
        var ui = window.pendingContextMenuUi || window.browserUi
        var host = window.pendingContextMenuHost
        if (!item || !view) {
            window.clearContextMenuRequest()
            return
        }
        var source = window.pendingContextMenuSelection
        var action = item.action
        var value = item.value
        var isHintAction = action === "hint-userscript-action"
                && !!window.pendingContextMenuRequest
                && !!window.pendingContextMenuRequest.hintAction
        window.clearContextMenuRequest(isHintAction)
        if (isHintAction) {
            if (host && host.hintWorkspace && host.hintWorkspace.browserUi === ui)
                host.hintWorkspace.activateHint(value, item.actionId || "")
            return
        }
        if (item.actionId && item.actionId.length > 0) {
            if (item.selectionSubject) {
                if (!source) { ui.status_text = "Selection source is unavailable"; return }
                window.selectionProviderFor(view).extract(function(result) {
                    if (!result || result.error) { ui.status_text = "Selection source changed or is unavailable"; return }
                    ui.execute_ui_action(item.actionId, result.text)
                }, source)
                return
            }
            if (item.actionId.indexOf("browser.edit.") === 0 && view.clipboardPopupToken) {
                if (!source) { ui.status_text = "Edit source is unavailable"; return }
                var popupEdit = ui.popup_clipboard_edit(view.clipboardPopupToken, item.actionId.slice("browser.edit.".length))
                if (popupEdit && popupEdit.kind) window.applyNativeEditAction(ui, view, popupEdit, source)
                return
            }
            if (!ui.execute_ui_action(item.actionId, value)) {
                return
            }
            if (item.actionId.indexOf("browser.edit.") === 0) {
                if (!source) { ui.take_engine_request(); ui.status_text = "Edit source is unavailable"; return }
                EngineRequests.execute(ui.take_engine_request(), ui, view.viewHost || window, window, source)
                return
            }
            var capturedSelectionToken = ui.take_selection_request()
            if (capturedSelectionToken.length > 0) {
                window.readSelection(ui, view, capturedSelectionToken, source)
            }
            if (action === "download-link") window.dispatchDownloadRequest(ui)
            if (ui === window.browserUi) {
                window.syncTabModel()
                window.executePendingEngineAction()
            } else if (action === "favourite-add" && host && host.executePendingEngineAction) {
                host.executePendingEngineAction()
            } else if (action === "open-link" && view) {
                var pending = ui.take_engine_action()
                if (pending.indexOf("navigate\t") === 0) {
                    var parts = pending.split("\t")
                    if (parts.length >= 3) {
                        view.url = parts.slice(2).join("\t")
                    }
                }
            } else if (action === "copy-link" && view) {
                var clipboardRequest = ui.take_clipboard_request()
                if (clipboardRequest.length > 0) {
                    window.copyToClipboard(
                                clipboardRequest,
                                !ui.clipboard_request_sensitive,
                                ui.clipboard_request_primary, ui)
                }
            } else if ((action === "web-copy" || action === "selection-search"
                        || action === "external-selection-send") && view) {
                var selectionToken = ui.take_selection_request()
                if (selectionToken.length > 0) {
                    window.readSelection(ui, view, selectionToken, source)
                }
            }
            return
        }
        if (action === "open-link") {
            ui.navigate(value)
            if (ui === window.browserUi) {
                window.updateActiveTabUrl()
            } else if (ui.initial_url !== undefined) {
                view.url = ui.initial_url
            } else {
                view.url = value
            }
            ui.status_text = "Link opened"
        } else if (action === "copy-link") {
            if (window.copyToClipboard(value, false, false, ui)) ui.status_text = "Link copied"
        } else if (action === "download-link") {
            if (ui.execute_ui_action("browser.link.download", value)) window.dispatchDownloadRequest(ui)
        } else if (action === "spell") {
            view.replaceMisspelledWord(window.boundedPageDialogText(value))
            ui.status_text = "Spelling replacement applied"
        } else if (action === "inspect") {
            view.triggerWebAction(WebEngineView.InspectElement)
            ui.status_text = "Inspect element requested"
        } else {
            var webAction = action === "web-copy" ? WebEngineView.Copy
                    : action === "web-undo" ? WebEngineView.Undo
                    : action === "web-redo" ? WebEngineView.Redo
                    : action === "web-cut" ? WebEngineView.Cut
                    : action === "web-paste" ? WebEngineView.Paste
                    : action === "web-delete" ? WebEngineView.Delete
                    : action === "web-select-all" ? WebEngineView.SelectAll
                    : WebEngineView.NoWebAction
            if (webAction !== WebEngineView.NoWebAction) {
                view.triggerWebAction(webAction)
                ui.status_text = "Edit action requested"
            }
        }
    }

    function acceptPageDialog() {
        var request = window.pendingAuthenticationRequest || window.pendingPageDialogRequest
        var ui = window.pendingPageDialogUi || window.browserUi
        var view = window.pendingPageDialogView
        var authentication = !!window.pendingAuthenticationRequest
        if (!request) {
            return
        }
        if (window.pendingAuthenticationRequest) {
            var username = window.boundedPageDialogText(window.runtime_pageDialogPopup.usernameText)
            var password = window.boundedPageDialogText(window.runtime_pageDialogPopup.passwordText)
            window.runtime_pageDialogPopup.usernameText = ""
            window.runtime_pageDialogPopup.passwordText = ""
            window.resolveQtRequest(
                ui, request, authentication ? "authentication" : "page-dialog",
                "dialogAccept", [username, password])
            username = ""
            password = ""
        } else if (window.pageDialogType(request) === "prompt") {
            window.resolveQtRequest(
                ui, request, "page-dialog", "dialogAccept",
                [window.boundedPageDialogText(window.runtime_pageDialogPopup.inputText)])
        } else {
            window.resolveQtRequest(
                ui, request, "page-dialog", "dialogAccept", [])
        }
        if (view && window.pageDialogSuppressChecked) {
            view.pageDialogSuppressed = true
        }
        window.clearPageDialog(false)
        ui.status_text = authentication ? "Authentication accepted" : "Page dialog accepted"
    }

    function rejectPageDialog() {
        var request = window.pendingAuthenticationRequest || window.pendingPageDialogRequest
        var ui = window.pendingPageDialogUi || window.browserUi
        var view = window.pendingPageDialogView
        var kind = window.pageDialogKind()
        if (!request) {
            return
        }
        if (kind === "beforeunload" && view && typeof view.noteBeforeUnloadRejected === "function")
            view.noteBeforeUnloadRejected()
        window.resolveQtRequest(
            ui, request, kind === "authentication" ? "authentication" : "page-dialog",
            "dialogReject", [])
        window.runtime_pageDialogPopup.usernameText = ""
        window.runtime_pageDialogPopup.passwordText = ""
        if (view && window.pageDialogSuppressChecked) {
            view.pageDialogSuppressed = true
        }
        window.clearPageDialog(false)
        ui.status_text = kind === "beforeunload"
                ? "Stayed on page"
                : kind === "authentication"
                  ? "Authentication rejected"
                  : "Page dialog rejected"
    }

    function permissionQueueKey(origin, permissionName, privateProfile, ui, view, tabIndex) {
        return JSON.stringify([ui ? String(ui.start_page_owner_token || "") : "",
            view ? String(view.promptLeaseIdentity || view.stableTabId || tabIndex) : tabIndex,
            origin, permissionName, !!privateProfile])
    }

    function permissionPolicyOwnerValid(group) {
        var view = group.view
        return !!group.ui && !!view && !!group.host && !group.host.runtimeReleased
            && !view.viewRetired && view.viewUi === group.ui && view.viewHost === group.host
            && !!group.policyOwner && view.documentRequestOwner === group.policyOwner
            && view.stableTabId === group.policyTabId && view.tabIndex === group.tabIndex
            && group.ui.active_tab_index === group.tabIndex && view.ownsTab && view.ownsTab()
    }

    function refreshPendingPermissionPolicies() {
        var queue = (window.permissionPromptQueue || []).slice(0)
        for (var i = 0; i < queue.length; ++i) {
            var group = queue[i]
            if (!group.policyPending || window.permissionPromptQueue.indexOf(group) < 0) continue
            if (!window.permissionPolicyOwnerValid(group)
                    || !group.requests.some(function(request) { return request && request.isValid })) {
                window.removePermissionGroups(function(candidate) { return candidate === group }, true)
                continue
            }
            var decision = String(group.ui.permission_decision(group.origin, group.name))
            if (decision === "pending" && Date.now() < group.policyDeadline) continue
            if (decision === "allow" || decision === "deny") {
                window.removePermissionGroups(function(candidate) { return candidate === group }, false)
                for (var j = 0; j < group.requests.length; ++j) {
                    window.resolveQtRequest(group.ui, group.requests[j], "permission",
                        decision === "allow" ? "grant" : "deny", [])
                }
            } else if (decision === "ask" || decision === "pending") {
                group.policyPending = false
                group.policyUnavailable = decision === "pending"
            } else {
                window.removePermissionGroups(function(candidate) { return candidate === group }, true)
            }
        }
        window.showNextPermissionPrompt()
    }

    function denyPermissionGroup(group) {
        if (!group || !group.requests) {
            return
        }
        for (var i = 0; i < group.requests.length; ++i) {
            var request = group.requests[i]
            window.resolveQtRequest(group.ui, request, "permission", "deny", [])
        }
    }

    function permissionQueueRequestCount(queue) {
        var count = 0
        for (var i = 0; i < queue.length; ++i) {
            count += queue[i].requests ? queue[i].requests.length : 0
        }
        return count
    }

    function clearDisplayedPermissionPrompt() {
        var wasVisible = window.permissionPromptVisible
        var host = window.pendingPermissionHost || window
        var leaseToken = window.pendingPermissionLeaseToken
        var leaseUi = window.permissionLeaseUi || window.browserUi
        var leaseView = window.permissionLeaseView
        window.permissionPromptVisible = false
        window.pendingPermissionRequest = null
        window.pendingPermissionUi = null
        window.pendingPermissionLeaseToken = ""
        window.pendingPermissionView = null
        window.permissionLeaseUi = null
        window.permissionLeaseView = null
        window.pendingPermissionTabIndex = -1
        window.pendingPermissionOrigin = ""
        window.pendingPermissionName = ""
        window.pendingPermissionPolicyNotice = ""
        window.pendingPermissionPrivate = false
        window.pendingPermissionGroupCount = 0
        window.pendingPermissionHost = null
        window.runtime_permissionPromptExpiry.stop()
        if (wasVisible) {
            window.restoreOverlayFocus(host)
        }
        window.completeNativePromptLease(leaseUi, leaseToken, leaseView)
    }

    function showNextPermissionPrompt() {
        var queue = window.permissionPromptQueue || []
        var group = queue.length > 0 && !queue[0].policyPending ? queue[0] : null
        var wasVisible = window.permissionPromptVisible
        // A queued prompt may belong to a different profile or window. Retire
        // the old focus return and input lease before displaying its successor.
        if (!group || (wasVisible && (window.pendingPermissionRequest !== group.requests[0]
                || window.permissionLeaseUi !== group.ui
                || window.permissionLeaseView !== (group.view || null)
                || window.pendingPermissionHost !== (group.host || window)
                || window.pendingPermissionTabIndex !== group.tabIndex))) {
            window.clearDisplayedPermissionPrompt()
            wasVisible = false
        }
        if (!group) return
        window.pendingPermissionRequest = group.requests[0] || null
        window.pendingPermissionUi = group.ui
        window.pendingPermissionView = group.view || null
        window.pendingPermissionTabIndex = group.tabIndex
        window.pendingPermissionOrigin = group.origin
        window.pendingPermissionName = group.name
        window.pendingPermissionPolicyNotice = group.policyUnavailable
            ? "Saved permissions couldn't be checked. Review this request before allowing access." : ""
        window.pendingPermissionPrivate = group.privateProfile
        window.pendingPermissionGroupCount = group.requests.length
        window.pendingPermissionHost = group.host || window
        if (!wasVisible) {
            window.captureOverlayFocus(window.pendingPermissionHost, group.view || null)
            window.permissionLeaseUi = group.ui
            window.permissionLeaseView = group.view || null
            window.pendingPermissionLeaseToken = window.beginNativePromptLease(
                        group.ui, group.view || null, "permission", group.tabIndex)
        }
        window.permissionPromptVisible = true
        if (!wasVisible) window.runtime_permissionPromptExpiry.restart()
    }

    function clearPermissionPrompt(denyRequest) {
        var queue = (window.permissionPromptQueue || []).slice(0)
        var group = queue.length > 0 ? queue.shift() : null
        if (denyRequest) {
            window.denyPermissionGroup(group)
        }
        window.permissionPromptQueue = queue
        window.showNextPermissionPrompt()
    }

    function removePermissionGroups(predicate, denyRequest) {
        var queue = window.permissionPromptQueue || []
        var kept = []
        for (var i = 0; i < queue.length; ++i) {
            var group = queue[i]
            if (predicate(group)) {
                if (denyRequest) {
                    window.denyPermissionGroup(group)
                }
            } else {
                kept.push(group)
            }
        }
        if (kept.length !== queue.length) {
            window.permissionPromptQueue = kept
            window.showNextPermissionPrompt()
        }
    }

    function queuePermissionPrompt(ui, request, tabIndex, privateProfile, hostWindow, view, policyPending) {
        var origin = request.origin.toString()
        var name = window.permissionTypeName(request.permissionType)
        var key = window.permissionQueueKey(origin, name, privateProfile, ui, view, tabIndex)
        var now = Date.now()
        var cooldowns = window.permissionCooldowns
        if (cooldowns[key] !== undefined && cooldowns[key] <= now) {
            delete cooldowns[key]
        }
        if (cooldowns[key] !== undefined) {
            window.resolveQtRequest(ui, request, "permission", "deny", [])
            ui.status_text = "Permission requests temporarily cooled down"
            return
        }
        var queue = (window.permissionPromptQueue || []).slice(0)
        var groupIndex = -1
        for (var i = 0; i < queue.length; ++i) {
            if (queue[i].key === key && queue[i].ui === ui
                    && queue[i].host === (hostWindow || window)
                    && queue[i].view === (view || null)
                    && queue[i].tabIndex === tabIndex
                    && queue[i].policyOwner === (view ? view.documentRequestOwner : null)) {
                groupIndex = i
                break
            }
        }
        if (window.permissionQueueRequestCount(queue)
                >= window.permissionPromptQueueLimit) {
            window.resolveQtRequest(ui, request, "permission", "deny", [])
            if (Object.keys(cooldowns).length >= 64) {
                cooldowns = ({})
            }
            cooldowns[key] = now + 30000
            window.permissionCooldowns = cooldowns
            ui.status_text = "Permission queue full; repeated requests cooled down"
            return
        }
        if (groupIndex >= 0) {
            var grouped = queue[groupIndex]
            grouped.requests = grouped.requests.concat([request])
            queue[groupIndex] = grouped
        } else {
            queue.push({
                key: key,
                requests: [request],
                ui: ui,
                tabIndex: tabIndex,
                origin: origin,
                name: name,
                privateProfile: privateProfile,
                host: hostWindow || window,
                view: view || null,
                policyPending: !!policyPending,
                policyDeadline: now + 2000,
                policyUnavailable: false,
                policyOwner: view ? view.documentRequestOwner : null,
                policyTabId: view ? view.stableTabId : ""
            })
        }
        window.permissionPromptQueue = queue
        if (policyPending) window.runtime_permissionPolicyRefresh.start()
        if (!window.permissionPromptVisible) {
            window.showNextPermissionPrompt()
        } else if (groupIndex === 0) {
            window.pendingPermissionGroupCount = queue[0].requests.length
        }
    }

    function handlePermissionRequested(request, tabIndex, ui, view, privateProfile, hostWindow) {
        if (!request || !request.isValid) {
            return
        }
        var permissionUi = ui || window.browserUi
        var permissionName = window.permissionTypeName(request.permissionType)
        if (permissionName.length === 0) {
            window.resolveQtRequest(permissionUi, request, "permission", "deny", [])
            permissionUi.status_text = "Blocked unsupported permission request"
            return
        }
        var origin = request.origin.toString()
        var decision = String(permissionUi.permission_decision(origin, permissionName))
        if (decision === "allow") {
            window.resolveQtRequest(permissionUi, request, "permission", "grant", [])
            permissionUi.status_text = "Allowed " + window.permissionDisplayName(permissionName)
            return
        }
        if (decision === "deny") {
            window.resolveQtRequest(permissionUi, request, "permission", "deny", [])
            permissionUi.status_text = "Denied " + window.permissionDisplayName(permissionName)
            return
        }
        if (decision !== "ask" && decision !== "pending") {
            window.resolveQtRequest(permissionUi, request, "permission", "deny", [])
            return
        }
        window.queuePermissionPrompt(
            permissionUi, request, tabIndex, !!privateProfile, hostWindow || window, view,
            decision === "pending")
    }

    function decidePermission(allow, lifetime) {
        var queue = window.permissionPromptQueue || []
        var group = queue.length > 0 ? queue[0] : null
        var ui = group && group.ui ? group.ui : window.browserUi
        if (!group || !group.requests || group.requests.length === 0) {
            window.clearPermissionPrompt(false)
            ui.status_text = "Permission request expired"
            return
        }
        if (group.policyOwner && !window.permissionPolicyOwnerValid(group)) {
            window.clearPermissionPrompt(true)
            ui.status_text = "Permission request expired"
            return
        }
        var validCount = 0
        for (var i = 0; i < group.requests.length; ++i) {
            if (group.requests[i] && group.requests[i].isValid) {
                validCount += 1
            }
        }
        if (validCount === 0) {
            window.clearPermissionPrompt(false)
            ui.status_text = "Permission request expired"
            return
        }
        if (lifetime.length > 0 && !ui.remember_permission(
                    window.pendingPermissionOrigin,
                    window.pendingPermissionName,
                    allow ? "allow" : "deny",
                    lifetime)) {
            return
        }
        for (var j = 0; j < group.requests.length; ++j) {
            var request = group.requests[j]
            if (!request || !request.isValid) {
                continue
            }
            if (allow) {
                window.resolveQtRequest(ui, request, "permission", "grant", [])
            } else {
                window.resolveQtRequest(ui, request, "permission", "deny", [])
            }
        }
        ui.status_text = (allow ? "Allowed " : "Denied ")
                + window.permissionDisplayName(window.pendingPermissionName)
        window.clearPermissionPrompt(false)
    }

    function handleImmediatePermissionRequested(ui, request, privateProfile, hostWindow, view) {
        window.handlePermissionRequested(request,
            view && view.tabIndex !== undefined ? Number(view.tabIndex) : -1,
            ui, view, privateProfile, hostWindow)
    }

    function cancelPermissionForTab(tabIndex) {
        window.removePermissionGroups(function(group) {
            return group.ui === window.browserUi && group.tabIndex === tabIndex
        }, true)
    }

    function cancelPermissionForUi(ui) {
        window.removePermissionGroups(function(group) {
            return group.ui === ui
        }, true)
    }

    function permissionPendingForTab(tabIndex) {
        var queue = window.permissionPromptQueue || []
        for (var i = 0; i < queue.length; ++i) {
            if (queue[i].tabIndex === tabIndex) {
                return true
            }
        }
        return false
    }

    function clearDesktopMediaRequest(cancelRequest) {
        var host = window.pendingDesktopMediaHost
        var request = window.pendingDesktopMediaRequest
        var hadRequest = !!request
        var ui = window.pendingDesktopMediaUi || window.browserUi
        var view = window.pendingDesktopMediaView
        var leaseToken = window.pendingDesktopMediaLeaseToken
        window.pendingDesktopMediaRequest = null
        window.pendingDesktopMediaUi = null
        window.pendingDesktopMediaView = null
        window.pendingDesktopMediaLeaseToken = ""
        window.pendingDesktopMediaHost = null
        window.pendingDesktopMediaWaitingForPortal = false
        window.pendingDesktopMediaPortalDeadlineMs = 0
        window.desktopMediaPromptVisible = false
        window.desktopMediaOrigin = ""
        if (hadRequest) {
            window.restoreOverlayFocus(host)
        }
        window.completeNativePromptLease(ui, leaseToken, view)
        if (cancelRequest && request) {
            window.resolveQtRequest(ui, request, "desktop-media", "cancel", [])
            if (ui) {
                ui.status_text = "Screen sharing cancelled"
            }
        }
    }

    function maybeOpenPendingDesktopMediaRequest() {
        if (!window.pendingDesktopMediaWaitingForPortal
                || !window.pendingDesktopMediaRequest) {
            return
        }
        if (Date.now() >= window.pendingDesktopMediaPortalDeadlineMs) {
            var timedOutUi = window.pendingDesktopMediaUi || window.browserUi
            window.clearDesktopMediaRequest(true)
            if (timedOutUi) {
                timedOutUi.status_text =
                        "Desktop portal check timed out; screen sharing cancelled"
            }
            return
        }
        var ui = window.pendingDesktopMediaUi || window.browserUi
        var status = window.desktopPortalCapabilityStatus(ui, "screen_cast")
        if (status === "pending" || status === "not-probed") {
            return
        }
        if (status !== "available") {
            window.clearDesktopMediaRequest(true)
            ui.status_text = "Required ScreenCast portal unavailable; screen sharing cancelled"
            return
        }
        window.pendingDesktopMediaWaitingForPortal = false
        window.desktopMediaPromptVisible = true
        ui.status_text = "Choose a screen or window to share"
    }

    function handleDesktopMediaRequested(ui, view, request, hostWindow) {
        if (!request) {
            return
        }
        if (window.pendingDesktopMediaRequest) {
            window.clearDesktopMediaRequest(true)
        }
        window.pendingDesktopMediaRequest = request
        window.pendingDesktopMediaUi = ui
        window.pendingDesktopMediaView = view
        window.pendingDesktopMediaLeaseToken = window.beginNativePromptLease(
                    ui, view, "desktop-media")
        window.pendingDesktopMediaHost = hostWindow || window
        window.captureOverlayFocus(window.pendingDesktopMediaHost, view)
        window.desktopMediaOrigin = view && view.url && view.url.host
                ? window.boundedPageDialogText(view.url.host)
                : "opaque or unavailable origin"
        var requestUi = window.pendingDesktopMediaUi || window.browserUi
        if (window.desktopPortalMode(requestUi) === "required") {
            var portalStatus = window.desktopPortalCapabilityStatus(requestUi, "screen_cast")
            if (portalStatus === "not-probed") {
                window.browserUi.probe_desktop_portals()
                portalStatus = window.desktopPortalCapabilityStatus(requestUi, "screen_cast")
            }
            if (portalStatus === "pending" || portalStatus === "not-probed") {
                window.pendingDesktopMediaWaitingForPortal = true
                window.pendingDesktopMediaPortalDeadlineMs = Date.now() + 5000
                requestUi.status_text = "Checking required ScreenCast portal…"
                return
            }
            if (portalStatus !== "available") {
                window.clearDesktopMediaRequest(true)
                requestUi.status_text = "Required ScreenCast portal unavailable; screen sharing cancelled"
                return
            }
        }
        window.pendingDesktopMediaWaitingForPortal = false
        window.desktopMediaPromptVisible = true
        requestUi.status_text = "Choose a screen or window to share"
    }

    function selectDesktopScreen(index) {
        var request = window.pendingDesktopMediaRequest
        var ui = window.pendingDesktopMediaUi || window.browserUi
        if (!request || !request.screensModel) {
            window.clearDesktopMediaRequest(true)
            return
        }
        window.resolveQtRequest(
            ui, request, "desktop-media", "selectScreen", [request.screensModel.index(index, 0)])
        window.recordCaptureSession(ui, window.pendingDesktopMediaView,
                                    window.pendingDesktopMediaHost,
                                    window.desktopMediaOrigin)
        window.clearDesktopMediaRequest(false)
        ui.status_text = "Screen sharing source selected"
    }

    function selectDesktopWindow(index) {
        var request = window.pendingDesktopMediaRequest
        var ui = window.pendingDesktopMediaUi || window.browserUi
        if (!request || !request.windowsModel) {
            window.clearDesktopMediaRequest(true)
            return
        }
        window.resolveQtRequest(
            ui, request, "desktop-media", "selectWindow", [request.windowsModel.index(index, 0)])
        window.recordCaptureSession(ui, window.pendingDesktopMediaView,
                                    window.pendingDesktopMediaHost,
                                    window.desktopMediaOrigin)
        window.clearDesktopMediaRequest(false)
        ui.status_text = "Window sharing source selected"
    }

    function clearDesktopMediaForView(view) {
        if (window.pendingDesktopMediaView === view) {
            window.clearDesktopMediaRequest(true)
        }
    }

    function recordCaptureSession(ui, view, hostWindow, origin) {
        if (!view || !hostWindow) {
            return
        }
        var sessions = (window.captureSessions || []).filter(function(session) {
            return session && session.view !== view
        })
        var sequence = window.captureSessionSequence + 1
        window.captureSessionSequence = sequence
        sessions.push({
            id: sequence,
            ui: ui,
            view: view,
            host: hostWindow,
            origin: window.boundedPageDialogText(origin || "opaque or unavailable origin"),
            status: "active"
        })
        window.captureSessions = sessions.slice(-8)
    }

    function captureSessionForView(view) {
        var sessions = window.captureSessions || []
        for (var i = 0; i < sessions.length; ++i) {
            if (sessions[i] && sessions[i].view === view) {
                return sessions[i]
            }
        }
        return null
    }

    function noteCaptureNavigation(view) {
        var session = window.captureSessionForView(view)
        if (!session) {
            return
        }
        var sessions = (window.captureSessions || []).slice(0)
        for (var i = 0; i < sessions.length; ++i) {
            if (sessions[i] && sessions[i].view === view) {
                sessions[i].status = "ended-by-navigation"
            }
        }
        window.captureSessions = sessions
        session.ui.status_text = "Capture ended with page navigation"
    }

    function stopCaptureSession(id) {
        var sessions = (window.captureSessions || []).slice(0)
        var target = null
        for (var i = 0; i < sessions.length; ++i) {
            if (sessions[i] && sessions[i].id === id) {
                target = sessions[i]
                break
            }
        }
        if (!target || !target.view) {
            return
        }
        target.status = "stop-requested"
        window.captureSessions = sessions
        target.ui.status_text = "Capture stop requested; reloading the page"
        target.view.reload()
    }

    function dismissCaptureSession(id) {
        window.captureSessions = (window.captureSessions || []).filter(function(session) {
            return session && session.id !== id
        })
    }

    function clearCaptureSessionForView(view) {
        window.captureSessions = (window.captureSessions || []).filter(function(session) {
            return session && session.view !== view
        })
    }

    function clearCaptureSessionsForHost(hostWindow) {
        window.captureSessions = (window.captureSessions || []).filter(function(session) {
            return session && session.host !== hostWindow
        })
    }

    function permissionOriginForView(view) {
        var raw = view && view.url ? view.url.toString() : ""
        var schemeEnd = raw.indexOf("://")
        if (schemeEnd < 1) {
            return ""
        }
        var authorityStart = schemeEnd + 3
        var authorityEnd = raw.length
        for (var i = 0; i < 3; ++i) {
            var separator = raw.indexOf(["/", "?", "#"][i], authorityStart)
            if (separator >= 0 && separator < authorityEnd) {
                authorityEnd = separator
            }
        }
        var authority = raw.slice(authorityStart, authorityEnd)
        var at = authority.lastIndexOf("@")
        if (at >= 0) {
            authority = authority.slice(at + 1)
        }
        return raw.slice(0, schemeEnd).toLowerCase() + "://"
                + authority.toLowerCase()
    }

    function applyPermissionReset(ui, origin, permission) {
        if (!ui) return
        if (permission === "notifications") window.closeWebNotificationsForOrigin(origin, ui)
        var reloaded = window.reloadViewsForPermission(origin, ui)
        ui.status_text = reloaded > 0
            ? "Permission reset; reloaded " + reloaded + (reloaded === 1 ? " page" : " pages") + " in this profile"
            : "Permission reset; no matching page was reloaded"
    }

    function reloadViewsForPermission(origin, sourceUi) {
        var issuer = sourceUi || window.browserUi
        var expected = String(origin || "").toLowerCase()
        if (!expected || !window.sharesPermissionSession(issuer, issuer)) return 0
        var candidates = []
        function addCandidate(view, ui, grantedOrigin) {
            if (!view || view.viewRetired === true || !ui || view.viewUi !== ui
                    || !window.sharesPermissionSession(issuer, ui)
                    || !view.ownsTab()
                    || (window.permissionOriginForView(view) !== expected
                        && String(grantedOrigin || "").toLowerCase() !== expected)) return
            if (!candidates.some(function(entry) { return entry.view === view }))
                candidates.push({ view: view, ui: ui })
        }

        var entries = window.liveWindowEntries()
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (!window.sharesPermissionSession(issuer, entry.ui)) continue
            var views = (entry.host.tabViews || []).slice(0)
            views.push(entry.host.activeWebView ? entry.host.activeWebView() : entry.view)
            for (var j = 0; j < views.length; ++j) addCandidate(views[j], entry.ui, "")
        }
        var sessions = window.captureSessions || []
        for (var k = 0; k < sessions.length; ++k) {
            var session = sessions[k]
            if (session && session.status === "active")
                addCandidate(session.view, session.ui, session.origin)
        }
        window.removePermissionGroups(function(group) {
            return group && String(group.origin || "").toLowerCase() === expected
                && window.sharesPermissionSession(issuer, group.ui)
        }, true)
        for (var n = 0; n < candidates.length; ++n) {
            window.noteCaptureNavigation(candidates[n].view)
            candidates[n].view.reload()
        }
        return candidates.length
    }

    function rendererTerminationName(status) {
        if (status === WebEngineView.CrashedTerminationStatus) {
            return "crashed"
        }
        if (status === WebEngineView.KilledTerminationStatus) {
            return "killed"
        }
        if (status === WebEngineView.AbnormalTerminationStatus) {
            return "abnormal termination"
        }
        return "terminated"
    }

    function handleRendererProcessTerminated(ui, view, hostWindow, tabIndex,
                                              terminationStatus, exitCode) {
        if (!view || terminationStatus === WebEngineView.NormalTerminationStatus) {
            return
        }
        // A failed renderer cannot complete its outstanding requests. Release
        // their actual window owners before offering recovery for the page.
        window.cancelScriptsForView(view)
        window.clearPageDialogForView(view)
        window.clearClientCertificateForView(view)
        window.clearCertificateErrorForView(view)
        window.clearWebAuthForView(view)
        window.clearContextMenuForView(view)
        window.clearDesktopMediaForView(view)
        window.clearCaptureSessionForView(view)
        window.clearSiteDataClearForView(view)
        var requestHost = hostWindow || view.viewHost || window
        if (requestHost.clearFileDialogForView) requestHost.clearFileDialogForView(view)
        window.removePermissionGroups(function(group) {
            return group.ui === ui && group.tabIndex === tabIndex
        }, true)
        if (ui && ui.note_renderer_process_terminated) {
            ui.note_renderer_process_terminated(tabIndex)
        }
        var now = Date.now()
        var count = view.rendererFailureAt > 0
                && now - view.rendererFailureAt <= 60000
                ? view.rendererFailureCount + 1 : 1
        view.rendererFailureAt = now
        view.rendererFailureCount = count
        view.rendererFailed = true
        view.rendererFailureSafeUrl = ui.display_url || "about:blank"
        view.rendererFailureReason = window.rendererTerminationName(terminationStatus)
        view.rendererFailureExitCode = exitCode
        window.rendererFailureView = view
        window.rendererFailureHost = hostWindow || window
        window.rendererFailureUi = ui
        window.rendererFailureTabIndex = tabIndex
        window.rendererFailureSafeUrl = view.rendererFailureSafeUrl
        window.rendererFailureReason = view.rendererFailureReason
        window.rendererFailureExitCode = exitCode
        window.rendererFailureCount = count
        var visible = window.rendererFailureHost !== window
                || tabIndex === window.browserUi.active_tab_index
        if (visible) {
            window.captureOverlayFocus(window.rendererFailureHost || window, view)
        }
        window.rendererFailureVisible = visible
        ui.status_text = visible
                ? "This page stopped working; reload to continue"
                : "A page stopped working in a background tab"
    }

    function showRendererFailureForTab(tabIndex) {
        var view = window.tabViewAt(tabIndex)
        if (!view || !view.rendererFailed) {
            return
        }
        window.rendererFailureView = view
        window.rendererFailureHost = window
        window.rendererFailureUi = window.browserUi
        window.rendererFailureTabIndex = tabIndex
        window.rendererFailureSafeUrl = view.rendererFailureSafeUrl
        window.rendererFailureReason = view.rendererFailureReason
        window.rendererFailureExitCode = view.rendererFailureExitCode
        window.rendererFailureCount = view.rendererFailureCount
        window.captureOverlayFocus(window, view)
        window.rendererFailureVisible = true
    }

    function clearRendererFailureForView(view) {
        var host = window.rendererFailureHost
        if (!view) {
            return
        }
        view.rendererFailed = false
        if (window.rendererFailureView === view) {
            var wasVisible = window.rendererFailureVisible
            window.rendererFailureVisible = false
            window.rendererFailureView = null
            window.rendererFailureHost = null
            window.rendererFailureUi = null
            window.rendererFailureTabIndex = -1
            if (wasVisible) {
                window.restoreOverlayFocus(host)
            }
        }
    }

    function reloadRendererFailure() {
        var host = window.rendererFailureHost
        var view = window.rendererFailureView
        var ui = window.rendererFailureUi || window.browserUi
        if (!view) {
            return
        }
        if (ui && ui.prepare_renderer_recovery
                && !ui.prepare_renderer_recovery(window.rendererFailureTabIndex)) {
            ui.status_text = "The page changed. Select the affected tab and reload it."
            return
        }
        window.rendererFailureVisible = false
        view.rendererFailed = false
        view.reload()
        window.restoreOverlayFocus(host)
        ui.status_text = window.rendererFailureCount > 1
                ? "Renderer reload requested; repeated crashes will not auto-reload"
                : "Renderer reload requested"
    }

    function closeRendererFailure() {
        var view = window.rendererFailureView
        var host = window.rendererFailureHost
        var index = window.rendererFailureTabIndex
        if (host === window && index >= 0) {
            view.rendererFailed = false
            if (window.closeTabAtIndex(index)) {
                window.executePendingEngineAction()
            }
        } else if (host && host.close) {
            host.close()
        }
        if (view && host !== window) {
            view.rendererFailed = false
        }
        window.rendererFailureVisible = false
        window.restoreOverlayFocus(host)
    }

    function showRendererFailureDiagnostics() {
        var ui = window.rendererFailureUi || window.browserUi
        var payload = ui.refresh_site_status()
        if (payload && payload.length > 0) {
            try {
                window.siteLedgerData = JSON.parse(payload)
                if (!window.siteLedgerVisible) {
                    window.openInternalSurface()
                }
                window.siteLedgerVisible = true
                return
            } catch (error) {
                // Fall through to the explicit status below.
            }
        }
        ui.status_text = "Renderer diagnostics unavailable"
    }

    function restartSoftwareRendering() {
        var ui = window.rendererFailureUi || window.browserUi
        if (window.softwareRendering) {
            ui.status_text = "Software rendering is already enabled"
            return
        }
        if (!window.browserUi.restart_software_rendering(
                    window.instanceLockPath,
                    window.storageBasePath,
                    window.temporaryProfile,
                    window.safeMode,
                    window.userscriptsOff,
                    window.instanceSelector)) {
            ui.status_text = "Software-rendering restart could not be started"
            return
        }
        ui.status_text = "Restarting with software rendering"
        window.continueQuitRequest()
    }

    function dispatchDownloadRequest(ui) {
        var owner = window.profileResourceRegistry.downloadInvocation(ui)
        if (owner && owner.host.downloadWorkspaceObject) owner.host.downloadWorkspaceObject.dispatchRequest()
        else if (ui && ui.take_download_request()) ui.fail_download_request(ui.download_request_token)
    }

    function attachDownloadStateUpdates(download, id, ui, host) {
        var ownerUi = ui || window.browserUi
        var ownerHost = host || window
        function update() {
            if (!ownerUi || !ownerHost || ownerHost.runtimeReleased === true) return
            ownerUi.update_download(id, window.downloadStateName(download), download.receivedBytes, false)
            if (ownerHost.downloadManagerVisible) ownerHost.refreshDownloads()
        }
        download.stateChanged.connect(update)
        download.isPausedChanged.connect(update)
        download.receivedBytesChanged.connect(update)
    }

    function downloadStagingDirectory(ui, id) {
        return ui && ui.download_staging_directory
                ? String(ui.download_staging_directory(id)) : ""
    }

    function acceptDownloadRequest(download, id, directory, suggestedName) {
        var finalName = window.browserUi.accept_download(id, directory, suggestedName)
        if (finalName.length === 0) {
            window.profileResourceRegistry.cancelDownloadRequest(window.browserUi, download)
            delete window.activeDownloads[id]
            return false
        }
        var stagingDirectory = window.downloadStagingDirectory(window.browserUi, id)
        if (stagingDirectory.length === 0) {
            window.profileResourceRegistry.cancelDownloadRequest(window.browserUi, download)
            window.browserUi.update_download(id, "cancelled", download.receivedBytes, true)
            return false
        }
        download.downloadDirectory = stagingDirectory
        download.downloadFileName = finalName
        window.attachDownloadStateUpdates(download, id)
        window.resolveQtRequest(window.browserUi, download, "download", "accept", [])
        window.browserUi.update_download(id, "in-progress", 0, false)
        if (window.downloadManagerVisible) {
            window.refreshDownloads()
        }
        return true
    }

    function acceptPendingDownload() {
        var id = window.pendingDownloadId
        var download = window.pendingDownloadRequests[id]
        if (!download) {
            window.pendingDownloadId = ""
            window.restoreOverlayFocus()
            return
        }
        var selectedPath = window.browserUi.local_file_path(window.fileDialogSurfaces.downloadSelectedFile)
        var finalName = window.browserUi.accept_download_path(id, selectedPath)
        if (finalName.length === 0) {
            window.profileResourceRegistry.cancelDownloadRequest(window.browserUi, download)
            window.browserUi.update_download(id, "cancelled", download.receivedBytes, true)
        } else {
            var separator = selectedPath.lastIndexOf("/")
            var directory = separator > 0 ? selectedPath.slice(0, separator) : "/"
            var stagingDirectory = window.downloadStagingDirectory(window.browserUi, id)
            if (stagingDirectory.length === 0) {
                window.profileResourceRegistry.cancelDownloadRequest(window.browserUi, download)
                window.browserUi.update_download(id, "cancelled", download.receivedBytes, true)
                delete window.pendingDownloadRequests[id]
                window.pendingDownloadId = ""
                window.pendingDownloadSuggestedName = ""
                window.restoreOverlayFocus()
                return
            }
            download.downloadDirectory = stagingDirectory
            download.downloadFileName = finalName
            window.activeDownloads[id] = download
            window.attachDownloadStateUpdates(download, id)
            window.resolveQtRequest(window.browserUi, download, "download", "accept", [])
            window.browserUi.update_download(id, "in-progress", 0, false)
        }
        delete window.pendingDownloadRequests[id]
        window.pendingDownloadId = ""
        window.pendingDownloadSuggestedName = ""
        if (window.downloadManagerVisible) {
            window.refreshDownloads()
        }
        window.restoreOverlayFocus()
    }

    function failPendingDownload(message) {
        var id = window.pendingDownloadId
        var download = window.pendingDownloadRequests[id]
        if (download) {
            window.profileResourceRegistry.markDownloadSelectionFailed(download)
            window.profileResourceRegistry.cancelDownloadRequest(window.browserUi, download)
            // Retain the existing retryable download record and URL. A picker
            // failure is an interruption, not a user cancellation.
            window.browserUi.update_download(id, "interrupted", download.receivedBytes, true)
        }
        delete window.pendingDownloadRequests[id]
        window.pendingDownloadId = ""
        window.pendingDownloadSuggestedName = ""
        if (window.downloadManagerVisible) window.refreshDownloads()
        window.restoreOverlayFocus()
        window.browserUi.status_text = message
    }

    function cancelPendingDownload() {
        var id = window.pendingDownloadId
        var download = window.pendingDownloadRequests[id]
        if (download) {
            window.profileResourceRegistry.cancelDownloadRequest(window.browserUi, download)
            window.browserUi.update_download(id, "cancelled", download.receivedBytes, true)
        }
        delete window.pendingDownloadRequests[id]
        window.pendingDownloadId = ""
        window.pendingDownloadSuggestedName = ""
        if (window.downloadManagerVisible) {
            window.refreshDownloads()
        }
        window.restoreOverlayFocus()
    }

    function handleDownloadRequested(download, id, safeName) {
        var requestedPath = download.savePageFormat === WebEngineDownloadRequest.MimeHtmlSaveFormat
                ? window.browserUi.take_save_page_path() : ""
        if (requestedPath.length > 0) {
            var requestedName = window.browserUi.accept_download_path(id, requestedPath)
            if (requestedName.length === 0) {
                window.profileResourceRegistry.cancelDownloadRequest(window.browserUi, download)
                window.browserUi.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            var requestedSeparator = requestedPath.lastIndexOf("/")
            var requestedDirectory = requestedSeparator > 0
                    ? requestedPath.slice(0, requestedSeparator) : "/"
            window.activeDownloads[id] = download
            var stagingDirectory = window.downloadStagingDirectory(window.browserUi, id)
            if (stagingDirectory.length === 0) {
                window.profileResourceRegistry.cancelDownloadRequest(window.browserUi, download)
                window.browserUi.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            download.downloadDirectory = stagingDirectory
            download.downloadFileName = requestedName
            window.attachDownloadStateUpdates(download, id)
            window.resolveQtRequest(window.browserUi, download, "download", "accept", [])
            window.browserUi.update_download(id, "in-progress", 0, false)
            return
        }
        var directory = window.browserUi.default_download_directory()
        if (!window.downloadsAskDestination()) {
            window.activeDownloads[id] = download
            window.acceptDownloadRequest(download, id, directory, safeName)
            return
        }
        if (window.pendingDownloadId.length > 0) {
            window.profileResourceRegistry.cancelDownloadRequest(window.browserUi, download)
            window.browserUi.update_download(id, "cancelled", download.receivedBytes, true)
            return
        }
        window.pendingDownloadRequests[id] = download
        window.pendingDownloadId = id
        window.pendingDownloadSuggestedName = safeName
        window.captureOverlayFocus(window, window.activeWebView())
        window.browserUi.update_download(id, "selecting-destination", 0, false)
        window.fileDialogSurfaces.downloadCurrentFile = window.fileUrlForPath(directory + "/" + safeName)
        window.fileDialogSurfaces.openDownload()
    }

    function handleDownloadFinished(download, selectionFailed, id) {
        var state = selectionFailed ? "interrupted" : window.downloadStateName(download)
        if (state === "completed") {
            window.browserUi.update_download(id, "completed", download.receivedBytes, false)
            if (window.browserUi.finalize_download(id)) return true
            state = "interrupted"
        }
        window.browserUi.discard_download_staging(id)
        window.browserUi.update_download(id, state, download.receivedBytes, true)
        window.completeFinishedDownload(id)
        return false
    }

    function completeFinishedDownload(id) {
        delete window.activeDownloads[id]
        delete window.pendingDownloadRequests[id]
        if (window.pendingDownloadId === id) {
            window.pendingDownloadId = ""
            window.pendingDownloadSuggestedName = ""
        }
        if (window.downloadManagerVisible) window.refreshDownloads()
    }
}
