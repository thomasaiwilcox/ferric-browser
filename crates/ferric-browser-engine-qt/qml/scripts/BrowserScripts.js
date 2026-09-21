.pragma library

// Browser-owned page scripts are versioned resources. The build manifest hashes
// this file with the QML module, so script changes are explicit and auditable.
var VERSION = "1"

function boundedCount(value) {
    return Math.max(1, Math.min(9999, Number(value) || 1))
}

function scroll(kind, direction, half, count) {
    return "(function(){"
        + "var kind=" + JSON.stringify(String(kind)) + ";"
        + "var direction=" + JSON.stringify(String(direction)) + ";"
        + "var half=" + (half ? "true" : "false") + ";"
        + "var count=" + String(boundedCount(count)) + ";"
        + "var root=document.scrollingElement||document.documentElement;"
        + "var target=root;var node=document.activeElement;"
        + "while(node&&node!==document.body&&node!==document.documentElement){"
        + "var style=window.getComputedStyle(node);"
        + "var vertical=(style.overflowY==='auto'||style.overflowY==='scroll')&&node.scrollHeight>node.clientHeight;"
        + "var horizontal=(style.overflowX==='auto'||style.overflowX==='scroll')&&node.scrollWidth>node.clientWidth;"
        + "if((direction==='up'||direction==='down')?vertical:horizontal){target=node;break;}"
        + "node=node.parentElement;}"
        + "var width=Math.max(window.innerWidth||0,target.clientWidth||0);"
        + "var height=Math.max(window.innerHeight||0,target.clientHeight||0);"
        + "if(kind==='scroll-to'){"
        + "var top=direction==='top'?0:Math.max(0,target.scrollHeight-height);"
        + "var left=direction==='top'?0:Math.max(0,target.scrollWidth-width);"
        + "target.scrollTo(left,top);"
        + "}else{"
        + "var step=kind==='scroll-page'?(half?0.5:0.9):0.15;"
        + "var dx=(direction==='left'?-1:direction==='right'?1:0)*Math.max(40,width*step)*count;"
        + "var dy=(direction==='up'?-1:direction==='down'?1:0)*Math.max(40,height*step)*count;"
        + "target.scrollBy(dx,dy);}"
        + "return true;})()"
}

function selection() {
    return "(function(){var e=document.activeElement;"
        + "if(e&&e.matches('input[type=password],textarea[data-password],*[aria-multiline=true][data-password]'))"
        + "return {error:'password fields are not copied'};"
        + "var s=window.getSelection(),t=s?String(s.toString()):'';"
        + "if(t.length>1048576)return {error:'selection is too large'};"
        + "return {text:t};})()"
}

function editor() {
    return "(function(){var e=document.activeElement;if(!e)return {error:'no focused control'};"
        + "if(e.matches('input[type=password],textarea[data-password],*[aria-multiline=true][data-password]'))"
        + "return {error:'password fields are not editable externally'};"
        + "if(e.matches('textarea,input[type=text],input[type=search],input[type=email],input[type=url]'))"
        + "return {ok:true,text:String(e.value||'')};"
        + "if(e.isContentEditable&&e.contentEditable==='plaintext-only')"
        + "return {ok:true,text:String(e.innerText||e.textContent||'')};"
        + "return {error:'focused control is not a supported plain-text editor'};})()"
}

function editorApply(original, updated) {
    return "(function(){var e=document.activeElement;if(!e)return {error:'no focused control'};"
        + "if(e.matches('input[type=password],textarea[data-password],*[aria-multiline=true][data-password]'))"
        + "return {error:'password fields are not editable externally'};"
        + "var current=e.matches('textarea,input[type=text],input[type=search],input[type=email],input[type=url]')?String(e.value||''):e.isContentEditable&&e.contentEditable==='plaintext-only'?String(e.innerText||e.textContent||''):null;"
        + "if(current===null)return {error:'focused control is not a supported plain-text editor'};"
        + "if(current!==" + JSON.stringify(original) + ")return {error:'field changed while editor was open'};"
        + "var next=" + JSON.stringify(updated) + ";if(e.matches('textarea,input[type=text],input[type=search],input[type=email],input[type=url]'))e.value=next;else e.textContent=next;"
        + "e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));return {ok:true};})()"
}

