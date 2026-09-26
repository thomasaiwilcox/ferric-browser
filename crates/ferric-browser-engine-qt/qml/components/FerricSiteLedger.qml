import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: ledger
    required property bool siteExperimentAvailable

    signal closeRequested()
    signal refreshRequested()
    signal activeOriginDataClearRequested()
    signal sanitizedReportCopyRequested(bool includeHost)
    signal siteDoctorProposalApplyRequested(string proposalId)
    signal siteDoctorExperimentRequested(string kind)

    visible: browserWindow.siteLedgerVisible
    commandText: ":site-status"
    title: "Site Ledger"
    message: browserWindow.siteLedgerData.private
             ? "Private session · site identity is withheld"
             : (browserWindow.siteLedgerData.origin || "No normalized HTTP(S) origin")
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc close"
    dialogWidth: 940 * scale
    dialogHeight: 660 * scale
    stackingOrder: 70
    onDismissRequested: closeRequested()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 0
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                text: "Refresh"
                Accessible.name: "Refresh Site Ledger"
                onClicked: refreshRequested()
            }
            Button {
                text: "Close"
                Accessible.name: "Close Site Ledger"
                onClicked: closeRequested()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            visible: !!(!browserWindow.siteLedgerData.private && browserWindow.siteLedgerData.origin)
            CheckBox {
                id: siteDataClearConfirmation
                text: "Clear supported site data"
                checked: false
                Accessible.name: "Confirm supported site data clearing"
                Accessible.description: "Clears page-origin local storage, Cache Storage, service-worker registrations, and visible cookies only; HTTP cache and other cookie-store entries remain"
            }
            Button {
                text: "Clear active-origin data"
                enabled: siteDataClearConfirmation.checked && !browserWindow.siteDataClearPending
                Accessible.name: "Clear supported active-origin site data"
                onClicked: {
                    siteDataClearConfirmation.checked = false
                    activeOriginDataClearRequested()
                }
            }
            Label {
                Layout.fillWidth: true
                text: "Per-origin: local storage, Cache Storage, service workers; cookies are page-visible-only; HTTP cache is profile-wide and excluded"
                color: browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
            }
        }

        RowLayout {
            Layout.fillWidth: true
            CheckBox {
                id: siteReportHost
                text: "Include current site host"
                checked: false
                Accessible.name: "Include current site host in report"
                Accessible.description: "Host disclosure is opt-in; paths and query strings are never included"
            }
            Button {
                text: "Copy sanitized report"
                Accessible.name: "Copy sanitized site report"
                onClicked: sanitizedReportCopyRequested(siteReportHost.checked)
            }
            Label {
                Layout.fillWidth: true
                text: "URLs, cookies, tokens, DOM, and account identifiers are excluded"
                color: browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Tab " + (browserWindow.siteLedgerData.capture && browserWindow.siteLedgerData.capture.tab_id || "unknown")
                  + " · document " + (browserWindow.siteLedgerData.capture && browserWindow.siteLedgerData.capture.document_id || "unknown")
                  + " · " + (browserWindow.siteLedgerData.renderer || "renderer unknown")
            color: browserWindow.mutedTextColor
            elide: Text.ElideRight
        }

        ListView {
            id: siteLedgerFacts
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(270, Math.max(66, siteLedgerFacts.contentHeight + 6))
            clip: true
            spacing: 6
            model: browserWindow.siteLedgerData.facts || []
            delegate: Rectangle {
                width: siteLedgerFacts.width
                height: Math.max(66, browserWindow.chromeRowHeight * 5)
                color: browserWindow.surfaceColor
                radius: 3

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 8
                    spacing: 2
                    Label {
                        Layout.fillWidth: true
                        text: modelData.id + " · " + modelData.state + " · " + modelData.capability
                        color: browserWindow.primaryTextColor
                        font.bold: true
                        elide: Text.ElideRight
                        Accessible.name: modelData.id + " fact"
                    }
                    Label {
                        Layout.fillWidth: true
                        text: modelData.provenance + " · " + modelData.scope + " · " + modelData.apply_time
                        color: browserWindow.mutedTextColor
                        elide: Text.ElideRight
                    }
                    Label {
                        Layout.fillWidth: true
                        text: JSON.stringify(modelData.value)
                        color: browserWindow.secondaryTextColor
                        elide: Text.ElideRight
                    }
                }
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Recent request decisions"
            color: browserWindow.primaryTextColor
            font.bold: true
        }

        ListView {
            id: siteLedgerDecisions
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 3
            model: browserWindow.siteLedgerData.blocking
                  ? (browserWindow.siteLedgerData.blocking.active_site_decisions || [])
                  : []
            delegate: Label {
                width: siteLedgerDecisions.width
                text: (modelData.decision || "unknown") + " · "
                      + (modelData.resource_host || "unknown host") + " · "
                      + "list " + (modelData.list_id || "unknown") + " · "
                      + (modelData.exception_list_id
                         ? "exception " + modelData.exception_list_id + " · " : "")
                      + (modelData.reason || "no reason")
                color: modelData.decision === "blocked" ? browserWindow.errorColor : browserWindow.successColor
                elide: Text.ElideRight
                Accessible.name: "Request decision"
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Safe actions: " + ((browserWindow.siteLedgerData.safe_remediation_actions || []).join(", ") || "none")
            color: browserWindow.warningColor
            wrapMode: Text.WordWrap
        }

        Label {
            Layout.fillWidth: true
            visible: browserWindow.siteLedgerData.site_doctor
                     && browserWindow.siteLedgerData.site_doctor.active
                     && browserWindow.siteLedgerData.site_doctor.active.kind
            text: browserWindow.siteLedgerData.site_doctor
                  && browserWindow.siteLedgerData.site_doctor.active
                  ? "Active experiment: "
                    + browserWindow.siteLedgerData.site_doctor.active.kind
                    + " (temporary; "
                    + (browserWindow.siteLedgerData.site_doctor.active.remaining_seconds || 0)
                    + "s remaining; reload/navigation ends it)"
                  : ""
            color: browserWindow.accentColor
            wrapMode: Text.WordWrap
        }

        Label {
            Layout.fillWidth: true
            visible: !!(browserWindow.siteLedgerData.site_doctor
                     && browserWindow.siteLedgerData.site_doctor.last_result
                     && browserWindow.siteLedgerData.site_doctor.last_result.kind)
            text: browserWindow.siteLedgerData.site_doctor
                  && browserWindow.siteLedgerData.site_doctor.last_result
                  ? "Site Doctor proposal: "
                    + browserWindow.siteLedgerData.site_doctor.last_result.kind
                    + " · "
                    + browserWindow.siteLedgerData.site_doctor.last_result.security_effect
                  : ""
            color: browserWindow.warningColor
            wrapMode: Text.WordWrap
            Accessible.name: "Site Doctor durable fix proposal"
        }

        RowLayout {
            Layout.fillWidth: true
            visible: !!(browserWindow.siteLedgerData.site_doctor
                     && browserWindow.siteLedgerData.site_doctor.last_result
                     && browserWindow.siteLedgerData.site_doctor.last_result.kind === "blocker-exception")
            CheckBox {
                id: siteDoctorProposalConfirmation
                text: "I reviewed this host-scoped exception"
                checked: false
                Accessible.name: "Confirm Site Doctor blocker exception"
                Accessible.description: "This saves a generated runtime override for the current host and does not change permissions or TLS"
            }
            Button {
                text: "Apply reviewed fix"
                enabled: siteDoctorProposalConfirmation.checked
                         && browserWindow.siteLedgerData.site_doctor.last_result.state === "pending-confirmation"
                Accessible.name: "Apply reviewed Site Doctor fix"
                onClicked: {
                    siteDoctorProposalConfirmation.checked = false
                    siteDoctorProposalApplyRequested(browserWindow.siteLedgerData.site_doctor.last_result.id)
                }
            }
        }

        Button {
            Layout.alignment: Qt.AlignLeft
            visible: browserWindow.siteLedgerData.site_doctor
                     && browserWindow.siteLedgerData.site_doctor.experiments
                     && browserWindow.siteLedgerData.site_doctor.experiments.indexOf("blocking-bypass") >= 0
            enabled: siteExperimentAvailable
            text: "Try blocker bypass once"
            Accessible.name: "Run one-shot blocker bypass experiment"
            onClicked: siteDoctorExperimentRequested("blocking-bypass")
        }

        Button {
            Layout.alignment: Qt.AlignLeft
            visible: browserWindow.siteLedgerData.site_doctor
                     && browserWindow.siteLedgerData.site_doctor.experiments
                     && browserWindow.siteLedgerData.site_doctor.experiments.indexOf("compiled-defaults") >= 0
            enabled: siteExperimentAvailable
            text: "Try compiled-default site settings once"
            Accessible.name: "Run one-shot compiled-default site settings experiment"
            onClicked: siteDoctorExperimentRequested("compiled-defaults")
        }

        Button {
            Layout.alignment: Qt.AlignLeft
            visible: browserWindow.siteLedgerData.site_doctor
                     && browserWindow.siteLedgerData.site_doctor.experiments
                     && browserWindow.siteLedgerData.site_doctor.experiments.indexOf("userscripts-off") >= 0
            enabled: siteExperimentAvailable
            text: "Try without matching userscripts once"
            Accessible.name: "Run one-shot userscript-free experiment"
            onClicked: siteDoctorExperimentRequested("userscripts-off")
        }

        Button {
            Layout.alignment: Qt.AlignLeft
            visible: browserWindow.siteLedgerData.site_doctor
                     && browserWindow.siteLedgerData.site_doctor.experiments
                     && browserWindow.siteLedgerData.site_doctor.experiments.indexOf("fresh-view") >= 0
            enabled: siteExperimentAvailable
            text: "Open fresh same-profile view once"
            Accessible.name: "Run one-shot fresh same-profile view experiment"
            onClicked: siteDoctorExperimentRequested("fresh-view")
        }
    }
}
