import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as Scripts
import "../../qml/scripts/PageInteractionScripts.js" as Policy

TestCase {
    id: tests
    name: "PolicyNameEvidence"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var reply
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) { loaded=request.status===WebEngineView.LoadSucceededStatus }
    }
    function evaluate(script,world) {
        done=false
        page.runJavaScript(script,world,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function init() {
        page.loaded=false
        page.url="http://127.0.0.1:8766/roots.html"
        tryVerify(function(){return page.loaded},10000)
    }
    function cleanup() { evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld) }
    function test_complete_and_truncated_names_data() {
        return [
            {tag:"ordinary",setup:"target.setAttribute('aria-label','  Short\\n title  ')",value:"Short title",complete:true},
            {tag:"decomposed",setup:"target.setAttribute('aria-label','e\\u0301')",value:"e\u0301",complete:true},
            {tag:"exact-ascii",setup:"target.setAttribute('aria-label','a'.repeat(512))",value:"a".repeat(512),complete:true},
            {tag:"over-ascii",setup:"target.setAttribute('aria-label','a'.repeat(513))",value:"a".repeat(512),complete:false},
            {tag:"exact-four-byte",setup:"target.setAttribute('aria-label',String.fromCodePoint(0x1f680).repeat(512))",value:String.fromCodePoint(0x1f680).repeat(512),complete:true},
            {tag:"over-four-byte",setup:"target.setAttribute('aria-label',String.fromCodePoint(0x1f680).repeat(513))",value:String.fromCodePoint(0x1f680).repeat(512),complete:false},
            {tag:"capped-attribute-fallback",setup:"target.setAttribute('aria-label',' '.repeat(2048)+'Hidden tail');target.title='Fallback'",value:"Fallback",complete:false},
            {tag:"title-over-codepoints",setup:"target.title='a'.repeat(513)",value:"a".repeat(512),complete:false},
            {tag:"exact-labelledby-id",setup:"const label=document.createElement('span');label.id='n'.repeat(1024);label.textContent='Chosen';document.body.append(label);target.setAttribute('aria-labelledby',label.id)",value:"Chosen",complete:true},
            {tag:"capped-labelledby-id",setup:"const label=document.createElement('span');label.id='n'.repeat(1025);label.textContent='Unseen';document.body.append(label);target.setAttribute('aria-labelledby',label.id);target.title='Fallback'",value:"Fallback",complete:false},
            {tag:"labelledby-cap",setup:"const ids=[];for(let i=0;i<17;i++){const label=document.createElement('span');label.id='name-'+i;label.textContent='part';document.body.append(label);ids.push(label.id)}target.setAttribute('aria-labelledby',ids.join(' '))",value:Array(16).fill("part").join(" "),complete:false},
            {tag:"exact-labelledby-count",setup:"const ids=[];for(let i=0;i<16;i++){const label=document.createElement('span');label.id='name-'+i;label.textContent='part';document.body.append(label);ids.push(label.id)}target.setAttribute('aria-labelledby',ids.join(' '))",value:Array(16).fill("part").join(" "),complete:true},
            {tag:"bounded-visible-walk",setup:"for(let i=0;i<300;i++){const child=document.createElement('span');child.textContent='x';target.append(child)}",value:Array(127).fill("x").join(" "),complete:false},
            {tag:"dropped-empty-children",setup:"for(let i=0;i<300;i++)target.append(document.createElement('span'));target.title='Fallback'",value:"Fallback",complete:false},
            {tag:"exact-node-capacity",setup:"for(let i=0;i<254;i++)target.append(document.createElement('span'));target.append(document.createTextNode('Exact'))",value:"Exact",complete:true},
            {tag:"over-node-capacity",setup:"for(let i=0;i<255;i++)target.append(document.createElement('span'));target.append(document.createTextNode('Exact'))",value:"Exact",complete:false},
            {tag:"shared-inspection-exhaustion",setup:"target.textContent='Need a text node';target.title='Fallback'",limit:1,value:"Fallback",complete:false},
            {tag:"input-value-excluded",setup:"target.remove();target=document.createElement('input');target.id='name-target';target.value='private-value';document.body.append(target)",value:"",complete:true,ordinary:false},
            {tag:"button-value-capped",setup:"target.remove();target=document.createElement('input');target.id='name-target';target.type='button';target.value='a'.repeat(513);document.body.append(target)",value:"a".repeat(512),complete:false,ordinary:false},
            {tag:"exact-associated-label-count",setup:"target.remove();target=document.createElement('input');target.id='name-target';target.type='checkbox';document.body.append(target);for(let i=0;i<16;i++){const label=document.createElement('label');label.htmlFor=target.id;label.textContent='part';document.body.append(label)}",value:Array(16).fill("part").join(" "),complete:true,ordinary:false},
            {tag:"capped-associated-label-count",setup:"target.remove();target=document.createElement('input');target.id='name-target';target.type='checkbox';document.body.append(target);for(let i=0;i<17;i++){const label=document.createElement('label');label.htmlFor=target.id;label.textContent='part';document.body.append(label)}",value:Array(16).fill("part").join(" "),complete:false,ordinary:false},
            {tag:"unchosen-large-source",setup:"const label=document.createElement('span');label.id='chosen-name';label.textContent='Chosen';document.body.append(label);target.setAttribute('aria-labelledby',label.id);target.setAttribute('aria-label','a'.repeat(9000))",value:"Chosen",complete:true},
            {tag:"unchosen-large-visible-text",setup:"target.setAttribute('aria-label','Chosen');target.textContent='a'.repeat(9000)",value:"Chosen",complete:true}
        ]
    }
    function test_complete_and_truncated_names(data) {
        evaluate("(()=>{let target=document.createElement('a');target.id='name-target';target.href='#name';target.style='position:fixed;left:10px;top:10px;z-index:2147483000;display:block;width:300px;height:40px';document.body.append(target);"+data.setup+";return true})()",WebEngineScript.MainWorld)
        const limit=data.limit===undefined?50000:data.limit
        const script="(()=>{"+Policy.scrollTargetRuntime()+Policy.hintNameRuntime()+";let remaining="+limit+";function inspect(){if(remaining===0)return false;remaining--;return true}const target=document.getElementById('name-target');const display=ftHintName(target,true);remaining="+limit+";const facts=typeof ftHintNameEvidence==='function'?ftHintNameEvidence(target,true):null,used="+limit+"-remaining;remaining="+limit+";const evidence={},description=ftHintDescription(target,'all','a[href]',null,evidence);return {supported:typeof ftHintNameEvidence==='function',display:display,facts:facts,description:description,descriptionName:evidence.name,descriptionUsed:"+limit+"-remaining,nameUsed:used};})()"
        const value=evaluate(script,WebEngineScript.ApplicationWorld)
        verify(value.supported,"The shared name extractor must report completeness")
        compare(value.facts.value,value.display,"Evidence and ordinary display use the same extraction algorithm")
        if(data.value!==undefined)compare(value.display,data.value)
        compare(value.facts.complete,data.complete,JSON.stringify(value))
        compare(JSON.stringify(value.descriptionName),JSON.stringify(value.facts),"Description captures the same name quality during its original extraction")
        compare(value.descriptionUsed,value.nameUsed,"Capturing description evidence must not walk the name a second time")
        if(data.limit===undefined && data.ordinary!==false) {
            const ordinary=evaluate(Scripts.hintCollector("links"),WebEngineScript.ApplicationWorld)
            verify(!ordinary.error,JSON.stringify(ordinary))
            const rows=Array.from(ordinary.candidates).filter(function(row){return row.href&&row.href.endsWith('#name')})
            compare(rows.length,1)
            compare(rows[0].text,value.display,"The actual ordinary collector retains its display text")
        }
    }
}
