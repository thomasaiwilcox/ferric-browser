import QtQuick
import QtWebEngine

// Maps engine request facts to bounded browser-owned presentation values. Live
// request resolution remains at the root command boundary.
QtObject {
    id: controller

    required property var browserWindow

    function permissionTypeName(permissionType) {
        if (permissionType === WebEnginePermission.MediaAudioCapture) return "microphone"
        if (permissionType === WebEnginePermission.MediaVideoCapture) return "camera"
        if (permissionType === WebEnginePermission.MediaAudioVideoCapture) return "camera-and-microphone"
        if (permissionType === WebEnginePermission.DesktopVideoCapture
                || permissionType === WebEnginePermission.DesktopAudioVideoCapture) return "screen-capture"
        if (permissionType === WebEnginePermission.Notifications) return "notifications"
        if (permissionType === WebEnginePermission.Geolocation) return "geolocation"
        if (permissionType === WebEnginePermission.ClipboardReadWrite) return "clipboard"
        if (permissionType === WebEnginePermission.LocalFontsAccess) return "local-fonts"
        return ""
    }

    function permissionDisplayName(permissionName) {
        return {
            "camera": "use your camera",
            "microphone": "use your microphone",
            "camera-and-microphone": "use your camera and microphone",
            "screen-capture": "capture your screen or window",
            "notifications": "show notifications",
            "geolocation": "access your location",
            "clipboard": "read and write your clipboard",
            "local-fonts": "use your local fonts"
        }[permissionName] || "use an unsupported browser capability"
    }

    function permissionCanRemember(origin, permissionName) {
        if (permissionName === "screen-capture") return false
        return origin.indexOf("https://") === 0
                || origin.indexOf("http://localhost") === 0
                || origin.indexOf("http://127.0.0.1") === 0
                || origin.indexOf("http://[::1]") === 0
    }

    function permissionCanRememberForSite(origin, permissionName) {
        return !controller.browserWindow.pendingPermissionPrivate
                && controller.permissionCanRemember(origin, permissionName)
    }

    function boundedPageDialogText(value) {
        var text = String(value || "")
        text = text.replace(/[\u0000-\u001f\u007f]/g, "�")
        return text.length > 4096 ? text.slice(0, 4096) + "…" : text
    }

    function pageDialogType(request) {
        if (!request) return "unknown"
        if (request.type === JavaScriptDialogRequest.DialogTypeAlert) return "alert"
        if (request.type === JavaScriptDialogRequest.DialogTypeConfirm) return "confirm"
        if (request.type === JavaScriptDialogRequest.DialogTypePrompt) return "prompt"
        if (request.type === JavaScriptDialogRequest.DialogTypeBeforeUnload) return "beforeunload"
        return "unknown"
    }

    function pageDialogKind() {
        if (controller.browserWindow.pendingAuthenticationRequest) return "authentication"
        return controller.pageDialogType(controller.browserWindow.pendingPageDialogRequest)
    }
}
