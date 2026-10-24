import QtQuick

// A popup uses the same page, command, input and management surfaces as every
// browser window. Only native request admission and opener identity differ.
FerricBrowserWindow {
    id: popupWindow
    width: 1024
    height: 720
    visible: profileBootstrapComplete
    // Web titles can change after load; native pages keep their owned caption.
    readonly property string currentPageTitle: windowUi.display_page_title(
        activeView && activeView.nativeUrl.length === 0 ? activeView.title : windowUi.page_title)
    title: (windowPrivacyLabel.length > 0 ? windowPrivacyLabel + " · " : "")
           + (currentPageTitle || "Ferric Browser")
           + (windowUi.context_name.length > 0
              ? " · " + (windowUi.context_label || windowUi.context_name) : "")

    property var popupRequest: null
    property var popupProfile: null
    property var popupOpenerUi: null
    readonly property var popupPermissionUi: windowUi
    property string popupJourneyToken: ""
    property bool popupPrivateProfile: false
    property bool popupEphemeralProfile: false
    property string popupEphemeralInvocationToken: ""
    property string popupProfileName: ""
    property string popupStorageBasePath: rootWindow.storageBasePath
    property string popupContextName: ""
    property string popupContextLabel: ""

    windowPopup: true
    windowPopupRequest: popupRequest
    windowPopupOpenerUi: popupOpenerUi
    windowPopupToken: popupJourneyToken
    windowSharedProfile: popupProfile
    windowProfileName: popupProfileName
    windowStorageBasePath: popupStorageBasePath
    windowProfileLabel: popupEphemeralProfile ? "Temporary" : popupProfileName
    windowPrivateProfile: popupPrivateProfile && !popupEphemeralProfile
    windowEphemeralProfile: popupEphemeralProfile
    windowEphemeralInvocationToken: popupEphemeralInvocationToken
}
