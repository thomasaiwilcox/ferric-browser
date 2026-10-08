import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Owns only the isolated stored-archive overlay. The normal browser profile
// and live-page navigation stay outside this component.
Loader {
    id: archiveHost
    required property var browserWindow
    required property var browserUi

    active: String(browserUi.versioning_archive_view_url || "").length > 0
    property int presentedCount: 0
    property int blankedCount: 0

    signal archivePresented()

    function closeArchive() {
        if (!item) return false
        item.closeArchive()
        return true
    }

    sourceComponent: Component {
        Item {
            id: archiveLayer
            anchors.fill: parent
            property string archiveNotice: ""
            property string archiveOperationId: ""
            readonly property bool closeRetryAvailable: archiveSurface.blankWaitExpired

            function closeArchive() { archiveSurface.closeArchive() }

            Shortcut {
                sequence: "Escape"
                context: Qt.WindowShortcut
                enabled: archiveLayer.visible
                         && (!archiveSurface.closingToBlank
                             || archiveSurface.blankWaitExpired)
                onActivated: archiveLayer.closeArchive()
            }

            Rectangle {
                anchors.fill: parent
                color: archiveHost.browserWindow.backgroundColor
            }

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                RowLayout {
                    Layout.fillWidth: true
                    Layout.minimumHeight: 104
                    Layout.margins: 8

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1

                        Label {
                            Layout.fillWidth: true
                            text: archiveLayer.archiveNotice.length > 0
                                  ? archiveLayer.archiveNotice
                                  : archiveSurface.archiveTitle.length > 0
                                  ? archiveSurface.archiveTitle : "Stored archive"
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            Accessible.role: Accessible.Heading
                        }

                        Label {
                            Layout.fillWidth: true
                            text: "Offline historical copy · never a live session · Captured "
                                  + (Number(archiveHost.browserUi.versioning_archive_captured_at_ms) > 0
                                     ? new Date(Number(archiveHost.browserUi.versioning_archive_captured_at_ms)).toLocaleString()
                                     : "time unavailable")
                            textFormat: Text.PlainText
                            color: archiveHost.browserWindow.mutedTextColor
                        }

                        Label {
                            Layout.fillWidth: true
                            // INVARIANT: inert metadata, never a navigation target.
                            text: "Source (metadata only): "
                                  + (String(archiveHost.browserUi.versioning_archive_source_url || "").length > 0
                                     ? String(archiveHost.browserUi.versioning_archive_source_url)
                                     : "unavailable")
                            textFormat: Text.PlainText
                            elide: Text.ElideMiddle
                            color: archiveHost.browserWindow.mutedTextColor
                            Accessible.name: "Recorded source URL, inert metadata only"
                            Accessible.description: "May contain private URL parameters. It is not clickable and never opens the live source."
                        }

                        Label {
                            Layout.fillWidth: true
                            text: String(archiveHost.browserUi.versioning_archive_fidelity_label || "Fidelity unavailable")
                            textFormat: Text.PlainText
                            color: archiveHost.browserWindow.mutedTextColor
                        }
                    }

                    Button {
                        text: archiveSurface.closingToBlank
                              ? (archiveSurface.blankWaitExpired ? "Retry close" : "Closing…")
                              : "Close archive"
                        enabled: !archiveSurface.closingToBlank || archiveSurface.blankWaitExpired
                        Accessible.name: "Close isolated stored archive"
                        onClicked: archiveLayer.closeArchive()
                    }
                }

                FerricVersioningArchiveView {
                    id: archiveSurface
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    authorizedArchiveUrl: archiveHost.browserUi.versioning_archive_view_url

                    onArchiveReadyChanged: {
                        if (archiveReady) {
                            archiveLayer.archiveOperationId =
                                String(archiveHost.browserUi.versioning_archive_view_operation || "")
                            Qt.callLater(openAuthorizedArchive)
                        }
                    }
                    onArchiveLoaded: {
                        archiveHost.presentedCount++
                        archiveHost.archivePresented()
                    }
                    onArchiveLoadFailed: {
                        archiveLayer.archiveNotice =
                            "Archive could not be displayed safely; closing offline copy…"
                        closeArchive()
                    }
                    onArchiveBlankingTimedOut: archiveLayer.archiveNotice =
                        "Blanking timed out; temporary file retained. Retry close or close the window."
                    onArchiveBlanked: {
                        archiveHost.blankedCount++
                        const operation = archiveLayer.archiveOperationId
                        Qt.callLater(function() {
                        if (archiveHost.browserUi.close_versioned_archive_view(operation))
                            archiveHost.browserWindow.scheduleRuntimeWork(0)
                        else
                            archiveLayer.archiveNotice =
                                "Archive is blank; temporary-file cleanup could not be scheduled. Retry close."
                        })
                    }
                }
            }
        }
    }
}
