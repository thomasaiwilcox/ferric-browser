import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: switcherResults
    required property var browserWindow
    required property var results
    readonly property int resultCount: resultList.count
    readonly property int currentIndex: resultList.currentIndex
    signal activationRequested(int index)
    signal actionRequested(int index, string action)

    function moveBy(delta) {
        if (resultList.count <= 0) {
            return
        }
        var next = Math.max(0, Math.min(resultList.count - 1,
                                        resultList.currentIndex + delta))
        resultList.positionViewAtIndex(next, ListView.Beginning)
        resultList.currentIndex = next
    }

    function moveToBeginning() {
        if (resultList.count > 0) {
            resultList.currentIndex = 0
            resultList.positionViewAtBeginning()
        }
    }

    function moveToEnd() {
        if (resultList.count > 0) {
            resultList.currentIndex = resultList.count - 1
            resultList.positionViewAtEnd()
        }
    }

    ListView {
        id: resultList
        anchors.fill: parent
        clip: true
        focus: true
        model: switcherResults.results
        currentIndex: count > 0 ? 0 : -1
        Accessible.role: Accessible.List
        Accessible.name: "Switcher results"
        Accessible.description: count + " results; selected result is announced with its kind and label"
        delegate: Rectangle {
            id: resultRow
            required property var modelData
            width: resultList.width
            height: Math.max(switcherResults.browserWindow.chromeRowHeight * 5,
                             resultColumn.implicitHeight + switcherResults.browserWindow.chromeRowHeight)
            readonly property var resultData: modelData
            readonly property int resultIndex: index
            color: index === resultList.currentIndex
                   ? switcherResults.browserWindow.selectionColor : "transparent"
            Accessible.role: Accessible.ListItem
            Accessible.selected: index === resultList.currentIndex
            Accessible.name: resultData.kind + " " + resultData.label
            Accessible.description: (resultData.profile || "")
                + (resultData.secondary ? " · " + resultData.secondary : "")
                + (index === resultList.currentIndex ? " · selected" : "")

            ColumnLayout {
                id: resultColumn
                anchors.fill: parent
                anchors.margins: 6
                spacing: 2
                Label {
                    Layout.fillWidth: true
                    color: switcherResults.browserWindow.primaryTextColor
                    text: "[" + resultRow.resultData.kind + "] " + resultRow.resultData.label
                    elide: Text.ElideRight
                }
                Label {
                    Layout.fillWidth: true
                    color: switcherResults.browserWindow.mutedTextColor
                    text: (resultRow.resultData.profile || "")
                          + (resultRow.resultData.secondary
                             ? " · " + resultRow.resultData.secondary : "")
                    elide: Text.ElideRight
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    Repeater {
                        model: resultRow.resultData.actions || []
                        delegate: Button {
                            required property var modelData
                            text: modelData
                            Accessible.name: modelData + " " + resultRow.resultData.kind
                            onClicked: switcherResults.actionRequested(resultRow.resultIndex, modelData)
                        }
                    }
                    Item { Layout.fillWidth: true }
                }
            }

            MouseArea {
                anchors.fill: parent
                onClicked: switcherResults.activationRequested(resultRow.resultIndex)
            }
        }
    }
}
