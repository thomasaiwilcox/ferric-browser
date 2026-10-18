pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtWebEngine

Item {
    id: root
    anchors.fill: parent

    required property var browserWindow
    property var hostWindow: browserWindow
    property alias pin: webAuthPinField.text

    signal accountSelected(string account)
    signal pinSubmitted()
    signal cancelled()
    signal retryRequested()
    function open(host) {
        hostWindow = host || browserWindow;
        parent = hostWindow.contentItem;
        webAuthPinField.text = "";
        accountList.currentIndex = 0;
        prompt.visible = true;
    }
    function close() {
        webAuthPinField.text = "";
        prompt.visible = false;
        parent = browserWindow.contentItem;
        hostWindow = browserWindow;
    }

    FerricModalSurface {
        id: prompt
        browserWindow: root.browserWindow
        hostWindow: root.hostWindow
        visible: false
        commandText: "Passkeys and security keys"
        title: "Verify your identity"
        message: "For: " + (root.browserWindow.pendingWebAuthRelyingParty
                            || "Relying party unavailable") + "\n"
                 + root.browserWindow.webAuthStatusText
        keyHelp: root.browserWindow.pendingWebAuthState
                 === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount
                 ? "j/k or ↑/↓ select  ·  enter use account  ·  esc cancel" :
                   root.browserWindow.pendingWebAuthState
                   === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin
                   ? "enter submit PIN  ·  esc cancel" : "r retry when available  ·  esc cancel"
        compactKeyHelp: "esc cancel"
        dialogWidth: 680 * scale
        dialogHeight: 520 * scale
        dialogBorderColor: root.browserWindow.privateColor
        initialFocusItem: root.browserWindow.pendingWebAuthState
                          === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin ? webAuthPinField :
                                                                                    cancelButton
        onDismissRequested: root.cancelled()

        Connections {
            target: root.browserWindow
            function onPendingWebAuthStateChanged() {
                webAuthPinField.text = "";
                prompt.focusInitialControl();
            }
        }

        function moveAccount(delta) {
            if (accountList.count === 0) {
                return;
            }
            accountList.currentIndex = (accountList.currentIndex + delta + accountList.count)
                    % accountList.count;
            accountList.positionViewAtIndex(accountList.currentIndex, ListView.Contain);
            accountList.forceActiveFocus();
        }

        function activateAccount() {
            if (accountList.currentIndex >= 0 && accountList.currentIndex < accountList.count) {
                root.accountSelected(
                            root.browserWindow.pendingWebAuthUserNames[accountList.currentIndex]);
            }
        }

        Shortcut {
            sequence: "Up"
            context: Qt.WindowShortcut
            enabled: prompt.interactive && root.browserWindow.pendingWebAuthState
                     === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount && (
                         !prompt.containsActiveFocus || prompt.focusWithin(accountList)
                         || cancelButton.activeFocus)
            onActivated: prompt.moveAccount(-1)
        }
        Shortcut {
            sequence: "K"
            context: Qt.WindowShortcut
            enabled: prompt.interactive && root.browserWindow.pendingWebAuthState
                     === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount && (
                         !prompt.containsActiveFocus || prompt.focusWithin(accountList)
                         || cancelButton.activeFocus)
            onActivated: prompt.moveAccount(-1)
        }
        Shortcut {
            sequence: "Down"
            context: Qt.WindowShortcut
            enabled: prompt.interactive && root.browserWindow.pendingWebAuthState
                     === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount && (
                         !prompt.containsActiveFocus || prompt.focusWithin(accountList)
                         || cancelButton.activeFocus)
            onActivated: prompt.moveAccount(1)
        }
        Shortcut {
            sequence: "J"
            context: Qt.WindowShortcut
            enabled: prompt.interactive && root.browserWindow.pendingWebAuthState
                     === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount && (
                         !prompt.containsActiveFocus || prompt.focusWithin(accountList)
                         || cancelButton.activeFocus)
            onActivated: prompt.moveAccount(1)
        }
        Shortcut {
            sequences: ["Return", "Enter"]
            context: Qt.WindowShortcut
            autoRepeat: false
            enabled: prompt.interactive && prompt.focusWithin(accountList)
                     && root.browserWindow.pendingWebAuthState
                     === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount
            onActivated: prompt.activateAccount()
        }
        Shortcut {
            sequence: "R"
            context: Qt.WindowShortcut
            enabled: prompt.interactive && root.browserWindow.pendingWebAuthState
                     === WebEngineWebAuthUxRequest.WebAuthUxState.RequestFailed && (
                         !prompt.containsActiveFocus || cancelButton.activeFocus)
            autoRepeat: false
            onActivated: root.retryRequested()
        }

        ColumnLayout {
            anchors.fill: parent
            FerricScrollColumn {
                Layout.fillWidth: true
                Layout.fillHeight: true
                hostWindow: prompt.hostWindow
                spacing: 8 * prompt.scale

                FerricManagerList {
                    id: accountList
                    objectName: "webAuthAccounts"
                    hostWindow: prompt.hostWindow
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.minimumHeight: 80 * prompt.scale
                    visible: root.browserWindow.pendingWebAuthState
                             === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount
                    clip: true
                    spacing: 6 * prompt.scale
                    currentIndex: 0
                    model: root.browserWindow.pendingWebAuthUserNames
                    Accessible.role: Accessible.List
                    Accessible.name: "WebAuthn accounts"

                    delegate: FerricCommandAction {
                        id: accountDelegate
                        required property string modelData
                        required property int index
                        width: accountList.width
                        browserWindow: root.browserWindow
                        keyHint: selected ? "enter" : ""
                        actionLabel: accountDelegate.modelData
                        selected: index === accountList.currentIndex && prompt.focusWithin(accountList)
                        onSelectionRequested: accountList.currentIndex = index
                        onClicked: root.accountSelected(accountDelegate.modelData)
                    }
                }
                Label {
                    textFormat: Text.PlainText
                    Layout.fillWidth: true
                    visible: root.browserWindow.pendingWebAuthState
                             === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin
                    wrapMode: Text.Wrap
                    text: "PIN attempts remaining: " + (root.browserWindow.pendingWebAuthRequest
                                                        ? root.browserWindow.pendingWebAuthRequest.pinRequest.remainingAttempts :
                                                          0) + "; minimum length: " + (
                              root.browserWindow.pendingWebAuthRequest
                              ? root.browserWindow.pendingWebAuthRequest.pinRequest.minPinLength :
                                0)
                    color: root.browserWindow.mutedTextColor
                    Accessible.name: "WebAuthn PIN guidance"
                }
                FerricTextField {
                    id: webAuthPinField
                    objectName: "webAuthPin"
                    Keys.onReturnPressed: function (event) {
                        if (!event.isAutoRepeat && event.modifiers === Qt.NoModifier)
                            root.pinSubmitted();
                    }
                    Keys.onEnterPressed: function (event) {
                        if (!event.isAutoRepeat && event.modifiers === Qt.NoModifier)
                            root.pinSubmitted();
                    }
                    Layout.fillWidth: true
                    visible: root.browserWindow.pendingWebAuthState
                             === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin
                    echoMode: TextInput.Password
                    placeholderText: "Security-key PIN"
                    Accessible.name: "WebAuthn PIN"
                    Accessible.role: Accessible.EditableText
                    Accessible.editable: true
                }
                Label {
                    textFormat: Text.PlainText
                    Layout.fillWidth: true
                    visible: root.browserWindow.pendingWebAuthState
                             === WebEngineWebAuthUxRequest.WebAuthUxState.FinishTokenCollection
                    text: "Follow the security-key instruction, then wait for completion."
                    color: root.browserWindow.secondaryTextColor
                    wrapMode: Text.WordWrap
                    Accessible.name: "WebAuthn security-key instruction"
                }
                Label {
                    textFormat: Text.PlainText
                    Layout.fillWidth: true
                    visible: root.browserWindow.pendingWebAuthState
                             === WebEngineWebAuthUxRequest.WebAuthUxState.RequestFailed
                    text: "The authenticator reported a failure. You may retry or cancel."
                    color: root.browserWindow.errorColor
                    wrapMode: Text.WordWrap
                    Accessible.name: "WebAuthn failure guidance"
                }
                Item {
                    Layout.fillHeight: true
                }
                FerricCommandAction {
                    Layout.fillWidth: true
                    visible: root.browserWindow.pendingWebAuthState
                             === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin
                    browserWindow: root.browserWindow
                    keyHint: "enter"
                    actionLabel: "Submit PIN"
                    selected: true
                    onClicked: root.pinSubmitted()
                }
                FerricCommandAction {
                    Layout.fillWidth: true
                    visible: root.browserWindow.pendingWebAuthState
                             === WebEngineWebAuthUxRequest.WebAuthUxState.RequestFailed
                    browserWindow: root.browserWindow
                    keyHint: "r"
                    actionLabel: "Retry"
                    selected: true
                    onClicked: root.retryRequested()
                }
            }
            FerricCommandAction {
                id: cancelButton
                objectName: "webAuthCancel"
                Layout.fillWidth: true
                browserWindow: root.browserWindow
                keyHint: "esc"
                actionLabel: "Cancel request"
                selected: activeFocus
                safe: true
                onClicked: root.cancelled()
            }
        }
    }
}
