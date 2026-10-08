import QtQuick
import QtQuick.Controls
import QtTest
import QtWebEngine
import "../../../crates/ferric-browser-engine-qt/qml/components"

TestCase {
    id: probe
    name: "PinnedBackendOwner"
    when: windowShown
    width: 480
    height: 320

    ApplicationWindow {
        id: testWindow
        width: 480
        height: 320
        visible: true
    }

    WebEngineProfilePrototype { id: firstProfilePrototype; storageName: "" }
    WebEngineProfilePrototype { id: secondProfilePrototype; storageName: "" }
    property var firstProfile: null
    property var secondProfile: null

    function initTestCase() {
        firstProfile = firstProfilePrototype.instance()
        secondProfile = secondProfilePrototype.instance()
        verify(firstProfile && secondProfile && firstProfile !== secondProfile)
    }

    Component {
        id: stripFactory
        Item { width: 480; height: 320 }
    }

    Component {
        id: hostFactory
        FerricVersioningViewHost {
            ownerProfile: probe.firstProfile
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
            profile: viewProfile
            onLoadingChanged: function(request) {
                if (request.status === WebEngineView.LoadStartedStatus)
                    versioningNavigationEpoch += 1
            }
        }
    }

    SignalSpy {
        id: rendererTermination
        signalName: "renderProcessTerminated"
    }

    function test_partial_backend_abi_refuses() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        verify(host)
        var partial = {
            ferricVersioningBackendRevision: function() { return 1 }
        }
        compare(host.nativeBackendWitness(partial), false)
    }

    function test_distinct_views_and_adapter_replacement() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(stripFactory, testWindow.contentItem)
        verify(host && strip)
        var first = host.createOwnedView(viewFactory, strip, {
            stableTabId: "first", viewProfile: firstProfile
        })
        var second = host.createOwnedView(viewFactory, strip, {
            stableTabId: "second", viewProfile: firstProfile
        })
        verify(first && second)
        compare(first.ferricVersioningBackendRevision(), 1)
        compare(second.ferricVersioningBackendRevision(), 1)
        var firstWitness = host.nativeBackendWitness(first)
        var secondWitness = host.nativeBackendWitness(second)
        verify(firstWitness && secondWitness)
        verify(firstWitness.viewId !== secondWitness.viewId)
        verify(host.nativeBackendWitnessMatches(first, firstWitness))
        verify(!host.nativeBackendWitnessMatches(second, firstWitness))

        first.profile = secondProfile
        verify(!host.nativeBackendWitnessMatches(first, firstWitness))
        first.profile = firstProfile
        var replacement = host.nativeBackendWitness(first)
        verify(replacement)
        compare(replacement.viewId, firstWitness.viewId)
        verify(replacement.adapterId !== firstWitness.adapterId)
        verify(!host.nativeBackendWitnessMatches(first, firstWitness))
        verify(host.nativeBackendWitnessMatches(first, replacement))
    }

    function test_held_ferric_ticket_refuses_rebound_adapter() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(stripFactory, testWindow.contentItem)
        verify(host && strip)
        var source = host.createOwnedView(viewFactory, strip, {
            stableTabId: "source", viewProfile: firstProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback", viewProfile: firstProfile
        })
        verify(source && fallback)
        source.url = "data:text/html,<html><body>owner witness</body></html>"
        tryVerify(function() {
            return source.versioningNavigationEpoch > 0 && !source.loading
        }, 15000)
        var ids = ["source", "fallback"]
        var views = [source, fallback]
        verify(host.holdReservedStash("operation-1", ids[0], ids[1],
                                      ids, views, ids[1]))
        compare(host.heldCount, 1)
        var oldWitness = host.nativeBackendWitness(source)
        source.profile = secondProfile
        source.profile = firstProfile
        tryVerify(function() { return !source.loading }, 15000)
        verify(!host.nativeBackendWitnessMatches(source, oldWitness))
        verify(!host.prepareHeldStash("operation-1", ids[0], ids[1],
                                      ids, views, ids[1]))
        compare(host.preparedCount, 0)
        compare(host.heldCount, 1)
        verify(host.releaseHeldStash("operation-1"))
        compare(host.heldCount, 0)
    }

    function test_owner_loss_refuses_a_held_ticket() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(stripFactory, testWindow.contentItem)
        verify(host && strip)
        var source = host.createOwnedView(viewFactory, strip, {
            stableTabId: "source", viewProfile: firstProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback", viewProfile: firstProfile
        })
        verify(source && fallback)
        source.url = "data:text/html,<html><body>owner loss</body></html>"
        tryVerify(function() {
            return source.versioningNavigationEpoch > 0 && !source.loading
        }, 15000)
        var ids = ["source", "fallback"]
        var views = [source, fallback]
        verify(host.holdReservedStash("operation-2", ids[0], ids[1],
                                      ids, views, ids[1]))
        source.versioningOwnerHost = null
        verify(!host.prepareHeldStash("operation-2", ids[0], ids[1],
                                      ids, views, ids[1]))
        source.versioningOwnerHost = host
        verify(host.releaseHeldStash("operation-2"))
    }

    function test_renderer_failure_invalidates_native_owner_witness() {
        var host = createTemporaryObject(hostFactory, testWindow.contentItem)
        var strip = createTemporaryObject(stripFactory, testWindow.contentItem)
        verify(host && strip)
        var source = host.createOwnedView(viewFactory, strip, {
            stableTabId: "source", viewProfile: firstProfile
        })
        var fallback = host.createOwnedView(viewFactory, strip, {
            stableTabId: "fallback", viewProfile: firstProfile
        })
        verify(source && fallback)
        source.url = "data:text/html,<html><body>renderer failure</body></html>"
        tryVerify(function() {
            return source.versioningNavigationEpoch > 0 && !source.loading
        }, 15000)
        var ids = ["source", "fallback"]
        var views = [source, fallback]
        verify(host.holdReservedStash("operation-3", ids[0], ids[1],
                                      ids, views, ids[1]))
        var witness = host.nativeBackendWitness(source)
        rendererTermination.target = source
        rendererTermination.clear()
        source.url = "chrome://crash"
        tryVerify(function() { return rendererTermination.count > 0 }, 15000)
        verify(!host.nativeBackendWitnessMatches(source, witness))
        verify(!host.prepareHeldStash("operation-3", ids[0], ids[1],
                                      ids, views, ids[1]))
        rendererTermination.target = null
    }
}
