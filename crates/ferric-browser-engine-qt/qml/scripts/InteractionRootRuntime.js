.pragma library

// Runs only inside the existing ApplicationWorld collector. These are rooted
// discovery facts, not candidate eligibility, uniqueness witnesses or permits.
function source() {
    return `function ftInteractionRootPass(context){
    const documents=[], roots=[];
    let entries=[], visitedDocuments=new Set(), weakDocuments=false;
    let locations=new WeakMap(), documentRecords=new Map(), predicateSteps=0, bindings=null, ruleBaseDepth=context.baseDepth, enteredDocument=null;
    let rootedNodes=new WeakMap();
    const checkedSelectors=new WeakSet();
    const limit=context.predicateLimit, state=context.state;
    function stop(reason){throw {ferricRootReason:reason};}
    function charge(){if(predicateSteps>=limit)stop('predicate-limit');predicateSteps++;}
    function complete(location,node){return location&&location.entry.ends.has(node);}
    function begin(domain){
        const owner=domain.nodeType===9?domain:domain.ownerDocument;
        if(owner)visitedDocuments.add(weakDocuments?new WeakRef(owner):owner);
        const entry={domain:domain,nodes:[domain],stack:[domain],ends:new WeakMap(),inspected:context.inspect()};
        if(context.suspendable){entry.parentIndices=[null];entry.indices=new WeakMap([[domain,0]]);}
        entries.push(entry);
        const old=locations.get(domain);
        if(!complete(old,domain))locations.set(domain,{entry:entry,index:0});
        return entry;
    }
    function record(entry,node){
        if(node===entry.domain)return;
        const parent=node.parentNode;
        while(entry.stack.length&&entry.stack[entry.stack.length-1]!==parent){
            entry.ends.set(entry.stack.pop(),entry.nodes.length);
        }
        // Keep physical positions for inspection limits and search charges,
        // but only elements can be selector roots/targets. Text and comments
        // remain observed by document tracking and name evidence.
        if(node.nodeType!==1){
            entry.nodes.push(null);
            if(entry.parentIndices)entry.parentIndices.push(null);
            return;
        }
        const index=entry.nodes.length;
        if(entry.indices){entry.parentIndices.push(entry.indices.get(parent));entry.indices.set(node,index);}
        entry.nodes.push(node);entry.stack.push(node);
        const old=locations.get(node);
        if(!complete(old,node))locations.set(node,{entry:entry,index:index});
    }
    function finish(entry,exhaustive){
        if(exhaustive&&entry.inspected)while(entry.stack.length)entry.ends.set(entry.stack.pop(),entry.nodes.length);
    }
    function inventory(root){
        let location=locations.get(root);
        if(location)return location;
        const doc=root.nodeType===9?root:root.ownerDocument;
        if(!doc||!doc.createTreeWalker)stop('unsupported-root');
        context.track(root);
        const entry=begin(root);
        if(!entry.inspected)return locations.get(root);
        const walker=doc.createTreeWalker(root,0xffffffff);
        let node,exhaustive=true;
        while((node=walker.nextNode())){
            if(!context.inspect()){exhaustive=false;break;}
            record(entry,node);
        }
        finish(entry,exhaustive);
        return locations.get(root);
    }
    function inventoryNode(entry,index){
        if(entry.nodes[index]===null)return null;
        if(!entry.weak)return entry.nodes[index];
        if(!entry.verified){
            const domain=WeakRef.prototype.deref.call(entry.domain),tree=WeakRef.prototype.deref.call(entry.tree);
            if(!domain||!tree||!domain.isConnected)stop('detached');
            // Validate each actual parent edge under the shared ledger.
            // Overlapping domains reuse only this synchronous pass's checks.
            const path=[];let current=domain;
            while(!rootedNodes.has(current)){
                charge();
                if(path.length>=50000)stop('predicate-limit');
                path.push(current);
                const parent=current.parentNode;
                if(!parent){rootedNodes.set(current,current);break;}
                current=parent;
            }
            const actual=rootedNodes.get(current);
            for(const node of path)rootedNodes.set(node,actual);
            if(actual!==tree)stop('detached');
            entry.verified=new Set([0]);
        }
        const path=[];let position=index;
        while(!entry.verified.has(position)){
            charge();
            const node=WeakRef.prototype.deref.call(entry.nodes[position]),parentIndex=entry.parentIndices[position];
            if(!node||!Number.isInteger(parentIndex)||parentIndex<0||parentIndex>=position
                ||node.parentNode!==WeakRef.prototype.deref.call(entry.nodes[parentIndex]))stop('detached');
            path.push(position);position=parentIndex;
        }
        for(const position of path)entry.verified.add(position);
        const node=WeakRef.prototype.deref.call(entry.nodes[index]);
        if(!node)stop('detached');
        return node;
    }
    function compound(node,step){
        if(!node||node.nodeType!==1)return false;
        // The browser's checked Rust selector supplies each SIMPLE compound.
        // Native matching preserves HTML/SVG/quirks case semantics. Ancestry
        // is handled separately below and never delegated to a composed CSS
        // selector. Renderer payloads cannot supply this captured request.
        if(!Array.isArray(step)||step.length!==3||typeof step[1]!=='string'||step[1].length<1||step[1].length>1024
                ||!Number.isSafeInteger(step[2])||step[2]<1||step[2]>1024)stop('invalid-predicate');
        for(let atom=0;atom<step[2];atom++)charge();
        try{return node.matches(step[1]);}catch(error){stop('invalid-predicate');}
    }
    function validateSelector(selector){
        const steps=selector&&selector.steps;
        if(!Array.isArray(steps)||!steps.length||steps.length>512)stop('invalid-predicate');
        // Checked request objects are immutable during this synchronous pass.
        // Validate shape once; every actual match and ancestry step still pays.
        if(!checkedSelectors.has(selector)){
            for(let i=0;i<steps.length;i++){
                charge();
                const step=steps[i];
                if(!Array.isArray(step)||step.length!==3||(i===0?step[0]!==null:step[0]!=='child'&&step[0]!=='descendant')
                    ||typeof step[1]!=='string'||step[1].length<1||step[1].length>1024
                    ||!Number.isSafeInteger(step[2])||step[2]<1||step[2]>1024)stop('invalid-predicate');
            }
            checkedSelectors.add(selector);
        }
        return steps;
    }
    function matches(node,selector,boundary){
        const steps=validateSelector(selector);
        function at(current,index){
            charge();
            if(!compound(current,steps[index]))return false;
            if(index===0)return true;
            if(current===boundary)return false;
            const relation=steps[index][0];
            for(let parent=current.parentElement;parent;parent=parent.parentElement){
                charge();
                if(at(parent,index-1))return true;
                if(parent===boundary||relation==='child')break;
            }
            return false;
        }
        return at(node,steps.length-1);
    }
    function find(root,selector,all){
        const location=inventory(root),entry=location.entry;
        const end=entry.ends.get(root),exhaustive=end!==undefined;
        const found=[];
        for(let i=location.index+1;i<(exhaustive?end:entry.nodes.length);i++){
            charge();
            const node=inventoryNode(entry,i);
            if(node&&node.nodeType===1&&matches(node,selector,root)){
                found.push(node);
                if(!all&&found.length===2)return {found:found,exhaustive:false};
            }
        }
        if(!exhaustive)stop('inspection-limit');
        return {found:found,exhaustive:true};
    }
    function kind(node){return node.nodeType===9?'document':node.nodeType===11?'open-shadow':'element';}
    function identity(node){
        let id=state.weak.get(node);
        if(id===undefined){
            if(!Number.isSafeInteger(state.next)||state.next<1||state.next>4294967295)stop('annotation-limit');
            id=state.next++;state.weak.set(node,id);
        }
        if(!Number.isSafeInteger(id)||id<1||id>4294967295)stop('annotation-limit');
        return id;
    }
    function freshEpoch(){
        const bytes=crypto.getRandomValues(new Uint8Array(16));bytes[6]=(bytes[6]&15)|64;bytes[8]=(bytes[8]&63)|128;
        const hex=Array.from(bytes,function(b){return b.toString(16).padStart(2,'0');}).join('');
        return hex.slice(0,8)+'-'+hex.slice(8,12)+'-'+hex.slice(12,16)+'-'+hex.slice(16,20)+'-'+hex.slice(20);
    }
    function rememberNode(node,documentId){
        if(!bindings)return;
        const id=identity(node),previous=bindings.nodes.get(id);
        if(previous){
            if(previous.document!==documentId||WeakRef.prototype.deref.call(previous.object)!==node)stop('changed-document');
            return;
        }
        if(bindings.nodes.size>=50000||typeof WeakRef!=='function')stop('annotation-limit');
        const root=node.getRootNode(),slot=node.assignedSlot||null;
        bindings.nodes.set(id,{object:new WeakRef(node),document:documentId,root:new WeakRef(root),assigned:slot?new WeakRef(slot):null});
    }
    function documentRecord(doc){
        if(documentRecords.has(doc))return documentRecords.get(doc);
        if(!doc||doc.nodeType!==9||!doc.defaultView||doc.defaultView.document!==doc)stop('detached');
        const path=context.framePath(doc);
        if(typeof path!=='string'||path.length>256)stop('detached');
        if(!state.proofEpochs)state.proofEpochs=new WeakMap();
        let epoch=state.proofEpochs.get(doc);
        if(!epoch){epoch=freshEpoch();state.proofEpochs.set(doc,epoch);}
        const id=identity(doc);
        const value={id:id,frame_owner:context.owner,frame_path:path,epoch:epoch,origin:String(doc.defaultView.origin),document_node:id,parent_host:null};
        if(doc!==document){
            const frame=doc.defaultView.frameElement;
            if(!frame||frame.contentDocument!==doc)stop('detached');
            value.parent_host=nodeRef(frame);
        }
        if(context.reserve)context.reserve(value);
        if(documents.length&&context.reserveBytes)context.reserveBytes(1);
        if(bindings){
            if(bindings.documents.size>=50000||typeof WeakRef!=='function')stop('annotation-limit');
            // The native continuation resolves exact objects, never a URL,
            // selector, frame ordinal or a second discovery walk. All retained
            // DOM references here are weak and expire with this collection.
            const host=doc===document?null:doc.defaultView.frameElement;
            bindings.documents.set(id,{document:new WeakRef(doc),host:host?new WeakRef(host):null,
                parent:host?new WeakRef(host.ownerDocument):null,path:path,epoch:epoch,origin:value.origin});
            rememberNode(doc,id);
        }
        documentRecords.set(doc,value);documents.push(value);
        return value;
    }
    function nodeRef(node){
        const doc=node.nodeType===9?node:node.ownerDocument;
        const documentId=documentRecord(doc).id;
        rememberNode(node,documentId);
        return {document:documentId,node:identity(node)};
    }
    function rememberTarget(node,witness,fragments){
        if(!bindings)return;
        const reference=witness.candidate,record=state.elements.get(reference.element_id);
        if(!record||record.element!==node||record.ownerDocument!==node.ownerDocument
                ||record.framePath!==reference.frame_path||reference.frame_owner!==context.owner
                ||record.relatedElementId!==reference.related_element_id
                ||state.weak.get(node)!==reference.element_id)stop('changed-document');
        const target=nodeRef(node);
        if(record.relatedElement){
            const related=nodeRef(record.relatedElement);
            if(related.document!==target.document||related.node!==reference.related_element_id)stop('changed-document');
        }else if(reference.related_element_id!==null)stop('changed-document');
        const previous=bindings.targets.get(reference.index);
        if(!fragments||fragments.complete!==true||!Array.isArray(fragments.eligible_fragments)
            ||fragments.eligible_fragments.length<1||fragments.eligible_fragments.length>8)stop('missing-evidence');
        const snapshot={candidate:reference,document:target.document,fragments:fragments};
        const limit=context.annotationLimit===undefined?2097152:context.annotationLimit;
        if(previous){
            if(previous.document!==target.document||WeakRef.prototype.deref.call(previous.element)!==node
                ||previous.fragment_json!==ftInteractionAnnotationJson(snapshot,limit))stop('changed-document');
            return;
        }
        if(bindings.targets.size>=5000)stop('annotation-limit');
        // Debit the complete primitive entry before copying or serializing it.
        // Repeated rules reuse this candidate snapshot without another debit.
        if(context.reserve)context.reserve(snapshot);
        if(bindings.targets.size&&context.reserveBytes)context.reserveBytes(1);
        const bytes=ftInteractionAnnotationSize(snapshot,limit);
        function rect(value){return Object.freeze({x:value.x,y:value.y,width:value.width,height:value.height});}
        function size(value){return Object.freeze({width:value.width,height:value.height});}
        const copied=Object.freeze({candidate:Object.freeze(Object.assign({},reference)),document:target.document,
            fragments:Object.freeze(Object.assign({},fragments,{
                target_border_box_css:size(fragments.target_border_box_css),top_viewport_css:size(fragments.top_viewport_css),
                owner_document_clip_css:rect(fragments.owner_document_clip_css),scope_clip_css:rect(fragments.scope_clip_css),
                projected_primary_fragment_css:rect(fragments.projected_primary_fragment_css),
                eligible_fragments:Object.freeze(fragments.eligible_fragments.map(function(value){
                    return Object.freeze({index:value.index,rect:rect(value.rect)});
                }))}))});
        bindings.targets.set(reference.index,{document:target.document,reference:copied.candidate,
            fragment:copied,fragment_json:JSON.stringify(copied),fragment_bytes:bytes,
            element:new WeakRef(node),related:record.relatedElement?new WeakRef(record.relatedElement):null});
    }
    function capture(path){
        charge();
        if(roots.length>=50000)stop('annotation-limit');
        const value={id:roots.length+1,start:nodeRef(path.start),start_kind:kind(path.start),hops:[],resolved:nodeRef(path.node),resolved_kind:kind(path.node)};
        for(const hop of path.hops){
            if(hop.kind==='open-shadow')value.hops.push({'open-shadow':{host:nodeRef(hop.host),root:nodeRef(hop.node)}});
            else if(hop.kind==='same-origin-frame')value.hops.push({'same-origin-frame':{host:nodeRef(hop.host),document:nodeRef(hop.node)}});
            else value.hops.push({'assigned-slot':{slot:nodeRef(hop.host),assigned:nodeRef(hop.node),owner_document:documentRecord(hop.node.ownerDocument).id}});
        }
        if(context.reserve)context.reserve(value);
        if(roots.length&&context.reserveBytes)context.reserveBytes(1);
        roots.push(value);return value.id;
    }
    function connected(node){
        if(node.nodeType===9)return !!node.defaultView&&node.defaultView.document===node;
        return node.isConnected;
    }
    function contains(root,node){
        for(let current=node;current;){
            charge();
            if(current===root||current.getRootNode&&current.getRootNode()===root)return true;
            let parent=context.parent(current);
            if(!parent){const doc=current.nodeType===9?current:current.ownerDocument,view=doc&&doc.defaultView;parent=view&&view!==window?view.frameElement:null;}
            current=parent;
        }
        return false;
    }
    function intersects(node){
        const scope=context.scope();
        const target=node.nodeType===9?node.documentElement:node.nodeType===11?node.host:node;
        return (!scope||contains(node,scope)||contains(scope,target))
            &&(!enteredDocument||contains(node,enteredDocument)||contains(enteredDocument,target));
    }
    function within(node){
        const scope=context.scope();
        return (!scope||contains(scope,node))&&(!enteredDocument||contains(enteredDocument,node));
    }
    function traversalDepth(node){
        let current=node,depth=0;
        while(current){
            charge();
            const root=current.getRootNode();
            if(root.nodeType===11){
                if(root.mode!=='open'||!root.host||root.host.shadowRoot!==root)stop('unsupported-root');
                current=root.host;
            }else if(root.nodeType===9){
                if(root===document)return depth;
                const view=root.defaultView,host=view&&view.frameElement;
                if(!host||!host.isConnected||host.contentDocument!==root||host.contentWindow!==view)stop('detached');
                current=host;
            }else stop('unsupported-root');
            if(++depth>context.depthLimit||depth+context.baseDepth>8)stop('unsupported-root');
        }
        stop('detached');
    }
    function enter(native){
        if(context.start!=='top-document'||context.baseDepth!==0||!native||typeof native!=='object'||Array.isArray(native)
            ||Object.keys(native).length!==3||!['epoch','origin','depth'].every(key=>Object.prototype.hasOwnProperty.call(native,key))
            ||typeof native.epoch!=='string'||!/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(native.epoch)
            ||typeof native.origin!=='string'||native.origin==='null'||native.origin!==String(window.origin)
            ||!Number.isSafeInteger(native.depth)||native.depth<1||native.depth>8)stop('unsupported-root');
        let doc=document;
        for(let depth=0;depth<native.depth;depth++){
            charge();if(!context.inspect())stop('inspection-limit');
            if(!doc.hasFocus())stop('detached');
            let active=doc.activeElement;
            for(let shadows=0;active&&active.shadowRoot;shadows++){
                charge();if(!context.inspect())stop('inspection-limit');
                if(shadows>=8||active.shadowRoot.mode!=='open')stop('unsupported-root');
                const inner=active.shadowRoot.activeElement;
                if(!inner||inner===active)stop('detached');
                active=inner;
            }
            if(!active||!['iframe','frame'].includes(active.localName)||!active.isConnected)stop('detached');
            const child=active.contentDocument,view=active.contentWindow;
            if(!child||!view||view.document!==child||child.defaultView!==view||view.frameElement!==active
                ||String(view.origin)!==native.origin)stop('detached');
            doc=child;
        }
        if(!doc.hasFocus()||doc.defaultView.__ferric_hintFrameDocument!==native.epoch)stop('detached');
        const depth=traversalDepth(doc),scope=context.scope();
        if(scope&&!contains(scope,doc)&&!contains(doc,scope))stop('outside-scope');
        enteredDocument=doc;
        return {document:doc,depth:depth};
    }
    function negative(path,stage,found){
        if(!intersects(path.node)||found.some(function(node){return !intersects(node);}))stop('outside-scope');
        const value={state:'search-complete',root:capture(path),stage:stage,cardinality:found.length===0?'absent':'ambiguous'};
        throw {ferricRootOutcome:value};
    }
    function focusedTopStart(spec){
        const native=context.focusedDocument;
        if(!native||typeof native!=='object'||Array.isArray(native)||Object.keys(native).length!==3
                ||!['epoch','origin','depth'].every(function(key){return Object.prototype.hasOwnProperty.call(native,key);})
                ||typeof native.epoch!=='string'||!/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(native.epoch)
                ||!Number.isSafeInteger(native.depth)||native.depth<1||native.depth>8
                ||typeof native.origin!=='string'||spec.receiver_origin!==native.origin)stop('unsupported-root');
        // Inspect only Document objects already encountered by this collector.
        // A native epoch selects an exact object in Top's existing namespace;
        // neither URLs nor another selector/DOM walk supply the association.
        let found=null;
        for(const reference of visitedDocuments){
            charge();
            const doc=weakDocuments?WeakRef.prototype.deref.call(reference):reference;
            if(!doc)stop('detached');
            let epoch,origin;
            try{
                if(doc===document||!doc.defaultView||doc.defaultView.document!==doc)continue;
                epoch=doc.defaultView.__ferric_hintFrameDocument;origin=String(doc.defaultView.origin);
            }catch(error){continue;}
            if(epoch!==native.epoch)continue;
            if(origin!==native.origin)stop('detached');
            if(found&&found!==doc)stop('unsupported-root');
            found=doc;
        }
        if(!found)stop(context.truncated&&context.truncated()?'truncated':'detached');
        if(!found.hasFocus())stop('detached');
        let active=found.activeElement;
        for(let depth=0;active&&active.shadowRoot&&depth<8;depth++){
            const inner=active.shadowRoot.activeElement;if(!inner||inner===active)break;active=inner;
        }
        if(!active||active.shadowRoot&&active.shadowRoot.activeElement)stop('unsupported-root');
        if(['iframe','frame','object','embed'].includes(active.localName))stop('detached');
        let current=found,depth=0;
        while(current!==document){
            charge();
            if(depth>=8)stop('unsupported-root');
            const host=current.defaultView&&current.defaultView.frameElement;
            if(!host||!host.isConnected||host.contentDocument!==current
                    ||host.contentWindow!==current.defaultView)stop('detached');
            current=host.ownerDocument;
            if(!current||!current.defaultView||current.defaultView.document!==current)stop('detached');
            depth++;
        }
        if(depth!==native.depth)stop('detached');
        ruleBaseDepth=traversalDepth(found);
        return found;
    }
    function resolve(descriptor){
        const spec=descriptor||{start:'top-document',steps:[]};
        ruleBaseDepth=context.baseDepth;
        let start=document;
        if(spec.start==='focused-frame'&&context.start==='top-document')start=focusedTopStart(spec);
        else if(spec.start!==context.start||spec.start==='focused-frame'
                &&(spec.receiver_origin!==context.receiverOrigin||spec.receiver_origin!==String(window.origin)))stop('unsupported-root');
        return resolveSteps(start,spec.steps,0,false);
    }
    function resolveSteps(start,steps,spent,perItem){
        if(!Array.isArray(steps)||spent+steps.length>context.depthLimit||ruleBaseDepth+spent+steps.length>8)stop('unsupported-root');
        const path={start:start,node:start,hops:[]};
        function unresolved(stage,found){
            // SearchComplete describes an outer authored root prefix. An
            // item-start path must never pretend that its step belongs there.
            if(perItem)stop(found.length?'ambiguous-primary':'missing-evidence');
            negative(path,stage,found);
        }
        for(let index=0;index<steps.length;index++){
            charge();
            if(!connected(path.node))stop('detached');
            if(!intersects(path.node))stop('outside-scope');
            const step=steps[index];
            let host;
            if(step.self===true){
                if(step.host_selector||path.node.nodeType!==1)stop('unsupported-root');
                host=path.node;
            }else{
                if(!step.host_selector)stop('invalid-predicate');
                const lookup=find(path.node,step.host_selector);
                if(lookup.found.length!==1)unresolved({'root-host':{step:index}},lookup.found);
                host=lookup.found[0];
            }
            if(!intersects(host))stop('outside-scope');
            let next;
            if(step.kind==='open-shadow'){
                next=host.shadowRoot;
                if(!next||next.mode!=='open')stop('unsupported-root');
            }else if(step.kind==='same-origin-frame'){
                if(host.localName!=='iframe'&&host.localName!=='frame')stop('unsupported-root');
                try{next=host.contentDocument;}catch(error){stop('unsupported-root');}
                if(!next||!next.defaultView||host.contentWindow!==next.defaultView)stop('unsupported-root');
            }else if(step.kind==='assigned-slot'){
                if(host.localName!=='slot'||typeof host.assignedElements!=='function'||!step.assigned_selector)stop('unsupported-root');
                const found=[];
                for(const assigned of host.assignedElements({flatten:false})){
                    if(!context.inspect())stop('inspection-limit');
                    if(matches(assigned,step.assigned_selector,assigned)){
                        if(!found.includes(assigned))found.push(assigned);
                        if(found.length===2)break;
                    }
                }
                if(found.length!==1)unresolved({'assigned-element':{step:index,slot:nodeRef(host)}},found);
                next=found[0];
                if(next.ownerDocument!==host.ownerDocument||next.assignedSlot!==host)stop('detached');
            }else stop('unsupported-root');
            if(!connected(next))stop('detached');
            if(!intersects(next))stop('outside-scope');
            path.hops.push({kind:step.kind,host:host,node:next});path.node=next;
            context.track(next);
        }
        if(!intersects(path.node))stop('outside-scope');
        return path;
    }
    function itemRoots(query,path,container){
        // This is logical DOM discovery and explicit root identity only.
        // Primary eligibility, predicates and final witnesses are separate;
        // no slot_count limit, rank, code ownership or action is inferred.
        const items=find(container,query.item_selector,true).found,result=[];
        for(const item of items){
            charge();
            if(!connected(item))stop('detached');
            // Items form a set intersected with the local lease. A proved
            // disjoint sibling is excluded, without allocating/tracking its
            // primary root. Unique host/container searches above stay strict:
            // filtering those could turn an ambiguous lookup into a singleton.
            if(!intersects(item))continue;
            const primary=resolveSteps(item,query.primary_root_steps||[],path.hops.length,true);
            if(query.primary_self===true&&(primary.node.nodeType!==1||query.primary_selector))stop('unsupported-root');
            result.push({item:nodeRef(item),root:capture(primary)});
        }
        return result;
    }
    function reason(value){
        if(value==='outside-scope')return 'unsupported-root';
        return ['truncated','inspection-limit','predicate-limit','annotation-limit','unsupported-root','detached','missing-evidence','ambiguous-root','ambiguous-primary','nested-items','changed-document','invalid-predicate'].includes(value)?value:'missing-evidence';
    }
    function targetNodes(root,selector,includeSelf){
        if(selector)validateSelector(selector);
        const location=inventory(root),entry=location.entry,end=entry.ends.get(root);
        if(end===undefined)stop('inspection-limit');
        const found=[];
        for(let index=location.index+(includeSelf?0:1);index<end;index++){
            charge();
            const node=inventoryNode(entry,index);
            // Eligibility already conclusively excluded these from the final
            // ordinary rows. Unknown/capped work must still be matched. This
            // applies only to targets, never logical item/root enumeration.
            if(node&&node.nodeType===1&&(!context.knownExcluded||!context.knownExcluded(node))
                &&(!selector||matches(node,selector,root)))found.push(node);
        }
        return found;
    }
    function targetFacts(nodes,predicates){
        const names=(predicates&&predicates.attributes||[]).map(function(test){return test.name;}),targets=[];
        if(context.reserveBytes)context.reserveBytes(2);
        for(const node of nodes){
            charge();
            if(!within(node))continue;
            const observation=context.observe(node,names);
            if(observation.unavailable)stop(observation.unavailable);
            if(observation.witness){
                rememberTarget(node,observation.witness,observation.fragments);
                if(targets.length&&context.reserveBytes)context.reserveBytes(1);
                targets.push(observation.witness);
            }
            else if(!observation.excluded)stop('missing-evidence');
        }
        return targets;
    }
    function observedItems(query,path,container){
        const items=find(container,query.item_selector,true).found.filter(intersects),members=new Set(items),result=[];
        if(context.reserveBytes)context.reserveBytes(2);
        for(const item of items){
            charge();
            if(!connected(item))stop('detached');
            let parentItem=null;
            for(let parent=item.parentElement;parent&&parent!==container;parent=parent.parentElement){
                charge();
                if(members.has(parent)){parentItem=nodeRef(parent);break;}
            }
            const value={item:nodeRef(item),parent_item:parentItem,
                attributes:context.attributes(item,(query.item_attributes||[]).map(function(test){return test.name;})),
                identity_attributes:context.attributes(item,query.identity_attributes||[]),primary:null};
            if(context.reserve)context.reserve(Object.assign({},value,{attributes:null,identity_attributes:null}),12);
            const firstRoot=roots.length;
            try{
                const primary=resolveSteps(item,query.primary_root_steps||[],path.hops.length,true);
                let nodes;
                if(query.primary_self===true){
                    if(primary.node.nodeType!==1||query.primary_selector)stop('unsupported-root');
                    nodes=[primary.node];
                }else{
                    if(!query.primary_selector)stop('invalid-predicate');
                    nodes=targetNodes(primary.node,query.primary_selector,false);
                }
                const targets=targetFacts(nodes,query.primary_predicates);
                value.primary={'targets-complete':{root:capture(primary),targets:targets}};
                if(context.reserve)context.reserve({'targets-complete':{root:value.primary['targets-complete'].root,targets:null}},4);
            }catch(error){
                roots.length=firstRoot;
                value.primary={unavailable:{reason:reason(error&&error.ferricRootReason)}};
                if(context.reserve)context.reserve(value.primary);
            }
            if(result.length&&context.reserveBytes)context.reserveBytes(1);
            result.push(value);
        }
        return result;
    }
    function run(request,batchId){
        const results=[],outcomes=[],identities=new Set(),observed=!!(request&&request.observations===true&&request.stamp);
        let annotationExhausted=false;
        if(!request||request.version!==1||!Array.isArray(request.rules)||request.rules.length>80)return {error:'invalid-request'};
        if(observed){
            bindings={batch:batchId===undefined?freshEpoch():batchId,stamp:JSON.stringify(request.stamp),plan:JSON.stringify(request),revision:state.revision,
                documents:new Map(),nodes:new Map(),targets:new Map(),annotation:null,scope:null,entered:null};
            Object.defineProperty(bindings,'ordinary_input',{value:context.ordinaryInput,enumerable:true});
        }
        let scopeFailure=null;
        if(observed&&enteredDocument)bindings.entered=nodeRef(enteredDocument);
        if(observed&&request.stamp.scope_lease){
            try{
                const scope=context.scope(),lease=state.scopeLease;
                if(!scope||!lease||lease.leaseId!==request.stamp.scope_lease)stop('detached');
                bindings.scope={node:nodeRef(scope),lease_id:lease.leaseId,element_id:lease.elementId,
                    frame_path:lease.framePath,kind:lease.kind,selector_revision:lease.selectorRevision,
                    parent_scope_lease_id:lease.parentScopeLeaseId||null};
            }catch(error){scopeFailure=reason(error&&error.ferricRootReason);}
        }
        if(observed&&context.reserve){
            context.reserve({version:1,stamp:request.stamp,documents:null,roots:null,outcomes:null,discovery_work:null},16);
            context.reserveBytes(6+Math.max(0,request.rules.length-1));
            context.reserve({version:1,targets:[]});
        }
        for(const query of request.rules){
            const rule=query&&query.rule;
            if(!rule||!['anchor','collection'].includes(rule.kind)||typeof rule.id!=='string'||rule.id.length<1||rule.id.length>64)return {error:'invalid-request'};
            const key=rule.kind+':'+rule.id;if(identities.has(key))return {error:'invalid-request'};identities.add(key);
            let result,observation;const firstRoot=roots.length;
            try{
                if(observed&&context.reserve)context.reserve({rule:{kind:rule.kind,id:rule.id},result:null},4);
                if(scopeFailure)stop(scopeFailure);
                const path=resolve(query.root),selector=rule.kind==='anchor'?query.scope_selector:query.container_selector;
                let scope=path.node;
                if(selector){
                    const lookup=find(path.node,selector);
                    if(lookup.found.length!==1)negative(path,rule.kind==='anchor'?{'anchor-scope':{}}:{'collection-container':{}},lookup.found);
                    scope=lookup.found[0];
                }else if(rule.kind==='collection'&&query.container_self!==true)stop('invalid-predicate');
                if(!intersects(scope))stop('outside-scope');
                if(observed){
                    const value=rule.kind==='anchor'?{targets:targetFacts(targetNodes(scope,query.target_selector,true),query.predicates)}:{items:observedItems(query,path,scope)};
                    const root=capture(path);
                    observation=rule.kind==='anchor'?{'anchor-observed':Object.assign({root:root,scope:nodeRef(scope),scope_kind:kind(scope)},value)}
                        :{'collection-observed':Object.assign({root:root,container:nodeRef(scope),container_kind:kind(scope)},value)};
                    result={state:'observed',root:root};
                }else{
                    const items=rule.kind==='collection'&&query.item_selector?itemRoots(query,path,scope):null;
                    result={state:'resolved',root:capture(path),scope:nodeRef(scope),scope_kind:kind(scope)};
                    if(items)result.item_roots=items;
                }
            }catch(error){
                // A later item failure cannot leave earlier, unreferenced
                // captures in the transport. Completed outer negatives retain
                // their own prefix root; other failed rules retain none.
                if(!(error&&error.ferricRootOutcome))roots.length=firstRoot;
                result=error&&error.ferricRootOutcome||{state:'unavailable',reason:error&&error.ferricRootReason||'unsupported-root'};
                observation=result.state==='search-complete'?{'search-complete':{root:result.root,stage:result.stage,cardinality:result.cardinality}}:{unavailable:{reason:reason(result.reason)}};
            }
            if(observed&&!annotationExhausted&&context.reserve){
                try{
                    if(observation['anchor-observed'])context.reserve({'anchor-observed':Object.assign({},observation['anchor-observed'],{targets:null})},4);
                    else if(observation['collection-observed'])context.reserve({'collection-observed':Object.assign({},observation['collection-observed'],{items:null})},4);
                    else context.reserve(observation);
                }catch(error){annotationExhausted=true;observation={unavailable:{reason:'annotation-limit'}};}
            }
            results.push(Object.assign({rule:{kind:rule.kind,id:rule.id}},result));
            if(observed)outcomes.push({rule:{kind:rule.kind,id:rule.id},result:observation});
        }
        const used=new Set();
        function use(node){used.add(node.document);}
        for(const root of roots){use(root.start);use(root.resolved);for(const hop of root.hops){const value=Object.values(hop)[0];Object.values(value).forEach(function(node){if(node&&typeof node==='object'&&node.document)use(node);});}}
        let changed=true;
        while(changed){changed=false;for(const doc of documents)if(used.has(doc.id)&&doc.parent_host&&!used.has(doc.parent_host.document)){used.add(doc.parent_host.document);changed=true;}}
        const keptDocuments=documents.filter(function(doc){return used.has(doc.id);});
        const result={version:1,stamp:request.stamp,documents:keptDocuments,roots:roots.slice(),results:results,predicate_steps:predicateSteps};
        if(observed){
            const work={predicate_steps:predicateSteps,inspections:context.inspected()};
            const envelope={version:1,stamp:request.stamp,documents:keptDocuments,roots:roots.slice(),outcomes:outcomes,discovery_work:work};
            const limit=context.annotationLimit===undefined?2097152:context.annotationLimit;
            let catalogue;
            function fragments(discard){
                const referenced=new Set(),parts=[],header='{"version":1,"targets":[',tail=']}';
                if(!discard)for(const outcome of outcomes){
                    const anchor=outcome.result['anchor-observed'],collection=outcome.result['collection-observed'];
                    if(anchor)for(const witness of anchor.targets)referenced.add(witness.candidate.index);
                    if(collection)for(const item of collection.items){
                        const primary=item.primary['targets-complete'];
                        if(primary)for(const witness of primary.targets)referenced.add(witness.candidate.index);
                    }
                }
                let bytes=header.length+tail.length;
                for(const [index,target] of bindings.targets){
                    if(!referenced.has(index)){bindings.targets.delete(index);continue;}
                    bytes+=target.fragment_bytes+(parts.length?1:0);
                    if(bytes>limit)stop('annotation-limit');
                    parts.push(target.fragment_json);
                }
                if(bytes>limit)stop('annotation-limit');
                return {bytes:bytes,json:header+parts.join(',')+tail};
            }
            try{
                if(annotationExhausted)stop('annotation-limit');
                if(context.reserve)context.reserve(work);
                catalogue=fragments(false);
                result.annotation_json=ftInteractionAnnotationJson(envelope,limit-catalogue.bytes);
            }
            catch(error){
                envelope.documents=[];envelope.roots=[];
                envelope.outcomes=outcomes.map(function(outcome){return {rule:outcome.rule,result:{unavailable:{reason:reason(error&&error.ferricRootReason)}}};});
                // Discarded prefixes keep their reservation debit, but cannot
                // leave positive fragment facts outside the final raw graph.
                catalogue=fragments(true);
                result.annotation_json=ftInteractionAnnotationJson(envelope,limit-catalogue.bytes);
            }
            const debit=context.annotationWork?context.annotationWork():NaN;
            if(!Number.isSafeInteger(debit)||debit<0||debit>limit)stop('missing-evidence');
            bindings.fragment_catalogue=catalogue.json;
            bindings.annotation_work=debit;
            result.annotation_work=debit;
            // This fixed-size carrier is sealed separately by the native
            // continuation; it never changes the annotation's schema or cap.
            result.annotation_batch=bindings.batch;
            // Seal exactly the original bounded wire string, before Rust's
            // canonical filtering can discard roots or excluded raw items.
            // Strings are immutable; no DOM object is retained through it.
            bindings.annotation=result.annotation_json;
            state.interactionProof=bindings;
        }
        return result;
    }
    function suspend(){
        if(weakDocuments||bindings||documents.length||roots.length||typeof WeakRef!=='function')stop('missing-evidence');
        let count=0;
        for(const entry of entries){
            count+=entry.nodes.length;
            if(count>50000||entry.weak||!entry.parentIndices||entry.parentIndices.length!==entry.nodes.length)stop('inspection-limit');
            entry.tree=new WeakRef(entry.domain.getRootNode());
            entry.domain=new WeakRef(entry.domain);
            entry.nodes=entry.nodes.map(function(node){return node===null?null:new WeakRef(node);});
            entry.stack.length=0;
            entry.indices=null;
            entry.weak=true;
        }
        const saved={entries:entries,locations:locations,
            documents:new Set(Array.from(visitedDocuments,function(doc){return new WeakRef(doc);})),
            entered:enteredDocument?new WeakRef(enteredDocument):null,predicate_steps:predicateSteps};
        entries=[];locations=new WeakMap();visitedDocuments=new Set();enteredDocument=null;
        return saved;
    }
    function resume(saved){
        if(entries.length||bindings||!saved||!Array.isArray(saved.entries)||saved.entries.length>50000
            ||!(saved.locations instanceof WeakMap)||!(saved.documents instanceof Set)
            ||!Number.isSafeInteger(saved.predicate_steps)||saved.predicate_steps<0||saved.predicate_steps>limit)stop('missing-evidence');
        entries=saved.entries;locations=saved.locations;visitedDocuments=saved.documents;
        saved.entries=[];saved.locations=null;saved.documents=null;
        weakDocuments=true;predicateSteps=saved.predicate_steps;
        enteredDocument=saved.entered?WeakRef.prototype.deref.call(saved.entered):null;
        if(saved.entered&&!enteredDocument)stop('detached');
    }
    function clear(){
        for(const entry of entries){entry.nodes.length=0;entry.stack.length=0;entry.domain=null;entry.indices=null;
            if(entry.parentIndices)entry.parentIndices.length=0;if(entry.verified)entry.verified.clear();}
        entries.length=0;documents.length=0;roots.length=0;visitedDocuments.clear();locations=new WeakMap();documentRecords.clear();bindings=null;enteredDocument=null;
        rootedNodes=new WeakMap();
    }
    return {begin:begin,record:record,finish:finish,run:run,clear:clear,enter:enter,
        suspend:suspend,resume:resume,newBatchId:freshEpoch,work:function(){return predicateSteps;},
        traversalDepth:traversalDepth,withinEntered:function(node){return !enteredDocument||contains(enteredDocument,node);}};
}`
}
