import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
import "../../qml/scripts/ChromePresentation.js" as ChromePresentation

TestCase {
    id: test
    name: "ProductPolish"
    when: windowShown

    property string downloadActionId: ""
    property string switcherAction: ""
    ApplicationWindow {
        id: host
        width: 900
        height: 700
        visible: true
        font.family: "monospace"
        font.pointSize: 10
        FontMetrics { id: actualFontMetrics; font: host.font }
        property real chromeScale: Math.max(1, actualFontMetrics.height / 14)
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
        property var currentView: recoveryView
        function activeWebView() { return currentView }
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
    QtObject {
        id: recoveryView
        property bool contentCanGoBack: true
        property bool formResubmissionPending: false
    }
    QtObject {
        id: navigationUi
        property int active_tab_index: 0
        property string current_url: "https://unreachable.example/"
        function tab_id_for_index(index) { return "tab-" + index }
        property bool navigation_failure_visible: true
        property string navigation_failure_kind: "dns"
        property string navigation_failure_url: "https://unreachable.example/"
        property string navigation_failure_requested_url: "https://unreachable.example/"
        property string navigation_failure_detail: "net::ERR_NAME_NOT_RESOLVED"
        function clear_navigation_failure() { navigation_failure_visible = false }
    }
    ListModel { id: downloads }
    Component {
        id: downloadFixture
        FerricDownloadManager {
            browserWindow: host
            downloadsModel: downloads
            onOpenRequested: function(downloadId, reveal) { test.downloadActionId = downloadId }
        }
    }
    Component {
        id: navigationFixture
        FerricNavigationFailure { browserWindow: host; browserUi: navigationUi }
    }
    Component {
        id: sessionsFixture
        FerricSessionManager { browserWindow: host; sessionsModel: [] }
    }
    Component {
        id: plainWindowFixture
        ApplicationWindow { width: 390; height: 600; visible: true; font: host.font }
    }
    Component {
        id: shutdownFixture
        FerricShutdownDecisionDialog { promptVisible: true }
    }
    Component {
        id: switcherFixture
        FerricSwitcherResults {
            width: 350
            height: 280
            browserWindow: host
            results: [{ kind: "session", label: "<b>Research</b>", profile: "Personal",
                        secondary: "3 tabs", actions: ["load-preview", "load"] }]
            onActionRequested: function(index, action) { test.switcherAction = action }
        }
    }

    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width = 900
        host.height = 700
        host.font.pointSize = 10
        host.requestActivate()
        tryCompare(host, "active", true)
        navigationUi.navigation_failure_visible = true
        navigationUi.active_tab_index = 0
        navigationUi.current_url = "https://unreachable.example/"
        host.currentView = recoveryView
        recoveryView.contentCanGoBack = true
        recoveryView.formResubmissionPending = false
        host.temporaryProfile = false
        host.ephemeralProfile = false
        downloadActionId = ""
        switcherAction = ""
        downloads.clear()
    }
    function create(fixture) {
        var item = createTemporaryObject(fixture, host.contentItem)
        verify(item !== null)
        verify(waitForPolish(host))
        return item
    }
    function capture(name) {
        if (testArtifactDirectory.length > 0) {
            verify(waitForPolish(host))
            wait(50)
            grabImage(host.contentItem).save(testArtifactDirectory + "/" + name + ".png")
        }
    }
    function inside(item, panel) {
        var position = item.mapToItem(panel, 0, 0)
        verify(position.x >= -1 && position.y >= -1)
        verify(position.x + item.width <= panel.width + 1)
        verify(position.y + item.height <= panel.height + 1)
    }
    function test_readable_progress_handles_unknown_sizes() {
        compare(ChromePresentation.downloadProgress(1572864, 3145728, 1048576, "in-progress"),
                "1.5 MiB of 3.0 MiB · 1.0 MiB/s")
        compare(ChromePresentation.downloadProgress(0, -1, 0, "in-progress"), "0 B")
        compare(ChromePresentation.downloadProgress(1024, 2048, 1024, "paused"), "1.0 KiB of 2.0 KiB")
        compare(ChromePresentation.byteSize(NaN), "Size unknown")
        compare(ChromePresentation.byteSize(-1), "Size unknown")
        compare(ChromePresentation.downloadStateLabel("constructor"), "Status unavailable")
    }
    function test_download_rate_survives_rapid_refreshes() {
        var sample = ChromePresentation.downloadRateSample(null,0,1000)
        sample = ChromePresentation.downloadRateSample(sample,400,2000)
        compare(sample.rate,400)
        sample = ChromePresentation.downloadRateSample(sample,400,2000)
        compare(sample.rate,400)
        sample = ChromePresentation.downloadRateSample(sample,450,2050)
        compare(sample.rate,400)
        sample = ChromePresentation.downloadRateSample(sample,1000,3000)
        compare(sample.rate,600)
    }
    function test_download_rate_resets_after_hidden_intervals_and_counter_or_clock_changes() {
        var sample = {at:1000,bytes:400,rate:400}
        sample = ChromePresentation.downloadRateSample(sample,10000,7000)
        compare(sample.rate,0)
        sample = ChromePresentation.downloadRateSample(sample,10500,8000)
        compare(sample.rate,500)
        sample = ChromePresentation.downloadRateSample(sample,0,9000)
        compare(sample.rate,0)
        sample = ChromePresentation.downloadRateSample(sample,100,10000)
        compare(sample.rate,100)
        sample = ChromePresentation.downloadRateSample(sample,100,9000)
        compare(sample.rate,0)
    }
    function test_shutdown_has_themed_safe_actions_with_long_text() {
        host.font.pointSize = 20
        var child = createTemporaryObject(plainWindowFixture)
        verify(child)
        var surface = createTemporaryObject(shutdownFixture, child.contentItem, {
            hostWindow: child, themeWindow: host,
            title: "<b>Close this window?</b>",
            message: "This page has unsaved form or editor state. ".repeat(12),
            proceedLabel: "Cancel active work and close this window"
        })
        verify(surface)
        child.requestActivate()
        tryCompare(child, "active", true)
        var keep = findChild(surface, "shutdownKeepButton")
        var proceed = findChild(surface, "shutdownProceedButton")
        var panel = findChild(surface, "shutdownDecisionPanel")
        compare(panel.color, host.panelColor)
        compare(findChild(surface, "shutdownTitle").textFormat, Text.PlainText)
        tryVerify(function() { return keep.activeFocus })
        verify(waitForPolish(child))
        inside(keep, panel)
        inside(proceed, panel)
        var scroll = findChild(surface, "shutdownMessageScroll")
        verify(scroll.contentHeight > scroll.height)
        var title = findChild(surface, "shutdownTitle")
        var message = findChild(surface, "shutdownMessage")
        verify(title.height >= title.implicitHeight - 1)
        verify(message.height >= message.implicitHeight - 1)
        verify(message.y >= title.y + title.height)
        var scrollbar = findChild(surface, "shutdownMessageScrollbar")
        verify(scrollbar)
        var scrollbarPosition = scrollbar.mapToItem(scroll, 0, 0)
        verify(scrollbarPosition.x > scroll.width / 2)
        if (testArtifactDirectory.length > 0)
            grabImage(child.contentItem).save(testArtifactDirectory + "/shutdown-narrow-large-font.png")
        mouseWheel(scroll, scroll.width / 2, scroll.height / 2, 0, -240)
        tryVerify(function() { return scroll.contentItem.contentY > 0 })
        inside(keep, panel)
        inside(proceed, panel)
        var spy = createTemporaryObject(signalSpyFixture, surface, {target: surface, signalName: "keepRequested"})
        keyClick(Qt.Key_Escape)
        compare(spy.count, 1)
    }
    Component { id: signalSpyFixture; SignalSpy {} }
    function test_navigation_failure_data() {
        return [{tag: "wide", width: 900, height: 700},
                {tag: "compact", width: 640, height: 480},
                {tag: "narrow", width: 390, height: 600}]
    }
    function test_navigation_failure(data) {
        host.width = data.width
        host.height = data.height
        var surface = create(navigationFixture)
        verify(surface.message.indexOf("address") >= 0)
        var details = findChild(surface, "navigationFailureDetails")
        var button = findChild(surface, "navigationFailureDetailsButton")
        verify(!details.visible)
        inside(button, surface.panelItem)
        inside(findChild(surface, "commandDialogAction-cancel"), surface.panelItem)
        capture("navigation-" + data.tag)
        mouseClick(button)
        tryVerify(function() { return details.visible })
        verify(details.text.indexOf("ERR_NAME_NOT_RESOLVED") >= 0)
        compare(details.textFormat, TextEdit.PlainText)
        capture("navigation-details-" + data.tag)
        host.requestActivate()
        tryCompare(host, "active", true)
        keyClick(Qt.Key_Escape)
        tryCompare(navigationUi, "navigation_failure_visible", false)
    }
    function test_navigation_recovery_keys_data() {
        return [{tag:"reopen",key:Qt.Key_R,signal:"reopenRequested"},
                {tag:"edit",key:Qt.Key_E,signal:"editRequested"},
                {tag:"back",key:Qt.Key_B,signal:"backRequested"}]
    }
    function test_navigation_recovery_actions_at_large_text_data() {
        return [{tag:"compact",width:640,height:480,font:20},
                {tag:"narrow",width:320,height:600,font:26}]
    }
    function test_navigation_recovery_actions_at_large_text(data) {
        host.width=data.width;host.height=data.height;host.font.pointSize=data.font
        var surface=create(navigationFixture)
        var scroll=findChild(surface,"commandDialogActionScroll")
        tryCompare(findChild(surface,"commandDialogAction-cancel"),"activeFocus",true)
        for(var i=0;i<surface.actions.length;i++) {
            keyClick(Qt.Key_J)
            var row=findChild(surface,"commandDialogAction-"+surface.actions[i].id)
            tryCompare(row,"activeFocus",true)
            verify(waitForPolish(host))
            var y=row.mapToItem(scroll,0,0).y
            verify(y+row.height>0 && y<scroll.height,"Selected action must be visible")
            if(row.height<=scroll.height) verify(y>=-1 && y+row.height<=scroll.height+1)
        }
        capture("navigation-large-text-"+data.tag)
    }
    function test_navigation_recovery_keys(data) {
        navigationUi.current_url = 'https://unreachable.example/?q=a;;b&token=secret#fragment'
        var surface = create(navigationFixture)
        var spy = createTemporaryObject(signalSpyFixture, surface, {target:surface,signalName:data.signal})
        tryCompare(findChild(surface,"commandDialogAction-cancel"),"activeFocus",true)
        keyClick(data.key)
        compare(spy.count,1)
        compare(navigationUi.navigation_failure_visible,false)
        if(data.tag==="reopen") compare(spy.signalArguments[0][0],navigationUi.current_url)
        if(data.tag==="edit") compare(spy.signalArguments[0][0],'open "'+navigationUi.current_url+'"')
    }
    function test_navigation_recovery_rejects_retired_owner_data() {
        return [{tag:"tab"},{tag:"address"},{tag:"view"}]
    }
    function test_navigation_recovery_rejects_retired_owner(data) {
        var surface = create(navigationFixture)
        var spy = createTemporaryObject(signalSpyFixture,surface,{target:surface,signalName:"reopenRequested"})
        if(data.tag==="tab") navigationUi.active_tab_index++
        else if(data.tag==="address") navigationUi.current_url="https://other.example/"
        else host.currentView=null
        surface.requestAction("reopen")
        compare(spy.count,0)
        tryCompare(navigationUi,"navigation_failure_visible",false)
    }
    function test_navigation_recovery_keeps_enter_safe_and_disables_back_at_boundary() {
        recoveryView.contentCanGoBack=false
        var surface=create(navigationFixture)
        var spy=createTemporaryObject(signalSpyFixture,surface,{target:surface,signalName:"backRequested"})
        verify(!findChild(surface,"commandDialogAction-back").enabled)
        tryCompare(findChild(surface,"commandDialogAction-cancel"),"activeFocus",true)
        keyClick(Qt.Key_B);compare(spy.count,0)
        verify(navigationUi.navigation_failure_visible)
        keyClick(Qt.Key_Return)
        tryCompare(navigationUi,"navigation_failure_visible",false)
    }
    function test_error_dialog_is_suspended_while_form_confirmation_owns_input() {
        var surface=create(navigationFixture)
        tryCompare(findChild(surface,"commandDialogAction-cancel"),"activeFocus",true)
        var detail=navigationUi.navigation_failure_detail
        recoveryView.formResubmissionPending=true
        verify(surface.visible)
        verify(!surface.enabled)
        compare(surface.opacity,0)
        verify(surface.Accessible.ignored)
        verify(navigationUi.navigation_failure_visible)
        recoveryView.formResubmissionPending=false
        verify(surface.enabled)
        compare(surface.opacity,1)
        compare(navigationUi.navigation_failure_detail,detail)
        tryCompare(findChild(surface,"commandDialogAction-cancel"),"activeFocus",true)
    }
    function test_suspended_error_dialog_still_expires_with_its_original_owner() {
        var surface=create(navigationFixture)
        recoveryView.formResubmissionPending=true
        host.currentView=null
        tryCompare(navigationUi,"navigation_failure_visible",false)
        verify(!surface.visible)
    }
    function test_saving_controls_fit_compact_large_fonts_data() {
        return [{tag:"normal",privateMode:false},{tag:"private",privateMode:true}]
    }
    function test_saving_controls_fit_compact_large_fonts(data) {
        host.width=640; host.height=480; host.font.pointSize=16
        host.temporaryProfile=data.privateMode
        downloads.append({id:"00000000-0000-0000-0000-000000000001",state:"in-progress",bytes:"1048576",
            total:1048576,speed:0,destination:"/owned/Research report – final.bin",reason:"",displayName:"Research <final> – Δ.bin",
            active:true,fileReady:false,retryAvailable:false,retryPending:false,finishing:true})
        var surface=create(downloadFixture)
        surface.sessionOnly=data.privateMode
        var list=findChild(surface,"downloadList")
        tryCompare(list,"count",1)
        tryVerify(function(){return list.itemAtIndex(0)!==null})
        var row=list.itemAtIndex(0)
        var cancel=findChild(row,"downloadCancelButton")
        verify(!findChild(row,"downloadProgressBar").visible,"Saving must not display completed network progress")
        verify(waitForPolish(host))
        var position=cancel.mapToItem(list,0,0)
        verify(position.y>=0 && position.y+cancel.height<=list.height,"Saving Cancel must fit without scrolling")
        surface.listFailed=true
        verify(waitForPolish(host))
        verify(Math.abs(cancel.mapToItem(list,0,0).y-position.y)<0.1,"History feedback must not move the control")
        verify(surface.historyStatusDescription.indexOf("Refresh")>=0)
        downloads.setProperty(0,"state","interrupted")
        downloads.setProperty(0,"active",false)
        downloads.setProperty(0,"finishing",false)
        downloads.setProperty(0,"retryAvailable",true)
        downloads.setProperty(0,"reason",Array(24).join("The destination could not be saved. Choose another folder and retry. "))
        mouseClick(findChild(row,"downloadDetailsButton"))
        verify(waitForPolish(host))
        list.positionViewAtEnd()
        var retry=findChild(row,"downloadRetryButton")
        retry.forceActiveFocus(Qt.TabFocusReason)
        tryCompare(retry,"activeFocus",true)
        verify(waitForPolish(host))
        position=retry.mapToItem(list,0,0)
        verify(position.y>=-1 && position.y+retry.height<=list.height+1,"Keyboard focus must reveal the requested control")
    }
    function test_downloads_use_filename_and_keep_command_id_in_details() {
        downloads.append({ id: "download-command-id", state: "completed", bytes: "1572864",
                           total: 1572864, speed: 0, destination: "/home/test/Downloads/archive-λ.tar.gz",
                           reason: "No reason", displayName: "archive-λ.tar.gz", active: false, fileReady: true, retryAvailable: true, retryPending: false, finishing: false })
        var surface = create(downloadFixture)
        var list = findChild(surface, "downloadList")
        tryCompare(list, "count", 1)
        var row = list.itemAtIndex(0)
        verify(row)
        compare(findChild(row, "downloadFileName").text, "archive-λ.tar.gz")
        var details = findChild(row, "downloadDetails")
        verify(!details.visible)
        capture("downloads-complete")
        mouseClick(findChild(row, "downloadDetailsButton"))
        tryVerify(function() { return details.visible })
        verify(details.text.indexOf("download-command-id") >= 0)
        verify(details.text.indexOf("No reason") < 0)
        var open = findChild(row, "downloadOpenButton")
        verify(open)
        mouseClick(open)
        compare(downloadActionId, "download-command-id")
    }
    function test_download_progress_has_visible_fill_data() {
        return [
            {tag:"dark",background:"#181825",panel:"#1e1e2e",surface:"#313244",accent:"#89b4fa",primary:"#cdd6f4",secondary:"#bac2de",muted:"#a6adc8",border:"#585b70"},
            {tag:"light",background:"#fafafa",panel:"#ffffff",surface:"#f0f2f5",accent:"#174ea6",primary:"#181818",secondary:"#333333",muted:"#555555",border:"#888888"},
            {tag:"minimal",background:"#080808",panel:"#080808",surface:"#181818",accent:"#eeeeee",primary:"#eeeeee",secondary:"#dddddd",muted:"#aaaaaa",border:"#666666"}
        ]
    }
    function test_download_progress_has_visible_fill(data) {
        var colors = {backgroundColor:data.background,panelColor:data.panel,surfaceColor:data.surface,
                      accentColor:data.accent,primaryTextColor:data.primary,secondaryTextColor:data.secondary,
                      mutedTextColor:data.muted,borderColor:data.border,selectionTextColor:data.primary}
        var previous = {}
        var oldDark = host.palette.dark
        var oldMidlight = host.palette.midlight
        try {
            for (var key in colors) { previous[key] = host[key]; host[key] = colors[key] }
            // Text selection colors are allowed to be subtle. Progress must
            // stay legible when the inherited selection palette matches its track.
            host.palette.highlight = data.panel
            host.palette.dark = data.panel
            host.palette.midlight = data.panel
            downloads.append({id:"visible-progress",state:"in-progress",bytes:"1024",total:4096,
                              speed:0,destination:"/tmp/report.bin",reason:"",displayName:"report.bin",
                              active:true,fileReady:false,retryAvailable:true,retryPending:false,finishing:false})
            var surface = create(downloadFixture)
            var list = findChild(surface,"downloadList")
            tryCompare(list,"count",1)
            var progress = findChild(list.itemAtIndex(0),"downloadProgressBar")
            verify(progress.visible)
            compare(progress.position,0.25)
            capture("downloads-progress-" + data.tag)
            // Measure the user-visible window rather than an isolated control
            // grab, which can be transparent in software-rendered Qt styles.
            var image = grabImage(host.contentItem)
            var scaleX = image.width / host.contentItem.width
            var scaleY = image.height / host.contentItem.height
            var fill = progress.mapToItem(host.contentItem,progress.width / 8,progress.height / 2)
            var track = progress.mapToItem(host.contentItem,progress.width * 3 / 4,progress.height / 2)
            function luminance(point) {
                function channel(value) {
                    value /= 255
                    return value <= 0.04045 ? value / 12.92 : Math.pow((value + 0.055) / 1.055,2.4)
                }
                var x = Math.floor(point.x * scaleX)
                var y = Math.floor(point.y * scaleY)
                return 0.2126 * channel(image.red(x,y)) + 0.7152 * channel(image.green(x,y))
                     + 0.0722 * channel(image.blue(x,y))
            }
            var a = luminance(fill)
            var b = luminance(track)
            var contrast = (Math.max(a,b) + 0.05) / (Math.min(a,b) + 0.05)
            verify(contrast >= 3, "Rendered progress fill is indistinguishable from its track: " + contrast)
        } finally {
            for (var key in previous) host[key] = previous[key]
            host.palette.highlight = Qt.binding(function() { return host.accentColor })
            host.palette.dark = oldDark
            host.palette.midlight = oldMidlight
        }
    }
    function test_unknown_download_size_stops_animating_when_paused() {
        downloads.append({ id: "active", state: "in-progress", bytes: "4096", total: -1,
                           speed: 1024, destination: "/tmp/archive.zip", reason: "", displayName: "archive.zip", active: true, fileReady: false, retryAvailable: true, retryPending: false, finishing: false })
        var surface = create(downloadFixture)
        var list = findChild(surface, "downloadList")
        tryCompare(list, "count", 1)
        var progress = findChild(list.itemAtIndex(0), "downloadProgressBar")
        verify(progress.visible && progress.indeterminate)
        downloads.setProperty(0, "state", "paused")
        tryCompare(progress, "indeterminate", false)
        capture("downloads-paused")
    }
    function test_download_failure_details_remain_available() {
        downloads.append({ id: "interrupted", state: "interrupted", bytes: "4096", total: 8192,
                           speed: 0, destination: "/tmp/archive.zip", reason: "Destination is not writable", displayName: "archive.zip", active: false, fileReady: false, retryAvailable: true, retryPending: false, finishing: false })
        var surface = create(downloadFixture)
        var list = findChild(surface, "downloadList")
        tryCompare(list, "count", 1)
        var row = list.itemAtIndex(0)
        var details = findChild(row, "downloadDetails")
        verify(!details.visible)
        mouseClick(findChild(row, "downloadDetailsButton"))
        tryVerify(function() { return details.visible })
        verify(details.text.indexOf("Destination is not writable") >= 0)
    }
    SignalSpy { id: downloadCancelSpy; signalName: "cancelRequestRequested" }
    function test_download_preparation_can_be_cancelled_and_prevents_duplicate_retries() {
        downloads.append({id:"retry-source",state:"cancelled",bytes:"0",total:0,speed:0,
            destination:"/tmp/report",reason:"",displayName:"report",active:false,fileReady:false,
            retryAvailable:false,retryPending:true,finishing:false})
        var surface = create(downloadFixture)
        surface.preparingDownload = true
        downloadCancelSpy.target = surface
        downloadCancelSpy.clear()
        var list = findChild(surface, "downloadList")
        tryCompare(list, "count", 1)
        tryVerify(function() { return list.itemAtIndex(0) !== null })
        var retry = findChild(list.itemAtIndex(0), "downloadRetryButton")
        compare(retry.text, "Retrying…")
        verify(!retry.enabled)
        var cancel = findChild(surface, "downloadCancelRequestButton")
        verify(cancel.visible)
        verify(waitForPolish(host))
        mouseClick(cancel)
        compare(downloadCancelSpy.count, 1)
        surface.preparingDownload = false
        downloads.setProperty(0, "retryPending", false)
        downloads.setProperty(0, "retryAvailable", true)
        tryVerify(function() { return retry.enabled })
        verify(!cancel.visible)
    }
    function test_empty_downloads_have_an_explanation() {
        var surface = create(downloadFixture)
        verify(findChild(surface, "downloadsEmptyState").visible)
        capture("downloads-empty")
    }
    function test_private_sessions_explain_why_saving_is_unavailable() {
        host.temporaryProfile = true
        var surface = create(sessionsFixture)
        verify(!findChild(surface, "sessionSaveButton").enabled)
        verify(surface.message.indexOf("regular profile") >= 0)
    }
    function test_switcher_readable_actions_retain_canonical_intents() {
        var surface = create(switcherFixture)
        var list = findChild(surface, "switcherResultList")
        tryCompare(list, "count", 1)
        var row = list.itemAtIndex(0)
        verify(row)
        var action = findChild(row, "switcherActionButton")
        compare(action.text, "Preview session")
        mouseClick(action)
        compare(switcherAction, "load-preview")
        verify(row.Accessible.name.indexOf("<b>Research</b>") >= 0)
        surface.results = []
        tryCompare(list, "count", 0)
        verify(findChild(surface, "switcherEmptyState").visible)
    }
    function test_shared_headers_render_page_text_literally() {
        var surface = create(navigationFixture)
        surface.title = "<b>Page title</b>"
        surface.message = "<img src='https://example.invalid/tracker'>"
        compare(findChild(surface, "dialogTitleLabel").textFormat, Text.PlainText)
        compare(findChild(surface, "dialogMessageLabel").textFormat, Text.PlainText)
    }
}
