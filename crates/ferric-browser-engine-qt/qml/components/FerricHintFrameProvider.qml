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
        run(parent, Scripts.hintFrameHost(""), function(host) {
            if (!host || host.error || host.none) {
                callback(host && host.error ? host : {owner:parent,foreign:foreign,modal:modal})
                return
            }
            if (depth >= 8) { callback({error:"frame-depth-limit"}); return }
            broker.focusedChild(parent, function(child) {
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
        var entries = chain(owner)
        if (!entries || !entries.length) { callback({error:"frame-lease-unavailable"}); return }
        var mappings = []
        function next(index) {
            if (index === entries.length) { callback({mappings:mappings,entries:entries}); return }
            var entry = entries[index]
            viewport(entry.owner, function(size) {
                if (!size || size.error || !size.focused) { callback({error:"frame-focus-lost"}); return }
                run(entry.parent, Scripts.hintFrameHost(entry.host, size), function(host) {
                    if (!host || host.error || !host.mapping) { callback(host || {error:"frame-unavailable"}); return }
                    broker.focusedChild(entry.parent, function(child) {
                        if (!child || child.owner !== entry.owner) { callback({error:"frame-association-changed"}); return }
                        // Recheck the parent after the asynchronous native scan.
                        run(entry.parent, Scripts.hintFrameHost(entry.host, size), function(finalHost) {
                            if (!finalHost || finalHost.error || !finalHost.mapping) { callback({error:"frame-association-changed"}); return }
                            mappings.push(finalHost.mapping)
                            next(index + 1)
                        })
                    })
                })
            })
        }
        next(0)
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
        if (!candidate || !candidate.geometry) return null
        var geometry = rectThrough(candidate.geometry, mappings)
        if (!geometry) return null
        var result = Object.assign({}, candidate, {geometry:geometry,frame_owner:owner})
        if (candidate.presentation) {
            var fragments = candidate.presentation.fragments.map(function(rect) {return rectThrough(rect,mappings)}).filter(Boolean)
            if (!fragments.length) return null
            result.presentation = {anchor:{x:fragments[0].x,y:fragments[0].y},fragments:fragments}
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
    function collect(selectedView, script, options, callback) {
        ensure(selectedView, function(attached) {
            if (attached.error) { callback(attached); return }
            var stamp=viewStamp()
            var collectionEpoch=browserWindow.hintCollectionEpoch
            function finish(result) {
                if (browserWindow.hintCollectionEpoch!==collectionEpoch) return
                if (!sameViewStamp(stamp)) {callback({error:"frame-viewport-changed"});return}
                geometryStamp=stamp
                callback(result)
            }
            function collectChild(owner, base, modal) {
                verifyChain(owner, function(proof) {
                    if (proof.error) { callback(proof); return }
                    var budget = {depth:Math.max(0,8-proof.entries.length),inspections:Math.max(0,50000-Number(base.inspected_nodes||0)),candidates:Math.max(0,5000-(base.candidates||[]).length)}
                    run(owner, Scripts.hintCollector(options.family,options.proxy,options.selectors,"",false,budget), function(child) {
                        if (!child || child.error) { callback(child || {error:"frame-collection-failed"}); return }
                        // Projection is captured after collection, not before a possibly slow DOM pass.
                        verifyChain(owner, function(finalProof) {
                            if (finalProof.error) { callback(finalProof); return }
                            var rows = (child.candidates||[]).filter(supported).map(function(candidate) {return project(candidate,owner,finalProof.mappings)}).filter(Boolean)
                            var parentRows = modal ? [] : (base.candidates||[])
                            finish(Object.assign({},base,{candidates:parentRows.concat(rows),
                                selector_revision:child.selector_revision,
                                inspected_nodes:Number(base.inspected_nodes||0)+Number(child.inspected_nodes||0),
                                truncated:!!base.truncated||!!child.truncated,
                                revision:Number(base.revision||0)+Number(child.revision||0)}))
                        })
                    })
                })
            }
            if (scopedOwner && (options.pointer || options.scope)) {callback({error:"unsupported-frame-action"});return}
            if (scopedOwner) {
                root("({viewport:{width:innerWidth,height:innerHeight}})", function(base) {
                    collectChild(scopedOwner,base,true)
                })
                return
            }
            root(script, function(base) {
                if (!base || base.error || options.scope || options.pointer || options.family === "surfaces" || options.family === "regions") { finish(base); return }
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
