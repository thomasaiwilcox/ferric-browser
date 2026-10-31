import QtQuick

// Runs before FerricTabViewController materializes Core descriptors. Only
// pages with no existing engine view are prepared as independent page windows.
QtObject {
    id: coordinator
    required property var rootWindow
    property bool splitting: false
    property bool stopping: false
    property int moveDispatchDepth: 0
    property var retiringChildren: []
    property var pendingExpansion: null
    property var expansionPrompt: null
    property var pendingPages: ({})
    property var pendingMoves: ({})
    property Timer moveDeadline: Timer {
        interval: 1000
        repeat: true
        running: Object.keys(coordinator.pendingMoves).length > 0
        onTriggered: {
            for (var nonce in coordinator.pendingMoves) {
                var pending = coordinator.pendingMoves[nonce]
                if (!pending.child || !pending.ui || Date.now() >= pending.expires) coordinator.failLivePage(nonce)
            }
        }
    }
    readonly property int automaticWindowLimit: 12

    property Component promptFactory: Component {
        FerricCommandDialog {
            required property int pageCount
            browserWindow: coordinator.rootWindow
            visible: true
            commandText: "Restore page windows"
            title: "Open " + pageCount + " page windows?"
            message: "Each page opens in its own window. Cancel keeps the current page; saved sessions are unchanged."
            actions: [
                { id: "cancel", key: "n", label: "Keep current page", safe: true },
                { id: "confirm", key: "y", label: "Open page windows" }
            ]
            onActionRequested: function(action) { coordinator.resolveExpansion(action === "confirm") }
        }
    }

    function liveChild(child) {
        return !!child && typeof child.destroy === "function"
    }
    function retainRetiringChild(child) {
        if (liveChild(child) && retiringChildren.indexOf(child)<0)
            retiringChildren=retiringChildren.concat([child])
    }
    function shutdownOwners() {
        var result=[]
        function add(child) {
            if (liveChild(child) && !child.runtimeReleased && result.indexOf(child)<0) result.push(child)
        }
        for (var id in pendingPages) {
            var page=pendingPages[id]
            if (page && page.committed) add(page.child)
        }
        for (var nonce in pendingMoves) {
            var move=pendingMoves[nonce]
            if (move && liveChild(move.child) && move.child.livePageCoreCommitted()) add(move.child)
        }
        return result
    }
    function stopForApplicationShutdown() {
        stopping=true
        pendingExpansion=null
        if (expansionPrompt) { expansionPrompt.close(); expansionPrompt.destroy(); expansionPrompt=null }
        var pages=Object.keys(pendingPages)
        var publicationPending=false
        for (var i=0;i<pages.length;++i) {
            var page=pendingPages[pages[i]]
            if (page && page.committed && liveChild(page.child) && !page.child.runtimeReleased
                    && (!page.child.windowPreparedPageCorePublished || !page.child.profileBootstrapComplete
                        || !page.child.activeView)) publicationPending=true
            if (page && !page.committed && !page.failed && liveChild(page.child)) {
                retainRetiringChild(page.child)
                failPreparedPage(page.child)
            }
        }
        var moves=Object.keys(pendingMoves)
        for (var j=0;j<moves.length;++j) failLivePage(moves[j])
        retiringChildren=retiringChildren.filter(liveChild)
        return !splitting && moveDispatchDepth===0 && retiringChildren.length===0 && !publicationPending
    }
    function resumeAfterAbortedShutdown() {
        stopping=false
        retiringChildren=retiringChildren.filter(liveChild)
        var owners=shutdownOwners()
        for (var i=0;i<owners.length;++i)
            if (owners[i].windowPreparedPageMode) owners[i].finishPreparedPageNavigation()
    }
    function completeLivePage(child) {
        var nonce=child ? child.windowLivePageNonce : ""
        var pending=pendingMoves[nonce]
        if (!pending || pending.child!==child) return
        var remaining=Object.assign({},pendingMoves)
        delete remaining[nonce]
        pendingMoves=remaining
        recheckLastWindow()
    }
    function releaseLivePage(child) {
        Qt.callLater(function() { coordinator.completeLivePage(child) })
    }

    function hasPendingWindows() {
        for (var nonce in pendingMoves) {
            var moving = pendingMoves[nonce]
            if (moving && moving.child && !moving.child.runtimeReleased) return true
        }
        for (var id in pendingPages) {
            var prepared = pendingPages[id]
            if (prepared && prepared.committed) return true
            if (prepared && !prepared.failed && prepared.child
                    && typeof prepared.child.destroy === "function" && !prepared.child.runtimeReleased) return true
        }
        return false
    }
    function recheckLastWindow() {
        Qt.callLater(function() {
            if (coordinator.rootWindow && coordinator.rootWindow.primaryPageRetired
                    && !coordinator.rootWindow.runtimeDisposing && !coordinator.rootWindow.applicationShutdownInProgress
                    && !coordinator.rootWindow.browserUi.page_closes_pending()) {
                var entries=coordinator.rootWindow.liveWindowEntries()
                if (entries.length>0 && !entries.some(function(entry) { return entry.host && entry.host.visible }))
                    coordinator.rootWindow.presentIdlePageSurvivor(entries[0])
            }
            if (coordinator.rootWindow && !coordinator.rootWindow.runtimeDisposing
                    && coordinator.rootWindow.primaryPageRetired
                    && !coordinator.rootWindow.applicationShutdownInProgress
                    && coordinator.rootWindow.liveWindowEntries().length === 0
                    && !coordinator.hasPendingWindows()
                    && !coordinator.rootWindow.browserUi.page_closes_pending())
                coordinator.rootWindow.beginApplicationShutdown()
        })
    }

    function failLivePage(nonce) {
        var pending = pendingMoves[nonce]
        if (!pending) return
        var child = pending.child
        // A definite Core commit is never an allocation rollback. Keep this
        // owner visible to shutdown until actual registration or native release.
        if (liveChild(child) && child.livePageCoreCommitted()) return
        if (stopping) retainRetiringChild(child)
        var remaining = Object.assign({},pendingMoves)
        delete remaining[nonce]
        pendingMoves = remaining
        if (pending.ui) pending.ui.cancel_live_page_move(nonce)
        if (child && typeof child.rejectPopupAdmission === "function") child.rejectPopupAdmission()
        recheckLastWindow()
    }
    function resumeLivePage(child) {
        var nonce=child ? child.windowLivePageNonce : ""
        var pending=pendingMoves[nonce]
        if (!pending || pending.child!==child || !child.windowLivePagePublished) return
        if (stopping && !child.livePageCoreCommitted()) { failLivePage(nonce); return }
        if (child.livePageCoreCommitted()) return
        var committed=false
        moveDispatchDepth+=1
        try {
            if (!stopping && Date.now()<pending.expires) committed=child.commitLivePage()
        } finally {
            moveDispatchDepth-=1
        }
        if (!committed) { failLivePage(nonce); return }
        // registerWindowOwner promotes/removes this exact pending record. A
        // committed registration retry remains an owned shutdown participant.
        var entry=rootWindow.browserWindowEntryForUi(child.windowUi)
        if (entry && entry.host===child) completeLivePage(child)
        recheckLastWindow()
    }
    function executeLivePageMove(ui, host, action) {
        var fields = action.split("\t")
        if (stopping || fields.length !== 3 || !ui || !host || host.runtimeReleased) {
            if (ui && fields.length===3) ui.cancel_live_page_move(fields[1])
            return false
        }
        var nonce = fields[1]
        var source = rootWindow.browserWindowEntryForUi(ui)
        if (!source || source.host !== host || !rootWindow.browserWindowFactory) {
            ui.cancel_live_page_move(nonce)
            return false
        }
        var controller = host.tabViewControllerObject
        if (!controller || !host.activeView || host.activeView.stableTabId !== fields[2]) {
            ui.cancel_live_page_move(nonce)
            return false
        }
        var child = rootWindow.browserWindowFactory.createObject(null, {
            windowProfileName:source.profileName,
            windowProfileLabel:host === rootWindow ? rootWindow.profileLabel : host.windowProfileLabel,
            windowStorageBasePath:host === rootWindow ? rootWindow.storageBasePath : host.windowStorageBasePath,
            windowPrivateProfile:source.privateProfile, windowEphemeralProfile:source.ephemeralProfile,
            windowEphemeralInvocationToken:source.ephemeralInvocationToken,
            windowSharedProfile:controller.profile, windowSharedRequestInterceptor:controller.interceptor,
            windowInitialVisible:false, windowLivePageMode:true, windowLivePageNonce:nonce,
            windowLivePageSourceUi:ui, windowLivePageSourceHost:host
        })
        if (!child || child.runtimeReleased) { ui.cancel_live_page_move(nonce); return false }
        var updated = Object.assign({},pendingMoves)
        updated[nonce] = { child:child, ui:ui, expires:Date.now()+15000 }
        pendingMoves = updated
        // Readiness can be synchronous; the child also schedules an owned
        // zero-delay callback after asynchronous policy bootstrap.
        if (child.windowLivePageAccepted && child.windowLivePagePublished) Qt.callLater(function() { coordinator.resumeLivePage(child) })
        return true
    }

    function destroyPreparedChild(child) {
        // A destroyed QObject can leave a truthy wrapper in a pending record.
        // Only live QML-created objects still provide the destruction method.
        if (child && typeof child.destroy === "function") {
            if (stopping) retainRetiringChild(child)
            child.destroy()
        }
    }

    function idsFor(ui) {
        var ids = []
        for (var i = 0; i < ui.tab_count; ++i) ids.push(ui.tab_id_for_index(i))
        return ids
    }

    function hasPendingFor(ui) {
        for (var id in pendingPages) {
            var pending = pendingPages[id]
            if (pending && pending.controller && pending.controller.browserUi === ui && !pending.failed) return true
        }
        return false
    }

    function targetFor(ui, id) {
        var index = ui.tab_index_for_id(id)
        return index >= 0 ? ui.navigation_target_for_index(index) : null
    }

    function sameTarget(left, right) {
        return !!left && !!right && left.tabId === right.tabId
                && left.generation === right.generation && left.documentId === right.documentId
    }

    function retireCommittedPreparedPage(id, pending) {
        // Let the exclusive child finish releasing its Ui/reporter first.
        Qt.callLater(function() {
            if (pendingPages[id] === pending) delete pendingPages[id]
            recheckLastWindow()
        })
    }

    function releasePreparedPage(child) {
        for (var id in pendingPages) {
            var pending = pendingPages[id]
            if (pending && pending.committed && pending.child === child) {
                retireCommittedPreparedPage(id,pending)
                return
            }
        }
    }

    function failCommittedPreparedPage(id) {
        var pending = pendingPages[id]
        if (!pending || !pending.committed) return
        var child = pending.child
        // A gate pauses admission; it cannot turn a definite ownership commit
        // into allocation failure while another owner can still cancel Quit.
        if (stopping && liveChild(child) && !child.runtimeReleased) return
        var original = child && typeof child.destroy === "function" && !child.runtimeReleased
                && child.windowPreparedPageMode && child.windowPreparedPageId === id
                && child.windowUi && !child.windowUi.profile_bootstrap_pending
                && child.windowUi.page_host_profile_id() === pending.profile
                && child.windowUi.core_window_id === pending.destinationWindow
        if (original && child.windowPreparedPageCorePublished && child.activeView
                && child.activeView.stableTabId === id
                && child.activeView.profile === child.windowWebEngineProfile
                && child.activeView.ownsTab()) {
            child.windowUi.status_text = "The page was kept; window registration is still pending"
            if (!rootWindow.liveWindowEntries().some(function(entry) {
                    return entry.host && entry.host !== child && entry.host.visible
                })) child.show()
            return
        }
        if (original && child.windowUi.tab_count === 1 && child.windowUi.tab_id_for_index(0) === id
                && typeof child.rejectPopupAdmission === "function") child.rejectPopupAdmission()
        retireCommittedPreparedPage(id,pending)
    }

    function completePreparedPage(child) {
        for (var id in pendingPages) {
            var pending = pendingPages[id]
            if (pending && pending.child === child && pending.committed) {
                delete pendingPages[id]
                recheckLastWindow()
                return
            }
        }
    }

    function resumePreparedPage(child) {
        for (var id in pendingPages) {
            var pending = pendingPages[id]
            if (!pending || pending.child !== child) continue
            if (pending.committed) return
            var controller = pending.controller
            if (!controller || !controller.ready || idsFor(controller.browserUi).indexOf(id) < 0) {
                delete pendingPages[id]
                destroyPreparedChild(child)
                recheckLastWindow()
                return
            }
            controller.syncTabModel()
            rootWindow.completeBrowserKeyAction(controller.browserUi, controller.browserHost)
            return
        }
    }

    function failPreparedPage(child) {
        for (var id in pendingPages) {
            var pending = pendingPages[id]
            if (!pending || pending.child !== child) continue
            pending.child = null
            pending.failed = true
            destroyPreparedChild(child)
            recheckLastWindow()
            if (pending.controller && pending.controller.ready) {
                pending.controller.syncTabModel()
                rootWindow.completeBrowserKeyAction(pending.controller.browserUi, pending.controller.browserHost)
            }
            return
        }
    }

    function resolveExpansion(accepted) {
        var pending = pendingExpansion
        var prompt = expansionPrompt
        expansionPrompt = null
        if (prompt) prompt.destroy()
        if (!pending || !pending.controller || !pending.controller.browserUi) {
            pendingExpansion = null
            return
        }
        var controller = pending.controller
        var ui = controller.browserUi
        if (idsFor(ui).join("\n") !== pending.signature) {
            pendingExpansion = null
            controller.syncTabModel()
            return
        }
        if (accepted) {
            pendingExpansion = { controller: controller, signature: pending.signature, approved: true }
        } else {
            // No extra view/window was allocated. Drop only the unpresented
            // restore descriptors, preserving the existing live page.
            var ids = idsFor(ui)
            for (var i = 0; i < ids.length; ++i) {
                if (ids[i] !== pending.keeper) ui.complete_tab_transfer(ids[i])
            }
            pendingExpansion = null
            ui.status_text = "Page-window restoration cancelled"
        }
        controller.syncTabModel()
    }

    function prepare(controller) {
        var ui = controller.browserUi
        if (ui.feature_tabs_presentation_effective !== "windows" || ui.tab_count <= 1) return true
        if (stopping || splitting) return false
        var ids = idsFor(ui)
        var live = controller.tabViews.filter(function(view) {
            return view && !view.viewRetired && ids.indexOf(view.stableTabId) >= 0
        })
        if (live.length > 1) {
            // A prior native allocation failure keeps its page usable in the
            // existing window. Never reopen or discard those live documents.
            return true
        }
        var selected = ui.tab_id_for_index(ui.active_tab_index)
        var keeper = live.length === 1 ? live[0].stableTabId : selected
        if (ids.indexOf(keeper) < 0) keeper = ids[0]
        var candidates = ids.filter(function(id) { return id !== keeper })
        // Core selection signals can synchronously re-enter QML before the
        // enclosing command publishes its captured navigation effect. Wait
        // for that intent; an unfinished navigation is never a blank page.
        for (var intentIndex = 0; intentIndex < candidates.length; ++intentIndex) {
            var descriptor = {}
            try { descriptor = JSON.parse(ui.tab_transfer_payload(candidates[intentIndex])) } catch (error) { return false }
            if (descriptor.loading === 1 && !descriptor.url) return false
        }
        var signature = ids.join("\n")
        if (ids.length > automaticWindowLimit
                && !(pendingExpansion && pendingExpansion.controller === controller
                     && pendingExpansion.signature === signature && pendingExpansion.approved)) {
            if (!expansionPrompt) {
                pendingExpansion = { controller: controller, signature: signature, keeper: keeper, approved: false }
                expansionPrompt = promptFactory.createObject(controller.browserHost.contentItem,
                    { hostWindow: controller.browserHost, pageCount: candidates.length })
                if (!expansionPrompt) {
                    pendingExpansion = null
                    ui.status_text = "The page-window confirmation couldn't be displayed"
                }
            }
            return false
        }
        pendingExpansion = null
        var source = rootWindow.browserWindowEntryForUi(ui)
        if (!source || !source.host || source.host.runtimeReleased) return false
        var factory = rootWindow.browserWindowFactory
        if (!factory) {
            ui.status_text = "The page window couldn't open. The page is kept in this window."
            return true
        }
        splitting = true
        for (var index = 0; index < candidates.length; ++index) {
            var id = candidates[index]
            var prepared = pendingPages[id]
            var target = targetFor(ui, id)
            if (prepared && (!sameTarget(prepared.target, target)
                    || prepared.context !== String(ui.context_name)
                    || prepared.profile !== ui.page_host_profile_id())) {
                destroyPreparedChild(prepared.child)
                delete pendingPages[id]
                prepared = null
            }
            if (prepared && prepared.failed) {
                delete pendingPages[id]
                splitting = false
                ui.status_text = "The page window couldn't open. The page is kept in this window."
                return true
            }
            var scrollX = -1
            var scrollY = -1
            for (var row = 0; row < controller.tabs.count; ++row) {
                var model = controller.tabs.get(row)
                if (model.tabId === id) {
                    scrollX = Number(model.scrollX === undefined ? -1 : model.scrollX)
                    scrollY = Number(model.scrollY === undefined ? -1 : model.scrollY)
                    break
                }
            }
            var child = prepared ? prepared.child : factory.createObject(null, {
                windowProfileName: source.profileName,
                windowProfileLabel: source.host === rootWindow ? rootWindow.profileLabel : source.host.windowProfileLabel,
                windowStorageBasePath: source.host === rootWindow ? rootWindow.storageBasePath : source.host.windowStorageBasePath,
                windowPrivateProfile: source.privateProfile,
                windowEphemeralProfile: source.ephemeralProfile,
                windowEphemeralInvocationToken: source.ephemeralInvocationToken,
                windowSharedProfile: controller.profile,
                windowSharedRequestInterceptor: controller.interceptor,
                windowInitialVisible: false,
                pageGroupId: source.host.pageGroupId,
                windowPreparedPageMode: true,
                windowPreparedPageSourceUi: ui,
                windowPreparedPageId: id,
                windowPreparedPageFocusSource: source.host,
                windowPreparedScrollX: scrollX,
                windowPreparedScrollY: scrollY
            })
            if (child && child.windowPreparedPageAdmitted && !child.windowPreparedPageAccepted) {
                if (!prepared) pendingPages[id] = { child: child, controller: controller,
                    target: target, context: String(ui.context_name), profile: ui.page_host_profile_id(),
                    foreground: id === selected && source.host.active, failed: false }
                splitting = false
                return false
            }
            if (stopping || !child || !child.windowPreparedPageAccepted
                    || !sameTarget(targetFor(ui, id), target)
                    || child.windowWebEngineProfile !== controller.profile
                    || child.windowUi.page_host_profile_id() !== ui.page_host_profile_id()
                    || !child.commitPreparedPage()) {
                delete pendingPages[id]
                destroyPreparedChild(child)
                ui.status_text = "The page window couldn't open. The page is kept in this window."
                break
            }
            pendingPages[id] = { child:child, controller:controller, target:target,
                context:String(ui.context_name), profile:ui.page_host_profile_id(),
                foreground:(prepared ? prepared.foreground : id === selected),
                destinationWindow:child.windowUi.core_window_id, committed:true, failed:false }
            child.windowPreparedPageActivateRequested = (prepared ? prepared.foreground : id === selected)
                    && id === selected && source.host.active
            child.windowPreparedPageCommitted = true
            child.registerWindowOwner()
            // Generic Wayland has no portable no-focus mapping guarantee.
            // Background pages stay unmapped until explicitly selected from
            // the switcher; they retain a real view and normal profile owner.
        }
        splitting = false
        // On allocation failure, normal materialization executes the retained
        // request here. Existing live documents and their state stay intact.
        return true
    }
}
