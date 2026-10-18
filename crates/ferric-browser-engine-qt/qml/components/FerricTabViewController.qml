import QtQuick

// Materialize reducer-owned tab descriptors as persistent engine views.
// No tab policy lives here: IDs, ordering and restore decisions come from BrowserUi.
Item {
    id: controller
    required property var browserUi
    required property var browserHost
    required property var tabs
    required property var viewFactory
    required property var viewParent
    required property var profile
    required property var interceptor
    required property bool transientProfile
    property bool ready: true
    property var tabViews: []
    property string tabIdentitySignature: ""
    signal activity
    signal restored

    onReadyChanged: {
        if (ready) syncTabModel()
    }

    function refreshMetadata() {
        for (var i = 0; i < tabs.count; ++i) {
            if (tabs.get(i).tabId !== browserUi.tab_id_for_index(i)) continue
            var pinned = browserUi.tab_pinned_at(i)
            var muted = browserUi.tab_muted_at(i)
            if (tabs.get(i).pinned !== pinned) tabs.setProperty(i, "pinned", pinned)
            if (tabs.get(i).muted !== muted) tabs.setProperty(i, "muted", muted)
        }
    }

    function applyRestorePayload(payload, hasModeLine) {
        var lines = payload.split("\n")
        var append = hasModeLine && lines.shift() === "append"
        var previousCount = tabs.count
        if (!append) {
            tabs.clear()
            previousCount = 0
        }
        for (var i = 0; i < lines.length; ++i) {
            if (lines[i].length === 0) {
                continue
            }
            var fields = lines[i].split("\t")
            var tabIndex = previousCount + i
            tabs.append({
                url: fields[0],
                title: fields.length > 4 && fields[4].length > 0 && !hasModeLine
                       ? fields[4]
                       : (i === 0 ? "Recovered tab" : "Recovered tab " + (i + 1)),
                loaded: tabIndex === browserUi.active_tab_index,
                pinned: fields.length > 1 && fields[1] === "true",
                muted: fields.length > 2 && fields[2] === "true",
                zoom: fields.length > 3 ? Number(fields[3]) : 1.0,
                scrollX: hasModeLine && fields.length > 4 ? Number(fields[4]) : -1,
                scrollY: hasModeLine && fields.length > 5 ? Number(fields[5]) : -1,
                suspended: false,
                discarded: false
            })
        }
    }

    function syncTabModel() {
        // Profile bootstrap replaces the provisional reducer identities. Do
        // not create renderers or consume restore data until their owner is ready.
        if (!ready || !viewFactory || !viewParent) return false
        var pending = browserUi.take_session_restore_values()
        if (pending.length > 0) {
            controller.applyRestorePayload(pending, true)
        }
        var coreIds = []
        for (var coreIndex = 0; coreIndex < browserUi.tab_count; ++coreIndex) {
            coreIds.push(browserUi.tab_id_for_index(coreIndex))
        }
        // Retire removed renderers before deleting their ListModel rows.
        // Qt may deliver engine notifications while those rows are invalidated.
        for (var retiring = 0; retiring < tabViews.length; ++retiring) {
            var oldView = tabViews[retiring]
            if (oldView.stableTabId.length > 0 && coreIds.indexOf(oldView.stableTabId) < 0) {
                oldView.viewRetired = true
                oldView.viewModel = ({ loaded:false, url:"", muted:false, zoom:1.0 })
            }
        }
        var aligned = pending.length === 0
                && tabs.count === coreIds.length
                && tabViews.length === coreIds.length
        if (aligned) {
            for (var alignedIndex = 0; alignedIndex < coreIds.length; ++alignedIndex) {
                if (tabs.get(alignedIndex).tabId !== coreIds[alignedIndex]
                        || tabViews[alignedIndex].stableTabId !== coreIds[alignedIndex]) {
                    aligned = false
                    break
                }
            }
        }
        var activeBeforeSync = browserUi.active_tab_index
        if (aligned && activeBeforeSync >= 0 && activeBeforeSync < tabs.count
                && !tabs.get(activeBeforeSync).loaded) {
            aligned = false
        }
        if (aligned) {
            refreshMetadata()
            return
        }
        var identitySignature = coreIds.join("\n")
        if (controller.tabIdentitySignature.length > 0
                && controller.tabIdentitySignature !== identitySignature) {
            activity()
        }
        controller.tabIdentitySignature = identitySignature
        for (var modelIndex = 0; modelIndex < coreIds.length; ++modelIndex) {
            var wantedId = coreIds[modelIndex]
            var foundIndex = -1
            for (var searchIndex = modelIndex; searchIndex < tabs.count; ++searchIndex) {
                if (tabs.get(searchIndex).tabId === wantedId) {
                    foundIndex = searchIndex
                    break
                }
            }
            if (foundIndex >= 0) {
                if (foundIndex !== modelIndex) {
                    tabs.move(foundIndex, modelIndex, 1)
                }
            } else if (modelIndex < tabs.count
                       && String(tabs.get(modelIndex).tabId || "").length === 0) {
                tabs.setProperty(modelIndex, "tabId", wantedId)
            } else {
                // Native descriptors must be present before constructing the
                // view: committing a placeholder can overwrite a queued undo
                // or clone destination through the active-tab callbacks.
                var descriptor = {}
                try { descriptor = JSON.parse(browserUi.tab_transfer_payload(wantedId)) } catch (error) {}
                var nativeDescriptor = descriptor.url === "rb://start" || descriptor.url === "rb://blank"
                tabs.insert(modelIndex, {
                    tabId: wantedId,
                    url: nativeDescriptor ? descriptor.url : "about:blank",
                    title: nativeDescriptor ? (descriptor.url === "rb://start" ? "Start page" : "Blank") : "New tab",
                    loaded: true,
                    pinned: false,
                    muted: false,
                    zoom: 1.0,
                    suspended: false,
                    discarded: false
                })
            }
        }
        while (tabs.count > coreIds.length) {
            tabs.remove(tabs.count - 1)
        }
        var active = browserUi.active_tab_index
        if (active >= 0 && active < tabs.count && !tabs.get(active).loaded) {
            tabs.setProperty(active, "loaded", true)
        }
        var remainingViews = tabViews.slice(0)
        var orderedViews = []
        for (var index = 0; index < tabs.count; ++index) {
            var tabId = coreIds[index]
            var view = null
            for (var viewSearch = 0; viewSearch < remainingViews.length; ++viewSearch) {
                if (remainingViews[viewSearch].stableTabId === tabId) {
                    view = remainingViews.splice(viewSearch, 1)[0]
                    break
                }
            }
            if (!view && remainingViews.length > 0
                    && String(remainingViews[0].stableTabId || "").length === 0) {
                view = remainingViews.shift()
            }
            if (!view) {
                view = viewFactory.createObject(viewParent, {
                tabIndex: index,
                stableTabId: tabId,
                viewUi: browserUi,
                viewHost: browserHost,
                viewTabs: tabs,
                viewModel: tabs.get(index),
                viewProfile: profile,
                viewInterceptor: interceptor,
                viewTransientProfile: transientProfile,
                viewTransferred: false
                })
            }
            if (!view) {
                browserUi.status_text = "These tabs couldn't be displayed. Try reopening the window."
                break
            }
            view.stableTabId = tabId
            orderedViews.push(view)
        }
        for (var staleIndex = 0; staleIndex < remainingViews.length; ++staleIndex) {
            remainingViews[staleIndex].viewRetired = true
            remainingViews[staleIndex].destroy()
        }
        tabViews = orderedViews
        for (var viewIndex = 0; viewIndex < tabViews.length; ++viewIndex) {
            tabViews[viewIndex].tabIndex = viewIndex
            tabViews[viewIndex].viewModel = tabs.get(viewIndex)
        }
        if (tabViews.length !== tabs.count || tabs.count !== browserUi.tab_count) {
            browserUi.status_text = "The tabs couldn't be synchronized. Try reopening the window."
            console.error("tab count invariant failed", tabViews.length,
                          tabs.count, browserUi.tab_count)
            return
        }
        for (var verifyIndex = 0; verifyIndex < tabs.count; ++verifyIndex) {
            if (tabs.get(verifyIndex).tabId !== coreIds[verifyIndex]
                    || tabViews[verifyIndex].stableTabId !== coreIds[verifyIndex]) {
                browserUi.status_text = "The tabs couldn't be synchronized. Try reopening the window."
                console.error("tab identity invariant failed", verifyIndex)
                return
            }
        }
        refreshMetadata()
        if (pending.length > 0) restored()
    }

}
