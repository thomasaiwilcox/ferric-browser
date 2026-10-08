import QtQuick
import QtTest
import QtWebEngine
import "../../qml/components"

TestCase {
    id: testCase
    name: "CleanBranchProfile"
    when: windowShown

    QtObject {
        id: rootWindow
        property var browserProfile: null
        property var browserWindowRegistry: []
    }
    QtObject { id: rootUi }

    FerricWindowRegistryController {
        id: controller
        browserWindow: rootWindow
        browserUi: rootUi
    }

    Component {
        id: profileFactory
        WebEngineProfile {
            offTheRecord: true
            storageName: ""
            persistentStoragePath: ""
            cachePath: ""
            persistentPermissionsPolicy: WebEngineProfile.AskEveryTime
            isPushServiceEnabled: false
        }
    }
    Component {
        id: viewFactory
        QtObject { property var profile: null }
    }
    Component {
        id: hostFactory
        QtObject {
            property var windowSharedProfile: null
            property string windowBranchToken: ""
            readonly property bool windowCleanBranch: windowBranchToken.length > 0
        }
    }
    Component {
        id: ownedHostFactory
        Item {
            property string windowBranchToken: "branch:00000000-0000-4000-8000-000000000006"
            readonly property bool windowCleanBranch: true
            property var windowSharedProfile: null
            property bool admitted: false
            WebEngineProfile {
                id: ownedProfile
                offTheRecord: true
                storageName: ""
                persistentStoragePath: ""
                cachePath: ""
                persistentPermissionsPolicy: WebEngineProfile.AskEveryTime
                isPushServiceEnabled: false
            }
            QtObject { id: ownedView; property var profile: ownedProfile }
            Component.onCompleted: admitted = controller.retainCleanBranchOwner(
                    windowBranchToken, this, ownedProfile, ownedView)
            Component.onDestruction: controller.releaseCleanBranchOwner(
                    windowBranchToken, this, ownedProfile, ownedView)
        }
    }

    function init() {
        controller.cleanBranchOwners = ({})
        rootWindow.browserProfile = null
    }

    function test_distinct_memory_only_profile_leases() {
        var first = createTemporaryObject(profileFactory, this)
        var second = createTemporaryObject(profileFactory, this)
        var firstHost = createTemporaryObject(hostFactory, this)
        var secondHost = createTemporaryObject(hostFactory, this)
        var firstView = createTemporaryObject(viewFactory, this, { profile: first })
        var secondView = createTemporaryObject(viewFactory, this, { profile: second })
        verify(first && second && firstHost && secondHost && firstView && secondView)
        verify(first !== second)
        verify(first.offTheRecord && second.offTheRecord)
        compare(first.storageName, "")
        compare(second.storageName, "")
        compare(first.persistentCookiesPolicy, WebEngineProfile.NoPersistentCookies)
        compare(second.httpCacheType, WebEngineProfile.MemoryHttpCache)
        var firstToken = "branch:00000000-0000-4000-8000-000000000001"
        var secondToken = "branch:00000000-0000-4000-8000-000000000002"
        firstHost.windowBranchToken = firstToken
        secondHost.windowBranchToken = secondToken
        rootWindow.browserProfile = first
        verify(!controller.retainCleanBranchOwner(
                   firstToken,
                   firstHost, first, firstView))
        rootWindow.browserProfile = null
        verify(controller.retainCleanBranchOwner(firstToken, firstHost, first, firstView))
        verify(controller.retainCleanBranchOwner(secondToken, secondHost, second, secondView))
        compare(controller.cleanBranchOwner(firstToken).profile, first)
        compare(controller.cleanBranchOwner(secondToken).profile, second)
        verify(!controller.retainCleanBranchOwner(secondToken, secondHost, first, secondView))
        verify(!controller.retainCleanBranchOwner(
                   "branch:00000000-0000-4000-8000-000000000003",
                   secondHost, first, firstView))
        verify(!controller.releaseCleanBranchOwner(firstToken, secondHost, first, firstView))
        verify(controller.releaseCleanBranchOwner(firstToken, firstHost, first, firstView))
        verify(controller.releaseCleanBranchOwner(secondToken, secondHost, second, secondView))
        compare(Object.keys(controller.cleanBranchOwners).length, 0)
    }

    function test_rejects_shared_and_persistent_profile_claims() {
        var profile = createTemporaryObject(profileFactory, this)
        var host = createTemporaryObject(hostFactory, this)
        var view = createTemporaryObject(viewFactory, this, { profile: profile })
        verify(profile && host && view)
        host.windowSharedProfile = profile
        host.windowBranchToken = "branch:00000000-0000-4000-8000-000000000004"
        verify(!controller.retainCleanBranchOwner(
                   "branch:00000000-0000-4000-8000-000000000004",
                   host, profile, view))
        host.windowSharedProfile = null
        var wrongView = createTemporaryObject(viewFactory, this)
        verify(!controller.retainCleanBranchOwner(
                   "branch:00000000-0000-4000-8000-000000000004",
                   host, profile, wrongView))
        verify(!controller.retainCleanBranchOwner("not-a-branch", host, profile, view))
        compare(Object.keys(controller.cleanBranchOwners).length, 0)
        var token = "branch:00000000-0000-4000-8000-000000000005"
        host.windowBranchToken = token
        verify(controller.retainCleanBranchOwner(token, host, profile, view))
        view.profile = null
        compare(controller.cleanBranchOwner(token), null)
        verify(controller.releaseCleanBranchOwner(token, host, profile, view))
    }

    function test_final_host_destruction_releases_exact_profile_and_view() {
        var host = createTemporaryObject(ownedHostFactory, this)
        verify(host && host.admitted)
        verify(controller.cleanBranchOwner(host.windowBranchToken) !== null)
        host.destroy()
        tryVerify(function() { return Object.keys(controller.cleanBranchOwners).length === 0 })
    }
}
