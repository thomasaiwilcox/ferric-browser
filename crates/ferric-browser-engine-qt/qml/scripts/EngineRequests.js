.pragma library

// Both window shells execute the same captured request against an exact view.
function execute(request, ui, host, services, source) {
    if (!request || !request.kind) return false
    host.syncTabModel()
    if (!ui.request_target_current(request.tabId, request.generation, request.documentId)) return true
    var index = ui.tab_index_for_id(request.tabId)
    var view = index >= 0 ? host.tabViewAt(index) : null
    if (!view) return true
    switch (request.kind) {
    case "navigate":
        if (host.tabsModel) host.tabsModel.setProperty(index, "loaded", true)
        view.setContentUrl(request.url, false, false, true)
        break
    case "reload":
        if (view.nativeUrl && view.nativeUrl.length > 0) {
            if (view.nativeUrl === "rb://start" && view.startPage) view.startPage.refresh()
        } else view.requestContentReload(request)
        break
    case "stop": view.stop(); break
    case "history":
        services.executeHistoryTraversal(view, request.offset < 0, Math.abs(request.offset))
        break
    case "find":
        if (host.searchWorkspace) host.searchWorkspace.execute(request, view)
        else view.findText(request.query, services.searchFindFlags(request.query, request.caseMode, request.backward))
        break
    case "clear-find":
        if (host.searchWorkspace) host.searchWorkspace.clear(view)
        else view.findText("")
        break
    case "scroll": case "scroll-page": case "scroll-to":
        services.runBrowserScript(view, services.scrollScript(request.kind, request.direction,
            request.half === true, request.count || 1))
        if (host === services && host.activeWebView() === view) services.scheduleScrollCapture()
        break
    case "scroll-target": services.applyScrollTargetAction(ui, view, request.action, host); break
    case "zoom":
        if (host.tabsModel) host.tabsModel.setProperty(index, "zoom", request.factor)
        view.zoomFactor = request.factor
        break
    case "print-pdf": services.executeCapturedPdf(ui, view, request.path); break
    case "save-page": services.executeCapturedSave(ui, view, request.path); break
    case "view-source": view.url = "view-source:" + request.url; ui.status_text = "Viewing page source"; break
    case "devtools": host.devToolsWorkspace.toggle(request.detach, view); break
    case "print": services.executeCapturedPrint(ui, view); break
    case "evaluate": services.executeCapturedScript(ui, view, request.script, request.world); break
    case "site-doctor-reload":
        if (services.siteDoctorUserscriptsDisabled(ui)) services.clearInstalledPageUserscripts(view)
        view.reload()
        ui.status_text = "Site Doctor experiment running"
        break
    case "site-doctor-fresh-view":
        view.setContentUrl(request.url, false)
        ui.status_text = "Site Doctor fresh view running"
        break
    case "edit":
        services.applyNativeEditAction(ui, view, request, source)
        break
    }
    return true
}
