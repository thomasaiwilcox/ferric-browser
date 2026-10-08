import QtQuick
import QtWebEngine

// Never construct a second persistent profile for an already-open disk path.
// A same-profile restore uses the exact existing native profile instead.
Loader {
    id: profileHost
    required property var browserWindow
    property var sharedProfile: null
    active: !sharedProfile
    asynchronous: false

    function instance() {
        return sharedProfile || (item ? item.instance() : null)
    }

    sourceComponent: Component {
        WebEngineProfilePrototype {
            storageName: profileHost.browserWindow.temporaryProfile
                         ? "" : "ferric-browser-" + profileHost.browserWindow.profileName
            persistentStoragePath: profileHost.browserWindow.temporaryProfile
                    || profileHost.browserWindow.storageBasePath.length === 0
                    ? "" : profileHost.browserWindow.storageBasePath
                           + "/webengine/" + profileHost.browserWindow.profileName
            cachePath: profileHost.browserWindow.temporaryProfile
                    || profileHost.browserWindow.storageBasePath.length === 0
                    ? "" : profileHost.browserWindow.storageBasePath
                           + "/webengine-cache/" + profileHost.browserWindow.profileName
            persistentPermissionsPolicy: WebEngineProfile.AskEveryTime
        }
    }
}
