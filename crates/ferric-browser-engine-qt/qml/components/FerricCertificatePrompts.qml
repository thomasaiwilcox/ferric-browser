import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

pragma ComponentBehavior: Bound

Item {
    id: root

    // The host owns request lifetime, security policy, and engine resolution.
    // This component only renders the bounded choices and emits user intents.
    required property var browserWindow

    signal clientCertificateAccepted(int index)
    signal clientCertificateRejected()
    signal certificateAccepted()
    signal certificateRejected()

    function openClientCertificate() {
        clientCertificatePopup.open()
    }

    function closeClientCertificate() {
        clientCertificatePopup.close()
    }

    function openCertificateError() {
        certificateErrorPopup.open()
    }

    function closeCertificateError() {
        certificateErrorPopup.close()
    }

    Popup {
        id: clientCertificatePopup
        parent: Overlay.overlay
        modal: true
        focus: true
        closePolicy: Popup.NoAutoClose
        width: Math.min(760 * root.browserWindow.chromeScale, root.browserWindow.width - 48)
        height: Math.min(520 * root.browserWindow.chromeScale, root.browserWindow.height - 32)
        padding: 14
        x: Math.round((root.browserWindow.width - width) / 2)
        y: Math.round((root.browserWindow.height - height) / 2)

        background: Rectangle {
            color: root.browserWindow.panelColor
            border.color: root.browserWindow.accentColor
            radius: 4
        }

        contentItem: ColumnLayout {
            focus: true
            Accessible.role: Accessible.Dialog
            Accessible.name: "Client certificate selection"
            spacing: 10

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    root.clientCertificateRejected()
                    event.accepted = true
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Select a client certificate"
                color: root.browserWindow.primaryTextColor
                font.bold: true
                Accessible.name: "Client certificate selection title"
            }

            Label {
                Layout.fillWidth: true
                text: "Host: " + (root.browserWindow.pendingClientCertificateHost
                                   || "opaque or unavailable host")
                color: root.browserWindow.mutedTextColor
                elide: Text.ElideMiddle
                Accessible.name: "Client certificate host"
            }

            Label {
                Layout.fillWidth: true
                text: "Choose one of the identities offered by the browser engine. Private keys are never exposed here."
                color: root.browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "Client certificate guidance"
            }

            ListView {
                id: clientCertificateList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: root.browserWindow.pendingClientCertificateOptions
                Accessible.role: Accessible.List
                Accessible.name: "Available client certificates"

                delegate: Rectangle {
                    id: certificateDelegate
                    required property var modelData
                    required property int index
                    width: clientCertificateList.width
                    height: Math.max(72 * root.browserWindow.chromeScale, 64)
                    color: certificateDelegate.index % 2 === 0
                           ? root.browserWindow.surfaceColor : root.browserWindow.panelColor
                    border.color: root.browserWindow.borderColor
                    border.width: 1

                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: 8
                        spacing: 10

                        ColumnLayout {
                            Layout.fillWidth: true
                            Label {
                                Layout.fillWidth: true
                                text: certificateDelegate.modelData.subject
                                color: root.browserWindow.primaryTextColor
                                elide: Text.ElideMiddle
                                Accessible.name: "Certificate subject"
                            }
                            Label {
                                Layout.fillWidth: true
                                text: "Issuer: " + certificateDelegate.modelData.issuer
                                      + (certificateDelegate.modelData.selfSigned
                                         ? " (self-signed)" : "")
                                color: root.browserWindow.mutedTextColor
                                elide: Text.ElideMiddle
                                Accessible.name: "Certificate issuer"
                            }
                        }

                        Button {
                            text: "Use"
                            Accessible.name: "Use this client certificate"
                            onClicked: root.clientCertificateAccepted(
                                           certificateDelegate.modelData.index)
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Cancel"
                    Accessible.name: "Cancel client certificate selection"
                    onClicked: root.clientCertificateRejected()
                }
            }
        }
    }

    Popup {
        id: certificateErrorPopup
        parent: Overlay.overlay
        modal: true
        focus: true
        closePolicy: Popup.NoAutoClose
        width: Math.min(680 * root.browserWindow.chromeScale, root.browserWindow.width - 48)
        height: Math.min(420 * root.browserWindow.chromeScale, root.browserWindow.height - 32)
        padding: 14
        x: Math.round((root.browserWindow.width - width) / 2)
        y: Math.round((root.browserWindow.height - height) / 2)

        background: Rectangle {
            color: root.browserWindow.panelColor
            border.color: root.browserWindow.warningColor
            radius: 4
        }

        contentItem: ColumnLayout {
            focus: true
            Accessible.role: Accessible.Dialog
            Accessible.name: "TLS certificate warning"
            spacing: 10

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    root.certificateRejected()
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    root.certificateAccepted()
                    event.accepted = true
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Certificate cannot be verified"
                color: root.browserWindow.warningColor
                font.bold: true
                Accessible.name: "TLS certificate warning title"
            }

            Label {
                Layout.fillWidth: true
                text: "Host: " + (root.browserWindow.pendingCertificateErrorHost
                                   || "opaque or unavailable host")
                color: root.browserWindow.mutedTextColor
                elide: Text.ElideMiddle
                Accessible.name: "TLS certificate host"
            }

            Label {
                Layout.fillWidth: true
                text: root.browserWindow.pendingCertificateErrorDescription
                color: root.browserWindow.primaryTextColor
                wrapMode: Text.WordWrap
                maximumLineCount: 12
                elide: Text.ElideRight
                Accessible.name: "TLS certificate error"
            }

            Label {
                Layout.fillWidth: true
                text: "Only continue if you recognize this host and understand the risk. This exception applies to this request only."
                color: root.browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "TLS certificate warning guidance"
            }

            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Go back"
                    Accessible.name: "Reject TLS certificate"
                    onClicked: root.certificateRejected()
                }
                Button {
                    text: "Accept once"
                    Accessible.name: "Accept TLS certificate for this request only"
                    onClicked: root.certificateAccepted()
                }
            }
        }
    }
}
