import QtQuick
import QtTest
import "../../qml/components"
TestCase {
    id: test
    name: "DeferredNotificationPolicy"
    property var browserUi: ui
    QtObject {
        id: ui
        property string status_text: ""
        property string decision: "pending"
        property string portalMode: "auto"
        property string portalStatus: "available"
        property bool profile_bootstrap_pending: false
        property int start_page_profile_revision: 1
        property string start_page_owner_token: ""
        property string profile_name: "Personal"
        property string current_url: "https://example.test/"
        function permission_decision(origin, permission) { return decision }
        function probe_desktop_portals() { portalStatus = "pending" }
    }
    QtObject {
        id: otherUi
        property string status_text: ""
        property bool profile_bootstrap_pending: false
        property int start_page_profile_revision: 1
        property string start_page_owner_token: "private-two"
        property string profile_name: "Personal"
        function permission_decision(origin,permission) { return "allow" }
    }
    QtObject { id: host; property bool runtimeReleased: false; property bool runtimeDisposing: false }
    QtObject {
        id: presenterMock
        property int presentCount: 0
        property int cancelCount: 0
        property bool usedPortal: false
        property bool accepts: true
        signal notificationPresented(var notification,string profileScope,string origin)
        signal notificationUnavailable(var notification,string profileScope,string origin)
        signal notificationFinished(var notification)
        signal notificationClicked(var notification,string profileScope,string origin)
        function present(notification, profile, origin, requiredPortal) {
            presentCount++; usedPortal=requiredPortal; return accepts
        }
        function cancel(notification) { cancelCount++; notificationFinished(notification); return true }
    }
    Component {
        id: notificationFixture
        QtObject {
            property int closeCount: 0
            property int showCount: 0
            property int clickCount: 0
            property string tag: ""
            property string origin: "https://example.test"
            signal closed()
            function close() { closeCount++ }
            function show() { showCount++ }
            function click() { clickCount++ }
        }
    }
    QtObject {
        id: window
        property bool runtimeDisposing: false
        property bool ownerVisible: true
        property bool notificationsEnabled: true
        property int focusCount: 0
        readonly property var primaryBrowserUi: ui
        property alias pendingPortalNotifications: notifications.pendingPortalNotifications
        property alias activeWebNotifications: notifications.activeWebNotifications
        property alias notificationOwners: notifications.notificationOwners
        property alias webNotificationSequence: notifications.webNotificationSequence
        function liveWindowEntries() { return ownerVisible ? [{ui:ui,host:host},{ui:otherUi,host:host}] : [] }
        function safeNotificationOrigin(notification) { return notification ? notification.origin : "opaque or unavailable origin" }
        function desktopNotificationsEnabled(value) { return notificationsEnabled }
        function desktopPortalMode(value) { return value.portalMode }
        function desktopPortalCapabilityStatus(value, capability) { return value.portalStatus }
        function boundedPageDialogText(value) { return String(value) }
        function sharesPermissionSession(left,right) { return left===right }
        function focusNotificationOrigin(scope,origin) { focusCount++ }
        function closeWebNotifications() { return notifications.closeWebNotifications() }
        function removePendingPortalNotification(notification) { return notifications.removePendingPortalNotification(notification) }
        function notificationOwnerFor(notification, requireKnown) { return notifications.notificationOwnerFor(notification, requireKnown) }
        function rememberNotificationOwner(notification, ui) { return notifications.rememberNotificationOwner(notification, ui) }
        function forgetNotificationOwner(notification) { return notifications.forgetNotificationOwner(notification) }
        function deferredNotificationOwnerValid(entry) { return notifications.deferredNotificationOwnerValid(entry) }
        function deferNotificationUntilPortal(ui, notification, privateProfile, previous) { return notifications.deferNotificationUntilPortal(ui, notification, privateProfile, previous) }
        function processPendingPortalNotifications() { return notifications.processPendingPortalNotifications() }
        function notificationProfileScope(ui, privateProfile) { return notifications.notificationProfileScope(ui, privateProfile) }
        function presentWebNotification(ui, notification, privateProfile, previous) { return notifications.presentWebNotification(ui, notification, privateProfile, previous) }
        function activeWebNotification(notification) { return notifications.activeWebNotification(notification) }
        function closeWebNotificationsForOrigin(origin, sourceUi) { return notifications.closeWebNotificationsForOrigin(origin, sourceUi) }
    }
    FerricWebNotificationController { id:notifications; rootWindow:window; notificationPresenter:presenterMock }
    function create() {
        var notification = createTemporaryObject(notificationFixture, test)
        verify(notification)
        return notification
    }
    function cleanup() { window.closeWebNotifications() }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        window.closeWebNotifications()
        window.runtimeDisposing = false
        window.ownerVisible = true
        window.notificationsEnabled = true
        window.focusCount = 0
        host.runtimeReleased = false
        host.runtimeDisposing = false
        ui.decision = "pending"
        ui.portalMode = "auto"
        ui.portalStatus = "available"
        ui.start_page_profile_revision = 1
        ui.start_page_owner_token = ""
        ui.profile_name = "Personal"
        ui.profile_bootstrap_pending = false
        presenterMock.presentCount = 0
        presenterMock.cancelCount = 0
        presenterMock.usedPortal = false
        presenterMock.accepts = true
    }
    function test_qt_authorized_delivery_does_not_wait_for_saved_policy_again() {
        var n = create()
        window.presentWebNotification(ui,n,false)
        compare(presenterMock.presentCount,1)
        compare(window.pendingPortalNotifications.length,0)
        compare(n.closeCount,0)
        compare(n.showCount,0)
        presenterMock.notificationPresented(n,window.notificationProfileScope(ui,false),n.origin)
        compare(n.showCount,1)
        compare(ui.decision,"pending")
    }

    function test_resolved_allow_uses_presenter_without_native_fallback() {
        ui.decision="allow"
        var n = create()
        window.presentWebNotification(ui,n,false)
        compare(n.closeCount,0)
        compare(n.showCount,0)
        compare(presenterMock.presentCount,1)
        compare(window.pendingPortalNotifications.length,0)
    }

    function test_authorized_delivery_waits_only_for_the_required_service() {
        ui.portalMode="required"
        ui.portalStatus="pending"
        var n = create()
        window.presentWebNotification(ui,n,false)
        compare(window.pendingPortalNotifications.length,1)
        compare(presenterMock.presentCount,0)
        ui.portalStatus="available"
        window.processPendingPortalNotifications()
        compare(presenterMock.presentCount,1)
        compare(presenterMock.usedPortal,true)
        compare(window.pendingPortalNotifications.length,0)
        compare(ui.decision,"pending")
    }

    function test_expiry_cannot_restart_its_deadline() {
        ui.portalMode="required"
        ui.portalStatus="pending"
        var n = create()
        window.presentWebNotification(ui,n,false)
        window.pendingPortalNotifications[0].deadlineMs = 0
        window.presentWebNotification(ui,n,false)
        compare(window.pendingPortalNotifications[0].deadlineMs,0)
        window.processPendingPortalNotifications()
        compare(n.closeCount,1)
        window.processPendingPortalNotifications()
        compare(n.closeCount,1)
        verify(ui.status_text.indexOf("timed out")>=0)
    }

    function test_owner_invalidation_data() {
        return [{tag:"profile"},{tag:"private-session"},{tag:"closed"},{tag:"released"},
                {tag:"retired"},{tag:"bootstrap"},{tag:"shutdown"}]
    }
    function test_owner_invalidation(data) {
        ui.portalMode="required"
        ui.portalStatus="pending"
        var n = create()
        window.presentWebNotification(ui,n,false)
        if(data.tag==="profile") ui.start_page_profile_revision++
        else if(data.tag==="private-session") ui.start_page_owner_token="other-private-session"
        else if(data.tag==="closed") window.ownerVisible=false
        else if(data.tag==="released") host.runtimeReleased=true
        else if(data.tag==="retired") host.runtimeDisposing=true
        else if(data.tag==="bootstrap") ui.profile_bootstrap_pending=true
        else window.runtimeDisposing=true
        ui.decision="allow"
        window.processPendingPortalNotifications()
        compare(n.closeCount,1)
        compare(presenterMock.presentCount,0)
        compare(window.pendingPortalNotifications.length,0)
    }
    function test_navigation_in_same_profile_does_not_cancel_notification() {
        var n = create()
        window.presentWebNotification(ui,n,false)
        ui.current_url="https://other.example.test/"
        notifications.pruneActiveOwners()
        compare(presenterMock.presentCount,1)
        compare(n.closeCount,0)
    }
    function test_queue_budget_closes_evicted_notifications() {
        ui.portalMode="required"
        ui.portalStatus="pending"
        var notifications=[]
        for(var i=0;i<33;++i) {
            var n=create(); notifications.push(n)
            window.presentWebNotification(ui,n,false)
        }
        compare(window.pendingPortalNotifications.length,32)
        compare(notifications[0].closeCount,1)
        for(var j=1;j<33;++j) compare(notifications[j].closeCount,0)
        notifications[32].closed()
        compare(window.pendingPortalNotifications.length,31)
    }
    function test_portal_completion_rechecks_current_policy() {
        var n=create()
        ui.decision="allow"
        ui.portalMode="required"
        ui.portalStatus="pending"
        window.presentWebNotification(ui,n,false)
        compare(window.pendingPortalNotifications.length,1)
        ui.decision="deny"
        ui.portalStatus="available"
        window.processPendingPortalNotifications()
        compare(n.closeCount,1)
        compare(presenterMock.presentCount,0)
    }
    function test_repeated_service_checks_do_not_extend_the_deadline() {
        ui.portalMode="required"
        ui.portalStatus="pending"
        var n=create()
        window.presentWebNotification(ui,n,false)
        var deadline=window.pendingPortalNotifications[0].deadlineMs
        for(var i=0;i<4;++i) window.processPendingPortalNotifications()
        compare(window.pendingPortalNotifications[0].deadlineMs,deadline)
        window.pendingPortalNotifications[0].deadlineMs=0
        ui.portalStatus="available"
        window.processPendingPortalNotifications()
        compare(n.closeCount,1)
        compare(window.pendingPortalNotifications.length,0)
        compare(presenterMock.presentCount,0)
    }
    function test_known_denial_blocks_authorized_delivery() {
        ui.decision="deny"
        var n=create()
        window.presentWebNotification(ui,n,false)
        compare(n.closeCount,1)
        compare(presenterMock.presentCount,0)
        compare(window.pendingPortalNotifications.length,0)
    }
    function test_new_denial_cancels_while_the_required_service_is_still_pending() {
        ui.portalMode="required"
        ui.portalStatus="pending"
        var n=create()
        window.presentWebNotification(ui,n,false)
        compare(window.pendingPortalNotifications.length,1)
        ui.decision="deny"
        window.processPendingPortalNotifications()
        compare(n.closeCount,1)
        compare(window.pendingPortalNotifications.length,0)
        compare(presenterMock.presentCount,0)
    }
    function test_disabling_notifications_cancels_a_required_service_wait() {
        ui.portalMode="required"
        ui.portalStatus="pending"
        var n=create()
        window.presentWebNotification(ui,n,false)
        window.notificationsEnabled=false
        window.processPendingPortalNotifications()
        compare(n.closeCount,1)
        compare(window.pendingPortalNotifications.length,0)
        compare(presenterMock.presentCount,0)
    }

    function test_native_once_consent_is_acknowledged_only_after_delivery() {
        ui.decision="ask"
        var n=create()
        window.presentWebNotification(ui,n,false)
        compare(presenterMock.presentCount,1)
        compare(n.showCount,0)
        presenterMock.notificationPresented(n,window.notificationProfileScope(ui,false),n.origin)
        compare(n.showCount,1)
        compare(n.closeCount,0)
        verify(ui.status_text.indexOf("sent")>=0)
    }
    function test_rejected_submission_does_not_claim_service_failure_or_fallback() {
        ui.decision="ask"
        presenterMock.accepts=false
        var n=create()
        window.presentWebNotification(ui,n,false)
        compare(n.showCount,0)
        compare(n.closeCount,1)
        verify(ui.status_text.indexOf("couldn't be sent")>=0)
        compare(Object.keys(window.activeWebNotifications).length,0)
    }
    function test_asynchronous_service_failure_is_reported_honestly() {
        ui.decision="allow"
        var n=create()
        window.presentWebNotification(ui,n,false)
        presenterMock.notificationUnavailable(n,"normal:Personal",n.origin)
        compare(n.showCount,0)
        compare(n.closeCount,1)
        verify(ui.status_text.indexOf("service unavailable")>=0)
        compare(window.notificationOwners.length,0)
    }
    function test_unidentified_site_does_not_expose_internal_origin_terminology() {
        var n=create()
        n.origin="opaque or unavailable origin"
        window.presentWebNotification(ui,n,false)
        compare(n.closeCount,1)
        compare(presenterMock.presentCount,0)
        verify(ui.status_text.indexOf("couldn't be identified")>=0)
        verify(ui.status_text.indexOf("opaque")<0)
    }
    function test_late_acknowledgement_does_not_show_in_a_closed_owner() {
        ui.decision="allow"
        var n=create()
        window.presentWebNotification(ui,n,false)
        window.ownerVisible=false
        presenterMock.notificationPresented(n,window.notificationProfileScope(ui,false),n.origin)
        compare(n.showCount,0)
        compare(n.closeCount,1)
    }
    function test_late_acknowledgement_respects_new_denial() {
        ui.decision="allow"
        var n=create()
        window.presentWebNotification(ui,n,false)
        ui.decision="deny"
        presenterMock.notificationPresented(n,window.notificationProfileScope(ui,false),n.origin)
        compare(n.showCount,0)
        compare(n.closeCount,1)
    }
    function test_private_session_tags_do_not_replace_another_session() {
        ui.decision="allow"
        ui.start_page_owner_token="private-one"
        var first=create(); first.tag="same-tag"
        var second=create(); second.tag="same-tag"
        window.presentWebNotification(ui,first,true)
        window.presentWebNotification(otherUi,second,true)
        compare(first.closeCount,0)
        compare(second.closeCount,0)
        compare(Object.keys(window.activeWebNotifications).length,2)
        verify(window.notificationProfileScope(ui,true)!==window.notificationProfileScope(otherUi,true))
    }
    function test_native_finish_releases_retained_owner() {
        ui.decision="allow"
        var n=create()
        window.presentWebNotification(ui,n,false)
        presenterMock.notificationFinished(n)
        compare(Object.keys(window.activeWebNotifications).length,0)
        compare(window.notificationOwners.length,0)
        compare(n.closeCount,1)
    }
    function test_required_portal_uses_portal_transport() {
        ui.decision="allow"
        ui.portalMode="required"
        var n=create()
        window.presentWebNotification(ui,n,false)
        compare(presenterMock.presentCount,1)
        compare(presenterMock.usedPortal,true)
    }
    function test_duplicate_native_delivery_and_acknowledgement_are_idempotent() {
        ui.decision="allow"
        var n=create()
        window.presentWebNotification(ui,n,false)
        window.presentWebNotification(ui,n,false)
        compare(presenterMock.presentCount,1)
        presenterMock.notificationPresented(n,"normal:Personal",n.origin)
        presenterMock.notificationPresented(n,"normal:Personal",n.origin)
        compare(n.showCount,1)
    }
    function test_page_close_cancels_transport_without_duplicate_page_close() {
        ui.decision="allow"
        var n=create()
        window.presentWebNotification(ui,n,false)
        n.closed()
        compare(presenterMock.cancelCount,1)
        compare(n.closeCount,0)
        compare(window.notificationOwners.length,0)
    }
    function test_click_requires_a_live_owner_and_current_permission() {
        ui.decision="allow"
        var n=create()
        window.presentWebNotification(ui,n,false)
        presenterMock.notificationClicked(n,"normal:Personal",n.origin)
        compare(n.clickCount,1)
        compare(window.focusCount,1)
        ui.decision="deny"
        presenterMock.notificationClicked(n,"normal:Personal",n.origin)
        compare(n.clickCount,1)
        compare(window.focusCount,1)
        compare(n.closeCount,1)
    }
    function test_late_click_does_not_open_a_replacement_profile() {
        ui.decision="allow"
        var n=create()
        window.presentWebNotification(ui,n,false)
        ui.start_page_profile_revision++
        presenterMock.notificationClicked(n,"normal:Personal",n.origin)
        compare(n.clickCount,0)
        compare(window.focusCount,0)
        compare(n.closeCount,1)
    }
    function test_disabled_notifications_never_queue() {
        window.notificationsEnabled=false
        var n=create()
        window.presentWebNotification(ui,n,false)
        compare(n.closeCount,1)
        compare(window.pendingPortalNotifications.length,0)
    }
}
