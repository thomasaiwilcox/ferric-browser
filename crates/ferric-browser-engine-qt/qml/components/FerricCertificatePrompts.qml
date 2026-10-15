pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: root
    anchors.fill: parent

    required property var browserWindow
    property var hostWindow: browserWindow

    signal clientCertificateAccepted(int index)
    signal clientCertificateRejected()
    signal certificateAccepted()
    signal certificateRejected()
    function openClientCertificate(host) {
        hostWindow = host || browserWindow;
        parent = hostWindow.contentItem;
        clientCertificatePrompt.visible = true;
    }
    function closeClientCertificate() {
        clientCertificatePrompt.visible = false;
        if (!certificateErrorPrompt.visible) { parent = browserWindow.contentItem; hostWindow = browserWindow; }
    }
    function openCertificateError(host) {
        hostWindow = host || browserWindow;
        parent = hostWindow.contentItem;
        certificateErrorPrompt.visible = true;
    }
    function closeCertificateError() {
        certificateErrorPrompt.visible = false;
        if (!clientCertificatePrompt.visible) { parent = browserWindow.contentItem; hostWindow = browserWindow; }
    }

    FerricModalSurface {
        id: clientCertificatePrompt
        browserWindow: root.browserWindow
        hostWindow: root.hostWindow
        visible: false
        commandText: "Client certificate"
        title: "Select a client certificate"
        message: "For: " + (root.browserWindow.pendingClientCertificateHost || "Host unavailable")
                 + "\nChoose an identity to use for this connection. Private keys stay protected."
        keyHelp: "j/k or ↑/↓ select  ·  enter use certificate  ·  esc cancel"
        dialogWidth: 760 * scale
        dialogHeight: 520 * scale
        initialFocusItem: clientCertificateCancel
        onDismissRequested: root.clientCertificateRejected()

        function moveChoice(delta) {
            if (clientCertificateList.count === 0) {
                return;
            }
            clientCertificateList.currentIndex = (clientCertificateList.currentIndex + delta
                                                  + clientCertificateList.count)
                    % clientCertificateList.count;
            clientCertificateList.positionViewAtIndex(clientCertificateList.currentIndex,
                                                      ListView.Contain);
            clientCertificateList.forceActiveFocus();
        }

        function activateCurrent() {
            var choice
                    = root.browserWindow.pendingClientCertificateOptions[clientCertificateList.currentIndex];
            if (choice) {
                root.clientCertificateAccepted(choice.index);
            }
        }

        Shortcut {
            sequence: "Up"
            context: Qt.WindowShortcut
            enabled: clientCertificatePrompt.interactive && (
                         !clientCertificatePrompt.containsActiveFocus
                         || clientCertificatePrompt.focusWithin(clientCertificateList)
                         || clientCertificateCancel.activeFocus)
            onActivated: clientCertificatePrompt.moveChoice(-1)
        }
        Shortcut {
            sequence: "K"
            context: Qt.WindowShortcut
            enabled: clientCertificatePrompt.interactive && (
                         !clientCertificatePrompt.containsActiveFocus
                         || clientCertificatePrompt.focusWithin(clientCertificateList)
                         || clientCertificateCancel.activeFocus)
            onActivated: clientCertificatePrompt.moveChoice(-1)
        }
        Shortcut {
            sequence: "Down"
            context: Qt.WindowShortcut
            enabled: clientCertificatePrompt.interactive && (
                         !clientCertificatePrompt.containsActiveFocus
                         || clientCertificatePrompt.focusWithin(clientCertificateList)
                         || clientCertificateCancel.activeFocus)
            onActivated: clientCertificatePrompt.moveChoice(1)
        }
        Shortcut {
            sequence: "J"
            context: Qt.WindowShortcut
            enabled: clientCertificatePrompt.interactive && (
                         !clientCertificatePrompt.containsActiveFocus
                         || clientCertificatePrompt.focusWithin(clientCertificateList)
                         || clientCertificateCancel.activeFocus)
            onActivated: clientCertificatePrompt.moveChoice(1)
        }
        Shortcut {
            sequence: "Return"
            context: Qt.WindowShortcut
            autoRepeat: false
            enabled: clientCertificatePrompt.interactive && clientCertificatePrompt.focusWithin(
                         clientCertificateList)
            onActivated: clientCertificatePrompt.activateCurrent()
        }
        Shortcut {
            sequence: "Enter"
            context: Qt.WindowShortcut
            autoRepeat: false
            enabled: clientCertificatePrompt.interactive && clientCertificatePrompt.focusWithin(
                         clientCertificateList)
            onActivated: clientCertificatePrompt.activateCurrent()
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 8 * clientCertificatePrompt.scale

            FerricManagerList {
                id: clientCertificateList
                objectName: "clientCertificateChoices"
                hostWindow: clientCertificatePrompt.hostWindow
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
                    keyHint: selected ? "enter" : ""
                    actionLabel: certificateDelegate.modelData.subject + " · "
                                 + certificateDelegate.modelData.issuer + (
                                     certificateDelegate.modelData.selfSigned ? " (self-signed)" :
                                                                                "")
                    selected: index === clientCertificateList.currentIndex && clientCertificatePrompt.focusWithin(clientCertificateList)
                    onSelectionRequested: clientCertificateList.currentIndex = index
                    onClicked: root.clientCertificateAccepted(certificateDelegate.modelData.index)
                }
            }
            Label {
                Layout.fillWidth: true
                visible: clientCertificateList.count === 0
                text: "No client certificates are available."
                color: root.browserWindow.mutedTextColor
                wrapMode: Text.Wrap
            }
            Button {
                id: clientCertificateCancel
                objectName: "clientCertificateCancel"
                Layout.alignment: Qt.AlignRight
                text: "Cancel"
                onClicked: root.clientCertificateRejected()
            }
        }
    }

    FerricCommandDialog {
        id: certificateErrorPrompt
        browserWindow: root.browserWindow
        hostWindow: root.hostWindow
        visible: false
        commandText: "Connection security"
        title: "Certificate cannot be verified"
        message: "For: " + (root.browserWindow.pendingCertificateErrorHost || "Host unavailable")
                 + "\nOnly continue if you recognize this host and understand the risk. The browser may reuse this certificate exception during this session. It does not change your system's trusted certificates."
        dialogWidth: 680 * scale
        dialogHeight: 350 * scale
        minimumBodyHeight: 44 * scale
        dialogBorderColor: root.browserWindow.warningColor
        cancelAction: "back"
        actions: [
            {
                id: "back",
                key: "n",
                label: "Go back",
                safe: true,
                shortcuts: ["N"]
            },
            {
                id: "accept",
                key: "y",
                label: "Continue for this session",
                shortcuts: ["Y"]
            }
        ]
        onActionRequested: function (action) {
            if (action === "accept") {
                root.certificateAccepted();
            } else {
                root.certificateRejected();
            }
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 8 * certificateErrorPrompt.scale

            TextArea {
                objectName: "certificateErrorDescription"
                Layout.fillWidth: true
                text: root.browserWindow.pendingCertificateErrorDescription
                Layout.alignment: Qt.AlignTop
                textFormat: TextEdit.PlainText
                readOnly: true
                selectByMouse: true
                selectByKeyboard: true
                padding: 0
                background: null
                color: root.browserWindow.primaryTextColor
                wrapMode: TextEdit.Wrap
                Accessible.name: "TLS certificate error"
            }
        }
    }
}
