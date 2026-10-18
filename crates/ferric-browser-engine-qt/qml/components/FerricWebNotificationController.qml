import QtQuick

// Native permission authority stays in the application. This controller retains
// authorized Qt notifications while desktop-service checks finish and owns
// their window-scoped presentation lifecycle.
QtObject {
    id: controller
    required property var rootWindow
    required property var notificationPresenter
    property var activeWebNotifications: ({})
    property var notificationOwners: []
    property var pendingPortalNotifications: []
    property int webNotificationSequence: 0

    property Timer workTimer: Timer {
        interval: 50
        repeat: true
        running: controller.pendingPortalNotifications.length > 0 || controller.notificationOwners.length > 0
        onTriggered: {
            controller.processPendingPortalNotifications()
            controller.pruneActiveOwners()
        }
    }
    property Connections presenterConnection: Connections {
        target: controller.notificationPresenter
        function onNotificationPresented(notification, profileScope, origin) {
            controller.presentationAccepted(notification, profileScope, origin)
        }
        function onNotificationUnavailable(notification, profileScope, origin) {
            controller.presentationUnavailable(notification, profileScope, origin)
        }
        function onNotificationFinished(notification) {
            if (controller.activeWebNotification(notification)) controller.closeNotification(notification)
            else controller.forgetActiveNotification(notification)
        }
        function onNotificationClicked(notification, profileScope, origin) {
            var owner = controller.notificationRecordFor(notification)
            if (!controller.deferredNotificationOwnerValid(owner)
                    || !rootWindow.desktopNotificationsEnabled(owner.ui)
                    || String(owner.ui.permission_decision(origin, "notifications")) === "deny") {
                controller.closeNotification(notification)
                return
            }
            notification.click()
            rootWindow.focusNotificationOrigin(profileScope, origin)
        }
    }
    function closeNotification(notification) {
        if (!notification) return
        controller.removePendingPortalNotification(notification)
        controller.forgetActiveNotification(notification)
        notificationPresenter.cancel(notification)
        notification.close()
    }
    function notificationRecordFor(notification) {
        return controller.notificationOwners.find(function(entry) { return entry.notification===notification }) || null
    }
    function captureOwner(notification, ui) {
        ui=ui || rootWindow.primaryBrowserUi
        var owner=rootWindow.liveWindowEntries().find(function(entry) { return entry && entry.ui===ui })
        return {notification:notification,ui:ui,host:owner ? owner.host : null,
            profileRevision:String(ui.start_page_profile_revision),
            privateOwner:String(ui.start_page_owner_token || ""),profileName:String(ui.profile_name || "")}
    }
    function pruneActiveOwners() {
        controller.notificationOwners.slice(0).forEach(function(entry) {
            if (!controller.deferredNotificationOwnerValid(entry)
                    || !rootWindow.desktopNotificationsEnabled(entry.ui))
                controller.closeNotification(entry.notification)
        })
        controller.notificationOwners = controller.notificationOwners.filter(function(entry) {
            return entry && entry.notification
        })
        var active = controller.activeWebNotifications
        Object.keys(active).forEach(function(key) { if (!active[key]) delete active[key] })
        controller.activeWebNotifications = active
    }
    function forgetActiveNotification(notification) {
        var active=controller.activeWebNotifications
        Object.keys(active).forEach(function(key) { if(active[key]===notification) delete active[key] })
        controller.activeWebNotifications=active
        controller.forgetNotificationOwner(notification)
    }
    function presentationUnavailable(notification, profileScope, origin) {
        if (!controller.activeWebNotification(notification)) return
        var owner=controller.notificationRecordFor(notification)
        var ui=controller.deferredNotificationOwnerValid(owner) ? owner.ui : null
        controller.closeNotification(notification)
        if(ui) ui.status_text="Desktop notification service unavailable; notification cancelled"
    }
    function presentationAccepted(notification, profileScope, origin) {
        if (!controller.activeWebNotification(notification)) {
            controller.closeNotification(notification)
            return
        }
        var owner=controller.notificationRecordFor(notification)
        if(!controller.deferredNotificationOwnerValid(owner)) {
            controller.closeNotification(notification)
            return
        }
        var ui=owner.ui
        if (owner.presented) return
        if(String(ui.permission_decision(origin,"notifications"))==="deny") {
            controller.forgetActiveNotification(notification)
            controller.closeNotification(notification)
            return
        }
        owner.presented = true
        notification.show()
        ui.status_text="Desktop notification sent for " + origin
    }

    function closeWebNotifications() {
        var active = rootWindow.activeWebNotifications || ({})
        for (var key in active) {
            if (active[key]) {
                controller.closeNotification(active[key])
            }
        }
        rootWindow.activeWebNotifications = ({})
        rootWindow.notificationOwners = []
        var pending = rootWindow.pendingPortalNotifications || []
        for (var pendingIndex = 0; pendingIndex < pending.length; ++pendingIndex) {
            if (pending[pendingIndex].notification) {
                controller.closeNotification(pending[pendingIndex].notification)
            }
        }
        rootWindow.pendingPortalNotifications = []
    }

    function removePendingPortalNotification(notification) {
        var pending = rootWindow.pendingPortalNotifications || []
        rootWindow.pendingPortalNotifications = pending.filter(function(entry) {
            return entry && entry.notification !== notification
        })
    }

    function notificationOwnerFor(notification, requireKnown) {
        var owners = rootWindow.notificationOwners || []
        for (var index = 0; index < owners.length; ++index) {
            if (owners[index] && owners[index].notification === notification) {
                return owners[index].ui || rootWindow.primaryBrowserUi
            }
        }
        return requireKnown ? null : rootWindow.primaryBrowserUi
    }

    function rememberNotificationOwner(notification, ui) {
        var owners = (rootWindow.notificationOwners || []).filter(function(entry) {
            return entry && entry.notification !== notification
        })
        var owner=controller.captureOwner(notification,ui)
        owners.push(owner)
        rootWindow.notificationOwners = owners
        return owner
    }

    function forgetNotificationOwner(notification) {
        rootWindow.notificationOwners = (rootWindow.notificationOwners || []).filter(function(entry) {
            return entry && entry.notification !== notification
        })
    }

    function deferredNotificationOwnerValid(entry) {
        if (!entry || !entry.ui || !entry.host || !entry.notification || rootWindow.runtimeDisposing) return false
        if (!rootWindow.liveWindowEntries().some(function(owner) {
            return owner && owner.ui===entry.ui && owner.host===entry.host
        })) return false
        return !entry.host.runtimeReleased && !entry.host.runtimeDisposing && !entry.ui.profile_bootstrap_pending
            && String(entry.ui.start_page_profile_revision)===entry.profileRevision
            && String(entry.ui.start_page_owner_token || "")===entry.privateOwner
            && String(entry.ui.profile_name || "")===entry.profileName
    }

    function deferNotificationUntilPortal(ui, notification, privateProfile, previous) {
        ui = ui || rootWindow.primaryBrowserUi
        var pending = rootWindow.pendingPortalNotifications || []
        if (pending.some(function(entry) { return entry.notification === notification })) return
        var entry = previous || controller.captureOwner(notification, ui)
        if (!previous) {
            entry.privateProfile = !!privateProfile
            entry.deadlineMs = Date.now() + 5000
        }
        if (!controller.deferredNotificationOwnerValid(entry) || Date.now() >= entry.deadlineMs) {
            controller.closeNotification(notification)
            return
        }
        pending = pending.slice(0)
        while (pending.length >= 32) {
            var oldest = pending.shift()
            if (oldest && oldest.notification) controller.closeNotification(oldest.notification)
        }
        pending.push(entry)
        rootWindow.pendingPortalNotifications = pending
        if (!previous) notification.closed.connect(function() {
            rootWindow.removePendingPortalNotification(notification)
        })
    }

    function processPendingPortalNotifications() {
        var pending = (rootWindow.pendingPortalNotifications || []).slice(0)
        for (var index = 0; index < pending.length; ++index) {
            var entry = pending[index]
            if (!entry || rootWindow.pendingPortalNotifications.indexOf(entry) < 0) continue
            if (!controller.deferredNotificationOwnerValid(entry)) {
                controller.removePendingPortalNotification(entry.notification)
                if (entry.notification) controller.closeNotification(entry.notification)
                continue
            }
            if (!rootWindow.desktopNotificationsEnabled(entry.ui)) {
                controller.closeNotification(entry.notification)
                entry.ui.status_text = "Desktop notifications are disabled"
                continue
            }
            var origin = rootWindow.safeNotificationOrigin(entry.notification)
            if (String(entry.ui.permission_decision(origin, "notifications")) === "deny") {
                controller.closeNotification(entry.notification)
                entry.ui.status_text = "Desktop notification blocked for " + origin
                continue
            }
            if (Date.now() >= entry.deadlineMs) {
                controller.closeNotification(entry.notification)
                entry.ui.status_text = "Desktop notification check timed out; notification cancelled"
                continue
            }
            var status = rootWindow.desktopPortalCapabilityStatus(entry.ui, "notifications")
            if (status === "pending" || status === "not-probed") continue
            if (status !== "available") {
                controller.closeNotification(entry.notification)
                entry.ui.status_text = "Required desktop notification service unavailable; notification cancelled"
                continue
            }
            // Remove this entry before presentation, which may defer it again.
            rootWindow.removePendingPortalNotification(entry.notification)
            rootWindow.presentWebNotification(entry.ui, entry.notification, entry.privateProfile, entry)
        }
    }

    function closeWebNotificationsForOrigin(origin, sourceUi) {
        var expected = String(origin || "").toLowerCase()
        if (expected.length === 0) {
            return
        }
        var active = rootWindow.activeWebNotifications || ({})
        for (var key in active) {
            var parts = String(key).split("\n")
            if (parts.length >= 3 && parts[1].toLowerCase() === expected
                    && rootWindow.sharesPermissionSession(sourceUi || rootWindow.primaryBrowserUi,
                        rootWindow.notificationOwnerFor(active[key], true))) {
                if (active[key]) {
                    controller.closeNotification(active[key])
                }
                delete active[key]
            }
        }
        rootWindow.activeWebNotifications = active
        var pending = rootWindow.pendingPortalNotifications || []
        var retained = []
        for (var pendingIndex = 0; pendingIndex < pending.length; ++pendingIndex) {
            var entry = pending[pendingIndex]
            var pendingOrigin = rootWindow.safeNotificationOrigin(
                        entry && entry.notification ? entry.notification : null)
            if (pendingOrigin.toLowerCase() === expected
                    && entry && rootWindow.sharesPermissionSession(sourceUi || rootWindow.primaryBrowserUi, entry.ui)) {
                if (entry.notification) {
                    controller.closeNotification(entry.notification)
                }
                if (entry.ui) {
                    entry.ui.status_text = "Desktop notification revoked for " + pendingOrigin
                }
            } else {
                retained.push(entry)
            }
        }
        rootWindow.pendingPortalNotifications = retained

    }

    function activeWebNotification(notification) {
        var active = rootWindow.activeWebNotifications || ({})
        var keys = Object.keys(active)
        for (var index = 0; index < keys.length; ++index) {
            if (active[keys[index]] === notification) {
                return true
            }
        }
        return false
    }

    function notificationProfileScope(ui, privateProfile) {
        var kind = privateProfile ? "private" : "normal"
        var name = ui && ui.profile_name ? String(ui.profile_name) : "default"
        name = rootWindow.boundedPageDialogText(name).replace(/[\u0000-\u001f\u007f\n\r]/g, "�")
        var owner=ui ? String(ui.start_page_owner_token || "") : ""
        if(privateProfile && owner.length===0) return ""
        return kind + ":" + encodeURIComponent(name.length > 0 ? name : "default")
            + (privateProfile ? ":" + encodeURIComponent(owner) : "")
    }

    function presentWebNotification(ui, notification, privateProfile, previous) {
        if (!notification) {
            return
        }
        if (controller.activeWebNotification(notification)) return
        var origin = rootWindow.safeNotificationOrigin(notification)
        if (origin === "opaque or unavailable origin") {
            controller.closeNotification(notification)
            if (ui) ui.status_text = "Desktop notification blocked: the site couldn't be identified"
            return
        }
        if (!rootWindow.desktopNotificationsEnabled(ui)) {
            controller.closeNotification(notification)
            if (ui) ui.status_text = "Desktop notifications are disabled"
            return
        }
        var decision = ui && ui.permission_decision
                ? String(ui.permission_decision(origin, "notifications")) : "deny"
        // Only Qt's presentNotification callback admits new notifications here.
        // AskEveryTime routes authorization through the application before this
        // callback. A pending saved-policy lookup cannot undo that grant; a
        // definite Deny, revocation, disabled setting or retired owner still can.
        if (decision !== "allow" && decision !== "ask" && decision !== "pending") {
            controller.closeNotification(notification)
            if (ui) ui.status_text = "Desktop notification blocked for " + origin
            return
        }
        if (rootWindow.desktopPortalMode(ui) === "required") {
            var portalStatus = rootWindow.desktopPortalCapabilityStatus(ui, "notifications")
            if (portalStatus === "not-probed") {
                rootWindow.primaryBrowserUi.probe_desktop_portals()
                portalStatus = rootWindow.desktopPortalCapabilityStatus(ui, "notifications")
            }
            if (portalStatus === "pending" || portalStatus === "not-probed") {
                rootWindow.deferNotificationUntilPortal(ui, notification, privateProfile, previous)
                ui.status_text = "Checking the desktop notification service…"
                return
            }
            if (portalStatus !== "available") {
                controller.closeNotification(notification)
                ui.status_text = "Required desktop notification service unavailable; notification cancelled"
                return
            }
        }
        var tag = notification.tag ? String(notification.tag) : ""
        var profileScope = rootWindow.notificationProfileScope(ui, privateProfile)
        if(profileScope.length===0) {
            controller.closeNotification(notification)
            return
        }
        var notificationKey = tag.length > 0
                ? profileScope + "\n" + origin + "\n" + rootWindow.boundedPageDialogText(tag)
                : profileScope + "\n" + origin + "\n#" + (++rootWindow.webNotificationSequence)
        var active = rootWindow.activeWebNotifications || ({})
        var replaced = active[notificationKey]
        if (replaced && replaced !== notification) {
            controller.closeNotification(replaced)
        }
        active[notificationKey] = notification
        rootWindow.activeWebNotifications = active
        var owner=rootWindow.rememberNotificationOwner(notification, ui)
        if(!controller.deferredNotificationOwnerValid(owner)) {
            controller.closeNotification(notification)
            return
        }
        notification.closed.connect(function() {
            var current = rootWindow.activeWebNotifications || ({})
            if (current[notificationKey] === notification) {
                delete current[notificationKey]
                rootWindow.activeWebNotifications = current
            }
            rootWindow.forgetNotificationOwner(notification)
            controller.notificationPresenter.cancel(notification)
        })
        var requiredPortal = rootWindow.desktopPortalMode(ui) === "required"
        if (controller.notificationPresenter.present(notification, profileScope, origin, requiredPortal)) {
            ui.status_text = "Sending desktop notification for " + origin
            return
        }
        controller.closeNotification(notification)
        ui.status_text = "Desktop notification couldn't be sent; try again"
    }
}
