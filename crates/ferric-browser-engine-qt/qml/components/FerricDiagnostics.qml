import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: diagnostics
    objectName: "browserDiagnostics"
    required property string previewText
    required property var summaryRows
    required property bool reportAvailable
    property bool detailsExpanded: false
    property bool reportPending: false
    property string reportError: ""
    property string startupDetails: ""
    property bool startupDetailsExpanded: false
    property string exportNotice: ""
    property string exportDetail: ""
    signal closeRequested()
    signal refreshRequested()
    signal copyRequested()
    signal saveRequested()
    visible: browserWindow.diagnosticsVisible
    commandText: ":diagnostics"
    title: "Diagnostics"
    message: ""
    keyHelp: "tab/shift-tab controls  ·  esc close"
    dialogWidth: 900 * scale
    dialogHeight: 620 * scale
    stackingOrder: 72
    onOpening: { detailsExpanded = false; startupDetailsExpanded = false }
    onDismissRequested: closeRequested()

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * diagnostics.scale
        Flow {
            Layout.fillWidth: true
            Layout.preferredHeight: childrenRect.height
            spacing: 6 * diagnostics.scale
            Button { objectName: "diagnosticsRefresh"; text: "Refresh"; enabled: !diagnostics.reportPending; onClicked: diagnostics.refreshRequested() }
            Button { objectName: "diagnosticsCopy"; text: "Copy preview"; enabled: diagnostics.reportAvailable && !diagnostics.reportPending; onClicked: diagnostics.copyRequested() }
            Button { objectName: "diagnosticsSave"; text: "Save report"; enabled: diagnostics.reportAvailable && !diagnostics.reportPending; onClicked: diagnostics.saveRequested() }
            Button { objectName: "diagnosticsClose"; text: "Close"; onClicked: diagnostics.closeRequested() }
        }
        FerricScrollColumn {
            objectName: "diagnosticsBody"
            Layout.fillWidth: true
            Layout.fillHeight: true
            hostWindow: diagnostics.hostWindow
            ColumnLayout {
                id: detailsColumn
                objectName: "diagnosticsContent"
                Layout.fillWidth: true
                Layout.fillHeight: false
                Layout.alignment: Qt.AlignTop
                spacing: 8 * diagnostics.scale
                FerricManagerFeedback {
                    objectName: "diagnosticsCollectionFeedback"
                    Layout.fillWidth: true
                    browserWindow: diagnostics.browserWindow
                    message: diagnostics.reportPending ? "Preparing report…"
                        : diagnostics.reportError.length > 0 ? "The report couldn't be prepared. Refresh to try again." : ""
                    detail: diagnostics.reportError
                    pending: diagnostics.reportPending
                    failed: diagnostics.reportError.length > 0
                }
                Repeater {
                    model: diagnostics.summaryRows
                    Label {
                        required property string modelData
                        Layout.fillWidth: true
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: diagnostics.browserWindow.primaryTextColor
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: "Review report details before sharing. Unmeasured features haven't been verified."
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: diagnostics.browserWindow.secondaryTextColor
                }
                Label {
                    Layout.fillWidth: true
                    text: "Interface scale: " + diagnostics.browserWindow.displayScaleLabel
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: diagnostics.browserWindow.secondaryTextColor
                }
                Label {
                    textFormat: Text.PlainText
                    Layout.fillWidth: true
                    text: diagnostics.browserWindow.reducedMotionActive ? "Interface animations: reduced" : "Interface animations: enabled"
                    color: diagnostics.browserWindow.secondaryTextColor
                    wrapMode: Text.Wrap
                }
                Label {
                    Layout.fillWidth: true
                    visible: diagnostics.browserWindow.themeContrastWarning.length > 0
                    text: "Theme contrast: " + diagnostics.browserWindow.themeContrastWarning
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: diagnostics.browserWindow.warningColor
                }
                Label {
                    textFormat: Text.PlainText
                    objectName: "diagnosticsUnavailable"
                    Layout.fillWidth: true
                    visible: !diagnostics.reportAvailable && !diagnostics.reportPending
                             && diagnostics.reportError.length === 0 && diagnostics.exportNotice.length === 0
                    text: "A report isn't ready. Refresh to try again."
                    wrapMode: Text.Wrap
                    color: diagnostics.browserWindow.warningColor
                }
                FerricManagerFeedback {
                    objectName: "diagnosticsExportNotice"
                    Layout.fillWidth: true
                    browserWindow: diagnostics.browserWindow
                    message: diagnostics.exportNotice
                    detail: diagnostics.exportDetail
                    failed: diagnostics.exportDetail.length > 0
                }
                ToolButton {
                    objectName: "diagnosticsStartupToggle"
                    visible: diagnostics.startupDetails.length > 0
                    text: diagnostics.startupDetailsExpanded ? "Hide local startup details" : "Local startup details"
                    onClicked: diagnostics.startupDetailsExpanded = !diagnostics.startupDetailsExpanded
                }
                Label {
                    Layout.fillWidth: true
                    visible: diagnostics.startupDetailsExpanded && diagnostics.startupDetails.length > 0
                    text: "These details may include local paths or settings. They are excluded from copied and saved reports."
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: diagnostics.browserWindow.secondaryTextColor
                }
                TextArea {
                    objectName: "diagnosticsStartupText"
                    Layout.fillWidth: true
                    visible: diagnostics.startupDetailsExpanded && diagnostics.startupDetails.length > 0
                    text: diagnostics.startupDetails
                    textFormat: TextEdit.PlainText
                    wrapMode: TextEdit.Wrap
                    readOnly: true
                    selectByMouse: true
                    selectByKeyboard: true
                    color: diagnostics.browserWindow.primaryTextColor
                    background: Rectangle { color: diagnostics.browserWindow.surfaceColor }
                    Accessible.name: "Local startup details"
                }
                ToolButton {
                    objectName: "diagnosticsDetailsToggle"
                    text: diagnostics.detailsExpanded ? "Hide report details" : "Report details"
                    enabled: diagnostics.previewText.length > 0
                    onClicked: diagnostics.detailsExpanded = !diagnostics.detailsExpanded
                }
                TextArea {
                    objectName: "diagnosticsReportText"
                    Layout.fillWidth: true
                    visible: diagnostics.detailsExpanded && diagnostics.previewText.length > 0
                    text: diagnostics.previewText
                    textFormat: TextEdit.PlainText
                    wrapMode: TextEdit.Wrap
                    readOnly: true
                    selectByMouse: true
                    selectByKeyboard: true
                    color: diagnostics.browserWindow.primaryTextColor
                    background: Rectangle { color: diagnostics.browserWindow.surfaceColor }
                    font.family: diagnostics.browserWindow.chromeFontFamily
                    Accessible.name: "Diagnostic report preview"
                }
            }
        }
    }
}
