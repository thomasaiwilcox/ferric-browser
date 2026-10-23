import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: bindingHelp
    objectName: "bindingHelpSurface"
    required property var browserUi
    property string searchText: browserWindow.bindingHelpSearch || ""
    signal closeRequested()
    signal refreshRequested()
    signal searchChanged(string text)
    visible: browserWindow.bindingHelpVisible
    commandText: ":binding-list"
    title: "Ferric Guide"
    message: ""
    keyHelp: manual.navigationHelp + " · Tab controls" + (manual.handlesEscape ? "" : " · Esc close")
    compactKeyHelp: manual.navigationHelp + (manual.handlesEscape ? "" : " · Esc close")
    dismissOnEscape: !manual.handlesEscape
    dialogWidth: 1120 * scale
    dialogHeight: Math.max(minimumDialogHeight, height - 64 * scale)
    dialogBorderColor: browserWindow.borderColor
    stackingOrder: 74
    initialFocusItem: manual.searchInput
    onDismissRequested: closeRequested()
    function focusSearch() { manual.focusSearch() }
    function setSearchQuery(text) {
        // User editing replaces the Guide's initial search binding. Apply an
        // explicit command query to the live reader, including after reopening.
        manual.setSearchQuery(text)
    }
    FerricGuide {
        id: manual
        anchors.fill: parent
        browserUi: bindingHelp.browserUi
        browserWindow: bindingHelp.browserWindow
        searchText: bindingHelp.searchText
        onSearchTextChanged: bindingHelp.searchChanged(searchText)
    }
}
