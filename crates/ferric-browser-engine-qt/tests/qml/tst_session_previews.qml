import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
TestCase {
    id: test
    name: "SessionPreviewPolish"
    when: windowShown
    ApplicationWindow {
        id: host
        width: 900
        height: 700
        visible: true
        font.family: "monospace"
        font.pointSize: 10
        property real chromeScale: 1
        property real chromeRowHeight: 24
        property color backgroundColor: "#181825"
        property color surfaceColor: "#313244"
        property color panelColor: "#1e1e2e"
        property color primaryTextColor: "#cdd6f4"
        property color secondaryTextColor: "#bac2de"
        property color mutedTextColor: "#a6adc8"
        property color borderColor: "#585b70"
        property color accentColor: "#89b4fa"
        property color warningColor: "#f9e2af"
        property color errorColor: "#f38ba8"
        property color selectionColor: "#45475a"
        property color selectionTextColor: "#cdd6f4"
        property bool downloadManagerVisible: true
        property bool sessionManagerVisible: true
        property bool temporaryProfile: false
        property bool ephemeralProfile: false
        property bool sessionPreviewVisible: true
        property bool sessionPreviewLoading: false
        property bool sessionPreviewError: false
        property bool sessionPreviewAppend: false
        property string sessionPreviewName: "Research"
        property string sessionPreviewText: ""
        property bool profileDeletePreviewVisible: false
        property string profileDeleteName: "work"
        property string profileDeletePreviewText: "/disposable/profile/path"
        property string pendingDeleteName: ""
        color: backgroundColor
        palette.window: backgroundColor
        palette.windowText: primaryTextColor
        palette.base: panelColor
        palette.text: primaryTextColor
        palette.placeholderText: mutedTextColor
        palette.button: surfaceColor
        palette.buttonText: primaryTextColor
        palette.highlight: accentColor
        palette.highlightedText: backgroundColor
    }

    Component { id: sessionFixture; FerricSessionPreview { browserWindow: host; savedTabs: [
        {label: "<b>Research</b>", url: "https://example.test/article", pinned: true, muted: false},
        {label: "Submitted form", url: null, pinned: false, muted: true}
    ] } }
    Component { id: deletionFixture; FerricProfileDeletePreview { browserWindow: host; folderText: host.profileDeletePreviewText; onCancelRequested: host.profileDeletePreviewVisible = false } }
    Component { id: managerFixture; FerricSessionManager { browserWindow: host; sessionsModel: [] } }
    Component { id: spyFixture; SignalSpy {} }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width = 900; host.height = 700; host.font.pointSize = 10
        host.sessionPreviewVisible = true; host.sessionPreviewLoading = false
        host.sessionPreviewError = false; host.sessionPreviewAppend = false
        host.profileDeletePreviewVisible = false
        host.sessionPreviewText = JSON.stringify({tabs: [
            {label: "<b>Research</b>", url: "https://example.test/article", pinned: true, muted: false},
            {label: "Submitted form", url: null, pinned: false, muted: true}
        ]})
        host.requestActivate(); tryCompare(host, "active", true)
    }
    function test_preview_data() {
        return [{tag: "wide", width: 900, height: 700, font: 10},
                {tag: "narrow", width: 390, height: 600, font: 10},
                {tag: "large-text", width: 390, height: 700, font: 16}]
    }
    function test_preview(row) {
        host.width = row.width; host.height = row.height; host.font.pointSize = row.font
        var surface = createTemporaryObject(sessionFixture, host.contentItem)
        verify(surface); verify(waitForPolish(host))
        verify(surface.canLoad)
        var list = findChild(surface, "sessionPreviewTabs")
        tryCompare(list, "count", 2)
        var first = list.itemAtIndex(0)
        verify(list.height >= findChild(first, "savedTabTitle").implicitHeight * 2)
        compare(findChild(first, "savedTabTitle").text, "<b>Research</b>")
        compare(findChild(first, "savedTabTitle").textFormat, Text.PlainText)
        list.positionViewAtIndex(1, ListView.Contain)
        verify(waitForPolish(host))
        var second = list.itemAtIndex(1)
        verify(findChild(second, "savedTabAddress").text.indexOf("won't be replayed") >= 0)
        var append = findChild(surface, "commandDialogAction-append")
        var point = append.mapToItem(surface.panelItem, 0, 0)
        verify(point.y >= 0 && point.y + append.height <= surface.panelItem.height)
        var replace = findChild(surface, "commandDialogAction-replace")
        var label = findChild(replace, "commandActionLabel")
        var labelPoint = label.mapToItem(surface.panelItem, 0, 0)
        verify(labelPoint.x + label.width <= surface.panelItem.width)
        verify(label.height >= label.implicitHeight - 1)
        if (testArtifactDirectory.length > 0)
            grabImage(host.contentItem).save(testArtifactDirectory + "/session-preview-" + row.tag + ".png")
        var spy = createTemporaryObject(spyFixture, surface, {target: surface, signalName: "loadRequested"})
        keyClick(Qt.Key_A)
        compare(spy.count, 1); compare(spy.signalArguments[0][0], true)
    }
    function test_loading_errors_and_empty_previews_cannot_restore() {
        var surface = createTemporaryObject(sessionFixture, host.contentItem)
        verify(surface)
        var spy = createTemporaryObject(spyFixture, surface, {target: surface, signalName: "loadRequested"})
        host.sessionPreviewLoading = true
        verify(!surface.canLoad)
        keyClick(Qt.Key_A); keyClick(Qt.Key_R)
        compare(spy.count, 0)
        host.sessionPreviewLoading = false; host.sessionPreviewError = true
        verify(!surface.canLoad)
        host.sessionPreviewError = false; surface.savedTabs = []
        verify(!surface.canLoad)
        keyClick(Qt.Key_A); keyClick(Qt.Key_R)
        compare(spy.count, 0)
    }
    function test_delete_waits_for_the_scope_preview_and_uses_safe_default() {
        host.profileDeletePreviewVisible = true
        var surface = createTemporaryObject(deletionFixture, host.contentItem)
        verify(surface)
        verify(surface.message.indexOf("Cookies and website storage stay on disk") >= 0)
        var confirm = findChild(surface, "commandDialogAction-confirm")
        verify(!confirm.enabled)
        var spy = createTemporaryObject(spyFixture, surface, {target: surface, signalName: "confirmRequested"})
        keyClick(Qt.Key_Y); compare(spy.count, 0)
        surface.folderText = ""
        surface.previewPending = false
        keyClick(Qt.Key_Y); compare(spy.count, 0)
        surface.folderText = "/disposable/profile/path"
        tryVerify(function() { var item = findChild(surface, "commandDialogAction-confirm"); return item && item.enabled })
        tryCompare(findChild(surface, "commandDialogAction-cancel"), "activeFocus", true)
        keyClick(Qt.Key_Return); compare(spy.count, 0)
        tryCompare(host, "profileDeletePreviewVisible", false)
        host.profileDeletePreviewVisible = true
        verify(waitForPolish(host))
        keyClick(Qt.Key_Y); compare(spy.count, 1)
    }
    function test_session_save_keeps_the_name_until_success_is_known() {
        var manager = createTemporaryObject(managerFixture, host.contentItem)
        verify(manager)
        verify(waitForPolish(host))
        var input = findChild(manager, "sessionNameInput")
        input.text = "draft-session"
        var spy = createTemporaryObject(spyFixture, manager, {target: manager, signalName: "saveRequested"})
        mouseClick(findChild(manager, "sessionSaveButton"))
        compare(spy.count, 1)
        compare(input.text, "draft-session")
    }
}
