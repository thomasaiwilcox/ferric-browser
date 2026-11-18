#pragma once
#include <QtCore/QString>

// Fixed isolated-world identity checks. The original Rust request and native
// receipts supply authority; this function does not query selectors, allocate
// codes, establish uniqueness/ranks or authorize an action.
inline QString ferricHintProofObjectProbe() {
    return QStringLiteral(R"JS((function(state,batch,expected,documents,captured,graphWork){
    const annotation=captured.annotation,fragmentCatalogue=captured.catalogue;
    let annotationLimit=2*1024*1024;
    if(typeof annotation!=='string'||annotation.length>annotationLimit||!(batch.nodes instanceof Map)||batch.nodes.size>50000
        ||!(batch.targets instanceof Map)||batch.targets.size>5000)return false;
    // Identity-valid facts must also come from the original browser query.
    // Normalize native JSON spelling in this same JS engine, preserving the
    // generated key order; the stored producer string is never reparsed here.
    if(expected.plan!==null&&(typeof expected.plan!=='string'||typeof batch.plan!=='string'
        ||batch.plan!==JSON.stringify(JSON.parse(expected.plan))))return false;
    function sameJson(left,right,remaining){
        const pending=[[left,right]];
        while(pending.length){
            if(--remaining<0)return false;
            const pair=pending.pop(),left=pair[0],right=pair[1];
            if(left===right)continue;
            if(left===null||right===null||typeof left!=='object'||typeof right!=='object')return false;
            if(Array.isArray(left)!==Array.isArray(right))return false;
            const keys=Object.keys(left);
            if(keys.length!==Object.keys(right).length)return false;
            for(const key of keys){
                if(!Object.prototype.hasOwnProperty.call(right,key))return false;
                pending.push([left[key],right[key]]);
            }
            if(pending.length>100000)return false;
        }
        return true;
    }
    if(expected.ordinary_input!==null){
        // Expected input comes from the retained Rust request and the issued
        // ordinary grant. Bound the producer string before parsing; sixfold
        // JSON escaping accommodates equivalent key/Unicode encoding without
        // inventing the small envelope's cap for the full selector snapshot.
        if(typeof expected.ordinary_input!=='string'||typeof batch.ordinary_input!=='string'
            ||batch.ordinary_input.length>6*expected.ordinary_input.length+512)return false;
        try{
            if(!sameJson(JSON.parse(batch.ordinary_input),JSON.parse(expected.ordinary_input),
                expected.ordinary_input.length))return false;
        }catch(error){return false;}
    }
    if(expected.annotation_work!==null){
        const work=expected.annotation_work;
        // Snapshot JSON contains only fixed ASCII keys, enum/owner/path text
        // and numbers. Check that invariant before treating its length as its
        // UTF-8 byte count. Catalogue identity/geometry are checked below;
        // these byte checks alone do not create a native snapshot receipt.
        if(!Number.isSafeInteger(batch.annotation_work)||batch.annotation_work!==work.bytes
            ||typeof fragmentCatalogue!=='string'
            ||fragmentCatalogue.length>work.bytes-work.raw_bytes
            ||/[^\x00-\x7f]/.test(fragmentCatalogue))return false;
        annotationLimit=Math.min(annotationLimit,work.bytes-fragmentCatalogue.length);
    }
    const facts=expected.facts,stamp=JSON.parse(expected.stamp);
    if(annotation!==expected.annotation){
        // Bound the retained renderer carrier itself before JSON.parse. The
        // expected Rust wire's bound does not bound a different spelling.
        let bytes=0;
        for(let index=0;index<annotation.length;index++){
            const unit=annotation.charCodeAt(index);
            if(unit<=0x7f)bytes++;
            else if(unit<=0x7ff)bytes+=2;
            else if(unit>=0xd800&&unit<=0xdbff&&index+1<annotation.length
                &&annotation.charCodeAt(index+1)>=0xdc00&&annotation.charCodeAt(index+1)<=0xdfff){bytes+=4;index++;}
            else bytes+=3;
            if(bytes>annotationLimit)return false;
        }
        // Preserve JSON meaning across harmless whitespace/key re-encoding.
        // Each comparison consumes at least one character of the bounded wire
        // input; explicit work/stack bounds avoid recursive graph comparison.
        if(!sameJson(JSON.parse(annotation),facts,expected.annotation.length))return false;
    }
    if(!facts||facts.version!==1||!Array.isArray(facts.roots)||facts.roots.length>50000
        ||!Array.isArray(facts.outcomes)||facts.outcomes.length>80)return false;
    let checks=0;
    // This meter covers the original graph loop only. Parsing/canonicalization
    // above and the document wrapper need their own admission and work bounds.
    function tick(){if(checks===100000)throw Error('proof-check-limit');graphWork.steps=++checks;}
    function id(value){return Number.isInteger(value)&&value>0&&value<=4294967295;}
    function deref(value){return value?WeakRef.prototype.deref.call(value):null;}
    function shape(value,names){return !!value&&typeof value==='object'&&!Array.isArray(value)
        &&Object.keys(value).length===names.length&&names.every(function(name){return Object.prototype.hasOwnProperty.call(value,name);});}
    const fragmentEntries=new Map(),usedFragments=new Set();
    if(expected.annotation_work!==null){
        const catalogue=JSON.parse(fragmentCatalogue);
        if(!shape(catalogue,['version','targets'])||catalogue.version!==1
            ||!Array.isArray(catalogue.targets)||catalogue.targets.length>5000)return false;
        for(const entry of catalogue.targets){
            tick();
            if(!shape(entry,['candidate','document','fragments'])||!id(entry.document)
                ||!shape(entry.candidate,['index','frame_owner','frame_path','element_id','related_element_id'])
                ||!Number.isInteger(entry.candidate.index)||entry.candidate.index<0||entry.candidate.index>=5000
                ||fragmentEntries.has(entry.candidate.index))return false;
            fragmentEntries.set(entry.candidate.index,entry);
        }
    }
    function size(value){return shape(value,['width','height'])&&Number.isFinite(value.width)&&value.width>0
        &&Number.isFinite(value.height)&&value.height>0;}
    function rect(value){return shape(value,['x','y','width','height'])&&Number.isFinite(value.x)&&Number.isFinite(value.y)
        &&Number.isFinite(value.width)&&value.width>0&&Number.isFinite(value.height)&&value.height>0
        &&Number.isFinite(value.x+value.width)&&Number.isFinite(value.y+value.height);}
    function sameSize(left,right){return size(left)&&size(right)&&left.width===right.width&&left.height===right.height;}
    function sameRect(left,right){return rect(left)&&rect(right)&&left.x===right.x&&left.y===right.y
        &&left.width===right.width&&left.height===right.height;}
    function inside(value,bounds){return value.x>=bounds.x-1e-6&&value.y>=bounds.y-1e-6
        &&value.x+value.width<=bounds.x+bounds.width+1e-6&&value.y+value.height<=bounds.y+bounds.height+1e-6;}
    function fragments(witness,captured,record){
        if(expected.annotation_work===null)return true;
        const entry=fragmentEntries.get(witness.candidate.index);
        if(!entry||entry.document!==captured.document)return false;
        for(const key of ['index','frame_owner','frame_path','element_id','related_element_id'])
            if(entry.candidate[key]!==witness.candidate[key])return false;
        const value=entry.fragments;
        const ordinary=expected.ordinary_geometry===null?null:expected.ordinary_geometry[witness.candidate.index];
        if(expected.ordinary_geometry!==null&&(!ordinary||ordinary.index!==witness.candidate.index
            ||ordinary.kind!==record.kind||!rect(ordinary.geometry)))return false;
        if(!shape(value,['complete','kind','policy','target_border_box_css','top_viewport_css',
            'owner_document_clip_css','scope_clip_css','projected_primary_fragment_css','fragment_index','eligible_fragments'])
            ||value.complete!==true||value.kind!==record.kind)return false;
        const kind=record.kind,family=record.family;
        const policy=family==='scrollables'||kind==='scrollable'||kind==='scrollable-region'?'scroll':
            ['input','select','textarea','contenteditable','focusable','focusable-surface','frame'].includes(kind)?'focus':
            family==='regions'||family==='surfaces'?'visible':kind==='link'?'link':'pointer';
        if(value.policy!==policy||!sameSize(value.target_border_box_css,witness.target_border_box_css)
            ||!sameSize(value.top_viewport_css,witness.top_viewport_css)
            ||value.top_viewport_css.width!==innerWidth||value.top_viewport_css.height!==innerHeight
            ||!sameRect(value.projected_primary_fragment_css,witness.projected_primary_fragment_css)
            ||value.fragment_index!==witness.fragment_index||!rect(value.owner_document_clip_css)
            ||!rect(value.scope_clip_css)||!Array.isArray(value.eligible_fragments)
            ||value.eligible_fragments.length<1||value.eligible_fragments.length>8)return false;
        // Owner-document clipping is in another coordinate space for nested
        // frames. Only collector-world rectangles use the scope/viewport here.
        const viewport={x:0,y:0,width:value.top_viewport_css.width,height:value.top_viewport_css.height};
        let previous=-1,primary=null,canonical=null;
        for(const alternative of value.eligible_fragments){
            tick();
            if(!shape(alternative,['index','rect'])||!Number.isInteger(alternative.index)
                ||alternative.index<=previous||alternative.index>=8||!rect(alternative.rect)
                ||!inside(alternative.rect,viewport)||!inside(alternative.rect,value.scope_clip_css)
                ||ordinary&&!inside(alternative.rect,ordinary.geometry))return false;
            previous=alternative.index;
            if(alternative.index===value.fragment_index)primary=alternative.rect;
            if(!canonical||alternative.rect.y<canonical.rect.y
                ||alternative.rect.y===canonical.rect.y&&alternative.rect.x<canonical.rect.x)canonical=alternative;
        }
        if(!primary||!sameRect(primary,value.projected_primary_fragment_css)
            ||value.fragment_index!==canonical.index||!sameRect(primary,canonical.rect))return false;
        if(!usedFragments.has(witness.candidate.index)){
            const encoded=JSON.stringify(entry);
            if(captured.fragment_json!==encoded||captured.fragment_bytes!==encoded.length
                ||JSON.stringify(captured.fragment)!==encoded)return false;
            usedFragments.add(witness.candidate.index);
        }
        return true;
    }
    const extraDocuments=new Map(),objects=new Map(),roots=new Map();
    const physicalDepths=new WeakMap([[document,expected.baseDepth]]);
    function physicalDepth(value){
        const path=[];let root=value.getRootNode();
        while(!physicalDepths.has(root)){
            tick();
            if(path.length+expected.baseDepth>=8)throw Error('proof-depth-limit');
            path.push(root);
            if(root.nodeType===11){
                if(root.mode!=='open'||!root.host||root.host.shadowRoot!==root)throw Error('shadow-root');
                root=root.host.getRootNode();
            }else if(root.nodeType===9){
                const view=root.defaultView,host=view&&view.frameElement;
                if(!host||!host.isConnected||host.contentDocument!==root||host.contentWindow!==view)throw Error('frame-root');
                root=host.getRootNode();
            }else throw Error('depth-root');
        }
        let depth=physicalDepths.get(root);
        if(depth+path.length>8)throw Error('proof-depth-limit');
        for(let index=path.length-1;index>=0;index--)physicalDepths.set(path[index],++depth);
        return physicalDepths.get(value.getRootNode());
    }
    function localDocument(id,extra){
        tick();
        const known=documents.get(id);
        if(known)return known.doc;
        if(!extra)return null;
        if(extraDocuments.has(id))return extraDocuments.get(id);
        const original=batch.documents.get(id),doc=original&&deref(original.document);
        let current=doc;
        for(let depth=0;current&&depth+expected.baseDepth<=8;depth++){
            tick();
            const entry=batch.documents.get(state.weak.get(current));
            if(!entry||deref(entry.document)!==current||!current.defaultView||current.defaultView.document!==current
                ||!state.proofEpochs||state.proofEpochs.get(current)!==entry.epoch
                ||String(current.defaultView.origin)!==entry.origin)return null;
            if(current===document){
                if(entry.path!=='0'||entry.host||entry.parent)return null;
                extraDocuments.set(id,doc);return doc;
            }
            const host=deref(entry.host),parent=deref(entry.parent);
            if(!host||!parent||!host.isConnected||host.ownerDocument!==parent||host.contentDocument!==current
                ||host.contentWindow!==current.defaultView||current.defaultView.frameElement!==host)return null;
            current=parent;
        }
        return null;
    }
    function node(ref,kind,extra){
        tick();
        if(!ref||!id(ref.document)||!id(ref.node)||(!extra&&!documents.has(ref.document)))throw Error('node-ref');
        const record=batch.nodes.get(ref.node);
        if(!record||record.document!==ref.document)throw Error('node-document');
        let object=objects.get(ref.node);
        if(!object){
            const doc=localDocument(ref.document,extra);
            object=deref(record.object);
            if(!object||!doc||record.document!==ref.document||state.weak.get(object)!==ref.node
                ||(object.nodeType===9?object:object.ownerDocument)!==doc
                ||deref(record.root)!==object.getRootNode()
                ||deref(record.assigned)!==(object.assignedSlot||null))throw Error('node-identity');
            if(object.nodeType===9){if(object!==doc)throw Error('document-node');}
            else if(!object.isConnected)throw Error('detached');
            const root=object.getRootNode();
            if(root.nodeType===11&&(!root.host||root.mode!=='open'||root.host.shadowRoot!==root))throw Error('shadow-root');
            objects.set(ref.node,object);
        }
        if(!['document','element','open-shadow'].includes(kind)
            ||(kind==='document'&&object.nodeType!==9)||(kind==='element'&&object.nodeType!==1)
            ||(kind==='open-shadow'&&(object.nodeType!==11||object.mode!=='open')))throw Error('node-kind');
        physicalDepth(object);
        return object;
    }
    function contains(root,value){tick();return root===value||root.contains(value);}
    function parent(value){
        if(value.assignedSlot)return value.assignedSlot;
        if(value.parentNode)return value.parentNode;
        if(value.nodeType===11)return value.host;
        if(value.nodeType===9&&value!==document)return value.defaultView&&value.defaultView.frameElement;
        return null;
    }
    function composed(root,value){
        for(;value;value=parent(value)){tick();if(value===root)return true;}
        return false;
    }
    let permit=null,entered=null;
    const permitted=new WeakMap(),enteredNodes=new WeakMap();
    if(expected.scope===null){
        if(stamp.scope_lease!==''||state.scopeLease||batch.scope!==null)return false;
    }else{
        const scope=expected.scope,lease=state.scopeLease,captured=batch.scope;
        if(!lease||!captured||scope.lease_id!==stamp.scope_lease)return false;
        const fields=['lease_id','element_id','frame_path','kind','selector_revision','parent_scope_lease_id'];
        if(Object.keys(scope).length!==fields.length||!fields.every(key=>Object.prototype.hasOwnProperty.call(scope,key)))return false;
        for(const field of fields)
            if(captured[field]!==scope[field])return false;
        if(lease.leaseId!==scope.lease_id||lease.elementId!==scope.element_id||lease.framePath!==scope.frame_path
            ||lease.kind!==scope.kind||lease.selectorRevision!==scope.selector_revision
            ||(lease.parentScopeLeaseId||null)!==scope.parent_scope_lease_id)return false;
        permit=node(captured.node,'element',true);
        const doc=permit.ownerDocument,entry=batch.documents.get(captured.node.document);
        if(captured.node.node!==scope.element_id||!entry||entry.path!==scope.frame_path
            ||deref(lease.elementRef)!==permit||deref(lease.ownerDocumentRef)!==doc)return false;
    }
    if(expected.entered===null){
        if(batch.entered||state.enteredScope)return false;
    }else{
        const native=expected.entered,scope=state.enteredScope;
        if(!scope||!batch.entered||expected.owner!==''||expected.baseDepth!==0
            ||!Number.isInteger(native.depth)||native.depth<1||native.depth>8
            ||!Array.isArray(native.chain)||native.chain.length!==native.depth+1)return false;
        entered=node(batch.entered,'document',true);
        const leaf=native.chain[native.depth],entry=batch.documents.get(batch.entered.document);
        if(!entry||deref(scope.document)!==entered||scope.epoch!==leaf.epoch||scope.origin!==leaf.origin
            ||scope.path!==entry.path)return false;
        let doc=entered,globalDepth=0;
        for(let depth=native.depth;depth>=0;depth--){
            tick();
            const current=native.chain[depth],view=doc&&doc.defaultView;
            if(!view||view.document!==doc||view[expected.nativeProperty]!==current.epoch
                ||String(view.origin)!==current.origin||!doc.hasFocus())return false;
            if(depth===0){if(doc!==document)return false;break;}
            const original=batch.documents.get(state.weak.get(doc)),host=original&&deref(original.host),parentDoc=original&&deref(original.parent);
            if(!host||!parentDoc||!host.isConnected||host.ownerDocument!==parentDoc
                ||host.contentDocument!==doc||host.contentWindow!==view||view.frameElement!==host)return false;
            if(node({document:state.weak.get(parentDoc),node:state.weak.get(host)},'element',true)!==host)return false;
            let root=host.getRootNode();
            while(root.nodeType===11){
                tick();
                if(++globalDepth>8||root.mode!=='open'||!root.host||root.host.shadowRoot!==root)return false;
                root=root.host.getRootNode();
            }
            if(root!==parentDoc||++globalDepth>8)return false;
            doc=parentDoc;
        }
        if(permit&&!composed(permit,entered)&&!composed(entered,permit))return false;
    }
    function withinPermit(value,root,cache){
        if(!root)return true;
        const path=[];let current=value,result=false;
        for(;current;current=parent(current)){
            tick();
            if(current===root){result=true;break;}
            if(cache.has(current)){result=cache.get(current);break;}
            path.push(current);
        }
        for(const node of path)cache.set(node,result);
        return result;
    }
    function within(value){return withinPermit(value,permit,permitted)&&withinPermit(value,entered,enteredNodes);}
    function intersects(value){
        return (!permit||withinPermit(value,permit,permitted)||composed(value,permit))
            &&(!entered||withinPermit(value,entered,enteredNodes)||composed(value,entered));
    }
    for(const fact of facts.roots){
        tick();
        if(!id(fact.id)||roots.has(fact.id)||!Array.isArray(fact.hops)||fact.hops.length>8)return false;
        const start=node(fact.start,fact.start_kind),depth=physicalDepth(start)+fact.hops.length;let current=start;
        if(depth>8)return false;
        if(!intersects(start))return false;
        for(const tagged of fact.hops){
            tick();
            const kinds=Object.keys(tagged);if(kinds.length!==1)return false;
            const kind=kinds[0],hop=tagged[kind],host=node(kind==='assigned-slot'?hop.slot:hop.host,'element');
            if(!contains(current,host)||!intersects(host))return false;
            if(kind==='open-shadow'){
                current=node(hop.root,'open-shadow');
                if(host.shadowRoot!==current||current.host!==host)return false;
            }else if(kind==='same-origin-frame'){
                current=node(hop.document,'document');
                if(host.contentDocument!==current||host.contentWindow!==current.defaultView
                    ||current.defaultView.frameElement!==host)return false;
            }else if(kind==='assigned-slot'){
                current=node(hop.assigned,'element');
                if(host.localName!=='slot'||current.assignedSlot!==host||current.ownerDocument!==host.ownerDocument
                    ||hop.owner_document!==state.weak.get(current.ownerDocument))return false;
            }else return false;
            if(!intersects(current))return false;
        }
        if(current!==node(fact.resolved,fact.resolved_kind))return false;
        roots.set(fact.id,{fact:fact,start:start,resolved:current,depth:depth});
    }
    function root(id){tick();const value=roots.get(id);if(!value)throw Error('root-ref');return value;}
    function targets(values,path,scope){
        if(!Array.isArray(values)||values.length>5000)return false;
        for(const witness of values){
            tick();
            const ref=witness.candidate,captured=batch.targets.get(ref.index),record=state.elements.get(ref.element_id);
            if(!captured||!record||captured.document!==path.fact.resolved.document)return false;
            for(const key of ['index','frame_owner','frame_path','element_id','related_element_id'])
                if(captured.reference[key]!==ref[key])return false;
            const target=node({document:captured.document,node:ref.element_id},'element');
            if(target!==deref(captured.element)||record.element!==target||record.ownerDocument!==target.ownerDocument
                ||record.framePath!==ref.frame_path||ref.frame_owner!==expected.owner
                ||record.relatedElementId!==ref.related_element_id||!contains(scope,target)||!within(target))return false;
            const related=ref.related_element_id===null?null:node({document:captured.document,node:ref.related_element_id},'element');
            if(related!==deref(captured.related)||related!==record.relatedElement||(related&&!within(related)))return false;
            if(!fragments(witness,captured,record))return false;
        }
        return true;
    }
    for(const outcome of facts.outcomes){
        tick();
        const kinds=Object.keys(outcome.result);if(kinds.length!==1)return false;
        const kind=kinds[0],value=outcome.result[kind];
        if(kind==='unavailable')continue;
        const path=root(value.root);
        if(kind==='search-complete'){
            const stage=value.stage&&value.stage['assigned-element'];
            if(stage){const slot=node(stage.slot,'element');if(slot.localName!=='slot'||!contains(path.resolved,slot)||!intersects(slot))return false;}
        }else if(kind==='anchor-observed'||kind==='anchor-complete'){
            const scope=node(value.scope,value.scope_kind);
            if(!contains(path.resolved,scope)||!intersects(scope)||!targets(value.targets||value.matches,path,scope))return false;
        }else if(kind==='collection-observed'||kind==='collection-complete'){
            const container=node(value.container,value.container_kind);
            if(!contains(path.resolved,container)||!intersects(container)||!Array.isArray(value.items)||value.items.length>50000)return false;
            for(const item of value.items){
                tick();
                const logical=node(item.item,'element');
                if(!contains(container,logical)||!intersects(logical))return false;
                if(item.parent_item){const parent=node(item.parent_item,'element');if(parent===logical||!contains(parent,logical)||!intersects(parent))return false;}
                const complete=kind==='collection-complete'?{root:item.primary_root,targets:[item.primary]}:item.primary['targets-complete'];
                if(complete){const primary=root(complete.root);if(primary.start!==logical
                    ||Math.max(path.depth,physicalDepth(logical))+primary.fact.hops.length>8
                    ||!targets(complete.targets,primary,primary.resolved))return false;}
            }
        }else return false;
    }
    // The final raw outcomes own membership, including predicate-excluded
    // witnesses. Discarded prefixes cannot leave an extra cached target.
    return expected.annotation_work===null||(usedFragments.size===fragmentEntries.size
        &&usedFragments.size===batch.targets.size);
}))JS");
}
