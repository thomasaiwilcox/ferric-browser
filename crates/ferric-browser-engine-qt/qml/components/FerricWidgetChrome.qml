import QtQuick

Item {
    id: chrome

    property var browserUi: null
    readonly property var runtimeController: FerricRuntimeController { host: chrome }

    function syncTabModel() {}
    function dispatchPendingEngineRequest() {
        runtimeController.executePendingEngineAction()
    }
    readonly property string statusText: browserUi ? browserUi.status_text : ""
    readonly property QtObject presentationWindow: QtObject {
        readonly property color accentColor: "#597a9f"
        readonly property color borderColor: "#a7adb5"
        readonly property real chromeOpacity: 1.0
        readonly property color errorColor: "#a32626"
        readonly property color modeInsertColor: "#315f45"
        readonly property color mutedTextColor: "#646b73"
        readonly property color panelColor: "#f4f4f4"
        readonly property color primaryTextColor: "#202327"
        readonly property color secondaryTextColor: "#42484f"
        readonly property real statusBarHeight: 44
        readonly property color successColor: "#276746"
        readonly property color surfaceColor: "#ffffff"
        readonly property color warningColor: "#8b5a00"

        function addressPresentation(value, maxCharacters) {
            return String(value || "").slice(0, maxCharacters)
        }

        function contrastText(color) {
            const luminance = color.r * 0.2126 + color.g * 0.7152 + color.b * 0.0722
            return luminance > 0.52 ? "#202327" : "#ffffff"
        }
    }

    implicitHeight: 44
    implicitWidth: 640

    function attachPageHost(tabId, hostToken, pageOwnerToken) {
        if (!chrome.browserUi) {
            return false
        }
        return chrome.browserUi.attach_page_host(
            tabId,
            chrome.browserUi.page_host_profile_id(),
            hostToken,
            pageOwnerToken)
    }

    function detachPageHost(tabId, hostToken, pageOwnerToken) {
        if (!chrome.browserUi) {
            return false
        }
        return chrome.browserUi.detach_page_host(
            tabId,
            chrome.browserUi.page_host_profile_id(),
            hostToken,
            pageOwnerToken)
    }

    FerricStatusBar {
        id: statusBar
        objectName: "widgetChromeStatusBar"
        anchors.fill: parent
        browserWindow: chrome.presentationWindow
        statusVisible: true
        mode: chrome.browserUi ? chrome.browserUi.mode : "normal"
        displayUrl: ""
        statusText: chrome.statusText
        contextName: "Default"
        contextColor: "#597a9f"
        profileName: "Default"
        temporaryProfile: false
        ephemeralProfile: false
        blockingSiteCount: 0
        activeTabIndex: 0
        tabCount: 1
        engineUpdateNotice: ""
        macroStatusText: ""
        activeView: null
        accessibleDetails: "Widget host"
    }
}
