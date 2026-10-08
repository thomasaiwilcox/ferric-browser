import QtQuick
import QtTest
import "../../qml/components"
import "../../qml/scripts/VersionedRestorePresentation.js" as RestorePresentation
import "../../qml/scripts/PopupWindowOwnership.js" as PopupWindowOwnership

TestCase {
    id: testCase
    name: "VersioningWindowOwner"
    when: windowShown
    property int destroyedWindows: 0

    Component { id: ownerComponent; FerricVersioningWindowOwner { } }
    Component {
        id: childComponent
        QtObject {
            property bool visible: true
            property bool shutdownApproved: false
            property int closeRequests: 0
            property int cancelRequests: 0
            property var tabViews: []
            property var activeDownloads: ({})
            property var pendingDownloadRequests: ({})
            property var downloadCompletionRequests: ({})
            property var nativePageCaptureSave: null
            property var nativePageCapturePoisonedTabIds: []
            property var nativePageCaptureRetiredDownloads: ({})
            property var nativePageCaptureRetiredPaths: ({})
            property bool ownedPopupOutstanding: false
            property var browserProfile: null
            property var popupProfile: null
            property var versioningWindowOwner: null
            property var browserWindowRegistry: []
            property var popupWindowRegistry: []
            property var rootWindow: null
            property var secondaryUi: null
            signal versioningDiscoveryChanged()
            function continueQuitRequest() { closeRequests += 1 }
            function abortApplicationShutdown() { cancelRequests += 1 }
            function hasActiveDownloads() {
                var maps = [activeDownloads, pendingDownloadRequests, downloadCompletionRequests]
                for (var i = 0; i < maps.length; ++i) {
                    var requests = maps[i] || ({})
                    for (var key in requests) {
                        if (requests[key])
                            return true
                    }
                }
                return false
            }
            function hasOwnedPopupWindows() {
                if (!rootWindow || !secondaryUi)
                    return ownedPopupOutstanding
                return PopupWindowOwnership.hasOwnedPopupWindows(
                           rootWindow, this, secondaryUi)
            }
            Component.onDestruction: testCase.destroyedWindows += 1
        }
    }

    function init() { destroyedWindows = 0 }

    function closeChildren(owner) {
        var children = owner.windows.slice()
        for (var i = 0; i < children.length; ++i) {
            children[i].shutdownApproved = true
            children[i].visible = false
        }
        wait(0)
    }

    function test_unparented_window_survives_gc_until_approved_close() {
        var owner = createTemporaryObject(ownerComponent, this)
        var ui = { status_text: "" }
        var child = RestorePresentation.openRestoreWindow(childComponent, {}, ui, owner)
        verify(child !== null)
        compare(owner.windows.length, 1)
        child = null
        gc()
        wait(0)
        compare(destroyedWindows, 0)
        compare(owner.windows.length, 1)

        // Temporary hiding is not an approval to release a live window.
        owner.windows[0].visible = false
        compare(owner.windows.length, 1)
        verify(!owner.releaseClosed(owner.windows[0]))
        owner.windows[0].visible = true
        closeChildren(owner)
        compare(owner.windows.length, 0)
        compare(destroyedWindows, 1)
    }

    function test_failed_hidden_child_bootstrap_drops_owner_reference() {
        var owner = createTemporaryObject(ownerComponent, this)
        var child = childComponent.createObject(null)
        child.visible = false
        verify(owner.retain(child))
        compare(owner.windows.length, 1)
        verify(owner.forgetDestroyed(child))
        compare(owner.windows.length, 0)
        verify(!owner.forgetDestroyed(child))
        child.destroy()
        wait(0)
        compare(destroyedWindows, 1)
    }

    function test_capacity_refuses_allocation_without_evicting_windows() {
        var owner = createTemporaryObject(ownerComponent, this)
        for (var i = 0; i < owner.maximumWindows; ++i)
            verify(owner.retain(childComponent.createObject(null)))
        verify(!owner.canCreate())
        var allocations = 0
        var factory = { createObject: function() { allocations += 1; return null } }
        var ui = { status_text: "" }
        compare(RestorePresentation.openRestoreWindow(factory, {}, ui, owner), null)
        compare(allocations, 0)
        compare(owner.windows.length, owner.maximumWindows)
        compare(destroyedWindows, 0)
        closeChildren(owner)
        compare(destroyedWindows, owner.maximumWindows)
    }

    function test_owned_children_participate_in_close_and_cancel() {
        var owner = createTemporaryObject(ownerComponent, this)
        var child = childComponent.createObject(null)
        verify(owner.retain(child))
        verify(!owner.retain(child))
        owner.requestCloseAll()
        compare(child.closeRequests, 1)
        owner.cancelCloseAll()
        compare(child.cancelRequests, 1)
        compare(owner.windows.length, 1)
        closeChildren(owner)
        compare(owner.windows.length, 0)
    }

    function test_retain_rolls_back_partial_signal_wiring() {
        var owner = createTemporaryObject(ownerComponent, this)
        var disconnects = 0
        var child = {
            visibleChanged: {
                connect: function(handler) { verify(typeof handler === "function") },
                disconnect: function(handler) {
                    verify(typeof handler === "function")
                    disconnects += 1
                }
            }
        }
        Object.defineProperty(child, "versioningDiscoveryChanged", { get: function() {
            throw new Error("synthetic discovery signal failure")
        } })

        verify(!owner.retain(child))
        compare(owner.windows.length, 0)
        compare(disconnects, 1)
        compare(owner.totalOwnedWindows(owner.maximumWindows), 0)
    }

    function test_child_release_notifies_a_waiting_root_after_count_reaches_zero() {
        var owner = createTemporaryObject(ownerComponent, this)
        var child = childComponent.createObject(null)
        verify(owner.retain(child))
        var observed = []
        owner.discoveryChanged.connect(function() {
            observed.push(owner.totalOwnedWindows(owner.maximumWindows))
        })
        child.shutdownApproved = true
        child.visible = false
        wait(0)
        compare(owner.windows.length, 0)
        compare(observed[observed.length - 1], 0)
    }

    function test_approved_close_retains_exact_download_owner_until_terminal_callback() {
        var owner = createTemporaryObject(ownerComponent, this)
        var child = childComponent.createObject(null)
        verify(owner.retain(child))
        // Qt can report isFinished before it delivers downloadFinished. The
        // request identity must keep this window alive until the terminal
        // callback removes the retained completion owner.
        var request = { id: 41, isFinished: true }
        Object.defineProperty(request, "view", { get: function() {
            throw new Error("owner release must not inspect the destroyed view getter")
        } })
        child.downloadCompletionRequests = { "41": request }
        child.shutdownApproved = true
        child.visible = false

        verify(!owner.releaseClosed(child))
        compare(owner.windows.length, 1)
        compare(destroyedWindows, 0)
        compare(child.downloadCompletionRequests["41"], request)

        // The actual terminal handler retires this identity before it retries
        // owner release.
        child.downloadCompletionRequests = ({})
        verify(owner.releaseClosed(child))
        wait(0)
        compare(owner.windows.length, 0)
        compare(destroyedWindows, 1)
    }

    function test_approved_close_retains_window_until_its_popup_owner_is_destroyed() {
        var owner = createTemporaryObject(ownerComponent, this)
        var child = childComponent.createObject(null)
        verify(owner.retain(child))
        var childUi = {}
        var popup = { popupOpenerHost: child, popupPermissionUi: null }
        var rootHost = { popupWindowRegistry: [{ host: popup, ui: childUi }] }
        child.rootWindow = rootHost
        child.secondaryUi = childUi
        child.shutdownApproved = true
        child.visible = false

        verify(!owner.releaseClosed(child))
        compare(owner.windows.length, 1)
        compare(destroyedWindows, 0)

        // Popup teardown removes the retained UI owner; only then may the
        // approved restore window be released.
        rootHost.popupWindowRegistry = []
        verify(owner.releaseClosed(child))
        wait(0)
        compare(owner.windows.length, 0)
        compare(destroyedWindows, 1)
    }

    function test_popup_ownership_fails_closed_for_malformed_registry_or_property_access() {
        var secondary = {}
        var ui = {}
        compare(PopupWindowOwnership.hasOwnedPopupWindows(
                    { popupWindowRegistry: [] }, secondary, ui), false)
        compare(PopupWindowOwnership.hasOwnedPopupWindows(
                    { popupWindowRegistry: null }, secondary, ui), true)
        compare(PopupWindowOwnership.hasOwnedPopupWindows(
                    { popupWindowRegistry: [{ host: null, ui: ui }] }, secondary, ui), true)
        var repeatedHost = {}
        compare(PopupWindowOwnership.hasOwnedPopupWindows(
                    { popupWindowRegistry: [
                        { host: repeatedHost }, { host: repeatedHost }
                    ] }, secondary, ui), true)

        var excessive = []
        for (var i = 0; i < 65; ++i)
            excessive.push({ host: {} })
        compare(PopupWindowOwnership.hasOwnedPopupWindows(
                    { popupWindowRegistry: excessive }, secondary, ui), true)

        var uiOwnedPopup = { popupOpenerHost: {}, popupPermissionUi: null }
        compare(PopupWindowOwnership.hasOwnedPopupWindows(
                    { popupWindowRegistry: [{ host: uiOwnedPopup, ui: ui }] },
                    secondary, ui), true)
        var permissionOwnedPopup = { popupOpenerHost: {}, popupPermissionUi: ui }
        compare(PopupWindowOwnership.hasOwnedPopupWindows(
                    { popupWindowRegistry: [{ host: permissionOwnedPopup, ui: {} }] },
                    secondary, ui), true)

        var hostileEntry = { host: {} }
        Object.defineProperty(hostileEntry, "ui", { get: function() {
            throw new Error("malformed popup ownership getter")
        } })
        compare(PopupWindowOwnership.hasOwnedPopupWindows(
                    { popupWindowRegistry: [hostileEntry] }, secondary, ui), true)

        var hostReads = 0
        var statefulEntry = { ui: {} }
        Object.defineProperty(statefulEntry, "host", { get: function() {
            hostReads += 1
            if (hostReads > 1)
                throw new Error("popup host must be read once per inspection")
            return { popupOpenerHost: {}, popupPermissionUi: null }
        } })
        compare(PopupWindowOwnership.hasOwnedPopupWindows(
                    { popupWindowRegistry: [statefulEntry] }, secondary, ui), false)
        compare(hostReads, 1)
    }

    function test_popup_registration_refuses_malformed_or_exhausted_registry() {
        compare(PopupWindowOwnership.canRegisterPopupWindow(
                    { popupWindowRegistry: [] }), true)
        compare(PopupWindowOwnership.canRegisterPopupWindow(
                    { popupWindowRegistry: null }), false)
        compare(PopupWindowOwnership.canRegisterPopupWindow(
                    { popupWindowRegistry: [{ host: null }] }), false)
        var repeatedHost = {}
        compare(PopupWindowOwnership.canRegisterPopupWindow({
                    popupWindowRegistry: [{ host: repeatedHost }, { host: repeatedHost }]
                }), false)

        var belowLimit = []
        for (var i = 0; i < PopupWindowOwnership.MAX_POPUP_WINDOWS - 1; ++i)
            belowLimit.push({ host: {} })
        compare(PopupWindowOwnership.canRegisterPopupWindow(
                    { popupWindowRegistry: belowLimit }), true)
        belowLimit.push({ host: {} })
        compare(belowLimit.length, PopupWindowOwnership.MAX_POPUP_WINDOWS)
        compare(PopupWindowOwnership.canRegisterPopupWindow(
                    { popupWindowRegistry: belowLimit }), false)

        var hostReads = 0
        var statefulEntry = {}
        Object.defineProperty(statefulEntry, "host", { get: function() {
            hostReads += 1
            if (hostReads > 1)
                throw new Error("popup host must be read once per inspection")
            return {}
        } })
        var statefulCanRegister = PopupWindowOwnership.canRegisterPopupWindow({
            popupWindowRegistry: [statefulEntry]
        })
        compare(statefulCanRegister, true)
        compare(hostReads, 1)

        var hostileRoot = {}
        Object.defineProperty(hostileRoot, "popupWindowRegistry", { get: function() {
            throw new Error("malformed popup registry getter")
        } })
        compare(PopupWindowOwnership.canRegisterPopupWindow(hostileRoot), false)

        var shuttingDownRoot = {
            popupWindowRegistry: [],
            applicationShutdownInProgress: true,
            shutdownApproved: false
        }
        compare(PopupWindowOwnership.canRegisterPopupWindow(shuttingDownRoot), false)
        compare(PopupWindowOwnership.registerPopupWindow(
                    shuttingDownRoot, {}, {}, {}), false)
        compare(shuttingDownRoot.popupWindowRegistry.length, 0)

        var approvedRoot = {
            popupWindowRegistry: [],
            applicationShutdownInProgress: false,
            shutdownApproved: true
        }
        compare(PopupWindowOwnership.canRegisterPopupWindow(approvedRoot), false)
        compare(PopupWindowOwnership.registerPopupWindow(
                    approvedRoot, {}, {}, {}), false)
    }

    function test_popup_registration_revalidates_registry_and_catches_write_failure() {
        var popupHost = {}
        var openerUi = {}
        var popupView = {}
        var rootHost = { popupWindowRegistry: [] }
        verify(PopupWindowOwnership.registerPopupWindow(
                   rootHost, popupHost, openerUi, popupView))
        compare(rootHost.popupWindowRegistry.length, 1)
        compare(rootHost.popupWindowRegistry[0].host, popupHost)
        compare(rootHost.popupWindowRegistry[0].ui, openerUi)
        compare(rootHost.popupWindowRegistry[0].view, popupView)

        var hostileEntry = {}
        Object.defineProperty(hostileEntry, "host", { get: function() {
            throw new Error("malformed popup owner getter")
        } })
        var malformedRoot = { popupWindowRegistry: [hostileEntry] }
        compare(PopupWindowOwnership.registerPopupWindow(
                    malformedRoot, {}, {}, {}), false)
        compare(malformedRoot.popupWindowRegistry.length, 1)

        var repeatedHost = {}
        var duplicateRoot = { popupWindowRegistry: [
            { host: repeatedHost }, { host: repeatedHost }
        ] }
        compare(PopupWindowOwnership.registerPopupWindow(
                    duplicateRoot, {}, {}, {}), false)
        compare(duplicateRoot.popupWindowRegistry.length, 2)

        var getterReads = 0
        var statefulEntry = {}
        Object.defineProperty(statefulEntry, "host", { get: function() {
            getterReads += 1
            if (getterReads > 1)
                throw new Error("popup host must be snapshotted once")
            return {}
        } })
        var statefulRoot = { popupWindowRegistry: [statefulEntry] }
        verify(PopupWindowOwnership.registerPopupWindow(
                   statefulRoot, {}, {}, {}))
        compare(getterReads, 1)
        compare(statefulRoot.popupWindowRegistry.length, 2)

        var setterRoot = {}
        Object.defineProperty(setterRoot, "popupWindowRegistry", {
            get: function() { return [] },
            set: function() { throw new Error("popup registry write refused") }
        })
        compare(PopupWindowOwnership.registerPopupWindow(
                    setterRoot, {}, {}, {}), false)
    }

    function test_popup_unregistration_is_exact_bounded_and_fail_closed() {
        var retainedHost = {}
        var closingHost = {}
        var rootHost = { popupWindowRegistry: [
            { host: retainedHost, ui: {}, view: null },
            { host: closingHost, ui: {}, view: null }
        ] }
        verify(PopupWindowOwnership.unregisterPopupWindow(rootHost, closingHost))
        compare(rootHost.popupWindowRegistry.length, 1)
        compare(rootHost.popupWindowRegistry[0].host, retainedHost)

        var hostileEntry = {}
        Object.defineProperty(hostileEntry, "host", { get: function() {
            throw new Error("malformed popup owner getter")
        } })
        var malformedRoot = { popupWindowRegistry: [hostileEntry] }
        compare(PopupWindowOwnership.unregisterPopupWindow(
                    malformedRoot, {}), false)
        compare(malformedRoot.popupWindowRegistry.length, 1)

        var repeatedHost = {}
        var duplicateRoot = { popupWindowRegistry: [
            { host: repeatedHost }, { host: repeatedHost }
        ] }
        compare(PopupWindowOwnership.unregisterPopupWindow(
                    duplicateRoot, repeatedHost), false)
        compare(duplicateRoot.popupWindowRegistry.length, 2)

        var setterRoot = {}
        Object.defineProperty(setterRoot, "popupWindowRegistry", {
            get: function() { return [{ host: {} }] },
            set: function() { throw new Error("popup registry write refused") }
        })
        compare(PopupWindowOwnership.unregisterPopupWindow(setterRoot, {}), false)
    }

    function test_popup_host_snapshot_is_bounded_unique_and_fail_closed() {
        compare(PopupWindowOwnership.snapshotPopupHosts(null), null)
        compare(PopupWindowOwnership.snapshotPopupHosts(
                    { popupWindowRegistry: null }), null)
        compare(PopupWindowOwnership.snapshotPopupHosts(
                    { popupWindowRegistry: [] }).length, 0)

        var firstHost = {}
        var secondHost = {}
        var hosts = PopupWindowOwnership.snapshotPopupHosts({
            popupWindowRegistry: [{ host: firstHost }, { host: secondHost }]
        })
        verify(hosts !== null)
        compare(hosts.length, 2)
        compare(hosts[0], firstHost)
        compare(hosts[1], secondHost)

        compare(PopupWindowOwnership.snapshotPopupHosts({
                    popupWindowRegistry: [{ host: firstHost }, { host: firstHost }]
                }), null)
        compare(PopupWindowOwnership.snapshotPopupHosts({
                    popupWindowRegistry: [{ host: null }]
                }), null)

        var overLimit = []
        for (var i = 0; i <= PopupWindowOwnership.MAX_POPUP_WINDOWS; ++i)
            overLimit.push({ host: {} })
        compare(PopupWindowOwnership.snapshotPopupHosts(
                    { popupWindowRegistry: overLimit }), null)

        var hostReads = 0
        var statefulEntry = {}
        Object.defineProperty(statefulEntry, "host", { get: function() {
            hostReads += 1
            if (hostReads > 1)
                throw new Error("popup host must be snapshotted once")
            return firstHost
        } })
        var statefulHosts = PopupWindowOwnership.snapshotPopupHosts({
            popupWindowRegistry: [statefulEntry]
        })
        verify(statefulHosts !== null)
        compare(hostReads, 1)

        var hostileRoot = {}
        Object.defineProperty(hostileRoot, "popupWindowRegistry", { get: function() {
            throw new Error("popup registry getter failed")
        } })
        compare(PopupWindowOwnership.snapshotPopupHosts(hostileRoot), null)

        var viewReads = 0
        var stableEntry = { host: firstHost }
        Object.defineProperty(stableEntry, "view", { get: function() {
            viewReads += 1
            if (viewReads > 1)
                throw new Error("popup view must be snapshotted once")
            return secondHost
        } })
        var entries = PopupWindowOwnership.snapshotPopupEntries({
            popupWindowRegistry: [stableEntry]
        })
        verify(entries !== null)
        compare(entries[0].host, firstHost)
        compare(entries[0].view, secondHost)
        compare(viewReads, 1)
        compare(PopupWindowOwnership.snapshotPopupEntries(
                    { popupWindowRegistry: null }), null)
        compare(PopupWindowOwnership.snapshotPopupEntries(
                    { popupWindowRegistry: overLimit }), null)
        compare(PopupWindowOwnership.snapshotPopupEntries({
                    popupWindowRegistry: [{ host: firstHost }, { host: firstHost }]
                }), null)
        var registryEntries = PopupWindowOwnership.snapshotPopupEntriesFromRegistry([
            { host: firstHost, view: secondHost }
        ])
        verify(registryEntries !== null)
        compare(registryEntries.length, 1)
        compare(registryEntries[0].host, firstHost)
        compare(registryEntries[0].view, secondHost)
        compare(registryEntries[0].ui, null)
        compare(PopupWindowOwnership.snapshotPopupEntriesFromRegistry([
                    { host: firstHost }, { host: firstHost }
                ]), null)

        var uiReads = 0
        var openerUi = {}
        var statefulUiEntry = { host: firstHost, view: secondHost }
        Object.defineProperty(statefulUiEntry, "ui", { get: function() {
            uiReads += 1
            if (uiReads > 1)
                throw new Error("popup opener UI must be snapshotted once")
            return openerUi
        } })
        var uiEntries = PopupWindowOwnership.snapshotPopupEntriesFromRegistry([
            statefulUiEntry
        ])
        verify(uiEntries !== null)
        compare(uiEntries[0].ui, openerUi)
        compare(uiReads, 1)

        var entryHostReads = 0
        var statefulHostEntry = { view: null }
        Object.defineProperty(statefulHostEntry, "host", { get: function() {
            entryHostReads += 1
            if (entryHostReads > 1)
                throw new Error("popup host must be snapshotted once")
            return firstHost
        } })
        var statefulEntries = PopupWindowOwnership.snapshotPopupEntries({
            popupWindowRegistry: [statefulHostEntry]
        })
        verify(statefulEntries !== null)
        compare(entryHostReads, 1)

        var hostileViewEntry = { host: firstHost }
        Object.defineProperty(hostileViewEntry, "view", { get: function() {
            throw new Error("popup view getter failed")
        } })
        compare(PopupWindowOwnership.snapshotPopupEntries({
                    popupWindowRegistry: [hostileViewEntry]
                }), null)
        var hostileUiEntry = { host: firstHost, view: secondHost }
        Object.defineProperty(hostileUiEntry, "ui", { get: function() {
            throw new Error("popup opener UI getter failed")
        } })
        compare(PopupWindowOwnership.snapshotPopupEntries({
                    popupWindowRegistry: [hostileUiEntry]
                }), null)
    }

    function test_nested_child_cancel_reaches_only_an_active_root_shutdown() {
        var owner = createTemporaryObject(ownerComponent, this)
        var nestedOwner = createTemporaryObject(ownerComponent, this)
        nestedOwner.parentOwner = owner
        var host = {
            applicationShutdownInProgress: true,
            aborts: 0,
            abortApplicationShutdown: function() {
                this.aborts += 1
                this.applicationShutdownInProgress = false
            }
        }
        owner.hostWindow = host
        verify(nestedOwner.cancelRootShutdown())
        compare(host.aborts, 1)
        verify(!nestedOwner.cancelRootShutdown())
        compare(host.aborts, 1)
        owner.hostWindow = null
        verify(!nestedOwner.cancelRootShutdown())
        compare(host.aborts, 1)
    }

    function test_downloads_route_by_exact_view_or_retained_request_not_id_alone() {
        var owner = createTemporaryObject(ownerComponent, this)
        var first = childComponent.createObject(null)
        var second = childComponent.createObject(null)
        verify(owner.retain(first))
        verify(owner.retain(second))
        var firstView = {}
        var secondView = {}
        first.tabViews = [firstView]
        second.tabViews = [secondView]
        var download = { id: 7, view: secondView }
        compare(owner.hostForDownload(download, secondView), second)
        compare(owner.hostForDownload({ id: 8, view: firstView }, firstView), first)
        second.activeDownloads = { "7": download }
        download.view = null
        compare(owner.hostForDownload(download, null), second)
        compare(owner.hostForDownload({ id: 7, view: null }, null), null)
        var late = { id: 9 }
        Object.defineProperty(late, "view", { get: function() {
            throw new Error("destroyed view getter must not run")
        } })
        second.activeDownloads = { "9": late }
        compare(owner.hostForDownload(late, null), second)
        compare(owner.hostForDownload({ id: 9 }, null), null)
        var finishedBeforeCallback = { id: 10, isFinished: true }
        second.downloadCompletionRequests = { "10": finishedBeforeCallback }
        compare(owner.hostForDownload(finishedBeforeCallback, null), second)
        second.downloadCompletionRequests = ({})
        compare(owner.hostForDownload(finishedBeforeCallback, null), null)
        compare(owner.hostForDownload(null), null)
        closeChildren(owner)
    }

    function test_host_for_download_requires_well_formed_owner_indexes() {
        var owner = createTemporaryObject(ownerComponent, this)
        var child = childComponent.createObject(null)
        verify(owner.retain(child))
        var request = { id: 47 }

        child.activeDownloads = null
        verify(owner.hostForDownload(request, null) === undefined)

        child.activeDownloads = Object.create({ "47": request })
        compare(owner.hostForDownload(request, null), null)

        var throwingHost = {
            tabViews: [],
            pendingDownloadRequests: ({}),
            downloadCompletionRequests: ({}),
            nativePageCaptureSave: null,
            browserProfile: {}
        }
        Object.defineProperty(throwingHost, "activeDownloads", {
            get: function() {
                throw new Error("owner request index is unavailable")
            }
        })
        verify(owner.hostForDownload(request, null, throwingHost) === undefined)

        Object.defineProperty(child, "activeDownloads", { configurable: true,
            writable: true, value: ({}) })
        child.tabViews = null
        verify(owner.hostForDownload(request, null) === undefined)
        closeChildren(owner)
    }

    function test_nested_download_routes_to_exact_descendant_request() {
        var owner = createTemporaryObject(ownerComponent, this)
        var nestedOwner = createTemporaryObject(ownerComponent, this)
        nestedOwner.parentOwner = owner
        var parent = childComponent.createObject(null)
        parent.versioningWindowOwner = nestedOwner
        var grandchild = childComponent.createObject(null)
        verify(owner.retain(parent))
        verify(nestedOwner.retain(grandchild))
        var view = {}
        var download = { id: 7, view: view }
        parent.activeDownloads = { "7": { id: 7 } }
        grandchild.tabViews = [view]
        compare(owner.hostForDownload(download, view), grandchild)
        download.view = null
        grandchild.activeDownloads = { "7": download }
        compare(owner.hostForDownload(download, null), grandchild)
        compare(owner.hostForDownload({ id: 7, view: null }, null), null)
        closeChildren(nestedOwner)
        closeChildren(owner)
    }

    function test_ambiguous_download_owner_is_not_first_match() {
        var owner = createTemporaryObject(ownerComponent, this)
        var first = childComponent.createObject(null)
        var second = childComponent.createObject(null)
        var sharedView = { marker: "synthetic-view" }
        first.tabViews = [sharedView]
        second.tabViews = [sharedView]
        verify(owner.retain(first))
        verify(owner.retain(second))
        var download = { id: 17, view: sharedView }
        verify(owner.hostForDownload(download, sharedView) === undefined)

        second.tabViews = []
        second.activeDownloads = { "17": download }
        verify(owner.hostForDownload(download, sharedView) === undefined)
        first.tabViews = []
        compare(owner.hostForDownload(download, null), second)
        closeChildren(owner)
    }

    function test_popup_of_restore_child_routes_exact_view_and_request() {
        var owner = createTemporaryObject(ownerComponent, this)
        var rootHost = {
            tabViews: [],
            activeDownloads: ({}),
            pendingDownloadRequests: ({}),
            downloadCompletionRequests: ({}),
            popupWindowRegistry: [],
            versioningWindowOwner: owner
        }
        owner.hostWindow = rootHost
        var child = childComponent.createObject(null)
        var popup = childComponent.createObject(null)
        var view = { marker: "popup-view" }
        child.popupWindowRegistry = [{ host: popup, view: view }]
        verify(owner.retain(child))
        var rootPopupView = {}
        var rootPopup = childComponent.createObject(null)
        rootHost.popupWindowRegistry = [{ host: rootPopup, view: rootPopupView }]
        var download = { id: 23, view: view }
        compare(owner.hostForDownload(download, view, rootHost), popup)
        popup.pendingDownloadRequests = { "23": download }
        download.view = null
        compare(owner.hostForDownload(download, null, rootHost), popup)
        popup.pendingDownloadRequests = null
        verify(owner.hostForDownload(download, null, rootHost) === undefined)
        popup.pendingDownloadRequests = { "23": download }
        compare(owner.hostForDownload(download, null, rootHost), popup)
        var completed = { id: 24, isFinished: true }
        popup.downloadCompletionRequests = { "24": completed }
        compare(owner.hostForDownload(completed, null, rootHost), popup)
        child.popupWindowRegistry = [{ host: rootPopup, view: rootPopupView }]
        verify(owner.hostForDownload(download, null, rootHost) === undefined)
        var excessive = []
        for (var i = 0; i < 65; ++i)
            excessive.push({ host: popup, view: view })
        child.popupWindowRegistry = excessive
        verify(owner.hostForDownload(download, null, rootHost) === undefined)
        child.popupWindowRegistry = []
        rootHost.popupWindowRegistry = []
        closeChildren(owner)
        popup.destroy()
        rootPopup.destroy()
    }

    function test_page_save_exact_path_routes_without_view_getter_and_conflicts_fail_closed() {
        var owner = createTemporaryObject(ownerComponent, this)
        var profile = {}
        var rootHost = {
            tabViews: [],
            activeDownloads: ({}),
            pendingDownloadRequests: ({}),
            downloadCompletionRequests: ({}),
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredPaths: ({}),
            browserProfile: profile,
            popupWindowRegistry: [],
            versioningWindowOwner: owner
        }
        owner.hostWindow = rootHost
        var first = childComponent.createObject(null)
        first.browserProfile = profile
        first.nativePageCaptureSave = {
            profile: profile,
            path: "/private/staging/one-use/capture.mhtml"
        }
        verify(owner.retain(first))
        var download = {
            id: 29,
            isSavePageDownload: true,
            downloadDirectory: "/private/staging/one-use",
            downloadFileName: "capture.mhtml"
        }
        Object.defineProperty(download, "view", { get: function() {
            throw new Error("authorized path routing must not inspect Qt's view getter")
        } })
        compare(owner.hostForDownload(download, null, rootHost), first)
        rootHost.popupWindowRegistry = null
        verify(owner.hostForDownload(download, null, rootHost) === undefined)
        rootHost.popupWindowRegistry = []
        compare(owner.hostForDownload(download, null, rootHost), first)
        var inaccessibleDownload = {}
        Object.defineProperty(inaccessibleDownload, "id", { get: function() {
            throw new Error("download identity is unavailable")
        } })
        verify(owner.hostForDownload(inaccessibleDownload, null, rootHost) === undefined)
        var oversized = {
            id: 30,
            isSavePageDownload: true,
            downloadDirectory: "/private/" + Array(4090).join("x"),
            downloadFileName: "capture.mhtml"
        }
        Object.defineProperty(oversized, "view", { get: function() {
            throw new Error("bounded path refusal must not inspect Qt's view getter")
        } })
        compare(owner.hostForDownload(oversized, null, rootHost), null)

        var duplicate = childComponent.createObject(null)
        duplicate.browserProfile = profile
        duplicate.nativePageCaptureSave = {
            profile: profile,
            path: first.nativePageCaptureSave.path
        }
        verify(owner.retain(duplicate))
        verify(owner.hostForDownload(download, null, rootHost) === undefined)
        closeChildren(owner)
    }

    function test_profile_save_page_poison_scan_is_bounded_and_profile_scoped() {
        var owner = createTemporaryObject(ownerComponent, this)
        var profile = {}
        var otherProfile = {}
        var rootHost = {
            browserProfile: profile,
            nativePageCapturePoisonedTabIds: [],
            popupWindowRegistry: []
        }
        owner.hostWindow = rootHost
        rootHost.versioningWindowOwner = owner
        compare(owner.profileHasPoisonedPageCaptureTab(profile), false)
        rootHost.popupWindowRegistry = null
        verify(owner.profileHasPoisonedPageCaptureTab(profile) === undefined)
        rootHost.popupWindowRegistry = []

        var hostilePopupEntry = {}
        Object.defineProperty(hostilePopupEntry, "host", { get: function() {
            throw new Error("malformed profile popup owner getter")
        } })
        rootHost.popupWindowRegistry = [hostilePopupEntry]
        verify(owner.profileHasPoisonedPageCaptureTab(profile) === undefined)
        rootHost.popupWindowRegistry = []

        var duplicatePopupHost = {}
        rootHost.popupWindowRegistry = [
            { host: duplicatePopupHost }, { host: duplicatePopupHost }
        ]
        verify(owner.profileHasPoisonedPageCaptureTab(profile) === undefined)
        rootHost.popupWindowRegistry = []

        var popup = childComponent.createObject(null)
        popup.popupProfile = profile
        popup.nativePageCapturePoisonedTabIds = ["popup-interrupted-tab"]
        rootHost.popupWindowRegistry = [{ host: popup }]
        compare(owner.profileHasPoisonedPageCaptureTab(profile), true)
        rootHost.popupWindowRegistry = []
        popup.destroy()

        var child = childComponent.createObject(null)
        child.browserProfile = profile
        child.nativePageCapturePoisonedTabIds = ["interrupted-tab"]
        verify(owner.retain(child))
        compare(owner.profileHasPoisonedPageCaptureTab(profile), true)

        child.browserProfile = otherProfile
        compare(owner.profileHasPoisonedPageCaptureTab(profile), false)
        child.browserProfile = profile
        child.nativePageCapturePoisonedTabIds = ({ malformed: true })
        verify(owner.profileHasPoisonedPageCaptureTab(profile) === undefined)
        child.nativePageCapturePoisonedTabIds = null
        verify(owner.profileHasPoisonedPageCaptureTab(profile) === undefined)
        closeChildren(owner)
    }

    function test_retired_page_save_routes_by_tombstone_without_view_getter() {
        var owner = createTemporaryObject(ownerComponent, this)
        var profile = {}
        var rootHost = {
            tabViews: [],
            activeDownloads: ({}),
            pendingDownloadRequests: ({}),
            downloadCompletionRequests: ({}),
            nativePageCaptureSave: null,
            nativePageCaptureRetiredDownloads: ({}),
            nativePageCaptureRetiredPaths: ({}),
            browserProfile: profile,
            popupWindowRegistry: [],
            versioningWindowOwner: owner
        }
        owner.hostWindow = rootHost

        var first = childComponent.createObject(null)
        first.browserProfile = profile
        first.nativePageCaptureRetiredDownloads = ({ "31": true })
        first.nativePageCaptureRetiredPaths = ({
            "/private/staging/retired/capture.mhtml": true
        })
        verify(owner.retain(first))

        var lateById = {
            id: "31",
            isSavePageDownload: true,
            downloadDirectory: "/user/chosen",
            downloadFileName: "other.mhtml"
        }
        Object.defineProperty(lateById, "view", { get: function() {
            throw new Error("retired ID routing must not inspect Qt's view getter")
        } })
        compare(owner.hostForDownload(lateById, null, rootHost), first)

        var lateByPath = {
            id: "32",
            isSavePageDownload: true,
            downloadDirectory: "/private/staging/retired",
            downloadFileName: "capture.mhtml"
        }
        Object.defineProperty(lateByPath, "view", { get: function() {
            throw new Error("retired path routing must not inspect Qt's view getter")
        } })
        compare(owner.hostForDownload(lateByPath, null, rootHost), first)

        var otherProfileHost = childComponent.createObject(null)
        otherProfileHost.browserProfile = {}
        otherProfileHost.nativePageCaptureRetiredDownloads = ({ "31": true })
        verify(owner.retain(otherProfileHost))
        compare(owner.hostForDownload(lateById, null, rootHost), first)
        closeChildren(owner)
    }

    function test_authorized_page_saves_are_serialized_per_profile() {
        var owner = createTemporaryObject(ownerComponent, this)
        var profile = {}
        var otherProfile = {}
        var rootHost = {
            browserProfile: profile,
            nativePageCaptureSave: null,
            versioningWindowOwner: owner
        }
        owner.hostWindow = rootHost
        var child = childComponent.createObject(null)
        child.browserProfile = profile
        verify(owner.retain(child))
        verify(owner.profilePageCaptureAvailable(profile, rootHost))
        child.nativePageCaptureSave = { profile: profile, path: "private-stage" }
        compare(owner.activePageCaptureHost(profile), child)
        verify(!owner.profilePageCaptureAvailable(profile, rootHost))
        verify(owner.profilePageCaptureAvailable(otherProfile, rootHost))
        verify(owner.profilePageCaptureAvailable(profile, child))
        child.nativePageCaptureSave = null
        compare(owner.activePageCaptureHost(profile), null)
        verify(owner.profilePageCaptureAvailable(profile, rootHost))
        closeChildren(owner)
    }

    function test_discovery_uses_owner_tokens_and_tracks_nested_windows() {
        var owner = createTemporaryObject(ownerComponent, this)
        var child = childComponent.createObject(null)
        var nested = childComponent.createObject(null)
        var nestedOwner = createTemporaryObject(ownerComponent, this)
        nestedOwner.parentOwner = owner
        child.versioningWindowOwner = nestedOwner
        var childEntry = { host: child, ui: {}, coreWindowId: "windowid-2", windowToken: "child-token" }
        var nestedEntry = { host: nested, ui: {}, coreWindowId: "windowid-2", windowToken: "nested-token" }
        child.browserWindowRegistry = [childEntry]
        nested.browserWindowRegistry = [nestedEntry]
        verify(owner.retain(child))
        verify(nestedOwner.retain(nested))
        compare(owner.discoveryEntries(64).length, 2)
        compare(owner.discoveryEntries(1).length, 1)
        compare(owner.discoveryEntries(0).length, 0)
        compare(owner.entryForOwnerToken("child-token"), childEntry)
        compare(owner.entryForOwnerToken("nested-token"), nestedEntry)
        verify(owner.containsHost(child))
        verify(owner.containsHost(nested))
        verify(!owner.containsHost({}))
        compare(owner.entryForOwnerToken("windowid-2"), null)
        compare(owner.entryForOwnerToken("stale-token"), null)
        nested.shutdownApproved = true
        compare(owner.discoveryEntries(64).length, 1)
        closeChildren(nestedOwner)
        closeChildren(owner)
        compare(owner.discoveryEntries(64).length, 0)
    }

    function test_nested_restore_uses_one_process_wide_window_budget() {
        var owner = createTemporaryObject(ownerComponent, this)
        var nestedOwner = createTemporaryObject(ownerComponent, this)
        nestedOwner.parentOwner = owner
        var parent = childComponent.createObject(null)
        parent.versioningWindowOwner = nestedOwner
        verify(owner.retain(parent))
        for (var i = 1; i < owner.maximumWindows; ++i)
            verify(nestedOwner.retain(childComponent.createObject(null)))
        compare(owner.totalOwnedWindows(owner.maximumWindows), owner.maximumWindows)
        verify(!owner.canCreate())
        verify(!nestedOwner.canCreate())

        var released = nestedOwner.windows[0]
        released.shutdownApproved = true
        released.visible = false
        compare(owner.totalOwnedWindows(owner.maximumWindows), owner.maximumWindows - 1)
        verify(owner.canCreate())
        verify(nestedOwner.canCreate())
        closeChildren(nestedOwner)
        closeChildren(owner)
    }

    function test_hidden_parent_is_retained_until_nested_windows_close() {
        var owner = createTemporaryObject(ownerComponent, this)
        var nestedOwner = createTemporaryObject(ownerComponent, this)
        nestedOwner.parentOwner = owner
        var parent = childComponent.createObject(null)
        parent.versioningWindowOwner = nestedOwner
        verify(owner.retain(parent))
        var grandchild = childComponent.createObject(null)
        verify(nestedOwner.retain(grandchild))
        grandchild.browserWindowRegistry = [
            { host: grandchild, ui: {}, coreWindowId: "windowid-3", windowToken: "grandchild-token" }
        ]
        parent.shutdownApproved = true
        parent.visible = false
        compare(owner.windows.length, 1)
        verify(!owner.releaseClosed(parent))
        verify(owner.containsHost(parent))
        verify(owner.containsHost(grandchild))
        compare(owner.entryForOwnerToken("grandchild-token").host, grandchild)
        owner.requestCloseAll()
        compare(grandchild.closeRequests, 1)
        owner.cancelCloseAll()
        compare(grandchild.cancelRequests, 1)
        parent.downloadCompletionRequests = { "55": { id: 55 } }
        grandchild.shutdownApproved = true
        grandchild.visible = false
        wait(0)
        compare(nestedOwner.windows.length, 0)
        compare(owner.windows.length, 1)
        compare(destroyedWindows, 1)
        parent.downloadCompletionRequests = ({})
        parent.versioningDiscoveryChanged()
        wait(0)
        compare(owner.windows.length, 0)
        compare(destroyedWindows, 2)
    }

    function test_misbound_or_cyclic_nested_owner_fails_closed() {
        var owner = createTemporaryObject(ownerComponent, this)
        var other = createTemporaryObject(ownerComponent, this)
        var child = childComponent.createObject(null)
        child.versioningWindowOwner = other
        verify(!owner.retain(child))
        other.parentOwner = owner
        verify(owner.retain(child))
        owner.parentOwner = other
        verify(!owner.canCreate())
        verify(!other.canCreate())
        owner.parentOwner = null
        closeChildren(owner)
    }

    function test_discovery_propagates_bootstrap_and_release_changes() {
        var owner = createTemporaryObject(ownerComponent, this)
        var child = childComponent.createObject(null)
        var notifications = 0
        owner.discoveryChanged.connect(function() { notifications += 1 })
        verify(owner.retain(child))
        compare(notifications, 1)
        child.versioningDiscoveryChanged()
        compare(notifications, 2)
        closeChildren(owner)
        compare(notifications, 3)
    }
}
