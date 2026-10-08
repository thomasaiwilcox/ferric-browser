import QtQuick
import QtTest
import QtWebEngine 6.8
import "../../qml/components" as FerricComponents

TestCase {
    id: root
    name: "RequestPresentationController"
    when: windowShown

    property var browserWindowStub: ({
        pendingPermissionPrivate: false,
        pendingPageDialogRequest: null,
        pendingAuthenticationRequest: null
    })

    Component {
        id: controllerComponent
        FerricComponents.FerricRequestPresentationController {
            browserWindow: root.browserWindowStub
        }
    }

    function test_permission_types_use_scoped_qt_enums() {
        var controller = createTemporaryObject(controllerComponent, root)
        verify(controller)

        compare(controller.permissionTypeName(
                    WebEnginePermission.PermissionType.MediaAudioCapture), "microphone")
        compare(controller.permissionTypeName(
                    WebEnginePermission.PermissionType.MediaVideoCapture), "camera")
        compare(controller.permissionTypeName(
                    WebEnginePermission.PermissionType.MediaAudioVideoCapture),
                "camera-and-microphone")
        compare(controller.permissionTypeName(
                    WebEnginePermission.PermissionType.DesktopVideoCapture), "screen-capture")
        compare(controller.permissionTypeName(
                    WebEnginePermission.PermissionType.DesktopAudioVideoCapture), "screen-capture")
        compare(controller.permissionTypeName(
                    WebEnginePermission.PermissionType.Notifications), "notifications")
        compare(controller.permissionTypeName(
                    WebEnginePermission.PermissionType.Geolocation), "geolocation")
        compare(controller.permissionTypeName(
                    WebEnginePermission.PermissionType.ClipboardReadWrite), "clipboard")
        compare(controller.permissionTypeName(
                    WebEnginePermission.PermissionType.LocalFontsAccess), "local-fonts")
        compare(controller.permissionTypeName(
                    WebEnginePermission.PermissionType.Unsupported), "")
    }
}
