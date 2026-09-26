import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Shared browser-owned modal frame. It deliberately uses window-scoped
// shortcuts because QtWebEngine can retain active focus while a prompt is
// visible. Policy and mutations remain in the owning composition root.
FocusScope {
    id: surface

    default property alias contentData: body.data

    required property var browserWindow
    property var hostWindow: browserWindow
    property string commandText: ":dialog"
    property string title: ""
    property string message: ""
    property string keyHelp: "esc close"
    property real dialogWidth: 640 * scale
    property real dialogHeight: 360 * scale
    property real minimumDialogWidth: 280 * scale
    property real minimumDialogHeight: 160 * scale
    property color dialogBorderColor: browserWindow.accentColor
    property int stackingOrder: 70
    property Item initialFocusItem: null
    property var previousFocusItem: null
    property bool restoreFocus: true
    property bool dismissOnEscape: true
    readonly property real scale: browserWindow.chromeScale === undefined
                                  ? 1 : browserWindow.chromeScale
    readonly property alias panelItem: panel
    readonly property alias bodyItem: body

    signal dismissRequested()

    anchors.fill: parent
    z: stackingOrder
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: title.length > 0 ? title : commandText

    function focusInitialControl() {
        Qt.callLater(function() {
            if (!surface.visible) {
                return
            }
            if (surface.initialFocusItem
                    && surface.initialFocusItem.visible
                    && surface.initialFocusItem.enabled) {
                surface.initialFocusItem.forceActiveFocus()
            } else {
                surface.forceActiveFocus()
            }
        })
    }

    function restorePreviousFocus() {
        var target = surface.previousFocusItem
        surface.previousFocusItem = null
        if (!surface.restoreFocus) {
            return
        }
        Qt.callLater(function() {
            if (!surface.visible && target && target.forceActiveFocus
                    && target.visible !== false && target.enabled !== false) {
                target.forceActiveFocus()
            }
        })
    }

    function prepareForDisplay() {
        previousFocusItem = hostWindow && hostWindow.activeFocusItem
                            ? hostWindow.activeFocusItem : null
        focusInitialControl()
    }

    onVisibleChanged: {
        if (visible) {
            surface.prepareForDisplay()
        } else {
            surface.restorePreviousFocus()
        }
    }
    Component.onCompleted: if (visible) prepareForDisplay()

    Shortcut {
        sequence: "Escape"
        context: Qt.WindowShortcut
        enabled: surface.visible && surface.dismissOnEscape
        onActivated: surface.dismissRequested()
    }

    Rectangle {
        anchors.fill: parent
        color: "#99000000"

        MouseArea {
            anchors.fill: parent
            // Modal background clicks never imply consent or dismissal.
            onClicked: surface.focusInitialControl()
        }
    }

    Rectangle {
        id: panel
        objectName: "ferricModalPanel"
        anchors.centerIn: parent
        width: Math.min(Math.max(surface.minimumDialogWidth,
                                 surface.dialogWidth),
                        Math.max(0, surface.width - 32 * surface.scale))
        height: Math.min(Math.max(surface.minimumDialogHeight,
                                  surface.dialogHeight),
                         Math.max(0, surface.height - 32 * surface.scale))
        color: surface.browserWindow.panelColor
        border.color: surface.dialogBorderColor
        border.width: 2
        radius: 6 * surface.scale

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16 * surface.scale
            spacing: 10 * surface.scale

            RowLayout {
                Layout.fillWidth: true
                spacing: 8 * surface.scale

                Label {
                    text: surface.commandText
                    color: surface.browserWindow.accentColor
                    font.family: surface.browserWindow.font.family
                    font.bold: true
                    Accessible.name: "Dialog command " + text
                }
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: 1
                    color: surface.browserWindow.borderColor
                }
            }

            Label {
                Layout.fillWidth: true
                visible: surface.title.length > 0
                text: surface.title
                color: surface.browserWindow.primaryTextColor
                font.bold: true
                font.pointSize: surface.browserWindow.font.pointSize + 1
                wrapMode: Text.WordWrap
            }

            Label {
                Layout.fillWidth: true
                visible: surface.message.length > 0
                text: surface.message
                color: surface.browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
            }

            Item {
                id: body
                Layout.fillWidth: true
                Layout.fillHeight: true
            }

            Label {
                Layout.fillWidth: true
                visible: surface.keyHelp.length > 0
                text: surface.keyHelp
                color: surface.browserWindow.mutedTextColor
                font.family: surface.browserWindow.font.family
                wrapMode: Text.WordWrap
                Accessible.name: "Keyboard controls: " + text
            }
        }
    }
}