function caret(operation, selecting) {
    var fields = String(operation).split("\t")
    if (fields[0] === "select") {
        var next = fields[1] === "toggle" ? !selecting : fields[1] === "on"
        return "(function(){return {ok:true,selecting:" + (next ? "true" : "false") + "};})()"
    }
    if (fields.length !== 3 || fields[0] !== "move") {
        return "(function(){return {error:'invalid caret operation'};})()"
    }
    var movement = {
        left: { direction: "backward", granularity: "character" },
        right: { direction: "forward", granularity: "character" },
        up: { direction: "backward", granularity: "line" },
        down: { direction: "forward", granularity: "line" },
        "word-prev": { direction: "backward", granularity: "word" },
        "word-next": { direction: "forward", granularity: "word" },
        "line-start": { direction: "backward", granularity: "lineboundary" },
        "line-end": { direction: "forward", granularity: "lineboundary" }
    }[fields[1]]
    var count = Number(fields[2])
    if (!movement || !Number.isInteger(count) || count < 1 || count > 9999) {
        return "(function(){return {error:'invalid caret movement'};})()"
    }
    return "(function(){var s=window.getSelection();if(!s)return {error:'selection API unavailable'};"
        + "if(typeof s.modify!=='function')return {error:'caret movement API unavailable'};"
        + "if(s.rangeCount===0){var w=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT),n=w.nextNode();"
        + "if(!n)return {error:'document has no text'};var r=document.createRange();r.setStart(n,0);r.collapse(true);s.removeAllRanges();s.addRange(r);}"
        + "for(var i=0;i<" + count + ";i++)s.modify(" + (selecting ? "'extend'" : "'move'") + ","
        + JSON.stringify(movement.direction) + "," + JSON.stringify(movement.granularity) + ");"
        + "return {ok:true};})()"
}

function downloadLink(url) {
    return "(function(){var a=document.createElement('a');a.href="
        + JSON.stringify(String(url))
        + ";a.download='';a.rel='noreferrer';document.body.appendChild(a);a.click();a.remove();return true;})()"
}

function clearSiteData() {
    return "(function(){"
        + "var result={pending:true,local_storage:'unavailable',cache_storage:'unavailable',"
        + "service_workers:'unavailable',cookies:'page-visible-only',http_cache:'profile-wide-only'};"
        + "try{localStorage.clear();result.local_storage='cleared';}catch(error){"
        + "result.local_storage='unavailable';}"
        + "try{var names=document.cookie?document.cookie.split(';'):[];"
        + "for(var i=0;i<names.length;i++){var name=names[i].split('=')[0].trim();"
        + "if(name)document.cookie=name+'=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';}"
        + "result.cookies='page-visible-only';}catch(error){result.cookies='unavailable';}"
        + "var jobs=[];"
        + "if(self.caches&&self.caches.keys){result.cache_storage='pending';"
        + "jobs.push(self.caches.keys().then(function(keys){return Promise.all(keys.map(function(key){"
        + "return self.caches.delete(key);}));}).then(function(){result.cache_storage='cleared';}"
        + ").catch(function(){result.cache_storage='unavailable';}));}"
        + "if(navigator.serviceWorker&&navigator.serviceWorker.getRegistrations){"
        + "result.service_workers='pending';jobs.push(navigator.serviceWorker.getRegistrations().then("
        + "function(registrations){return Promise.all(registrations.map(function(registration){"
        + "return registration.unregister();}));}).then(function(){result.service_workers='cleared';}"
        + ").catch(function(){result.service_workers='unavailable';}));}"
        + "Promise.all(jobs).then(function(){result.pending=false;"
        + "window.__ferric_browserSiteDataClearResult=JSON.stringify(result);});"
        + "window.__ferric_browserSiteDataClearResult=JSON.stringify(result);"
        + "return window.__ferric_browserSiteDataClearResult;})()"
}
