import QtQuick
import QtQuick.Dialogs

// Native dialog widgets are centralized here. The composition root supplies
// intent handling and request resolution; this component only owns dialog
// presentation and reports acceptance or cancellation.
Item {
    id: root

    signal downloadAccepted()
    signal downloadRejected()
    signal journeyExportAccepted()
    signal journeyExportRejected()
    signal diagnosticsExportAccepted()
    signal diagnosticsExportRejected()
    signal engineFileAccepted()
    signal engineFileRejected()
    signal engineFolderAccepted()
    signal engineFolderRejected()
    signal userscriptManifestAccepted()

    readonly property url downloadSelectedFile: downloadChooser.selectedFile
    property alias downloadCurrentFile: downloadChooser.currentFile
    readonly property url journeyExportSelectedFile: journeyExportChooser.selectedFile
    property alias journeyExportCurrentFile: journeyExportChooser.currentFile
    readonly property url diagnosticsExportSelectedFile: diagnosticsExportChooser.selectedFile
    property alias diagnosticsExportCurrentFile: diagnosticsExportChooser.currentFile
    readonly property url engineFileSelectedFile: engineFileChooser.selectedFile
    readonly property var engineFileSelectedFiles: engineFileChooser.selectedFiles
    readonly property bool engineFileVisible: engineFileChooser.visible
    property alias engineFileCurrentFile: engineFileChooser.currentFile
    property alias engineFileMode: engineFileChooser.fileMode
    property alias engineFileNameFilters: engineFileChooser.nameFilters
    property alias engineFileTitle: engineFileChooser.title
    readonly property url engineFolderSelectedFolder: engineFolderChooser.selectedFolder
    readonly property bool engineFolderVisible: engineFolderChooser.visible
    readonly property url userscriptManifestSelectedFile: userscriptManifestChooser.selectedFile

    function openDownload() { downloadChooser.open() }
    function openJourneyExport() { journeyExportChooser.open() }
    function openDiagnosticsExport() { diagnosticsExportChooser.open() }
    function openEngineFile() { engineFileChooser.open() }
    function openEngineFolder() { engineFolderChooser.open() }
    function openUserscriptManifest() { userscriptManifestChooser.open() }
    function closeEngineFile() { engineFileChooser.close() }
    function closeEngineFolder() { engineFolderChooser.close() }

    FileDialog {
        id: downloadChooser
        title: "Choose download destination"
        fileMode: FileDialog.SaveFile
        nameFilters: ["All files (*)"]
        onAccepted: root.downloadAccepted()
        onRejected: root.downloadRejected()
    }

    FileDialog {
        id: journeyExportChooser
        title: "Choose journey export destination"
        fileMode: FileDialog.SaveFile
        nameFilters: ["JSON files (*.json)", "All files (*)"]
        onAccepted: root.journeyExportAccepted()
        onRejected: root.journeyExportRejected()
    }

    FileDialog {
        id: diagnosticsExportChooser
        title: "Choose diagnostics export destination"
        fileMode: FileDialog.SaveFile
        nameFilters: ["JSON files (*.json)", "All files (*)"]
        onAccepted: root.diagnosticsExportAccepted()
        onRejected: root.diagnosticsExportRejected()
    }

    FileDialog {
        id: engineFileChooser
        title: "Choose file"
        onAccepted: root.engineFileAccepted()
        onRejected: root.engineFileRejected()
    }

    FolderDialog {
        id: engineFolderChooser
        title: "Choose folder"
        onAccepted: root.engineFolderAccepted()
        onRejected: root.engineFolderRejected()
    }

    FileDialog {
        id: userscriptManifestChooser
        title: "Install userscript manifest"
        fileMode: FileDialog.OpenFile
        nameFilters: ["Userscript manifests (*.toml)", "All files (*)"]
        onAccepted: root.userscriptManifestAccepted()
    }
}
