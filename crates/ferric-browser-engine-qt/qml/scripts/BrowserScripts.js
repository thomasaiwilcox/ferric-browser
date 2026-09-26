.pragma library

// Browser-owned page scripts are versioned resources. The build manifest hashes
// this file with the QML module, so script changes are explicit and auditable.
var VERSION = "9"

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
        + "var target=(window.__ferric_browserScrollTarget&&window.__ferric_browserScrollTarget.isConnected)?window.__ferric_browserScrollTarget:root;var node=target===root?document.activeElement:null;"
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

function hintSelector(family) {
    if (family === true || family === "links") {
        return "a[href],area[href],link[href],[role='link'][href]"
    }
    if (family === "inputs")
        return "textarea,select,input:not([type='hidden']),[contenteditable]:not([contenteditable='false'])"
    if (family === "buttons")
        return "button,[role='button'],[role='option'],[role='tab'],[role='checkbox'],[role='switch'],[role^='menuitem']"
    if (family === "images")
        return "img"
    if (family === "media")
        return "audio,video"
    if (family === "scrollables")
        return "*"
    return "a,area,textarea,select,input:not([type='hidden']),button,frame,iframe,img,audio,video,link,summary,"
        + "[contenteditable]:not([contenteditable='false']),[onclick],[onmousedown],"
        + "[role='link'],[role='option'],[role='button'],[role='tab'],[role='checkbox'],"
        + "[role='switch'],[role='menuitem'],[role='menuitemcheckbox'],"
        + "[role='menuitemradio'],[role='treeitem'],[aria-haspopup],[ng-click],[ngClick],"
        + "[data-ng-click],[x-ng-click],[tabindex]:not([tabindex='-1'])"
}

