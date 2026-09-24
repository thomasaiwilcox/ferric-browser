import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Presents available desktop-media sources. Selection and cancellation remain
// typed intents handled by the composition root.
Rectangle {
    id: prompt

    required property var browserWindow
    property var hostWindow: null
    property bool showingWindows: false

    signal screenRequested(int index)
    signal windowRequested(int index)
    signal cancellationRequested()

    width: Math.min(680, hostWindow ? hostWindow.width - 80 : 600)
    height: Math.min(520, hostWindow ? hostWindow.height - 100 : 420)
    anchors.centerIn: parent
    z: 95
    visible: browserWindow.desktopMediaPromptVisible
             && browserWindow.pendingDesktopMediaHost === hostWindow
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Screen sharing source chooser"
    color: browserWindow.panelColor
    border.color: browserWindow.warningColor
    border.width: 2
    onVisibleChanged: {
        if (visible) {
            showingWindows = false
            forceActiveFocus()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: "Choose what to share"
            color: prompt.browserWindow.primaryTextColor
            font.bold: true
            Accessible.name: "Screen sharing source chooser"
        }
        Label {
            Layout.fillWidth: true
            text: "Origin: " + prompt.browserWindow.desktopMediaOrigin
            color: prompt.browserWindow.mutedTextColor
            elide: Text.ElideMiddle
            Accessible.name: "Screen sharing requesting origin"
        }
        Label {
            Layout.fillWidth: true
            text: "Select one source for this request. Ferric Browser will not reuse a previous source."
            color: prompt.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }
        RowLayout {
            Layout.fillWidth: true
            Button {
                text: "Screens"
                checked: !prompt.showingWindows
                checkable: true
                onClicked: prompt.showingWindows = false
                Accessible.name: "Show screens"
            }
            Button {
                text: "Windows"
                checked: prompt.showingWindows
                checkable: true
                onClicked: prompt.showingWindows = true
                Accessible.name: "Show windows"
            }
            Item { Layout.fillWidth: true }
        }
        ListView {
            id: desktopMediaSourceList
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: prompt.browserWindow.pendingDesktopMediaRequest
                   ? (prompt.showingWindows
                      ? prompt.browserWindow.pendingDesktopMediaRequest.windowsModel
                      : prompt.browserWindow.pendingDesktopMediaRequest.screensModel)
                   : null
            delegate: Button {
                width: desktopMediaSourceList.width
                text: (prompt.showingWindows ? "Window " : "Screen ") + (index + 1)
                Accessible.name: text
                onClicked: {
                    if (prompt.showingWindows) {
                        prompt.windowRequested(index)
                    } else {
                        prompt.screenRequested(index)
                    }
                }
            }
        }
        Label {
            Layout.fillWidth: true
            visible: desktopMediaSourceList.count === 0
            text: prompt.showingWindows
                  ? "No windows are available." : "No screens are available."
            color: prompt.browserWindow.warningColor
            horizontalAlignment: Text.AlignHCenter
        }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                text: "Cancel"
                Accessible.name: "Cancel screen sharing source selection"
                onClicked: prompt.cancellationRequested()
            }
        }
    }

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            prompt.cancellationRequested()
            event.accepted = true
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            event.accepted = true
        }
    }
}
