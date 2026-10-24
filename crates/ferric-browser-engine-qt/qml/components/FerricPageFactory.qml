import QtQuick

QtObject {
    id: pageFactory
    required property var rootWindow
    required property var primaryUi
    required property var addressField
    required property var popupFactory
    property Component viewComponent: Component {
        FerricPageView {
            factory: pageFactory
            viewUi: pageFactory.primaryUi
            viewTabs: pageFactory.rootWindow.tabs
            viewProfile: pageFactory.rootWindow.browserProfile
            viewInterceptor: pageFactory.rootWindow.primaryRequestInterceptor
            viewTransientProfile: pageFactory.rootWindow.temporaryProfile
        }
    }
}
