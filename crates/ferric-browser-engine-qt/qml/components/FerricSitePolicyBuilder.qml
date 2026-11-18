import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/SitePolicyEditorSchema.js" as Schema
import "../scripts/SitePolicyBuilder.js" as Builder
import "../scripts/KeyboardPresentation.js" as Keyboard

FerricScrollColumn {
    id: builder
    required property var workspace
    hostWindow: workspace.hostWindow
    property string kind: ""
    property var seed: ({})
    property var scope: ({})
    property string scopeReport: ""
    property string keyReport: ""
    property string notice: ""
    property string sourceUrl: ""
    property int generation: 0
    property int formGeneration: 0
    property var errors: ({})
    property string captureField: ""
    property bool capturePressed: false
    property int capturedKey: 0
    property string canonicalFrom: ""
    property string canonicalTo: ""
    property var capturedFacts: null
    property var keptSamples: []
    readonly property string chosenPattern: pattern.text
    readonly property bool scopeChosen: scopeChoice.currentIndex >= 0 && chosenPattern.length > 0
    signal draftRequested(string source, string id, string pattern, var interaction)
    signal addRequested(string kind, var value, bool admitHintKey)
    onVisibleChanged: if (!visible) { captureField = ""; capturePressed = false }

    function reset() {
        workspace.previewCloseRequested()
        generation++; kind = ""; seed = ({}); scope = ({}); scopeReport = ""; keyReport = ""
        sourceUrl="";capturedFacts=null;keptSamples=[]
        notice = ""; captureField = ""; capturePressed = false; capturedKey = 0; canonicalFrom = ""; canonicalTo = ""
        scopeChoice.currentIndex = -1; pattern.text = ""; ruleId.text = ""; samples.text = ""
    }
    function acceptCapture(facts) {
        capturedFacts=facts
        notice="Inspect the bounded facts. Only fields you explicitly choose are copied to the local template."
    }
    function begin(nextKind) {
        reset(); kind = nextKind
        sourceUrl = workspace.contextUrl
        seed = Schema.defaultValue(nextKind === "policy" ? "interaction" : nextKind)
        errors = ({}); formGeneration++
        if (nextKind === "remap") { mappingId.text = seed.id; fromKey.text = seed.from; toKey.text = seed.to }
        request({action:"scope",url:sourceUrl,pattern:null,samples:[]}, {operation:"scope"})
    }
    function request(payload, context) {
        workspace.request(payload, Object.assign({action:"builder",builder:builder,generation:generation},context))
    }
    function receive(result, context) {
        if (context.generation !== generation) return
        if (context.operation === "scope") { scope = result; return }
        if (context.operation === "samples") {
            if (context.pattern === pattern.text && context.samples === samples.text) scopeReport = JSON.stringify(result, null, 2)
            return
        }
        if (context.operation === "key") {
            var input = context.field === "from" ? fromKey : toKey
            if (input.text !== context.token) return
            if (context.field === "from") canonicalFrom = result.canonical
            else canonicalTo = result.canonical
            keyReport = JSON.stringify(result, null, 2)
        }
    }
    function inspectKey(field) {
        var token = field === "from" ? fromKey.text : toKey.text
        request({action:"key",token:token},{operation:"key",field:field,token:token})
    }
    function interaction() {
        if (Object.keys(errors).length) throw new Error("Correct the invalid form fields first.")
        if (kind === "policy") return {enabled:false}
        var value = Schema.clone(seed)
        if (kind === "remap") {
            if (!canonicalFrom || !canonicalTo) throw new Error("Inspect both keys before adding the mapping.")
            value.from = canonicalFrom; value.to = canonicalTo
        }
        return value
    }
    function create() {
        if (kind === "labels") { notice = "Add label settings to a selected draft."; return }
        if (!scopeChosen) { notice = "Choose exact path, site-wide or a deliberate custom scope."; return }
        try {
            var value = interaction(), patch = {enabled:false}
            if (kind !== "policy") patch[kind === "anchor" ? "anchors" : kind === "collection" ? "collections" : "remaps"] = [value]
            if (kind === "remap" && admitHint.checked) patch.hint_page_keys = [value.from]
            draftRequested(destination.currentText, ruleId.text, chosenPattern, patch)
        } catch (error) { notice = String(error) }
    }
    function add() {
        try { addRequested(kind, interaction(), kind === "remap" && admitHint.checked) }
        catch (error) { notice = String(error) }
    }
    Label { Layout.fillWidth: true; text: "Build a site policy"; font.bold: true }
    Label {
        Layout.fillWidth: true; wrapMode: Text.Wrap; textFormat: Text.PlainText
        text: "Start explicitly. New policies are disabled until you enable them in the editor. Review and preview before applying; this builder never changes page behavior while editing."
    }
    Flow {
        Layout.fillWidth: true; Layout.preferredHeight: childrenRect.height; spacing: 6
        Button { text: "New site policy"; objectName: "policyBuildNew"; onClicked: builder.begin("policy") }
        Button { text: "New anchor"; objectName: "policyBuildAnchor"; onClicked: builder.begin("anchor") }
        Button { text: "New collection"; objectName: "policyBuildCollection"; onClicked: builder.begin("collection") }
        Button { text: "New key mapping"; objectName: "policyBuildMapping"; onClicked: builder.begin("remap") }
        Button { text: "Label space"; onClicked: builder.begin("labels") }
    }
    Label { Layout.fillWidth: true; text: builder.notice; visible: text.length > 0; wrapMode: Text.Wrap; textFormat: Text.PlainText; Accessible.name: text }
    ColumnLayout {
        Layout.fillWidth: true; visible: builder.kind.length > 0
        ColumnLayout {
            Layout.fillWidth:true;visible:builder.kind==="anchor"||builder.kind==="collection"
            Label { Layout.fillWidth:true;wrapMode:Text.Wrap;text:"Capture an admissible page target. For collections, select the exact primary first, then its logical item and container from the captured ancestor path. This reads one bounded sample and performs no page action." }
            ComboBox { id:captureFamily;model:Schema.families;Accessible.name:"Capture invocation family";objectName:"policyCaptureFamily" }
            Button { text:"Capture target on selected tab";objectName:"policyBuilderCapture";enabled:!!builder.workspace.selected&&!builder.workspace.busy(builder.workspace.selected);onClicked:builder.workspace.startCapture(captureFamily.currentText) }
        }
        FerricSitePolicyCaptureFacts {
            Layout.fillWidth:true
            focusScope:builder.workspace
            facts:builder.capturedFacts
            kind:builder.kind
            onUsed:function(value){
                if(builder.kind==="collection"){delete builder.seed.container_selector;delete builder.seed.container_self;delete builder.seed.primary_selector;delete builder.seed.primary_self}
                if(value.predicates)value.predicates=Object.assign({},builder.seed.predicates||{},value.predicates)
                if(value.primary_predicates)value.primary_predicates=Object.assign({},builder.seed.primary_predicates||{},value.primary_predicates)
                builder.seed=Object.assign({},builder.seed,value);builder.formGeneration++
                builder.notice="Chosen facts copied to the local template. Review repeated-item meaning, root paths and fragility; add, preview and apply separately."
            }
        }
        ColumnLayout {
            Layout.fillWidth:true;visible:builder.kind==="anchor"||builder.kind==="collection"
            Flow {
                Layout.fillWidth:true;Layout.preferredHeight:childrenRect.height;spacing:6
                Button {
                    text:"Keep this sample for comparison";objectName:"policyKeepSample"
                    enabled:!!builder.capturedFacts&&builder.keptSamples.length<8
                    onClicked:builder.keptSamples=builder.keptSamples.concat([Builder.sample(builder.capturedFacts)])
                }
                Button { text:"Clear kept samples";enabled:builder.keptSamples.length>0;onClicked:builder.keptSamples=[] }
            }
            Label { Layout.fillWidth:true;wrapMode:Text.Wrap;text:"Keep up to eight samples explicitly, navigate, then capture another target. Comparisons are advisory, remain in memory, and are omitted from drafts and exports. Only chosen predicates enter the template." }
            TextArea {
                Keys.priority:Keys.BeforeItem
                Keys.onPressed:function(event){builder.workspace.handleTabKey(event)};
                Layout.fillWidth:true;readOnly:true;selectByMouse:true;textFormat:TextEdit.PlainText;wrapMode:TextEdit.Wrap
                text:Builder.compareSamples(builder.keptSamples,builder.capturedFacts)
                Accessible.name:"Explicit sample comparison";objectName:"policySampleComparison"
            }
        }
        Label { text:"Choose a deliberate policy scope" }
        Label {
            Layout.fillWidth: true; wrapMode: Text.Wrap; textFormat: Text.PlainText
            text: "Selected origin: " + (builder.scope.origin || "Choose an HTTP(S) page first")
                + ". Query and fragment do not participate in site matching. Exact scheme, host and port are retained."
        }
        ComboBox {
            id: scopeChoice; Layout.fillWidth: true; model: ["Exact path", "Site-wide on this exact origin", "Custom pattern"]
            currentIndex: -1; Accessible.name: "Policy scope"; objectName: "policyBuilderScope"
            displayText: currentIndex<0?"Choose exact path, site-wide or custom":currentText
            onActivated: {
                pattern.text = currentIndex === 0 ? builder.scope.exact_pattern || "" : currentIndex === 1 ? builder.scope.site_pattern || "" : ""
                builder.scopeReport = ""
                if (currentIndex === 0 && !pattern.text) builder.notice = "This path contains wildcard syntax. Choose a deliberate custom pattern; it cannot be represented as exact."
            }
        }
        FerricTextField {
            id: pattern; Layout.fillWidth: true; readOnly: scopeChoice.currentIndex !== 2
            Accessible.name: "Reviewed site pattern"; objectName: "policyBuilderPattern"
            onTextEdited: builder.scopeReport = ""
        }
        Label { Layout.fillWidth: true; wrapMode: Text.Wrap; text: "Optional positive and negative URL samples, one per line (up to eight). The browser's existing matcher checks them; samples are not saved with the rule." }
        TextArea {
                Keys.priority:Keys.BeforeItem
                Keys.onPressed:function(event){builder.workspace.handleTabKey(event)}; id: samples; Layout.fillWidth: true; Layout.preferredHeight: 80; textFormat: TextEdit.PlainText; wrapMode: TextEdit.Wrap; Accessible.name: "Explicit URL match samples"; objectName: "policyBuilderSamples" }
        Button {
            text: "Check pattern and samples"; enabled: builder.scopeChosen; objectName: "policyBuilderCheckScope"
            onClicked: builder.request({action:"scope",url:builder.sourceUrl,pattern:pattern.text,
                samples:samples.text.split(/\r?\n/).filter(function(value) { return value.trim().length > 0 })}, {operation:"samples",pattern:pattern.text,samples:samples.text})
        }
        TextArea {
                Keys.priority:Keys.BeforeItem
                Keys.onPressed:function(event){builder.workspace.handleTabKey(event)}; Layout.fillWidth: true; visible: text.length > 0; readOnly: true; selectByMouse: true; persistentSelection: true; textFormat: TextEdit.PlainText; wrapMode: TextEdit.Wrap; text: builder.scopeReport; Accessible.name: "Scope match results" }
        Loader {
            id: form; Layout.fillWidth: true
            readonly property bool wantsForm: builder.kind !== "remap" && builder.kind !== "policy" && builder.kind.length > 0
            active: wantsForm
            property int revision: builder.formGeneration
            onRevisionChanged: { active = false; active = Qt.binding(function() { return form.wantsForm }) }
            sourceComponent: FerricPolicyValue {
                errors: builder.errors
                spec: ({type:builder.kind}); container: builder; fieldKey: "seed"; fieldPath: "builder"
                onEdited: builder.notice = ""
                onIssue: function(path, message, subtree) {
                    var next = Object.assign({}, builder.errors)
                    Object.keys(next).forEach(function(key) { if (key === path || (subtree && key.indexOf(path + ".") === 0)) delete next[key] })
                    if (message) next[path] = message
                    builder.errors = next
                }
            }
        }
        ColumnLayout {
            Layout.fillWidth: true; visible: builder.kind === "collection"
            Label { Layout.fillWidth: true; wrapMode: Text.Wrap; text: "Code-array helper: generate a fixed-width numeric or letter grid, review every code, then use it explicitly. This replaces this new collection's codes only. The shared preview reports actual reservations, holes, capacity and conflicts." }
            ComboBox { id: codeKind; model: ["numeric", "letters"]; Accessible.name: "Generated code style" }
            Flow {
                Layout.fillWidth: true; Layout.preferredHeight: childrenRect.height; spacing: 8
                Column { Label { text: "Start" } SpinBox { id: start; from: 1; to: 99999999; value: 1; editable: true; Accessible.name: "First generated rank" } }
                Column { Label { text: "Rows" } SpinBox { id: rows; from: 1; to: 64; value: 1; editable: true; Accessible.name: "Grid rows" } }
                Column { Label { text: "Columns" } SpinBox { id: columns; from: 1; to: 64; value: 3; editable: true; Accessible.name: "Grid columns" } }
            }
            FerricTextField { id: alphabet; Layout.fillWidth: true; text: "asdfghjkl"; Accessible.name: "Code helper alphabet" }
            Button {
                text: "Generate and review explicit codes"; objectName: "policyGenerateCodes"
                enabled: Object.keys(builder.errors).length === 0
                onClicked: {
                    try {
                        builder.seed.codes = Builder.codes(codeKind.currentText,start.value,rows.value,columns.value,alphabet.text)
                        builder.formGeneration++
                        builder.notice = "Explicit codes: " + JSON.stringify(builder.seed.codes) + ". Preview checks their actual code-space and source conflicts."
                    } catch (error) { builder.notice = String(error) }
                }
            }
        }
        ColumnLayout {
            Layout.fillWidth: true; visible: builder.kind === "remap"
            FerricTextField { id: mappingId; Layout.fillWidth: true; placeholderText: "Mapping ID"; Accessible.name: "Mapping ID"; objectName: "policyBuilderMappingId"; onTextEdited: builder.seed.id = text }
            FerricTextField {
                id: fromKey; Layout.fillWidth: true; Accessible.name: "Mapping source key"; objectName: "policyBuilderFrom"
                onTextEdited: builder.canonicalFrom = ""
            }
            Flow {
                Layout.fillWidth: true; Layout.preferredHeight: childrenRect.height; spacing: 6
                Button { text: "Capture source key"; objectName: "policyCaptureSourceKey"; onClicked: { builder.capturePressed = false; builder.captureField = "from"; keyCapture.forceActiveFocus() } }
                Button { text: "Inspect source"; objectName: "policyInspectSourceKey"; onClicked: builder.inspectKey("from") }
            }
            FerricTextField {
                id: toKey; Layout.fillWidth: true; Accessible.name: "Terminal destination key"; objectName: "policyBuilderTo"
                onTextEdited: builder.canonicalTo = ""
            }
            Flow {
                Layout.fillWidth: true; Layout.preferredHeight: childrenRect.height; spacing: 6
                Button { text: "Capture destination key"; objectName: "policyCaptureDestinationKey"; onClicked: { builder.capturePressed = false; builder.captureField = "to"; keyCapture.forceActiveFocus() } }
                Button { text: "Inspect destination"; objectName: "policyInspectDestinationKey"; onClicked: builder.inspectKey("to") }
            }
            FocusScope {
                id: keyCapture; Layout.fillWidth: true; implicitHeight: captureText.implicitHeight + 20
                visible: builder.captureField.length > 0; activeFocusOnTab: true
                Accessible.role: Accessible.EditableText; Accessible.name: "Capture one logical key; Escape cancels"
                Rectangle { anchors.fill: parent; color: "transparent"; border.width: 2; border.color: builder.workspace.browserWindow.accentColor }
                Label { id: captureText; anchors.fill: parent; anchors.margins: 10; wrapMode: Text.Wrap; text: "Press one key for " + builder.captureField + ". Escape cancels. No command or page action is executed." }
                Keys.priority: Keys.BeforeItem
                Keys.onShortcutOverride: function(event) { event.accepted = true }
                Keys.onPressed: function(event) {
                    event.accepted = true
                    if (event.key === Qt.Key_Escape) {
                        var cancelledInput = builder.captureField === "from" ? fromKey : toKey
                        builder.captureField = ""; builder.capturePressed = false; cancelledInput.forceActiveFocus(); return
                    }
                    if (event.isAutoRepeat || builder.capturePressed) return
                    var token = Keyboard.logicalNormalKeyText(event)
                    if (!token) return
                    var input = builder.captureField === "from" ? fromKey : toKey
                    input.text = token; builder.capturedKey = event.key
                    if (builder.captureField === "from") builder.canonicalFrom = ""; else builder.canonicalTo = ""
                    builder.inspectKey(builder.captureField); builder.capturePressed = true
                }
                Keys.onReleased: function(event) {
                    event.accepted = true
                    if (!builder.capturePressed || event.isAutoRepeat || event.key !== builder.capturedKey) return
                    var input = builder.captureField === "from" ? fromKey : toKey
                    builder.captureField = ""; builder.capturePressed = false; input.forceActiveFocus()
                }
            }
            Repeater {
                model: Schema.contexts
                CheckBox {
                    required property string modelData
                    text: modelData; checked: builder.kind === "remap" && !!builder.seed.contexts && builder.seed.contexts.indexOf(modelData) >= 0
                    onToggled: {
                        var values = builder.seed.contexts || []
                        builder.seed.contexts = checked ? values.concat([modelData]).filter(function(value,index,array) { return array.indexOf(value) === index }) : values.filter(function(value) { return value !== modelData })
                    }
                }
            }
            CheckBox { id: admitHint; text: "Also add this source to the selected Hint page keys"; objectName: "policyBuilderAdmitHint" }
            Label { Layout.fillWidth: true; wrapMode: Text.Wrap; text: "Normal-page-key requires Mixed mode and separate ordinary page-key admission. Choose receiver scope and exact frame origins in the full editor. Captured keys describe syntax; native testing is a separate physical page gesture." }
            TextArea {
                Keys.priority:Keys.BeforeItem
                Keys.onPressed:function(event){builder.workspace.handleTabKey(event)}; Layout.fillWidth: true; readOnly: true; selectByMouse: true; textFormat: TextEdit.PlainText; wrapMode: TextEdit.Wrap; text: builder.keyReport; Accessible.name: "Key syntax and native capability feedback" }
        }
        Label { Layout.fillWidth: true; wrapMode: Text.Wrap; text: "Create a new inactive rule, or add this section to the selected editable draft. All fields, root paths, predicates, receiver scopes and code arrays remain editable in the full draft editor." }
        ComboBox { id: destination; model: ["temporary", "generated"]; Accessible.name: "Builder draft destination";objectName:"policyBuilderDestination" }
        FerricTextField { id: ruleId; Layout.fillWidth: true; placeholderText: "New unique rule ID"; Accessible.name: "New builder rule ID"; objectName: "policyBuilderId" }
        Button { text: "Create inactive draft"; enabled: builder.scopeChosen && builder.kind !== "labels"; objectName: "policyBuilderCreate"; onClicked: builder.create() }
        Button {
            text: "Add to selected draft"; objectName: "policyBuilderAdd"
            enabled: builder.kind !== "policy" && !!builder.workspace.selected && !builder.workspace.selected.view.read_only && !builder.workspace.busy(builder.workspace.selected) && !builder.workspace.hasErrors(builder.workspace.selected)
            onClicked: builder.add()
        }
    }
}
