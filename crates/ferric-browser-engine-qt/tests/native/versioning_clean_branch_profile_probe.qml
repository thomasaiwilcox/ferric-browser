import QtQuick
import QtQuick.Window
import QtWebEngine
import "../../qml/components" as FerricComponents

Window {
    id: root
    width: 320
    height: 240
    visible: true

    property var browserProfile: parentProfile
    property var firstProfileObject: firstProfile
    property var secondProfileObject: secondProfile

    QtObject { id: browserUi }
    QtObject {
        id: firstHost
        property string windowBranchToken: "branch:00000000-0000-4000-8000-000000000011"
        readonly property bool windowCleanBranch: true
        property var windowSharedProfile: null
    }
    QtObject {
        id: secondHost
        property string windowBranchToken: "branch:00000000-0000-4000-8000-000000000012"
        readonly property bool windowCleanBranch: true
        property var windowSharedProfile: null
    }

    FerricComponents.FerricWindowRegistryController {
        id: registry
        browserWindow: root
        browserUi: browserUi
    }

    WebEngineProfile { id: parentProfile; offTheRecord: true }
    WebEngineProfile {
        id: firstProfile
        offTheRecord: true
        storageName: ""
        persistentStoragePath: ""
        cachePath: ""
        persistentPermissionsPolicy: WebEngineProfile.AskEveryTime
        isPushServiceEnabled: false
    }
    WebEngineProfile {
        id: secondProfile
        offTheRecord: true
        storageName: ""
        persistentStoragePath: ""
        cachePath: ""
        persistentPermissionsPolicy: WebEngineProfile.AskEveryTime
        isPushServiceEnabled: false
    }

    WebEngineView { id: firstView; profile: firstProfile; visible: false }
    WebEngineView { id: secondView; profile: secondProfile; visible: false }

    function admitBoth() {
        return registry.retainCleanBranchOwner(firstHost.windowBranchToken,
                                               firstHost, firstProfile, firstView)
                && registry.retainCleanBranchOwner(secondHost.windowBranchToken,
                                                   secondHost, secondProfile, secondView)
    }

    function releaseBoth() {
        return registry.releaseCleanBranchOwner(firstHost.windowBranchToken,
                                                firstHost, firstProfile, firstView)
                && registry.releaseCleanBranchOwner(secondHost.windowBranchToken,
                                                    secondHost, secondProfile, secondView)
    }
}
