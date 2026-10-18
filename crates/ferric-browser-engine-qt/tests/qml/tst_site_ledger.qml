import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
import "../../qml/scripts/ChromePresentation.js" as Presentation

TestCase {
    id: test
    name: "SiteLedgerPolish"
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
        property bool siteLedgerVisible: true
        property bool siteDataClearPending: false
        property var siteLedgerData: ({})
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

    Component { id: fixture; FerricSiteLedger { browserWindow: host; siteExperimentAvailable: true } }
    Component { id: spyFixture; SignalSpy {} }
    function sampleData() {
        return { private: false, origin: "https://example.test", capture: {tab_id: "tab-id", document_id: "document-id", profile_id: "profile-id"},
            facts: [
                {id: "network.blocking", value: {enabled: true, active_site_blocked_requests: 5}},
                {id: "permissions", value: {ready: true, durable_rules: []}},
                {id: "capture", state: "unknown", capability: "unavailable", value: {status: "unavailable"}},
                {id: "page.userscripts", state: "unknown", value: {status: "available", matching_active_site: true}},
                {id: "compatibility.workarounds", value: {status: "available", active_ids: []}}
            ],
            interaction: {mode: "normal", focus: {category: "editable"}, shortcut_policy: {value: "mixed", source: "site"}},
            blocking: {active_site_decisions: []},
            site_doctor: {active: null, last_result: null, experiments: ["blocking-bypass", "compiled-defaults", "userscripts-off", "fresh-view"]}
        }
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width = 900; host.height = 700; host.font.pointSize = 10
        host.siteLedgerVisible = true
        host.siteLedgerData = sampleData()
        host.requestActivate()
        tryCompare(host, "active", true)
    }
    function create() {
        var surface = createTemporaryObject(fixture, host.contentItem)
        verify(surface)
        verify(waitForPolish(host))
        return surface
    }
    function reveal(surface, item) {
        var scroll = findChild(surface, "siteLedgerScroll")
        item.forceActiveFocus()
        surface.revealFocusedControl()
        verify(waitForPolish(host))
        tryVerify(function() {
            var point = item.mapToItem(scroll, 0, 0)
            return point.y >= -1 && point.y + Math.min(item.height, scroll.height) <= scroll.height + 1
        }, 1000, item.objectName + " must scroll into view")
    }
    function test_layout_data() {
        return [{tag: "wide", width: 900, height: 700, font: 10},
                {tag: "compact", width: 640, height: 480, font: 10},
                {tag: "narrow", width: 390, height: 600, font: 10},
                {tag: "large-text", width: 390, height: 700, font: 16}]
    }
    function test_layout(row) {
        host.width = row.width; host.height = row.height; host.font.pointSize = row.font
        var surface = create()
        var scroll = findChild(surface, "siteLedgerScroll")
        verify(scroll.contentHeight > scroll.height)
        var summary = findChild(surface, "siteFactSummary-network.blocking")
        verify(summary.text.indexOf("5 requests blocked") >= 0)
        verify(findChild(surface, "siteFactSummary-capture").text.indexOf("unavailable") >= 0)
        verify(findChild(surface, "siteFactSummary-permissions").text.indexOf("No saved rules") >= 0)
        verify(findChild(surface, "siteFactSummary-compatibility.workarounds").text.indexOf("No compatibility adjustments") >= 0)
        verify(findChild(surface, "siteFactSummary-page.userscripts").text.indexOf("does not confirm they ran") >= 0)
        var details = findChild(surface, "siteTechnicalDetails")
        verify(!details.visible)
        if (testArtifactDirectory.length > 0)
            grabImage(host.contentItem).save(testArtifactDirectory + "/site-ledger-" + row.tag + ".png")
        var copy = findChild(surface, "siteReportCopyButton")
        reveal(surface, copy)
        verify(copy.width <= scroll.width)
        var toggle = findChild(surface, "siteTechnicalDetailsButton")
        reveal(surface, toggle)
        mouseClick(toggle)
        tryCompare(details, "visible", true)
        compare(details.textFormat, TextEdit.PlainText)
        verify(details.readOnly && details.selectByMouse)
        verify(details.text.indexOf("document-id") >= 0)
        reveal(surface, details)
    }
    function test_keyboard_keeps_focused_actions_in_view() {
        host.width = 390; host.height = 600
        var surface = create()
        var close = findChild(surface, "siteLedgerCloseButton")
        var copy = findChild(surface, "siteReportCopyButton")
        close.forceActiveFocus()
        for (var i = 0; i < 30 && !copy.activeFocus; ++i) keyClick(Qt.Key_Tab)
        verify(copy.activeFocus)
        var scroll = findChild(surface, "siteLedgerScroll")
        var point = copy.mapToItem(scroll, 0, 0)
        verify(point.y >= -1 && point.y + copy.height <= scroll.height + 1)
        var spy = createTemporaryObject(spyFixture, surface, {target: surface, signalName: "sanitizedReportCopyRequested"})
        keyClick(Qt.Key_Return)
        compare(spy.count, 1)
        compare(spy.signalArguments[0][0], false)
        var consent = findChild(surface, "siteReportHost")
        consent.forceActiveFocus()
        keyClick(Qt.Key_Return)
        compare(consent.checked, true)
        keyClick(Qt.Key_Enter)
        compare(consent.checked, false)
    }
    function test_consents_reset_for_changed_site_document_and_proposal() {
        var value = sampleData()
        value.site_doctor.last_result = {id: "fix-1", kind: "blocker-exception", origin: value.origin, state: "pending-confirmation"}
        host.siteLedgerData = value
        var surface = create()
        var hostConsent = findChild(surface, "siteReportHost")
        var clearConsent = findChild(surface, "siteDataClearConfirmation")
        var fixConsent = findChild(surface, "siteDoctorProposalConfirmation")
        hostConsent.checked = true; clearConsent.checked = true; fixConsent.checked = true
        value = sampleData()
        value.site_doctor.last_result = {id: "fix-2", kind: "blocker-exception", origin: value.origin, state: "pending-confirmation"}
        host.siteLedgerData = value
        compare(fixConsent.checked, false)
        compare(hostConsent.checked, true)
        value = sampleData(); value.capture.document_id = "new-document"
        host.siteLedgerData = value
        compare(hostConsent.checked, false); compare(clearConsent.checked, false)
        hostConsent.checked = true; clearConsent.checked = true
        value = sampleData(); value.origin = "https://other.test"
        host.siteLedgerData = value
        compare(hostConsent.checked, false); compare(clearConsent.checked, false)
    }
    function test_private_data_is_withheld_and_copy_cannot_disclose_host() {
        var value = sampleData(); value.private = true; value.origin = null
        value.interaction.private_url_forbidden = "https://private.test/?token=secret"
        host.siteLedgerData = value
        var surface = create()
        verify(findChild(surface, "siteFactSummary-network.blocking").text.indexOf("withheld") >= 0)
        var hostConsent = findChild(surface, "siteReportHost")
        verify(!hostConsent.visible)
        hostConsent.checked = true
        var spy = createTemporaryObject(spyFixture, surface, {target: surface, signalName: "sanitizedReportCopyRequested"})
        findChild(surface, "siteReportCopyButton").clicked()
        compare(spy.signalArguments[0][0], false)
        var details = findChild(surface, "siteTechnicalDetails")
        verify(details.text.indexOf("document-id") < 0)
        verify(details.text.indexOf("private.test") < 0)
        verify(findChild(surface, "siteLedgerInteractionSummary").text.indexOf("private.test") < 0)
    }
    function test_site_tests_emit_canonical_intents_and_require_confirmation() {
        var value = sampleData()
        value.site_doctor.last_result = {id: "fix-reviewed", kind: "blocker-exception", origin: value.origin, state: "pending-confirmation"}
        host.siteLedgerData = value
        var surface = create()
        var spy = createTemporaryObject(spyFixture, surface, {target: surface, signalName: "siteDoctorExperimentRequested"})
        var action = findChild(surface, "siteExperiment-compiled-defaults")
        reveal(surface, action); mouseClick(action)
        compare(spy.signalArguments[0][0], "compiled-defaults")
        var apply = findChild(surface, "siteDoctorApplyButton")
        verify(!apply.enabled)
        findChild(surface, "siteDoctorProposalConfirmation").checked = true
        verify(apply.enabled)
        var fixSpy = createTemporaryObject(spyFixture, surface, {target: surface, signalName: "siteDoctorProposalApplyRequested"})
        reveal(surface, apply); mouseClick(apply)
        compare(fixSpy.signalArguments[0][0], "fix-reviewed")
        verify(!findChild(surface, "siteDoctorProposalConfirmation").checked)
    }
    function test_saved_permission_availability_data() {
        return [{tag:"loading",status:"pending",expected:"Checking saved permission rules…"},
            {tag:"unavailable",status:"unavailable",expected:"Saved permission rules unavailable"},
            {tag:"unchecked",status:"unknown",expected:"Saved permission rules haven't been fully checked"}]
    }
    function test_saved_permission_availability(data) {
        var fact={id:"permissions",value:{ready:false,status:data.status,durable_rules:[]}}
        compare(Presentation.siteFactSummary(fact,false,true),data.expected)
        compare(Presentation.siteFactSummary(fact,true,true),"Site details are withheld in private browsing")
    }
    function test_unknown_facts_do_not_become_success() {
        compare(Presentation.siteFactSummary({id: "capture", value: {}}, false, true), "Sharing status unavailable")
        compare(Presentation.siteFactSummary({id: "permissions", value: {}}, false, true), "Saved permission rules haven't been fully checked")
        compare(Presentation.siteFactTitle("constructor"), "Site information")
        compare(Presentation.siteExperimentLabel("constructor"), "Temporary site test")
        verify(Presentation.siteFactSummary({id: "network.blocking", value: {enabled: false, active_site_blocked_requests: 0}}, true, true).indexOf("0 requests") < 0)
    }
}
