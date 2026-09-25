.pragma library

// Browser-owned page scripts are versioned resources. The build manifest hashes
// this file with the QML module, so script changes are explicit and auditable.
var VERSION = "4"

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

function focusProbe() {
    return "(function(){var state=window.__ferric_browserFocusState;"
        + "return state?{sequence:Number(state.sequence)||0,editable:!!state.editable,"
        + "user_activated:!!state.user_activated,kind:String(state.kind||'unknown')}"
        + ":{sequence:0,editable:false,user_activated:false,kind:'unknown'};})()"
}

function focusObserverSource() {
    return "(function(){"
        + "if(window.__ferric_browserFocusObserverInstalled)return;"
        + "window.__ferric_browserFocusObserverInstalled=true;"
        + "var sequence=0;var userGestureUntil=0;"
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
        + "return {editable:editable,kind:kind};}"
        + "function publish(userActivated){var state=classify();"
        + "window.__ferric_browserFocusState={sequence:++sequence,editable:state.editable,user_activated:!!userActivated,kind:state.kind};}"
        + "window.__ferric_browserAuthorizeExplicitFocus=function(){userGestureUntil=performance.now()+1500;};"
        + "function noteGesture(event){if(event.isTrusted!==false){userGestureUntil=performance.now()+1500;publish(true);}}"
        + "document.addEventListener('pointerdown',noteGesture,true);"
        + "document.addEventListener('keydown',function(event){if(event.key==='Tab')noteGesture(event);},true);"
        + "document.addEventListener('focusin',function(){publish(performance.now()<=userGestureUntil);},true);"
        + "document.addEventListener('focusout',function(){setTimeout(function(){publish(false);},0);},true);"
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

function hintSelector(linksOnly) {
    if (linksOnly) {
        return "a[href],area[href],link[href],[role='link'][href]"
    }
    return "a,area,textarea,select,input:not([type='hidden']),button,frame,iframe,img,link,summary,"
        + "[contenteditable]:not([contenteditable='false']),[onclick],[onmousedown],"
        + "[role='link'],[role='option'],[role='button'],[role='tab'],[role='checkbox'],"
        + "[role='switch'],[role='menuitem'],[role='menuitemcheckbox'],"
        + "[role='menuitemradio'],[role='treeitem'],[aria-haspopup],[ng-click],[ngClick],"
        + "[data-ng-click],[x-ng-click],[tabindex]:not([tabindex='-1'])"
}

function hintCollector(linksOnly) {
    var selector = hintSelector(linksOnly)
    var linkSelector = hintSelector(true)
    return "(function(){"
        + "const out=[];const seen=new Set();const selector=" + JSON.stringify(selector) + ";"
        + "const linkSelector=" + JSON.stringify(linkSelector) + ";"
        + "const elements=new Map();window.__ferric_browserHintElements=elements;let nextElementId=1;"
        + "function add(el,ox,oy,framePath){if(out.length>=5000||seen.has(el))return;seen.add(el);"
        + "const s=getComputedStyle(el),r=el.getBoundingClientRect(),ownerView=el.ownerDocument&&el.ownerDocument.defaultView;"
        + "const vw=ownerView?ownerView.innerWidth:window.innerWidth,vh=ownerView?ownerView.innerHeight:window.innerHeight,x=r.x+ox,y=r.y+oy;"
        + "if(s.display==='none'||s.visibility==='hidden'||s.pointerEvents==='none'||Number(s.opacity)===0||r.width<=0||r.height<=0)return;"
        + "if(r.bottom<=0||r.right<=0||r.top>=vh||r.left>=vw||x+r.width<=0||y+r.height<=0||x>=window.innerWidth||y>=window.innerHeight)return;"
        + "let kind='aria';if(el.matches(linkSelector))kind='link';else if(el.matches('button,[role=button]'))kind='button';"
        + "else if(el.matches('input'))kind='input';else if(el.matches('select'))kind='select';"
        + "else if(el.matches('textarea'))kind='textarea';else if(el.isContentEditable)kind='contenteditable';"
        + "const text=String(el.getAttribute('aria-label')||el.getAttribute('title')||el.innerText||el.value||'').replace(/[\\n\\r]+/g,' ').trim().slice(0,512);"
        + "const href=kind==='link'?String(el.href||el.getAttribute('href')||''):null;"
        + "const elementId=nextElementId++;elements.set(elementId,{element:el,framePath:framePath});"
        + "out.push({element_id:elementId,kind:kind,frame_path:framePath,text:text,href:href,geometry:{x:x,y:y,width:r.width,height:r.height}});}"
        + "function visit(root,framePath,ox,oy,depth){if(out.length>=5000||depth>8)return;"
        + "root.querySelectorAll(selector).forEach(function(el){add(el,ox,oy,framePath);});if(out.length>=5000)return;"
        + "root.querySelectorAll('*').forEach(function(el){if(el.shadowRoot)visit(el.shadowRoot,framePath,ox,oy,depth);});"
        + "if(root.nodeType!==9)return;root.querySelectorAll('iframe,frame').forEach(function(frame,index){if(out.length>=5000||depth>=8)return;"
        + "const fs=getComputedStyle(frame),fr=frame.getBoundingClientRect(),ownerView=frame.ownerDocument&&frame.ownerDocument.defaultView;"
        + "const vw=ownerView?ownerView.innerWidth:window.innerWidth,vh=ownerView?ownerView.innerHeight:window.innerHeight,fx=fr.x+ox,fy=fr.y+oy;"
        + "if(fs.display==='none'||fs.visibility==='hidden'||fr.width<=0||fr.height<=0||fr.bottom<=0||fr.right<=0||fr.top>=vh||fr.left>=vw||fx+fr.width<=0||fy+fr.height<=0||fx>=window.innerWidth||fy>=window.innerHeight)return;"
        + "try{if(frame.contentDocument)visit(frame.contentDocument,framePath+'.'+index,ox+fr.x,oy+fr.y,depth+1);}catch(error){}});}"
        + "visit(document,'0',0,0,0);return {candidates:out};})()"
}

function hintFresh(candidate) {
    var selector = hintSelector(false)
    var linkSelector = hintSelector(true)
    return "(function(){const elementId=" + Number(candidate.element_id) + ",path=" + JSON.stringify(candidate.frame_path || "0") + ";"
        + "const selector=" + JSON.stringify(selector) + ",linkSelector=" + JSON.stringify(linkSelector) + ";"
        + "const elements=window.__ferric_browserHintElements,record=elements&&elements.get(elementId);"
        + "if(!record||record.framePath!==path||!record.element||!record.element.isConnected)return {visible:false,element_id:elementId};"
        + "const el=record.element;if(!el.matches(selector))return {visible:false,element_id:elementId};"
        + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView;if(!ownerView)return {visible:false,element_id:elementId};"
        + "const s=ownerView.getComputedStyle(el),r=el.getBoundingClientRect();let bx=0,by=0,view=ownerView;"
        + "try{while(view&&view!==window){const frame=view.frameElement;if(!frame)return {visible:false,element_id:elementId};const fr=frame.getBoundingClientRect();bx+=fr.x;by+=fr.y;view=frame.ownerDocument.defaultView;}}catch(error){return {visible:false,element_id:elementId};}"
        + "if(view!==window)return {visible:false,element_id:elementId};"
        + "let kind='aria';if(el.matches(linkSelector))kind='link';else if(el.matches('button,[role=button]'))kind='button';"
        + "else if(el.matches('input'))kind='input';else if(el.matches('select'))kind='select';else if(el.matches('textarea'))kind='textarea';else if(el.isContentEditable)kind='contenteditable';"
        + "return {visible:s.display!=='none'&&s.visibility!=='hidden'&&s.pointerEvents!=='none'&&Number(s.opacity)!==0&&r.width>0&&r.height>0&&r.bottom>0&&r.right>0&&r.top<ownerView.innerHeight&&r.left<ownerView.innerWidth&&r.bottom+by>0&&r.right+bx>0&&r.top+by<window.innerHeight&&r.left+bx<window.innerWidth,element_id:elementId,kind:kind,frame_path:path,"
        + "text:String(el.getAttribute('aria-label')||el.getAttribute('title')||el.innerText||el.value||'').replace(/[\\n\\r]+/g,' ').trim().slice(0,512),"
        + "href:kind==='link'?String(el.href||el.getAttribute('href')||''):null,geometry:{x:r.x+bx,y:r.y+by,width:r.width,height:r.height}};})()"
}

function hintFocus(elementId) {
    return "(function(){const elements=window.__ferric_browserHintElements,record=elements&&elements.get("
        + Number(elementId) + ");const el=record&&record.element;"
        + "if(!el||!el.isConnected||!el.matches('input,select,textarea,[contenteditable]:not([contenteditable=false])'))return false;"
        + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView;if(ownerView&&typeof ownerView.__ferric_browserAuthorizeExplicitFocus==='function')ownerView.__ferric_browserAuthorizeExplicitFocus();"
        + "el.focus();return true;})()"
}

function hintClick(elementId) {
    var selector = hintSelector(false)
    return "(function(){const selector=" + JSON.stringify(selector) + ",elements=window.__ferric_browserHintElements,record=elements&&elements.get("
        + Number(elementId) + ");const el=record&&record.element;"
        + "if(!el||!el.isConnected||!el.matches(selector)||typeof el.click!=='function')return false;el.click();return true;})()"
}
