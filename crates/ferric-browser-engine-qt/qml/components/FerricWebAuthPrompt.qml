import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtWebEngine

pragma ComponentBehavior: Bound

Item {
    id: root
    anchors.fill: parent

    required property var browserWindow
    property alias pin: webAuthPinField.text

    signal accountSelected(string account)
    signal pinSubmitted()
    signal cancelled()
    signal retryRequested()

    function open() { prompt.visible = true }
    function close() { prompt.visible = false }

    FerricModalSurface {
        id: prompt
        browserWindow: root.browserWindow
        visible: false
        commandText: ":webauth"
        title: "WebAuthn security-key request"
        message: root.browserWindow.webAuthStatusText
        keyHelp: root.browserWindow.pendingWebAuthState
                 === WebEngineWebAuthUxRequest.SelectAccount
                 ? "j/k or ↑/↓ select  ·  enter use account  ·  esc cancel"
                 : root.browserWindow.pendingWebAuthState
                   === WebEngineWebAuthUxRequest.CollectPin
                   ? "enter submit PIN  ·  esc cancel"
                   : "r retry when available  ·  esc cancel"
        dialogWidth: 680 * scale
        dialogHeight: 520 * scale
        dialogBorderColor: root.browserWindow.privateColor
        initialFocusItem: root.browserWindow.pendingWebAuthState
                          === WebEngineWebAuthUxRequest.CollectPin
                          ? webAuthPinField : accountList
        onDismissRequested: root.cancelled()

        function moveAccount(delta) {
            if (accountList.count === 0) {
                return
            }
            accountList.currentIndex = (accountList.currentIndex + delta
                                        + accountList.count) % accountList.count
            accountList.positionViewAtIndex(accountList.currentIndex, ListView.Contain)
        }

        function activateAccount() {
            if (accountList.currentIndex >= 0
                    && accountList.currentIndex < accountList.count) {
                root.accountSelected(
                    root.browserWindow.pendingWebAuthUserNames[accountList.currentIndex])
            }
        }

        Shortcut {
            sequence: "Up"
            context: Qt.WindowShortcut
            enabled: prompt.visible && root.browserWindow.pendingWebAuthState
                     === WebEngineWebAuthUxRequest.SelectAccount
            onActivated: prompt.moveAccount(-1)
        }
        Shortcut {
            sequence: "K"
            context: Qt.WindowShortcut
            enabled: prompt.visible && root.browserWindow.pendingWebAuthState
                     === WebEngineWebAuthUxRequest.SelectAccount
            onActivated: prompt.moveAccount(-1)
        }
        Shortcut {
            sequence: "Down"
            context: Qt.WindowShortcut
            enabled: prompt.visible && root.browserWindow.pendingWebAuthState
                     === WebEngineWebAuthUxRequest.SelectAccount
            onActivated: prompt.moveAccount(1)
        }
        Shortcut {
            sequence: "J"
            context: Qt.WindowShortcut
            enabled: prompt.visible && root.browserWindow.pendingWebAuthState
                     === WebEngineWebAuthUxRequest.SelectAccount
            onActivated: prompt.moveAccount(1)
        }
        Shortcut {
            sequence: "Return"
            context: Qt.WindowShortcut
            enabled: prompt.visible
                     && (root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.SelectAccount
                         || root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.CollectPin)
            onActivated: {
                if (root.browserWindow.pendingWebAuthState
                        === WebEngineWebAuthUxRequest.CollectPin) {
                    root.pinSubmitted()
                } else {
                    prompt.activateAccount()
                }
            }
        }
        Shortcut {
            sequence: "Enter"
            context: Qt.WindowShortcut
            enabled: prompt.visible
                     && (root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.SelectAccount
                         || root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.CollectPin)
            onActivated: {
                if (root.browserWindow.pendingWebAuthState
                        === WebEngineWebAuthUxRequest.CollectPin) {
                    root.pinSubmitted()
                } else {
                    prompt.activateAccount()
                }
            }
        }
        Shortcut {
            sequence: "R"
            context: Qt.WindowShortcut
            enabled: prompt.visible && root.browserWindow.pendingWebAuthState
                     === WebEngineWebAuthUxRequest.RequestFailed
            onActivated: root.retryRequested()
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 8 * prompt.scale

            Label {
                Layout.fillWidth: true
                text: "Relying party: "
                      + (root.browserWindow.pendingWebAuthRelyingParty
                         || "opaque or unavailable relying party")
                color: root.browserWindow.mutedTextColor
                elide: Text.ElideMiddle
                Accessible.name: "WebAuthn relying party"
            }
            ListView {
                id: accountList
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.SelectAccount
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
                    keyHint: index === accountList.currentIndex ? "enter" : ""
                    actionLabel: accountDelegate.modelData
                    selected: index === accountList.currentIndex
                    onSelectionRequested: accountList.currentIndex = index
                    onClicked: root.accountSelected(accountDelegate.modelData)
                }
            }
            Label {
                Layout.fillWidth: true
                visible: root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.CollectPin
                text: "PIN attempts remaining: "
                      + (root.browserWindow.pendingWebAuthRequest
                         ? root.browserWindow.pendingWebAuthRequest.pinRequest.remainingAttempts : 0)
                      + "; minimum length: "
                      + (root.browserWindow.pendingWebAuthRequest
                         ? root.browserWindow.pendingWebAuthRequest.pinRequest.minPinLength : 0)
                color: root.browserWindow.mutedTextColor
                Accessible.name: "WebAuthn PIN guidance"
            }
            TextField {
                id: webAuthPinField
                Layout.fillWidth: true
                visible: root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.CollectPin
                echoMode: TextInput.Password
                placeholderText: "Security-key PIN"
                Accessible.name: "WebAuthn PIN"
                Accessible.role: Accessible.EditableText
                Accessible.editable: true
            }
            Label {
                Layout.fillWidth: true
                visible: root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.FinishTokenCollection
                text: "Follow the security-key instruction, then wait for completion."
                color: root.browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "WebAuthn security-key instruction"
            }
            Label {
                Layout.fillWidth: true
                visible: root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.RequestFailed
                text: "The authenticator reported a failure. You may retry or cancel."
                color: root.browserWindow.errorColor
                wrapMode: Text.WordWrap
                Accessible.name: "WebAuthn failure guidance"
            }
            Item { Layout.fillHeight: true }
            FerricCommandAction {
                Layout.fillWidth: true
                visible: root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.CollectPin
                browserWindow: root.browserWindow
                keyHint: "enter"
                actionLabel: "Submit PIN"
                selected: true
                onClicked: root.pinSubmitted()
            }
            FerricCommandAction {
                Layout.fillWidth: true
                visible: root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.RequestFailed
                browserWindow: root.browserWindow
                keyHint: "r"
                actionLabel: "Retry"
                selected: true
                onClicked: root.retryRequested()
            }
            FerricCommandAction {
                Layout.fillWidth: true
                browserWindow: root.browserWindow
                keyHint: "esc"
                actionLabel: "Cancel request"
                safe: true
                onClicked: root.cancelled()
            }
        }
    }
}
