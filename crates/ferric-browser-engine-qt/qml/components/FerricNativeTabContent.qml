import QtQuick
import QtQuick.Controls

Item {
    id: content
    required property var webView
    required property var browserUi
    required property var browserWindow
    required property var browserHost
    property var tabsModel: null
    property string nativeUrl: ""
    property bool traversalPending: false
    property bool navigationPending: false
    property bool committedContent: false
    property int navigationBaseIndex: 0
    readonly property int tabIndex: webView.tabIndex >= 0 ? webView.tabIndex : browserUi.active_tab_index
    readonly property int engineHistoryIndex: historyCursor.count
    readonly property bool canNavigateBack: { var revision = browserUi.content_history_revision; return browserUi.content_history_available(tabIndex, -1) }
    readonly property bool canNavigateForward: { var revision = browserUi.content_history_revision; return browserUi.content_history_available(tabIndex, 1) }
    readonly property var startPage: startLoader.item
    property bool startCreated: false
    visible: true
    z: 3

    Instantiator {
        id: historyCursor
        model: content.webView.history.backItems
        onCountChanged: Qt.callLater(content.synchronizeHistory)
        delegate: QtObject { required property url url; required property int offset }
    }
    Instantiator {
        id: forwardCursor
        model: content.webView.history.forwardItems
        delegate: QtObject { required property url url; required property int offset }
    }
    function engineOffset(destination, expected, backwards) {
        if (webView.url.toString() === destination && expected === 0) return 0
        // WebEngine can publish its history model after the URL notification.
        // Resolve against the settled model rather than a captured stale count.
        var models = backwards ? [historyCursor, forwardCursor] : [forwardCursor, historyCursor]
        var nearest = null
        for (var modelIndex = 0; modelIndex < models.length; ++modelIndex) {
            for (var index = 0; index < models[modelIndex].count; ++index) {
                var entry = models[modelIndex].objectAt(index)
                if (!entry || entry.url.toString() !== destination) continue
                if (entry.offset === expected) return expected
                if (nearest === null || Math.abs(entry.offset) < Math.abs(nearest)) nearest = entry.offset
            }
            if (nearest !== null) return nearest
        }
        return null
    }
    function engineEntry(index) {
        if (index === engineHistoryIndex) return { url: webView.url.toString(), engine_index: index }
        for (var row = 0; row < historyCursor.count; ++row) {
            var entry = historyCursor.objectAt(row)
            if (entry && engineHistoryIndex + entry.offset === index) return { url: entry.url.toString(), engine_index: index }
        }
        return null
    }
    function synchronizeHistory() {
        if (nativeUrl.length > 0 || webView.loading) return
        if (navigationPending && browserUi.load_state !== "complete") return
        var snapshot
        try { snapshot = JSON.parse(browserUi.content_history_export(tabIndex)) } catch (error) { return }
        if (!snapshot || !snapshot.entries || !snapshot.entries[snapshot.cursor]) return
        var current = snapshot.entries[snapshot.cursor]
        var destination = webView.url.toString()
        if (current.url !== destination) {
            var nativeCurrent = current.url === "rb://start" || current.url === "rb://blank"
            if (!nativeCurrent && current.engine_index === engineHistoryIndex) {
                // replaceState changes this engine entry rather than adding one.
                current.url = destination
            } else {
                var base = nativeCurrent ? navigationBaseIndex
                        : engineHistoryIndex + engineOffset(current.url, current.engine_index - engineHistoryIndex, true)
                if (!nativeCurrent && base >= engineHistoryIndex) return
                if (!nativeCurrent) current.engine_index = base
                snapshot.entries.length = snapshot.cursor + 1
                for (var added = base + 1; added < engineHistoryIndex; ++added) {
                    var intermediate = engineEntry(added)
                    if (intermediate) snapshot.entries.push(intermediate)
                }
                snapshot.entries.push({url: destination, engine_index: engineHistoryIndex})
                snapshot.cursor = snapshot.entries.length - 1
            }
            while (snapshot.entries.length > 4096) { snapshot.entries.shift(); snapshot.cursor -= 1 }
            browserUi.content_history_import(tabIndex, JSON.stringify(snapshot))
            navigationPending = false; traversalPending = false
            return
        }
        if (current.engine_index === engineHistoryIndex) { navigationPending = false; traversalPending = false; return }
        var previous = snapshot.entries[snapshot.cursor - 1]
        var baseline = navigationBaseIndex
        if (previous && previous.url !== "rb://start" && previous.url !== "rb://blank") {
            baseline = engineHistoryIndex + engineOffset(previous.url, previous.engine_index - engineHistoryIndex, true)
            previous.engine_index = baseline
        }
        var old = engineEntry(current.engine_index)
        var preserveCurrent = old && old.url === current.url && baseline < current.engine_index
        var segment = []
        for (var index = (preserveCurrent ? current.engine_index : baseline) + 1; index < engineHistoryIndex; ++index) {
            var missing = engineEntry(index)
            if (missing) segment.push(missing)
        }
        segment.push({ url: current.url, engine_index: engineHistoryIndex })
        var insertion = snapshot.cursor + (preserveCurrent ? 1 : 0)
        snapshot.entries.splice.apply(snapshot.entries, [insertion, preserveCurrent ? 0 : 1].concat(segment))
        snapshot.cursor += segment.length - (preserveCurrent ? 0 : 1)
        if (snapshot.entries.length > 4096) {
            var removed = snapshot.entries.length - 4096
            snapshot.entries.splice(0, removed); snapshot.cursor -= removed
        }
        browserUi.content_history_import(tabIndex, JSON.stringify(snapshot))
    }
    function focusContent() { if (nativeUrl === "rb://start" && startPage) startPage.focusContent() }
    function setContentUrl(destination, traversal, engineMoves) {
        if (destination === "rb://start" || destination === "rb://blank") {
            webView.stop()
            nativeUrl = destination
            committedContent = true
            traversalPending = false
            navigationPending = false
            browserUi.content_navigated(tabIndex, destination, engineHistoryIndex, !!traversal)
            if (tabsModel && tabIndex >= 0 && tabIndex < tabsModel.count) {
                tabsModel.setProperty(tabIndex, "url", destination)
                tabsModel.setProperty(tabIndex, "title", destination === "rb://start" ? "Start page" : "Blank")
            }
            if (destination === "rb://start") {
                startCreated = true
                if (startPage) startPage.refresh()
                Qt.callLater(focusContent)
            }
        } else {
            if (!traversal) navigationBaseIndex = engineHistoryIndex
            nativeUrl = ""
            if (typeof browserWindow.installFocusObserver === "function") browserWindow.installFocusObserver(webView)
            traversalPending = !!traversal
            if (webView.url.toString() !== destination && !traversal) {
                navigationPending = true
                webView.url = destination
            } else if (!traversal && committedContent && !navigationPending && !webView.loading) {
                browserUi.navigation_committed_for(tabIndex, destination, webView.title)
                browserUi.navigation_completed_for(tabIndex)
                noteCommit()
            }
            if (tabsModel && tabIndex >= 0 && tabIndex < tabsModel.count) tabsModel.setProperty(tabIndex, "url", destination)
            if (traversal && !engineMoves && webView.url.toString() === destination) {
                browserUi.navigation_url_changed_for(tabIndex, destination)
                browserUi.navigation_committed_for(tabIndex, destination, webView.title)
                browserUi.navigation_completed_for(tabIndex)
                browserUi.content_navigated(tabIndex, destination, engineHistoryIndex, true)
                traversalPending = false
            }
        }
    }
    function beginNavigation() { if (nativeUrl.length === 0) navigationPending = true }
    function noteSameDocument() {
        Qt.callLater(synchronizeHistory)
    }
    function noteCommit() {
        if (nativeUrl.length > 0) return
        navigationPending = false
        committedContent = true
        browserUi.content_navigated(tabIndex, webView.url.toString(), engineHistoryIndex, traversalPending)
        traversalPending = false
        Qt.callLater(synchronizeHistory)
    }
    Rectangle { anchors.fill: parent; visible: content.nativeUrl.length > 0; color: content.browserWindow.backgroundColor }
    Loader {
        id: startLoader
        anchors.fill: parent
        active: content.startCreated
        visible: content.nativeUrl === "rb://start"
        onLoaded: Qt.callLater(content.focusContent)
        sourceComponent: FerricStartPage {
            browserUi: content.browserUi
            browserWindow: content.browserWindow
            browserHost: content.browserHost
            contentView: content.webView
        }
    }
    Image {
        x: -10000
        width: 32; height: 32
        source: content.nativeUrl.length === 0 ? content.webView.icon : ""
        sourceSize: Qt.size(32, 32)
        // Keep the capture item outside the viewport; opacity would be baked
        // into the cached image. The provider reads already-loaded engine icons.
        visible: true
        onStatusChanged: {
            if (status !== Image.Ready || content.nativeUrl.length > 0) return
            var path = content.browserUi.start_page_icon_path(content.webView.url.toString(), true)
            if (path.length > 0) grabToImage(function(result) { result.saveToFile(path) })
        }
    }
}
