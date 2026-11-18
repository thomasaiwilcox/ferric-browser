#pragma once
#include <QtCore/QString>

// Fixed isolated-world code. Native code supplies issuer/document/child tokens;
// no renderer payload chooses the parent world or the native child frame.
inline QString ferricHintFrameAncestryProbe() {
    return QStringLiteral(R"JS((function(request){
    let records=window.__ferric_hintNativeAncestry;
    if(request.mode==='release'){
        if(request.tokens!==undefined&&(!Array.isArray(request.tokens)||request.tokens.length>64
            ||request.tokens.some(token=>typeof token!=='string')))return false;
        const tokens=request.tokens===undefined?null:new Set(request.tokens);
        if(records instanceof Map)for(const [key,record] of records){
            if(record.issuer===request.issuer&&(tokens?tokens.has(key):!request.token||key===request.token)){
                record.release();records.delete(key);
            }
        }
        return true;
    }
    try{
        if(window[request.property]!==request.epoch||String(window.origin)!==request.origin
            ||document.hasFocus()!==true||typeof WeakRef!=='function')return null;
        function deref(value){return value?WeakRef.prototype.deref.call(value):null;}
        let work=0;
        function tick(){if(++work>1024)throw Error('ancestry-limit');}
        function geometry(frame){
            tick();const rect=frame.getBoundingClientRect();
            const value=[rect.x,rect.y,rect.width,rect.height,
                frame.clientLeft,frame.clientTop,frame.clientWidth,frame.clientHeight];
            return value.every(Number.isFinite)?value:null;
        }
        function focused(){
            let active=document.activeElement;
            for(let depth=0;active&&active.shadowRoot;depth++){
                tick();if(depth>=8||active.shadowRoot.mode!=='open')return null;
                const inner=active.shadowRoot.activeElement;
                if(!inner||inner===active)return null;
                active=inner;
            }
            return active&&active.ownerDocument===document&&active.isConnected
                &&['iframe','frame'].includes(active.localName)?active:null;
        }
        const host=focused();if(!host)return null;
        if(request.mode==='verify'){
            const record=records instanceof Map&&records.get(request.token);
            // Observer callbacks classify unrelated updates outside this
            // native probe. Unprocessed records remain a conservative fence;
            // verification never scans a mutation batch on its work allowance.
            if(record&&record.observer.takeRecords().length)record.dirty=true;
            if(!record||record.issuer!==request.issuer||record.epoch!==request.epoch
                ||record.child!==request.child||record.childEpoch!==request.child_epoch
                ||record.dirty||deref(record.host)!==host
                ||deref(record.document)!==document)return null;
            const currentGeometry=geometry(host);
            if(!currentGeometry||!Array.isArray(record.geometry)||record.geometry.length!==8
                ||currentGeometry.some((value,index)=>value!==record.geometry[index]))return null;
            for(const entry of record.path){
                tick();const node=deref(entry.node);
                if(!node||!node.isConnected||node.ownerDocument!==document
                    ||node.parentNode!==deref(entry.parent)||node.assignedSlot!==deref(entry.slot))return null;
            }
            for(const entry of record.roots){
                tick();const root=deref(entry.root),owner=deref(entry.host);
                if(!root||root.mode!=='open'||root.host!==owner||!owner||owner.shadowRoot!==root)return null;
            }
            return {depth:record.depth,checks:work};
        }
        if(request.mode!=='capture'||window.__ferric_hintNativeAncestryLifecycle!==1)return null;
        if(!records){records=new Map();window.__ferric_hintNativeAncestry=records;}
        if(!(records instanceof Map)||records.size>=64||records.has(request.token))return null;
        const path=[],roots=[],slots=new Set(),seenNodes=new WeakSet(),seenRoots=new WeakSet();let current=host,depth=0;
        // Actual parent/root ownership determines depth. A light-DOM frame
        // assigned to a slot stays in its owner Document; assignment is a
        // dependency, not an extra implicit shadow traversal.
        while(current!==document){
            tick();
            if(!current||!current.isConnected||current.ownerDocument!==document||path.length>=256)return null;
            if(current.nodeType===11){
                if(current.mode!=='open'||!current.host||current.host.shadowRoot!==current||++depth>8)return null;
                roots.push({root:new WeakRef(current),host:new WeakRef(current.host)});
                seenRoots.add(current);
                current=current.host;continue;
            }
            const slot=current.assignedSlot||null;
            if(slot){if(slot.ownerDocument!==document||!slot.isConnected)return null;slots.add(slot);}
            path.push({node:new WeakRef(current),parent:current.parentNode?new WeakRef(current.parentNode):null,
                slot:slot?new WeakRef(slot):null});
            seenNodes.add(current);
            current=current.parentNode;
        }
        // A direct assigned slot can itself be forwarded into another slot.
        // Retain the entire bounded placement graph, including manual slots,
        // without charging those dependencies as the frame's actual depth.
        const pending=Array.from(slots),seenSlots=new WeakSet();
        while(pending.length){
            tick();const slot=pending.pop();
            if(seenSlots.has(slot))continue;seenSlots.add(slot);
            current=slot;
            while(current!==document){
                tick();
                if(!current||!current.isConnected||current.ownerDocument!==document)return null;
                if(current.nodeType===11){
                    if(current.mode!=='open'||!current.host||current.host.shadowRoot!==current)return null;
                    if(!seenRoots.has(current)){
                        roots.push({root:new WeakRef(current),host:new WeakRef(current.host)});seenRoots.add(current);
                    }
                    current=current.host;continue;
                }
                if(seenNodes.has(current))break;
                if(path.length>=256)return null;
                const assigned=current.assignedSlot||null;
                if(assigned){
                    if(assigned.ownerDocument!==document||!assigned.isConnected)return null;
                    slots.add(assigned);pending.push(assigned);
                }
                path.push({node:new WeakRef(current),parent:current.parentNode?new WeakRef(current.parentNode):null,
                    slot:assigned?new WeakRef(assigned):null});
                seenNodes.add(current);current=current.parentNode;
            }
        }
        const hostGeometry=geometry(host);if(!hostGeometry)return null;
        const record={issuer:request.issuer,epoch:request.epoch,child:request.child,childEpoch:request.child_epoch,
            host:new WeakRef(host),document:new WeakRef(document),path:path,roots:roots,depth:depth,
            geometry:hostGeometry,dirty:false};
        const dirty=function(){record.dirty=true;};
        const slotPlacement=roots.length>0||slots.size>0;
        // This receipt proves frame placement, not page-wide quiescence.
        // Keep ABA retirement for actual path nodes and slot dependencies;
        // unrelated progress/style/text updates cannot change that placement.
        const changed=function(rows){
            let remaining=1024;
            for(const row of rows){
                if(--remaining<0){dirty();return;}
                if(row.type==='attributes'){
                    if(row.attributeName==='slot'||row.attributeName==='name'
                        ||seenNodes.has(row.target)&&['src','srcdoc','sandbox'].includes(row.attributeName)){dirty();return;}
                }else{
                    // Slot placement also depends on other nodes in its root.
                    // Preserve the conservative structural fence for those
                    // graphs; ordinary light-DOM frames need only path edges.
                    if(slotPlacement){dirty();return;}
                    for(const nodes of [row.addedNodes,row.removedNodes])for(const node of nodes){
                        if(--remaining<0||seenNodes.has(node)){dirty();return;}
                    }
                }
            }
        };
        const observer=new MutationObserver(changed),watched=new Set([document]);
        for(const entry of roots)watched.add(deref(entry.root));
        // Observe assigned-slot roots too, without counting them as the
        // host's own roots. Mutation/slot events retire even ABA assignments.
        for(const slot of slots)watched.add(slot.getRootNode());
        const watchedSlots=Array.from(slots,function(slot){return new WeakRef(slot);});
        record.observer=observer;
        const retire=function(){
            record.release();
            if(records.get(request.token)===record)records.delete(request.token);
        };
        record.release=function(){record.dirty=true;observer.disconnect();for(const weak of watchedSlots){
            const slot=deref(weak);if(slot)slot.removeEventListener('slotchange',dirty,true);
        }window.removeEventListener('pagehide',retire,true);};
        try{
            for(const root of watched){tick();observer.observe(root,{subtree:true,childList:true,attributes:true});}
            for(const slot of slots){tick();slot.addEventListener('slotchange',dirty,true);}
            window.addEventListener('pagehide',retire,true);
        }catch(error){record.release();return null;}
        watched.clear();slots.clear();
        records.set(request.token,record);
        return {depth:depth,checks:work};
    }catch(error){return null;}
}))JS");
}
