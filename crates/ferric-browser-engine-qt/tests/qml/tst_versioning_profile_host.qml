import QtQuick
import QtTest
import "../../qml/components"

TestCase {
    name: "VersioningProfileHost"
    when: windowShown

    QtObject {
        id: testWindow
        property bool temporaryProfile: true
        property string profileName: "synthetic"
        property string storageBasePath: ""
    }
    Component { id: hostComponent; FerricVersioningProfileHost { } }

    function test_shared_profile_skips_prototype_and_preserves_exact_instance() {
        var source = createTemporaryObject(hostComponent, this, { browserWindow: testWindow })
        verify(source !== null)
        compare(source.status, Loader.Ready)
        var profile = source.instance()
        verify(profile !== null)
        verify(profile.offTheRecord)

        var destination = createTemporaryObject(hostComponent, this, {
            browserWindow: testWindow,
            sharedProfile: profile
        })
        verify(destination !== null)
        verify(!destination.active)
        compare(destination.item, null)
        compare(destination.instance(), profile)
        compare(source.instance(), profile)
        verify(source.active)
    }
}
