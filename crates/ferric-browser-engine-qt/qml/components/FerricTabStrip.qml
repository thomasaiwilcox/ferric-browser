import QtQuick
import QtQuick.Window
import QtQuick.Controls
import QtQuick.Layouts

GridLayout {
    id: tabStrip
    required property var browserWindow
    required property var tabsModel
    required property int activeTabIndex
    property bool vertical: false
    property bool tooltipsEnabled: true
    property var tooltipRow: null
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

    // One tooltip per strip; page titles always remain literal text.
    ToolTip {
        id: titleTooltip
        objectName: "tabTitleTooltip"
        parent: tabStrip.tooltipRow || tabStrip
        visible: tabStrip.tooltipsEnabled && !!tabStrip.tooltipRow
                 && tabStrip.tooltipRow.titleElided
        delay: 600
        text: tabStrip.tooltipRow ? tabStrip.tooltipRow.title : ""
        font: tabStrip.browserWindow.font
        readonly property var hostWindow: tabStrip.Window.window
        readonly property point anchorPosition: tabStrip.tooltipRow && hostWindow
            ? tabStrip.tooltipRow.mapToItem(hostWindow.contentItem,0,0) : Qt.point(0,0)
        readonly property bool anchorOnLeft: !hostWindow || anchorPosition.x < hostWindow.width / 2
        readonly property real maximumWidth: {
            var width = hostWindow ? hostWindow.width : tabStrip.width
            var space = width - 16
            if (tabStrip.vertical && tabStrip.tooltipRow)
                space = anchorOnLeft ? width-anchorPosition.x-tabStrip.tooltipRow.width-12 : anchorPosition.x-12
            return Math.max(1,Math.min(720,space))
        }
        readonly property real maximumHeight: Math.max(1, (hostWindow ? hostWindow.height : 700) / 2 - 16)
        width: Math.min(implicitWidth, maximumWidth)
        height: Math.min(implicitHeight, maximumHeight)
        x: tabStrip.vertical ? (anchorOnLeft ? parent.width+6 : -width-6) : (parent.width-width)/2
        y: tabStrip.vertical ? (parent.height-height)/2
           : hostWindow && anchorPosition.y > hostWindow.height/2 ? -height-6 : parent.height+6
        FontMetrics { id: tooltipMetrics; font: titleTooltip.font }
        contentItem: Label {
            text: titleTooltip.text
            textFormat: Text.PlainText
            font: titleTooltip.font
            wrapMode: Text.Wrap
            maximumLineCount: Math.max(1, Math.min(8, Math.floor((titleTooltip.maximumHeight
                - titleTooltip.topPadding - titleTooltip.bottomPadding) / tooltipMetrics.height)))
            elide: Text.ElideRight
            color: tabStrip.browserWindow.primaryTextColor
        }
        background: Rectangle {
            color: tabStrip.browserWindow.panelColor
            border.color: tabStrip.browserWindow.borderColor
            radius: 3
        }
    }

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

        WheelHandler {
            target: null
            enabled: tabList.count > 1
            onWheel: function(event) {
                var delta = event.angleDelta.y !== 0
                            ? event.angleDelta.y : -event.angleDelta.x
                if (delta === 0 || tabStrip.activeTabIndex < 0) {
                    return
                }
                var step = delta < 0 ? 1 : -1
                tabStrip.selectRequested(
                    (tabStrip.activeTabIndex + step + tabList.count) % tabList.count)
                event.accepted = true
            }
        }

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
            readonly property bool titleElided: tabTitle.implicitWidth > tabTitle.width
            Component.onDestruction: if (tabStrip.tooltipRow === tabRow) tabStrip.tooltipRow = null

            HoverHandler {
                id: tabHover
                onHoveredChanged: {
                    if (hovered) tabStrip.tooltipRow = tabRow
                    else if (tabStrip.tooltipRow === tabRow) tabStrip.tooltipRow = null
                }
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
                textFormat: Text.PlainText
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