function hintCollector(family) {
    var selector = hintSelector(family)
    var linkSelector = hintSelector(true)
    return "(function(){"
        + "const out=[];const seen=new Set();const activeIds=new Set();const selector=" + JSON.stringify(selector) + ";"
        + "const linkSelector=" + JSON.stringify(linkSelector) + ";"
        + "const family=" + JSON.stringify(family || "all") + ";"
        + "let state=window.__ferric_browserHintState;if(!state){state={weak:new WeakMap(),elements:new Map(),next:1,revision:1,dirty:true,roots:new WeakSet(),observers:[],windows:[],resize:null};window.__ferric_browserHintState=state;}if(!state.windows)state.windows=[];const elements=state.elements;window.__ferric_browserHintElements=elements;"
        + "function dirty(){state.dirty=true;state.revision=Math.min(Number.MAX_SAFE_INTEGER,state.revision+1);}if(!state.tracking){state.tracking=true;state.dirtyHandler=dirty;if(typeof ResizeObserver==='function')state.resize=new ResizeObserver(dirty);}"
        + "function track(root){if(state.roots.has(root))return;state.roots.add(root);try{const observer=new MutationObserver(dirty);observer.observe(root,{subtree:true,childList:true,attributes:true,characterData:true});state.observers.push(observer);}catch(error){}const doc=root.nodeType===9?root:root.ownerDocument,view=doc&&doc.defaultView;if(view&&state.windows.indexOf(view)<0){try{view.addEventListener('scroll',dirty,true);view.addEventListener('resize',dirty,true);state.windows.push(view);}catch(error){}}}"
        + "function parentOf(node){if(!node)return null;const root=node.getRootNode&&node.getRootNode();return node.parentElement||(root&&root.host)||null;}"
        + "function linkedSurface(el,r){for(let node=parentOf(el);node;node=parentOf(node))if(node.matches&&node.matches(linkSelector))return node;const doc=el.ownerDocument,points=[[r.left+r.width/2,r.top+r.height/2],[r.left+Math.min(4,r.width/2),r.top+Math.min(4,r.height/2)],[r.right-Math.min(4,r.width/2),r.bottom-Math.min(4,r.height/2)]];for(const point of points)for(const hit of doc.elementsFromPoint(point[0],point[1]))for(let node=hit;node;node=parentOf(node))if(node.matches&&node.matches(linkSelector)){const q=node.getBoundingClientRect();if(q.left<=r.left+1&&q.top<=r.top+1&&q.right>=r.right-1&&q.bottom>=r.bottom-1)return node;}return null;}"
        + "function inert(el){for(let node=el;node;node=parentOf(node))if(node.inert||(node.hasAttribute&&node.hasAttribute('inert')))return true;return false;}"
        + "function composedContains(parent,node){for(let current=node;current;){if(current===parent)return true;const root=current.getRootNode&&current.getRootNode();current=current.parentElement||(root&&root.host)||null;}return false;}"
        + "function hit(el,r){const doc=el.ownerDocument,isLink=el.matches&&el.matches(linkSelector),href=isLink?String(el.href||el.getAttribute('href')||''):'';const points=[[r.left+r.width/2,r.top+r.height/2],[r.left+Math.min(4,r.width/2),r.top+Math.min(4,r.height/2)],[r.right-Math.min(4,r.width/2),r.bottom-Math.min(4,r.height/2)]];return points.some(function(p){let node=doc.elementFromPoint(p[0],p[1]);while(node&&node.shadowRoot){const inner=node.shadowRoot.elementFromPoint(p[0],p[1]);if(!inner||inner===node)break;node=inner;}if(composedContains(el,node))return true;if(!isLink)return false;for(let current=node;current;current=parentOf(current))if(current!==el&&current.matches&&current.matches(linkSelector)&&String(current.href||current.getAttribute('href')||'')===href)return true;return false;});}"
        + "function hasPreferredLink(el,r){if(!el.matches(linkSelector))return false;const scope=parentOf(el);if(!scope||!scope.querySelectorAll)return false;const href=String(el.href||el.getAttribute('href')||''),area=r.width*r.height;for(const candidate of scope.querySelectorAll(linkSelector)){if(candidate===el||String(candidate.href||candidate.getAttribute('href')||'')!==href)continue;const q=candidate.getBoundingClientRect(),view=candidate.ownerDocument&&candidate.ownerDocument.defaultView,s=view&&view.getComputedStyle(candidate);if(q.width>0&&q.height>0&&q.width*q.height+1<area&&s&&s.display!=='none'&&s.visibility!=='hidden'&&s.pointerEvents!=='none'&&Number(s.opacity)!==0&&hit(candidate,q))return true;}return false;}"
        + "function clipped(el,r,view){let left=Math.max(0,r.left),top=Math.max(0,r.top),right=Math.min(view.innerWidth,r.right),bottom=Math.min(view.innerHeight,r.bottom);for(let p=parentOf(el);p&&right>left&&bottom>top;p=parentOf(p)){const pv=p.ownerDocument&&p.ownerDocument.defaultView||view,s=pv.getComputedStyle(p);if(/(auto|scroll|hidden|clip)/.test(s.overflow+s.overflowX+s.overflowY)){const q=p.getBoundingClientRect();left=Math.max(left,q.left);top=Math.max(top,q.top);right=Math.min(right,q.right);bottom=Math.min(bottom,q.bottom);}}return {left:left,top:top,right:right,bottom:bottom,width:Math.max(0,right-left),height:Math.max(0,bottom-top)};}"
        + "function intersect(a,b){const left=Math.max(a.left,b.left),top=Math.max(a.top,b.top),right=Math.min(a.right,b.right),bottom=Math.min(a.bottom,b.bottom);return {left:left,top:top,right:right,bottom:bottom,width:Math.max(0,right-left),height:Math.max(0,bottom-top)};}"
        + "function add(el,ox,oy,framePath,topClip){if(out.length>=5000||seen.has(el))return;seen.add(el);"
        + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView;if(!ownerView)return;const s=ownerView.getComputedStyle(el),r=clipped(el,el.getBoundingClientRect(),ownerView);const vw=ownerView.innerWidth,vh=ownerView.innerHeight,topRect=intersect({left:r.left+ox,top:r.top+oy,right:r.right+ox,bottom:r.bottom+oy},topClip),x=topRect.left,y=topRect.top;"
        + "const scrollable=(el.scrollHeight>el.clientHeight||el.scrollWidth>el.clientWidth)&&/(auto|scroll)/.test(s.overflow+s.overflowX+s.overflowY);if(family==='scrollables'&&!scrollable)return;"
        + "if(s.display==='none'||s.visibility==='hidden'||s.pointerEvents==='none'||Number(s.opacity)===0||el.disabled||el.getAttribute('aria-disabled')==='true'||inert(el)||r.width<=0||r.height<=0||topRect.width<=0||topRect.height<=0||hasPreferredLink(el,r)||!hit(el,r))return;"
        + "if(r.bottom<=0||r.right<=0||r.top>=vh||r.left>=vw||topRect.right<=0||topRect.bottom<=0||x>=window.innerWidth||y>=window.innerHeight)return;"
        + "let kind='aria';if(el.matches(linkSelector))kind='link';else if(el.matches('button,[role=button]'))kind='button';"
        + "else if(el.matches('input'))kind='input';else if(el.matches('select'))kind='select';"
        + "else if(el.matches('textarea'))kind='textarea';else if(el.isContentEditable)kind='contenteditable';else if(el.matches('img'))kind='image';else if(el.matches('audio,video'))kind='media';else if(scrollable)kind='scrollable';"
        + "if(family==='all'&&(kind==='image'||kind==='media')&&linkedSurface(el,r))return;"
        + "const text=String(el.getAttribute('aria-label')||el.getAttribute('title')||el.innerText||el.value||'').replace(/[\\u0000-\\u001f\\u007f-\\u009f]+/g,' ').replace(/\\s+/g,' ').trim().slice(0,512);"
        + "let href=kind==='link'?String(el.href||el.getAttribute('href')||''):(kind==='image'||kind==='media'?String(el.currentSrc||el.src||''):null);if(href===''||href&&href.length>8192||href&&/[\\u0000-\\u001f\\u007f-\\u009f]/.test(href))href=null;if(kind==='link'&&href===null)kind='aria';"
        + "let elementId=state.weak.get(el);if(!elementId){elementId=state.next++;state.weak.set(el,elementId);}elements.set(elementId,{element:el,framePath:framePath});activeIds.add(elementId);if(state.resize)try{state.resize.observe(el);}catch(error){}"
        + "out.push({element_id:elementId,kind:kind,frame_path:framePath,text:text,href:href,geometry:{x:x,y:y,width:topRect.width,height:topRect.height}});}"
        + "function visit(root,framePath,ox,oy,depth,topClip){if(out.length>=5000||depth>8)return;track(root);"
        + "root.querySelectorAll(selector).forEach(function(el){add(el,ox,oy,framePath,topClip);});if(out.length>=5000)return;"
        + "root.querySelectorAll('*').forEach(function(el){if(family==='all'){const view=el.ownerDocument&&el.ownerDocument.defaultView,s=view&&view.getComputedStyle(el);if(s&&(el.scrollHeight>el.clientHeight||el.scrollWidth>el.clientWidth)&&/(auto|scroll)/.test(s.overflow+s.overflowX+s.overflowY))add(el,ox,oy,framePath,topClip);}if(el.shadowRoot)visit(el.shadowRoot,framePath,ox,oy,depth+1,topClip);});"
        + "if(root.nodeType!==9)return;root.querySelectorAll('iframe,frame').forEach(function(frame,index){if(out.length>=5000||depth>=8)return;"
        + "const fs=getComputedStyle(frame),raw=frame.getBoundingClientRect(),fr=clipped(frame,raw,frame.ownerDocument.defaultView),ownerView=frame.ownerDocument&&frame.ownerDocument.defaultView;"
        + "const vw=ownerView?ownerView.innerWidth:window.innerWidth,vh=ownerView?ownerView.innerHeight:window.innerHeight,frameClip=intersect({left:fr.left+ox,top:fr.top+oy,right:fr.right+ox,bottom:fr.bottom+oy},topClip);"
        + "if(fs.display==='none'||fs.visibility==='hidden'||fs.pointerEvents==='none'||Number(fs.opacity)===0||inert(frame)||fr.width<=0||fr.height<=0||frameClip.width<=0||frameClip.height<=0||fr.bottom<=0||fr.right<=0||fr.top>=vh||fr.left>=vw||!hit(frame,fr))return;"
        + "try{if(frame.contentDocument)visit(frame.contentDocument,framePath+'.'+index,ox+raw.x,oy+raw.y,depth+1,frameClip);}catch(error){}});}"
        + "visit(document,'0',0,0,0,{left:0,top:0,right:innerWidth,bottom:innerHeight});for(const id of Array.from(elements.keys()))if(!activeIds.has(id)){const record=elements.get(id);if(state.resize&&record)try{state.resize.unobserve(record.element);}catch(error){}elements.delete(id);}state.dirty=false;return {candidates:out,revision:state.revision,viewport:{width:innerWidth,height:innerHeight}};})()"
}

