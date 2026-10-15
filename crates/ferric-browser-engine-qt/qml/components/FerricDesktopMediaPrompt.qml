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
    visible: browserWindow.desktopMediaPromptVisible && browserWindow.pendingDesktopMediaHost
             === hostWindow
    commandText: "Screen sharing"
    title: "Choose what to share"
    message: "Origin: " + browserWindow.desktopMediaOrigin
    keyHelp: "s screens  ·  w windows  ·  j/k or ↑/↓ select  ·  enter share  ·  esc cancel"
    compactKeyHelp: "esc cancel"
    dialogWidth: 680 * scale
    dialogHeight: 520 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 95
    initialFocusItem: cancelButton

    function selectCategory(windows) {
        showingWindows = windows;
        sourceList.currentIndex = sourceList.count > 0 ? 0 : -1;
        sourceList.forceActiveFocus();
    }

    function moveChoice(delta) {
        if (sourceList.count === 0) {
            return;
        }
        sourceList.currentIndex = (sourceList.currentIndex + delta + sourceList.count)
                % sourceList.count;
        sourceList.positionViewAtIndex(sourceList.currentIndex, ListView.Contain);
        sourceList.forceActiveFocus();
    }

    function activateCurrent() {
        if (sourceList.currentIndex < 0 || sourceList.currentIndex >= sourceList.count) {
            return;
        }
        if (showingWindows) {
            windowRequested(sourceList.currentIndex);
        } else {
            screenRequested(sourceList.currentIndex);
        }
    }

    onDismissRequested: cancellationRequested()

    Shortcut {
        sequence: "S"
        context: Qt.WindowShortcut
        enabled: prompt.visible && (!prompt.containsActiveFocus || prompt.focusWithin(sourceList)
                                    || cancelButton.activeFocus)
        onActivated: prompt.selectCategory(false)
    }
    Shortcut {
        sequence: "W"
        context: Qt.WindowShortcut
        enabled: prompt.visible && (!prompt.containsActiveFocus || prompt.focusWithin(sourceList)
                                    || cancelButton.activeFocus)
        onActivated: prompt.selectCategory(true)
    }
    Shortcut {
        sequence: "Up"
        context: Qt.WindowShortcut
        enabled: prompt.visible && (!prompt.containsActiveFocus || prompt.focusWithin(sourceList)
                                    || cancelButton.activeFocus)
        onActivated: prompt.moveChoice(-1)
    }
    Shortcut {
        sequence: "K"
        context: Qt.WindowShortcut
        enabled: prompt.visible && (!prompt.containsActiveFocus || prompt.focusWithin(sourceList)
                                    || cancelButton.activeFocus)
        onActivated: prompt.moveChoice(-1)
    }
    Shortcut {
        sequence: "Down"
        context: Qt.WindowShortcut
        enabled: prompt.visible && (!prompt.containsActiveFocus || prompt.focusWithin(sourceList)
                                    || cancelButton.activeFocus)
        onActivated: prompt.moveChoice(1)
    }
    Shortcut {
        sequence: "J"
        context: Qt.WindowShortcut
        enabled: prompt.visible && (!prompt.containsActiveFocus || prompt.focusWithin(sourceList)
                                    || cancelButton.activeFocus)
        onActivated: prompt.moveChoice(1)
    }
    Shortcut {
        sequence: "Return"
        context: Qt.WindowShortcut
        autoRepeat: false
        enabled: prompt.visible && prompt.focusWithin(sourceList)
        onActivated: prompt.activateCurrent()
    }
    Shortcut {
        sequence: "Enter"
        context: Qt.WindowShortcut
        autoRepeat: false
        enabled: prompt.visible && prompt.focusWithin(sourceList)
        onActivated: prompt.activateCurrent()
    }

    ColumnLayout {
        anchors.fill: parent
        FerricScrollColumn {
            Layout.fillWidth: true
            Layout.fillHeight: true
            hostWindow: prompt.hostWindow
            spacing: 8 * prompt.scale

            Label {
                Layout.fillWidth: true
                text: "Select one source for this request. Ferric Browser will not reuse a previous source."
                color: prompt.browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.fillWidth: true
                Button {
                    Layout.fillWidth: true
                    objectName: "sharingScreens"
                    text: "Screens"
                    highlighted: !prompt.showingWindows
                    onClicked: prompt.selectCategory(false)
                }
                Button {
                    Layout.fillWidth: true
                    objectName: "sharingWindows"
                    text: "Windows"
                    highlighted: prompt.showingWindows
                    onClicked: prompt.selectCategory(true)
                }
            }
            FerricManagerList {
                id: sourceList
                objectName: "sharingSources"
                hostWindow: prompt.hostWindow
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.minimumHeight: 80 * prompt.scale
                clip: true
                spacing: 6 * prompt.scale
                currentIndex: 0
                model: prompt.browserWindow.pendingDesktopMediaRequest ? (prompt.showingWindows
                                                                          ? prompt.browserWindow.pendingDesktopMediaRequest.windowsModel :
                                                                            prompt.browserWindow.pendingDesktopMediaRequest.screensModel) :
                                                                         null
                Accessible.role: Accessible.List
                Accessible.name: prompt.showingWindows ? "Shareable windows" : "Shareable screens"

                delegate: FerricCommandAction {
                    required property int index
                    required property var model
                    width: sourceList.width
                    browserWindow: prompt.browserWindow
                    keyHint: selected ? "enter" : ""
                    actionLabel: model.display !== undefined && String(model.display).length > 0
                                 ? String(model.display) : (prompt.showingWindows ? "Window " :
                                                                                    "Screen ") + (
                                       index + 1)
                    selected: index === sourceList.currentIndex && prompt.focusWithin(sourceList)
                    onSelectionRequested: sourceList.currentIndex = index
                    onClicked: {
                        sourceList.currentIndex = index;
                        prompt.activateCurrent();
                    }
                }
            }
            Label {
                Layout.fillWidth: true
                visible: sourceList.count === 0
                text: prompt.showingWindows ? "No windows are available." :
                                              "No screens are available."
                color: prompt.browserWindow.warningColor
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
            }
        }
        Button {
            id: cancelButton
            objectName: "sharingCancel"
            Layout.alignment: Qt.AlignRight
            text: "Cancel"
            onClicked: prompt.cancellationRequested()
        }
    }
}
