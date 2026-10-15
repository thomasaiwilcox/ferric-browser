import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Pane {
    id: page
    padding: 0
    font.pointSize: fontSize
    required property var browserUi
    required property var browserWindow
    required property var browserHost
    required property var contentView
    property string selectedView: browserUi.start_page_view || "guide"
    property bool initialSnapshot: false
    property bool viewChosen: false
    property bool refreshTopRequested: true
    property int requestedRankRevision: -1
    property bool menuActive: false
    property var activeControl: null
    property var topTiles: [null, null, null, null, null, null, null, null]
    property var favouriteTiles: [null, null, null, null, null, null, null, null]
    property var bookmarks: []
    property var dismissed: []
    property int selectedSlot: 0
    property string capturedUrl: ""
    property string capturedTitle: ""
    property string editingAction: "pin"
    property string editBookmarkId: ""
    property int editingSlot: 1
    property alias guideSurface: guide
    readonly property bool inputActive: menuActive || editor.visible || dismissedDialog.visible
        || (activeControl && activeControl.activeFocus) || (selectedView === "guide" && guide.inputActive)
    background: Rectangle { color: page.browserWindow.backgroundColor }
    Accessible.name: "Ferric start page"
    Accessible.role: Accessible.Pane

    function rebuild() {
        var top = [null, null, null, null, null, null, null, null]
        var fav = [null, null, null, null, null, null, null, null]
        var saved = [], hidden = []
        var kinds = browserUi.start_page_kinds || []
        for (var i = 0; i < kinds.length; ++i) {
            var tile = { id: browserUi.start_page_ids[i], url: browserUi.start_page_urls[i], title: browserUi.start_page_titles[i], site: browserUi.start_page_sites[i] }
            var slot = Number(browserUi.start_page_slots[i]) - 1
            if (kinds[i] === "top" && slot >= 0 && slot < 8) top[slot] = tile
            else if (kinds[i] === "favourite" && slot >= 0 && slot < 8) fav[slot] = tile
            else if (kinds[i] === "bookmark") saved.push(tile)
            else if (kinds[i] === "dismissed") hidden.push(tile.site)
        }
        favouriteTiles = fav; bookmarks = saved; dismissed = hidden
        if ((!initialSnapshot || refreshTopRequested) && browserUi.start_page_rank_revision > requestedRankRevision) { topTiles = top; refreshTopRequested = false }
        else {
            // Explicit dismissals remove a tile in place; background rankings never reorder it.
            topTiles = topTiles.map(function(tile) { return tile && hidden.indexOf(tile.site) < 0 && (browserUi.start_page_eligible_sites || []).indexOf(tile.site) >= 0 && (browserUi.start_page_eligible_urls || []).indexOf(tile.url) >= 0 ? tile : null })
        }
        if (!initialSnapshot && browserUi.start_page_ready) {
            if (!viewChosen) selectedView = browserUi.start_page_view || "guide"
            initialSnapshot = true
            if (visible && !inputActive) scheduleContentFocus(true)
        }
    }
    function refresh() { requestedRankRevision = browserUi.start_page_rank_revision; refreshTopRequested = true; browserUi.refresh_start_page() }
    function chooseView(view, remember) {
        selectedView = view
        viewChosen = true
        if (remember !== false) browserUi.start_page_action("view", 0, view, "")
        scheduleContentFocus(false)
    }
    function cycleView() {
        var views = ["top-sites", "favourites", "guide"]
        chooseView(views[(views.indexOf(selectedView) + 1) % 3])
    }
    function controlFocusChanged(control, focused) {
        if (focused) activeControl = control
        else if (activeControl === control) activeControl = null
    }
    function scheduleContentFocus(requireIdle) {
        focusTimer.requireIdle = requireIdle
        focusTimer.restart()
    }
    Timer {
        id: focusTimer
        property bool requireIdle: true
        interval: 0
        onTriggered: if (page.visible && !editor.visible && !dismissedDialog.visible
                         && (!requireIdle || !page.inputActive)) page.focusContent()
    }
    Timer { id: refreshRequestTimer; interval: 0; onTriggered: if (page.visible) page.refresh() }
    function focusContent() {
        if (selectedView === "guide") { if (guide.readerActive) guide.focusReader(); else guide.focusEntries(); return }
        var item = tileRepeater.itemAt(selectedSlot)
        if (!item) return
        item.focusTile()
        var flickable = siteScroll.contentItem
        if (flickable && flickable.contentY !== undefined) {
            var position = item.mapToItem(flickable, 0, 0)
            if (position.y < 0) flickable.contentY = Math.max(0, flickable.contentY + position.y)
            else if (position.y + item.height > flickable.height) flickable.contentY = Math.min(Math.max(0, flickable.contentHeight - flickable.height), flickable.contentY + position.y + item.height - flickable.height)
        }
    }
    function activate(slot) {
        var tile = (selectedView === "favourites" ? favouriteTiles : topTiles)[slot]
        if (tile) { browserUi.navigate(tile.url); contentView.setContentUrl(browserUi.initial_url, false); browserWindow.completeBrowserKeyAction(browserUi, browserHost) }
    }
    function editSlot(slot, action, tile) {
        editingSlot = slot + 1; editingAction = action; editBookmarkId = ""
        editUrl.text = tile ? tile.url : ""
        editTitle.text = tile ? tile.title : ""
        bookmarkPicker.currentIndex = 0
        editor.open()
    }
    function addCaptured(url, title) {
        capturedUrl = url; capturedTitle = title; chooseView("favourites", false)
        var empty = favouriteTiles.findIndex(function(tile) { return tile === null })
        editSlot(empty >= 0 ? empty : selectedSlot, "pin", { url: url, title: title })
    }
    function handleNavigation(event) {
        if (inputActive) return false
        if (selectedView === "guide") return guide.handleNavigation(event)
        var focused = tileRepeater.itemAt(selectedSlot)
        if (!focused || !focused.tileFocused || !browserUi.start_page_key_available()) return false
        if (event.modifiers !== Qt.NoModifier) return false
        if (event.key >= Qt.Key_1 && event.key <= Qt.Key_8) {
            if (!event.isAutoRepeat) activate(event.key - Qt.Key_1)
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (!event.isAutoRepeat) {
                if (selectedView === "favourites" && !favouriteTiles[selectedSlot]) editSlot(selectedSlot, "pin", null)
                else activate(selectedSlot)
            }
        } else if (event.key === Qt.Key_H || event.key === Qt.Key_Left) selectedSlot = Math.max(0, selectedSlot - 1)
        else if (event.key === Qt.Key_L || event.key === Qt.Key_Right) selectedSlot = Math.min(7, selectedSlot + 1)
        else if (event.key === Qt.Key_J || event.key === Qt.Key_Down) selectedSlot = Math.min(7, selectedSlot + tileGrid.columns)
        else if (event.key === Qt.Key_K || event.key === Qt.Key_Up) selectedSlot = Math.max(0, selectedSlot - tileGrid.columns)
        else if (event.key === Qt.Key_Escape) browserUi.escape()
        else return false
        focusContent(); event.accepted = true; return true
    }
    Component.onCompleted: { if (browserUi.start_page_revision > 0) rebuild(); refresh(); scheduleContentFocus(true) }
    Timer {
        interval: 5000; running: page.visible; repeat: true
        onTriggered: if (!page.browserUi.start_page_busy) page.browserUi.refresh_start_page_metadata()
    }
    Connections {
        target: page.browserUi
        function onStart_page_revisionChanged() { page.rebuild() }
        function onStart_page_profile_revisionChanged() {
            page.initialSnapshot = false; page.viewChosen = false; page.selectedView = "guide"
            page.topTiles = [null, null, null, null, null, null, null, null]
            page.favouriteTiles = [null, null, null, null, null, null, null, null]
            page.bookmarks = []; page.dismissed = []; page.refreshTopRequested = true
            page.requestedRankRevision = page.browserUi.start_page_rank_revision
            if (editor.visible) editor.close()
            if (dismissedDialog.visible) dismissedDialog.close()
            if (page.visible) refreshRequestTimer.restart()
        }
    }
    readonly property color canvasColor: browserWindow.backgroundColor
    readonly property color inkColor: browserWindow.primaryTextColor
    readonly property color accentColor: browserWindow.accentColor
    readonly property color cardColor: Qt.tint(canvasColor, Qt.rgba(inkColor.r, inkColor.g, inkColor.b, 0.045))
    readonly property int gridColumns: contentFrame.width >= 860 ? 4 : contentFrame.width >= 430 ? 2 : 1
    Item {
        id: contentFrame
        width: Math.min(1080, Math.max(0, page.width - (page.width < 600 ? 32 : 72)))
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.topMargin: page.height < 600 ? 20 : 36
        anchors.bottomMargin: 20
        ColumnLayout {
            anchors.fill: parent
            spacing: page.height < 600 ? 18 : 26
            GridLayout {
                Layout.fillWidth: true
                columns: contentFrame.width > 620 ? 2 : 1
                rowSpacing: 16
                Label {
                    Layout.fillWidth: true
                    text: "Ferric"
                    color: page.inkColor
                    font.pointSize: page.fontSize + 9
                    font.weight: Font.DemiBold
                    font.letterSpacing: -0.5
                }
                Rectangle {
                    Layout.alignment: contentFrame.width > 620 ? Qt.AlignRight : Qt.AlignLeft
                    Layout.fillWidth: contentFrame.width <= 620
                    implicitWidth: modes.implicitWidth + 8
                    implicitHeight: Math.max(46, modes.implicitHeight + 8)
                    color: page.cardColor
                    radius: 12
                    RowLayout {
                        id: modes
                        anchors.fill: parent
                        anchors.margins: 4
                        spacing: 2
                        Repeater {
                            model: [{ label: "Top Sites", view: "top-sites" }, { label: "Favourites", view: "favourites" }, { label: "Guide", view: "guide" }]
                            FerricStartButton {
                                required property var modelData
                                objectName: "startViewControl" + modelData.view
                                Layout.fillWidth: true
                                text: modelData.label
                                quiet: true
                                checkable: true
                                autoExclusive: true
                                checked: page.selectedView === modelData.view
                                browserWindow: page.browserWindow
                                Accessible.name: modelData.label + (checked ? ", selected" : "")
                                onActiveFocusChanged: page.controlFocusChanged(this, activeFocus)
                                onClicked: page.chooseView(modelData.view)
                            }
                        }
                    }
                }
            }
            GridLayout {
                Layout.fillWidth: true
                columns: contentFrame.width > 620 ? 2 : 1
                rowSpacing: 12
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 7
                    Label {
                        Layout.fillWidth: true
                        text: page.selectedView === "guide" ? "A guide to Ferric" : page.selectedView === "favourites" ? "Your favourites" : "Your top sites"
                        color: page.inkColor
                        font.pointSize: page.fontSize + 14
                        font.weight: Font.DemiBold
                        wrapMode: Text.WordWrap
                    }
                    Label {
                        Layout.fillWidth: true
                        text: page.selectedView === "guide" ? "Explore features and the shortcuts configured for you."
                              : page.selectedView === "favourites" ? "The places you choose, in the order you choose."
                              : "The places you come back to."
                        color: page.browserWindow.secondaryTextColor
                        font.pointSize: page.fontSize + 1
                        wrapMode: Text.WordWrap
                    }
                }
                RowLayout {
                    visible: page.selectedView !== "guide"
                    Layout.alignment: contentFrame.width > 620 ? Qt.AlignRight | Qt.AlignVCenter : Qt.AlignLeft
                    spacing: 6
                    FerricStartButton {
                        visible: page.selectedView === "top-sites"
                        text: page.dismissed.length > 0 ? "Hidden sites (" + page.dismissed.length + ")" : "Hidden sites"
                        quiet: true
                        browserWindow: page.browserWindow
                        onClicked: dismissedDialog.open()
                        onActiveFocusChanged: page.controlFocusChanged(this, activeFocus)
                    }
                    FerricStartButton {
                        visible: page.selectedView === "top-sites"
                        text: page.browserUi.start_page_busy ? "Refreshing…" : "Refresh"
                        enabled: !page.browserUi.start_page_busy
                        browserWindow: page.browserWindow
                        onClicked: page.refresh()
                        onActiveFocusChanged: page.controlFocusChanged(this, activeFocus)
                    }
                    FerricStartButton {
                        visible: page.selectedView === "favourites"
                        text: "Add favourite"
                        prominent: true
                        browserWindow: page.browserWindow
                        onClicked: page.addCaptured("", "")
                        onActiveFocusChanged: page.controlFocusChanged(this, activeFocus)
                    }
                }
            }
            Label {
                visible: page.contentView.viewTransientProfile
                Layout.fillWidth: true
                text: "Private session · favourites and view choices are temporary"
                color: page.browserWindow.privateColor
                font.pointSize: page.fontSize
                wrapMode: Text.WordWrap
            }
            Label {
                visible: page.browserUi.start_page_error.length > 0
                Layout.fillWidth: true
                text: page.browserUi.start_page_error
                color: page.browserWindow.errorColor
                wrapMode: Text.WordWrap
            }
            ScrollView {
                id: siteScroll
                visible: page.selectedView !== "guide"
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ColumnLayout {
                    width: siteScroll.availableWidth
                    spacing: 20
                    Label {
                        visible: page.selectedView === "top-sites" && page.topTiles.every(function(tile) { return tile === null })
                        Layout.fillWidth: true
                        text: "Your sites will appear as you browse. You can also save a few favourites to get started."
                        color: page.browserWindow.mutedTextColor
                        wrapMode: Text.WordWrap
                        font.pointSize: page.fontSize
                    }
                    GridLayout {
                        id: tileGrid
                        Layout.fillWidth: true
                        columns: page.gridColumns
                        columnSpacing: 16
                        rowSpacing: 16
                        Repeater {
                            id: tileRepeater
                            model: 8
                            delegate: Item {
                                id: tileItem
                                required property int index
                                Layout.fillWidth: true
                                Layout.preferredWidth: 1
                                Layout.preferredHeight: Math.max(172, page.fontSize * 16)
                                property var tile: (page.selectedView === "favourites" ? page.favouriteTiles : page.topTiles)[index]
                                readonly property bool tileFocused: tileButton.activeFocus
                                function focusTile() { tileButton.forceActiveFocus() }
                                Button {
                                    id: tileButton
                                    anchors.fill: parent
                                    padding: 18
                                    hoverEnabled: true
                                    Accessible.name: "Slot " + (index + 1) + ": " + (tile ? (tile.title || tile.site) + ", press " + (index + 1) + " to open " + tile.url : page.selectedView === "favourites" ? "add a favourite" : "empty")
                                    onActiveFocusChanged: if (activeFocus) page.selectedSlot = index
                                    onClicked: { page.selectedSlot = index; if (tile) page.activate(index); else if (page.selectedView === "favourites") page.editSlot(index, "pin", null) }
                                    Keys.onPressed: function(event) { page.handleNavigation(event) }
                                    contentItem: ColumnLayout {
                                        spacing: 12
                                        RowLayout {
                                            Layout.fillWidth: true
                                            Rectangle {
                                                implicitWidth: 42
                                                implicitHeight: 42
                                                radius: 12
                                                color: page.canvasColor
                                                Image {
                                                    id: tileIcon
                                                    anchors.centerIn: parent
                                                    source: tile ? page.browserUi.start_page_icon_path(tile.url, false) : ""
                                                    sourceSize: Qt.size(24, 24)
                                                    width: 24; height: 24
                                                    fillMode: Image.PreserveAspectFit
                                                    visible: status === Image.Ready
                                                }
                                                FerricStartGlyph {
                                                    anchors.centerIn: parent
                                                    width: 24; height: 24
                                                    visible: tileIcon.status !== Image.Ready
                                                    objectName: "siteFallbackGlyph" + (index + 1)
                                                    kind: !tile && page.selectedView === "favourites" ? "plus" : "globe"
                                                    color: page.browserWindow.mutedTextColor
                                                    opacity: tile || page.selectedView === "favourites" ? 0.85 : 0.4
                                                }
                                            }
                                            Item { Layout.fillWidth: true }
                                            FerricStartKeycap {
                                                objectName: "favouriteShortcut" + (index + 1)
                                                visible: !!tile
                                                text: String(index + 1)
                                                browserWindow: page.browserWindow
                                            }
                                            Label { visible: !tile; text: "Slot " + (index + 1); color: page.browserWindow.mutedTextColor; font.pointSize: page.fontSize - 1 }
                                        }
                                        Item { Layout.fillHeight: true }
                                        Label {
                                            Layout.fillWidth: true
                                            text: tile ? (tile.title || tile.site) : page.selectedView === "favourites" ? "Add a favourite" : "Waiting for a site"
                                            color: tile || page.selectedView === "favourites" ? page.inkColor : page.browserWindow.mutedTextColor
                                            font.pointSize: page.fontSize + 1
                                            font.weight: tile ? Font.DemiBold : Font.Normal
                                            elide: Text.ElideRight
                                            maximumLineCount: 2
                                            wrapMode: Text.WordWrap
                                            textFormat: Text.PlainText
                                        }
                                        Label {
                                            Layout.fillWidth: true
                                            Layout.rightMargin: tile ? 24 : 0
                                            text: tile ? tile.site : page.selectedView === "favourites" ? "Choose a site for this slot" : ""
                                            color: page.browserWindow.mutedTextColor
                                            font.pointSize: page.fontSize - 1
                                            elide: Text.ElideMiddle
                                            textFormat: Text.PlainText
                                        }
                                    }
                                    background: Rectangle {
                                        radius: 14
                                        color: tileButton.hovered || tileButton.down ? Qt.tint(page.cardColor, Qt.rgba(page.inkColor.r, page.inkColor.g, page.inkColor.b, 0.025)) : tile ? page.cardColor : "transparent"
                                        border.width: tileButton.activeFocus ? 2 : 1
                                        border.color: tileButton.activeFocus ? page.accentColor : tile ? Qt.tint(page.canvasColor, Qt.rgba(page.inkColor.r, page.inkColor.g, page.inkColor.b, 0.07)) : Qt.tint(page.canvasColor, Qt.rgba(page.inkColor.r, page.inkColor.g, page.inkColor.b, 0.12))
                                    }
                                }
                                Button {
                                    id: tileOptions
                                    onActiveFocusChanged: page.controlFocusChanged(this, activeFocus)
                                    visible: !!tile
                                    anchors.right: parent.right
                                    anchors.bottom: parent.bottom
                                    anchors.rightMargin: 10
                                    anchors.bottomMargin: 10
                                    width: 32; height: 32
                                    hoverEnabled: true
                                    Accessible.name: "Options for slot " + (index + 1)
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Site options"
                                    contentItem: FerricStartGlyph { kind: "menu"; color: page.browserWindow.mutedTextColor }
                                    background: Rectangle { radius: 8; color: tileOptions.hovered || tileOptions.activeFocus ? page.browserWindow.surfaceColor : "transparent"; border.width: tileOptions.activeFocus ? 1 : 0; border.color: page.accentColor }
                                    onClicked: tileMenu.open()
                                    Menu {
                                        id: tileMenu
                                        property string promptLeaseToken: ""
                                        palette.window: page.canvasColor
                                        palette.base: page.canvasColor
                                        palette.text: page.inkColor
                                        palette.buttonText: page.inkColor
                                        palette.highlight: page.browserWindow.surfaceColor
                                        palette.highlightedText: page.inkColor
                                        background: Rectangle { color: page.canvasColor; radius: 10; border.color: page.browserWindow.borderColor }
                                        onAboutToShow: {
                                            promptLeaseToken = page.browserWindow.beginNativePromptLease(
                                                page.browserUi,
                                                page.browserWindow.activeWebView(), "qt-menu")
                                            page.menuActive = true
                                        }
                                        onAboutToHide: page.menuActive = false
                                        onClosed: {
                                            var token = promptLeaseToken
                                            promptLeaseToken = ""
                                            page.browserWindow.completeNativePromptLease(
                                                page.browserUi, token,
                                                page.browserWindow.activeWebView())
                                        }
                                        MenuItem { text: "Open"; onTriggered: page.activate(index) }
                                        MenuItem { visible: page.selectedView === "top-sites"; text: "Add to Favourites"; onTriggered: page.addCaptured(tile.url, tile.title) }
                                        MenuItem { visible: page.selectedView === "top-sites"; text: "Hide site"; onTriggered: page.browserUi.start_page_action("dismiss", 0, tile.site, "") }
                                        MenuItem { visible: page.selectedView === "favourites"; text: "Edit bookmark"; onTriggered: page.editSlot(index, "edit", tile) }
                                        MenuItem { visible: page.selectedView === "favourites"; text: "Move to slot…"; onTriggered: page.editSlot(index, "move", tile) }
                                        MenuItem { visible: page.selectedView === "favourites"; text: "Unpin (keep bookmark)"; onTriggered: page.browserUi.start_page_action("unpin", index + 1, "", "") }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            FerricGuide {
                id: guide
                visible: page.selectedView === "guide"
                Layout.fillWidth: true
                Layout.fillHeight: true
                browserUi: page.browserUi
                browserWindow: page.browserWindow
            }
            GridLayout {
                Layout.fillWidth: true
                columns: contentFrame.width > 740 ? 2 : 1
                rowSpacing: 10
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    FerricStartKeycap { visible: page.selectedView !== "guide"; text: "1–8"; browserWindow: page.browserWindow }
                    Label {
                        Layout.fillWidth: true
                        text: page.selectedView === "guide" ? guide.navigationHelp : "Open a saved site · arrows/hjkl select · Enter open"
                        color: page.browserWindow.mutedTextColor
                        font.pointSize: page.fontSize - 1
                        wrapMode: Text.WordWrap
                    }
                }
                RowLayout {
                    Layout.alignment: contentFrame.width > 740 ? Qt.AlignRight : Qt.AlignLeft
                    spacing: 8
                    FerricStartKeycap { text: "Ctrl+Shift+Space"; browserWindow: page.browserWindow }
                    Label { text: "Switch view"; color: page.browserWindow.mutedTextColor; font.pointSize: page.fontSize - 1 }
                }
            }
        }
    }
    readonly property real fontSize: browserWindow.chromeFontPointSize || 10
    Dialog {
        id: editor
        objectName: "ferricFavouriteEditor"
        anchors.centerIn: parent
        width: Math.min(560, page.width - 32)
        padding: page.width < 500 ? 18 : 24
        height: Math.min(implicitHeight, page.height - 32)
        modal: true
        title: page.editingAction === "move" ? "Move favourite" : page.editingAction === "edit" ? "Edit bookmark" : "Add a favourite"
        background: Rectangle { color: page.canvasColor; radius: 16; border.color: page.browserWindow.borderColor }
        header: Label {
            text: editor.title
            leftPadding: editor.padding; rightPadding: editor.padding; topPadding: 22; bottomPadding: 10
            color: page.inkColor
            font.pointSize: page.fontSize + 5
            font.weight: Font.DemiBold
            wrapMode: Text.WordWrap
        }
        footer: Item {
            implicitHeight: 62
            RowLayout {
                anchors.left: parent.left; anchors.right: parent.right
                anchors.leftMargin: editor.padding; anchors.rightMargin: editor.padding
                spacing: 8
                Item { Layout.fillWidth: true }
                FerricStartButton { text: "Cancel"; quiet: true; browserWindow: page.browserWindow; onClicked: editor.reject() }
                FerricStartButton { text: page.editingAction === "move" ? "Move favourite" : "Save favourite"; prominent: true; enabled: !page.browserUi.start_page_busy; browserWindow: page.browserWindow; onClicked: editor.accept() }
            }
        }
        onOpened: {
            if (page.editingAction === "move") {
                var control = destinationSlots.itemAt(destination.value - 1)
                if (control) control.forceActiveFocus()
            } else editUrl.forceActiveFocus()
        }
        onRejected: page.focusContent()
        onAccepted: {
            var action = page.editingAction
            var value = action === "move" ? String(destination.value) : page.editBookmarkId.length > 0 ? page.editBookmarkId : editUrl.text
            if (action === "pin" && page.editBookmarkId.length > 0) action = "bookmark"
            if (!page.browserUi.start_page_action(action, page.editingSlot, value, editTitle.text)) editor.open()
            else page.focusContent()
        }
        contentItem: ScrollView {
            id: editorScroll
            implicitHeight: editorFields.implicitHeight
            clip: true
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            ColumnLayout {
            id: editorFields
            width: editorScroll.availableWidth
            spacing: 14
            Label {
                Layout.fillWidth: true
                text: page.editingAction === "edit" ? "Changes also update this site's bookmark."
                      : page.editingAction === "move" ? "Choose a new position. Two saved favourites swap places."
                      : "Choose a position, then add a site or use an existing bookmark."
                wrapMode: Text.WordWrap
                color: page.browserWindow.secondaryTextColor
                font.pointSize: page.fontSize
                lineHeight: 1.4
            }
            Label { visible: page.editingAction === "pin"; text: "Favourite slot"; color: page.inkColor; font.pointSize: page.fontSize; font.weight: Font.DemiBold }
            GridLayout {
                visible: page.editingAction === "pin"
                Layout.fillWidth: true
                columns: width < 340 ? 4 : 8
                columnSpacing: 6; rowSpacing: 6
                Repeater {
                    model: 8
                    FerricStartButton {
                        required property int index
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        leftPadding: 6; rightPadding: 6
                        text: String(index + 1)
                        checkable: true
                        autoExclusive: true
                        checked: page.editingSlot === index + 1
                        browserWindow: page.browserWindow
                        Accessible.name: "Favourite slot " + (index + 1) + (page.favouriteTiles[index] ? ", occupied" : ", empty")
                        onClicked: page.editingSlot = index + 1
                    }
                }
            }
            Label {
                visible: page.editingAction === "pin" && !!page.favouriteTiles[page.editingSlot - 1]
                Layout.fillWidth: true
                text: "This replaces “" + (page.favouriteTiles[page.editingSlot - 1] ? (page.favouriteTiles[page.editingSlot - 1].title || page.favouriteTiles[page.editingSlot - 1].site) : "") + "”. Its bookmark will be kept."
                color: page.browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
                textFormat: Text.PlainText
                font.pointSize: page.fontSize
            }
            FerricStartComboBox {
                id: bookmarkPicker
                visible: page.editingAction === "pin"
                Layout.fillWidth: true
                browserWindow: page.browserWindow
                model: ["Enter a site address"].concat(page.bookmarks.map(function(tile) { return tile.title || tile.url }))
                Accessible.name: "Choose an existing bookmark"
                onActivated: {
                    var bookmark = currentIndex > 0 ? page.bookmarks[currentIndex - 1] : null
                    page.editBookmarkId = bookmark ? bookmark.id : ""
                    if (bookmark) { editUrl.text = bookmark.url; editTitle.text = bookmark.title }
                }
            }
            ColumnLayout {
                visible: page.editingAction !== "move"
                Layout.fillWidth: true
                spacing: 8
                Label { text: "Site address"; color: page.inkColor; font.pointSize: page.fontSize; font.weight: Font.DemiBold }
                FerricStartField { id: editUrl; Layout.fillWidth: true; browserWindow: page.browserWindow; placeholderText: "https://example.org"; Accessible.name: "Favourite URL"; onTextEdited: page.editBookmarkId = ""; onAccepted: if (!page.browserUi.start_page_busy) editor.accept() }
            }
            ColumnLayout {
                visible: page.editingAction !== "move"
                Layout.fillWidth: true
                spacing: 8
                Label { text: "Name"; color: page.inkColor; font.pointSize: page.fontSize; font.weight: Font.DemiBold }
                FerricStartField { id: editTitle; Layout.fillWidth: true; browserWindow: page.browserWindow; placeholderText: "A name you'll recognise"; Accessible.name: "Favourite label"; onAccepted: if (!page.browserUi.start_page_busy) editor.accept() }
            }
            GridLayout {
                id: destination
                property int value: 1
                visible: page.editingAction === "move"
                Layout.fillWidth: true
                columns: width < 340 ? 4 : 8
                columnSpacing: 6; rowSpacing: 6
                Repeater {
                    id: destinationSlots
                    model: 8
                    FerricStartButton {
                        required property int index
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        leftPadding: 6; rightPadding: 6
                        text: String(index + 1)
                        checkable: true
                        autoExclusive: true
                        checked: destination.value === index + 1
                        browserWindow: page.browserWindow
                        Accessible.name: "Destination slot " + (index + 1)
                        onClicked: destination.value = index + 1
                    }
                }
            }
            Label { Layout.fillWidth: true; visible: page.browserUi.start_page_error.length > 0; text: page.browserUi.start_page_error; color: page.browserWindow.errorColor; wrapMode: Text.WordWrap; font.pointSize: page.fontSize }
        }
        }
    }
    Dialog {
        id: dismissedDialog
        anchors.centerIn: parent
        width: Math.min(520, page.width - 32)
        height: Math.min(450, page.height - 32)
        padding: 20
        modal: true
        title: "Hidden sites"
        background: Rectangle { color: page.canvasColor; radius: 16; border.color: page.browserWindow.borderColor }
        header: Label { text: dismissedDialog.title; leftPadding: 20; topPadding: 22; bottomPadding: 12; color: page.inkColor; font.pointSize: page.fontSize + 5; font.weight: Font.DemiBold }
        footer: Item {
            implicitHeight: 60
            FerricStartButton { anchors.right: parent.right; anchors.rightMargin: 20; text: "Done"; browserWindow: page.browserWindow; onClicked: dismissedDialog.close() }
        }
        onClosed: page.focusContent()
        ScrollView {
            id: hiddenSitesScroll
            anchors.fill: parent
            clip: true
            ColumnLayout {
                width: hiddenSitesScroll.availableWidth
                spacing: 12
                Label { Layout.fillWidth: true; text: page.dismissed.length === 0 ? "No hidden sites. Use a site's menu to hide a suggestion." : "Restore a site here, then refresh Top Sites to see its new position."; color: page.browserWindow.secondaryTextColor; wrapMode: Text.WordWrap; font.pointSize: page.fontSize; lineHeight: 1.4 }
                Repeater {
                    model: page.dismissed
                    RowLayout {
                        required property string modelData
                        Layout.fillWidth: true
                        Label { Layout.fillWidth: true; text: modelData; color: page.inkColor; elide: Text.ElideMiddle; textFormat: Text.PlainText }
                        FerricStartButton { text: "Restore"; browserWindow: page.browserWindow; Accessible.name: "Restore " + modelData; onClicked: page.browserUi.start_page_action("restore", 0, modelData, "") }
                    }
                }
            }
        }
    }
}