function hintDirtyRevision() {
    return "(function(){const s=window.__ferric_browserHintState;return s&&s.tracking?{dirty:s.dirty,revision:s.revision}:{dirty:false,revision:0};})()"
}

function hintStopTracking() {
    return "(function(){const s=window.__ferric_browserHintState;if(!s)return true;if(s.dirtyHandler)for(const view of s.windows||[])try{view.removeEventListener('scroll',s.dirtyHandler,true);view.removeEventListener('resize',s.dirtyHandler,true);}catch(error){}for(const o of s.observers)o.disconnect();if(s.resize)s.resize.disconnect();s.observers=[];s.windows=[];s.roots=new WeakSet();s.tracking=false;s.dirty=false;return true;})()"
}

function hintFresh(candidate) {
    var selector = hintSelector("all")
    var linkSelector = hintSelector(true)
    return "(function(){const elementId=" + Number(candidate.element_id) + ",path=" + JSON.stringify(candidate.frame_path || "0") + ";"
        + "const selector=" + JSON.stringify(selector) + ",linkSelector=" + JSON.stringify(linkSelector) + ";"
        + "const state=window.__ferric_browserHintState,elements=state&&state.elements,record=elements&&elements.get(elementId);"
        + "if(!record||record.framePath!==path||!record.element||!record.element.isConnected)return {visible:false,element_id:elementId};"
        + "const el=record.element;const maybeScrollable=el.scrollHeight>el.clientHeight||el.scrollWidth>el.clientWidth;if(!el.matches(selector)&&!maybeScrollable)return {visible:false,element_id:elementId};"
        + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView;if(!ownerView)return {visible:false,element_id:elementId};"
        + "function parentOf(node){if(!node)return null;const root=node.getRootNode&&node.getRootNode();return node.parentElement||(root&&root.host)||null;}"
        + "function inert(node){for(let p=node;p;p=parentOf(p))if(p.inert||(p.hasAttribute&&p.hasAttribute('inert')))return true;return false;}"
        + "function contains(parent,node){for(let current=node;current;current=parentOf(current))if(current===parent)return true;return false;}"
        + "function clipped(node,rect,view){let left=Math.max(0,rect.left),top=Math.max(0,rect.top),right=Math.min(view.innerWidth,rect.right),bottom=Math.min(view.innerHeight,rect.bottom);for(let p=parentOf(node);p&&right>left&&bottom>top;p=parentOf(p)){const pv=p.ownerDocument&&p.ownerDocument.defaultView||view,ps=pv.getComputedStyle(p);if(/(auto|scroll|hidden|clip)/.test(ps.overflow+ps.overflowX+ps.overflowY)){const q=p.getBoundingClientRect();left=Math.max(left,q.left);top=Math.max(top,q.top);right=Math.min(right,q.right);bottom=Math.min(bottom,q.bottom);}}return {left:left,top:top,right:right,bottom:bottom,width:Math.max(0,right-left),height:Math.max(0,bottom-top)};}"
        + "function intersect(a,b){const left=Math.max(a.left,b.left),top=Math.max(a.top,b.top),right=Math.min(a.right,b.right),bottom=Math.min(a.bottom,b.bottom);return {left:left,top:top,right:right,bottom:bottom,width:Math.max(0,right-left),height:Math.max(0,bottom-top)};}"
        + "function hit(node,rect){const doc=node.ownerDocument,isLink=node.matches&&node.matches(linkSelector),href=isLink?String(node.href||node.getAttribute('href')||''):'';const points=[[rect.left+rect.width/2,rect.top+rect.height/2],[rect.left+Math.min(4,rect.width/2),rect.top+Math.min(4,rect.height/2)],[rect.right-Math.min(4,rect.width/2),rect.bottom-Math.min(4,rect.height/2)]];return points.some(function(p){let target=doc.elementFromPoint(p[0],p[1]);while(target&&target.shadowRoot){const inner=target.shadowRoot.elementFromPoint(p[0],p[1]);if(!inner||inner===target)break;target=inner;}if(contains(node,target))return true;if(!isLink)return false;for(let current=target;current;current=parentOf(current))if(current!==node&&current.matches&&current.matches(linkSelector)&&String(current.href||current.getAttribute('href')||'')===href)return true;return false;});}"
        + "const s=ownerView.getComputedStyle(el),local=clipped(el,el.getBoundingClientRect(),ownerView);let r=local,view=ownerView;"
        + "if(s.display==='none'||s.visibility==='hidden'||s.pointerEvents==='none'||Number(s.opacity)===0||el.disabled||el.getAttribute('aria-disabled')==='true'||inert(el)||local.width<=0||local.height<=0||!hit(el,local))return {visible:false,element_id:elementId};"
        + "try{while(view&&view!==window){const frame=view.frameElement;if(!frame||!frame.isConnected)return {visible:false,element_id:elementId};const parentView=frame.ownerDocument&&frame.ownerDocument.defaultView;if(!parentView)return {visible:false,element_id:elementId};const fs=parentView.getComputedStyle(frame),raw=frame.getBoundingClientRect(),fr=clipped(frame,raw,parentView);if(fs.display==='none'||fs.visibility==='hidden'||fs.pointerEvents==='none'||Number(fs.opacity)===0||inert(frame)||fr.width<=0||fr.height<=0||!hit(frame,fr))return {visible:false,element_id:elementId};r=intersect({left:r.left+raw.x,top:r.top+raw.y,right:r.right+raw.x,bottom:r.bottom+raw.y},fr);if(r.width<=0||r.height<=0)return {visible:false,element_id:elementId};view=parentView;}}catch(error){return {visible:false,element_id:elementId};}"
        + "if(view!==window)return {visible:false,element_id:elementId};"
        + "let kind='aria';if(el.matches(linkSelector))kind='link';else if(el.matches('button,[role=button]'))kind='button';"
        + "else if(el.matches('input'))kind='input';else if(el.matches('select'))kind='select';else if(el.matches('textarea'))kind='textarea';else if(el.isContentEditable)kind='contenteditable';else if(el.matches('img'))kind='image';else if(el.matches('audio,video'))kind='media';else if((el.scrollHeight>el.clientHeight||el.scrollWidth>el.clientWidth)&&/(auto|scroll)/.test(s.overflow+s.overflowX+s.overflowY))kind='scrollable';"
        + "const text=String(el.getAttribute('aria-label')||el.getAttribute('title')||el.innerText||el.value||'').replace(/[\\u0000-\\u001f\\u007f-\\u009f]+/g,' ').replace(/\\s+/g,' ').trim().slice(0,512);"
        + "let href=kind==='link'?String(el.href||el.getAttribute('href')||''):(kind==='image'||kind==='media'?String(el.currentSrc||el.src||''):null);if(href===''||href&&href.length>8192||href&&/[\\u0000-\\u001f\\u007f-\\u009f]/.test(href))href=null;if(kind==='link'&&href===null)kind='aria';"
        + "return {visible:r.width>0&&r.height>0&&r.bottom>0&&r.right>0&&r.top<window.innerHeight&&r.left<window.innerWidth,element_id:elementId,kind:kind,frame_path:path,"
        + "text:text,"
        + "href:href,geometry:{x:r.left,y:r.top,width:r.width,height:r.height}};})()"
}

