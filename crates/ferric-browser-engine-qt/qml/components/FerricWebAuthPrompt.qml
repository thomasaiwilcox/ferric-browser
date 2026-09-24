import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtWebEngine

pragma ComponentBehavior: Bound

Item {
    id: root

    // Request validation and WebEngine calls remain with the browser window.
    // This component renders state supplied by that owner and emits intents.
    required property var browserWindow
    property alias pin: webAuthPinField.text

    signal accountSelected(string account)
    signal pinSubmitted()
    signal cancelled()
    signal retryRequested()

    function open() {
        prompt.open()
    }

    function close() {
        prompt.close()
    }

    Popup {
        id: prompt
        parent: Overlay.overlay
        modal: true
        focus: true
        closePolicy: Popup.NoAutoClose
        width: Math.min(680 * root.browserWindow.chromeScale, root.browserWindow.width - 48)
        height: Math.min(520 * root.browserWindow.chromeScale, root.browserWindow.height - 32)
        padding: 14
        x: Math.round((root.browserWindow.width - width) / 2)
        y: Math.round((root.browserWindow.height - height) / 2)

        background: Rectangle {
            color: root.browserWindow.panelColor
            border.color: root.browserWindow.privateColor
            radius: 4
        }

        contentItem: ColumnLayout {
            focus: true
            Accessible.role: Accessible.Dialog
            Accessible.name: "WebAuthn security-key prompt"
            spacing: 10

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    root.cancelled()
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    if (root.browserWindow.pendingWebAuthState
                            === WebEngineWebAuthUxRequest.CollectPin) {
                        root.pinSubmitted()
                        event.accepted = true
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                text: "WebAuthn security-key request"
                color: root.browserWindow.privateColor
                font.bold: true
                Accessible.name: "WebAuthn title"
            }

            Label {
                Layout.fillWidth: true
                text: "Relying party: " + (root.browserWindow.pendingWebAuthRelyingParty
                                             || "opaque or unavailable relying party")
                color: root.browserWindow.mutedTextColor
                elide: Text.ElideMiddle
                Accessible.name: "WebAuthn relying party"
            }

            Label {
                Layout.fillWidth: true
                text: root.browserWindow.webAuthStatusText
                color: root.browserWindow.primaryTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "WebAuthn status"
            }

            ListView {
                id: accountList
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.SelectAccount
                clip: true
                model: root.browserWindow.pendingWebAuthUserNames
                Accessible.role: Accessible.List
                Accessible.name: "WebAuthn accounts"

                delegate: Button {
                    id: accountDelegate
                    required property string modelData
                    width: accountList.width
                    text: accountDelegate.modelData
                    Accessible.name: "Use WebAuthn account " + accountDelegate.modelData
                    onClicked: root.accountSelected(accountDelegate.modelData)
                }
            }

            Label {
                Layout.fillWidth: true
                visible: root.browserWindow.pendingWebAuthState
                         === WebEngineWebAuthUxRequest.CollectPin
                text: "PIN attempts remaining: "
                      + (root.browserWindow.pendingWebAuthRequest
                         ? root.browserWindow.pendingWebAuthRequest.pinRequest.remainingAttempts
                         : 0)
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
                onVisibleChanged: if (visible) forceActiveFocus()
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

            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Cancel"
                    Accessible.name: "Cancel WebAuthn request"
                    onClicked: root.cancelled()
                }
                Button {
                    visible: root.browserWindow.pendingWebAuthState
                             === WebEngineWebAuthUxRequest.CollectPin
                    text: "Submit PIN"
                    Accessible.name: "Submit WebAuthn PIN"
                    onClicked: root.pinSubmitted()
                }
                Button {
                    visible: root.browserWindow.pendingWebAuthState
                             === WebEngineWebAuthUxRequest.RequestFailed
                    text: "Retry"
                    Accessible.name: "Retry WebAuthn request"
                    onClicked: root.retryRequested()
                }
            }
        }
    }
}
