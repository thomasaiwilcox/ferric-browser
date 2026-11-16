import QtQuick
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as Scripts

// Browser-owned routing and projection for an explicitly focused frame chain.
// Parent iframe references stay in the parent's isolated world; native handles
// stay in the broker. Only bounded facts cross either boundary.
Item {
    id: provider
    visible: false
    required property var browserWindow
    property var view: null
    property string rootOwner: ""
    property string scopedOwner: ""
    property var sources: ({})
    property int epoch: 0
    property bool pollPending: false
    property string lastFailure: ""
    property var polledRevisions: ({})
    property var geometryStamp: null
    readonly property int leaseCount: broker.leaseCount
    readonly property alias nativeBroker: broker
    FerricHintFrameBroker { id: broker }

    function root(script, callback) {
        if (!view) return false
        if (rootOwner) {run(rootOwner,script,callback);return true}
        var token = epoch
        return browserWindow.runBrowserScript(view, script, function(value) {
            if (token === epoch) callback(value)
        })
    }
    function ensure(selectedView, callback) {
        if (view && view !== selectedView) stop()
        view = selectedView
        var token = epoch
        broker.attach(view.mainFrame, function(result) {
            if (token !== epoch) return
            if (!result || result.error) { callback(result || {error:"frame-unavailable"}); return }
            rootOwner = result.owner
            callback({owner:rootOwner})
        })
    }
    function run(owner, script, callback) {
        var token = epoch
        broker.run(owner, script, function(result) {
            if (token === epoch) callback(result)
        })
    }
    function viewport(owner, callback) {
        run(owner, "({width:innerWidth,height:innerHeight,focused:document.hasFocus()})", callback)
    }
    function discover(parent, depth, foreign, modal, callback) {
        var token = epoch
        run(parent, Scripts.hintFrameHost(""), function(host) {
            if (!host || host.error || host.none) {
                callback(host && host.error ? host : {owner:parent,foreign:foreign,modal:modal})
                return
            }
            if (depth >= 8) { callback({error:"frame-depth-limit"}); return }
            broker.focusedChild(parent, function(child) {
                if (token !== epoch) return
                if (!child || child.error) { callback(child || {error:"frame-unavailable"}); return }
                viewport(child.owner, function(size) {
                    if (!size || size.error || !size.focused) { callback({error:"frame-focus-lost"}); return }
                    run(parent, Scripts.hintFrameHost(host.host, size), function(checked) {
                        if (!checked || checked.error) { callback(checked || {error:"frame-unavailable"}); return }
                        sources[child.owner] = {parent:parent,host:host.host}
                        discover(child.owner, depth + 1, foreign || checked.cross_origin,
                                 modal || checked.modal, callback)
                    })
                })
            })
        })
    }
    function chain(owner) {
        var result = [], current = owner
        for (var depth = 0; current !== rootOwner && depth < 8; ++depth) {
            var source = sources[current]
            if (!source) return null
            result.unshift({owner:current,parent:source.parent,host:source.host})
            current = source.parent
        }
        return current === rootOwner ? result : null
    }
    function verifyChain(owner, callback) {
        var token = epoch
        var entries = chain(owner)
        if (!entries || !entries.length) { callback({error:"frame-lease-unavailable"}); return }
        var mappings = [], viewports = []
        function next(index) {
            if (token !== epoch) return
            if (index === entries.length) {
                broker.verifyFocusedLeaf(owner, function(leaf) {
                    if (token !== epoch) return
                    if (leaf !== true) { callback(leaf && leaf.error ? leaf : {error:"frame-focus-lost"}); return }
                    callback({mappings:mappings,entries:entries,viewports:viewports})
                })
                return
            }
            var entry = entries[index]
            viewport(entry.owner, function(size) {
                if (!size || size.error || !size.focused) { callback({error:"frame-focus-lost"}); return }
                run(entry.parent, Scripts.hintFrameHost(entry.host, size), function(host) {
                    if (!host || host.error || !host.mapping) { callback(host || {error:"frame-unavailable"}); return }
                    broker.focusedChild(entry.parent, function(child) {
                        if (token !== epoch) return
                        if (!child || child.owner !== entry.owner) { callback({error:"frame-association-changed"}); return }
                        // The native scan is asynchronous. Re-read both sides;
                        // a new mapping cannot validate facts from the old one.
                        viewport(entry.owner, function(finalSize) {
                            if (!finalSize || !finalSize.focused) { callback({error:"frame-focus-lost"}); return }
                            if (!sameSize(size, finalSize)) { callback({error:"frame-geometry-changed"}); return }
                            run(entry.parent, Scripts.hintFrameHost(entry.host, finalSize), function(finalHost) {
                                if (!finalHost || finalHost.error || !finalHost.mapping) { callback({error:"frame-association-changed"}); return }
                                if (host.host !== finalHost.host || !sameSize(host.viewport, finalHost.viewport)
                                        || JSON.stringify(host.mapping) !== JSON.stringify(finalHost.mapping)) {
                                    callback({error:"frame-geometry-changed"}); return
                                }
                                mappings.push(finalHost.mapping)
                                viewports.push({parent:finalHost.viewport,child:{width:finalSize.width,height:finalSize.height}})
                                next(index + 1)
                            })
                        })
                    })
                })
            })
        }
        next(0)
    }
    function sameSize(first, second) {
        return validSize(first) && validSize(second)
                && first.width === second.width && first.height === second.height
    }
    function validSize(value) {
        return !!value && Number.isFinite(value.width) && Number.isFinite(value.height)
                && value.width > 0 && value.height > 0
    }
    function isArrayValue(value) {
        // The qualified Qt broker delivers QVariantList as V4Sequence, not a
        // native JS Array. Accept that exact sequence kind, never array-likes.
        return Array.isArray(value) || Object.prototype.toString.call(value) === "[object V4Sequence]"
    }
    function validRect(value) {
        return !!value && typeof value === "object" && !isArrayValue(value)
                && Object.keys(value).length === 4
                && Number.isFinite(value.x) && Number.isFinite(value.y) && validSize(value)
                && Number.isFinite(value.x + value.width) && Number.isFinite(value.y + value.height)
    }
    function inside(x, y, bounds) {
        return x >= bounds.x - 1e-6 && y >= bounds.y - 1e-6
                && x <= bounds.x + bounds.width + 1e-6 && y <= bounds.y + bounds.height + 1e-6
    }
    function validPresentation(value, bounds) {
        if (value === undefined || value === null) return true
        if (typeof value !== "object" || isArrayValue(value) || Object.keys(value).length !== 2
                || !value.anchor || typeof value.anchor !== "object" || isArrayValue(value.anchor)
                || Object.keys(value.anchor).length !== 2
                || !Number.isFinite(value.anchor.x) || !Number.isFinite(value.anchor.y)
                || !isArrayValue(value.fragments) || value.fragments.length < 1 || value.fragments.length > 8) return false
        return value.fragments.every(function(rect) {
            return validRect(rect) && inside(rect.x, rect.y, bounds)
                    && inside(rect.x + rect.width, rect.y + rect.height, bounds)
        }) && value.fragments.some(function(rect) { return inside(value.anchor.x, value.anchor.y, rect) })
    }
    function validCollection(value, inspections, candidates) {
        // Validate before subtraction, defaulting, clipping or projection can
        // turn malformed renderer fields into apparently valid budget/facts.
        return !!value && isArrayValue(value.candidates) && value.candidates.length <= candidates
                && Number.isSafeInteger(value.inspected_nodes) && value.inspected_nodes >= 0
                && value.inspected_nodes <= inspections && typeof value.truncated === "boolean"
                && Number.isSafeInteger(value.revision) && value.revision >= 0
                && typeof value.selector_revision === "string" && value.selector_revision.length <= 64
                && validSize(value.viewport) && value.candidates.every(function(candidate) {
                    return !!candidate && validRect(candidate.geometry)
                            && validPresentation(candidate.presentation, candidate.geometry)
                })
    }
    function sameChainProof(first, second) {
        return !!first && !!second && !first.error && !second.error
                && Array.isArray(first.entries) && first.entries.length > 0
                && Array.isArray(first.mappings) && first.mappings.length === first.entries.length
                && Array.isArray(first.viewports) && first.viewports.length === first.entries.length
                && JSON.stringify(first.entries) === JSON.stringify(second.entries)
                && JSON.stringify(first.mappings) === JSON.stringify(second.mappings)
                && JSON.stringify(first.viewports) === JSON.stringify(second.viewports)
    }
    function collectionFitsProof(base, child, proof) {
        return !!proof && !proof.error && Array.isArray(proof.viewports) && proof.viewports.length > 0
                && sameSize(base && base.viewport, proof.viewports[0].parent)
                && sameSize(child && child.viewport, proof.viewports[proof.viewports.length - 1].child)
    }
    function rectThrough(rect, mappings) {
        var result = {x:rect.x,y:rect.y,width:rect.width,height:rect.height}
        for (var index = mappings.length - 1; index >= 0; --index) {
            var map = mappings[index], clip = map.content
            var left = Math.max(clip.left, map.left + result.x * map.scaleX)
            var top = Math.max(clip.top, map.top + result.y * map.scaleY)
            var right = Math.min(clip.right, map.left + (result.x + result.width) * map.scaleX)
            var bottom = Math.min(clip.bottom, map.top + (result.y + result.height) * map.scaleY)
            if (![left,top,right,bottom].every(Number.isFinite) || right <= left || bottom <= top) return null
            result = {x:left,y:top,width:right-left,height:bottom-top}
        }
        return result
    }
    function project(candidate, owner, mappings) {
        if (!candidate || !validRect(candidate.geometry) || !validPresentation(candidate.presentation, candidate.geometry)) return null
        var geometry = rectThrough(candidate.geometry, mappings)
        if (!geometry) return null
        var result = Object.assign({}, candidate, {geometry:geometry,frame_owner:owner})
        if (candidate.presentation) {
            var fragments = candidate.presentation.fragments.map(function(rect) {return rectThrough(rect,mappings)}).filter(Boolean)
            if (!fragments.length) return null
            var anchor = {x:candidate.presentation.anchor.x,y:candidate.presentation.anchor.y}
            for (var index=mappings.length-1;index>=0;--index) {
                var map=mappings[index]
                anchor={x:map.left+anchor.x*map.scaleX,y:map.top+anchor.y*map.scaleY}
                if (!Number.isFinite(anchor.x)||!Number.isFinite(anchor.y)
                        || anchor.x<map.content.left||anchor.x>map.content.right
                        || anchor.y<map.content.top||anchor.y>map.content.bottom) {
                    anchor=null
                    break
                }
            }
            if (!anchor || !fragments.some(function(rect){return inside(anchor.x,anchor.y,rect)})) {
                var primary=fragments[0]
                fragments.forEach(function(rect) {
                    if (rect.y<primary.y||rect.y===primary.y&&rect.x<primary.x) primary=rect
                })
                anchor={x:primary.x+primary.width/2,y:primary.y+primary.height/2}
            }
            result.presentation = {anchor:anchor,fragments:fragments}
        }
        return result
    }
    function supported(candidate) {
        return ["link","button","input","select","textarea","contenteditable","editor-focus","label-control","aria","focusable","frame"].indexOf(candidate.kind) >= 0
    }
    function viewStamp() {
        return view ? {zoom:Number(view.zoomFactor),width:Number(view.width),height:Number(view.height)} : null
    }
    function sameViewStamp(stamp) {
        var current=viewStamp()
        return !!current && !!stamp && current.zoom===stamp.zoom && current.width===stamp.width && current.height===stamp.height
    }
    // The native capture owns policy, grants and original per-world inventories.
    // This orchestration carries facts; it does not assign semantic labels.
    function collectInteraction(selectedView, options, adapter, request, callback) {
        var token=epoch, collectionEpoch=browserWindow.hintCollectionEpoch
        var stamp={zoom:Number(selectedView.zoomFactor),width:Number(selectedView.width),height:Number(selectedView.height)}
        var worlds=Object.create(null), stages=0, finished=false, modal=false, checkpoint="attach"
        function current() {
            return token===epoch && collectionEpoch===browserWindow.hintCollectionEpoch
                    && view===selectedView
        }
        function finish(value) {
            if(finished||!current())return
            finished=true
            if(!sameViewStamp(stamp))value={error:"frame-viewport-changed"}
            if(!value.error)geometryStamp=stamp
            callback(value,request)
        }
        function fail(value) {
            var cause=value&&(value.error||value.interaction_error)||""
            finish({error:"interaction-collection-"+checkpoint+"-unavailable",
                cause:cause,retryable:cause==="frame-timeout"||cause==="stale-frame-document"})
        }
        function decode(text) {
            try {return text?JSON.parse(text):null} catch(error) {return null}
        }
        function complete() {
            checkpoint="validation"
            adapter.validate(request,function(valid) {
                if(!current())return
                if(!valid){fail();return}
                checkpoint="geometry"
                var metadata=[], topViewport=null, inspected=0, truncated=false, revision=0
                var keys=Object.keys(worlds)
                for(var index=0;index<keys.length;index++) {
                    var owner=keys[index], world=worlds[owner]
                    if(!world.sealed){fail();return}
                    var geometry=decode(adapter.collectionGeometry(request,owner))
                    if(!geometry||!validSize(geometry.viewport)||!Array.isArray(geometry.mappings)){fail();return}
                    if(topViewport&&!sameSize(topViewport,geometry.viewport)){fail();return}
                    topViewport=geometry.viewport
                    metadata.push({owner:owner,annotation_json:world.policy.annotation_json,
                        annotation_batch:world.policy.annotation_batch,viewport:world.ordinary.viewport,
                        revision:world.policy.revision,geometry:geometry})
                    inspected+=world.ordinary.interaction_work.inspections+world.policy.interaction_work.inspections
                    truncated=truncated||world.ordinary.truncated||world.policy.truncated
                    revision=Math.min(Number.MAX_SAFE_INTEGER,revision+world.policy.revision)
                }
                if(!topViewport||inspected>50000){fail();return}
                checkpoint="normalization"
                var result=decode(adapter.finishCollection(request,!modal&&!scopedOwner))
                if(!result||!Array.isArray(result.candidates)||typeof result.annotation_json!=="string"){fail();return}
                finish({candidates:result.candidates,selector_revision:options.selectors.selector_revision,
                    revision:revision,truncated:truncated,inspected_nodes:inspected,
                    viewport:topViewport,interaction_worlds:metadata,interaction_annotation:result.annotation_json})
            })
        }
        function next() {
            if(!current())return
            var text=adapter.collectionWorkPlan(request)
            if(!text){complete();return}
            var grant=decode(text)
            if(!grant||++stages>4){fail();return}
            var owner=grant.owner, plan=decode(adapter.collectorPlanForOwner(request,owner))
            if(!plan){fail();return}
            var world=worlds[owner]
            checkpoint=grant.phase
            var refresh=world?Scripts.hintCollector(options.family,options.proxy,options.selectors,options.scope,
                options.includeFacets,Object.assign({},world.budget,grant.budget),plan,true):null
            var script=grant.phase==="ordinary"
                    ?Scripts.hintCollector(options.family,options.proxy,options.selectors,options.scope,
                        options.includeFacets,grant.budget,plan,true)
                    :world?Scripts.hintFinishInteraction(plan,world.ordinary.interaction_pending,grant.budget,
                        options.selectionCheck===true,refresh):""
            if(!script){fail();return}
            run(owner||rootOwner,script,function(value) {
                if(!current())return
                if(value&&value.error==="stale-interaction-collection") {
                    finish({error:"interaction-collection-stale"});return
                }
                if(!value||value.error||!value.interaction_work){fail(value);return}
                var work=value.interaction_work
                var metadata={nonce:grant.phase==="ordinary"?value.interaction_pending:value.annotation_batch,
                    revision:value.revision,selector_revision:value.selector_revision,
                    inspections:work.inspections,predicate_steps:work.predicate_steps,
                    truncated:value.truncated,viewport:value.viewport}
                if(grant.phase==="ordinary") {
                    checkpoint="ordinary-accept"
                    if(!adapter.acceptCollectionOrdinary(request,grant.stage,owner,
                            JSON.stringify(value.candidates),JSON.stringify(metadata))){fail();return}
                    worlds[owner]={ordinary:value,budget:grant.budget,sealed:false}
                    next()
                } else {
                    checkpoint="policy-seal"
                    metadata.annotation_bytes=work.annotation_bytes
                    adapter.verifyCollectionPolicy(request,grant.stage,owner,value.annotation_json,
                        JSON.stringify(metadata),function(valid) {
                        if(!current())return
                        if(!valid){
                            // A change can arrive after policy extraction but
                            // before the native seal. Keep the failed proof
                            // unusable, and let the existing bounded initial
                            // retry recapture it just as for an earlier change.
                            run(owner||rootOwner,Scripts.hintDirtyRevision(),function(state) {
                                if(!current())return
                                if(state&&!state.error&&state.dirty===true
                                        &&Number.isSafeInteger(state.revision)&&state.revision>0)
                                    finish({error:"interaction-collection-stale"})
                                else fail()
                            })
                            return
                        }
                        world.policy=value;world.sealed=true
                        next()
                    })
                }
            })
        }
        ensure(selectedView,function(attached) {
            if(!current())return
            if(attached.error){fail(attached);return}
            if(scopedOwner&&(options.pointer||options.scope)){fail();return}
            function bind(selectedOwner) {
                checkpoint="binding"
                adapter.bindCollection(request,rootOwner,selectedOwner,function(bound) {
                    if(!current())return
                    if(!bound){fail();return}
                    checkpoint="work-start"
                    if(!adapter.beginCollectionWork(request)){fail();return}
                    next()
                })
            }
            if(scopedOwner) {bind(scopedOwner);return}
            discover(rootOwner,0,false,false,function(found) {
                if(!current())return
                modal=!!(found&&!found.error&&found.foreign&&found.modal)
                var unsupported=found&&found.foreign&&(options.scope||options.pointer
                    ||options.family==="surfaces"||options.family==="regions")
                bind(found&&!found.error&&!unsupported?found.owner:"")
            })
        })
    }
    function collect(selectedView, script, options, callback) {
        ensure(selectedView, function(attached) {
            if (attached.error) { callback(attached); return }
            var token=epoch
            var stamp=viewStamp()
            var collectionEpoch=browserWindow.hintCollectionEpoch
            function finish(result) {
                if (token!==epoch || browserWindow.hintCollectionEpoch!==collectionEpoch) return
                if (!sameViewStamp(stamp)) {callback({error:"frame-viewport-changed"});return}
                geometryStamp=stamp
                callback(result)
            }
            function collectChild(owner, base, modal) {
                if (!validCollection(base, 50000, 5000)) { finish({error:"invalid-frame-collection"}); return }
                verifyChain(owner, function(proof) {
                    if (proof.error) { finish(proof); return }
                    var budget = {depth:Math.max(0,8-proof.entries.length),inspections:50000-base.inspected_nodes,candidates:5000-base.candidates.length}
                    run(owner, Scripts.hintCollector(options.family,options.proxy,options.selectors,"",false,budget), function(child) {
                        if (!child || child.error) { finish(child || {error:"frame-collection-failed"}); return }
                        if (!validCollection(child, budget.inspections, budget.candidates)) { finish({error:"invalid-frame-collection"}); return }
                        verifyChain(owner, function(finalProof) {
                            if (finalProof.error) { finish(finalProof); return }
                            if (!sameChainProof(proof, finalProof)) { finish({error:"frame-geometry-changed"}); return }
                            if (!collectionFitsProof(base, child, finalProof)) { finish({error:"frame-geometry-changed"}); return }
                            var rows = child.candidates.filter(supported).map(function(candidate) {return project(candidate,owner,finalProof.mappings)}).filter(Boolean)
                            var parentRows = modal ? [] : base.candidates
                            finish(Object.assign({},base,{candidates:parentRows.concat(rows),
                                selector_revision:child.selector_revision,
                                inspected_nodes:base.inspected_nodes+child.inspected_nodes,
                                truncated:base.truncated||child.truncated,
                                revision:Math.min(Number.MAX_SAFE_INTEGER,base.revision+child.revision)}))
                        })
                    })
                })
            }
            if (scopedOwner && (options.pointer || options.scope)) {callback({error:"unsupported-frame-action"});return}
            if (scopedOwner) {
                root("({candidates:[],inspected_nodes:0,truncated:false,revision:0,selector_revision:'',viewport:{width:innerWidth,height:innerHeight}})", function(base) {
                    collectChild(scopedOwner,base,true)
                })
                return
            }
            root(script, function(base) {
                if (!base || base.error) { finish(base); return }
                if (!validCollection(base, 50000, 5000)) { finish({error:"invalid-frame-collection"}); return }
                if (options.scope || options.pointer || options.family === "surfaces" || options.family === "regions") { finish(base); return }
                discover(rootOwner,0,false,false,function(found) {
                    // Passive focus loss never turns an ordinary top-level hint session into an error.
                    if (found.error || !found.foreign || found.owner === rootOwner) { finish(base); return }
                    collectChild(found.owner,base,found.modal)
                })
            })
        })
    }
    function fresh(candidate, script, callback) {
        var owner = String(candidate.frame_owner || "")
        if (!owner) return root(script,callback)
        if (!sameViewStamp(geometryStamp)) {callback({visible:false});return true}
        if (!sources[owner]) { callback({visible:false}); return true }
        verifyChain(owner,function(proof) {
            if (proof.error) { callback({visible:false}); return }
            run(owner,script,function(local) {
                if (!local || !local.visible || local.error) { callback({visible:false}); return }
                var projected = project(local,owner,proof.mappings)
                if (!projected) { callback({visible:false}); return }
                // The chosen target must be exposed through every parent iframe at this point.
                var point = {x:projected.geometry.x+projected.geometry.width/2,y:projected.geometry.y+projected.geometry.height/2}
                function hit(index) {
                    if (index === proof.entries.length) { callback(sameViewStamp(geometryStamp) ? projected : {visible:false}); return }
                    var entry = proof.entries[index], map = proof.mappings[index]
                    run(entry.parent,Scripts.hintFrameHost(entry.host,{width:map.viewportWidth,height:map.viewportHeight},point),function(result) {
                        if (!result || result.error || JSON.stringify(result.mapping) !== JSON.stringify(map)) { callback({visible:false}); return }
                        point = {x:(point.x-map.left)/map.scaleX,y:(point.y-map.top)/map.scaleY}
                        hit(index+1)
                    })
                }
                hit(0)
            })
        })
        return true
    }
    function action(candidate, script, callback, selectors) {
        var owner = String(candidate.frame_owner || "")
        if (!owner) return root(script,callback)
        // Revalidate the exact document, parent chain and target exposure immediately
        // before dispatch; a failure never grants a fallback click.
        fresh(candidate,selectors ? Scripts.hintFresh(candidate,selectors) : browserWindow.hintFreshScript(candidate),function(value) {
            if (!value.visible || value.text !== candidate.text || value.href !== candidate.href || value.kind !== candidate.kind
                    || ["x","y","width","height"].some(function(key) {return Math.abs(value.geometry[key]-candidate.geometry[key])>0.75})) { callback(false); return }
            var token = epoch
            broker.dispatch(owner,"(()=>{if(!document.hasFocus())return false;return ("+script+");})()",function(result) {
                if (token === epoch) callback(result)
            })
        })
        return true
    }
    function enter(candidate, selectors, callback) {
        if (!view || browserWindow.activeWebView() !== view) {callback({error:"frame-view-changed"});return}
        view.forceActiveFocus()
        var owner = String(candidate.frame_owner || "") || rootOwner
        var focusScript = Scripts.hintFocus(candidate,selectors)
        action(candidate,focusScript,function(focused) {
            if (focused !== true) { lastFailure="frame-focus-rejected";callback({error:lastFailure}); return }
            discover(owner,chain(owner) ? chain(owner).length : 0,true,false,function(found) {
                if (found.error || found.owner === owner) { lastFailure=String(found.error||"frame-entry-unavailable");callback({error:lastFailure}); return }
                scopedOwner = found.owner
                callback({ok:true})
            })
        },selectors)
    }
    function dirty(callback) {
        if (pollPending) return
        pollPending = true
        var owners = Object.keys(sources), pending = owners.length+1, dirty = false
        var token = epoch
        function completed(owner,value) {
            if(token!==epoch)return
            var revision=Number(value && value.revision || 0)
            dirty = dirty || !!(value && (value.error || value.dirty && polledRevisions[owner] !== revision))
            polledRevisions[owner]=revision
            if (--pending===0) {pollPending=false;callback({dirty:dirty,revision:-1})}
        }
        root(Scripts.hintDirtyRevision(),function(value){completed("root",value)})
        owners.forEach(function(owner) {run(owner,Scripts.hintDirtyRevision(),function(value){completed(owner,value)})})
    }
    function stop() {
        ++epoch
        Object.keys(sources).forEach(function(owner) {broker.run(owner,Scripts.hintStopTracking()+"",function(){})})
        if (view) browserWindow.runBrowserScript(view,"(()=>{window.__ferric_hintFrameHosts=undefined;return true;})()",function(){})
        broker.clear()
        sources = ({})
        geometryStamp=null
        polledRevisions = ({})
        pollPending = false
        rootOwner = ""
        scopedOwner = ""
        view = null
    }
}