function hintFocus(elementId) {
    return "(function(){const elements=window.__ferric_browserHintElements,record=elements&&elements.get("
        + Number(elementId) + ");const el=record&&record.element;"
        + "if(!el||!el.isConnected||!el.matches('input,select,textarea,[contenteditable]:not([contenteditable=false])'))return false;"
        + "function parentOf(node){if(!node)return null;const root=node.getRootNode&&node.getRootNode();return node.parentElement||(root&&root.host)||null;}function contains(parent,node){for(let current=node;current;current=parentOf(current))if(current===parent)return true;return false;}function actionable(node){const view=node.ownerDocument&&node.ownerDocument.defaultView;if(!view)return false;const s=view.getComputedStyle(node),r=node.getBoundingClientRect();if(s.display==='none'||s.visibility==='hidden'||s.pointerEvents==='none'||Number(s.opacity)===0||node.disabled||node.getAttribute('aria-disabled')==='true'||r.width<=0||r.height<=0)return false;for(let p=node;p;p=parentOf(p))if(p.inert||(p.hasAttribute&&p.hasAttribute('inert')))return false;const points=[[r.left+r.width/2,r.top+r.height/2],[r.left+Math.min(4,r.width/2),r.top+Math.min(4,r.height/2)],[r.right-Math.min(4,r.width/2),r.bottom-Math.min(4,r.height/2)]];return points.some(function(point){return contains(node,node.ownerDocument.elementFromPoint(point[0],point[1]));});}if(!actionable(el))return false;"
        + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView;if(ownerView&&typeof ownerView.__ferric_browserAuthorizeExplicitFocus==='function')ownerView.__ferric_browserAuthorizeExplicitFocus();"
        + "el.focus();return true;})()"
}

