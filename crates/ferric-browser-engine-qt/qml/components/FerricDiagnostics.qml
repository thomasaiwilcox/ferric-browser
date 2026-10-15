import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: diagnostics
    required property string previewText
    required property var summaryRows
    required property bool reportAvailable
    property bool detailsExpanded: false
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
    onDismissRequested: closeRequested()

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * diagnostics.scale
        Flow {
            Layout.fillWidth: true
            Layout.preferredHeight: childrenRect.height
            spacing: 6 * diagnostics.scale
            Button { text: "Refresh"; onClicked: diagnostics.refreshRequested() }
            Button { objectName: "diagnosticsCopy"; text: "Copy preview"; enabled: diagnostics.reportAvailable; onClicked: diagnostics.copyRequested() }
            Button { objectName: "diagnosticsSave"; text: "Save report"; enabled: diagnostics.reportAvailable; onClicked: diagnostics.saveRequested() }
            Button { text: "Close"; onClicked: diagnostics.closeRequested() }
        }
        FerricScrollColumn {
            objectName: "diagnosticsBody"
            Layout.fillWidth: true
            Layout.fillHeight: true
            hostWindow: diagnostics.hostWindow
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
                objectName: "diagnosticsUnavailable"
                Layout.fillWidth: true
                visible: !diagnostics.reportAvailable && diagnostics.exportNotice.length === 0
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
                objectName: "diagnosticsDetailsToggle"
                text: diagnostics.detailsExpanded ? "Hide report details" : "Report details"
                onClicked: diagnostics.detailsExpanded = !diagnostics.detailsExpanded
            }
            TextArea {
                objectName: "diagnosticsReportText"
                Layout.fillWidth: true
                visible: diagnostics.detailsExpanded
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
