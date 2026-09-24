import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as BrowserScripts
import "../scripts/SpellcheckPresentation.js" as SpellcheckPresentation

FerricBrowserRuntimePresentation {
    id: window

    readonly property var switcherRefreshTimer: window.switcherRefreshTimerObject
    readonly property var switcherBatchTimer: window.switcherBatchTimerObject
    readonly property var requestPresentationController: window.requestPresentationControllerObject

    Connections {
        target: browserUi
        function onActive_tab_indexChanged() {
            window.noteTabActivity()
        }
        function onTab_countChanged() {
            window.noteTabActivity()
        }
        function onUserscript_install_stateChanged() {
            var state = browserUi.userscript_install_state || "idle"
            if (state === "installed") {
                window.settingsNotice = "Userscript installed"
            } else if (state === "removed") {
                window.settingsNotice = "Userscript removed"
            } else if (state.indexOf("error:") === 0) {
                window.settingsNotice = "Userscript operation failed: "
                        + state.substring(6)
            }
        }
        function onContext_route_idChanged() {
            window.showContextRoute(browserUi)
        }
        function onContext_workspaceChanged() {
            window.routeContextWorkspace(browserUi)
        }
        function onModeChanged() {
            if (browserUi.mode === "command" || browserUi.mode === "search") {
                window.captureModeFocus()
                Qt.callLater(function() {
                    if (browserUi.mode === "command") {
                        commandSurface.focusInput()
                    } else if (browserUi.mode === "search") {
                        searchSurface.focusInput()
                    }
                })
            } else {
                window.restoreModeFocus()
            }
            if (browserUi.mode === "hint") {
                window.startHintCollection()
            } else {
                if (browserUi.mode !== "caret") {
                    window.caretSelecting = false
                }
                window.rapidHintConfirmationVisible = false
                if (window.hintResults.length > 0) {
                    window.hintResults = []
                    window.hintInput = ""
                }
            }
        }
        function onConfig_jsonChanged() {
            window.refreshChromeAppearance()
            window.configureLinkCleaningUpdateTimer()
            window.refreshSpellcheckInventory(browserUi)
            if (browserProfile) {
                browserProfile.spellCheckEnabled = window.spellcheckEnabled(browserUi)
                browserProfile.spellCheckLanguages = window.spellcheckLanguages(browserUi)
            }
            if (window.settingsVisible) {
                window.refreshSettings()
            }
        }
        function onTheme_background_colorChanged() {
            window.refreshChromeAppearance()
        }
        function onSystem_font_scaleChanged() {
            window.refreshChromeAppearance()
        }
        function onExternal_navigation_visibleChanged() {
            if (browserUi.external_navigation_visible) {
                window.openInternalSurface()
            } else {
                window.closeInternalSurface()
            }
        }
        function onBinding_overlayChanged() {
            window.bindingOverlayVisible = false
            bindingOverlayTimer.stop()
            if (browserUi.binding_overlay.length > 0) {
                bindingOverlayTimer.restart()
            }
        }
    }

    function activateSwitcher(index) {
        var result = switcherResults[index]
        if (!result) {
            return
        }
        var owner = window.switcherOwner(result)
        var generation = result.generation === null || result.generation === undefined
                ? "" : String(result.generation)
        if (owner.ui.activate_switcher_result(result.kind, result.id, generation)) {
            if (owner.host === window) {
                window.syncTabModel()
                window.executePendingEngineAction()
            } else if (owner.host.applySwitcherEngineAction) {
                owner.host.applySwitcherEngineAction()
            }
            closeSwitcher()
            window.activateBrowserWindow(owner.host, owner.ui)
            if (result.workspace) {
                window.routeContextWorkspace(owner.ui)
            }
        }
    }

    function activateSwitcherAction(index, action) {
        var result = switcherResults[index]
        if (!result || !action) {
            return
        }
        var owner = window.switcherOwner(result)
        var generation = result.generation === null || result.generation === undefined
                ? "" : String(result.generation)
        if (owner.ui.activate_switcher_action(result.kind, result.id, action, generation)) {
            if (owner.host === window) {
                window.syncTabModel()
                window.executePendingEngineAction()
            } else if (owner.host.applySwitcherEngineAction) {
                owner.host.applySwitcherEngineAction()
            }
            closeSwitcher()
            window.activateBrowserWindow(owner.host, owner.ui)
            if (result.workspace) {
                window.routeContextWorkspace(owner.ui)
            }
        }
    }

    Shortcut {
        sequence: "Ctrl+P"
        onActivated: window.showSwitcher()
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

    function downloadsAskDestination() {
        return browserUi.feature_downloads_ask_destination
    }

    function permissionTypeName(permissionType) {
        return requestPresentationController.permissionTypeName(permissionType)
    }

    function permissionDisplayName(permissionName) {
        return requestPresentationController.permissionDisplayName(permissionName)
    }

    function permissionCanRemember(origin, permissionName) {
        return requestPresentationController.permissionCanRemember(origin, permissionName)
    }

    function permissionCanRememberForSite(origin, permissionName) {
        return requestPresentationController.permissionCanRememberForSite(origin, permissionName)
    }

    function boundedPageDialogText(value) {
        return requestPresentationController.boundedPageDialogText(value)
    }

    function pageDialogType(request) {
        return requestPresentationController.pageDialogType(request)
    }

    function pageDialogKind() {
        return requestPresentationController.pageDialogKind()
    }

    function clearPageDialog(rejectRequest) {
        var request = window.pendingAuthenticationRequest || window.pendingPageDialogRequest
        var hadRequest = !!request
        var ui = window.pendingPageDialogUi || browserUi
        var authentication = window.pageDialogIsAuthentication
        window.pendingPageDialogRequest = null
        window.pendingAuthenticationRequest = null
        window.pendingPageDialogUi = null
        window.pendingPageDialogView = null
        window.pageDialogIsAuthentication = false
        window.pageDialogMessage = ""
        window.pageDialogTitle = "Page dialog"
        window.pageDialogOrigin = ""
        window.pageDialogDefaultText = ""
        window.pageDialogSuppressChecked = false
        pageDialogPopup.usernameText = ""
        pageDialogPopup.passwordText = ""
        pageDialogPopup.inputText = ""
        pageDialogPopup.close()
        if (hadRequest) {
            window.restoreOverlayFocus()
        }
        if (rejectRequest && request) {
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

    function handleJavaScriptDialogRequested(ui, view, request, privateProfile) {
        if (!request) {
            return
        }
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
        window.captureOverlayFocus(window, view)
        window.pageDialogMessage = window.boundedPageDialogText(request.message)
        var title = window.boundedPageDialogText(request.title)
        window.pageDialogTitle = title.length > 0 ? title : "Page " + kind
        window.pageDialogOrigin = request.securityOrigin && request.securityOrigin.host
                ? window.boundedPageDialogText(request.securityOrigin.host)
                : "opaque or unavailable origin"
        window.pageDialogDefaultText = window.boundedPageDialogText(request.defaultText)
        window.pageDialogSuppressChecked = false
        pageDialogPopup.inputText = window.pageDialogDefaultText
        pageDialogPopup.open()
        ui.status_text = privateProfile ? "Private page dialog" : "Page dialog"
    }

    function handleAuthenticationDialogRequested(ui, view, request, privateProfile) {
        if (!request) {
            return
        }
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
        window.captureOverlayFocus(window, view)
        window.pageDialogIsAuthentication = true
        window.pageDialogTitle = request.proxyHost
                ? "Proxy authentication"
                : "HTTP authentication"
        window.pageDialogOrigin = request.proxyHost
                ? window.boundedPageDialogText(request.proxyHost)
                : (request.url && request.url.host
                   ? window.boundedPageDialogText(request.url.host)
                   : "opaque or unavailable origin")
        var realm = window.boundedPageDialogText(request.realm)
        window.pageDialogMessage = realm.length > 0
                ? "Credentials requested for realm: " + realm
                : "This site is requesting credentials"
        window.pageDialogSuppressChecked = false
        pageDialogPopup.usernameText = ""
        pageDialogPopup.passwordText = ""
        pageDialogPopup.open()
        ui.status_text = privateProfile ? "Private authentication prompt" : "Authentication prompt"
    }

    function clearClientCertificateRequest(selectNone) {
        var selection = window.pendingClientCertificateSelection
        var hadSelection = !!selection
        var ui = window.pendingClientCertificateUi || browserUi
        window.pendingClientCertificateSelection = null
        window.pendingClientCertificateUi = null
        window.pendingClientCertificateView = null
        window.pendingClientCertificateOptions = []
        window.pendingClientCertificateHost = ""
        certificatePrompts.closeClientCertificate()
        if (hadSelection && selectNone) {
            window.resolveQtRequest(ui, selection, "client-certificate", "selectNone", [])
        }
        if (hadSelection) {
            window.restoreOverlayFocus()
        }
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
        window.pendingClientCertificateOptions = options
        window.pendingClientCertificateHost = selection.host && selection.host.host
                ? window.boundedPageDialogText(selection.host.host)
                : "opaque or unavailable host"
        window.captureOverlayFocus(hostWindow || window, view)
        certificatePrompts.openClientCertificate()
        ui.status_text = privateProfile
                ? "Private client certificate selection"
                : "Client certificate selection"
    }

    function acceptClientCertificate(index) {
        var selection = window.pendingClientCertificateSelection
        var ui = window.pendingClientCertificateUi || browserUi
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
        var ui = window.clearClientCertificateRequest(true) || browserUi
        ui.status_text = "Client certificate request cancelled"
    }

    function googleCertificateHost(host) {
        var value = String(host || "").toLowerCase()
        return value === "google.com" || value.indexOf("google.") === 0
                || value.indexOf(".google.") >= 0 || value.endsWith(".google")
    }

    function clearCertificateErrorRequest(rejectRequest) {
        var error = window.pendingCertificateError
        var hadError = !!error
        var ui = window.pendingCertificateErrorUi || browserUi
        window.pendingCertificateError = null
        window.pendingCertificateErrorUi = null
        window.pendingCertificateErrorView = null
        window.pendingCertificateErrorHost = ""
        window.pendingCertificateErrorDescription = ""
        certificatePrompts.closeCertificateError()
        if (hadError && rejectRequest) {
            window.resolveQtRequest(ui, error, "client-certificate", "rejectCertificate", [])
        }
        if (hadError) {
            window.restoreOverlayFocus()
        }
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
        var host = error.url && error.url.host ? String(error.url.host) : ""
        if (!error.overridable || !error.isMainFrame) {
            window.resolveQtRequest(ui, error, "client-certificate", "rejectCertificate", [])
            ui.status_text = error.isMainFrame
                    ? "TLS certificate error blocked"
                    : "TLS certificate error blocked for a subresource"
            return
        }
        if (window.googleCertificateHost(host)) {
            window.resolveQtRequest(ui, error, "client-certificate", "rejectCertificate", [])
            ui.status_text = "TLS certificate error blocked for protected host"
            return
        }
        window.pendingCertificateError = error
        window.pendingCertificateErrorUi = ui
        window.pendingCertificateErrorView = view
        window.pendingCertificateErrorHost = window.boundedPageDialogText(
                    host || "opaque or unavailable host")
        window.pendingCertificateErrorDescription = window.boundedPageDialogText(
                    error.description || "The certificate could not be verified")
        window.captureOverlayFocus(hostWindow || window, view)
        certificatePrompts.openCertificateError()
        ui.status_text = "TLS certificate confirmation required"
    }

    function acceptCertificateError() {
        var error = window.pendingCertificateError
        var ui = window.pendingCertificateErrorUi || browserUi
        if (!error) {
            return
        }
        window.resolveQtRequest(ui, error, "client-certificate", "acceptCertificate", [])
        window.clearCertificateErrorRequest(false)
        ui.status_text = "TLS certificate accepted for this request only"
    }

    function rejectCertificateError() {
        var ui = window.clearCertificateErrorRequest(true) || browserUi
        ui.status_text = "TLS certificate rejected"
    }

    function webAuthStateLabel(state) {
        if (state === WebEngineWebAuthUxRequest.SelectAccount) {
            return "Select an account"
        }
        if (state === WebEngineWebAuthUxRequest.CollectPin) {
            return "Enter your security-key PIN"
        }
        if (state === WebEngineWebAuthUxRequest.FinishTokenCollection) {
            return "Touch or confirm your security key"
        }
        if (state === WebEngineWebAuthUxRequest.RequestFailed) {
            return "Security-key request failed"
        }
        if (state === WebEngineWebAuthUxRequest.Completed) {
            return "Security-key request completed"
        }
        if (state === WebEngineWebAuthUxRequest.Cancelled) {
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
        if (state === WebEngineWebAuthUxRequest.Completed) {
            var completedUi = window.clearWebAuthRequest(false) || browserUi
            completedUi.status_text = "WebAuthn completed"
        } else if (state === WebEngineWebAuthUxRequest.Cancelled) {
            var cancelledUi = window.clearWebAuthRequest(false) || browserUi
            cancelledUi.status_text = "WebAuthn cancelled"
        }
    }

    function clearWebAuthRequest(cancelRequest) {
        var request = window.pendingWebAuthRequest
        var hadRequest = !!request
        var ui = window.pendingWebAuthUi || browserUi
        window.pendingWebAuthRequest = null
        window.pendingWebAuthUi = null
        window.pendingWebAuthView = null
        window.pendingWebAuthUserNames = []
        window.pendingWebAuthState = null
        window.pendingWebAuthRelyingParty = ""
        window.webAuthStatusText = ""
        webAuthPrompt.pin = ""
        webAuthPrompt.close()
        if (hadRequest && cancelRequest
                && request.state !== WebEngineWebAuthUxRequest.Completed
                && request.state !== WebEngineWebAuthUxRequest.Cancelled) {
            window.resolveQtRequest(ui, request, "webauth", "cancel", [])
        }
        if (hadRequest) {
            window.restoreOverlayFocus()
        }
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
        window.pendingWebAuthUserNames = names
        window.pendingWebAuthState = request.state
        window.pendingWebAuthRelyingParty = window.boundedPageDialogText(
                    request.relyingPartyId || "opaque or unavailable relying party")
        window.webAuthStatusText = window.webAuthStateLabel(request.state)
        request.stateChanged.connect(function(state) {
            window.updateWebAuthState(request, state)
        })
        window.captureOverlayFocus(hostWindow || window, view)
        webAuthPrompt.open()
        ui.status_text = "WebAuthn security-key prompt"
    }

    function selectWebAuthAccount(account) {
        var request = window.pendingWebAuthRequest
        if (!request || window.pendingWebAuthState !== WebEngineWebAuthUxRequest.SelectAccount) {
            return
        }
        request.setSelectedAccount(window.boundedPageDialogText(account))
    }

    function submitWebAuthPin() {
        var request = window.pendingWebAuthRequest
        if (!request || window.pendingWebAuthState !== WebEngineWebAuthUxRequest.CollectPin) {
            return
        }
        var pin = window.boundedPageDialogText(webAuthPrompt.pin)
        webAuthPrompt.pin = ""
        request.setPin(pin)
        pin = ""
    }

    function retryWebAuth() {
        var request = window.pendingWebAuthRequest
        if (request && window.pendingWebAuthState === WebEngineWebAuthUxRequest.RequestFailed) {
            request.retry()
        }
    }

    function cancelWebAuth() {
        var ui = window.clearWebAuthRequest(true) || browserUi
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

    function clearContextMenuRequest() {
        var hadRequest = !!window.pendingContextMenuRequest
        window.pendingContextMenuRequest = null
        window.pendingContextMenuView = null
        window.pendingContextMenuUi = null
        window.pendingContextMenuHost = null
        window.contextMenuItems = []
        contextMenu.close()
        if (hadRequest) {
            window.restoreOverlayFocus()
        }
    }

    function clearContextMenuForView(view) {
        if (window.pendingContextMenuView === view) {
            window.clearContextMenuRequest()
        }
    }

    function contextMenuItem(label, action, value, actionId) {
        return { label: label, action: action, value: value || "",
                 actionId: actionId || "" }
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
                    items.push(window.contextMenuItem(String(labels[i]), "userscript-action",
                                                       value, String(ids[i])))
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
                                              "browser.selection.copy"))
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
            items.push(window.contextMenuItem("Cut", "web-cut"))
        }
        if (editFlags & 16) {
            items.push(window.contextMenuItem("Paste", "web-paste"))
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
        items.push(window.contextMenuItem("Inspect element", "inspect"))
        window.pendingContextMenuRequest = request
        window.pendingContextMenuView = view
        window.pendingContextMenuUi = ui
        window.pendingContextMenuHost = hostWindow || window
        window.contextMenuItems = items
        request.accepted = true
        window.captureOverlayFocus(hostWindow || window, view)
        contextMenu.open()
        ui.status_text = "Context menu"
    }

    function activateContextMenuItem(item) {
        var view = window.pendingContextMenuView
        var ui = window.pendingContextMenuUi || browserUi
        var host = window.pendingContextMenuHost
        if (!item || !view) {
            window.clearContextMenuRequest()
            return
        }
        var action = item.action
        var value = item.value
        window.clearContextMenuRequest()
        if (action === "hint-userscript-action") {
            window.activateHint(value, item.actionId || "")
            return
        }
        if (item.actionId && item.actionId.length > 0) {
            if (!ui.execute_ui_action(item.actionId, value)) {
                return
            }
            if (ui === browserUi) {
                window.executePendingEngineAction()
            } else if (action === "open-link" && view) {
                var pending = ui.take_engine_action()
                if (pending.indexOf("navigate\t") === 0) {
                    var parts = pending.split("\t")
                    if (parts.length >= 3) {
                        view.url = parts.slice(2).join("\t")
                    }
                }
            } else if (action === "download-link" && view) {
                if (ui.take_download_request()) {
                    window.runBrowserScript(view, window.downloadLinkScript(ui.download_request_url),
                                            function() {
                                                ui.complete_download_request(ui.download_request_token, true)
                                            })
                }
            } else if (action === "copy-link" && view) {
                var clipboardRequest = ui.take_clipboard_request()
                if (clipboardRequest.length > 0) {
                    window.copyToClipboard(
                                clipboardRequest,
                                !ui.clipboard_request_sensitive,
                                ui.clipboard_request_primary)
                }
            } else if ((action === "web-copy" || action === "selection-search"
                        || action === "external-selection-send") && view) {
                var selectionToken = ui.take_selection_request()
                if (selectionToken.length > 0) {
                    window.runBrowserScript(view, window.selectionScript(), function(value) {
                        ui.deliver_selection(selectionToken, JSON.stringify(value))
                    })
                }
            }
            return
        }
        if (action === "open-link") {
            if (host && host.popupPermissionUi !== undefined) {
                view.url = value
            } else {
                ui.navigate(value)
                if (ui === browserUi) {
                    window.updateActiveTabUrl()
                } else if (ui.initial_url !== undefined) {
                    view.url = ui.initial_url
                } else {
                    view.url = value
                }
            }
            ui.status_text = "Link opened"
        } else if (action === "copy-link") {
            window.copyToClipboard(value, false)
            ui.status_text = "Link copied"
        } else if (action === "download-link") {
            window.runBrowserScript(view, window.downloadLinkScript(value))
            ui.status_text = "Download requested"
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
                ui.status_text = "Edit action applied"
            }
        }
    }

    function acceptPageDialog() {
        var request = window.pendingAuthenticationRequest || window.pendingPageDialogRequest
        var ui = window.pendingPageDialogUi || browserUi
        var view = window.pendingPageDialogView
        var authentication = !!window.pendingAuthenticationRequest
        if (!request) {
            return
        }
        if (window.pendingAuthenticationRequest) {
            var username = window.boundedPageDialogText(pageDialogPopup.usernameText)
            var password = window.boundedPageDialogText(pageDialogPopup.passwordText)
            pageDialogPopup.usernameText = ""
            pageDialogPopup.passwordText = ""
            window.resolveQtRequest(
                ui, request, authentication ? "authentication" : "page-dialog",
                "dialogAccept", [username, password])
            username = ""
            password = ""
        } else if (window.pageDialogType(request) === "prompt") {
            window.resolveQtRequest(
                ui, request, "page-dialog", "dialogAccept",
                [window.boundedPageDialogText(pageDialogPopup.inputText)])
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
        var ui = window.pendingPageDialogUi || browserUi
        var view = window.pendingPageDialogView
        var kind = window.pageDialogKind()
        if (!request) {
            return
        }
        window.resolveQtRequest(
            ui, request, kind === "authentication" ? "authentication" : "page-dialog",
            "dialogReject", [])
        pageDialogPopup.usernameText = ""
        pageDialogPopup.passwordText = ""
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

    function permissionQueueKey(origin, permissionName, privateProfile) {
        return origin + "\t" + permissionName + "\t" + (privateProfile ? "private" : "normal")
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

    function showNextPermissionPrompt() {
        var queue = window.permissionPromptQueue
        var wasVisible = window.permissionPromptVisible
        if (!queue || queue.length === 0) {
            window.permissionPromptVisible = false
            window.pendingPermissionRequest = null
            window.pendingPermissionUi = null
            window.pendingPermissionTabIndex = -1
            window.pendingPermissionOrigin = ""
            window.pendingPermissionName = ""
            window.pendingPermissionPrivate = false
            window.pendingPermissionGroupCount = 0
            window.pendingPermissionHost = null
            permissionPromptExpiry.stop()
            if (wasVisible) {
                window.restoreOverlayFocus()
            }
            return
        }
        var group = queue[0]
        window.pendingPermissionRequest = group.requests[0] || null
        window.pendingPermissionUi = group.ui
        window.pendingPermissionTabIndex = group.tabIndex
        window.pendingPermissionOrigin = group.origin
        window.pendingPermissionName = group.name
        window.pendingPermissionPrivate = group.privateProfile
        window.pendingPermissionGroupCount = group.requests.length
        window.pendingPermissionHost = group.host || window
        if (!wasVisible) {
            window.captureOverlayFocus(window.pendingPermissionHost)
        }
        window.permissionPromptVisible = true
        permissionPromptExpiry.restart()
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

    function queuePermissionPrompt(ui, request, tabIndex, privateProfile, hostWindow) {
        var origin = request.origin.toString()
        var name = window.permissionTypeName(request.permissionType)
        var key = window.permissionQueueKey(origin, name, privateProfile)
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
            if (queue[i].key === key) {
                groupIndex = i
                break
            }
        }
        if (groupIndex < 0 && window.permissionQueueRequestCount(queue)
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
                host: hostWindow || window
            })
        }
        window.permissionPromptQueue = queue
        if (!window.permissionPromptVisible) {
            window.showNextPermissionPrompt()
        } else if (groupIndex === 0) {
            window.pendingPermissionGroupCount = queue[0].requests.length
        }
    }

    function handlePermissionRequested(request, tabIndex) {
        if (!request || !request.isValid) {
            return
        }
        var permissionName = window.permissionTypeName(request.permissionType)
        if (permissionName.length === 0) {
            window.resolveQtRequest(browserUi, request, "permission", "deny", [])
            browserUi.status_text = "Blocked unsupported permission request"
            return
        }
        var origin = request.origin.toString()
        var decision = permissionName === "camera-and-microphone"
                ? "ask"
                : browserUi.permission_decision(origin, permissionName)
        if (permissionName === "camera-and-microphone") {
            var cameraDecision = browserUi.permission_decision(origin, "camera")
            var microphoneDecision = browserUi.permission_decision(origin, "microphone")
            if (cameraDecision === "deny" || microphoneDecision === "deny") {
                decision = "deny"
            } else if (cameraDecision === "allow" && microphoneDecision === "allow") {
                decision = "allow"
            } else {
                decision = "ask"
            }
        }
        if (decision === "allow") {
            window.resolveQtRequest(browserUi, request, "permission", "grant", [])
            browserUi.status_text = "Allowed " + window.permissionDisplayName(permissionName)
            return
        }
        if (decision === "deny") {
            window.resolveQtRequest(browserUi, request, "permission", "deny", [])
            browserUi.status_text = "Denied " + window.permissionDisplayName(permissionName)
            return
        }
        window.queuePermissionPrompt(
            browserUi, request, tabIndex, window.temporaryProfile, window)
    }

    function decidePermission(allow, lifetime) {
        var queue = window.permissionPromptQueue || []
        var group = queue.length > 0 ? queue[0] : null
        var ui = group && group.ui ? group.ui : browserUi
        if (!group || !group.requests || group.requests.length === 0) {
            window.clearPermissionPrompt(false)
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

    function handleImmediatePermissionRequested(ui, request, privateProfile, hostWindow) {
        if (!request || !request.isValid) {
            return
        }
        var permissionName = window.permissionTypeName(request.permissionType)
        if (permissionName.length === 0) {
            window.resolveQtRequest(ui, request, "permission", "deny", [])
            ui.status_text = "Blocked unsupported permission request"
            return
        }
        var requestOrigin = request.origin.toString()
        var decision = permissionName === "camera-and-microphone"
                ? "ask"
                : ui.permission_decision(requestOrigin, permissionName)
        if (permissionName === "camera-and-microphone") {
            var cameraDecision = ui.permission_decision(requestOrigin, "camera")
            var microphoneDecision = ui.permission_decision(requestOrigin, "microphone")
            if (cameraDecision === "deny" || microphoneDecision === "deny") {
                decision = "deny"
            } else if (cameraDecision === "allow" && microphoneDecision === "allow") {
                decision = "allow"
            }
        }
        if (decision === "allow") {
            window.resolveQtRequest(ui, request, "permission", "grant", [])
            ui.status_text = "Allowed " + window.permissionDisplayName(permissionName)
        } else {
            if (decision === "ask") {
                window.queuePermissionPrompt(ui, request, -1, privateProfile, hostWindow)
            } else {
                window.resolveQtRequest(ui, request, "permission", "deny", [])
                ui.status_text = "Denied " + window.permissionDisplayName(permissionName)
            }
        }
    }

    function cancelPermissionForTab(tabIndex) {
        window.removePermissionGroups(function(group) {
            return group.tabIndex === tabIndex
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
        var request = window.pendingDesktopMediaRequest
        var hadRequest = !!request
        var ui = window.pendingDesktopMediaUi || browserUi
        window.pendingDesktopMediaRequest = null
        window.pendingDesktopMediaUi = null
        window.pendingDesktopMediaView = null
        window.pendingDesktopMediaHost = null
        window.pendingDesktopMediaWaitingForPortal = false
        window.pendingDesktopMediaPortalDeadlineMs = 0
        window.desktopMediaPromptVisible = false
        window.desktopMediaOrigin = ""
        if (hadRequest) {
            window.restoreOverlayFocus()
        }
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
            var timedOutUi = window.pendingDesktopMediaUi || browserUi
            window.clearDesktopMediaRequest(true)
            if (timedOutUi) {
                timedOutUi.status_text =
                        "Desktop portal check timed out; screen sharing cancelled"
            }
            return
        }
        var ui = window.pendingDesktopMediaUi || browserUi
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
        window.pendingDesktopMediaHost = hostWindow || window
        window.captureOverlayFocus(window.pendingDesktopMediaHost, view)
        window.desktopMediaOrigin = view && view.url && view.url.host
                ? window.boundedPageDialogText(view.url.host)
                : "opaque or unavailable origin"
        var requestUi = window.pendingDesktopMediaUi || browserUi
        if (window.desktopPortalMode(requestUi) === "required") {
            var portalStatus = window.desktopPortalCapabilityStatus(requestUi, "screen_cast")
            if (portalStatus === "not-probed") {
                browserUi.probe_desktop_portals()
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
        var ui = window.pendingDesktopMediaUi || browserUi
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
        var ui = window.pendingDesktopMediaUi || browserUi
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

    function reloadViewsForPermission(origin) {
        var expected = String(origin || "").toLowerCase()
        var candidates = []
        function addCandidate(view, ui) {
            if (!view || window.permissionOriginForView(view) !== expected) {
                return
            }
            for (var i = 0; i < candidates.length; ++i) {
                if (candidates[i].view === view) {
                    return
                }
            }
            candidates.push({ view: view, ui: ui || browserUi })
        }

        addCandidate(window.activeWebView(), browserUi)
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (entry && entry.host !== window) {
                addCandidate(entry.view, entry.ui)
            }
        }
        var sessions = window.captureSessions || []
        for (var j = 0; j < sessions.length; ++j) {
            var session = sessions[j]
            if (session && session.status === "active") {
                addCandidate(session.view, session.ui)
            }
        }

        window.removePermissionGroups(function(group) {
            return group && String(group.origin || "").toLowerCase() === expected
        }, true)
        for (var k = 0; k < candidates.length; ++k) {
            window.noteCaptureNavigation(candidates[k].view)
            candidates[k].view.reload()
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
                || tabIndex === browserUi.active_tab_index
        if (visible) {
            window.captureOverlayFocus(window.rendererFailureHost || window, view)
        }
        window.rendererFailureVisible = visible
        ui.status_text = visible
                ? "Renderer " + view.rendererFailureReason
                : "Renderer failed in background tab"
    }

    function showRendererFailureForTab(tabIndex) {
        var view = tabViewAt(tabIndex)
        if (!view || !view.rendererFailed) {
            return
        }
        window.rendererFailureView = view
        window.rendererFailureHost = window
        window.rendererFailureUi = browserUi
        window.rendererFailureTabIndex = tabIndex
        window.rendererFailureSafeUrl = view.rendererFailureSafeUrl
        window.rendererFailureReason = view.rendererFailureReason
        window.rendererFailureExitCode = view.rendererFailureExitCode
        window.rendererFailureCount = view.rendererFailureCount
        window.captureOverlayFocus(window, view)
        window.rendererFailureVisible = true
    }

    function clearRendererFailureForView(view) {
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
                window.restoreOverlayFocus()
            }
        }
    }

    function reloadRendererFailure() {
        var view = window.rendererFailureView
        var ui = window.rendererFailureUi || browserUi
        if (!view) {
            return
        }
        if (ui && ui.prepare_renderer_recovery
                && !ui.prepare_renderer_recovery(window.rendererFailureTabIndex)) {
            ui.status_text = "Renderer recovery target is stale"
            return
        }
        window.rendererFailureVisible = false
        view.rendererFailed = false
        view.reload()
        window.restoreOverlayFocus()
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
        window.restoreOverlayFocus()
    }

    function showRendererFailureDiagnostics() {
        var ui = window.rendererFailureUi || browserUi
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
        var ui = window.rendererFailureUi || browserUi
        if (window.softwareRendering) {
            ui.status_text = "Software rendering is already enabled"
            return
        }
        if (!browserUi.restart_software_rendering(
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
        window.beginQuitRequest()
    }

    Timer {
        id: permissionPromptExpiry
        interval: 30000
        repeat: false
        onTriggered: {
            var ui = window.pendingPermissionUi || browserUi
            window.clearPermissionPrompt(true)
            ui.status_text = "Permission request expired"
        }
    }

    FerricPageDialog {
        id: pageDialogPopup
        browserWindow: window
    }

    function attachDownloadStateUpdates(download, id) {
        download.stateChanged.connect(function() {
            browserUi.update_download(id, window.downloadStateName(download), download.receivedBytes, false)
            if (window.downloadManagerVisible) {
                window.refreshDownloads()
            }
        })
        download.isPausedChanged.connect(function() {
            browserUi.update_download(id, window.downloadStateName(download), download.receivedBytes, false)
            if (window.downloadManagerVisible) {
                window.refreshDownloads()
            }
        })
    }

    function downloadStagingDirectory(ui, id) {
        return ui && ui.download_staging_directory
                ? String(ui.download_staging_directory(id)) : ""
    }

    function acceptDownloadRequest(download, id, directory, suggestedName) {
        var finalName = browserUi.accept_download(id, directory, suggestedName)
        if (finalName.length === 0) {
            window.resolveQtRequest(browserUi, download, "download", "cancel", [])
            delete window.activeDownloads[id]
            return false
        }
        var stagingDirectory = window.downloadStagingDirectory(browserUi, id)
        if (stagingDirectory.length === 0) {
            window.resolveQtRequest(browserUi, download, "download", "cancel", [])
            browserUi.update_download(id, "cancelled", download.receivedBytes, true)
            return false
        }
        download.downloadDirectory = stagingDirectory
        download.downloadFileName = finalName
        window.attachDownloadStateUpdates(download, id)
        window.resolveQtRequest(browserUi, download, "download", "accept", [])
        browserUi.update_download(id, "in-progress", 0, false)
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
        var selectedPath = fileDialogSurfaces.downloadSelectedFile.toLocalFile()
        var finalName = browserUi.accept_download_path(id, selectedPath)
        if (finalName.length === 0) {
            window.resolveQtRequest(browserUi, download, "download", "cancel", [])
            browserUi.update_download(id, "cancelled", download.receivedBytes, true)
        } else {
            var separator = selectedPath.lastIndexOf("/")
            var directory = separator > 0 ? selectedPath.slice(0, separator) : "/"
            var stagingDirectory = window.downloadStagingDirectory(browserUi, id)
            if (stagingDirectory.length === 0) {
                window.resolveQtRequest(browserUi, download, "download", "cancel", [])
                browserUi.update_download(id, "cancelled", download.receivedBytes, true)
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
            window.resolveQtRequest(browserUi, download, "download", "accept", [])
            browserUi.update_download(id, "in-progress", 0, false)
        }
        delete window.pendingDownloadRequests[id]
        window.pendingDownloadId = ""
        window.pendingDownloadSuggestedName = ""
        if (window.downloadManagerVisible) {
            window.refreshDownloads()
        }
        window.restoreOverlayFocus()
    }

    function cancelPendingDownload() {
        var id = window.pendingDownloadId
        var download = window.pendingDownloadRequests[id]
        if (download) {
            window.resolveQtRequest(browserUi, download, "download", "cancel", [])
            browserUi.update_download(id, "cancelled", download.receivedBytes, true)
        }
        delete window.pendingDownloadRequests[id]
        window.pendingDownloadId = ""
        window.pendingDownloadSuggestedName = ""
        if (window.downloadManagerVisible) {
            window.refreshDownloads()
        }
        window.restoreOverlayFocus()
    }

    function handleDownloadRequested(download) {
        var id = String(download.id)
        var safeName = browserUi.offer_download(id, download.url.toString(), download.suggestedFileName)
        var requestedPath = download.savePageFormat === WebEngineDownloadRequest.MimeHtmlSaveFormat
                ? browserUi.take_save_page_path() : ""
        if (requestedPath.length > 0) {
            var requestedName = browserUi.accept_download_path(id, requestedPath)
            if (requestedName.length === 0) {
                window.resolveQtRequest(browserUi, download, "download", "cancel", [])
                browserUi.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            var requestedSeparator = requestedPath.lastIndexOf("/")
            var requestedDirectory = requestedSeparator > 0
                    ? requestedPath.slice(0, requestedSeparator) : "/"
            window.activeDownloads[id] = download
            var stagingDirectory = window.downloadStagingDirectory(browserUi, id)
            if (stagingDirectory.length === 0) {
                window.resolveQtRequest(browserUi, download, "download", "cancel", [])
                browserUi.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            download.downloadDirectory = stagingDirectory
            download.downloadFileName = requestedName
            window.attachDownloadStateUpdates(download, id)
            window.resolveQtRequest(browserUi, download, "download", "accept", [])
            browserUi.update_download(id, "in-progress", 0, false)
            return
        }
        var directory = browserUi.default_download_directory()
        if (!window.downloadsAskDestination()) {
            window.activeDownloads[id] = download
            window.acceptDownloadRequest(download, id, directory, safeName)
            return
        }
        if (window.pendingDownloadId.length > 0) {
            window.resolveQtRequest(browserUi, download, "download", "cancel", [])
            browserUi.update_download(id, "cancelled", download.receivedBytes, true)
            return
        }
        window.pendingDownloadRequests[id] = download
        window.pendingDownloadId = id
        window.pendingDownloadSuggestedName = safeName
        window.captureOverlayFocus(window, window.activeWebView())
        browserUi.update_download(id, "selecting-destination", 0, false)
        fileDialogSurfaces.downloadCurrentFile = window.fileUrlForPath(directory + "/" + safeName)
        fileDialogSurfaces.openDownload()
    }

    function handleDownloadFinished(download) {
        var id = String(download.id)
        var state = window.downloadStateName(download)
        if (state === "completed" && !browserUi.finalize_download(id)) {
            state = "interrupted"
        } else if (state !== "completed") {
            browserUi.discard_download_staging(id)
        }
        browserUi.update_download(id, state, download.receivedBytes, true)
        delete window.activeDownloads[id]
        delete window.pendingDownloadRequests[id]
        if (window.pendingDownloadId === id) {
            window.pendingDownloadId = ""
            window.pendingDownloadSuggestedName = ""
        }
        if (window.downloadManagerVisible) {
            window.refreshDownloads()
        }
    }

}
