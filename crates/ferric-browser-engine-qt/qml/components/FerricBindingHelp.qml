import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: bindingHelp
    signal closeRequested()
    signal refreshRequested()
    signal searchChanged(string text)
    visible: browserWindow.bindingHelpVisible
    commandText: ":bind"
    title: "Ferric Guide"
    message: ""
    keyHelp: manual.navigationHelp + " · Tab controls · Esc close"
    dismissOnEscape: !manual.handlesEscape
    dialogWidth: 1120 * scale
    dialogHeight: Math.max(minimumDialogHeight, height - 64 * scale)
    dialogBorderColor: browserWindow.borderColor
    stackingOrder: 74
    initialFocusItem: manual.searchInput
    onDismissRequested: closeRequested()
    function focusSearch() { manual.searchInput.forceActiveFocus() }
    FerricGuide {
        id: manual
        anchors.fill: parent
        browserUi: bindingHelp.browserWindow.primaryBrowserUi
        browserWindow: bindingHelp.browserWindow
        searchText: bindingHelp.browserWindow.bindingHelpSearch
        onSearchTextChanged: bindingHelp.searchChanged(searchText)
    }
}
