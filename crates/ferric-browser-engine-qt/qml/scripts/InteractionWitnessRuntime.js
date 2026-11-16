.pragma library

// Private per-pass observations. Only final ordinary rows can supply witnesses;
// these facts do not allocate codes or grant an action/receiver permit.
function source() {
    return `function ftInteractionAnnotationSize(value,limit){
    let remaining=limit;
    function take(bytes){if(bytes>remaining)throw {ferricRootReason:'annotation-limit'};remaining-=bytes;}
    function string(text){
        take(2);
        for(let i=0;i<text.length;i++){
            const unit=text.charCodeAt(i);
            if(unit===34||unit===92||unit===8||unit===9||unit===10||unit===12||unit===13)take(2);
            else if(unit<32)take(6);
            else if(unit<128)take(1);
            else if(unit<2048)take(2);
            else if(unit>=0xd800&&unit<=0xdbff&&i+1<text.length&&text.charCodeAt(i+1)>=0xdc00&&text.charCodeAt(i+1)<=0xdfff){take(4);i++;}
            else if(unit>=0xd800&&unit<=0xdfff)take(6);
            else take(3);
        }
    }
    function measure(part,depth){
        if(depth>32)throw {ferricRootReason:'annotation-limit'};
        if(part===null){take(4);return;}
        if(typeof part==='string'){string(part);return;}
        if(typeof part==='boolean'){take(part?4:5);return;}
        if(typeof part==='number'&&Number.isFinite(part)){take(JSON.stringify(part).length);return;}
        if(!part||typeof part!=='object')throw {ferricRootReason:'missing-evidence'};
        take(2);
        if(Array.isArray(part)){
            for(let i=0;i<part.length;i++){if(i)take(1);measure(part[i],depth+1);}
        }else{
            const keys=Object.keys(part);
            for(let i=0;i<keys.length;i++){if(i)take(1);string(keys[i]);take(1);measure(part[keys[i]],depth+1);}
        }
    }
    measure(value,0);
    return limit-remaining;
}
function ftInteractionAnnotationJson(value,limit){
    // Preflight escaped UTF-8 bytes BEFORE JSON.stringify allocates its output.
    // Ordinary candidate JSON is independent of this annotation-only bound.
    ftInteractionAnnotationSize(value,limit);
    return JSON.stringify(value);
}
function ftInteractionWitnessPass(context){
    let coverage=new WeakMap(),materials=new WeakMap(),rows=new WeakMap();
    let remainingBytes=context.annotationLimit===undefined?2097152:context.annotationLimit;
    function take(bytes){
        if(bytes>remainingBytes){remainingBytes=0;throw {ferricRootReason:'annotation-limit'};}
        remainingBytes-=bytes;
    }
    function reserve(value,placeholders){
        const discount=placeholders||0;
        try{remainingBytes-=ftInteractionAnnotationSize(value,remainingBytes+discount)-discount;}
        catch(error){remainingBytes=0;throw error;}
    }
    // Charged reservations include discarded prefixes and failed debits.
    // This counter is not a measurement of allocated or retained memory.
    function annotationWork(){return (context.annotationLimit===undefined?2097152:context.annotationLimit)-remainingBytes;}
    function begin(element){
        let value=coverage.get(element);
        if(!value){value={pending:0,blocked:false};coverage.set(element,value);}
        return context.failures();
    }
    function queue(element,path){begin(element);coverage.get(element).pending|=path;}
    function finish(element,path,before){
        const value=coverage.get(element);
        if(!value)return;
        value.pending&=~path;
        if(context.failures()!==before)value.blocked=true;
    }
    function admit(element,material){materials.set(element,material);}
    function finalize(candidates,elements){
        rows=new WeakMap();
        for(let index=0;index<candidates.length;index++){
            const candidate=candidates[index],record=elements.get(candidate.element_id);
            if(record)rows.set(record.element,{index:index,candidate:candidate,record:record,material:materials.get(record.element)});
        }
    }
    function attribute(element,name){
        let fact;
        if(!context.inspect())fact={missing:{}};
        else{
            const value=element.getAttribute(name);
            fact=value===null?{'complete-absent':{}}:{'complete-present':{value:value}};
        }
        // Do not retain a large DOM string in a growing observation table
        // before checking the shared annotation budget. No value is shortened
        // into a complete fact when this admission fails.
        reserve(fact);
        return fact;
    }
    function attributes(element,names){
        const fields=names||[],values=[];
        take(2+Math.max(0,fields.length-1));
        for(const name of fields){
            reserve({name:name,fact:null},4);
            values.push({name:name,fact:attribute(element,name)});
        }
        return values;
    }
    function knownExcluded(element){
        const value=coverage.get(element);
        return !rows.has(element)&&!!value&&!value.pending&&!value.blocked;
    }
    function observe(element,names){
        const row=rows.get(element);
        if(!row){
            const value=coverage.get(element);
            return value&&!value.pending&&!value.blocked?{excluded:true}:{unavailable:'truncated'};
        }
        const record=row.record;let material=row.material;
        if(!material)return {unavailable:'missing-evidence'};
        if(!element.isConnected||record.ownerDocument!==element.ownerDocument||record.element!==element)return {unavailable:'detached'};
        if(record.relatedElement&&(!record.relatedElement.isConnected||record.relatedElement.ownerDocument!==record.ownerDocument))return {unavailable:'detached'};
        if(context.deferred){
            if(context.state.elements.get(row.candidate.element_id)!==record)return {unavailable:'changed-document'};
            if(!context.inspect())return {unavailable:'inspection-limit'};
            const local=ftLocalVisible(element,element.ownerDocument.defaultView,material.policy,context.cache);
            const projected=local&&ftProjectedRect(element,local,material.policy,context.cache);
            const top=projected&&ftIntersect(projected,material.topClip),expected=row.candidate.geometry;
            if(!top||top.left!==expected.x||top.top!==expected.y||top.width!==expected.width||top.height!==expected.height
                ||local.raw.width!==material.local.raw.width||local.raw.height!==material.local.raw.height)return {unavailable:'changed-document'};
            material={local:local,policy:material.policy,topClip:material.topClip,evidence:material.evidence};
        }
        let geometry=material.fragmentEvidence;
        if(!geometry){
            const rect=material.topRect,projected=material.projected;
            if(material.singleBorder&&element.ownerDocument===document
                    &&rect.left===projected.left&&rect.top===projected.top&&rect.right===projected.right&&rect.bottom===projected.bottom){
                // The one original border fragment already passed this action
                // policy's local clip/hit/projection checks. No extra scope
                // crop or parent-frame check is needed. Reusing those exact
                // facts lets a complete small scope survive unrelated work
                // exhaustion without a new DOM read or a fresh allowance.
                const primary={x:rect.left,y:rect.top,width:rect.width,height:rect.height};
                geometry={complete:true,reason:null,primary:primary,primary_index:0,eligible_fragments:[{index:0,rect:primary}]};
            }else{
                geometry={};
                ftHintPresentation(element,material.local,material.policy,material.topClip,context.cache,geometry);
            }
            material.fragmentEvidence=geometry;
        }
        if(!geometry.complete)return {unavailable:geometry.reason||'missing-evidence'};
        if(!geometry.primary)return {excluded:true};
        const facts=material.evidence||{},name=facts.name||ftHintNameEvidence(element,false);
        const nameFact=name.complete?{'complete-present':{value:name.value}}:{truncated:{}};
        const hrefFact=facts.href||{missing:{}};
        reserve(nameFact);reserve(hrefFact);
        const witness={
            candidate:{index:row.index,frame_owner:context.owner,frame_path:row.candidate.frame_path,element_id:row.candidate.element_id,related_element_id:row.candidate.related_element_id},
            name:nameFact,href:hrefFact,role:attribute(element,'role'),attributes:attributes(element,names),
            target_border_box_css:{width:material.local.raw.width,height:material.local.raw.height},
            projected_primary_fragment_css:geometry.primary,fragment_index:geometry.primary_index,top_viewport_css:{width:innerWidth,height:innerHeight}
        };
        reserve(Object.assign({},witness,{name:null,href:null,role:null,attributes:null}),16);
        const clip=material.local.clipped,scope=material.topClip;
        return {witness:witness,fragments:{complete:true,kind:row.candidate.kind,policy:material.policy,
            target_border_box_css:witness.target_border_box_css,top_viewport_css:witness.top_viewport_css,
            owner_document_clip_css:{x:clip.left,y:clip.top,width:clip.right-clip.left,height:clip.bottom-clip.top},
            scope_clip_css:{x:scope.left,y:scope.top,width:scope.right-scope.left,height:scope.bottom-scope.top},
            projected_primary_fragment_css:geometry.primary,fragment_index:geometry.primary_index,
            eligible_fragments:geometry.eligible_fragments}};
    }
    function clear(){coverage=null;materials=null;rows=null;}
    function suspend(){
        // WeakMap keys do not keep visited/ordinary DOM objects alive. Values
        // contain only coverage scalars, original records and material facts;
        // neither the context nor its style cache crosses the callback.
        const saved={coverage:coverage,materials:materials,rows:rows};
        coverage=null;materials=null;rows=null;
        return saved;
    }
    function resume(saved){
        if(!saved||!(saved.coverage instanceof WeakMap)||!(saved.materials instanceof WeakMap)
            ||!(saved.rows instanceof WeakMap))throw {ferricRootReason:'missing-evidence'};
        coverage=saved.coverage;materials=saved.materials;rows=saved.rows;
        saved.coverage=null;saved.materials=null;saved.rows=null;
    }
    return {begin:begin,queue:queue,finish:finish,admit:admit,finalize:finalize,knownExcluded:knownExcluded,observe:observe,attributes:attributes,reserve:reserve,reserveBytes:take,annotationWork:annotationWork,clear:clear,suspend:suspend,resume:resume};
}`
}