function hintClick(elementId) {
    var selector = hintSelector(false)
    return "(function(){const selector=" + JSON.stringify(selector) + ",elements=window.__ferric_browserHintElements,record=elements&&elements.get("
        + Number(elementId) + ");const el=record&&record.element;"
        + "if(!el||!el.isConnected||!el.matches(selector)||typeof el.click!=='function')return false;function parentOf(node){if(!node)return null;const root=node.getRootNode&&node.getRootNode();return node.parentElement||(root&&root.host)||null;}function contains(parent,node){for(let current=node;current;current=parentOf(current))if(current===parent)return true;return false;}const view=el.ownerDocument&&el.ownerDocument.defaultView;if(!view)return false;const s=view.getComputedStyle(el),r=el.getBoundingClientRect();if(s.display==='none'||s.visibility==='hidden'||s.pointerEvents==='none'||Number(s.opacity)===0||el.disabled||el.getAttribute('aria-disabled')==='true'||r.width<=0||r.height<=0)return false;for(let p=el;p;p=parentOf(p))if(p.inert||(p.hasAttribute&&p.hasAttribute('inert')))return false;const points=[[r.left+r.width/2,r.top+r.height/2],[r.left+Math.min(4,r.width/2),r.top+Math.min(4,r.height/2)],[r.right-Math.min(4,r.width/2),r.bottom-Math.min(4,r.height/2)]];if(!points.some(function(point){return contains(el,el.ownerDocument.elementFromPoint(point[0],point[1]));}))return false;el.click();return true;})()"
}

function hintSetScrollTarget(elementId) {
    return "(function(){const elements=window.__ferric_browserHintElements,record=elements&&elements.get("
        + Number(elementId) + ");const el=record&&record.element;if(!el||!el.isConnected)return false;"
        + "const s=getComputedStyle(el),scrollable=(el.scrollHeight>el.clientHeight||el.scrollWidth>el.clientWidth)&&/(auto|scroll)/.test(s.overflow+s.overflowX+s.overflowY);"
        + "const r=el.getBoundingClientRect(),hit=el.ownerDocument.elementFromPoint(r.left+r.width/2,r.top+r.height/2);if(!scrollable||s.display==='none'||s.visibility==='hidden'||s.pointerEvents==='none'||r.width<=0||r.height<=0||!(hit===el||el.contains(hit)))return false;window.__ferric_browserScrollTarget=el;return true;})()"
}
