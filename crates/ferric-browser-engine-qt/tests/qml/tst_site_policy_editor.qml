import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
import "../../qml/scripts/SitePolicyBuilder.js" as Builder

TestCase {
    id: test
    name: "SitePolicyEditor"
    when: windowShown
    ApplicationWindow {
        id: host
        width: 1120; height: 900; visible: true
        property bool sitePoliciesVisible: false
        property real chromeScale: 1
        property color panelColor: "#202020"
        property color borderColor: "#808080"
        property color accentColor: "#80c0ff"
        property color primaryTextColor: "white"
        property color secondaryTextColor: "#dddddd"
        property color mutedTextColor: "#bbbbbb"
        property color warningColor: "#ffff80"
        property color selectionColor: "#404080"
        property color selectionTextColor: "white"
        property var editor: null
    }
    QtObject {
        id: api
        property bool profile_bootstrap_pending: false
        property string site_policy_result: ""
        property string status_text: ""
        property int tab_count:1
        property int active_tab_index:0
        property var requests: []
        property var commands: []
        property var views: ({})
        function site_policy_editor_request(payload) {
            var request = JSON.parse(payload)
            var token = "request-" + requests.length
            requests = requests.concat([{token:token, payload:request}])
            return token
        }
        function execute_interactive_command(command) { commands = commands.concat([command]); return true }
        function site_policy_draft_snapshot(id) { return JSON.stringify(views[id] || {error:"unknown draft"}) }
        function tab_id_for_index(index) {return index===0?"tab-1":""}
    }
    Component { id: editorType; FerricSitePolicyWorkspace { browserWindow: host; browserUi: api } }
    function typeText(text) { for (var index = 0; index < text.length; ++index) keyClick(text[index].toUpperCase().charCodeAt(0)) }
    function view(id, readOnly) {
        var rule = {id:"player", pattern:"https://example.test/*", priority:0, set:{"zoom.default":1.25}, interaction:{enabled:true}}
        return {id:id, revision:1, source:readOnly ? "authored:0" : "temporary", configuration_revision:7,
            profile_revision:3, read_only:readOnly === true, dirty:false, dirty_fields:[], validation:null,
            operation:null, required_capabilities:[], rebased_from:null,
            rules:[{id:"player", original:rule, copied_from:null, proposed:JSON.parse(JSON.stringify(rule)), resolution:null}]}
    }
    function init() {
        api.requests = []; api.commands = []; api.views = ({}); api.profile_bootstrap_pending = false
        host.editor = editorType.createObject(host.contentItem)
        verify(host.editor !== null)
        host.editor.receive({workspace:true, profile_instance:"profile-1", desired:[], drafts:[], persistence:"session-only"})
        host.editor.show("https://example.test/")
        host.requestActivate()
    }
    function cleanup() { host.editor.destroy(); host.editor = null; host.sitePoliciesVisible = false; wait(0) }
    function open(id, readOnly) {
        var snapshot = view(id, readOnly); api.views[id] = snapshot
        host.editor.addView(snapshot)
        // Select the editor using its actual tab control.
        var tabs = findChild(host.editor, "policyEditorTab")
        mouseClick(tabs)
        wait(20)
        return host.editor.selected
    }
    function ack(index, result, error) {
        api.site_policy_result = JSON.stringify({request:api.requests[index].token, result:result, error:error})
        wait(0)
    }
    function expand(path) {
        var include = findChild(host.editor, "policyInclude:" + path)
        if (include && !include.checked) include.click()
        var button = findChild(host.editor, "policyExpand:" + path)
        verify(button !== null, path)
        if (button.text === "Edit") button.click()
        wait(0)
    }
    function test_background_refresh_does_not_reopen_workspace_or_replace_edits() {
        var session=open("refresh",false)
        session.proposals[0].priority=12;session.editEpoch++
        host.editor.close();api.commands=[]
        host.editor.refresh()
        compare(api.commands.length,1)
        compare(api.commands[0],'site-policy-list --all --url "https://example.test/"')
        host.editor.receive({catalogue:true,profile_instance:"profile-1",configuration_revision:8,
            desired:[],drafts:[{id:"refresh"}],persistence:"session-only"})
        compare(host.sitePoliciesVisible,false)
        compare(host.editor.selected,session)
        compare(session.proposals[0].priority,12)
        compare(session.editEpoch,1)
        compare(host.editor.catalogue.configuration_revision,8)
        compare(host.editor.catalogue.drafts[0].id,"refresh")
    }
    function test_nested_fields_explicit_clear_and_section_removal() {
        var session = open("first", false)
        expand("rules.0.interaction")
        expand("rules.0.interaction.anchors")
        findChild(host.editor, "policyAdd:rules.0.interaction.anchors").click(); wait(0)
        expand("rules.0.interaction.anchors.0.predicates")
        expand("rules.0.interaction.anchors.0.predicates.attributes")
        findChild(host.editor, "policyAdd:rules.0.interaction.anchors.0.predicates.attributes").click(); wait(0)
        var attribute = findChild(host.editor, "policyField:rules.0.interaction.anchors.0.predicates.attributes.0.name")
        verify(attribute !== null); attribute.forceActiveFocus(); attribute.selectAll(); typeText("role")
        compare(session.proposals[0].interaction.anchors[0].predicates.attributes[0].name, "role")
        expand("rules.0.interaction.collections")
        findChild(host.editor, "policyAdd:rules.0.interaction.collections").click(); wait(0)
        expand("rules.0.interaction.collections.0.primary_root_steps")
        findChild(host.editor, "policyAdd:rules.0.interaction.collections.0.primary_root_steps").click(); wait(0)
        verify(findChild(host.editor, "policyField:rules.0.interaction.collections.0.primary_root_steps.0.kind") !== null)
        findChild(host.editor, "policyClear:rules.0.interaction.anchors").click()
        compare(session.proposals[0].interaction.anchors.length, 0)
        host.editor.submit(session, null)
        compare(api.requests[0].payload.proposals[0].interaction.anchors.length, 0)
        verify(api.requests[0].payload.proposals[0].set === undefined)
        ack(0, view("first", false))
        findChild(host.editor, "policyInclude:rules.0.interaction").click()
        verify(session.proposals[0].interaction === undefined)
        host.editor.review(session)
        var diff = JSON.parse(session.diff)
        compare(diff.rules[0].proposed.set["zoom.default"], 1.25)
        verify(diff.rules[0].proposed.interaction === undefined)
    }
    function test_invalid_numeric_input_blocks_submission_and_draft_switch() {
        var session = open("first", false)
        var number = findChild(host.editor, "policyField:rules.0.priority")
        number.forceActiveFocus(); number.selectAll(); keyClick(Qt.Key_Minus)
        verify(host.editor.hasErrors(session))
        host.editor.submit(session, {action:"validate"})
        compare(api.requests.length, 0)
        host.editor.openDraft("other")
        compare(host.editor.selected, session)
        compare(number.text, "-")
        number.selectAll(); keyClick(Qt.Key_4)
        verify(!host.editor.hasErrors(session))
        compare(session.proposals[0].priority, 4)
    }
    function test_invalid_nested_number_blocks_list_restructuring() {
        var session = open("list-edit", false)
        var path = "rules.0.interaction.anchors"
        expand("rules.0.interaction")
        expand(path)
        var add = findChild(host.editor, "policyAdd:" + path)
        add.click(); wait(0)
        add.click(); wait(0)
        expand(path + ".0.predicates")
        expand(path + ".0.predicates.width_px")
        var number = findChild(host.editor, "policyField:" + path + ".0.predicates.width_px.min")
        number.forceActiveFocus(); number.selectAll(); keyClick(Qt.Key_Minus)
        verify(host.editor.hasErrors(session))
        var down = findChild(host.editor, "policyMoveDown:" + path + ".0")
        var up = findChild(host.editor, "policyMoveUp:" + path + ".1")
        var remove = findChild(host.editor, "policyRemove:" + path + ".1")
        verify(!add.enabled)
        verify(!down.enabled)
        verify(!up.enabled)
        verify(!remove.enabled)
        compare(number.text, "-")
        compare(session.proposals[0].interaction.anchors[0].predicates.width_px.min, 0)
        host.editor.submit(session, {action:"validate"})
        compare(api.requests.length, 0)

        number.selectAll(); keyClick(Qt.Key_4)
        verify(!host.editor.hasErrors(session))
        verify(add.enabled && down.enabled && up.enabled && remove.enabled)
        down.click(); wait(0)
        compare(session.proposals[0].interaction.anchors[1].predicates.width_px.min, 4)
        verify(session.proposals[0].interaction.anchors[0].predicates === undefined)
        host.editor.submit(session, null)
        compare(api.requests.length, 1)
        compare(api.requests[0].payload.proposals[0].interaction.anchors[1].predicates.width_px.min, 4)
    }
    function test_refresh_receipt_and_new_draft_keep_native_control_and_undo() {
        var first = open("first", false)
        var input = findChild(host.editor, "policyField:rules.0.pattern")
        verify(input !== null)
        input.forceActiveFocus(); input.selectAll(); typeText("https://changed.test/*")
        compare(input.text, "https://changed.test/*")
        verify(input.activeFocus)
        compare(first.proposals[0].pattern, "https://changed.test/*")
        input.select(8, 15)
        var selection = input.selectedText
        host.editor.receive({workspace:true, profile_instance:"profile-1", desired:[], drafts:[]})
        first.view = Object.assign({}, first.view, {operation:{revision:1, state:"reload-required", saved:true, active:false}})
        var second = open("second", false)
        verify(second !== first)
        host.editor.selectedIndex = 0; wait(0)
        compare(findChild(host.editor, "policyField:rules.0.pattern"), input)
        compare(input.selectedText, selection)
        verify(input.canUndo)
        input.undo()
        compare(input.text, "https://example.test/*")
        compare(second.proposals[0].pattern, "https://example.test/*")
    }
    function test_apply_captures_reviewed_revision_and_retains_newer_edit() {
        var session = open("first", false)
        session.proposals[0].pattern = "https://reviewed.test/*"; session.editEpoch++
        host.editor.review(session)
        verify(session.diff.indexOf("zoom.default") >= 0)
        host.editor.submit(session, {action:"apply", activate:true})
        compare(api.requests[0].payload.proposals[0].pattern, "https://reviewed.test/*")
        session.proposals[0].pattern = "https://newer.test/*"; session.editEpoch++
        var result = view("first", false); result.revision = 2
        ack(0, result)
        compare(api.commands[0], "site-policy-apply first --revision 2 --activate")
        compare(session.proposals[0].pattern, "https://newer.test/*")
        verify(session.editEpoch !== session.storedEpoch)
        verify(session.reviewedEpoch !== session.editEpoch)
    }
    function test_rejection_keeps_text_selection_and_draft() {
        var session = open("first", false)
        var input = findChild(host.editor, "policyField:rules.0.pattern")
        input.forceActiveFocus(); input.selectAll(); typeText("https://invalid.test/*"); input.select(0, 5)
        host.editor.submit(session, {action:"validate"})
        ack(0, null, "configuration changed; rebase explicitly")
        compare(input.text, "https://invalid.test/*")
        compare(input.selectedText, "https")
        verify(!session.pending)
        verify(host.editor.notice.indexOf("rebase") >= 0)
    }
    function test_authored_is_read_only_and_copy_is_explicit() {
        var session = open("authored", true)
        var input = findChild(host.editor, "policyField:rules.0.pattern")
        verify(input.readOnly)
        verify(!findChild(host.editor, "policyApply").enabled)
        host.editor.submit(session, {action:"copy", destination:"temporary", new_id:"player-copy"})
        compare(api.requests[0].payload.action, "copy")
        compare(api.requests[0].payload.destination, "temporary")
        compare(api.commands.length, 0)
    }
    function test_preview_marks_unsubmitted_edits_and_copy_opens_full_diff() {
        var session = open("first", false)
        session.view = Object.assign({}, session.view, {preview:{revision:1, kind:"static-only"}})
        verify(!host.editor.previewIsStale(session))
        var input = findChild(host.editor, "policyField:rules.0.priority")
        input.forceActiveFocus(); input.selectAll(); keyClick(Qt.Key_4)
        verify(host.editor.previewIsStale(session))
        host.editor.submit(session, {action:"copy", destination:"temporary", new_id:null})
        var patched = view("first", false); patched.revision = 2
        ack(0, patched)
        compare(api.requests[1].payload.action, "copy")
        var copied = view("copy", false)
        copied.rules[0].copied_from = copied.rules[0].original
        copied.rules[0].original = null
        copied.rules[0].resolution = {source:"temporary", winner:copied.rules[0].proposed}
        ack(1, copied)
        compare(host.editor.selected.view.id, "copy")
        var diff = JSON.parse(host.editor.selected.diff)
        compare(diff.rules[0].proposed.set["zoom.default"], 1.25)
        compare(diff.rules[0].prospective_resolution.source, "temporary")
        compare(host.editor.selected.reviewedEpoch, -1)
        verify(!findChild(host.editor, "policyApply").enabled)
    }
    function test_receipts_distinguish_failure_activation_and_durability() {
        verify(host.editor.operationSummary({state:"failed",saved:false}).indexOf("before commit") >= 0)
        verify(host.editor.operationSummary({state:"reload-required",saved:true}).indexOf("not active") >= 0)
        verify(host.editor.operationSummary({state:"reload-required",saved:true,durability_uncertain:true}).indexOf("durability could not be confirmed") >= 0)
        verify(host.editor.operationSummary({state:"applied",saved:false}).indexOf("memory only") >= 0)
    }
    function builder(kind) {
        findChild(host.editor, "policyBuilderTab").click()
        var item = host.editor.guidedBuilder
        item.begin(kind)
        compare(api.requests[api.requests.length-1].payload.action, "scope")
        ack(api.requests.length-1, {origin:"https://example.test",exact_pattern:"https://example.test/",site_pattern:"https://example.test/*"})
        return item
    }
    function test_builder_requires_scope_and_creates_only_an_inactive_local_proposal() {
        var item = builder("anchor")
        item.create()
        compare(api.requests.length, 1)
        verify(item.notice.indexOf("Choose exact") >= 0)
        var scope = findChild(item, "policyBuilderScope")
        scope.currentIndex = 0; scope.activated(0)
        compare(findChild(item, "policyBuilderPattern").text, "https://example.test/")
        findChild(item, "policyBuilderId").text = "navigation"
        var name = findChild(item, "policyField:builder.id")
        verify(name !== null); name.forceActiveFocus(); typeText("nav")
        var code = findChild(item, "policyField:builder.code")
        code.forceActiveFocus(); typeText("m")
        item.create()
        compare(api.requests[1].payload.source, "temporary")
        compare(api.requests[1].payload.pattern, "https://example.test/")
        ack(1, view("built", false))
        compare(host.editor.selected.proposals[0].interaction.enabled, false)
        compare(host.editor.selected.proposals[0].interaction.anchors[0].id, "nav")
        compare(host.editor.selected.proposals[0].interaction.anchors[0].code, "m")
        verify(host.editor.selected.editEpoch !== host.editor.selected.storedEpoch)
        compare(api.commands.length, 0)
    }
    function test_code_array_helper_is_explicit_bounded_and_prefix_free() {
        var codes = Builder.codes("numeric",1,3,4,"unused")
        compare(codes.length,12); compare(codes[0],"01"); compare(codes[11],"12")
        var letters = Builder.codes("letters",1,3,3,"as")
        compare(letters[0],"aaaa"); compare(letters[8],"saaa")
        for (var i=0;i<letters.length;++i) for (var j=0;j<letters.length;++j)
            if (i !== j) verify(letters[j].indexOf(letters[i]) !== 0)
        verifyException(function() { Builder.codes("numeric",1,9,8,"") })
        verifyException(function() { Builder.codes("letters",1,1,2,"aa") })
        verifyException(function() { Builder.codes("letters",1,1,2,"åb") })
    }
    function verifyException(callback) { var rejected = false; try { callback() } catch(error) { rejected = true }; verify(rejected) }
    function test_key_capture_consumes_chords_until_the_source_release_without_commands() {
        open("mapping",false)
        var item = builder("remap")
        findChild(item,"policyCaptureSourceKey").click()
        keyPress(Qt.Key_Q,Qt.ControlModifier)
        compare(findChild(item,"policyBuilderFrom").text,"Ctrl+q")
        compare(api.requests[1].payload.action,"key")
        keyRelease(Qt.Key_Control)
        compare(item.captureField,"from")
        keyRelease(Qt.Key_Q,Qt.ControlModifier)
        compare(item.captureField,"")
        ack(1,{canonical:"Ctrl+q",qualifications:[]})
        findChild(item,"policyCaptureDestinationKey").click()
        keyClick(Qt.Key_K)
        ack(2,{canonical:"k",qualifications:[]})
        item.seed.id = "play"
        item.add()
        compare(host.editor.selected.proposals[0].interaction.remaps[0].from,"Ctrl+q")
        compare(host.editor.selected.proposals[0].interaction.remaps[0].to,"k")
        compare(host.editor.selected.proposals[0].interaction.enabled,true)
        compare(api.commands.length,0)
    }
    function test_capture_advice_is_plain_text_and_preserves_unchosen_predicates() {
        var session=open("capture",false),item=builder("anchor")
        item.seed.predicates={role:"button"}
        item.acceptCapture({tag:"a",hint_name:"<b>name</b>; quit",root:{start:"top-document",steps:[]},root_complete:true,
            selectors:[{selector:'a[aria-label="<b>name</b>"]',count:2,complete:true}],slot_paths:[],ancestors:[],
            attributes:[{name:"aria-label",value:"<img src=x onerror=alert(1)>",present:true}],geometry:{x:1,y:1,width:10,height:10}})
        var choice=findChild(item,"policyCapturedSelector")
        choice.currentIndex=0
        compare(choice.contentItem.textFormat,Text.PlainText)
        verify(choice.displayText.indexOf("2 matches")>=0)
        findChild(item,"policyUseCapturedFacts").click()
        compare(item.seed.target_selector,'a[aria-label="<b>name</b>"]')
        compare(item.seed.predicates.role,"button")
        verify(item.seed.predicates.hint_name===undefined)
        verify(session.proposals[0].interaction.anchors===undefined)
        compare(api.commands.length,0)
    }
    function test_profile_change_retires_old_ui_and_pending_reply() {
        var session = open("first", false)
        host.editor.follow(session, {action:"validate"})
        host.editor.receive({workspace:true, profile_instance:"profile-2", desired:[], drafts:[]})
        compare(host.editor.sessions.length, 0)
        ack(0, view("first", false))
        compare(host.editor.sessions.length, 0)
    }
    function test_kept_samples_are_explicit_bounded_advice_and_never_enter_drafts() {
        var session=open("first",false),item=host.editor.guidedBuilder
        item.begin("anchor")
        var facts={version:1,location:"https://example.test/path",tag:"a",root:{start:"top-document",steps:[]},root_complete:true,
            selectors:[{selector:"#stable",count:1,complete:true},{selector:"a",count:4,complete:true}],
            hint_name:"private page text",attributes:[],ancestors:[],slot_paths:[]}
        item.acceptCapture(facts)
        compare(item.keptSamples.length,0)
        findChild(item,"policyKeepSample").click()
        compare(item.keptSamples.length,1)
        verify(JSON.stringify(item.keptSamples).indexOf("private page text")<0)
        facts=JSON.parse(JSON.stringify(facts));facts.selectors[0].count=2;facts.selectors.pop()
        item.acceptCapture(facts)
        var comparison=Builder.compareSamples(item.keptSamples,item.capturedFacts)
        verify(comparison.indexOf("kept 1; current 2")>=0)
        verify(comparison.indexOf("not evaluated")>=0)
        for(var index=1;index<8;index++)findChild(item,"policyKeepSample").click()
        compare(item.keptSamples.length,8)
        verify(!findChild(item,"policyKeepSample").enabled)
        compare(session.editEpoch,0)
        verify(JSON.stringify(session.proposals).indexOf("#stable")<0)
        item.reset();compare(item.keptSamples.length,0)
    }
    function test_page_test_rejects_unstored_local_changes_without_leaving_editor() {
        var session=open("first",false)
        session.editEpoch++
        host.editor.startPageTest(session,"play","passthrough")
        compare(api.requests.length,0)
        verify(host.sitePoliciesVisible)
        verify(host.editor.notice.indexOf("Apply the current reviewed edits")>=0)
    }
    function test_cancelled_page_test_ignores_a_late_failure() {
        open("first",false)
        host.editor.pending["late"]={action:"test-page-ready",test:{}}
        host.sitePoliciesVisible=false
        host.editor.receive({request:"late",error:"owner changed"})
        verify(!host.sitePoliciesVisible)
        compare(api.commands.length,0)
    }
    function test_builder_sample_text_allows_tab_navigation_without_inserting_text() {
        open("first",false)
        findChild(host.editor,"policyBuilderTab").click()
        host.editor.guidedBuilder.begin("policy")
        var input=findChild(host.editor,"policyBuilderSamples")
        input.text="https://example.test/a";input.forceActiveFocus()
        verify(input.activeFocus)
        keyClick(Qt.Key_Tab)
        verify(!input.activeFocus)
        compare(input.text,"https://example.test/a")
        keyClick(Qt.Key_Backtab,Qt.ShiftModifier)
        verify(input.activeFocus)
    }
    function test_layout_and_focus_data() {
        return [{tag:"narrow",width:380,height:720,font:18,scale:1},
            {tag:"large-text",width:600,height:760,font:28,scale:1.5},
            {tag:"high-contrast",width:560,height:720,font:20,scale:1,contrast:true}]
    }
    function test_header_tab_navigation_and_live_tab_identity() {
        open("first",false)
        var header=findChild(host.editor,"dialogMessageLabel")
        header.forceActiveFocus();keyClick(Qt.Key_Tab)
        verify(!header.activeFocus)
        findChild(host.editor,"policyLivePreview").click()
        compare(api.commands[0],'site-policy-preview first --tab "tab-1"')
    }
    function test_new_capture_controls_join_tab_order_and_focus_stays_in_dialog() {
        open("first",false);findChild(host.editor,"policyBuilderTab").click()
        var builder=host.editor.guidedBuilder
        builder.begin("anchor")
        findChild(builder,"policyBuilderCapture").forceActiveFocus()
        builder.acceptCapture({version:1,tag:"a",hint_name:"Home",root:{start:"top-document",steps:[]},root_complete:true,
            selectors:[{selector:"#home",count:1,complete:true}],ancestors:[],slot_paths:[],attributes:[]})
        var selector=findChild(builder,"policyCapturedSelector")
        for(var step=0;step<10&&!selector.activeFocus;++step)keyClick(Qt.Key_Tab)
        verify(selector.activeFocus,"Newly visible capture selector must participate in Tab order")
        findChild(builder,"policyBuilderAdd").forceActiveFocus();keyClick(Qt.Key_Tab)
        var item=host.activeFocusItem
        while(item&&item!==host.editor)item=item.parent
        compare(item,host.editor)
    }
    function test_collection_uses_the_selected_slot_root_and_relative_counts() {
        open("first",false);findChild(host.editor,"policyBuilderTab").click()
        var builder=host.editor.guidedBuilder;builder.begin("collection")
        var slot={start:"top-document",steps:[{kind:"open-shadow",host_selector:"#host"},
            {kind:"assigned-slot",host_selector:"slot[name=entry]",assigned_selector:"#assigned"}]}
        var primary={available:true,primary_self:false,primary_root_steps:[],suggestions:[{selector:"a",count:1,complete:true}]}
        builder.acceptCapture({version:1,tag:"a",hint_name:"Home",attributes:[],root:{start:"top-document",steps:[]},root_complete:true,selectors:[],
            ancestors:[{level:0,tag:"a",attributes:[],selectors:[],primary:primary},
                {level:1,tag:"article",attributes:[],selectors:[{selector:"article.card",count:4,complete:true}],primary:primary}],
            slot_paths:[{root:slot,ancestor_level:2,selectors:[],relative_ancestors:[null,{selectors:[{selector:"article.card",count:2,complete:true}]}]}]})
        var root=findChild(builder,"policyCapturedRoot");root.currentIndex=1;root.activated(1)
        findChild(builder,"policyCapturedItem").currentIndex=1
        var item=findChild(builder,"policyCapturedItemSelector");item.currentIndex=0
        verify(item.displayText.indexOf("2 matches")>=0)
        findChild(builder,"policyCapturedRootContainer").click()
        findChild(builder,"policyCapturedPrimary").currentIndex=0
        findChild(builder,"policyUseCapturedFacts").click()
        compare(JSON.stringify(builder.seed.root),JSON.stringify(slot))
        compare(builder.seed.container_self,true)
        compare(builder.seed.item_selector,"article.card")
        compare(builder.seed.primary_selector,"a")
    }
    function test_layout_and_focus(row) {
        var original={width:host.width,height:host.height,point:host.font.pointSize,pixel:host.font.pixelSize,
            scale:host.chromeScale,accent:host.accentColor,panel:host.panelColor}
        try {
            host.width=row.width;host.height=row.height;host.font.pixelSize=row.font;host.chromeScale=row.scale
            if(row.contrast){host.accentColor="yellow";host.panelColor="black"}
            open("first",false)
            var input=findChild(host.editor,"policyField:rules.0.pattern")
            input.forceActiveFocus();input.selectAll();typeText("https://changed.test/*");input.select(8,15)
            var selection=input.selectedText
            findChild(host.editor,"policyBuilderTab").click();host.editor.guidedBuilder.begin("policy")
            var ruleId=findChild(host.editor,"policyBuilderId")
            ruleId.forceActiveFocus();wait(50)
            verify(host.editor.panelItem.width<=host.width)
            verify(host.editor.panelItem.height<=host.height)
            verify(host.editor.bodyItem.height>50)
            var position=ruleId.mapToItem(host.editor.bodyItem,0,0)
            verify(position.y>=-1&&position.y+ruleId.height<=host.editor.bodyItem.height+1,
                "Focused rule ID must be visible after scrolling")
            compare(ruleId.Accessible.name,"New builder rule ID")
            compare(input.selectedText,selection);verify(input.canUndo)
            compare(host.editor.panelItem.border.color,host.accentColor)
        } finally {
            host.width=original.width;host.height=original.height;host.chromeScale=original.scale
            host.accentColor=original.accent;host.panelColor=original.panel
            if(original.pixel>0)host.font.pixelSize=original.pixel;else host.font.pointSize=original.point
        }
    }
}
