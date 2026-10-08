import QtQuick
import QtWebEngine

// Owns WebEngineView QObjects before they are displayed. Moving a view's
// visual parent alone cannot save it when its creation parent is destroyed.
Item {
    id: host
    visible: false

    required property var ownerProfile
    required property var originalHost
    readonly property int maximumRetainedViews: 32
    readonly property int retainedCount: retainedEntries.length
    readonly property int preparedCount: preparedEntries.length
    readonly property int heldCount: heldEntries.length
    readonly property int restoringCount: restoringEntries.length
    property var retainedEntries: []
    property var preparedEntries: []
    property var heldEntries: []
    property var restoringEntries: []
    signal restorationStarted(var view)

    // The pinned engine's v1 methods identify a native view/adapter, but do
    // not assert that RTC, cookies or downloads are safe. Public Qt has no
    // such methods and continues through the existing fail-closed path.
    function nativeBackendWitness(view) {
        if (!view) {
            return false
        }
        var revision = view.ferricVersioningBackendRevision
        var viewId = view.ferricVersioningViewId
        var adapterId = view.ferricVersioningAdapterId
        if (revision === undefined && viewId === undefined
                && adapterId === undefined) {
            return null
        }
        if (typeof revision !== "function" || typeof viewId !== "function"
                || typeof adapterId !== "function"
                || view.ferricVersioningBackendRevision() !== 1) {
            return false
        }
        var currentViewId = view.ferricVersioningViewId()
        var currentAdapterId = view.ferricVersioningAdapterId()
        var uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
        if (typeof currentViewId !== "string" || !uuid.test(currentViewId)
                || typeof currentAdapterId !== "string"
                || !uuid.test(currentAdapterId)
                || view.profile !== ownerProfile) {
            return false
        }
        return { viewId: currentViewId, adapterId: currentAdapterId,
                 profile: ownerProfile }
    }

    function nativeBackendWitnessMatches(view, witness) {
        var current = nativeBackendWitness(view)
        if (witness === null) {
            return current === null
        }
        return current !== null && current !== false
                && witness && witness.profile === ownerProfile
                && current.profile === witness.profile
                && current.viewId === witness.viewId
                && current.adapterId === witness.adapterId
    }

    function createOwnedView(component, presentationParent, properties) {
        if (!component || !presentationParent || !ownerProfile
                || !properties || properties.viewProfile !== ownerProfile) {
            return null
        }
        // The QObject creation parent remains this host even after the
        // visual QQuickItem parent moves into the tab stack.
        var view = component.createObject(host, properties)
        if (!view) {
            return null
        }
        if (view.profile !== ownerProfile) {
            view.destroy()
            return null
        }
        if (view.versioningOwnerHost === undefined) {
            view.destroy()
            return null
        }
        view.versioningOwnerHost = host
        view.parent = presentationParent
        return view
    }

    function retainedIndex(id) {
        for (var i = 0; i < retainedEntries.length; ++i) {
            if (retainedEntries[i].id === id) {
                return i
            }
        }
        return -1
    }

    function retainedView(id) {
        var index = retainedIndex(id)
        return index >= 0 ? retainedEntries[index].view : null
    }

    function retainedViewForTab(tabId) {
        for (var i = 0; i < retainedEntries.length; ++i) {
            if (retainedEntries[i].tabId === tabId) {
                return retainedEntries[i].view
            }
        }
        return null
    }

    function preparedIndex(id) {
        for (var i = 0; i < preparedEntries.length; ++i) {
            if (preparedEntries[i].id === id) {
                return i
            }
        }
        return -1
    }

    function heldIndex(id) {
        for (var i = 0; i < heldEntries.length; ++i) {
            if (heldEntries[i].id === id) {
                return i
            }
        }
        return -1
    }

    function preparedIdForView(view) {
        for (var i = 0; i < preparedEntries.length; ++i) {
            if (preparedEntries[i].view === view
                    && preparedEntries[i].freezeRequested) {
                return preparedEntries[i].id
            }
        }
        return ""
    }

    function restoringIndex(id) {
        for (var i = 0; i < restoringEntries.length; ++i) {
            if (restoringEntries[i].id === id) {
                return i
            }
        }
        return -1
    }

    function restoringIdForView(view) {
        for (var i = 0; i < restoringEntries.length; ++i) {
            if (restoringEntries[i].view === view) {
                return restoringEntries[i].id
            }
        }
        return ""
    }

    function restoringView(id) {
        var index = restoringIndex(id)
        return index >= 0 ? restoringEntries[index].view : null
    }

    function isPreparedView(view) {
        for (var held = 0; held < heldEntries.length; ++held) {
            if (heldEntries[held].view === view
                    || heldEntries[held].fallback === view) {
                return true
            }
        }
        for (var i = 0; i < preparedEntries.length; ++i) {
            if (preparedEntries[i].view === view
                    || preparedEntries[i].fallback === view) {
                return true
            }
        }
        for (var retained = 0; retained < retainedEntries.length; ++retained) {
            if (retainedEntries[retained].stagedStash
                    && !retainedEntries[retained].committed
                    && retainedEntries[retained].fallback === view) {
                return true
            }
        }
        for (var restoring = 0; restoring < restoringEntries.length; ++restoring) {
            if (restoringEntries[restoring].view === view
                    || restoringEntries[restoring].fallback === view) {
                return true
            }
        }
        return false
    }

    function hasPreparedTab(tabId) {
        for (var held = 0; held < heldEntries.length; ++held) {
            if (heldEntries[held].tabId === tabId) {
                return true
            }
        }
        for (var i = 0; i < preparedEntries.length; ++i) {
            if (preparedEntries[i].tabId === tabId) {
                return true
            }
        }
        for (var restoring = 0; restoring < restoringEntries.length; ++restoring) {
            if (restoringEntries[restoring].tabId === tabId) {
                return true
            }
        }
        return false
    }

    // A Start wakeup names an admitted tab, never the tab with current focus.
    // This proves only that its original native view is still visible and
    // owned here; activity and consent must be checked separately.
    function hasAdmittedVisibleSource(tabId, projectedTabIds, visibleViews) {
        if (typeof tabId !== "string" || tabId.length === 0
                || !Array.isArray(projectedTabIds) || !Array.isArray(visibleViews)
                || projectedTabIds.length !== visibleViews.length) {
            return false
        }
        var source = null
        var seen = []
        for (var i = 0; i < projectedTabIds.length; ++i) {
            var projectedId = projectedTabIds[i]
            var view = visibleViews[i]
            if (typeof projectedId !== "string" || projectedId.length === 0
                    || seen.indexOf(projectedId) >= 0 || !view
                    || view.stableTabId !== projectedId
                    || view.profile !== ownerProfile
                    || view.versioningOwnerHost !== host) {
                return false
            }
            seen.push(projectedId)
            if (projectedId === tabId) {
                source = view
            }
        }
        return source !== null && source.parent && source.parent !== host
                && !source.loading
                && source.lifecycleState === WebEngineView.LifecycleState.Active
                && !hasRetainedTab(tabId) && !hasPreparedTab(tabId)
                && !isPreparedView(source)
    }

    // Staging never reports a frozen stash. The caller must have admitted the
    // exact operation and allocated its fallback before invoking this host.
    // The fallback ID comes from that reservation, not the candidate view.
    function reservedPair(tabId, fallbackTabId,
                          projectedTabIds, visibleViews, activeTabId) {
        if (!Array.isArray(projectedTabIds) || !Array.isArray(visibleViews)
                || projectedTabIds.length !== visibleViews.length
                || activeTabId !== fallbackTabId) {
            return null
        }
        var source = null
        var fallback = null
        var seen = []
        for (var i = 0; i < projectedTabIds.length; ++i) {
            var projectedId = projectedTabIds[i]
            var view = visibleViews[i]
            if (typeof projectedId !== "string" || projectedId.length === 0
                    || seen.indexOf(projectedId) >= 0 || !view
                    || view.stableTabId !== projectedId) {
                return null
            }
            seen.push(projectedId)
            if (projectedId === tabId) {
                source = view
            } else if (projectedId === fallbackTabId) {
                fallback = view
            }
        }
        if (!source || !fallback || source === fallback
                || source.profile !== ownerProfile || fallback.profile !== ownerProfile
                || source.versioningOwnerHost !== host
                || fallback.versioningOwnerHost !== host
                || !source.parent || !fallback.parent
                || fallback.loading || !fallback.visible
                || (String(fallback.url) !== "" && String(fallback.url) !== "about:blank")) {
            return null
        }
        return { source: source, fallback: fallback }
    }

    // Read-only native fallback acknowledgement. The original remains visible
    // while the application gathers fresh eligibility evidence for staging.
    function hasMaterializedReservedFallback(tabId, fallbackTabId,
                                             projectedTabIds, visibleViews, activeTabId) {
        return reservedPair(tabId, fallbackTabId,
                            projectedTabIds, visibleViews, activeTabId) !== null
    }

    // A read-only native identity hold can precede the Rust stage ticket.
    // It protects the exact source QObject without hiding or freezing it.
    function holdReservedStash(id, tabId, fallbackTabId,
                               projectedTabIds, visibleViews, activeTabId) {
        var pair = reservedPair(tabId, fallbackTabId,
                                projectedTabIds, visibleViews, activeTabId)
        var backendWitness = pair ? nativeBackendWitness(pair.source) : false
        if (typeof id !== "string" || id.length === 0 || id.length > 128
                || pair === null || backendWitness === false || heldIndex(id) >= 0
                || preparedIndex(id) >= 0 || retainedIndex(id) >= 0
                || restoringIndex(id) >= 0
                || heldEntries.length + preparedEntries.length
                    + retainedEntries.length + restoringEntries.length
                    >= maximumRetainedViews
                || !Number.isInteger(pair.source.versioningNavigationEpoch)
                || pair.source.loading || !pair.source.visible
                || !pair.source.enabled
                || pair.source.lifecycleState !== WebEngineView.LifecycleState.Active
                || hasRetainedTab(tabId) || hasPreparedTab(tabId)
                || isPreparedView(pair.source) || isPreparedView(pair.fallback)) {
            return false
        }
        heldEntries = heldEntries.concat([{
            id: id,
            view: pair.source,
            tabId: tabId,
            fallback: pair.fallback,
            fallbackTabId: fallbackTabId,
            originalVisualParent: pair.source.parent,
            originalVisible: pair.source.visible,
            originalEnabled: pair.source.enabled,
            navigationEpoch: pair.source.versioningNavigationEpoch,
            backendWitness: backendWitness
        }])
        return true
    }

    // A changed projection or fallback refuses mutation but retains the
    // witness, so a terminal Rust stage still has an exact cleanup target.
    function prepareHeldStash(id, tabId, fallbackTabId,
                              projectedTabIds, visibleViews, activeTabId) {
        var index = heldIndex(id)
        if (index < 0) {
            return false
        }
        var entry = heldEntries[index]
        var pair = reservedPair(tabId, fallbackTabId,
                                projectedTabIds, visibleViews, activeTabId)
        if (entry.tabId !== tabId || entry.fallbackTabId !== fallbackTabId
                || pair === null || pair.source !== entry.view
                || pair.fallback !== entry.fallback
                || !nativeBackendWitnessMatches(entry.view, entry.backendWitness)
                || entry.view.versioningNavigationEpoch !== entry.navigationEpoch
                || entry.view.parent !== entry.originalVisualParent
                || entry.view.visible !== entry.originalVisible
                || entry.view.enabled !== entry.originalEnabled) {
            return false
        }
        var remaining = heldEntries.slice(0)
        remaining.splice(index, 1)
        heldEntries = remaining
        if (prepareStash(id, entry.view, ownerProfile,
                         tabId, fallbackTabId, entry.fallback)) {
            return true
        }
        heldEntries = heldEntries.concat([entry])
        return false
    }

    // Only a pre-ticket cancellation may release an untouched native hold.
    // Once Rust has issued a stage ticket, use restoreUncommittedStash.
    function releaseHeldStash(id) {
        var index = heldIndex(id)
        if (index < 0) {
            return false
        }
        var entry = heldEntries[index]
        if (!entry.view || entry.view.profile !== ownerProfile
                || entry.view.versioningOwnerHost !== host
                || entry.view.stableTabId !== entry.tabId
                || entry.view.parent !== entry.originalVisualParent
                || entry.view.visible !== entry.originalVisible
                || entry.view.enabled !== entry.originalEnabled
                || entry.view.lifecycleState !== WebEngineView.LifecycleState.Active) {
            return false
        }
        var remaining = heldEntries.slice(0)
        remaining.splice(index, 1)
        heldEntries = remaining
        return true
    }

    function prepareReservedStash(id, tabId, fallbackTabId,
                                  projectedTabIds, visibleViews, activeTabId) {
        var pair = reservedPair(tabId, fallbackTabId,
                                projectedTabIds, visibleViews, activeTabId)
        return pair !== null && prepareStash(id, pair.source, ownerProfile,
                                             tabId, fallbackTabId, pair.fallback)
    }

    function prepareStash(id, view, profile, tabId, fallbackTabId, fallback) {
        var backendWitness = nativeBackendWitness(view)
        if (typeof id !== "string" || id.length === 0 || id.length > 128
                || typeof tabId !== "string" || tabId.length === 0
                || typeof fallbackTabId !== "string" || fallbackTabId.length === 0
                || fallbackTabId === tabId
                || !view || !fallback || view === fallback
                || backendWitness === false
                || profile !== ownerProfile || view.profile !== ownerProfile
                || fallback.profile !== ownerProfile
                || view.versioningOwnerHost !== host
                || fallback.versioningOwnerHost !== host
                || view.stableTabId !== tabId
                || fallback.stableTabId !== fallbackTabId
                || !Number.isInteger(view.versioningNavigationEpoch)
                || view.loading || !fallback.visible || !fallback.parent
                || !view.parent || view.parent === host
                || retainedEntries.length + preparedEntries.length
                    + heldEntries.length + restoringEntries.length
                    >= maximumRetainedViews
                || retainedIndex(id) >= 0 || preparedIndex(id) >= 0
                || heldIndex(id) >= 0
                || restoringIndex(id) >= 0
                || hasRetainedTab(tabId) || hasPreparedTab(tabId)
                || isPreparedView(view)
                || isPreparedView(fallback)) {
            return false
        }
        preparedEntries = preparedEntries.concat([{
            id: id,
            view: view,
            tabId: tabId,
            profileLease: profile,
            fallback: fallback,
            fallbackTabId: fallback.stableTabId,
            fallbackVisualParent: fallback.parent,
            originalVisualParent: view.parent,
            originalVisible: view.visible,
            originalEnabled: view.enabled,
            navigationEpoch: view.versioningNavigationEpoch,
            backendWitness: backendWitness,
            freezeRequested: false
        }])
        view.visible = false
        view.enabled = false
        view.parent = host
        return true
    }

    function preparedViewIsCurrent(entry) {
        return entry && entry.view && entry.fallback
                && entry.view.profile === ownerProfile
                && entry.fallback.profile === ownerProfile
                && entry.view.versioningOwnerHost === host
                && entry.fallback.versioningOwnerHost === host
                && entry.view.stableTabId === entry.tabId
                && entry.fallback.stableTabId === entry.fallbackTabId
                && nativeBackendWitnessMatches(entry.view, entry.backendWitness)
                && entry.view.versioningNavigationEpoch === entry.navigationEpoch
                && !entry.view.loading && entry.fallback.visible
                && entry.fallback.parent === entry.fallbackVisualParent
                && entry.view.parent === host
    }

    function requestPreparedFreeze(id) {
        var index = preparedIndex(id)
        if (index < 0) {
            return false
        }
        var entry = preparedEntries[index]
        if (!preparedViewIsCurrent(entry) || entry.freezeRequested
                || entry.view.recommendedState !== WebEngineView.LifecycleState.Frozen) {
            return false
        }
        entry.freezeRequested = true
        entry.view.lifecycleState = WebEngineView.LifecycleState.Frozen
        return true
    }

    // The actual lifecycle property, not the requested transition, is the
    // acknowledgement. Duplicate and late callbacks cannot commit a second
    // record or a newly navigated page under the old captured ID.
    function acknowledgeFrozenStash(id, view) {
        var index = preparedIndex(id)
        if (index < 0) {
            return false
        }
        var entry = preparedEntries[index]
        if (view !== entry.view || !entry.freezeRequested
                || !preparedViewIsCurrent(entry)
                || view.lifecycleState !== WebEngineView.LifecycleState.Frozen
                || retainedIndex(id) >= 0 || hasRetainedTab(entry.tabId)) {
            return false
        }
        var remaining = preparedEntries.slice(0)
        remaining.splice(index, 1)
        preparedEntries = remaining
        retainedEntries = retainedEntries.concat([{
            id: entry.id,
            view: entry.view,
            tabId: entry.tabId,
            profileLease: entry.profileLease,
            fallback: entry.fallback,
            fallbackTabId: entry.fallbackTabId,
            originalHost: originalHost,
            originalVisualParent: entry.originalVisualParent,
            originalVisible: entry.originalVisible,
            originalEnabled: entry.originalEnabled,
            stagedStash: true,
            committed: false,
            membership: "Retained"
        }])
        return true
    }

    // The trusted adapter supplies application callbacks, but this host
    // controls when they run. A requested lifecycle never reaches commit;
    // only the exact view's observed Frozen state can cross this boundary.
    // A refused commit begins native restoration. It does not acknowledge
    // restoration until the actual Active lifecycle can be observed later.
    function completeFrozenStash(id, view, commit) {
        if (typeof commit !== "function" || !acknowledgeFrozenStash(id, view)) {
            return false
        }
        var result
        try {
            result = commit()
        } catch (error) {
            // The callback may have committed before failing to return.
            // Preserve native ownership until that outcome is reconciled.
            return false
        }
        if (result === true) {
            var committedIndex = retainedIndex(id)
            if (committedIndex >= 0) {
                retainedEntries[committedIndex].committed = true
                retainedEntries[committedIndex].fallback = null
            }
            return true
        }
        if (result !== false) {
            return false
        }
        restoreRetainedStash(id, view)
        return false
    }

    // A native freeze may succeed while the application commit is refused.
    // Keep the exact QObject alive and give it back to its original presenter;
    // releaseView would destroy unsaved page state instead.
    function restoreRetainedStash(id, view) {
        var index = retainedIndex(id)
        if (index < 0) {
            return false
        }
        var entry = retainedEntries[index]
        if (!entry.stagedStash || entry.committed
                || view !== entry.view || !entry.view
                || !entry.originalVisualParent || entry.view.parent !== host
                || entry.view.versioningOwnerHost !== host
                || entry.view.profile !== ownerProfile
                || entry.view.stableTabId !== entry.tabId) {
            return false
        }
        if (restoringIndex(id) >= 0) {
            return false
        }
        var remaining = retainedEntries.slice(0)
        remaining.splice(index, 1)
        retainedEntries = remaining
        return beginRestoration(entry)
    }

    function cancelPreparedStash(id) {
        var index = preparedIndex(id)
        if (index < 0) {
            return false
        }
        var entry = preparedEntries[index]
        if (!entry.view || !entry.originalVisualParent
                || restoringIndex(id) >= 0) {
            return false
        }
        var remaining = preparedEntries.slice(0)
        remaining.splice(index, 1)
        preparedEntries = remaining
        return beginRestoration(entry)
    }

    // A terminal application stage asks this host to restore its exact view.
    // Repeated requests are idempotent while Active acknowledgement is still
    // pending. A committed stash cannot be restored through this path.
    function restoreUncommittedStash(id) {
        var held = heldIndex(id)
        if (held >= 0) {
            var entry = heldEntries[held]
            if (!entry.view || entry.view.profile !== ownerProfile
                    || entry.view.versioningOwnerHost !== host
                    || entry.view.stableTabId !== entry.tabId
                    || entry.view.parent !== entry.originalVisualParent) {
                return false
            }
            var remaining = heldEntries.slice(0)
            remaining.splice(held, 1)
            heldEntries = remaining
            return beginRestoration(entry)
        }
        var prepared = preparedIndex(id)
        if (prepared >= 0) {
            return cancelPreparedStash(id)
        }
        var retained = retainedIndex(id)
        if (retained >= 0) {
            return restoreRetainedStash(id, retainedEntries[retained].view)
        }
        return restoringIndex(id) >= 0
    }

    function beginRestoration(entry) {
        restoringEntries = restoringEntries.concat([entry])
        entry.view.lifecycleState = WebEngineView.LifecycleState.Active
        entry.view.parent = entry.originalVisualParent
        entry.view.visible = entry.originalVisible
        entry.view.enabled = entry.originalEnabled
        // An already-Active view may emit no lifecycle change. Notify after
        // the visual presenter is restored so the adapter can check Active
        // and retry exact logical cleanup without a synthetic acknowledgement.
        restorationStarted(entry.view)
        return true
    }

    // `Active` is an observation, not the request in beginRestoration. The
    // exact source and fallback remain protected if logical cleanup fails.
    function acknowledgeRestoredStash(id, view, cleanup) {
        var index = restoringIndex(id)
        if (index < 0 || typeof cleanup !== "function") {
            return false
        }
        var entry = restoringEntries[index]
        if (entry.view !== view || !view
                || view.lifecycleState !== WebEngineView.LifecycleState.Active
                || view.profile !== ownerProfile
                || view.versioningOwnerHost !== host
                || view.stableTabId !== entry.tabId
                || view.parent !== entry.originalVisualParent
                || view.visible !== entry.originalVisible
                || view.enabled !== entry.originalEnabled) {
            return false
        }
        try {
            if (cleanup() !== true) {
                return false
            }
        } catch (error) {
            return false
        }
        var remaining = restoringEntries.slice(0)
        remaining.splice(index, 1)
        restoringEntries = remaining
        return true
    }

    function hasRetainedTab(tabId) {
        for (var i = 0; i < retainedEntries.length; ++i) {
            if (retainedEntries[i].tabId === tabId) {
                return true
            }
        }
        return false
    }

    function membershipForView(view) {
        for (var i = 0; i < retainedEntries.length; ++i) {
            if (retainedEntries[i].view === view) {
                return retainedEntries[i].membership
            }
        }
        return view && view.versioningOwnerHost === host ? "Visible" : ""
    }

    function ownsRetainedView(view) {
        for (var i = 0; i < retainedEntries.length; ++i) {
            if (retainedEntries[i].view === view) {
                return true
            }
        }
        return false
    }

    function visibleViewsOnly(views) {
        var visible = []
        for (var i = 0; i < views.length; ++i) {
            if (!ownsRetainedView(views[i])) {
                visible.push(views[i])
            }
        }
        return visible
    }

    function retainView(id, view, profile, tabId) {
        if (typeof id !== "string" || id.length === 0 || id.length > 128
                || typeof tabId !== "string" || tabId.length === 0
                || !view || profile !== ownerProfile || view.profile !== ownerProfile
                || view.stableTabId !== tabId
                || view.versioningOwnerHost !== host
                || retainedEntries.length + preparedEntries.length
                    + heldEntries.length + restoringEntries.length
                    >= maximumRetainedViews
                || retainedIndex(id) >= 0 || hasRetainedTab(tabId)
                || preparedIndex(id) >= 0 || heldIndex(id) >= 0
                || restoringIndex(id) >= 0
                || hasPreparedTab(tabId) || isPreparedView(view)) {
            return false
        }
        var visualParent = view.parent
        if (!visualParent || visualParent === host) {
            return false
        }
        view.visible = false
        view.enabled = false
        view.parent = host
        retainedEntries = retainedEntries.concat([{
            id: id,
            view: view,
            tabId: tabId,
            profileLease: profile,
            originalHost: originalHost,
            originalVisualParent: visualParent,
            membership: "Retained"
        }])
        return true
    }

    function releaseView(id) {
        var index = retainedIndex(id)
        if (index < 0) {
            return false
        }
        var entry = retainedEntries[index]
        var remaining = retainedEntries.slice(0)
        remaining.splice(index, 1)
        retainedEntries = remaining
        if (entry.view) {
            entry.view.visible = false
            entry.view.parent = host
            entry.view.versioningOwnerHost = null
            entry.view.destroy()
        }
        return true
    }
}
