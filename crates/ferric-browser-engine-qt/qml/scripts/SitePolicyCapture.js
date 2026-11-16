.pragma library

// Selected-node advisory facts only. This code reads no value/HTML/history and
// has no focus, event, navigation or configuration-writing operation.
function factsSource() {
    return String.raw`function(element,fresh){
        'use strict';
        let visits=0,tests=0;
        const allowed=['id','data-testid','data-id','role','aria-label','name','type'];
        function bounded(value,limit){return typeof value==='string'&&value.length<=limit&&!/[\x00-\x1f\x7f]/.test(value)?value:null;}
        function attributes(node){
            return allowed.filter(name=>node.hasAttribute(name)).map(name=>{
                const value=bounded(node.getAttribute(name),256);
                return value===null?{name:name,present:true,oversized:true}:{name:name,value:value,present:true};
            });
        }
        function selectors(node){
            const result=[],tag=node.localName;
            for(const name of ['id','data-testid','data-id','role','name']){
                const value=bounded(node.getAttribute(name),128);
                if(!value)continue;
                const selector=name==='id'?'#'+CSS.escape(value):tag+'['+name+'="'+value.replace(/\\/g,'\\\\').replace(/"/g,'\\"')+'"]';
                if(selector.length<=512)result.push(selector);
            }
            for(let index=0;index<Math.min(3,node.classList.length);index++){const value=bounded(node.classList.item(index),64);if(value)result.push(tag+'.'+CSS.escape(value));}
            if(/^[a-z][a-z0-9-]*$/.test(tag))result.push(tag);
            return [...new Set(result)].slice(0,6);
        }
        function suggestions(node,scope,includeSelf){
            const values=selectors(node).map(selector=>({selector:selector,count:0,complete:true,
                advisory:'One sample does not prove selector stability. Prefer meaningful IDs or attributes; review repeated items separately.'}));
            if(!scope)return values.map(value=>Object.assign(value,{complete:false}));
            const walker=node.ownerDocument.createTreeWalker(scope,1);let current=includeSelf&&scope.nodeType===1?scope:walker.nextNode(),complete=true;
            while(current){
                if(++visits>50000){complete=false;break;}
                for(const value of values){
                    if(++tests>100000){complete=false;break;}
                    try{if(current.matches(value.selector))value.count++;}catch(error){complete=false;}
                }
                if(!complete)break;
                current=walker.nextNode();
            }
            return values.map(value=>Object.assign(value,{complete:complete}));
        }
        function best(node,scope,includeSelf){
            const values=suggestions(node,scope,includeSelf),unique=values.find(value=>value.complete&&value.count===1);
            return {selector:unique?unique.selector:values.length?values[0].selector:null,unique:!!unique,options:values};
        }
        function boundary(node){
            const root=node.getRootNode();
            if(root&&root.host)return {host:root.host,node:root,kind:'open-shadow'};
            if(root&&root.nodeType===9){
                try{const host=root.defaultView.frameElement;if(host)return {host:host,node:root,kind:'same-origin-frame'};}catch(error){}
            }
            return null;
        }
        function rootPath(node){
            const hops=[];let current=node,base=current.getRootNode(),complete=true;
            for(let depth=0;depth<9;depth++){
                const hop=boundary(current);if(!hop)break;
                if(depth===8){complete=false;break;}
                hops.unshift(hop);current=hop.host;base=current.getRootNode();
            }
            let focused=false;
            try{focused=base.nodeType===9&&base.defaultView!==base.defaultView.top;}catch(error){focused=true;}
            const root={start:focused?'focused-frame':'top-document',steps:[]};
            if(focused)root.receiver_origin=base.defaultView.location.origin;
            let scope=base;
            for(const hop of hops){
                const choice=best(hop.host,scope,true);
                if(!choice.selector){complete=false;break;}
                root.steps.push({kind:hop.kind,host_selector:choice.selector});
                complete=complete&&choice.unique;scope=hop.node;
            }
            return {root:root,complete:complete,scope:node.getRootNode()};
        }
        function parent(node){
            if(node.assignedSlot)return node.assignedSlot;
            if(node.parentElement)return node.parentElement;
            const hop=boundary(node);return hop?hop.host:null;
        }
        function relative(item,target){
            const hops=[];let current=target;
            for(let depth=0;!(item===current||item.contains(current));depth++){
                if(depth>=8)return {available:false,reason:'root depth exceeds eight'};
                const hop=boundary(current);if(!hop)return {available:false,reason:'primary is not contained by this item'};
                hops.unshift(hop);current=hop.host;
            }
            let scope=item,complete=true;const steps=[];
            for(const hop of hops){
                if(hop.host===scope)steps.push({kind:hop.kind,self:true});
                else {const choice=best(hop.host,scope,false);if(!choice.selector)return {available:false,reason:'no host selector'};
                    steps.push({kind:hop.kind,host_selector:choice.selector});complete=complete&&choice.unique;}
                scope=hop.node;
            }
            return {available:true,complete:complete,primary_root_steps:steps,primary_self:scope===target,
                suggestions:scope===target?[]:suggestions(target,scope,false)};
        }
        const root=rootPath(element),targetSuggestions=suggestions(element,root.scope,true),ancestors=[],ancestorNodes=[];
        let current=element;
        for(let depth=0;current&&depth<9;depth++,current=parent(current)){
            const path=rootPath(current);
            ancestorNodes.push(current);
            ancestors.push({level:depth,tag:current.localName,attributes:attributes(current),root:path.root,root_complete:path.complete,
                selectors:suggestions(current,path.scope,true),primary:relative(current,element)});
        }
        const slotPaths=[];
        for(let node=element,depth=0;node&&depth<8;node=node.parentElement,depth++){
            if(!node.assignedSlot)continue;
            const slot=node.assignedSlot,path=rootPath(slot),slotChoice=best(slot,slot.getRootNode(),true),assigned=selectors(node)[0];
            if(slotChoice.selector&&assigned){
                path.root.steps.push({kind:'assigned-slot',host_selector:slotChoice.selector,assigned_selector:assigned});
                slotPaths.push({root:path.root,ancestor_level:ancestorNodes.indexOf(node),
                    relative_ancestors:ancestorNodes.map(ancestor=>node.contains(ancestor)&&ancestor.getRootNode()===node.getRootNode()?{selectors:suggestions(ancestor,node,false)}:null),
                    complete:false,note:'Review the flattened assignment and preview it; assignment uniqueness has not been established.',
                    selectors:suggestions(element,node,true)});
            }
        }
        const editable=element.matches('input,textarea,select,[contenteditable]')||['input','textarea','select','contenteditable','editor-focus','label-control'].includes(fresh.kind),role=bounded(element.getAttribute('role'),64);
        let href=null;
        try{if(fresh.href){const url=new URL(fresh.href);if(url.protocol==='http:'||url.protocol==='https:')href={origin:url.origin,path:url.pathname};}}catch(error){}
        return {version:1,activation_permitted:false,tag:element.localName,hint_name:editable?'':Array.from(fresh.text||'').slice(0,160).join(''),
            role:role&&/^[a-z][a-z0-9-]*$/.test(role)?role:null,attributes:attributes(element),href:href,geometry:fresh.geometry,
            location:element.ownerDocument.location.origin+element.ownerDocument.location.pathname,
            root:root.root,root_complete:root.complete,selectors:targetSuggestions,ancestors:ancestors,ancestors_incomplete:!!current,slot_paths:slotPaths,
            bounded:visits<=50000&&tests<=100000,note:'Advisory facts from one explicit sample. Nothing is saved until you choose fields and apply the reviewed draft.'};
    }`;
}
