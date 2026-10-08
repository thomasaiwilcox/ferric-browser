// Reject before opening a Qt popup if a clean branch's exact owner/profile,
// first view, permission UI or request interceptor has gone stale.
function sourceReady(rootWindow, host, ui, profile, interceptor, view) {
    try {
        if (!rootWindow || !host || !host.windowCleanBranchPublished
                || host.windowShutdownApproved)
            return false
        var owner = rootWindow.cleanBranchOwner(host.windowBoundBranchToken)
        var firstView = owner && owner.view === host.windowBranchInitialView
        var groupView = false
        if (firstView && view !== owner.view
                && Array.isArray(host.windowBranchGroupViews)) {
            groupView = host.windowBranchGroupViews.indexOf(view) >= 0
                    && (view.viewHost === host
                        || view.versioningOwnerHost === host.versioningViewHostObject)
                    && view.viewUi === ui
                    && view.viewInterceptor === interceptor
        }
        return !!owner && owner.host === host
                && firstView && owner.profile === profile
                && (owner.view === view || groupView)
                && ui === host.windowBranchUi
                && interceptor === host.windowRequestInterceptor
                && profile.offTheRecord === true
                && view.profile === profile
    } catch (error) {
        return false
    }
}

function popupReady(rootWindow, popup, view) {
    try {
        if (!rootWindow || !popup || !view
                || !popup.popupBranchToken || !popup.popupOpenerHost)
            return false
        var owner = rootWindow.cleanBranchOwner(popup.popupBranchToken)
        return !!owner && owner.host === popup.popupOpenerHost
                && owner.host.windowCleanBranchPublished
                && owner.profile === popup.popupProfile
                && owner.profile.offTheRecord === true
                && owner.view === owner.host.windowBranchInitialView
                && view.profile === owner.profile
                && popup.popupPermissionUi === owner.host.windowBranchUi
                && popup.popupRequestInterceptor
                   === owner.host.windowRequestInterceptor
                && popup.popupPrivateProfile === true
                && popup.popupEphemeralProfile === true
    } catch (error) {
        return false
    }
}
