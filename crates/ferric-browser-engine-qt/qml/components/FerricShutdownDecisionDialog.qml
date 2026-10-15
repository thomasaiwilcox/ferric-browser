import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Keyboard-first shutdown decision surface. Window-level shortcuts keep the
// prompt operable even when QtWebEngine still owns the active focus item after
// a compositor close request. The action rows remain ordinary Buttons for
// pointer and assistive-technology users.
FocusScope {
    id: dialog
    objectName: "shutdownDecisionDialog"

    required property var hostWindow
    property var themeWindow: hostWindow
    property bool promptVisible: false
    property string title: ""
    property string message: ""
    property string commandText: ":window-close"
    property string keepLabel: "Keep browser open"
    property string proceedLabel: "Continue"
    property string keepAccessibleName: keepLabel
    property string proceedAccessibleName: proceedLabel
    property color dialogBorderColor: themeWindow.warningColor
    property int dialogHeight: 220
    property int stackingOrder: 100
    property int currentIndex: 0
    property var previousFocusItem: null
    property bool decisionTaken: false
    readonly property real scale: themeWindow.chromeScale === undefined
                                  ? 1 : themeWindow.chromeScale

    signal keepRequested()
    signal proceedRequested()

    anchors.fill: parent
    z: stackingOrder
    visible: promptVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: title

    function focusChoice(index) {
        dialog.currentIndex = index === 1 ? 1 : 0
        Qt.callLater(function() {
            if (!dialog.visible) {
                return
            }
            if (dialog.currentIndex === 1) {
                proceedButton.forceActiveFocus()
            } else {
                keepButton.forceActiveFocus()
            }
        })
    }

    function focusSafeChoice() {
        focusChoice(0)
    }

    function moveChoice() {
        focusChoice(dialog.currentIndex === 0 ? 1 : 0)
    }

    function activateChoice() {
        if (dialog.currentIndex === 1) {
            requestProceed()
        } else {
            requestKeep()
        }
    }

    function requestKeep() {
        if (!dialog.visible || dialog.decisionTaken) {
            return
        }
        dialog.decisionTaken = true
        dialog.keepRequested()
    }

    function requestProceed() {
        if (!dialog.visible || dialog.decisionTaken) {
            return
        }
        dialog.decisionTaken = true
        dialog.proceedRequested()
    }

    function restorePreviousFocus() {
        var target = dialog.previousFocusItem
        dialog.previousFocusItem = null
        Qt.callLater(function() {
            if (dialog.visible) {
                return
            }
            if (target && target.forceActiveFocus
                    && target.visible !== false && target.enabled !== false) {
                target.forceActiveFocus()
            }
        })
    }

    onVisibleChanged: {
        if (visible) {
            dialog.previousFocusItem = dialog.hostWindow.activeFocusItem || null
            dialog.decisionTaken = false
            dialog.focusSafeChoice()
        } else {
            dialog.restorePreviousFocus()
        }
    }
    Component.onCompleted: if (visible) focusSafeChoice()

    Keys.onPressed: function(event) {
        var plain = !(event.modifiers & (Qt.ControlModifier
                                         | Qt.AltModifier
                                         | Qt.MetaModifier))
        var keyText = String(event.text || "").toLowerCase()
        if (event.key === Qt.Key_Escape || (plain && keyText === "n")) {
            dialog.requestKeep()
            event.accepted = true
        } else if (plain && keyText === "y") {
            dialog.requestProceed()
            event.accepted = true
        } else if (event.key === Qt.Key_Up || (plain && keyText === "k")) {
            dialog.focusChoice(0)
            event.accepted = true
        } else if (event.key === Qt.Key_Down || (plain && keyText === "j")) {
            dialog.focusChoice(1)
            event.accepted = true
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (event.modifiers & Qt.ControlModifier) {
                dialog.requestProceed()
            } else {
                dialog.activateChoice()
            }
            event.accepted = true
        }
    }

    // These shortcuts are deliberately window-scoped rather than focus-scoped:
    // a native Wayland close request can arrive while the WebEngine view still
    // owns focus. The visible prompt must nevertheless own the next decision.
    Shortcut {
        sequence: "Escape"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.requestKeep()
    }
    Shortcut {
        sequence: "N"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.requestKeep()
    }
    Shortcut {
        sequence: "Y"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.requestProceed()
    }
    Shortcut {
        sequence: "Up"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.focusChoice(0)
    }
    Shortcut {
        sequence: "K"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.focusChoice(0)
    }
    Shortcut {
        sequence: "Down"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.focusChoice(1)
    }
    Shortcut {
        sequence: "J"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.focusChoice(1)
    }
    Shortcut {
        sequence: "Return"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.activateChoice()
    }
    Shortcut {
        sequence: "Enter"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.activateChoice()
    }
    Shortcut {
        sequence: "Ctrl+Return"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.requestProceed()
    }

    Rectangle {
        anchors.fill: parent
        color: "#99000000"

        MouseArea {
            anchors.fill: parent
            // Swallow page clicks while a shutdown decision is active. Clicking
            // outside does not choose a destructive action.
            onClicked: dialog.focusChoice(dialog.currentIndex)
        }
    }

    Rectangle {
        id: panel
        objectName: "shutdownDecisionPanel"
        anchors.centerIn: parent
        width: Math.min(620 * dialog.scale, Math.max(0, dialog.width - 32 * dialog.scale))
        height: Math.min(Math.max(dialog.dialogHeight,
                                  contentLayout.implicitHeight + 32 * dialog.scale),
                         Math.max(0, dialog.height - 32 * dialog.scale))
        color: dialog.themeWindow.panelColor
        border.color: dialog.dialogBorderColor
        border.width: 2
        radius: 6 * dialog.scale

        ColumnLayout {
            id: contentLayout
            anchors.fill: parent
            anchors.margins: 16 * dialog.scale
            spacing: 10 * dialog.scale

            ScrollView {
                id: copyScroll
                objectName: "shutdownMessageScroll"
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredHeight: copyLayout.implicitHeight
                implicitHeight: copyLayout.implicitHeight
                contentWidth: availableWidth
                contentHeight: copyLayout.implicitHeight
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ScrollBar.vertical.policy: ScrollBar.AsNeeded
                ScrollBar.vertical.active: true
                ScrollBar.vertical.objectName: "shutdownMessageScrollbar"

                ColumnLayout {
                    id: copyLayout
                    width: copyScroll.availableWidth
                    height: implicitHeight
                    spacing: 10 * dialog.scale

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8 * dialog.scale
                        Label {
                            text: dialog.commandText
                            textFormat: Text.PlainText
                            color: dialog.themeWindow.accentColor
                            font.family: dialog.themeWindow.font.family
                            font.bold: true
                        }
                        Rectangle {
                            Layout.fillWidth: true
                            implicitHeight: 1
                            color: dialog.themeWindow.borderColor
                        }
                    }
                    Label {
                        objectName: "shutdownTitle"
                        Layout.fillWidth: true
                        text: dialog.title
                        textFormat: Text.PlainText
                        color: dialog.themeWindow.primaryTextColor
                        font.bold: true
                        font.pointSize: dialog.themeWindow.font.pointSize + 1
                        wrapMode: Text.Wrap
                    }
                    Label {
                        objectName: "shutdownMessage"
                        Layout.fillWidth: true
                        text: dialog.message
                        textFormat: Text.PlainText
                        color: dialog.themeWindow.secondaryTextColor
                        wrapMode: Text.Wrap
                    }
                }
            }

            Button {
                id: keepButton
                objectName: "shutdownKeepButton"
                Layout.fillWidth: true
                Layout.preferredHeight: Math.max(42 * dialog.scale,
                                                  keepContent.implicitHeight + 14 * dialog.scale)
                text: dialog.keepLabel
                hoverEnabled: true
                Accessible.name: dialog.keepAccessibleName + ". Escape or N."
                KeyNavigation.tab: proceedButton
                KeyNavigation.backtab: proceedButton
                onActiveFocusChanged: if (activeFocus) dialog.currentIndex = 0
                onHoveredChanged: if (hovered) dialog.currentIndex = 0
                onClicked: dialog.requestKeep()

                contentItem: RowLayout {
                    id: keepContent
                    spacing: 10 * dialog.scale
                    Label {
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        text: dialog.currentIndex === 0 ? "›" : " "
                        color: dialog.themeWindow.accentColor
                        font.bold: true
                    }
                    Label {
                        text: "esc / n"
                        color: dialog.themeWindow.mutedTextColor
                        font.family: dialog.themeWindow.font.family
                    }
                    Label {
                        Layout.fillWidth: true
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        text: dialog.keepLabel
                        color: dialog.currentIndex === 0
                               ? dialog.themeWindow.selectionTextColor
                               : dialog.themeWindow.primaryTextColor
                        font.bold: dialog.currentIndex === 0
                    }
                }

                background: Rectangle {
                    color: dialog.currentIndex === 0
                           ? dialog.themeWindow.selectionColor
                           : (keepButton.hovered
                              ? dialog.themeWindow.surfaceColor : "transparent")
                    border.color: dialog.currentIndex === 0
                                  ? dialog.themeWindow.accentColor
                                  : dialog.themeWindow.borderColor
                    border.width: 1
                    radius: 4 * dialog.scale
                }
            }

            Button {
                id: proceedButton
                objectName: "shutdownProceedButton"
                Layout.fillWidth: true
                Layout.preferredHeight: Math.max(42 * dialog.scale,
                                                  proceedContent.implicitHeight + 14 * dialog.scale)
                text: dialog.proceedLabel
                hoverEnabled: true
                Accessible.name: dialog.proceedAccessibleName + ". Y."
                KeyNavigation.tab: keepButton
                KeyNavigation.backtab: keepButton
                onActiveFocusChanged: if (activeFocus) dialog.currentIndex = 1
                onHoveredChanged: if (hovered) dialog.currentIndex = 1
                onClicked: dialog.requestProceed()

                contentItem: RowLayout {
                    id: proceedContent
                    spacing: 10 * dialog.scale
                    Label {
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        text: dialog.currentIndex === 1 ? "›" : " "
                        color: dialog.dialogBorderColor
                        font.bold: true
                    }
                    Label {
                        text: "y"
                        color: dialog.themeWindow.mutedTextColor
                        font.family: dialog.themeWindow.font.family
                    }
                    Label {
                        Layout.fillWidth: true
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        text: dialog.proceedLabel
                        color: dialog.currentIndex === 1
                               ? dialog.themeWindow.selectionTextColor
                               : dialog.themeWindow.primaryTextColor
                        font.bold: dialog.currentIndex === 1
                    }
                }

                background: Rectangle {
                    color: dialog.currentIndex === 1
                           ? dialog.themeWindow.selectionColor
                           : (proceedButton.hovered
                              ? dialog.themeWindow.surfaceColor : "transparent")
                    border.color: dialog.currentIndex === 1
                                  ? dialog.dialogBorderColor
                                  : dialog.themeWindow.borderColor
                    border.width: 1
                    radius: 4 * dialog.scale
                }
            }

            Label {
                Layout.fillWidth: true
                text: "j/k or ↑/↓ select  ·  enter activate"
                color: dialog.themeWindow.mutedTextColor
                font.family: dialog.themeWindow.font.family
                wrapMode: Text.WordWrap
                Accessible.name: text
            }
        }
    }
}
