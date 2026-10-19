import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as BrowserScripts
import "../scripts/ChromePresentation.js" as ChromePresentation
import "../scripts/SpellcheckPresentation.js" as SpellcheckPresentation
import "../scripts/SwitcherSources.js" as SwitcherSources
import "../scripts/SwitcherResults.js" as SwitcherResults
import "../scripts/CloseRequests.js" as CloseRequests
import "../scripts/TargetSearchPresentation.js" as TargetSearchPresentation
import "../scripts/EngineRequests.js" as EngineRequests
import "../scripts/KeyboardPresentation.js" as KeyboardPresentation

// Owns window behavior through an explicit host; no inherited creation scope.
QtObject {
    required property var host
    readonly property var window: host

    function submitInteractiveCommand(text) {
        var focusedContextWindow = window.focusExistingContextWindow(text, window.browserUi)
        var succeeded = focusedContextWindow
                || window.browserUi.execute_interactive_command(text)
        if (!succeeded) {
            window.pendingInteractiveCommand = window.browserUi.command_retryable ? text : ""
            window.commandSurface.showFeedback(window.browserUi.status_text,
                                        !window.browserUi.command_retryable)
            window.commandSurface.selectAllInput()
            return
        }

        if (focusedContextWindow) window.browserUi.finish_command_entry()
        window.pendingInteractiveCommand = ""
        var commandStatus = window.browserUi.status_text
        window.profileSessionWorkspaceObject.presentCommandRequest()
        if (window.browserUi.library_kind.length > 0) {
            window.libraryPage = 0
            window.openInternalSurface()
            window.libraryManagerVisible = true
            window.refreshLibraryManager()
        }
        if (text.trim().indexOf("context-enter ") === 0) {
            window.routeContextWorkspace(window.browserUi)
        }
        window.syncTabModel()
        window.executePendingEngineAction()
        commandStatus = window.browserUi.status_text
        window.commandSurface.clearInput()
        window.showCommandNotice(commandStatus, false)
    }
}
