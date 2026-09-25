import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

pragma ComponentBehavior: Bound

Item {
    id: root
    anchors.fill: parent

    required property var browserWindow

    signal clientCertificateAccepted(int index)
    signal clientCertificateRejected()
    signal certificateAccepted()
    signal certificateRejected()

    function openClientCertificate() { clientCertificatePrompt.visible = true }
    function closeClientCertificate() { clientCertificatePrompt.visible = false }
    function openCertificateError() { certificateErrorPrompt.visible = true }
    function closeCertificateError() { certificateErrorPrompt.visible = false }

    FerricModalSurface {
        id: clientCertificatePrompt
        browserWindow: root.browserWindow
        visible: false
        commandText: ":client-certificate"
        title: "Select a client certificate"
        message: "Choose one of the identities offered by the browser engine. Private keys are never exposed here."
        keyHelp: "j/k or ↑/↓ select  ·  enter use certificate  ·  esc cancel"
        dialogWidth: 760 * scale
        dialogHeight: 520 * scale
        initialFocusItem: clientCertificateList
        onDismissRequested: root.clientCertificateRejected()

        function moveChoice(delta) {
            if (clientCertificateList.count === 0) {
                return
            }
            clientCertificateList.currentIndex =
                    (clientCertificateList.currentIndex + delta
                     + clientCertificateList.count) % clientCertificateList.count
            clientCertificateList.positionViewAtIndex(
                clientCertificateList.currentIndex, ListView.Contain)
        }

        function activateCurrent() {
            var choice = root.browserWindow.pendingClientCertificateOptions[
                clientCertificateList.currentIndex]
            if (choice) {
                root.clientCertificateAccepted(choice.index)
            }
        }

        Shortcut { sequence: "Up"; context: Qt.WindowShortcut; enabled: clientCertificatePrompt.visible; onActivated: clientCertificatePrompt.moveChoice(-1) }
        Shortcut { sequence: "K"; context: Qt.WindowShortcut; enabled: clientCertificatePrompt.visible; onActivated: clientCertificatePrompt.moveChoice(-1) }
        Shortcut { sequence: "Down"; context: Qt.WindowShortcut; enabled: clientCertificatePrompt.visible; onActivated: clientCertificatePrompt.moveChoice(1) }
        Shortcut { sequence: "J"; context: Qt.WindowShortcut; enabled: clientCertificatePrompt.visible; onActivated: clientCertificatePrompt.moveChoice(1) }
        Shortcut { sequence: "Return"; context: Qt.WindowShortcut; enabled: clientCertificatePrompt.visible; onActivated: clientCertificatePrompt.activateCurrent() }
        Shortcut { sequence: "Enter"; context: Qt.WindowShortcut; enabled: clientCertificatePrompt.visible; onActivated: clientCertificatePrompt.activateCurrent() }

        ColumnLayout {
            anchors.fill: parent
            spacing: 8 * clientCertificatePrompt.scale

            Label {
                Layout.fillWidth: true
                text: "Host: " + (root.browserWindow.pendingClientCertificateHost
                                   || "opaque or unavailable host")
                color: root.browserWindow.mutedTextColor
                elide: Text.ElideMiddle
                Accessible.name: "Client certificate host"
            }
            ListView {
                id: clientCertificateList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 6 * clientCertificatePrompt.scale
                currentIndex: 0
                model: root.browserWindow.pendingClientCertificateOptions
                Accessible.role: Accessible.List
                Accessible.name: "Available client certificates"

                delegate: FerricCommandAction {
                    id: certificateDelegate
                    required property var modelData
                    required property int index
                    width: clientCertificateList.width
                    browserWindow: root.browserWindow
                    keyHint: index === clientCertificateList.currentIndex ? "enter" : ""
                    actionLabel: certificateDelegate.modelData.subject
                                 + " · " + certificateDelegate.modelData.issuer
                                 + (certificateDelegate.modelData.selfSigned
                                    ? " (self-signed)" : "")
                    selected: index === clientCertificateList.currentIndex
                    onSelectionRequested: clientCertificateList.currentIndex = index
                    onClicked: root.clientCertificateAccepted(
                                   certificateDelegate.modelData.index)
                }
            }
        }
    }

    FerricCommandDialog {
        id: certificateErrorPrompt
        browserWindow: root.browserWindow
        visible: false
        commandText: ":certificate-error"
        title: "Certificate cannot be verified"
        message: "Only continue if you recognize this host and understand the risk. This exception applies to this request only."
        dialogWidth: 680 * scale
        dialogHeight: 440 * scale
        dialogBorderColor: root.browserWindow.warningColor
        cancelAction: "back"
        actions: [
            { id: "back", key: "n", label: "Go back", safe: true,
              shortcuts: ["N"] },
            { id: "accept", key: "y", label: "Accept once",
              shortcuts: ["Y"] }
        ]
        onActionRequested: function(action) {
            if (action === "accept") {
                root.certificateAccepted()
            } else {
                root.certificateRejected()
            }
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 8 * certificateErrorPrompt.scale

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
                Layout.fillHeight: true
                text: root.browserWindow.pendingCertificateErrorDescription
                color: root.browserWindow.primaryTextColor
                wrapMode: Text.WordWrap
                maximumLineCount: 12
                elide: Text.ElideRight
                Accessible.name: "TLS certificate error"
            }
        }
    }
}
