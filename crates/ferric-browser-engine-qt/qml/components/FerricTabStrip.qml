import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

GridLayout {
    id: tabStrip
    required property var browserWindow
    required property var tabsModel
    required property int activeTabIndex
    property bool vertical: false
    signal selectRequested(int index)
    signal closeRequested(int index)
    signal newRequested()

    columns: vertical ? 1 : 2
    rows: vertical ? 2 : 1
    rowSpacing: 1
    columnSpacing: 1
    Accessible.role: Accessible.PageTabList
    Accessible.name: "Browser tabs"
    Accessible.description: tabList.count + " browser tabs"

    function ensureActiveTabVisible() {
        if (visible && activeTabIndex >= 0 && activeTabIndex < tabList.count) {
            tabList.positionViewAtIndex(activeTabIndex, ListView.Contain)
        }
    }

    onActiveTabIndexChanged: Qt.callLater(ensureActiveTabVisible)
    onVerticalChanged: Qt.callLater(ensureActiveTabVisible)
    onVisibleChanged: Qt.callLater(ensureActiveTabVisible)

    ListView {
        id: tabList
        objectName: "tabList"
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.minimumWidth: 0
        Layout.minimumHeight: 0
        orientation: tabStrip.vertical ? ListView.Vertical : ListView.Horizontal
        boundsBehavior: Flickable.StopAtBounds
        clip: true
        spacing: 1
        model: tabStrip.tabsModel
        currentIndex: tabStrip.activeTabIndex
        onCountChanged: Qt.callLater(tabStrip.ensureActiveTabVisible)
        onWidthChanged: Qt.callLater(tabStrip.ensureActiveTabVisible)
        onHeightChanged: Qt.callLater(tabStrip.ensureActiveTabVisible)

        delegate: Rectangle {
            id: tabRow
            required property int index
            required property string title
            required property bool pinned
            required property bool muted
            readonly property int tabIndex: index
            width: tabStrip.vertical ? tabList.width
                   : Math.min(tabList.width,
                              Math.min(400,
                                       Math.max(220, tabList.width
                                                / Math.max(1, tabList.count))))
            height: tabStrip.browserWindow.tabBarHeight
            color: tabIndex === tabStrip.activeTabIndex
                   ? tabStrip.browserWindow.surfaceColor
                   : tabStrip.browserWindow.backgroundColor

            Accessible.role: Accessible.PageTab
            Accessible.name: "Tab " + (tabIndex + 1) + ": "
                             + (tabRow.title || "New tab")
            Accessible.selected: tabIndex === tabStrip.activeTabIndex
            ToolTip.visible: tabHover.hovered && tabTitle.implicitWidth > tabTitle.width
            ToolTip.delay: 600
            ToolTip.text: tabRow.title || "New tab"

            HoverHandler {
                id: tabHover
            }

            MouseArea {
                objectName: "tabSelectArea"
                anchors.fill: parent
                onClicked: tabStrip.selectRequested(tabRow.tabIndex)
            }

            Text {
                id: tabTitle
                anchors.left: parent.left
                anchors.leftMargin: 9
                anchors.right: tabClose.left
                anchors.rightMargin: 4
                anchors.verticalCenter: parent.verticalCenter
                color: tabStrip.browserWindow.readableTextColor(
                           tabStrip.browserWindow.primaryTextColor, tabRow.color)
                font.family: tabStrip.browserWindow.font.family
                font.pointSize: tabStrip.browserWindow.font.pointSize
                font.bold: tabRow.tabIndex === tabStrip.activeTabIndex
                text: (tabIndex + 1) + "  "
                      + (tabRow.pinned ? "◆ " : "")
                      + (tabRow.muted ? "[M] " : "")
                      + (tabRow.title || "New tab")
                elide: Text.ElideRight
            }

            ToolButton {
                id: tabClose
                objectName: "tabCloseButton"
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: visible ? tabStrip.browserWindow.tabBarHeight : 0
                height: parent.height
                text: "×"
                padding: 0
                visible: tabRow.tabIndex === tabStrip.activeTabIndex
                         || tabHover.hovered
                background: Item {}
                contentItem: Text {
                    text: tabClose.text
                    color: tabStrip.browserWindow.readableTextColor(
                               tabStrip.browserWindow.primaryTextColor, tabRow.color)
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
                Accessible.name: "Close tab " + (tabRow.tabIndex + 1)
                onClicked: tabStrip.closeRequested(tabRow.tabIndex)
            }

            Rectangle {
                width: tabStrip.vertical ? 2 : parent.width
                height: tabStrip.vertical ? parent.height : 2
                anchors.left: parent.left
                anchors.bottom: parent.bottom
                color: tabStrip.browserWindow.accentColor
                visible: tabRow.tabIndex === tabStrip.activeTabIndex
            }
        }
    }

    ToolButton {
        id: newTabButton
        objectName: "newTabButton"
        text: "+"
        Layout.fillHeight: !tabStrip.vertical
        Layout.fillWidth: tabStrip.vertical
        Layout.preferredWidth: tabStrip.vertical
                               ? tabStrip.width : tabStrip.browserWindow.tabBarHeight
        Layout.maximumWidth: tabStrip.vertical
                             ? tabStrip.width : tabStrip.browserWindow.tabBarHeight
        Layout.preferredHeight: tabStrip.browserWindow.tabBarHeight
        padding: 0
        background: Rectangle {
            color: parent.hovered ? tabStrip.browserWindow.surfaceColor
                                  : tabStrip.browserWindow.backgroundColor
        }
        contentItem: Text {
            text: parent.text
            color: tabStrip.browserWindow.readableTextColor(
                       tabStrip.browserWindow.primaryTextColor,
                       newTabButton.hovered ? tabStrip.browserWindow.surfaceColor
                                            : tabStrip.browserWindow.backgroundColor)
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
        Accessible.name: "New tab"
        onClicked: tabStrip.newRequested()
    }
}
