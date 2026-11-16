import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/SitePolicyEditorSchema.js" as PolicySchema

FerricModalSurface {
    id: workspace
    objectName: "sitePolicyWorkspace"
    customTabTraversal: true
    required property var browserUi
    property var catalogue: ({})
    property string profileInstance: ""
    property string contextUrl: ""
    property string notice: ""
    property string explanation: ""
    property var sessions: []
    property int selectedIndex: -1
    readonly property var selected: selectedIndex >= 0 ? sessions[selectedIndex] : null
    property var pending: ({})
    readonly property alias guidedBuilder: policyBuilder
    property string captureId: ""
    property string captureDraft: ""
    property int captureGeneration: -1
    property var pageTest: null
    property bool focusedPreview:false
    ListModel { id: sessionRows }
    signal closed()
    signal previewCloseRequested()

    visible: hostWindow.sitePoliciesVisible
    enabled: !!browserUi && !browserUi.profile_bootstrap_pending
    commandText: ":site-policy"
    title: "Site Policies"
    message: "Drafts do not change page behavior. Review the full rule before applying."
    keyHelp: "tab/shift-tab controls · esc close and keep drafts"
    dialogWidth: 1120 * scale
    dialogHeight: 800 * scale
    stackingOrder: 77
    initialFocusItem: refreshButton
    onDismissRequested: close()

    function close() { hostWindow.sitePoliciesVisible = false; closed() }
    function show(url) { contextUrl = url || ""; hostWindow.sitePoliciesVisible = true }
    function quoted(text) { return JSON.stringify(String(text)) }
    function command(text) {
        var accepted=browserUi.execute_interactive_command(text)
        if (!accepted) notice = browserUi.status_text || "Request rejected"
        return accepted
    }
    function finishFocusedPreview() {
        if(!focusedPreview)return
        focusedPreview=false
        if(!browserUi.profile_bootstrap_pending)show(contextUrl)
    }
    function refresh() { command("site-policy-list --all" + (contextUrl ? " --url " + quoted(contextUrl) : "")) }
    function hasErrors(session) { return Object.keys(session.errors).length > 0 }
    function previewIsStale(session) {
        return !!session.view.preview && (session.editEpoch !== session.storedEpoch
            || session.view.preview.revision !== session.view.revision)
    }
    function operationSummary(operation) {
        if (!operation) return "Not applied"
        var states = {
            "preparing":"Preparing the reviewed revision",
            "writing":"Saving the reviewed revision",
            "activation-pending":"Configuration activation pending",
            "applied":"Configuration active",
            "reload-required":"Changes are not active; configuration reload required",
            "failed":"Save failed before commit; draft retained",
            "completion-unknown":"Save completion is unknown; inspect the saved state before retrying"
        }
        return (states[operation.state] || operation.state)
            + (operation.saved === true ? ". Saved to disk" : operation.saved === false && operation.state !== "failed" ? " (memory only)" : "")
            + (operation.durability_uncertain ? ". The file was committed, but durability could not be confirmed." : "")
    }
    function busy(session) {
        var operation = session.view.operation
        return session.pending || session.applyWait > 0 || (!!operation && ["preparing", "writing", "activation-pending"].indexOf(operation.state) >= 0)
    }
    function request(payload, context) {
        var token = browserUi.site_policy_editor_request(JSON.stringify(payload))
        if (!token) { notice = "Profile unavailable; the draft is retained."; return false }
        pending[token] = context || {}
        if (context && context.session) context.session.pending = true
        return true
    }
    function addView(view) {
        for (var index = 0; index < sessions.length; ++index) {
            if (sessions[index].view.id === view.id) { sessions[index].view = view; selectedIndex = index; return sessions[index] }
        }
        if (sessions.length >= 16) { notice = "Discard a draft before opening another editor (limit 16)."; return null }
        var session = sessionType.createObject(workspace)
        session.view = view
        // Assign mutable JS values directly. Initial-property QVariant conversion
        // would detach nested maps from the native controls editing them.
        session.proposals = proposalsFor(view)
        sessions = sessions.concat([session]); sessionRows.append({entry:session}); selectedIndex = sessions.length - 1
        return session
    }
    function proposalsFor(view) {
        return view.rules.map(function(entry) {
            var basis = entry.proposed || entry.original || entry.copied_from
            return {pattern:basis.pattern, priority:basis.priority,
                interaction:entry.proposed && entry.proposed.interaction ? PolicySchema.clone(entry.proposed.interaction) : null}
        })
    }
    function openDraft(id) {
        if (selected && hasErrors(selected)) { notice = "Correct the invalid input before switching drafts."; return }
        var view = JSON.parse(browserUi.site_policy_draft_snapshot(id))
        if (view.error) notice = view.error
        else addView(view)
    }
    function openRule(source, id, isNew, pattern) {
        if (selected && hasErrors(selected)) { notice = "Correct the invalid input before switching drafts."; return }
        request({action:"open", source:source, id:id, new:isNew, pattern:isNew ? pattern : null}, {action:"open"})
    }
    function submit(session, next) {
        if (!session || session.pending || hasErrors(session)) return
        if (session.editEpoch === session.storedEpoch) { follow(session, next); return }
        if (request({action:"patch", draft:session.view.id, revision:session.view.revision,
                proposals:PolicySchema.clone(session.proposals)},
                {action:"patch", session:session, epoch:session.editEpoch, next:next})) {
            session.locked = next && (next.action === "rebase" || next.action === "copy")
        }
    }
    function follow(session, next) {
        if (!next) return
        if (next.action === "preview") {
            if(next.pageFocus){
                focusedPreview=true;hostWindow.sitePoliciesVisible=false
                var root=hostWindow.rootWindow||hostWindow
                root.restoreOverlayFocus(hostWindow)
            }
            if(!command("site-policy-preview " + session.view.id + (next.tab ? " --tab " + quoted(next.tab) : "")))finishFocusedPreview()
            notice = next.tab ? "Native preview requested. Configuration is unchanged." : "Static preview requested. Live matching is not tested."
        } else if (next.action === "apply") {
            session.applyWait = 4
            command("site-policy-apply " + session.view.id + " --revision " + session.view.revision + (next.activate ? " --activate" : ""))
            notice = "Apply requested for revision " + session.view.revision + ". Check the saved and active receipt below."
        } else {
            var payload = Object.assign({}, next, {draft:session.view.id, revision:session.view.revision})
            if (request(payload, {action:next.action, session:session})) session.locked = next.action === "rebase" || next.action === "copy"
        }
    }
    function review(session) {
        if (!session || hasErrors(session)) return
        var rules = session.view.rules.map(function(entry, index) {
            var rule = PolicySchema.clone(entry.proposed || entry.original || entry.copied_from)
            var proposal = session.proposals[index]
            rule.pattern = proposal.pattern; rule.priority = proposal.priority; rule.interaction = proposal.interaction
            if (!PolicySchema.present(proposal, "interaction") && Object.keys(rule.set || {}).length === 0) rule = null
            return {id:entry.id, original:entry.original, copied_from:entry.copied_from,
                proposed:rule, prospective_resolution:entry.resolution}
        })
        session.diff = JSON.stringify({source:session.view.source, configuration_revision:session.view.configuration_revision,
            profile_revision:session.view.profile_revision, rules:rules, rebased_from:session.view.rebased_from}, null, 2)
        session.reviewedEpoch = session.editEpoch
        notice = "Review includes ordinary settings and whole-rule shadowing. Apply uses this exact draft revision."
    }
    function addBuilderValue(kind, value, admitHintKey) {
        var session = selected
        if (!session || session.view.read_only || busy(session) || hasErrors(session)) { notice = "Choose an editable draft first."; return }
        var proposal = session.proposals[session.ruleIndex]
        var patch = proposal.interaction ? PolicySchema.clone(proposal.interaction) : {enabled:false}
        if (kind === "labels") patch.labels = value
        else {
            var field = kind === "anchor" ? "anchors" : kind === "collection" ? "collections" : "remaps"
            patch[field] = (patch[field] || []).concat([value])
            if (kind === "remap" && admitHintKey) patch.hint_page_keys = (patch.hint_page_keys || []).concat([value.from]).filter(function(token,index,array) { return array.indexOf(token) === index })
        }
        proposal.interaction = patch; session.editEpoch++; session.formGeneration++
        sections.currentIndex = 1
        notice = "Added to the local draft. Review, validate and preview before applying."
    }
    function startCapture(family) {
        if (!selected || busy(selected)) { notice="Choose a stable draft before capturing a page target.";return }
        var view=hostWindow.activeWebView()
        if (!view || !view.stableTabId) { notice="Select a live HTTP(S) tab first.";return }
        captureId="";captureDraft=selected.view.id;captureGeneration=policyBuilder.generation
        request({action:"capture",draft:captureDraft,revision:selected.view.revision,tab:view.stableTabId,family:family},
            {action:"builder-capture",generation:captureGeneration})
    }
    function startPageTest(session, mapping, context) {
        if (!session || busy(session) || hasErrors(session) || session.editEpoch!==session.storedEpoch) {
            notice="Apply the current reviewed edits before testing on the page.";return
        }
        var view=hostWindow.activeWebView()
        if (!view || !view.stableTabId) { notice="Select a live page first.";return }
        request({action:"test-page",draft:session.view.id,revision:session.view.revision,
            rule:session.view.rules[session.ruleIndex].id,mapping:mapping,context:context},
            {action:"test-page",session:session,epoch:session.editEpoch,profile:profileInstance,
                view:view,tab:view.stableTabId,url:String(view.url),mapping:mapping})
    }
    function pageTestCurrent(test) {
        var view=hostWindow.activeWebView()
        return profileInstance===test.profile && !browserUi.profile_bootstrap_pending
            && view===test.view && view.stableTabId===test.tab && String(view.url)===test.url
            && selected===test.session && test.session.editEpoch===test.epoch
    }
    function matchingPageRoute(test) {
        var route=JSON.parse(browserUi.page_key_route(test.source)||"{}")
        return route.allowed===true&&route.context===test.context
            &&route.source===test.source&&route.destination===test.destination ? route : null
    }
    function pageTestFailed(reason) {
        pageTest=null
        browserUi.execute_interactive_command("mode-enter normal")
        notice="Page test unavailable: "+reason+". Draft retained."
        hostWindow.sitePoliciesVisible=true
    }
    Timer {
        interval:100;repeat:true;running:workspace.pageTest!==null
        onTriggered: {
            var test=workspace.pageTest
            var root=workspace.hostWindow.rootWindow||workspace.hostWindow
            if (!workspace.pageTestCurrent(test) || workspace.visible) { workspace.pageTestFailed("page, focus owner or draft changed");return }
            if (!workspace.hostWindow.active) { workspace.pageTest=null;workspace.notice="Page test cancelled: window focus changed. Request a fresh test.";return }
            if (test.waiting) return
            if (!test.entered) {
                test.entered=true
                var mode={"passthrough-once":"passthrough-once","passthrough":"passthrough","normal-page-key":"normal","hint-page-key":"normal"}[test.context]
                if (!workspace.browserUi.execute_interactive_command("mode-enter "+mode)
                        || (test.context==="hint-page-key"&&!workspace.browserUi.execute_interactive_command("hint"))) {
                    workspace.pageTestFailed("the requested mode could not be entered");return
                }
                return
            }
            if (!test.receiverPrepared) {
                if (!test.preparing) {
                    test.preparing=true
                    if (!workspace.request({action:"test-page",draft:test.session.view.id,revision:test.session.view.revision,
                        rule:test.session.view.rules[test.session.ruleIndex].id,mapping:test.mapping,context:test.context,
                        prepare_receiver:true,tab:test.tab},{action:"test-page-receiver",test:test})) workspace.pageTestFailed("the receiver could not be prepared")
                }
                return
            }
            if (!test.focusReturned) {
                var focusController=root.runtime_focusOverlayController
                if (!focusController.focusPermitted(workspace.hostWindow,test.view)) { workspace.pageTestFailed("another native surface owns focus");return }
                focusController.contentFocusTarget(test.view).forceActiveFocus()
                test.focusReturned=true
                root.probeActiveFocus(workspace.browserUi,workspace.browserUi.active_tab_index,test.view)
            }
            var route=workspace.matchingPageRoute(test)
            if (route) {
                test.waiting=true
                if (!workspace.request({action:"test-page",draft:test.session.view.id,revision:test.session.view.revision,
                    rule:test.session.view.rules[test.session.ruleIndex].id,mapping:test.mapping,context:test.context},
                    {action:"test-page-ready",test:test})) workspace.pageTestFailed("the active draft could not be checked")
                return
            }
            if (++test.attempts>=20) {
                var unavailable=JSON.parse(workspace.browserUi.page_key_route(test.source)||"{}")
                workspace.pageTestFailed(unavailable.reason||"the active mapping or eligible noneditable receiver is unavailable");return
            }
            workspace.hostWindow.browserKeyRouterObject.probePageReceiver(test.view,
                root.focusProbeScript(),workspace.browserUi.input_epoch)
        }
    }
    function acceptCapture(facts) {
        if (!selected || selected.view.id!==captureDraft || policyBuilder.generation!==captureGeneration || facts.capture_id!==captureId) {
            notice="The capture belongs to an older builder or draft; request a fresh sample.";return
        }
        policyBuilder.acceptCapture(facts)
    }
    function receive(value) {
        if (value.workspace === true || value.catalogue === true) {
            var owner = value.profile_instance || ""
            if (profileInstance && profileInstance !== owner) {
                var retired = sessions; sessionRows.clear(); sessions = []; selectedIndex = -1; pending = ({})
                explanation = ""; notice = ""
                policyBuilder.reset()
                retired.forEach(function(session) { session.destroy() })
            }
            profileInstance = owner; catalogue = value
            return
        }
        if (!value.request) {
            if (value.error) { notice=value.error;finishFocusedPreview() }
            else if (value.kind === "static-only" || value.kind === "live" || value.kind === "live-unavailable") {
                notice = value.status === "pending-live" ? "Native preview pending for draft revision " + value.revision : value.message
                sessions.forEach(function(session) {
                    if (session.view.id !== value.draft) return
                    var snapshot = JSON.parse(browserUi.site_policy_draft_snapshot(value.draft))
                    if (!snapshot.error) session.view = snapshot
                })
            }
            else if (value.id && value.rules) {
                if (!selected || !hasErrors(selected)) addView(value)
                else notice = "Draft ready: " + value.id + ". Correct invalid input before switching."
            } else if (value.status) notice = JSON.stringify(value)
            else if (value.activation_permitted === false) explanation = JSON.stringify(value, null, 2)
            return
        }
        if (!value.request || !pending[value.request]) return
        var context = pending[value.request]; delete pending[value.request]
        var session = context.session
        if (session) { session.pending = false; session.locked = false }
        if (value.error) {
            if (context.action==="test-page-ready"||context.action==="test-page-receiver") { if (pageTest===context.test) pageTestFailed(value.error) }
            else notice=value.error
            return
        }
        var result = value.result
        if (context.action === "test-page-receiver") {
            if (pageTest!==context.test) return
            if (!pageTestCurrent(context.test)||visible) { pageTestFailed("page test owner changed");return }
            context.test.receiverPrepared=true
            return
        }
        if (context.action === "test-page-ready") {
            var test=context.test,route=matchingPageRoute(test)
            if (pageTest!==test) return
            if (!pageTestCurrent(test)||visible||!hostWindow.active||!route) { pageTestFailed("receiver or active mapping changed");return }
            pageTest=null
            notice="Ready for a physical "+test.source+" → "+test.destination+" test in "+test.context+" on the selected page. Its noneditable receiver is eligible. Delivery has not been tested. Ctrl+Esc returns to Normal."
            hostWindow.showCommandNotice(notice,false)
            return
        }
        if (context.action === "test-page") {
            if (!pageTestCurrent(context)) { notice="Page or draft changed; request a fresh page test.";return }
            pageTest=Object.assign({},context,result,{attempts:0,entered:false})
            previewCloseRequested();close()
            return
        }
        if (context.action === "builder") { context.builder.receive(result, context); return }
        if (context.action === "builder-capture") {
            if (context.generation!==policyBuilder.generation) { previewCloseRequested();return }
            captureId=result.id
            return
        }
        if (context.action === "builder-open") {
            var built = addView(result)
            if (built) {
                built.proposals[0].interaction = context.interaction
                built.editEpoch++; built.formGeneration++
                sections.currentIndex = 1
                notice = "Inactive draft created. Enable it deliberately, then review and preview before applying."
            }
            return
        }
        if (context.action === "discard") {
            sessionRows.remove(sessions.indexOf(session))
            sessions = sessions.filter(function(item) { return item !== session })
            selectedIndex = sessions.length ? 0 : -1; session.destroy(); refresh(); return
        }
        if (context.action === "open" || context.action === "copy") {
            var opened = addView(result)
            if (opened && context.action === "copy") {
                review(opened)
                opened.reviewedEpoch = -1
                notice = "Copy created an inactive snapshot. Review its preserved ordinary settings and prospective winning source before applying."
            }
            refresh(); return
        }
        if (!session) return
        session.view = result
        if (context.action === "patch") session.storedEpoch = context.epoch
        if (context.action === "rebase") {
            session.proposals = proposalsFor(result); session.formGeneration++
            session.errors = ({}); session.editEpoch++; session.storedEpoch = session.editEpoch
            review(session); session.reviewedEpoch = -1
            notice = "Rebased explicitly. Review the before/after diff and apply separately."
        }
        follow(session, context.next)
    }
    Component {
        id: sessionType
        QtObject {
            property var view
            property var proposals
            property int editEpoch: 0
            property int storedEpoch: 0
            property int reviewedEpoch: -1
            property int formGeneration: 0
            property var errors: ({})
            property bool pending: false
            property bool locked: false
            property int applyWait: 0
            property string diff: ""
            property int ruleIndex: 0
            function issue(path, message, subtree) {
                var next = Object.assign({}, errors)
                Object.keys(next).forEach(function(key) { if (key === path || (subtree && key.indexOf(path + ".") === 0)) delete next[key] })
                if (message) next[path] = message
                errors = next
            }
        }
    }
    Connections {
        target: workspace.browserUi
        function onSite_policy_resultChanged() {
            try { workspace.receive(JSON.parse(workspace.browserUi.site_policy_result)) }
            catch (error) { workspace.notice = "Could not read the policy response: " + error }
        }
        function onProfile_bootstrap_pendingChanged() {
            if (!workspace.browserUi.profile_bootstrap_pending) return
            workspace.pageTest=null
            workspace.focusedPreview=false
            policyBuilder.reset()
            workspace.pending = ({})
            workspace.sessions.forEach(function(session) { session.pending = false; session.locked = false; session.applyWait = 0 })
            if (workspace.visible) workspace.close()
        }
    }
    Timer {
        interval: 500; repeat: true
        running: workspace.visible && workspace.enabled && !!workspace.selected && workspace.busy(workspace.selected)
        onTriggered: {
            var session = workspace.selected
            if (session.pending) return
            var view = JSON.parse(workspace.browserUi.site_policy_draft_snapshot(session.view.id))
            if (!view.error) session.view = view // Metadata only: never replace proposals or controls.
            if (session.applyWait > 0) session.applyWait--
        }
    }
    ColumnLayout {
        anchors.fill: parent
        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true; wrapMode: Text.Wrap; textFormat: Text.PlainText
                text: "Catalogue snapshot: desired revision " + (workspace.catalogue.configuration_revision || 0)
                    + " · active revision " + (workspace.catalogue.active_revision || 0)
                    + " · " + (workspace.catalogue.persistence || "session-only")
            }
            Button { id: refreshButton; text: "Refresh"; objectName: "policyRefresh"; onClicked: workspace.refresh() }
            Button { text: "Close"; objectName: "policyClose"; onClicked: workspace.close() }
        }
        Label {
            Layout.fillWidth: true; visible: text.length > 0; text: workspace.notice
            textFormat: Text.PlainText; wrapMode: Text.Wrap; Accessible.name: text
        }
        TabBar {
            id: sections
            Layout.fillWidth: true
            TabButton { text: "Rules and sources" }
            TabButton { text: "Draft editor"; objectName: "policyEditorTab" }
            TabButton { text: "Import / export" }
            TabButton { text: "Explain" }
            TabButton { text: "Guided builder"; objectName: "policyBuilderTab" }
        }
        StackLayout {
            Layout.fillWidth: true; Layout.fillHeight: true
            currentIndex: sections.currentIndex
            FerricScrollColumn {
                hostWindow: workspace.hostWindow
                Label { Layout.fillWidth: true; text: "Exact source rules"; font.bold: true }
                Label { Layout.fillWidth: true; wrapMode: Text.Wrap; text: "Same-ID rules replace whole lower rules before URL matching. Authored files are read-only. Refresh preserves unsaved editor text." }
                Repeater {
                    model: workspace.catalogue.desired || []
                    delegate: Button {
                        id: ruleButton
                        required property var modelData
                        Layout.fillWidth: true
                        text: modelData.source + " / " + modelData.id + " · " + modelData.rule.pattern
                            + (modelData.shadowed_by ? " · shadowed by " + modelData.shadowed_by.source : modelData.retired_by ? " · retired by " + modelData.retired_by : modelData.matched ? " · matches" : " · does not match")
                        Accessible.name: text
                        contentItem: Label { text: ruleButton.text; textFormat: Text.PlainText; wrapMode: Text.Wrap }
                        onClicked: { workspace.openRule(modelData.source, modelData.id, false, ""); sections.currentIndex = 1 }
                    }
                }
                Label { text: "New rule"; font.bold: true }
                Button { text: "Open guided builder"; onClicked: sections.currentIndex = 4 }
                ComboBox { id: newSource; model: ["temporary", "generated"]; Accessible.name: "New rule destination" }
                FerricTextField { id: newId; Layout.fillWidth: true; placeholderText: "New unique rule ID"; Accessible.name: "New rule ID"; objectName: "policyNewId" }
                FerricTextField { id: newPattern; Layout.fillWidth: true; placeholderText: "https://example.test/*"; Accessible.name: "New HTTP(S) pattern"; objectName: "policyNewPattern" }
                Button { text: "Create draft"; onClicked: { workspace.openRule(newSource.currentText, newId.text, true, newPattern.text); sections.currentIndex = 1 } }
            }
            ColumnLayout {
                RowLayout {
                    Layout.fillWidth: true
                    ComboBox {
                        id: draftPicker
                        Layout.fillWidth: true
                        model: workspace.sessions.map(function(session) { return session.view.source + " / " + session.view.rules.map(function(rule) { return rule.id }).join(", ") })
                        currentIndex: workspace.selectedIndex
                        Accessible.name: "Open policy draft"
                        onActivated: {
                            if (workspace.selected && workspace.hasErrors(workspace.selected)) { workspace.notice = "Correct invalid input before switching drafts."; currentIndex = workspace.selectedIndex }
                            else workspace.selectedIndex = currentIndex
                        }
                    }
                    ComboBox {
                        id: storedDrafts
                        model: workspace.catalogue.drafts || []
                        textRole: "id"; Accessible.name: "Rust draft catalogue"
                    }
                    Button { text: "Open draft"; enabled: storedDrafts.currentIndex >= 0; onClicked: workspace.openDraft(storedDrafts.currentText) }
                }
                Item {
                    Layout.fillWidth: true; Layout.fillHeight: true
                    Repeater {
                        model: sessionRows
                        delegate: FerricScrollColumn {
                            id: draftForm
                            required property var entry
                            required property int index
                            readonly property var session: entry
                            anchors.fill: parent
                            visible: index === workspace.selectedIndex
                            hostWindow: workspace.hostWindow
                            Label {
                                Layout.fillWidth: true; wrapMode: Text.Wrap; textFormat: Text.PlainText
                                text: "Draft " + draftForm.session.view.id + " · revision " + draftForm.session.view.revision
                                    + " · base configuration " + draftForm.session.view.configuration_revision
                                    + " / profile " + draftForm.session.view.profile_revision
                                    + (draftForm.session.view.read_only ? " · read-only" : "")
                                    + (draftForm.session.editEpoch !== draftForm.session.storedEpoch ? " · unsubmitted edits" : "")
                            }
                            Label {
                                Layout.fillWidth: true; wrapMode: Text.Wrap; textFormat: Text.PlainText
                                text: "Dirty fields: " + draftForm.session.view.dirty_fields.join(", ")
                                    + "\nRequired capabilities: " + draftForm.session.view.required_capabilities.join(", ")
                                    + "\n" + (draftForm.session.view.validation ? draftForm.session.view.validation.message : "Not validated at this revision")
                                    + "\n" + workspace.operationSummary(draftForm.session.view.operation)
                            }
                            Label {
                                Layout.fillWidth: true; visible: text.length > 0; wrapMode: Text.Wrap; textFormat: Text.PlainText
                                text: Object.keys(draftForm.session.errors).map(function(key) { return key + ": " + draftForm.session.errors[key] }).join("\n")
                            }
                            ComboBox {
                                Layout.fillWidth: true; model: draftForm.session.view.rules.map(function(rule) { return rule.id })
                                currentIndex: draftForm.session.ruleIndex; Accessible.name: "Rule within draft batch"
                                enabled: !workspace.hasErrors(draftForm.session)
                                onActivated: draftForm.session.ruleIndex = currentIndex
                            }
                            Repeater {
                                model: draftForm.session.proposals.length
                                delegate: Loader {
                                    id: ruleForm
                                    required property int index
                                    Layout.fillWidth: true
                                    visible: index === draftForm.session.ruleIndex
                                    property bool visited: false
                                    onVisibleChanged: if (visible) visited = true
                                    Component.onCompleted: if (visible) visited = true
                                    active: visited
                                    // Rebuild only after an explicitly requested rebase, never on a receipt.
                                    property int generation: draftForm.session.formGeneration
                                    onGenerationChanged: { active = false; active = visited }
                                    sourceComponent: FerricPolicyValue {
                                        spec: ({type:"proposal"})
                                        container: draftForm.session.proposals
                                        fieldKey: ruleForm.index
                                        fieldPath: "rules." + ruleForm.index
                                        readOnly: draftForm.session.view.read_only || draftForm.session.locked
                                        errors: draftForm.session.errors
                                        onEdited: draftForm.session.editEpoch++
                                        onIssue: function(path, message, subtree) { draftForm.session.issue(path, message, subtree) }
                                    }
                                }
                            }
                            Flow {
                                Layout.fillWidth: true; Layout.preferredHeight: childrenRect.height; spacing: 6
                                Button { text: "Update draft"; objectName: "policyUpdate"; enabled: !draftForm.session.view.read_only && !draftForm.session.pending && !workspace.hasErrors(draftForm.session); onClicked: workspace.submit(draftForm.session, null) }
                                Button { text: "Validate"; objectName: "policyValidate"; enabled: !draftForm.session.pending && !workspace.hasErrors(draftForm.session); onClicked: workspace.submit(draftForm.session, {action:"validate"}) }
                                Button { text: "Review full diff"; objectName: "policyReview"; enabled: !workspace.hasErrors(draftForm.session); onClicked: workspace.review(draftForm.session) }
                                Button { text: "Rebase explicitly"; objectName: "policyRebase"; enabled: !draftForm.session.view.read_only && !workspace.busy(draftForm.session) && !workspace.hasErrors(draftForm.session); onClicked: workspace.submit(draftForm.session, {action:"rebase"}) }
                                Button { text: "Discard draft"; objectName: "policyDiscard"; enabled: !workspace.busy(draftForm.session); onClicked: workspace.request({action:"discard", draft:draftForm.session.view.id}, {action:"discard", session:draftForm.session}) }
                            }
                            CheckBox { id: activate; text: "Activate configuration after save (no webpage reload)";objectName:"policyActivate" }
                            ComboBox {
                                id: previewTab
                                Layout.fillWidth: true
                                model: workspace.browserUi.tab_count || 0
                                currentIndex: workspace.browserUi.active_tab_index === undefined ? -1 : workspace.browserUi.active_tab_index
                                displayText:currentIndex>=0?"Tab "+(currentIndex+1):"Choose a tab"
                                delegate:ItemDelegate { required property int index;width:previewTab.width;text:"Tab "+(index+1) }
                                Accessible.name: "Live preview tab"
                            }
                            Flow {
                                Layout.fillWidth: true; Layout.preferredHeight: childrenRect.height; spacing: 6
                                Button { text: "Static preview"; objectName: "policyStaticPreview"; enabled: !draftForm.session.pending && !workspace.hasErrors(draftForm.session); onClicked: workspace.submit(draftForm.session, {action:"preview", tab:null}) }
                                Button { text: "Preview on tab"; objectName: "policyLivePreview"; enabled: previewTab.currentIndex >= 0 && previewTab.currentIndex<workspace.browserUi.tab_count && !draftForm.session.pending && !workspace.hasErrors(draftForm.session); onClicked: workspace.submit(draftForm.session, {action:"preview", tab:workspace.browserUi.tab_id_for_index(previewTab.currentIndex)}) }
                                Button { text:"Preview focused frame on selected tab";objectName:"policyFocusedPreview";enabled:previewTab.currentIndex>=0&&previewTab.currentIndex===workspace.browserUi.active_tab_index&&!draftForm.session.pending&&!workspace.hasErrors(draftForm.session);onClicked:workspace.submit(draftForm.session,{action:"preview",tab:workspace.browserUi.tab_id_for_index(previewTab.currentIndex),pageFocus:true}) }
                                Button { text: "Close preview"; onClicked: workspace.previewCloseRequested() }
                            }
                            Label { Layout.fillWidth:true;wrapMode:Text.Wrap;text:"Focused-frame preview temporarily returns native focus to the selected page, then returns here. It does not focus a DOM target or activate anything." }
                            Label { Layout.fillWidth:true;wrapMode:Text.Wrap;text:"Test an applied key mapping separately: close the editor, check the actual mode and eligible receiver, then press the source key yourself. Hint page keys are reserved before label input; raw passthrough bypasses mappings. Ctrl+Esc returns to Normal." }
                            ComboBox {
                                id:testMapping;Layout.fillWidth:true;Accessible.name:"Mapping to test";objectName:"policyTestMapping"
                                model: {
                                    var epoch=draftForm.session.editEpoch
                                    var patch=draftForm.session.proposals[draftForm.session.ruleIndex].interaction
                                    return patch&&patch.remaps?patch.remaps.map(function(value){return value.id}):[]
                                }
                            }
                            ComboBox { id:testContext;Layout.fillWidth:true;model:["passthrough-once","passthrough","normal-page-key","hint-page-key"];Accessible.name:"Page test context";objectName:"policyTestContext" }
                            Button {
                                text:"Test mapping on page";objectName:"policyTestOnPage"
                                enabled:testMapping.currentIndex>=0&&!workspace.busy(draftForm.session)&&!workspace.hasErrors(draftForm.session)
                                onClicked:workspace.startPageTest(draftForm.session,testMapping.currentText,testContext.currentText)
                            }
                            TextArea {
                Keys.priority:Keys.BeforeItem
                Keys.onPressed:function(event){workspace.handleTabKey(event)};
                                Layout.fillWidth: true; visible: !!draftForm.session.view.preview
                                readOnly: true; selectByMouse: true; persistentSelection: true; textFormat: TextEdit.PlainText; wrapMode: TextEdit.Wrap
                                text: draftForm.session.view.preview ? JSON.stringify(draftForm.session.view.preview, null, 2) : ""
                                Accessible.name: "Nonactivating preview receipt"; objectName: "policyPreviewReceipt"
                            }
                            Label {
                                Layout.fillWidth: true; wrapMode: Text.Wrap
                                visible: workspace.previewIsStale(draftForm.session)
                                text: "This preview is from an older version. Preview the current edits before relying on its results."
                                objectName: "policyPreviewStale"
                            }
                            Button {
                                text: "Apply reviewed revision"; objectName: "policyApply"
                                enabled: !draftForm.session.view.read_only && !workspace.busy(draftForm.session)
                                    && !workspace.hasErrors(draftForm.session) && draftForm.session.reviewedEpoch === draftForm.session.editEpoch
                                onClicked: workspace.submit(draftForm.session, {action:"apply", activate:activate.checked})
                            }
                            Label { Layout.fillWidth: true; wrapMode: Text.Wrap; text: "Copy creates an inactive snapshot, with no live inheritance. A same-ID copy preserves the exact destination's ordinary settings, or those of the highest lower source if the destination is absent. It shadows the entire lower rule; a higher source can still win. A new ID copies only Interaction. Review the resulting full diff." }
                            ComboBox { id: copyDestination; model: ["temporary", "generated"]; Accessible.name: "Copy destination" }
                            FerricTextField { id: cloneId; Layout.fillWidth: true; placeholderText: "Optional new ID (single rule only)"; Accessible.name: "Clone rule ID" }
                            Button {
                                text: "Copy to new draft"; enabled: !draftForm.session.pending && !workspace.hasErrors(draftForm.session)
                                onClicked: workspace.submit(draftForm.session, {action:"copy", destination:copyDestination.currentText, new_id:cloneId.text || null})
                            }
                            TextArea {
                Keys.priority:Keys.BeforeItem
                Keys.onPressed:function(event){workspace.handleTabKey(event)};
                                Layout.fillWidth: true; Layout.preferredHeight: Math.max(160, implicitHeight)
                                readOnly: true; selectByMouse: true; textFormat: TextEdit.PlainText; wrapMode: TextEdit.Wrap
                                text: draftForm.session.diff; Accessible.name: "Reviewed full rule diff"; objectName: "policyDiff"
                            }
                        }
                    }
                }
            }
            FerricScrollColumn {
                hostWindow: workspace.hostWindow
                Label { Layout.fillWidth: true; wrapMode: Text.Wrap; text: "Import creates an inactive draft. Export writes a local file, including in private sessions. Files are never watched as configuration sources." }
                FerricTextField { id: filePath; Layout.fillWidth: true; placeholderText: "Absolute local path"; Accessible.name: "Policy import or export path" }
                CheckBox { id: replace; text: "Review replacement of existing IDs on import" }
                ComboBox { id: importDestination; model: ["temporary", "generated"]; Accessible.name: "Import destination" }
                Button { text: "Import draft"; onClicked: { workspace.command("site-policy-import " + workspace.quoted(filePath.text) + (importDestination.currentText === "temporary" ? " --temp" : "") + (replace.checked ? " --replace-existing" : "")); workspace.notice = "Import requested. Refresh the draft catalogue when the operation completes." } }
                CheckBox { id: overwrite; text: "Allow replacing the export file" }
                CheckBox { id: persist; text: "I understand export persists this draft to disk" }
                Button {
                    text: "Export selected draft"; enabled: !!workspace.selected && persist.checked && !workspace.selected.pending && !workspace.hasErrors(workspace.selected)
                    onClicked: {
                        if (workspace.selected.editEpoch !== workspace.selected.storedEpoch) { workspace.notice = "Update the draft before exporting its immutable revision."; return }
                        workspace.command("site-policy-export " + workspace.quoted(filePath.text) + " --draft " + workspace.selected.view.id + (overwrite.checked ? " --overwrite" : ""))
                    }
                }
            }
            FerricScrollColumn {
                hostWindow: workspace.hostWindow
                Label {
                    Layout.fillWidth: true; wrapMode: Text.Wrap
                    text: "Explain effective values, source boundaries, reserved codes and key ownership. Inspection never submits a key or activates a target. The snapshot remains selectable until the next explicit request."
                }
                ComboBox { id: explainKind; model: ["Effective policy", "Key token", "Hint code"]; Accessible.name: "Explanation subject" }
                FerricTextField { id: explainToken; Layout.fillWidth: true; visible: explainKind.currentIndex > 0; Accessible.name: "Key token or hint code" }
                CheckBox { id: explainExact; enabled: !!workspace.selected; text: "Include the exact selected source / rule" }
                Button {
                    text: "Explain snapshot"; objectName: "policyExplain"
                    onClicked: {
                        var command = "site-policy-explain" + (workspace.contextUrl ? " --url " + workspace.quoted(workspace.contextUrl) : "")
                        if (explainKind.currentIndex > 0) command += (explainKind.currentIndex === 1 ? " --key " : " --code ") + workspace.quoted(explainToken.text)
                        if (explainExact.checked && workspace.selected) command += " --source " + workspace.selected.view.source + " --id " + workspace.selected.view.rules[workspace.selected.ruleIndex].id
                        workspace.command(command)
                    }
                }
                TextArea {
                Keys.priority:Keys.BeforeItem
                Keys.onPressed:function(event){workspace.handleTabKey(event)};
                    Layout.fillWidth: true; readOnly: true; selectByMouse: true; persistentSelection: true
                    wrapMode: TextEdit.Wrap; textFormat: TextEdit.PlainText
                    text: workspace.explanation; Accessible.name: "Policy explanation snapshot"
                }
            }
            FerricSitePolicyBuilder {
                id: policyBuilder
                workspace: workspace
                onDraftRequested: function(source, id, pattern, interaction) {
                    workspace.request({action:"open",source:source,id:id,new:true,pattern:pattern},
                        {action:"builder-open",interaction:PolicySchema.clone(interaction)})
                }
                onAddRequested: function(kind, value, admitHintKey) { workspace.addBuilderValue(kind, value, admitHintKey) }
            }
        }
    }
}
