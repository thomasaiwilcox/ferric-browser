import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Source selection remains an intent handled by the composition root.
FerricModalSurface {
    id: prompt

    property bool showingWindows: false

    signal screenRequested(int index)
    signal windowRequested(int index)
    signal cancellationRequested()

    visible: browserWindow.desktopMediaPromptVisible
             && browserWindow.pendingDesktopMediaHost === hostWindow
    commandText: ":share"
    title: "Choose what to share"
    message: "Origin: " + browserWindow.desktopMediaOrigin
    keyHelp: "s screens  ·  w windows  ·  j/k or ↑/↓ select  ·  enter share  ·  esc cancel"
    dialogWidth: 680 * scale
    dialogHeight: 520 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 95
    initialFocusItem: sourceList

    function selectCategory(windows) {
        showingWindows = windows
        sourceList.currentIndex = sourceList.count > 0 ? 0 : -1
    }

    function moveChoice(delta) {
        if (sourceList.count === 0) {
            return
        }
        sourceList.currentIndex = (sourceList.currentIndex + delta
                                   + sourceList.count) % sourceList.count
        sourceList.positionViewAtIndex(sourceList.currentIndex, ListView.Contain)
    }

    function activateCurrent() {
        if (sourceList.currentIndex < 0 || sourceList.currentIndex >= sourceList.count) {
            return
        }
        if (showingWindows) {
            windowRequested(sourceList.currentIndex)
        } else {
            screenRequested(sourceList.currentIndex)
        }
    }

    onDismissRequested: cancellationRequested()

    Shortcut { sequence: "S"; context: Qt.WindowShortcut; enabled: prompt.visible; onActivated: prompt.selectCategory(false) }
    Shortcut { sequence: "W"; context: Qt.WindowShortcut; enabled: prompt.visible; onActivated: prompt.selectCategory(true) }
    Shortcut { sequence: "Up"; context: Qt.WindowShortcut; enabled: prompt.visible; onActivated: prompt.moveChoice(-1) }
    Shortcut { sequence: "K"; context: Qt.WindowShortcut; enabled: prompt.visible; onActivated: prompt.moveChoice(-1) }
    Shortcut { sequence: "Down"; context: Qt.WindowShortcut; enabled: prompt.visible; onActivated: prompt.moveChoice(1) }
    Shortcut { sequence: "J"; context: Qt.WindowShortcut; enabled: prompt.visible; onActivated: prompt.moveChoice(1) }
    Shortcut { sequence: "Return"; context: Qt.WindowShortcut; enabled: prompt.visible; onActivated: prompt.activateCurrent() }
    Shortcut { sequence: "Enter"; context: Qt.WindowShortcut; enabled: prompt.visible; onActivated: prompt.activateCurrent() }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * prompt.scale

        Label {
            Layout.fillWidth: true
            text: "Select one source for this request. Ferric Browser will not reuse a previous source."
            color: prompt.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }
        RowLayout {
            Layout.fillWidth: true
            FerricCommandAction {
                Layout.fillWidth: true
                browserWindow: prompt.browserWindow
                keyHint: "s"
                actionLabel: "Screens"
                selected: !prompt.showingWindows
                onClicked: prompt.selectCategory(false)
            }
            FerricCommandAction {
                Layout.fillWidth: true
                browserWindow: prompt.browserWindow
                keyHint: "w"
                actionLabel: "Windows"
                selected: prompt.showingWindows
                onClicked: prompt.selectCategory(true)
            }
        }
        ListView {
            id: sourceList
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6 * prompt.scale
            currentIndex: 0
            model: prompt.browserWindow.pendingDesktopMediaRequest
                   ? (prompt.showingWindows
                      ? prompt.browserWindow.pendingDesktopMediaRequest.windowsModel
                      : prompt.browserWindow.pendingDesktopMediaRequest.screensModel)
                   : null
            Accessible.role: Accessible.List
            Accessible.name: prompt.showingWindows ? "Shareable windows" : "Shareable screens"

            delegate: FerricCommandAction {
                required property int index
                width: sourceList.width
                browserWindow: prompt.browserWindow
                keyHint: index === sourceList.currentIndex ? "enter" : ""
                actionLabel: (prompt.showingWindows ? "Window " : "Screen ") + (index + 1)
                selected: index === sourceList.currentIndex
                onSelectionRequested: sourceList.currentIndex = index
                onClicked: {
                    sourceList.currentIndex = index
                    prompt.activateCurrent()
                }
            }
        }
        Label {
            Layout.fillWidth: true
            visible: sourceList.count === 0
            text: prompt.showingWindows
                  ? "No windows are available." : "No screens are available."
            color: prompt.browserWindow.warningColor
            horizontalAlignment: Text.AlignHCenter
        }
    }
}
