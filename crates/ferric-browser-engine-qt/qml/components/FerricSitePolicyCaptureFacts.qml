import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/SitePolicyEditorSchema.js" as Schema

ColumnLayout {
    id: chooser
    property var facts: null
    property var focusScope:null
    property string kind: "anchor"
    property string notice: ""
    property var attributes: []
    property var itemAttributes: []
    property var identityAttributes: []
    readonly property var ancestors: facts ? facts.ancestors : []
    readonly property var item: ancestors[itemChoice.currentIndex] || null
    readonly property var container: ancestors[containerChoice.currentIndex] || null
    readonly property var slotRoot:facts&&rootChoice.currentIndex>0?facts.slot_paths[rootChoice.currentIndex-1]:null
    readonly property var itemSelectors:slotRoot?((slotRoot.relative_ancestors||[])[itemChoice.currentIndex]||{}).selectors||[]:item?item.selectors:[]
    readonly property var containerSelectors:slotRoot?((slotRoot.relative_ancestors||[])[containerChoice.currentIndex]||{}).selectors||[]:container?container.selectors:[]
    readonly property var targetSelectors: facts ? (rootChoice.currentIndex>0?facts.slot_paths[rootChoice.currentIndex-1].selectors:facts.selectors) : []
    signal used(var value)
    visible: !!facts
    onFactsChanged: {
        attributes=[];itemAttributes=[];identityAttributes=[];rootChoice.currentIndex=0;targetChoice.currentIndex=-1
        itemChoice.currentIndex=-1;containerChoice.currentIndex=-1;itemSelector.currentIndex=-1;primarySelector.currentIndex=-1
        useName.checked=false;useRole.checked=false;useHref.checked=false;useGeometry.checked=false;notice=""
    }
    function labels(values) { return (values||[]).map(function(value){return value.selector+" · "+(value.complete?value.count+" matches":"at least "+value.count+"; incomplete")}) }
    function predicates() {
        var result={}
        if(useName.checked&&facts.hint_name)result.hint_name={equals:facts.hint_name}
        if(useRole.checked&&facts.role)result.role=facts.role
        if(useHref.checked&&facts.href)result.href=Schema.clone(facts.href)
        if(attributes.length)result.attributes=attributes.map(function(index){var attr=facts.attributes[index];return attr.oversized?{name:attr.name,present:true}:{name:attr.name,equals:attr.value}})
        if(useGeometry.checked&&facts.geometry){
            var g=facts.geometry;result.width_px={min:g.width,max:g.width};result.height_px={min:g.height,max:g.height}
            if(facts.viewport){var x=g.x/facts.viewport.width,y=g.y/facts.viewport.height
                if(isFinite(x)&&x>=0&&x<=1)result.viewport_x={min:x,max:x}
                if(isFinite(y)&&y>=0&&y<=1)result.viewport_y={min:y,max:y}}
        }
        return result
    }
    function useFacts() {
        if(!facts)return
        var value={}
        if(kind==="anchor"){
            if(targetChoice.currentIndex<0){notice="Choose a selector and inspect its match count.";return}
            value.root=Schema.clone(rootChoice.currentIndex>0?facts.slot_paths[rootChoice.currentIndex-1].root:facts.root)
            value.target_selector=targetSelectors[targetChoice.currentIndex].selector
            var targetPredicates=predicates();if(Object.keys(targetPredicates).length)value.predicates=targetPredicates
        } else if(kind==="collection"){
            if(!item||itemSelector.currentIndex<0){notice="Choose the representative logical item and its repeated selector.";return}
            var primary=item.primary
            if(!primary.available||(!primary.primary_self&&primarySelector.currentIndex<0)){notice="Choose an exact primary relative to each item.";return}
            if(!rootContainer.checked&&(!container||containerChoice.currentIndex<=itemChoice.currentIndex||containerSelector.currentIndex<0)){notice="Choose a container above the item, or explicitly use the root.";return}
            if(slotRoot&&(slotRoot.ancestor_level<0||itemChoice.currentIndex>=slotRoot.ancestor_level)){
                notice="Choose an item inside the assigned-slot root; capture a containing assigned node if necessary.";return
            }
            if(slotRoot&&!rootContainer.checked&&containerChoice.currentIndex>=slotRoot.ancestor_level){
                notice="Choose a container inside the assigned-slot root, or explicitly use that root as the container.";return
            }
            value.root=Schema.clone(slotRoot?slotRoot.root:rootContainer.checked?facts.root:container.root)
            if(rootContainer.checked)value.container_self=true
            else value.container_selector=containerSelectors[containerSelector.currentIndex].selector
            value.item_selector=itemSelectors[itemSelector.currentIndex].selector
            if(itemAttributes.length)value.item_attributes=itemAttributes.map(function(index){var attr=item.attributes[index];return attr.oversized?{name:attr.name,present:true}:{name:attr.name,equals:attr.value}})
            if(identityAttributes.length)value.identity_attributes=identityAttributes.slice()
            value.primary_root_steps=Schema.clone(primary.primary_root_steps)
            if(primary.primary_self)value.primary_self=true
            else value.primary_selector=primary.suggestions[primarySelector.currentIndex].selector
            var primaryPredicates=predicates();if(Object.keys(primaryPredicates).length)value.primary_predicates=primaryPredicates
        } else return
        used(value)
    }
    component PlainChoice: ComboBox {
        id: control
        contentItem: Label { text: control.displayText; textFormat: Text.PlainText; verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight }
        delegate: ItemDelegate {
            id: entry
            required property string modelData
            text: modelData
            width: control.width
            contentItem: Label { text: entry.text; textFormat: Text.PlainText; wrapMode: Text.Wrap }
        }
    }
    Label { Layout.fillWidth:true;wrapMode:Text.Wrap;textFormat:Text.PlainText;text:chooser.facts?"Captured "+chooser.facts.tag+" · "+chooser.facts.hint_name+". One sample does not prove stability. Multiple matches are never resolved by picking the first.":"" }
    Label { Layout.fillWidth:true;wrapMode:Text.Wrap;textFormat:Text.PlainText;text:chooser.notice;visible:text.length>0;Accessible.name:text }
    Label { Layout.fillWidth:true;wrapMode:Text.Wrap;visible:!!chooser.facts&&chooser.facts.ancestors_incomplete===true;text:"Ancestor evidence stopped at the capture depth limit. Choose an available root or capture a nearer representative; omitted ancestors have not been inspected." }
    PlainChoice { id:rootChoice;Layout.fillWidth:true;model:chooser.facts?["DOM root (top/shadow/frame)"].concat(chooser.facts.slot_paths.map(function(value,index){return "Assigned slot path "+(index+1)+" (preview assignment)"})):[];Accessible.name:"Captured root path";objectName:"policyCapturedRoot";onActivated:{itemSelector.currentIndex=-1;containerSelector.currentIndex=-1;targetChoice.currentIndex=-1} }
    TextArea {
                Keys.priority:Keys.BeforeItem
                Keys.onPressed:function(event){if(chooser.focusScope)chooser.focusScope.handleTabKey(event)}; Layout.fillWidth:true;readOnly:true;selectByMouse:true;textFormat:TextEdit.PlainText;wrapMode:TextEdit.Wrap;text:chooser.facts?JSON.stringify(rootChoice.currentIndex>0?chooser.facts.slot_paths[rootChoice.currentIndex-1].root:chooser.facts.root,null,2):"";Accessible.name:"Explicit captured root descriptor" }
    Label { Layout.fillWidth:true;wrapMode:Text.Wrap;text:chooser.facts&&chooser.facts.root_complete?"Host selectors were unique in this bounded sample. Preview the complete path again.":"Root uniqueness is incomplete or ambiguous. Refine the root fields and preview; this is not a live guarantee." }
    PlainChoice { id:targetChoice;Layout.fillWidth:true;visible:chooser.kind==="anchor";model:chooser.labels(chooser.targetSelectors);currentIndex:-1;Accessible.name:"Advisory target selector and match count";objectName:"policyCapturedSelector" }
    ColumnLayout {
        Layout.fillWidth:true;visible:chooser.kind==="collection"
        Label { Layout.fillWidth:true;wrapMode:Text.Wrap;text:"Choose a repeated item, not an ID identifying only this instance. Counts below refer to its captured root; preview recomputes ranks inside the chosen container." }
        PlainChoice { id:itemChoice;Layout.fillWidth:true;model:chooser.ancestors.map(function(value){return value.level+" · "+value.tag});currentIndex:-1;Accessible.name:"Representative logical item ancestor";objectName:"policyCapturedItem";onActivated:{itemSelector.currentIndex=-1;primarySelector.currentIndex=-1;chooser.itemAttributes=[];chooser.identityAttributes=[]} }
        PlainChoice { id:itemSelector;Layout.fillWidth:true;model:chooser.labels(chooser.itemSelectors);currentIndex:-1;Accessible.name:"Repeated item selector";objectName:"policyCapturedItemSelector" }
        CheckBox { id:rootContainer;text:"Use the captured document or shadow root as the container";objectName:"policyCapturedRootContainer" }
        PlainChoice { id:containerChoice;Layout.fillWidth:true;enabled:!rootContainer.checked;model:chooser.ancestors.map(function(value){return value.level+" · "+value.tag});currentIndex:-1;Accessible.name:"Collection container ancestor";objectName:"policyCapturedContainer" }
        PlainChoice { id:containerSelector;Layout.fillWidth:true;enabled:!rootContainer.checked;model:chooser.labels(chooser.containerSelectors);currentIndex:-1;Accessible.name:"Container selector";objectName:"policyCapturedContainerSelector" }
        Label { Layout.fillWidth:true;wrapMode:Text.Wrap;text:chooser.item?JSON.stringify(chooser.item.primary,null,2):"Choose an item to inspect its relative primary path and root/self alternatives.";textFormat:Text.PlainText }
        PlainChoice { id:primarySelector;Layout.fillWidth:true;model:chooser.labels(chooser.item&&chooser.item.primary.available?chooser.item.primary.suggestions:[]);currentIndex:-1;Accessible.name:"Primary selector relative to each item";objectName:"policyCapturedPrimary" }
        Label { Layout.fillWidth:true;wrapMode:Text.Wrap;text:"Optional item eligibility tests and stable identity attributes (up to four each). Requiring a per-item ID value may intentionally keep only that one item. Identity uses attribute names across items; preview reports duplicates or unavailable ranks." }
        Repeater {
            model:chooser.item?chooser.item.attributes:[]
            ColumnLayout {
                required property int index;required property var modelData
                Layout.fillWidth:true
                CheckBox {
                    id:eligibleAttribute
                    Layout.fillWidth:true
                    enabled:checked||chooser.itemAttributes.length<4
                    text:"Require item "+modelData.name+(modelData.oversized?" presence":" = "+modelData.value)
                    contentItem:Label{text:eligibleAttribute.text;textFormat:Text.PlainText;wrapMode:Text.Wrap}
                    onToggled:chooser.itemAttributes=checked?chooser.itemAttributes.concat([index]):chooser.itemAttributes.filter(function(value){return value!==index})
                }
                CheckBox {
                    text:"Use item "+modelData.name+" for identity"
                    enabled:!modelData.oversized&&(checked||chooser.identityAttributes.length<4)
                    onToggled:chooser.identityAttributes=checked?chooser.identityAttributes.concat([modelData.name]):chooser.identityAttributes.filter(function(value){return value!==modelData.name})
                }
            }
        }
    }
    CheckBox { id:useName;text:"Use the captured normalized Hint name";enabled:!!chooser.facts&&!!chooser.facts.hint_name }
    CheckBox { id:useRole;text:"Use the captured role";enabled:!!chooser.facts&&!!chooser.facts.role }
    CheckBox { id:useHref;text:"Use the link origin and path (query and fragment omitted)";enabled:!!chooser.facts&&!!chooser.facts.href }
    CheckBox { id:useGeometry;text:"Use captured dimensions and eligible viewport coordinates (fragile across layout changes)" }
    Repeater {
        model:chooser.facts?chooser.facts.attributes:[]
        CheckBox {
            id: attributeChoice
            required property int index;required property var modelData
            enabled:checked||chooser.attributes.length<4
            contentItem: Label { text: attributeChoice.text; textFormat: Text.PlainText; wrapMode: Text.Wrap }
            text:"Use "+modelData.name+(modelData.oversized?" presence (value exceeds capture bound)":" = "+modelData.value)
            onToggled:chooser.attributes=checked?chooser.attributes.concat([index]):chooser.attributes.filter(function(value){return value!==index})
        }
    }
    Button { text:"Use chosen facts in local template";objectName:"policyUseCapturedFacts";onClicked:chooser.useFacts() }
}
