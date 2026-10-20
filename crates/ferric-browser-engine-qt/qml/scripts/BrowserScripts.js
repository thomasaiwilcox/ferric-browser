.pragma library
.import "PageInteractionScripts.js" as PageInteractionScripts

// Browser-owned page scripts are versioned resources. The build manifest hashes
// this file with the QML module, so script changes are explicit and auditable.
var VERSION = "27"

// Page userscripts are trusted local content, but their failure boundary is
// browser policy. Keeping the wrappers here prevents QML from assembling page
// code and makes both execution paths use one reviewed contract.
function pageUserscriptRun(source) {
    return "(function(){try{" + String(source)
        + "\n}catch(error){return false;}return true;})()"
}

function pageUserscriptInstall(source) {
    return "(function(){try{" + String(source)
        + "\n}catch(error){}})()"
}

function scrollPosition() {
    return "(function(){var root=document.scrollingElement||document.documentElement;"
        + "if(!root)return null;return {x:Math.max(0,root.scrollLeft||0),y:Math.max(0,root.scrollTop||0)};})()"
}

function restoreScrollPosition(x, y) {
    var left = Math.max(0, Number(x) || 0)
    var top = Math.max(0, Number(y) || 0)
    return "(function(){var root=document.scrollingElement||document.documentElement;"
        + "if(!root)return false;root.scrollTo(" + left + "," + top + ");return true;})()"
}

function cosmeticFilter(css) {
    return "(function(){try{"
        + "var id='ferric-browser-cosmetic-filter';"
        + "var old=document.getElementById(id);if(old)old.remove();"
        + "var style=document.createElement('style');style.id=id;"
        + "style.textContent=" + JSON.stringify(String(css)) + ";"
        + "(document.head||document.documentElement).appendChild(style);"
        + "}catch(error){return false;}return true;})()"
}

// Only numeric node identities, offsets and revision counters leave this probe.
function selectionMetadata() {
    return "(function(){var m=window.__ferric_selectionMetadata;if(!m)return {error:'selection-observer-unavailable'};return m();})()"
}

