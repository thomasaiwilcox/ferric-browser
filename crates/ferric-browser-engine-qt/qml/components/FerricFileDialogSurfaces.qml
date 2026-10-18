import QtCore
import QtQuick
import QtQuick.Window
import QtQuick.Dialogs
import io.github.ferricbrowser 1.0

// Native dialog widgets are centralized here. The composition root supplies
// intent handling and request resolution; this component only owns dialog
// presentation and reports acceptance or cancellation.
Item {
    id: root
    objectName: "browserFileDialogs"

    property var diagnosticsHostWindow: null
    property var engineHostWindow: null
    property bool portalRequired: false
    property bool enginePortalRequired: portalRequired
    property bool diagnosticsPortalRequired: portalRequired

    signal downloadAccepted()
    signal downloadRejected()
    signal downloadFailed(string message)
    signal journeyExportAccepted()
    signal journeyExportRejected()
    signal journeyExportFailed(string message)
    signal diagnosticsExportAccepted()
    signal diagnosticsExportRejected()
    signal diagnosticsExportFailed(string message)
    signal engineFileAccepted()
    signal engineFileRejected()
    signal engineFileFailed(string message)
    signal engineChooserOpened(string backend)
    signal engineFolderAccepted()
    signal engineFolderRejected()
    signal engineFolderFailed(string message)
    signal userscriptManifestAccepted()
    signal userscriptManifestFailed(string message)

    readonly property url downloadSelectedFile: downloadChooser.selectedFile
    property alias downloadCurrentFile: downloadChooser.currentFile
    readonly property url journeyExportSelectedFile: journeyExportChooser.selectedFile
    property alias journeyExportCurrentFile: journeyExportChooser.currentFile
    readonly property url diagnosticsExportSelectedFile: diagnosticsExportChooser.selectedFile
    property alias diagnosticsExportCurrentFile: diagnosticsExportChooser.currentFile
    property var engineFileDialog: null
    property url engineFileDefaultFile: ""
    property url engineFileCurrentFolder: StandardPaths.writableLocation(StandardPaths.HomeLocation)
    property int engineFileMode: FileDialog.OpenFile
    property var engineFileNameFilters: ["All files (*)"]
    property int engineFileOptions: 0
    property string engineFileTitle: "Choose file"
    property url engineChosenFile: ""
    property var engineChosenFiles: []
    readonly property url engineFileSelectedFile: engineChosenFile
    readonly property var engineFileSelectedFiles: engineChosenFiles
    readonly property bool engineFileVisible: !!engineFileDialog && engineFileDialog.visible
    property var engineFolderDialog: null
    property url engineFolderCurrentFolder: StandardPaths.writableLocation(StandardPaths.HomeLocation)
    property url engineChosenFolder: ""
    readonly property url engineFolderSelectedFolder: engineChosenFolder
    readonly property bool engineFolderVisible: !!engineFolderDialog && engineFolderDialog.visible
    readonly property url userscriptManifestSelectedFile: userscriptManifestChooser.selectedFile

    function openDownload() { downloadChooser.open() }
    function openJourneyExport() { journeyExportChooser.open() }
    function openDiagnosticsExport() { diagnosticsExportChooser.open() }
    function closeDiagnosticsExport() { diagnosticsExportChooser.close() }
    function retireEngineFileDialog() {
        var dialog = engineFileDialog
        engineFileDialog = null
        if (!dialog) return
        if (String(dialog.currentFolder).length > 0) engineFileCurrentFolder = dialog.currentFolder
        dialog.close()
        dialog.destroy()
    }
    function openEngineFile() {
        retireEngineFolderDialog()
        retireEngineFileDialog()
        engineChosenFile = ""
        engineChosenFiles = []
        var intent = { title: engineFileTitle, fileMode: engineFileMode,
            nameFilters: engineFileNameFilters, options: engineFileOptions,
            portalRequired: enginePortalRequired }
        if (engineHostWindow) intent.parentWindow = engineHostWindow
        if (String(engineFileCurrentFolder).length > 0) intent.currentFolder = engineFileCurrentFolder
        // An empty default is an absence of selection, never a request to
        // reset a Qt dialog that might still retain a previous file.
        var dialog = engineFileFactory.createObject(root, intent)
        if (!dialog) { engineFileFailed("The file chooser could not open. Try again."); return }
        engineFileDialog = dialog
        if (String(engineFileDefaultFile).length > 0) dialog.selectedFile = engineFileDefaultFile
        dialog.open()
    }
    function finishEngineFileDialog(dialog, accepted, message) {
        if (engineFileDialog !== dialog) return
        if (String(dialog.currentFolder).length > 0) engineFileCurrentFolder = dialog.currentFolder
        engineChosenFile = accepted ? dialog.selectedFile : ""
        engineChosenFiles = accepted ? Array.prototype.slice.call(dialog.selectedFiles) : []
        engineFileDialog = null
        dialog.destroy()
        if (message) engineFileFailed(message)
        else if (accepted) engineFileAccepted()
        else engineFileRejected()
    }
    function retireEngineFolderDialog() {
        var dialog = engineFolderDialog
        engineFolderDialog = null
        if (!dialog) return
        if (String(dialog.currentFolder).length > 0) engineFolderCurrentFolder = dialog.currentFolder
        dialog.close()
        dialog.destroy()
    }
    function openEngineFolder() {
        retireEngineFileDialog()
        retireEngineFolderDialog()
        engineChosenFolder = ""
        var intent = { currentFolder: engineFolderCurrentFolder, portalRequired: enginePortalRequired }
        if (engineHostWindow) intent.parentWindow = engineHostWindow
        var dialog = engineFolderFactory.createObject(root, intent)
        if (!dialog) { engineFolderFailed("The folder chooser could not open. Try again."); return }
        engineFolderDialog = dialog
        dialog.open()
    }
    function finishEngineFolderDialog(dialog, accepted, message) {
        if (engineFolderDialog !== dialog) return
        if (String(dialog.currentFolder).length > 0) engineFolderCurrentFolder = dialog.currentFolder
        engineChosenFolder = accepted ? dialog.selectedFolder : ""
        engineFolderDialog = null
        dialog.destroy()
        if (message) engineFolderFailed(message)
        else if (accepted) engineFolderAccepted()
        else engineFolderRejected()
    }
    function openUserscriptManifest() { userscriptManifestChooser.open() }
    function closeEngineFile() { retireEngineFileDialog() }
    function closeEngineFolder() { retireEngineFolderDialog() }

    FerricFileChooserProgress { chooser: downloadChooser; ownerWindow: root.engineHostWindow }
    FerricFileChooserProgress { chooser: journeyExportChooser; ownerWindow: root.engineHostWindow }
    FerricFileChooserProgress { chooser: diagnosticsExportChooser; ownerWindow: root.diagnosticsHostWindow }
    FerricFileChooserProgress { chooser: root.engineFileDialog; ownerWindow: root.engineHostWindow }
    FerricFileChooserProgress { chooser: root.engineFolderDialog; ownerWindow: root.engineHostWindow }
    FerricFileChooserProgress { chooser: userscriptManifestChooser; ownerWindow: root.engineHostWindow }

    NativeFileChooser {
        id: downloadChooser
        parentWindow: root.engineHostWindow
        portalRequired: root.portalRequired
        title: "Choose download destination"
        fileMode: FileDialog.SaveFile
        nameFilters: ["All files (*)"]
        onAccepted: root.downloadAccepted()
        onRejected: root.downloadRejected()
        onFailed: function(message) { root.downloadFailed(message) }
    }

    NativeFileChooser {
        id: journeyExportChooser
        parentWindow: root.engineHostWindow
        portalRequired: root.portalRequired
        title: "Choose journey export destination"
        fileMode: FileDialog.SaveFile
        nameFilters: ["JSON files (*.json)", "All files (*)"]
        onAccepted: root.journeyExportAccepted()
        onRejected: root.journeyExportRejected()
        onFailed: function(message) { root.journeyExportFailed(message) }
    }

    NativeFileChooser {
        id: diagnosticsExportChooser
        parentWindow: root.diagnosticsHostWindow
        portalRequired: root.diagnosticsPortalRequired
        objectName: "diagnosticsExportChooser"
        title: "Choose diagnostics export destination"
        fileMode: FileDialog.SaveFile
        nameFilters: ["JSON files (*.json)", "All files (*)"]
        onAccepted: root.diagnosticsExportAccepted()
        onRejected: root.diagnosticsExportRejected()
        onFailed: function(message) { root.diagnosticsExportFailed(message) }
    }

    property Component engineFileFactory: Component {
        NativeFileChooser {
            id: engineFileChooser
            objectName: "engineFileChooser"
            onBackendChanged: if (backend.length > 0) root.engineChooserOpened(backend)
            onAccepted: root.finishEngineFileDialog(engineFileChooser, true)
            onRejected: root.finishEngineFileDialog(engineFileChooser, false)
            onFailed: function(message) { root.finishEngineFileDialog(engineFileChooser, false, message) }
        }
    }

    property Component engineFolderFactory: Component {
        NativeFileChooser {
            id: engineFolderChooser
            fileMode: NativeFileChooser.Folder
            objectName: "engineFolderChooser"
            title: "Choose folder"
            onBackendChanged: if (backend.length > 0) root.engineChooserOpened(backend)
            onAccepted: root.finishEngineFolderDialog(engineFolderChooser, true)
            onRejected: root.finishEngineFolderDialog(engineFolderChooser, false)
            onFailed: function(message) { root.finishEngineFolderDialog(engineFolderChooser, false, message) }
        }
    }

    NativeFileChooser {
        id: userscriptManifestChooser
        parentWindow: root.engineHostWindow
        portalRequired: root.portalRequired
        title: "Install userscript manifest"
        fileMode: FileDialog.OpenFile
        nameFilters: ["Userscript manifests (*.toml)", "All files (*)"]
        onAccepted: root.userscriptManifestAccepted()
        onFailed: function(message) { root.userscriptManifestFailed(message) }
    }
}
