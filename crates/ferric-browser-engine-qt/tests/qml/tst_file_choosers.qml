import QtQuick
import QtQuick.Window
import QtQuick.Dialogs
import QtTest
import "../../qml/components"
TestCase {
    id: test
    name: "PerRequestFileChooser"
    when: windowShown
    width: 640
    height: 480
    visible: true
    Component { id: fixture; FerricFileDialogSurfaces { engineHostWindow: test.Window.window } }
    SignalSpy { id: accepted; signalName: "engineFileAccepted" }
    SignalSpy { id: rejected; signalName: "engineFileRejected" }
    SignalSpy { id: failed; signalName: "engineFileFailed" }
    SignalSpy { id: folderAccepted; signalName: "engineFolderAccepted" }
    SignalSpy { id: folderRejected; signalName: "engineFolderRejected" }
    property var surfaces: null
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot set.*exist|Cannot assign/)
        surfaces = createTemporaryObject(fixture, test)
        verify(surfaces)
        accepted.target = surfaces
        rejected.target = surfaces
        failed.target = surfaces
        folderAccepted.target = surfaces
        folderRejected.target = surfaces
        folderAccepted.clear()
        folderRejected.clear()
        accepted.clear()
        rejected.clear()
        failed.clear()
        surfaces.engineFileCurrentFolder = Qt.resolvedUrl(".")
    }
    function cleanup() {
        surfaces.closeEngineFile()
        surfaces.closeEngineFolder()
        folderAccepted.target = null
        folderRejected.target = null
        accepted.target = null
        rejected.target = null
        failed.target = null
    }
    function test_next_request_has_no_retained_file_from_previous_dialog() {
        var previous = Qt.resolvedUrl("tst_file_choosers.qml").toString()
        surfaces.engineFileDefaultFile = previous
        surfaces.openEngineFile()
        var old = surfaces.engineFileDialog
        verify(old)
        compare(old.selectedFile.toString(), previous)
        surfaces.engineFileDefaultFile = ""
        surfaces.openEngineFile()
        var fresh = surfaces.engineFileDialog
        verify(fresh && fresh !== old)
        compare(fresh.selectedFile.toString(), "")
        compare(surfaces.engineFileSelectedFiles.length, 0)
        compare(rejected.count, 0)
    }
    function test_acceptance_copies_only_this_request_selection() {
        var chosen = Qt.resolvedUrl("tst_file_choosers.qml").toString()
        surfaces.engineFileDefaultFile = chosen
        surfaces.openEngineFile()
        var dialog = surfaces.engineFileDialog
        dialog.accept()
        tryCompare(accepted, "count", 1)
        compare(surfaces.engineFileDialog, null)
        compare(surfaces.engineFileSelectedFile.toString(), chosen)
        compare(surfaces.engineFileSelectedFiles.length, 1)
        compare(surfaces.engineFileSelectedFiles[0].toString(), chosen)
        surfaces.engineFileDefaultFile = ""
        surfaces.openEngineFile()
        compare(surfaces.engineFileSelectedFile.toString(), "")
        compare(surfaces.engineFileSelectedFiles.length, 0)
    }
    function test_save_suggestion_does_not_have_to_exist() {
        surfaces.engineFileMode = FileDialog.SaveFile
        var destination = Qt.resolvedUrl("not-created-résumé + 100%.txt").toString()
        surfaces.engineFileDefaultFile = destination
        surfaces.openEngineFile()
        var dialog = surfaces.engineFileDialog
        compare(dialog.fileMode, FileDialog.SaveFile)
        compare(dialog.selectedFile.toString(), destination)
        dialog.accept()
        tryCompare(accepted, "count", 1)
        compare(surfaces.engineFileSelectedFile.toString(), destination)
    }
    function test_retired_callbacks_cannot_finish_new_request() {
        surfaces.openEngineFile()
        var old = surfaces.engineFileDialog
        surfaces.openEngineFile()
        var current = surfaces.engineFileDialog
        verify(old !== current)
        old.accepted()
        old.rejected()
        old.failed("Retired picker failure")
        compare(accepted.count, 0)
        compare(rejected.count, 0)
        compare(failed.count, 0)
        compare(surfaces.engineFileDialog, current)
        verify(current.visible)
        current.reject()
        tryCompare(rejected, "count", 1)
        compare(surfaces.engineFileDialog, null)
    }
    function test_failure_retires_request_without_reporting_cancellation() {
        surfaces.openEngineFile()
        var dialog = surfaces.engineFileDialog
        dialog.failed("Choose a file stored on this computer")
        compare(failed.count, 1)
        compare(failed.signalArguments[0][0], "Choose a file stored on this computer")
        compare(rejected.count, 0)
        compare(accepted.count, 0)
        compare(surfaces.engineFileDialog, null)
        compare(surfaces.engineFileSelectedFiles.length, 0)
        surfaces.openEngineFile()
        verify(surfaces.engineFileDialog)
    }
    function test_required_portal_failure_remains_a_failure() {
        // Offscreen has no system chooser. This checks the real native failure
        // signal and QML ownership path without contacting the desktop bus.
        surfaces.enginePortalRequired = true
        surfaces.openEngineFile()
        tryCompare(failed, "count", 1)
        compare(rejected.count, 0)
        compare(surfaces.engineFileDialog, null)
        verify(failed.signalArguments[0][0].indexOf("system file chooser") >= 0)
    }
    function test_programmatic_cancellation_suppresses_native_rejection() {
        surfaces.openEngineFile()
        verify(surfaces.engineFileDialog)
        surfaces.closeEngineFile()
        compare(surfaces.engineFileDialog, null)
        compare(rejected.count, 0)
        compare(accepted.count, 0)
    }
    function test_folder_requests_retire_previous_native_dialogs() {
        surfaces.openEngineFolder()
        var old = surfaces.engineFolderDialog
        verify(old)
        surfaces.openEngineFolder()
        var current = surfaces.engineFolderDialog
        verify(current && current !== old)
        compare(surfaces.engineFolderSelectedFolder.toString(), "")
        old.accepted()
        old.rejected()
        compare(folderAccepted.count, 0)
        compare(folderRejected.count, 0)
        compare(surfaces.engineFolderDialog, current)
        current.reject()
        tryCompare(folderRejected, "count", 1)
    }
    function test_request_mode_switch_keeps_only_one_chooser() {
        surfaces.openEngineFile()
        var oldFile = surfaces.engineFileDialog
        surfaces.openEngineFolder()
        compare(surfaces.engineFileDialog, null)
        verify(surfaces.engineFolderDialog)
        oldFile.accepted()
        compare(accepted.count, 0)
        surfaces.openEngineFile()
        compare(surfaces.engineFolderDialog, null)
        verify(surfaces.engineFileDialog)
        compare(folderRejected.count, 0)
        compare(rejected.count, 0)
    }
    function test_folder_programmatic_close_does_not_repeat_cancellation() {
        surfaces.openEngineFolder()
        surfaces.closeEngineFolder()
        compare(surfaces.engineFolderDialog, null)
        compare(folderRejected.count, 0)
    }

}