function selection(expectedRevision) {
    return "(function(){var m=window.__ferric_selectionMetadata;if(!m)return {error:'selection-observer-unavailable'};"
        + (expectedRevision === undefined ? "" : "if(m().revision!==" + JSON.stringify(expectedRevision) + ")return {error:'selection-changed'};")
        + "var e=document.activeElement;for(var d=0;e&&e.shadowRoot&&e.shadowRoot.activeElement&&d<16;d++)e=e.shadowRoot.activeElement;"
        + "if(e&&e.shadowRoot&&e.shadowRoot.activeElement)return {error:'shadow focus depth is unsupported'};"
        + "if(e&&e.matches('input[type=password],textarea[data-password],*[aria-multiline=true][data-password]'))"
        + "return {error:'password fields are not copied'};"
        + "var s=window.getSelection(),t='';"
        + "if(e&&/^(INPUT|TEXTAREA)$/.test(e.tagName)&&typeof e.selectionStart==='number')t=String(e.value||'').slice(e.selectionStart,e.selectionEnd);"
        + "else {if(e&&e.getRootNode&&e.getRootNode().getSelection)s=e.getRootNode().getSelection();"
        + "if(e&&e!==document.body&&e!==document.documentElement&&!e.isContentEditable&&!e.shadowRoot&&s&&s.anchorNode&&!e.contains(s.anchorNode))return {error:'focused selection is unsupported'};"
        + "if(s&&s.anchorNode){var root=s.anchorNode.getRootNode();if(root!==document&&root.host&&root.host.shadowRoot!==root)return {error:'closed-shadow selection is unsupported'};}"
        + "t=s?String(s.toString()):'';}"
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

function focusProbe() {
    return "(function(){var state=window.__ferric_browserReadFocusState?window.__ferric_browserReadFocusState():window.__ferric_browserFocusState;"
        + "return state?{sequence:Number(state.sequence)||0,context:Number(state.context)||0,editable:!!state.editable,"
        + "user_activated:!!state.user_activated,kind:String(state.kind||'unknown')}"
        + ":{sequence:0,editable:false,user_activated:false,kind:'unknown'};})()"
}

function historyTraversalProbe() {
    return "(function(){var state=window.__ferric_browserHistoryTraversal;"
        + "return state?{document:state.document,sequence:state.sequence,url:location.href}:null;})()"
}

function focusObserverSource() {
    return "(function(){"
        + "if(window.__ferric_browserFocusObserverInstalled)return;"
        + "window.__ferric_browserFocusObserverInstalled=true;"
        + "var selectionRevision=0,selectionIdentities=new WeakMap(),selectionNextIdentity=0;"
        + "function selectionId(node){if(!node)return 0;if(!selectionIdentities.has(node))selectionIdentities.set(node,++selectionNextIdentity);return selectionIdentities.get(node);}"
        + "window.__ferric_selectionMetadata=function(){var e=document.activeElement;for(var depth=0;e&&e.shadowRoot&&e.shadowRoot.activeElement&&depth<16;depth++)e=e.shadowRoot.activeElement;"
        + "var s=window.getSelection();if(e&&e.getRootNode&&e.getRootNode().getSelection)s=e.getRootNode().getSelection();"
        + "var offsets=e&&typeof e.selectionStart==='number'?[e.selectionStart,e.selectionEnd]:[selectionId(s&&s.anchorNode),s?s.anchorOffset:0,selectionId(s&&s.focusNode),s?s.focusOffset:0];"
        + "return {focused:document.hasFocus(),identity:selectionId(e),frame:!!e&&/^(IFRAME|FRAME)$/.test(e.tagName),revision:JSON.stringify([selectionRevision].concat(offsets))};};"
        + "function selectionChanged(){selectionRevision++;console.debug('__ferric_browser_selection_changed');}"
        + "document.addEventListener('selectionchange',selectionChanged,true);document.addEventListener('input',selectionChanged,true);"

        // This state lives in ApplicationWorld. Only real popstate events
        // distinguish traversal from a new pushState with the same URL.
        + "if(window===window.top){const documentId=window.crypto&&typeof window.crypto.randomUUID==='function'?window.crypto.randomUUID():String(performance.timeOrigin)+':'+String(Math.random())+':'+String(Math.random());"
        + "const historyState={document:documentId,sequence:0};"
        + "window.__ferric_browserHistoryTraversal=historyState;"
        + "window.addEventListener('popstate',function(event){if(event.isTrusted)historyState.sequence++;},true);}"
        + "var sequence=0;var userGestureUntil=0;var identities=new WeakMap();var nextIdentity=0;var watchedElement=null;var watchedParent=null;"
        + "var removalObserver=new MutationObserver(function(){window.__ferric_browserReadFocusState();});"
        + "function watch(element){var parent=element&&element.parentNode;if(element===watchedElement&&parent===watchedParent)return;removalObserver.disconnect();watchedElement=element;watchedParent=parent;var node=parent;for(var depth=0;node&&depth<64;depth++,node=node.parentNode||node.host){removalObserver.observe(node,{childList:true});}}"
        + "function classify(){"
        + "var element=document.activeElement;var depth=0;"
        + "while(element&&element.shadowRoot&&depth<16&&element.shadowRoot.activeElement){"
        + "element=element.shadowRoot.activeElement;depth++;}"
        + "var tag=element&&element.tagName?String(element.tagName).toLowerCase():'';"
        + "var type=element&&element.type?String(element.type).toLowerCase():'';"
        + "var excluded=['button','checkbox','file','hidden','image','radio','range','reset','submit'];"
        + "var editable=!!element&&(element.isContentEditable===true||tag==='textarea'||tag==='select'||"
        + "(tag==='input'&&excluded.indexOf(type)<0));"
        + "var kind=editable?(tag==='textarea'?'textarea':element.isContentEditable?'contenteditable':"
        + "tag==='input'&&type==='password'?'password':'input'):tag==='iframe'?'frame':'other';"
        + "var context=0;if(editable){if(!identities.has(element))identities.set(element,++nextIdentity);context=identities.get(element);}return {editable:editable,kind:kind,context:context,element:editable?element:null};}"
        + "function publish(userActivated){var state=classify();watch(state.element);"
        + "window.__ferric_browserFocusState={sequence:++sequence,context:state.context,editable:state.editable,user_activated:!!userActivated,kind:state.kind};console.debug('__ferric_browser_focus_changed');}"
        + "window.__ferric_browserReadFocusState=function(){var current=classify();watch(current.element);var old=window.__ferric_browserFocusState;if(!old||old.context!==current.context||old.editable!==current.editable)publish(false);return window.__ferric_browserFocusState;};"
        + "window.__ferric_browserAuthorizeExplicitFocus=function(){userGestureUntil=performance.now()+1500;};"
        + "function noteGesture(event){if(event.isTrusted!==false){userGestureUntil=performance.now()+1500;publish(true);}}"
        + "document.addEventListener('pointerdown',noteGesture,true);"
        + "document.addEventListener('keydown',function(event){if(event.key==='Tab')noteGesture(event);},true);"
        + "document.addEventListener('focusin',function(){publish(performance.now()<=userGestureUntil);},true);"
        + "document.addEventListener('focusout',function(){queueMicrotask(function(){publish(false);});},true);"
        + "document.addEventListener('input',function(){queueMicrotask(window.__ferric_browserReadFocusState);},true);"
        + "publish(false);})();"
}

function formStateProbe() {
    return "(function() {"
        + "var elements = document.querySelectorAll('input,textarea,select,[contenteditable=\"true\"]');"
        + "for (var i = 0; i < elements.length; ++i) {"
        + "var e = elements[i];"
        + "if (e.isContentEditable) return 'unknown';"
        + "if (e.tagName === 'INPUT' && (e.type === 'checkbox' || e.type === 'radio')"
        + " && e.checked !== e.defaultChecked) return 'dirty';"
        + "if (e.tagName === 'SELECT' && e.selectedIndex !== e.defaultSelectedIndex) return 'dirty';"
        + "if (e.tagName !== 'SELECT' && e.value !== e.defaultValue) return 'dirty';"
        + "} return 'safe'; })()"
}

function siteDataClearResult() {
    return "window.__ferric_browserSiteDataClearResult || ''"
}

function shutdownPageProbe() {
    return "(function() {"
        + "if (typeof window.onbeforeunload === 'function') return 'unknown';"
        + "var elements = document.querySelectorAll('input,textarea,select,[contenteditable=\"true\"]');"
        + "if (elements.length > 128) return 'unknown';"
        + "for (var i = 0; i < elements.length; ++i) {"
        + "var e = elements[i];"
        + "if (e.isContentEditable) return 'unknown';"
        + "if (e.tagName === 'INPUT' && (e.type === 'checkbox' || e.type === 'radio')"
        + " && e.checked !== e.defaultChecked) return 'dirty';"
        + "if (e.tagName === 'SELECT') {"
        + "if (e.options.length > 512) return 'unknown';"
        + "for (var j = 0; j < e.options.length; ++j)"
        + " if (e.options[j].selected !== e.options[j].defaultSelected) return 'dirty';"
        + "} else if (e.value !== e.defaultValue) return 'dirty';"
        + "} return 'clean'; })()"
}


// The imported resource is independently versioned and covered by the build
// manifest. Every compatibility entry point fails closed if that contract is
// unavailable or mismatched.
var PAGE_INTERACTION_VERSION = "7"

function interactionFailureScript(code) {
    return "(function(){return {ok:false,error:" + JSON.stringify(code) + "};})()"
}

function interactionScript(name, args) {
    var asset
    try {
        asset = PageInteractionScripts
    } catch (error) {
        return interactionFailureScript("script-resource-unavailable")
    }
    if (!asset || asset.VERSION !== PAGE_INTERACTION_VERSION)
        return interactionFailureScript("script-version-mismatch")
    var builder = asset[name]
    if (typeof builder !== "function")
        return interactionFailureScript("script-resource-unavailable")
    try {
        return builder.apply(null, args)
    } catch (error) {
        return interactionFailureScript("script-error")
    }
}

function scroll(kind, direction, half, count) {
    return interactionScript("scroll", [kind, direction, half, count])
}

function scrollTargetPolicy(policy) {
    return interactionScript("scrollTargetPolicy", [policy])
}

function scrollTargetStatus() {
    return interactionScript("scrollTargetStatus", [])
}

function hintCollector(family, proxyTargets, selectorSnapshot, scopeLeaseId, includeFacets, budget) {
    return interactionScript("hintCollector", [family, proxyTargets, selectorSnapshot, scopeLeaseId, includeFacets, budget])
}

function hintDirtyRevision() {
    return interactionScript("hintDirtyRevision", [])
}

function hintStopTracking(scopeLeaseId) {
    return interactionScript("hintStopTracking", [scopeLeaseId])
}

function hintAcquireScopeLease(request, candidate, selectorSnapshot) {
    return interactionScript("hintAcquireScopeLease", [request, candidate, selectorSnapshot])
}

function hintClearScopeLease(scopeLeaseId) {
    return interactionScript("hintClearScopeLease", [scopeLeaseId])
}

function spatialScopeGeometry(scopeLeaseId) {
    return interactionScript("spatialScopeGeometry", [scopeLeaseId])
}

function hintFresh(candidate, selectorSnapshot) {
    return interactionScript("hintFresh", [candidate, selectorSnapshot])
}

function hintPointerPreflight(candidate, viewWidth, viewHeight, selectorSnapshot) {
    return interactionScript("hintPointerPreflight", [candidate, viewWidth, viewHeight, selectorSnapshot])
}

function hintFocus(candidate, selectorSnapshot) {
    return interactionScript("hintFocus", [candidate, selectorSnapshot])
}

function hintClick(candidate, selectorSnapshot) {
    return interactionScript("hintClick", [candidate, selectorSnapshot])
}

function hintSetScrollTarget(elementId, framePath) {
    return interactionScript("hintSetScrollTarget", [elementId, framePath])
}

function hintFrameHost(lease, viewport, point) {
    return PageInteractionScripts.hintFrameHost(lease, viewport, point)
}
