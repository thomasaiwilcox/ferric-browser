import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ChromePresentation.js" as ChromePresentation

FerricModalSurface {
    id: ledger
    required property bool siteExperimentAvailable
    property string noticeText: ""
    property bool noticeIsWarning: false
    property real experimentRemainingSeconds: activeTest.remaining_seconds || 0
    readonly property var snapshot: browserWindow.siteLedgerData || ({})
    readonly property bool privateSession: snapshot.private === true
    readonly property var doctor: snapshot.site_doctor || ({})
    readonly property var activeTest: doctor.active || ({})
    readonly property var proposal: doctor.last_result || ({})
    readonly property string scopeKey: ChromePresentation.siteScopeKey(snapshot)
    readonly property string proposalKey: JSON.stringify([scopeKey, proposal.id || "", proposal.state || "", proposal.mutation || null])
    readonly property bool hasSiteProposal: !privateSession && !!snapshot.origin
        && proposal.kind === "blocker-exception" && proposal.origin === snapshot.origin

    signal closeRequested()
    signal refreshRequested()
    signal activeOriginDataClearRequested()
    signal sanitizedReportCopyRequested(bool includeHost)
    signal siteDoctorProposalApplyRequested(string proposalId)
    signal siteDoctorExperimentRequested(string kind)

    function resetConsents() {
        siteReportHost.checked = false
        siteDataClearConfirmation.checked = false
        siteDoctorProposalConfirmation.checked = false
    }
    function showNotice(message, warning) {
        noticeIsWarning = warning
        noticeText = message
    }
    function revealFocusedControl() {
        var item = hostWindow ? hostWindow.activeFocusItem : null
        var ancestor = item
        while (ancestor && ancestor !== content) ancestor = ancestor.parent
        if (!ancestor || !scroll.contentItem) return
        var viewport = scroll.contentItem
        var top = item.mapToItem(content, 0, 0).y
        if (top + item.height <= viewport.contentY + viewport.height && top >= viewport.contentY) return
        if (item.height > viewport.height && top < viewport.contentY
                && top + item.height > viewport.contentY + viewport.height) return
        var next = item.height >= viewport.height ? top : top < viewport.contentY ? top - 8 * scale
            : top + Math.min(item.height, viewport.height) - viewport.height + 8 * scale
        viewport.contentY = Math.max(0, Math.min(viewport.contentHeight - viewport.height, next))
    }
    Connections {
        target: ledger.hostWindow
        ignoreUnknownSignals: true
        function onActiveFocusItemChanged() { Qt.callLater(ledger.revealFocusedControl) }
    }
    onScopeKeyChanged: { resetConsents(); noticeText = ""; technicalDetails.checked = false }
    onProposalKeyChanged: siteDoctorProposalConfirmation.checked = false
    onVisibleChanged: { resetConsents(); technicalDetails.checked = false; noticeText = "" }

    visible: browserWindow.siteLedgerVisible
    commandText: ":site-status"
    title: "Site Ledger"
    message: privateSession ? "Private session · site identity is withheld"
             : (snapshot.origin || "Site information is available for HTTP and HTTPS pages")
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc close"
    dialogWidth: 940 * scale
    dialogHeight: 660 * scale
    stackingOrder: 70
    initialFocusItem: closeButton
    onDismissRequested: closeRequested()

    ColumnLayout {
        anchors.fill: parent
        spacing: 10 * ledger.scale
        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: "Snapshot"
                Accessible.description: "Refresh to check for changes"
                textFormat: Text.PlainText
                color: ledger.browserWindow.mutedTextColor
                wrapMode: Text.Wrap
            }
            Button { text: "Refresh"; onClicked: ledger.refreshRequested() }
            Button { id: closeButton; objectName: "siteLedgerCloseButton"; text: "Close"; onClicked: ledger.closeRequested() }
        }
        Label {
            objectName: "siteLedgerNotice"
            Layout.fillWidth: true
            visible: text.length > 0
            text: ledger.noticeText
            textFormat: Text.PlainText
            color: ledger.noticeIsWarning ? ledger.browserWindow.warningColor : ledger.browserWindow.secondaryTextColor
            wrapMode: Text.Wrap
            Accessible.name: text
        }
        ScrollView {
            id: scroll
            objectName: "siteLedgerScroll"
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            contentHeight: content.implicitHeight
            onContentHeightChanged: Qt.callLater(ledger.revealFocusedControl)
            clip: true
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            ScrollBar.vertical.active: true
            ColumnLayout {
                id: content
                width: scroll.availableWidth
                height: implicitHeight
                spacing: 12 * ledger.scale
                Repeater {
                    model: ledger.snapshot.facts || []
                    Rectangle {
                        required property var modelData
                        objectName: "siteFact-" + modelData.id
                        Layout.fillWidth: true
                        implicitHeight: factContent.implicitHeight + 20 * ledger.scale
                        color: ledger.browserWindow.surfaceColor
                        radius: 4 * ledger.scale
                        ColumnLayout {
                            id: factContent
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: 10 * ledger.scale
                            spacing: 4 * ledger.scale
                            Label {
                                Layout.fillWidth: true
                                text: ChromePresentation.siteFactTitle(modelData.id)
                                textFormat: Text.PlainText
                                color: ledger.browserWindow.primaryTextColor
                                font.bold: true
                                wrapMode: Text.Wrap
                            }
                            Label {
                                objectName: "siteFactSummary-" + modelData.id
                                Layout.fillWidth: true
                                text: ChromePresentation.siteFactSummary(modelData, ledger.privateSession, !!ledger.snapshot.origin)
                                textFormat: Text.PlainText
                                color: ledger.browserWindow.secondaryTextColor
                                wrapMode: Text.Wrap
                            }
                        }
                    }
                }
                Label {
                    Layout.fillWidth: true
                    visible: (ledger.snapshot.facts || []).length === 0
                    text: "No site information is available. Refresh to try again."
                    color: ledger.browserWindow.secondaryTextColor
                    wrapMode: Text.Wrap
                }
                Label {
                    Layout.fillWidth: true
                    text: "Troubleshoot this site"
                    color: ledger.browserWindow.primaryTextColor
                    font.bold: true
                    wrapMode: Text.Wrap
                }
                Label {
                    Layout.fillWidth: true
                    text: ledger.privateSession ? "Site tests are unavailable in private browsing."
                        : !ledger.snapshot.origin ? "Open a web page to use site tests."
                        : "These tests are temporary. Reloading, navigating away or the time limit ends a test. A fresh tab uses this profile's cookies and storage."
                    color: ledger.browserWindow.secondaryTextColor
                    wrapMode: Text.Wrap
                }
                Label {
                    objectName: "siteExperimentStatus"
                    Layout.fillWidth: true
                    visible: !ledger.privateSession && !!ledger.activeTest.kind
                    text: ChromePresentation.siteExperimentLabel(ledger.activeTest.kind)
                        + " · " + ledger.experimentRemainingSeconds + " seconds remaining"
                    textFormat: Text.PlainText
                    color: ledger.browserWindow.accentColor
                    wrapMode: Text.Wrap
                }
                Flow {
                    Layout.fillWidth: true
                    Layout.preferredHeight: childrenRect.height
                    spacing: 8 * ledger.scale
                    visible: !ledger.privateSession && !!ledger.snapshot.origin
                    Repeater {
                        model: ledger.doctor.experiments || []
                        Button {
                            id: experimentButton
                            required property string modelData
                            objectName: "siteExperiment-" + modelData
                            width: Math.min(implicitWidth, scroll.availableWidth)
                            enabled: ledger.siteExperimentAvailable
                            text: ChromePresentation.siteExperimentLabel(modelData)
                            Accessible.name: text
                            contentItem: Label {
                                text: experimentButton.text
                                font: experimentButton.font
                                color: experimentButton.enabled ? ledger.browserWindow.primaryTextColor : ledger.browserWindow.mutedTextColor
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                horizontalAlignment: Text.AlignHCenter
                            }
                            onClicked: ledger.siteDoctorExperimentRequested(modelData)
                        }
                    }
                }
                Label {
                    Layout.fillWidth: true
                    visible: ledger.hasSiteProposal
                    text: ledger.proposal.state === "applied" ? "The content-blocking exception was saved for this site."
                        : "The test suggests a content-blocking exception for " + (ledger.proposal.origin || "this site")
                          + ". Saving it allows this site's requests through content blocking. Site permissions and connection security stay unchanged."
                    textFormat: Text.PlainText
                    color: ledger.browserWindow.warningColor
                    wrapMode: Text.Wrap
                }
                CheckBox {
                    id: siteDoctorProposalConfirmation
                    objectName: "siteDoctorProposalConfirmation"
                    Layout.fillWidth: true
                    visible: ledger.hasSiteProposal && ledger.proposal.state === "pending-confirmation"
                    text: "I reviewed this site's blocking exception"
                    contentItem: Label {
                        text: siteDoctorProposalConfirmation.text
                        font: siteDoctorProposalConfirmation.font
                        color: ledger.browserWindow.primaryTextColor
                        wrapMode: Text.Wrap
                    }
                    checked: false
                }
                Button {
                    objectName: "siteDoctorApplyButton"
                    visible: siteDoctorProposalConfirmation.visible
                    enabled: siteDoctorProposalConfirmation.checked
                    text: "Apply reviewed fix"
                    onClicked: {
                        siteDoctorProposalConfirmation.checked = false
                        ledger.siteDoctorProposalApplyRequested(ledger.proposal.id)
                    }
                }
                Label {
                    Layout.fillWidth: true
                    visible: !ledger.privateSession && !!ledger.snapshot.origin
                    text: "Clear site data"
                    color: ledger.browserWindow.primaryTextColor
                    font.bold: true
                }
                Label {
                    Layout.fillWidth: true
                    visible: siteDataClearConfirmation.visible
                    text: "Clears local storage, Cache Storage, service workers and page-visible cookies for this origin. HttpOnly cookies, other cookie-store entries and the HTTP cache remain. History, permissions and other sites are kept."
                    color: ledger.browserWindow.secondaryTextColor
                    wrapMode: Text.Wrap
                }
                CheckBox {
                    id: siteDataClearConfirmation
                    objectName: "siteDataClearConfirmation"
                    Layout.fillWidth: true
                    visible: !ledger.privateSession && !!ledger.snapshot.origin
                    text: "I understand the limits of clearing this site's data"
                    contentItem: Label {
                        text: siteDataClearConfirmation.text
                        font: siteDataClearConfirmation.font
                        color: ledger.browserWindow.primaryTextColor
                        wrapMode: Text.Wrap
                    }
                    checked: false
                }
                Button {
                    objectName: "siteDataClearButton"
                    visible: siteDataClearConfirmation.visible
                    enabled: siteDataClearConfirmation.checked && !ledger.browserWindow.siteDataClearPending
                    text: ledger.browserWindow.siteDataClearPending ? "Clearing…" : "Clear site data"
                    onClicked: {
                        siteDataClearConfirmation.checked = false
                        ledger.activeOriginDataClearRequested()
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: "Share a diagnostic report"
                    color: ledger.browserWindow.primaryTextColor
                    font.bold: true
                }
                Label {
                    Layout.fillWidth: true
                    text: "The report excludes full URLs, page content, cookies, tokens and account identifiers. The current site host is included only if you choose it below."
                    color: ledger.browserWindow.secondaryTextColor
                    wrapMode: Text.Wrap
                }
                CheckBox {
                    id: siteReportHost
                    objectName: "siteReportHost"
                    Layout.fillWidth: true
                    visible: !ledger.privateSession && !!ledger.snapshot.origin
                    text: "Include current site host"
                    checked: false
                    contentItem: Label {
                        text: siteReportHost.text
                        font: siteReportHost.font
                        color: ledger.browserWindow.primaryTextColor
                        wrapMode: Text.Wrap
                    }
                    Accessible.description: "Host disclosure is opt-in; paths and query strings are never included"
                }
                Button {
                    objectName: "siteReportCopyButton"
                    text: "Copy report"
                    onClicked: ledger.sanitizedReportCopyRequested(!ledger.privateSession && siteReportHost.checked)
                }
                Button {
                    id: technicalDetails
                    objectName: "siteTechnicalDetailsButton"
                    text: checked ? "Hide technical details" : "Technical details"
                    checkable: true
                }
                Label {
                    id: interactionSummary
                    objectName: "siteLedgerInteractionSummary"
                    Layout.fillWidth: true
                    visible: technicalDetails.checked
                    text: ChromePresentation.interactionDetails(ledger.snapshot.interaction)
                    textFormat: Text.PlainText
                    color: ledger.browserWindow.secondaryTextColor
                    wrapMode: Text.Wrap
                    Accessible.name: "On-demand interaction status"
                    Accessible.description: text
                }
                TextArea {
                    objectName: "siteTechnicalDetails"
                    Layout.fillWidth: true
                    visible: technicalDetails.checked
                    text: ledger.privateSession ? "Site and document metadata are withheld in private browsing."
                        : JSON.stringify({capture: ledger.snapshot.capture, facts: ledger.snapshot.facts || [],
                            requests: (ledger.snapshot.blocking || {}).active_site_decisions || [],
                            commands: ledger.snapshot.safe_remediation_actions || []}, null, 2)
                    readOnly: true
                    selectByMouse: true
                    textFormat: TextEdit.PlainText
                    wrapMode: TextEdit.Wrap
                    color: ledger.browserWindow.secondaryTextColor
                    background: Rectangle { color: ledger.browserWindow.surfaceColor }
                    Accessible.name: "Technical site information"
                }
            }
        }
    }
}
