import QtQuick
import QtTest
import QtWebEngine
import "../../qml/components" as Ferric

TestCase {
    name: "PermissionTypes"
    Ferric.FerricRequestPresentationController {
        id: controller
        browserWindow: ({})
    }
    function test_supported_data() {
        return [
            { tag: "microphone", type: WebEnginePermission.PermissionType.MediaAudioCapture, name: "microphone" },
            { tag: "camera", type: WebEnginePermission.PermissionType.MediaVideoCapture, name: "camera" },
            { tag: "combined", type: WebEnginePermission.PermissionType.MediaAudioVideoCapture, name: "camera-and-microphone" },
            { tag: "desktop", type: WebEnginePermission.PermissionType.DesktopVideoCapture, name: "screen-capture" },
            { tag: "desktop-audio", type: WebEnginePermission.PermissionType.DesktopAudioVideoCapture, name: "screen-capture" },
            { tag: "notifications", type: WebEnginePermission.PermissionType.Notifications, name: "notifications" },
            { tag: "location", type: WebEnginePermission.PermissionType.Geolocation, name: "geolocation" },
            { tag: "clipboard", type: WebEnginePermission.PermissionType.ClipboardReadWrite, name: "clipboard" },
            { tag: "fonts", type: WebEnginePermission.PermissionType.LocalFontsAccess, name: "local-fonts" },
            { tag: "unsupported", type: WebEnginePermission.PermissionType.Unsupported, name: "" },
            { tag: "mouse-lock", type: WebEnginePermission.PermissionType.MouseLock, name: "" }
        ]
    }
    function test_supported(data) {
        verify(data.type !== undefined)
        compare(controller.permissionTypeName(data.type), data.name)
    }
}
