import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/TargetSearchPresentation.js" as Presentation

FerricModalSurface {
    id: search
    required property bool searchVisible
    required property string query
    required property string scope
    required property var rows
    required property string selectedIdentity
    required property string selectedMeaning
    signal queryEdited(string value)
    signal moveRequested(int delta)
    signal selectionRequested(string identity)
    signal confirmRequested()
    signal cancelRequested()
    signal closed()
    property bool wasOpen: false
    visible: searchVisible
    commandText: ":target-search"
    title: "Find a page control"
    message: Presentation.scopeLabel(scope)
    dialogWidth: 660 * scale
    dialogHeight: 520 * scale
    minimumBodyHeight: 140 * scale
    keyHelp: "↑/↓ select  ·  enter activate  ·  esc cancel"
    initialFocusItem: searchInput
    restoreFocus: false
    onOpening: wasOpen = true
    onVisibleChanged: if (!visible && wasOpen) { wasOpen = false; closed() }
    onDismissRequested: cancelRequested()

    function focusInput() { if (interactive) searchInput.forceActiveFocus() }
    function revealSelection() {
        for (var i = 0; i < rows.length; ++i) {
            if (rows[i].identity === selectedIdentity && rows[i].meaning === selectedMeaning) {
                results.positionViewAtIndex(i, ListView.Contain)
                return
            }
        }
    }
    onSelectedIdentityChanged: revealTimer.restart()
    onSelectedMeaningChanged: revealTimer.restart()
    onRowsChanged: revealTimer.restart()
    Timer { id: revealTimer; interval: 0; onTriggered: search.revealSelection() }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * search.scale
        FerricTextField {
            id: searchInput
            objectName: "targetSearchInput"
            Layout.fillWidth: true
            text: search.query
            placeholderText: "Search control text"
            maximumLength: 256
            Accessible.name: "Search visible controls"
            onTextEdited: search.queryEdited(text)
            Keys.onPressed: function(event) {
                if (!search.interactive || event.modifiers !== Qt.NoModifier) return
                if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
                    search.moveRequested(event.key === Qt.Key_Up ? -1 : 1)
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    event.accepted = true
                    if (!event.isAutoRepeat && search.selectedIdentity.length > 0) search.confirmRequested()
                }
            }
        }
        FerricManagerList {
            id: results
            objectName: "targetSearchList"
            Layout.fillWidth: true
            Layout.fillHeight: true
            hostWindow: search.hostWindow
            model: search.rows
            spacing: 4 * search.scale
            Accessible.name: "Visible control results"
            delegate: ItemDelegate {
                id: row
                required property var modelData
                width: results.width
                implicitHeight: rowContent.implicitHeight + 16 * search.scale
                highlighted: modelData.identity === search.selectedIdentity && modelData.meaning === search.selectedMeaning
                Accessible.name: Presentation.kindLabel(modelData.kind) + ": " + modelData.text
                onClicked: { search.selectionRequested(modelData.identity); search.focusInput() }
                contentItem: ColumnLayout {
                    id: rowContent
                    spacing: 4 * search.scale
                    Label {
                        objectName: "targetSearchResultText"
                        Layout.fillWidth: true
                        text: row.modelData.text || "Unnamed control"
                        textFormat: Text.PlainText
                        color: search.browserWindow.primaryTextColor
                        wrapMode: Text.Wrap
                        maximumLineCount: 3
                        elide: Text.ElideRight
                    }
                    Label {
                        Layout.fillWidth: true
                        text: Presentation.kindLabel(row.modelData.kind)
                        textFormat: Text.PlainText
                        color: search.browserWindow.secondaryTextColor
                        wrapMode: Text.Wrap
                    }
                }
                background: Rectangle {
                    color: row.highlighted ? search.browserWindow.selectionColor : "transparent"
                    radius: 3 * search.scale
                }
            }
        }
        Label {
            Layout.fillWidth: true
            text: search.rows.length === 0 ? (search.query.length > 0 ? "No matching controls. Try fewer words." : "No controls are visible in this area.") : search.rows.length + (search.rows.length === 1 ? " control" : " controls")
            textFormat: Text.PlainText
            color: search.browserWindow.secondaryTextColor
            wrapMode: Text.Wrap
        }
        Flow {
            Layout.fillWidth: true
            Layout.preferredHeight: childrenRect.height
            spacing: 8 * search.scale
            Button { text: "Cancel"; onClicked: search.cancelRequested() }
            Button {
                objectName: "targetSearchConfirm"
                text: "Activate"
                Accessible.name: "Activate selected control"
                enabled: search.selectedIdentity.length > 0
                onClicked: search.confirmRequested()
            }
        }
    }
}
