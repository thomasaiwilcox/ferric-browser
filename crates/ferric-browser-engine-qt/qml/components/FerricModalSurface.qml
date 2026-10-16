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
    property string commandText: "Ferric"
    property string title: ""
    property string message: ""
    property string keyHelp: "esc close"
    property string compactKeyHelp: "esc close"
    property real dialogWidth: 640 * scale
    property real dialogHeight: 360 * scale
    property real minimumDialogWidth: 280 * scale
    property real minimumDialogHeight: 160 * scale
    property real minimumBodyHeight: 100 * scale
    property color dialogBorderColor: browserWindow.accentColor
    property int stackingOrder: 70
    property Item initialFocusItem: null
    property var previousFocusItem: null
    property bool restoreFocus: true
    property bool dismissOnEscape: true
    property bool displayPrepared: false
    readonly property bool suspended: !!hostWindow && (
        hostWindow.windowCloseConfirmationVisible === true
        || hostWindow.windowShutdownPromptVisible === true
        || hostWindow.windowShutdownPagePromptVisible === true
        || hostWindow.windowShutdownStoragePromptVisible === true
        || hostWindow.shutdownConfirmationVisible === true
        || hostWindow.shutdownPromptVisible === true
        || hostWindow.shutdownPagePromptVisible === true
        || hostWindow.applicationShutdownForcePromptVisible === true)
    readonly property var inputScope: hostWindow && hostWindow.modalInputScope ? hostWindow.modalInputScope : null
    property var registeredInputScope: null
    readonly property bool windowAvailable: !!hostWindow && hostWindow.runtimeReleased !== true
                                           && hostWindow.runtimeDisposing !== true
                                           && (!inputScope || inputScope.acceptingSurfaces === true)
    readonly property bool wantsInput: visible && enabled && !suspended && windowAvailable
    readonly property bool interactive: wantsInput && (!inputScope || inputScope.activeSurface === surface)
    function syncInputScope() {
        if (registeredInputScope && (registeredInputScope !== inputScope || !wantsInput)) {
            if (registeredInputScope.acceptingSurfaces === true) registeredInputScope.unregisterSurface(surface)
            registeredInputScope = null
        }
        if (wantsInput && displayPrepared && inputScope && registeredInputScope !== inputScope) {
            registeredInputScope = inputScope
            inputScope.registerSurface(surface)
        }
    }
    onInputScopeChanged: syncInputScope()
    onWantsInputChanged: syncInputScope()
    onInteractiveChanged: {
        if (interactive && displayPrepared) ownershipFocusTimer.restart()
        else {
            initialFocusTimer.stop()
            ownershipFocusTimer.stop()
        }
    }
    Component.onDestruction: if (registeredInputScope && registeredInputScope.acceptingSurfaces === true) registeredInputScope.unregisterSurface(surface)
    // Text already uses the configured font. Limit extra spacing by the
    // available window, so large fonts retain room for context and controls.
    property real maximumSpacingScale: Math.max(1, Math.min(width / 800, height / 650))
    readonly property real scale: Math.min(maximumSpacingScale,
        browserWindow.chromeScale === undefined ? 1 : browserWindow.chromeScale)
    readonly property alias panelItem: panel
    readonly property alias bodyItem: body
    readonly property alias headerItem: headerScroll
    readonly property bool compactHelp: width < 600 * scale
                                        || keyboardHelpMetrics.width > frame.width
    TextMetrics {
        id: keyboardHelpMetrics
        font: surface.browserWindow.font
        text: surface.keyHelp
    }
    readonly property bool containsActiveFocus: {
        var item = hostWindow ? hostWindow.activeFocusItem : null;
        while (item && item !== surface)
            item = item.parent;
        return item === surface;
    }

    signal dismissRequested()
    signal opening()
    function focusWithin(control) {
        var item = hostWindow ? hostWindow.activeFocusItem : null;
        while (item && item !== control)
            item = item.parent;
        return item === control;
    }

    anchors.fill: parent
    z: stackingOrder
    // Capture the previous focus before requesting our own. A declarative
    // focus: visible steals it before prepareForDisplay can remember it.
    focus: false
    Accessible.role: Accessible.Dialog
    Accessible.name: title.length > 0 ? title : commandText

    function focusInitialControl() {
        if (!surface.interactive) return;
        // Take ownership now, before another prompt can capture the page as
        // its return target. Repeat after layout for newly visible controls.
        applyInitialFocus();
        initialFocusTimer.restart();
    }

    function applyInitialFocus() {
        if (!surface.interactive)
            return;
        if (surface.initialFocusItem && surface.initialFocusItem.visible
                && surface.initialFocusItem.enabled)
            surface.initialFocusItem.forceActiveFocus();
        else
            surface.forceActiveFocus();
    }

    function restorePreviousFocus() {
        restoreFocusTarget = surface.previousFocusItem;
        surface.previousFocusItem = null;
        if (!surface.restoreFocus || !surface.browserWindow
                || surface.browserWindow.runtimeDisposing === true
                || (surface.hostWindow && surface.hostWindow.runtimeReleased === true)) {
            return;
        }
        restoreFocusTimer.restart();
    }

    property var restoreFocusTarget: null
    Timer {
        id: ownershipFocusTimer
        interval: 0
        // A dismissed prompt may restore an edited field in this surface.
        // Reclaim focus only when it has not already returned to our controls.
        onTriggered: if (surface.interactive && !surface.containsActiveFocus)
                         surface.applyInitialFocus()
    }
    Timer {
        id: initialFocusTimer
        interval: 0
        onTriggered: surface.applyInitialFocus()
    }
    Timer {
        id: restoreFocusTimer
        interval: 0
        onTriggered: {
            var target = surface.restoreFocusTarget;
            surface.restoreFocusTarget = null;
            if (!surface.visible && surface.windowAvailable
                    && (!surface.inputScope || surface.inputScope.allowsFocus(target))
                    && target && target.forceActiveFocus && target.visible !== false
                    && target.enabled !== false) {
                target.forceActiveFocus();
            }
        }
    }

    function prepareForDisplay() {
        if (!displayPrepared) {
            previousFocusItem = hostWindow && hostWindow.activeFocusItem ? hostWindow.activeFocusItem : null;
            displayPrepared = true;
            opening();
        }
        focusInitialControl();
    }

    onVisibleChanged: {
        if (shortcutsButton)
            shortcutsButton.checked = false;
        if (visible) {
            surface.prepareForDisplay();
            surface.syncInputScope();
            surface.focusInitialControl();
        } else {
            surface.syncInputScope();
            displayPrepared = false;
            surface.restorePreviousFocus();
        }
    }
    Component.onCompleted: if (visible) {
        prepareForDisplay()
        syncInputScope()
        focusInitialControl()
    }

    Keys.onPressed: function (event) {
        if (surface.interactive && surface.dismissOnEscape && event.key === Qt.Key_Escape) {
            event.accepted = true;
            surface.dismissRequested();
        } else if (surface.interactive && event.modifiers === Qt.NoModifier && (event.key
                                                                            === Qt.Key_Return
                                                                            || event.key
                                                                            === Qt.Key_Enter)) {
            var control = surface.hostWindow ? surface.hostWindow.activeFocusItem : null;
            // Use AbstractButton's public activation method so checkboxes,
            // actions and disabled controls retain their native semantics.
            if (control && typeof control.click === "function") {
                event.accepted = true;
                if (!event.isAutoRepeat)
                    control.click();
            }
        }
    }

    Shortcut {
        sequence: "Escape"
        context: Qt.WindowShortcut
        enabled: surface.interactive && surface.dismissOnEscape
        autoRepeat: false
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
        width: Math.min(Math.max(surface.minimumDialogWidth, surface.dialogWidth),
                        Math.max(0, surface.width - 32 * surface.scale))
        height: Math.min(Math.max(surface.minimumDialogHeight, surface.dialogHeight),
                         Math.max(0, surface.height - 32 * surface.scale))
        color: surface.browserWindow.panelColor
        border.color: surface.dialogBorderColor
        border.width: 2
        radius: 6 * surface.scale

        ColumnLayout {
            id: frame
            anchors.fill: parent
            anchors.margins: 16 * surface.scale
            spacing: 10 * surface.scale

            RowLayout {
                id: contextRow
                Layout.fillWidth: true
                spacing: 8 * surface.scale

                Label {
                    objectName: "dialogContextLabel"
                    textFormat: Text.PlainText
                    text: surface.commandText
                    Layout.maximumWidth: frame.width
                    elide: Text.ElideRight
                    color: surface.browserWindow.accentColor
                    font.family: surface.browserWindow.font.family
                    font.bold: true
                    Accessible.name: (text.indexOf(":") === 0 ? "Dialog command " :
                                                                "Browser dialog ") + text
                }
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: 1
                    color: surface.browserWindow.borderColor
                }
            }

            FerricScrollColumn {
                id: headerScroll
                objectName: "dialogHeaderScroll"
                Layout.fillWidth: true
                Layout.minimumHeight: 0
                Layout.preferredHeight: Math.min(contentLayout.implicitHeight, Math.max(0,
                                                                                        frame.height
                                                                                        - contextRow.implicitHeight
                                                                                        - (keyHelpRow.visible
                                                                                           ? keyHelpRow.implicitHeight :
                                                                                             0) - frame.spacing
                                                                                        * (keyHelpRow.visible
                                                                                           ? 3 : 2)
                                                                                        - surface.minimumBodyHeight),
                                                 frame.height * 0.45)
                Layout.maximumHeight: Layout.preferredHeight
                visible: surface.title.length > 0 || surface.message.length > 0 || (
                             surface.compactHelp && shortcutsButton.checked)
                hostWindow: surface.hostWindow
                spacing: 8 * surface.scale
                onContentHeightChanged: if (shortcutsButton.checked) shortcutRevealTimer.restart()
                Label {
                    objectName: "dialogTitleLabel"
                    Layout.fillWidth: true
                    textFormat: Text.PlainText
                    visible: surface.title.length > 0
                    text: surface.title
                    color: surface.browserWindow.primaryTextColor
                    font.bold: true
                    font.pointSize: surface.browserWindow.font.pointSize + 1
                    wrapMode: Text.Wrap
                }
                TextArea {
                    objectName: "dialogMessageLabel"
                    Layout.fillWidth: true
                    visible: surface.message.length > 0
                    textFormat: TextEdit.PlainText
                    text: surface.message
                    readOnly: true
                    selectByMouse: true
                    selectByKeyboard: true
                    activeFocusOnTab: true
                    padding: 0
                    background: null
                    color: surface.browserWindow.secondaryTextColor
                    wrapMode: TextEdit.Wrap
                    Accessible.name: "Dialog message"
                }
                Label {
                    Layout.fillWidth: true
                    objectName: "dialogKeyboardInstructions"
                    visible: shortcutsButton.checked && surface.compactHelp
                    text: surface.keyHelp
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: surface.browserWindow.mutedTextColor
                    Accessible.name: "Keyboard controls: " + text
                }
            }

            Item {
                id: body
                Layout.fillWidth: true
                Layout.fillHeight: true
            }

            RowLayout {
                id: keyHelpRow
                Layout.fillWidth: true
                visible: surface.keyHelp.length > 0
                Label {
                    Layout.fillWidth: true
                    textFormat: Text.PlainText
                    text: surface.compactHelp ? surface.compactKeyHelp : surface.keyHelp
                    color: surface.browserWindow.mutedTextColor
                    wrapMode: Text.Wrap
                    Accessible.name: "Keyboard controls: " + surface.keyHelp
                }
                ToolButton {
                    id: shortcutsButton
                    objectName: "dialogKeyboardHelp"
                    visible: surface.compactHelp
                    text: "Keys"
                    checkable: true
                    Accessible.name: "Show keyboard shortcuts"
                    onCheckedChanged: if (checked)
                                          shortcutRevealTimer.restart()
                    ToolTip.visible: hovered
                    ToolTip.text: surface.keyHelp
                }
                Timer {
                    id: shortcutRevealTimer
                    interval: 0
                    onTriggered: headerScroll.contentItem.contentY = Math.max(0,
                                                                              headerScroll.contentHeight
                                                                              - headerScroll.availableHeight)
                }
            }
        }
    }
}
