import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: guide
    objectName: "ferricGuide"
    required property var browserUi
    required property var browserWindow
    property string searchText: ""
    property bool readerActive: false
    property string selectedCategory: ""
    property alias searchInput: search
    property alias entriesList: entries
    property var rows: []
    property var categories: []
    property var activeTextSelection: null
    readonly property bool compact: width < Math.max(740, fontSize * 74)
    readonly property bool inputActive: search.activeFocus || sectionFilter.activeFocus || sectionFilter.popup.visible
        || backToIndex.activeFocus || advanced.activeFocus
        || (activeTextSelection && activeTextSelection.activeFocus)
    readonly property var currentRow: entries.currentIndex >= 0 && entries.currentIndex < rows.length && !rows[entries.currentIndex].heading ? rows[entries.currentIndex] : null
    readonly property bool articleOpen: currentRow !== null && (!compact || readerActive)
    readonly property bool handlesEscape: (search.activeFocus && searchText.length > 0)
        || articlePane.activeFocus || (compact && readerActive)
    readonly property string navigationHelp: articlePane.activeFocus || (compact && readerActive)
        ? "j/k scroll · Space page · Esc all features"
        : compact ? "j/k browse · Enter open feature" : "j/k browse · Enter focus article · PgUp/PgDn scroll"
    readonly property var currentShortcuts: currentRow ? currentRow.bindings.map(function(binding) { return binding.keys })
        .filter(function(keys, index, values) { return keys.indexOf("Unbound") !== 0 && values.indexOf(keys) === index }) : []
    readonly property real readingWidth: article.width
    readonly property real fontSize: fontProbe.font.pointSize > 0 ? fontProbe.font.pointSize : (browserWindow.chromeFontPointSize || 10)
    readonly property color canvasColor: browserWindow.backgroundColor
    readonly property color inkColor: browserWindow.primaryTextColor
    readonly property color accentColor: browserWindow.accentColor
    readonly property color raisedColor: Qt.tint(canvasColor, Qt.rgba(inkColor.r, inkColor.g, inkColor.b, 0.035))
    Label { id: fontProbe; visible: false }

    function column(name, index, fallback) {
        var values = browserUi[name]
        return values && values[index] !== undefined ? values[index] : fallback
    }
    function rebuild() {
        var result = [], sectionNames = [], features = []
        var sourceCategories = browserUi.guide_categories || []
        for (var i = 0; i < sourceCategories.length; ++i) {
            var category = sourceCategories[i]
            if (sectionNames.indexOf(category) < 0) sectionNames.push(category)
            var identity = column("guide_ids", i, column("guide_titles", i, ""))
            var feature = features.find(function(item) { return item.identity === identity })
            var binding = { keys: column("guide_keys", i, ""), mode: column("guide_modes", i, ""), command: column("guide_commands", i, ""), notes: column("guide_binding_notes", i, "") }
            if (feature) { feature.bindings.push(binding); continue }
            features.push({ heading: false, identity: identity, category: category,
                title: column("guide_titles", i, ""), summary: column("guide_summaries", i, ""),
                explanation: column("guide_explanations", i, column("guide_details", i, "")),
                arguments: column("guide_arguments", i, ""), examples: column("guide_examples", i, ""),
                recovery: column("guide_recoveries", i, ""), count: column("guide_counts", i, ""),
                bindings: [binding], key: "feature:" + identity })
        }
        categories = sectionNames
        var previous = ""
        for (var n = 0; n < features.length; ++n) {
            var item = features[n]
            if (selectedCategory.length > 0 && item.category !== selectedCategory) continue
            if (item.category !== previous) { result.push({ heading: true, category: item.category, key: "heading:" + item.category }); previous = item.category }
            result.push(item)
        }
        var oldKey = currentRow ? currentRow.key : ""
        rows = result
        var next = result.findIndex(function(row) { return row.key === oldKey && !row.heading })
        if (next < 0 && searchText.length === 0 && selectedCategory.length === 0)
            next = result.findIndex(function(row) { return row.identity === "mode-enter" })
        entries.currentIndex = next >= 0 ? next : result.findIndex(function(row) { return !row.heading })
        if (!currentRow) readerActive = false
        positionCurrentTimer.restart()
    }
    function refresh() { browserUi.refresh_guide(searchText); rebuild() }
    function focusEntries() {
        var returning = compact && readerActive
        readerActive = false
        entries.forceActiveFocus()
        if (entries.currentIndex < 0) move(1)
        focusEntriesTimer.returning = returning
        focusEntriesTimer.restart()
    }
    function focusReader() {
        if (!currentRow) return
        readerActive = true
        if (activeTextSelection) activeTextSelection.focus = false
        backToIndex.focus = false; advanced.focus = false
        articlePane.forceActiveFocus()
        focusReaderTimer.restart()
    }
    // Owned timers cancel deferred layout/focus work when this view unloads.
    Timer {
        id: positionCurrentTimer
        interval: 0
        onTriggered: if (entries.currentIndex >= 0) entries.positionViewAtIndex(entries.currentIndex, ListView.Contain)
    }
    Timer {
        id: focusEntriesTimer
        property bool returning: false
        interval: 0
        onTriggered: {
            if (entries.visible && !guide.readerActive && (entries.focus
                    || (returning && !search.activeFocus && !sectionFilter.activeFocus && !sectionFilter.popup.visible)))
                entries.forceActiveFocus()
        }
    }
    Timer {
        id: focusReaderTimer
        interval: 0
        onTriggered: if (guide.readerActive && articlePane.visible && !guide.inputActive) articlePane.forceActiveFocus()
    }
    function move(delta) {
        var next = entries.currentIndex + delta
        while (next >= 0 && next < rows.length && rows[next].heading) next += delta
        if (next >= 0 && next < rows.length) {
            entries.currentIndex = next
            entries.positionViewAtIndex(next, ListView.Contain)
        }
    }
    function paragraphs(text) {
        if (text.indexOf("\n\n") >= 0) return text.split("\n\n")
        var sentences = text.split(". "), result = []
        for (var i = 0; i < sentences.length; i += 2) result.push(sentences.slice(i, i + 2).join(". ") + (i + 2 < sentences.length ? "." : ""))
        return result
    }
    function modeName(mode) { return mode.split(", ").map(function(value) { return value.replace(/-/g, " ").replace(/^./, function(letter) { return letter.toUpperCase() }) }).join(" · ") }
    function scrollArticle(delta) {
        var viewport = articleScroll.contentItem
        if (viewport && viewport.contentY !== undefined) viewport.contentY = Math.max(0, Math.min(viewport.contentHeight - viewport.height, viewport.contentY + delta))
    }
    function handleNavigation(event) {
        if (inputActive || (!entries.activeFocus && !articlePane.activeFocus)) return false
        if (typeof browserUi.start_page_key_available === "function" && !browserUi.start_page_key_available()) return false
        var plain = event.modifiers === Qt.NoModifier
        var reading = articlePane.activeFocus
        if (plain && articleOpen && event.key === Qt.Key_PageDown) scrollArticle(articleScroll.height * 0.8)
        else if (plain && articleOpen && event.key === Qt.Key_PageUp) scrollArticle(-articleScroll.height * 0.8)
        else if (articleOpen && reading && event.key === Qt.Key_Space
                 && (plain || event.modifiers === Qt.ShiftModifier)) scrollArticle(articleScroll.height * (plain ? 0.8 : -0.8))
        else if (plain && (event.key === Qt.Key_Down || event.key === Qt.Key_J)) { if (reading) scrollArticle(guide.fontSize * 4); else move(1) }
        else if (plain && (event.key === Qt.Key_Up || event.key === Qt.Key_K)) { if (reading) scrollArticle(-guide.fontSize * 4); else move(-1) }
        else if (plain && (event.key === Qt.Key_Right || event.key === Qt.Key_L)) focusReader()
        else if (plain && (event.key === Qt.Key_Left || event.key === Qt.Key_H)) focusEntries()
        else if (plain && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space)) focusReader()
        else if (plain && event.key === Qt.Key_Escape && reading) focusEntries()
        else return false
        event.accepted = true
        return true
    }
    onSearchTextChanged: { selectedCategory = ""; sectionFilter.currentIndex = 0 }
    onCurrentRowChanged: { if (articleScroll.contentItem) articleScroll.contentItem.contentY = 0; advanced.checked = false }
    Keys.onEscapePressed: function(event) {
        if (articlePane.activeFocus || (compact && readerActive)) { focusEntries(); event.accepted = true }
        else event.accepted = false
    }
    Component.onCompleted: refresh()
    Connections {
        target: guide.browserUi
        function onConfig_jsonChanged() { guide.refresh() }
        function onGuide_detailsChanged() { guide.rebuild() }
    }
    RowLayout {
        anchors.fill: parent
        spacing: 24
        ColumnLayout {
            id: catalogue
            visible: !guide.compact || !guide.articleOpen
            Layout.preferredWidth: guide.compact ? guide.width : Math.min(320, Math.max(270, guide.fontSize * 27))
            Layout.fillWidth: guide.compact
            Layout.fillHeight: true
            spacing: 12
            FerricStartField {
                id: search
                Layout.fillWidth: true
                browserWindow: guide.browserWindow
                leftPadding: 40
                placeholderText: "Find a feature or shortcut"
                Accessible.name: "Search the Ferric Guide"
                text: guide.searchText
                onTextChanged: { guide.searchText = text; guide.refresh() }
                FerricStartGlyph { x: 12; anchors.verticalCenter: parent.verticalCenter; width: 18; height: 18; kind: "search"; color: guide.browserWindow.mutedTextColor }
                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Down || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { guide.focusEntries(); event.accepted = true }
                    else if (event.key === Qt.Key_Escape) { if (text.length > 0) text = ""; guide.focusEntries(); event.accepted = true }
                }
            }
            FerricStartComboBox {
                id: sectionFilter
                Layout.fillWidth: true
                browserWindow: guide.browserWindow
                model: ["All sections"].concat(guide.categories)
                Accessible.name: "Filter guide sections"
                onActivated: { guide.selectedCategory = currentIndex > 0 ? guide.categories[currentIndex - 1] : ""; guide.rebuild() }
            }
            Label {
                visible: guide.rows.length === 0
                Layout.fillWidth: true
                text: "No matching features. Try a different word or shortcut."
                color: guide.browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
                padding: 12
            }
            ListView {
                id: entries
                objectName: "ferricGuideEntries"
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 4
                model: guide.rows
                keyNavigationEnabled: false
                activeFocusOnTab: true
                Accessible.name: "Ferric features"
                Keys.onPressed: function(event) { guide.handleNavigation(event) }
                ScrollBar.vertical: ScrollBar { width: 6 }
                delegate: Rectangle {
                    id: featureRow
                    objectName: "guideFeatureRow" + modelData.identity
                    required property var modelData
                    required property int index
                    readonly property bool selected: entries.currentIndex === index
                    width: entries.width - 10
                    height: modelData.heading ? sectionLabel.implicitHeight + 24 : rowContent.implicitHeight + 22
                    radius: 9
                    color: modelData.heading ? "transparent" : selected ? Qt.tint(guide.raisedColor, Qt.rgba(guide.accentColor.r, guide.accentColor.g, guide.accentColor.b, 0.09)) : rowHover.containsMouse ? guide.raisedColor : "transparent"
                    border.width: !modelData.heading && selected && entries.activeFocus ? 1 : 0
                    border.color: guide.accentColor
                    Accessible.role: modelData.heading ? Accessible.Heading : Accessible.Button
                    Accessible.name: modelData.heading ? modelData.category : modelData.title + (selected ? ", selected" : "")
                    Accessible.description: modelData.summary || ""
                    Accessible.onPressAction: { entries.currentIndex = index; if (guide.compact) guide.focusReader(); else guide.focusEntries() }
                    Label {
                        id: sectionLabel
                        visible: modelData.heading
                        x: 12; y: 16; width: parent.width - 24
                        text: modelData.category || ""
                        color: guide.browserWindow.mutedTextColor
                        font.pointSize: guide.fontSize - 1
                        font.weight: Font.DemiBold
                        wrapMode: Text.WordWrap
                    }
                    ColumnLayout {
                        id: rowContent
                        objectName: "guideFeatureContent"
                        visible: !modelData.heading
                        x: 12; y: 11; width: parent.width - 32
                        spacing: 6
                        Label {
                            Layout.fillWidth: true
                            text: modelData.title || ""
                            color: featureRow.selected ? guide.inkColor : guide.browserWindow.secondaryTextColor
                            font.pointSize: guide.fontSize
                            font.weight: featureRow.selected ? Font.DemiBold : Font.Normal
                            wrapMode: Text.WordWrap
                            maximumLineCount: 2
                            elide: Text.ElideRight
                            textFormat: Text.PlainText
                        }
                        Label {
                            Layout.fillWidth: true
                            text: modelData.bindings ? modelData.bindings.filter(function(binding) { return binding.keys.indexOf("Unbound") !== 0 }).map(function(binding) { return binding.keys }).slice(0, 3).join("   ·   ") || "Command entry" : ""
                            color: featureRow.selected ? guide.accentColor : guide.browserWindow.mutedTextColor
                            font.family: "monospace"
                            font.pointSize: guide.fontSize - 1
                            elide: Text.ElideRight
                            textFormat: Text.PlainText
                        }
                    }
                    MouseArea {
                        id: rowHover
                        anchors.fill: parent
                        hoverEnabled: true
                        enabled: !modelData.heading
                        cursorShape: Qt.PointingHandCursor
                        onClicked: { entries.currentIndex = index; if (guide.compact) guide.focusReader(); else guide.focusEntries() }
                    }
                }
            }
        }
        Rectangle {
            visible: !guide.compact
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            color: Qt.rgba(guide.inkColor.r, guide.inkColor.g, guide.inkColor.b, 0.1)
        }
        FocusScope {
            id: articlePane
            objectName: "ferricGuideArticle"
            visible: !guide.compact || guide.articleOpen
            Layout.fillWidth: true
            Layout.fillHeight: true
            Keys.onPressed: function(event) { guide.handleNavigation(event) }

            ScrollView {
                id: articleScroll
                objectName: "ferricGuideReaderScroll"
                anchors.fill: parent
                anchors.topMargin: guide.compact ? 0 : 4
                anchors.bottomMargin: 8
                anchors.rightMargin: 8
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ColumnLayout {
                    id: article
                    width: Math.min(620 * guide.fontSize / 10, articleScroll.availableWidth)
                    x: Math.max(0, (articleScroll.availableWidth - width) / 2)
                    spacing: 22
                    FerricStartButton { id: backToIndex; objectName: "ferricGuideBack"; visible: guide.compact; text: "‹  All features"; quiet: true; browserWindow: guide.browserWindow; onClicked: guide.focusEntries() }
                    Label {
                        Layout.fillWidth: true
                        text: guide.currentRow ? guide.currentRow.category : "Ferric Guide"
                        color: guide.browserWindow.mutedTextColor
                        font.pointSize: guide.fontSize - 1
                        wrapMode: Text.WordWrap
                    }
                    Label {
                        Layout.fillWidth: true
                        text: guide.currentRow ? guide.currentRow.title : "No matching features"
                        color: guide.inkColor
                        font.pointSize: guide.fontSize + 10
                        font.weight: Font.DemiBold
                        wrapMode: Text.WordWrap
                        textFormat: Text.PlainText
                        Accessible.role: Accessible.Heading
                    }
                    Label {
                        visible: !guide.currentRow
                        Layout.fillWidth: true
                        text: "Try another feature name, command, or shortcut. Clear the search to browse all sections."
                        color: guide.browserWindow.secondaryTextColor
                        font.pointSize: guide.fontSize + 1
                        wrapMode: Text.WordWrap
                        lineHeight: 1.5
                    }
                    Label {
                        visible: guide.currentRow !== null
                        Layout.fillWidth: true
                        text: guide.currentRow ? guide.currentRow.summary : ""
                        color: guide.browserWindow.secondaryTextColor
                        font.pointSize: guide.fontSize + 2
                        wrapMode: Text.WordWrap
                        lineHeight: 1.45
                        textFormat: Text.PlainText
                    }
                    Flow {
                        visible: guide.articleOpen && guide.currentShortcuts.length > 0
                        Layout.fillWidth: true
                        spacing: 8
                        Repeater {
                            model: guide.currentShortcuts
                            FerricStartKeycap { required property string modelData; text: modelData; maximumWidth: article.width; browserWindow: guide.browserWindow }
                        }
                    }
                    ColumnLayout {
                        visible: guide.articleOpen
                        Layout.fillWidth: true
                        spacing: 14
                        TapHandler { onTapped: guide.focusReader() }
                        Label { text: "How it works"; color: guide.inkColor; font.pointSize: guide.fontSize + 1; font.weight: Font.DemiBold; Accessible.role: Accessible.Heading }
                        Repeater {
                            model: guide.currentRow ? guide.paragraphs(guide.currentRow.explanation) : []
                            Label {
                                required property string modelData
                                objectName: "ferricGuideExplanation"
                                Layout.fillWidth: true
                                text: modelData
                                color: guide.browserWindow.secondaryTextColor
                                font.pointSize: guide.fontSize + 1
                                wrapMode: Text.WordWrap
                                lineHeight: 1.5
                                textFormat: Text.PlainText
                            }
                        }
                    }
                    ColumnLayout {
                        visible: guide.articleOpen && guide.currentRow !== null
                        Layout.fillWidth: true
                        spacing: 12
                        Label { text: "Shortcuts & commands"; color: guide.inkColor; font.pointSize: guide.fontSize + 1; font.weight: Font.DemiBold }
                        Repeater {
                            model: guide.currentRow ? guide.currentRow.bindings : []
                            RowLayout {
                                required property var modelData
                                Layout.fillWidth: true
                                spacing: 14
                                FerricStartKeycap {
                                    Layout.alignment: Qt.AlignTop
                                    visible: modelData.keys.indexOf("Unbound") !== 0
                                    text: modelData.keys
                                    maximumWidth: Math.min(180, article.width * 0.4)
                                    browserWindow: guide.browserWindow
                                }
                                Label { Layout.alignment: Qt.AlignTop; visible: modelData.keys.indexOf("Unbound") === 0; text: "Command"; color: guide.browserWindow.mutedTextColor; font.pointSize: guide.fontSize }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 3
                                    TextArea {
                                        objectName: "ferricGuideInvocation"
                                        Layout.fillWidth: true
                                        text: guide.currentRow && guide.currentRow.identity === "reserved-recovery" ? modelData.command : ":" + modelData.command
                                        readOnly: true; selectByMouse: true; activeFocusOnTab: false
                                        wrapMode: TextEdit.Wrap; textFormat: TextEdit.PlainText
                                        font.family: "monospace"; font.pointSize: guide.fontSize
                                        color: guide.browserWindow.secondaryTextColor
                                        padding: 0
                                        background: null
                                        Accessible.name: "Configured command " + modelData.command
                                        Keys.onEscapePressed: function(event) { guide.focusReader(); event.accepted = true }
                                        onActiveFocusChanged: { if (activeFocus) guide.activeTextSelection = this; else if (guide.activeTextSelection === this) guide.activeTextSelection = null }
                                    }
                                    Label { Layout.fillWidth: true; text: "Available in " + guide.modeName(modelData.mode); color: guide.browserWindow.mutedTextColor; font.pointSize: guide.fontSize - 1; wrapMode: Text.WordWrap }
                                }
                            }
                        }
                    }
                    ColumnLayout {
                        visible: guide.articleOpen && guide.currentRow && guide.currentRow.examples.length > 0
                        Layout.fillWidth: true
                        spacing: 10
                        Label { text: "Try it"; color: guide.inkColor; font.pointSize: guide.fontSize + 1; font.weight: Font.DemiBold }
                        TextArea {
                            Layout.fillWidth: true
                            text: guide.currentRow ? guide.currentRow.examples : ""
                            readOnly: true; selectByMouse: true; activeFocusOnTab: false
                            wrapMode: TextEdit.Wrap; textFormat: TextEdit.PlainText
                            color: guide.browserWindow.secondaryTextColor
                            font.family: "monospace"; font.pointSize: guide.fontSize
                            padding: 12
                            background: Rectangle { color: guide.canvasColor; radius: 9 }
                            Accessible.name: "Command examples"
                            Keys.onEscapePressed: function(event) { guide.focusReader(); event.accepted = true }
                            onActiveFocusChanged: { if (activeFocus) guide.activeTextSelection = this; else if (guide.activeTextSelection === this) guide.activeTextSelection = null }
                        }
                    }
                    Rectangle {
                        visible: guide.articleOpen && guide.currentRow && guide.currentRow.recovery.length > 0
                        Layout.fillWidth: true
                        implicitHeight: recoveryContent.implicitHeight + 28
                        radius: 10
                        color: Qt.tint(guide.raisedColor, Qt.rgba(guide.accentColor.r, guide.accentColor.g, guide.accentColor.b, 0.06))
                        ColumnLayout {
                            id: recoveryContent
                            x: 14; y: 14; width: parent.width - 28
                            spacing: 6
                            Label { text: "Exit & recovery"; color: guide.inkColor; font.pointSize: guide.fontSize; font.weight: Font.DemiBold }
                            Label { Layout.fillWidth: true; text: guide.currentRow ? guide.currentRow.recovery : ""; color: guide.browserWindow.secondaryTextColor; font.pointSize: guide.fontSize; wrapMode: Text.WordWrap; lineHeight: 1.45; textFormat: Text.PlainText }
                        }
                    }
                    FerricStartButton {
                        id: advanced
                        objectName: "ferricGuideAdvanced"
                        visible: guide.articleOpen
                        quiet: true; checkable: true
                        text: checked ? "⌄  Command & binding details" : "›  Command & binding details"
                        browserWindow: guide.browserWindow
                    }
                    ColumnLayout {
                        visible: guide.articleOpen && advanced.checked
                        Layout.fillWidth: true
                        spacing: 14
                        Label { Layout.fillWidth: true; text: guide.currentRow ? guide.currentRow.count : ""; color: guide.browserWindow.secondaryTextColor; wrapMode: Text.WordWrap; lineHeight: 1.4; font.pointSize: guide.fontSize }
                        Label { Layout.fillWidth: true; text: guide.currentRow && guide.currentRow.arguments.length > 0 ? "Arguments\n" + guide.currentRow.arguments : "No arguments"; color: guide.browserWindow.secondaryTextColor; wrapMode: Text.WordWrap; lineHeight: 1.5; font.pointSize: guide.fontSize; textFormat: Text.PlainText }
                        Repeater {
                            model: guide.currentRow ? guide.currentRow.bindings : []
                            Label { required property var modelData; Layout.fillWidth: true; text: modelData.notes; color: guide.browserWindow.mutedTextColor; wrapMode: Text.WordWrap; lineHeight: 1.4; font.pointSize: guide.fontSize - 1; textFormat: Text.PlainText }
                        }
                    }
                    Item { implicitHeight: 8 }
                }
            }
        }
    }
}
