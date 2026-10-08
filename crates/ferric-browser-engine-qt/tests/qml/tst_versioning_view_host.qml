import QtQuick
import QtQuick.Controls
import QtTest
import QtWebEngine
import "../../qml/components"

TestCase {
    id: probe
    name: "VersioningViewHost"
    when: windowShown
    width: 480
    height: 320

    ApplicationWindow {
        id: testWindow
        width: 480
        height: 320
        visible: true
    }

    WebEngineProfilePrototype {
        id: profilePrototype
        storageName: ""
    }

    WebEngineProfilePrototype {
        id: foreignProfilePrototype
        storageName: ""
    }

    property var disposableProfile: null
    property var foreignProfile: null
    property int destroyedViews: 0

    function initTestCase() {
        disposableProfile = profilePrototype.instance()
        foreignProfile = foreignProfilePrototype.instance()
        verify(disposableProfile, "the off-record profile instance must exist")
        verify(foreignProfile && foreignProfile !== disposableProfile)
    }

    function init() {
        destroyedViews = 0
    }

    Component {
        id: visualContainerFactory
        Item { width: 480; height: 320 }
    }

    Component {
        id: hostFactory
        FerricVersioningViewHost {
            ownerProfile: probe.disposableProfile
            originalHost: testWindow
        }
    }

    Component {
        id: viewFactory
        WebEngineView {
            property string stableTabId: ""
            property var viewProfile: null
            property var versioningOwnerHost: null
            property int versioningNavigationEpoch: 0
            property bool loaded: false
            property string loadFailure: ""
            profile: viewProfile
            onLoadingChanged: function(request) {
                if (request.status === WebEngineView.LoadStartedStatus) {
                    versioningNavigationEpoch += 1
                } else if (request.status === WebEngineView.LoadSucceededStatus) {
                    loaded = true
                } else if (request.status === WebEngineView.LoadFailedStatus) {
                    loadFailure = request.errorString
                }
            }
            Component.onDestruction: probe.destroyedViews += 1
        }
    }

    function evaluate(view, script) {
        var answer = { done: false, value: null }
        view.runJavaScript(script, function(value) {
            answer.value = value
            answer.done = true
        })
        tryVerify(function() { return answer.done }, 10000)
        return answer.value
    }

    function test_exact_view_survives_tab_presentation_removal_then_releases() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        verify(host && strip)
        var view = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-1", viewProfile: probe.disposableProfile
        })
        verify(view)
        compare(view.parent, strip)
        view.url = "data:text/html,<html><body>synthetic</body></html>"
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        compare(evaluate(view, "window.sentinel={identity:'original'}; window.sentinel.identity"),
                "original")

        var visibleViews = [view]
        compare(host.retainedCount, 0)
        verify(host.retainView("ret-1", view, probe.disposableProfile, "tab-1"))
        visibleViews = host.visibleViewsOnly(visibleViews)
        strip.destroy()
        compare(visibleViews.length, 0)
        compare(host.retainedCount, 1)
        compare(host.visibleViewsOnly([view]).length, 0)
        compare(host.retainedView("ret-1"), view)
        verify(host.hasRetainedTab("tab-1"))
        verify(host.ownsRetainedView(view))
        compare(host.membershipForView(view), "Retained")
        compare(view.parent, host)
        verify(!host.restoreRetainedStash("ret-1", view))
        compare(evaluate(view, "window.sentinel.identity"), "original")
        verify(!host.retainView("ret-1", view, probe.disposableProfile, "tab-1"))
        verify(!host.retainView("ret-2", view, probe.disposableProfile, "tab-1"))
        compare(host.retainedCount, 1)

        verify(host.releaseView("ret-1"))
        tryCompare(probe, "destroyedViews", 1, 5000)
        compare(host.retainedCount, 0)
        compare(host.retainedView("ret-1"), null)
        verify(!host.hasRetainedTab("tab-1"))
        verify(!host.ownsRetainedView(view))
        verify(!host.releaseView("ret-1"))
    }

    function test_profile_and_tab_identity_are_exact() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        verify(host && strip)
        var view = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-foreign", viewProfile: disposableProfile
        })
        verify(view)
        verify(!host.retainView("ret-foreign", view, foreignProfile, "tab-foreign"))
        verify(!host.retainView("ret-wrong-tab", view,
                                disposableProfile, "another-tab"))
        compare(host.retainedCount, 0)
        compare(host.membershipForView(view), "Visible")
        compare(view.parent, strip)
    }

    function test_admitted_start_source_check_is_read_only_and_exact() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        verify(host && strip)
        var source = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-start", viewProfile: disposableProfile
        })
        var other = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-other", viewProfile: disposableProfile
        })
        verify(source && other)
        var ids = ["tab-start", "tab-other"]
        var views = [source, other]
        verify(!host.hasAdmittedVisibleSource("tab-start", ids, [other, source]))
        verify(!host.hasAdmittedVisibleSource("tab-start", ids,
                                               [source, source]))
        verify(!host.hasAdmittedVisibleSource("tab-start",
                                               ["tab-start", "tab-start"], views))
        verify(!host.hasAdmittedVisibleSource("tab-missing", ids, views))
        verify(host.hasAdmittedVisibleSource("tab-start", ids, views))
        compare(source.parent, strip)
        compare(host.preparedCount, 0)
        compare(host.retainedCount, 0)
        source.stableTabId = "tab-changed"
        verify(!host.hasAdmittedVisibleSource("tab-start", ids, views))
        source.stableTabId = "tab-start"
    }

    function test_staged_stash_requires_observed_freeze_and_preserves_instance() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        verify(host && strip)
        var view = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-stage", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-fallback", viewProfile: disposableProfile
        })
        verify(view && fallback)
        view.url = "data:text/html,<html><body>staged</body></html>"
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        compare(evaluate(view, "window.stashSentinel={id:'original'}; stashSentinel.id"),
                "original")
        var originalEpoch = view.versioningNavigationEpoch

        verify(host.prepareStash("ret-stage", view, disposableProfile,
                                 "tab-stage", "tab-fallback", fallback))
        compare(host.preparedCount, 1)
        compare(host.retainedCount, 0)
        verify(!host.acknowledgeFrozenStash("ret-stage", view))
        tryCompare(view, "recommendedState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(host.requestPreparedFreeze("ret-stage"))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(host.acknowledgeFrozenStash("ret-stage", view))
        verify(!host.acknowledgeFrozenStash("ret-stage", view))
        compare(host.preparedCount, 0)
        compare(host.retainedCount, 1)
        compare(host.retainedView("ret-stage"), view)
        view.lifecycleState = WebEngineView.LifecycleState.Active
        view.parent = strip
        view.visible = true
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        compare(evaluate(view, "window.stashSentinel.id"), "original")
        compare(view.versioningNavigationEpoch, originalEpoch)
        verify(host.releaseView("ret-stage"))
    }

    function test_reserved_stash_requires_aligned_visible_projection_and_selected_blank_fallback() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        verify(host && strip)
        var source = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-reserved", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-reserved", viewProfile: disposableProfile
        })
        verify(source && fallback)
        var ids = ["tab-reserved", "fallback-reserved"]
        var views = [source, fallback]
        verify(!host.hasMaterializedReservedFallback(ids[0], ids[1],
                                                     ids, views, ids[0]))
        verify(!host.prepareReservedStash("ret-reserved", ids[0], ids[1],
                                          ids, views, ids[0]))
        verify(!host.prepareReservedStash("ret-reserved", ids[0], ids[1],
                                          ids, [fallback, source], ids[1]))
        verify(!host.prepareReservedStash("ret-reserved", ids[0], ids[1],
                                          [ids[0], ids[0]], views, ids[1]))
        verify(!host.prepareReservedStash("ret-reserved", ids[0], ids[1],
                                          ids, [source], ids[1]))
        compare(host.preparedCount, 0)
        compare(source.parent, strip)
        fallback.url = "data:text/html,<html><body>not blank</body></html>"
        tryVerify(function() { return !fallback.loading }, 5000)
        verify(!host.hasMaterializedReservedFallback(ids[0], ids[1],
                                                     ids, views, ids[1]))
        verify(!host.prepareReservedStash("ret-reserved", ids[0], ids[1],
                                          ids, views, ids[1]))
        fallback.url = "about:blank"
        tryVerify(function() { return !fallback.loading }, 5000)
        verify(host.hasMaterializedReservedFallback(ids[0], ids[1],
                                                    ids, views, ids[1]))
        compare(source.parent, strip)
        compare(host.preparedCount, 0)
        verify(host.prepareReservedStash("ret-reserved", ids[0], ids[1],
                                         ids, views, ids[1]))
        compare(host.preparedCount, 1)
        compare(source.parent, host)
        verify(host.cancelPreparedStash("ret-reserved"))
        tryCompare(source, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        verify(host.acknowledgeRestoredStash("ret-reserved", source,
            function() { return true }))
        compare(source.parent, strip)
        compare(host.restoringCount, 0)
    }

    function test_read_only_hold_survives_pre_hide_refusal_for_exact_cleanup() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        var source = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-held", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-held", viewProfile: disposableProfile
        })
        verify(host && strip && source && fallback)
        source.url = "data:text/html,<html><body>held source</body></html>"
        tryVerify(function() { return source.loaded && !source.loading }, 15000)
        compare(evaluate(source, "window.heldSentinel='same';heldSentinel"), "same")
        var epoch = source.versioningNavigationEpoch
        var ids = ["tab-held", "fallback-held"]
        var views = [source, fallback]
        verify(host.holdReservedStash("ret-held", ids[0], ids[1],
                                      ids, views, ids[1]))
        compare(host.heldCount, 1)
        compare(source.parent, strip)
        verify(source.visible && source.enabled)
        verify(host.isPreparedView(source))
        verify(host.isPreparedView(fallback))
        verify(!host.holdReservedStash("ret-held", ids[0], ids[1],
                                       ids, views, ids[1]))
        verify(!host.prepareHeldStash("wrong-stage", ids[0], ids[1],
                                      ids, views, ids[1]))

        fallback.url = "data:text/html,<html><body>changed fallback</body></html>"
        tryVerify(function() { return !fallback.loading }, 15000)
        verify(!host.prepareHeldStash("ret-held", ids[0], ids[1],
                                      ids, views, ids[1]))
        compare(host.heldCount, 1)
        compare(host.preparedCount, 0)
        compare(source.parent, strip)
        verify(!host.restoreUncommittedStash("wrong-stage"))
        verify(host.restoreUncommittedStash("ret-held"))
        compare(host.heldCount, 0)
        compare(host.restoringCount, 1)
        compare(host.restoringView("ret-held"), source)
        compare(host.restoringView("wrong-stage"), null)
        verify(host.restoreUncommittedStash("ret-held"))
        tryCompare(source, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        verify(!host.acknowledgeRestoredStash("ret-held", fallback,
            function() { return true }))
        verify(!host.acknowledgeRestoredStash("ret-held", source,
            function() { return false }))
        compare(host.restoringCount, 1)
        compare(host.restoringView("ret-held"), source)
        verify(host.acknowledgeRestoredStash("ret-held", source,
            function() { return true }))
        compare(host.restoringCount, 0)
        compare(host.restoringView("ret-held"), null)
        compare(source.parent, strip)
        compare(evaluate(source, "window.heldSentinel"), "same")
        compare(source.versioningNavigationEpoch, epoch)
    }

    function test_read_only_hold_advances_to_prepared_stage_once() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        var source = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-held-stage", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-held-stage", viewProfile: disposableProfile
        })
        verify(host && strip && source && fallback)
        var ids = ["tab-held-stage", "fallback-held-stage"]
        var views = [source, fallback]
        verify(host.holdReservedStash("ret-held-stage", ids[0], ids[1],
                                      ids, views, ids[1]))
        verify(host.prepareHeldStash("ret-held-stage", ids[0], ids[1],
                                     ids, views, ids[1]))
        compare(host.heldCount, 0)
        compare(host.preparedCount, 1)
        compare(source.parent, host)
        verify(!host.prepareHeldStash("ret-held-stage", ids[0], ids[1],
                                      ids, views, ids[1]))
        verify(host.restoreUncommittedStash("ret-held-stage"))
        tryCompare(source, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        verify(host.acknowledgeRestoredStash("ret-held-stage", source,
            function() { return true }))
        compare(source.parent, strip)
    }

    function test_read_only_hold_refuses_new_native_navigation_before_hide() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        var source = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-held-nav", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-held-nav", viewProfile: disposableProfile
        })
        verify(host && strip && source && fallback)
        var ids = ["tab-held-nav", "fallback-held-nav"]
        var views = [source, fallback]
        verify(host.holdReservedStash("ret-held-nav", ids[0], ids[1],
                                      ids, views, ids[1]))
        var epoch = source.versioningNavigationEpoch
        source.url = "data:text/html,<html><body>new document</body></html>"
        tryVerify(function() {
            return source.versioningNavigationEpoch > epoch && !source.loading
        }, 15000)
        verify(!host.prepareHeldStash("ret-held-nav", ids[0], ids[1],
                                      ids, views, ids[1]))
        compare(host.heldCount, 1)
        compare(host.preparedCount, 0)
        compare(source.parent, strip)
        verify(host.restoreUncommittedStash("ret-held-nav"))
        tryCompare(source, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        verify(host.acknowledgeRestoredStash("ret-held-nav", source,
            function() { return true }))
    }

    function test_pre_ticket_hold_release_requires_untouched_active_source() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        var source = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-held-abort", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-held-abort", viewProfile: disposableProfile
        })
        verify(host && strip && source && fallback)
        var ids = ["tab-held-abort", "fallback-held-abort"]
        var views = [source, fallback]
        verify(host.holdReservedStash("ret-held-abort", ids[0], ids[1],
                                      ids, views, ids[1]))
        source.visible = false
        verify(!host.releaseHeldStash("ret-held-abort"))
        compare(host.heldCount, 1)
        source.visible = true
        verify(host.releaseHeldStash("ret-held-abort"))
        compare(host.heldCount, 0)
        compare(host.restoringCount, 0)
        verify(!host.restoreUncommittedStash("ret-held-abort"))
        compare(source.parent, strip)
    }

    function test_staged_stash_rolls_back_on_navigation_or_missing_fallback() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        var view = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-rollback", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-rollback", viewProfile: disposableProfile
        })
        verify(view && fallback)
        view.url = "data:text/html,<html><body>rollback</body></html>"
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        verify(!host.prepareStash("ret-no-fallback", view, disposableProfile,
                                  "tab-rollback", "fallback-rollback", null))
        verify(!host.prepareStash("ret-wrong-fallback", view, disposableProfile,
                                  "tab-rollback", "different-fallback", fallback))
        verify(!host.prepareStash("ret-source-as-fallback", view, disposableProfile,
                                  "tab-rollback", "tab-rollback", fallback))
        verify(!host.prepareStash("ret-empty-fallback-id", view, disposableProfile,
                                  "tab-rollback", "", fallback))
        compare(view.parent, strip)
        compare(host.preparedCount, 0)
        verify(host.prepareStash("ret-rollback", view, disposableProfile,
                                 "tab-rollback", "fallback-rollback", fallback))
        verify(!host.prepareStash("ret-duplicate", view, disposableProfile,
                                  "tab-rollback", "fallback-rollback", fallback))
        var previousEpoch = view.versioningNavigationEpoch
        view.url = "data:text/html,<html><body>navigated</body></html>"
        tryVerify(function() { return view.versioningNavigationEpoch > previousEpoch }, 5000)
        verify(!host.requestPreparedFreeze("ret-rollback"))
        verify(!host.acknowledgeFrozenStash("ret-rollback", view))
        verify(host.cancelPreparedStash("ret-rollback"))
        compare(host.preparedCount, 0)
        compare(host.restoringCount, 1)
        compare(host.retainedCount, 0)
        compare(view.parent, strip)
        verify(view.visible)
        verify(!host.cancelPreparedStash("ret-rollback"))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        verify(host.acknowledgeRestoredStash("ret-rollback", view,
            function() { return true }))
        compare(host.restoringCount, 0)
    }

    function test_cancelling_already_active_stage_notifies_after_reparenting() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        var view = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-active-cancel", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-active-cancel", viewProfile: disposableProfile
        })
        verify(host && strip && view && fallback)
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        var notifications = 0
        host.restorationStarted.connect(function(restored) {
            compare(restored, view)
            compare(restored.parent, strip)
            verify(restored.visible && restored.enabled)
            notifications += 1
        })
        verify(host.prepareStash("active-cancel", view, disposableProfile,
                                 "tab-active-cancel", "fallback-active-cancel", fallback))
        verify(host.cancelPreparedStash("active-cancel"))
        compare(notifications, 1)
        compare(view.lifecycleState, WebEngineView.LifecycleState.Active)
        verify(host.acknowledgeRestoredStash("active-cancel", view,
            function() { return true }))
        compare(host.restoringCount, 0)
    }

    function test_cleanup_request_restores_only_exact_uncommitted_stage() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        var view = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-cleanup-request", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-cleanup-request", viewProfile: disposableProfile
        })
        verify(host && strip && view && fallback)
        view.url = "data:text/html,<html><body>cleanup request</body></html>"
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        compare(evaluate(view, "window.cleanupSentinel='same';cleanupSentinel"), "same")
        var epoch = view.versioningNavigationEpoch
        verify(host.prepareStash("ret-cleanup-request", view, disposableProfile,
                                 "tab-cleanup-request", "fallback-cleanup-request", fallback))
        verify(!host.restoreUncommittedStash("wrong-stage"))
        compare(host.preparedCount, 1)
        verify(host.restoreUncommittedStash("ret-cleanup-request"))
        compare(host.preparedCount, 0)
        compare(host.restoringCount, 1)
        verify(host.restoreUncommittedStash("ret-cleanup-request"))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        verify(host.acknowledgeRestoredStash("ret-cleanup-request", view,
            function() { return true }))
        compare(host.restoringCount, 0)
        compare(view.parent, strip)
        compare(evaluate(view, "window.cleanupSentinel"), "same")
        compare(view.versioningNavigationEpoch, epoch)
        verify(!host.restoreUncommittedStash("ret-cleanup-request"))

        verify(host.prepareStash("ret-cleanup-frozen", view, disposableProfile,
                                 "tab-cleanup-request", "fallback-cleanup-request", fallback))
        tryCompare(view, "recommendedState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(host.requestPreparedFreeze("ret-cleanup-frozen"))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(host.acknowledgeFrozenStash("ret-cleanup-frozen", view))
        verify(host.restoreUncommittedStash("ret-cleanup-frozen"))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        verify(host.acknowledgeRestoredStash("ret-cleanup-frozen", view,
            function() { return true }))
        compare(evaluate(view, "window.cleanupSentinel"), "same")
        compare(view.versioningNavigationEpoch, epoch)
    }

    function test_rejected_application_commit_restores_exact_frozen_view() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        var view = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-commit-rejected", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-commit-rejected", viewProfile: disposableProfile
        })
        verify(view && fallback)
        view.url = "data:text/html,<html><body>restore</body></html>"
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        compare(evaluate(view,
            "window.restoreSentinel={identity:'original'};restoreSentinel.identity"),
            "original")
        var originalEpoch = view.versioningNavigationEpoch

        verify(host.prepareStash("ret-rejected", view, disposableProfile,
                                 "tab-commit-rejected", "fallback-commit-rejected", fallback))
        tryCompare(view, "recommendedState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(host.requestPreparedFreeze("ret-rejected"))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(!host.restoreRetainedStash("ret-rejected", view))
        verify(host.acknowledgeFrozenStash("ret-rejected", view))
        verify(!host.restoreRetainedStash("ret-rejected", fallback))
        compare(host.retainedCount, 1)
        verify(host.restoreRetainedStash("ret-rejected", view))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        compare(view.parent, strip)
        verify(view.visible)
        verify(view.enabled)
        compare(host.retainedCount, 0)
        compare(host.restoringCount, 1)
        verify(host.isPreparedView(view))
        verify(host.isPreparedView(fallback))
        verify(!host.acknowledgeRestoredStash("ret-rejected", fallback,
            function() { return true }))
        verify(!host.acknowledgeRestoredStash("ret-rejected", view,
            function() { return false }))
        compare(host.restoringCount, 1)
        verify(host.acknowledgeRestoredStash("ret-rejected", view,
            function() { return true }))
        compare(host.restoringCount, 0)
        compare(host.membershipForView(view), "Visible")
        compare(evaluate(view, "window.restoreSentinel.identity"), "original")
        compare(view.versioningNavigationEpoch, originalEpoch)
        verify(!host.restoreRetainedStash("ret-rejected", view))
    }

    function test_host_calls_commit_only_after_actual_freeze_and_reports_restoration() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        var view = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-callback", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-callback", viewProfile: disposableProfile
        })
        verify(view && fallback)
        view.url = "data:text/html,<html><body>callback</body></html>"
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        compare(evaluate(view, "window.callbackSentinel='same';callbackSentinel"), "same")
        var originalEpoch = view.versioningNavigationEpoch
        var commits = 0
        var rollbacks = 0
        verify(host.prepareStash("ret-callback", view, disposableProfile,
                                 "tab-callback", "fallback-callback", fallback))
        verify(!host.completeFrozenStash("ret-callback", view,
            function() { commits += 1; return true }))
        compare(commits, 0)
        compare(rollbacks, 0)
        tryCompare(view, "recommendedState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(host.requestPreparedFreeze("ret-callback"))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(!host.completeFrozenStash("ret-callback", fallback,
            function() { commits += 1; return true }))
        compare(commits, 0)
        verify(!host.completeFrozenStash("ret-callback", view,
            function() { commits += 1; return false }))
        compare(commits, 1)
        compare(rollbacks, 0)
        compare(host.preparedCount, 0)
        compare(host.retainedCount, 0)
        compare(host.restoringCount, 1)
        verify(host.isPreparedView(fallback))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        verify(host.acknowledgeRestoredStash("ret-callback", view,
            function() { rollbacks += 1; return true }))
        compare(rollbacks, 1)
        compare(host.restoringCount, 0)
        compare(view.parent, strip)
        compare(evaluate(view, "window.callbackSentinel"), "same")
        compare(view.versioningNavigationEpoch, originalEpoch)
        verify(!host.completeFrozenStash("ret-callback", view,
            function() { commits += 1; return true }))
        compare(commits, 1)

        verify(host.prepareStash("ret-callback-2", view, disposableProfile,
                                 "tab-callback", "fallback-callback", fallback))
        tryCompare(view, "recommendedState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(host.requestPreparedFreeze("ret-callback-2"))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(host.completeFrozenStash("ret-callback-2", view,
            function() { commits += 1; return true }))
        compare(commits, 2)
        compare(rollbacks, 1)
        compare(host.retainedCount, 1)
        compare(host.retainedView("ret-callback-2"), view)
        verify(!host.isPreparedView(fallback))
        verify(!host.restoreRetainedStash("ret-callback-2", view))
        verify(!host.completeFrozenStash("ret-callback-2", view,
            function() { commits += 1; return true }))
        compare(commits, 2)
        verify(host.releaseView("ret-callback-2"))
    }

    function test_uncertain_application_callback_keeps_native_owner() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        var view = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-uncertain", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-uncertain", viewProfile: disposableProfile
        })
        verify(view && fallback)
        view.url = "data:text/html,<html><body>uncertain</body></html>"
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        verify(host.prepareStash("ret-uncertain", view, disposableProfile,
                                 "tab-uncertain", "fallback-uncertain", fallback))
        tryCompare(view, "recommendedState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(host.requestPreparedFreeze("ret-uncertain"))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(!host.completeFrozenStash("ret-uncertain", view,
            function() { throw new Error("unknown application outcome") }))
        compare(host.retainedCount, 1)
        compare(host.restoringCount, 0)
        compare(host.retainedView("ret-uncertain"), view)
        compare(view.parent, host)
        verify(host.isPreparedView(fallback))
        // Synthetic fixture teardown, not a claim that an uncertain real
        // application outcome may be discarded without reconciliation.
        verify(host.releaseView("ret-uncertain"))
    }

    function test_cancel_after_freeze_request_restores_original_live_page() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(visualContainerFactory, testWindow.contentItem)
        var view = host.createOwnedView(viewFactory, strip, {
            stableTabId: "tab-cancel", viewProfile: disposableProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback-cancel", viewProfile: disposableProfile
        })
        verify(view && fallback)
        view.url = "data:text/html,<html><body>cancel</body></html>"
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        compare(evaluate(view,
            "window.cancelSentinel={identity:'original'};cancelSentinel.identity"),
            "original")
        var originalEpoch = view.versioningNavigationEpoch

        verify(host.prepareStash("ret-cancel", view, disposableProfile,
                                 "tab-cancel", "fallback-cancel", fallback))
        tryCompare(view, "recommendedState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(host.requestPreparedFreeze("ret-cancel"))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Frozen, 5000)
        verify(host.cancelPreparedStash("ret-cancel"))
        tryCompare(view, "lifecycleState", WebEngineView.LifecycleState.Active, 5000)
        compare(host.restoringCount, 1)
        verify(host.acknowledgeRestoredStash("ret-cancel", view,
            function() { return true }))
        compare(host.restoringCount, 0)
        compare(view.parent, strip)
        verify(view.visible)
        compare(evaluate(view, "window.cancelSentinel.identity"), "original")
        compare(view.versioningNavigationEpoch, originalEpoch)
        compare(host.preparedCount, 0)
        compare(host.retainedCount, 0)
        verify(!host.acknowledgeFrozenStash("ret-cancel", view))
    }
}
