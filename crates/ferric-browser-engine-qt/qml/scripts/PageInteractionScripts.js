.pragma library

// Versioned ApplicationWorld script builders for scroll and Hint operations.
// BrowserScripts.js remains the stable QML-facing compatibility facade.
var VERSION = "1"

function boundedCount(value) {
    return Math.max(1, Math.min(9999, Number(value) || 1))
}

function scrollTargetRuntime() {
    return "function ftRoot(){return document.scrollingElement||document.documentElement||null;}"
        + "function ftParent(node){if(!node)return null;if(node.assignedSlot)return node.assignedSlot;const root=node.getRootNode&&node.getRootNode();return node.parentElement||(root&&root.host)||null;}"
        + "function ftContains(parent,node){for(let current=node,depth=0;current&&depth<256;current=ftParent(current),depth++)if(current===parent)return true;return false;}"
        + "function ftAxisScrollable(element,axis){if(!element||element.nodeType!==1)return false;const owner=element.ownerDocument,view=owner&&owner.defaultView;if(!view)return false;const root=ftRoot();if(owner===document&&element===root)return axis==='x'?element.scrollWidth>view.innerWidth:element.scrollHeight>view.innerHeight;const style=view.getComputedStyle(element);if(axis==='x')return ['auto','scroll','overlay'].indexOf(style.overflowX)>=0&&element.scrollWidth>element.clientWidth;if(axis==='y')return ['auto','scroll','overlay'].indexOf(style.overflowY)>=0&&element.scrollHeight>element.clientHeight;return false;}"
        + "function ftIntersect(a,b){const left=Math.max(a.left,b.left),top=Math.max(a.top,b.top),right=Math.min(a.right,b.right),bottom=Math.min(a.bottom,b.bottom);return {left:left,top:top,right:right,bottom:bottom,width:Math.max(0,right-left),height:Math.max(0,bottom-top)};}"
        + "function ftDeepHit(doc,x,y){let target=doc&&doc.elementFromPoint(x,y);for(let depth=0;target&&target.shadowRoot&&depth<8;depth++){const inner=target.shadowRoot.elementFromPoint(x,y);if(!inner||inner===target)break;target=inner;}return target&&target.shadowRoot?null:target;}"
        + "function ftPointerDeepHit(doc,x,y){let current=doc,pointX=x,pointY=y;for(let depth=0;current&&depth<16;depth++){const target=ftDeepHit(current,pointX,pointY);if(!target)return null;if(target.localName!=='iframe'&&target.localName!=='frame')return target;try{const view=current.defaultView,style=view.getComputedStyle(target);if(style.transform!=='none'||style.perspective!=='none'||Number(style.zoom||1)!==1)return null;for(let node=target,guard=0;node&&guard<256;node=ftParent(node),guard++){const ancestorStyle=(node.ownerDocument&&node.ownerDocument.defaultView||view).getComputedStyle(node);if(ancestorStyle.transform!=='none'||ancestorStyle.perspective!=='none'||Number(ancestorStyle.zoom||1)!==1)return null;}const child=target.contentDocument,childView=child&&child.defaultView,rect=target.getBoundingClientRect(),width=target.clientWidth,height=target.clientHeight,left=rect.left+target.clientLeft,top=rect.top+target.clientTop;if(!child||!childView||childView.document!==child||!ftReachableFromTopDocument(target,document)||!(width>0&&height>0)||pointX<left||pointY<top||pointX>=left+width||pointY>=top+height)return null;pointX=(pointX-left)*childView.innerWidth/width;pointY=(pointY-top)*childView.innerHeight/height;current=child;}catch(error){return null;}}return null;}"
        + "function ftHit(element,rect){const doc=element&&element.ownerDocument;if(!doc||!rect||rect.width<=0||rect.height<=0)return false;const points=[[rect.left+rect.width/2,rect.top+rect.height/2],[rect.left+Math.min(4,rect.width/2),rect.top+Math.min(4,rect.height/2)],[rect.right-Math.min(4,rect.width/2),rect.bottom-Math.min(4,rect.height/2)]];return points.some(function(point){return ftContains(element,ftDeepHit(doc,point[0],point[1]));});}"
        + "function ftInert(element){let node=element,depth=0;for(;node&&depth<256;node=ftParent(node),depth++)if(node.inert||(node.hasAttribute&&node.hasAttribute('inert')))return true;return !!node;}"
        + "function ftDisabled(element){try{return !!(element.matches&&element.matches(':disabled'))||element.getAttribute('aria-disabled')==='true';}catch(error){return true;}}"
        + "function ftFocusEligible(element){if(!element||ftDisabled(element)||ftInert(element))return false;try{return element.matches('input:not([type=hidden]),select,textarea,button,iframe,object,embed,summary,a[href],area[href]')||!!element.isContentEditable||element.tabIndex>=0;}catch(error){return false;}}"
        + "function ftHintPolicy(kind,family){if(family==='scrollables'||kind==='scrollable')return 'scroll';if(kind==='input'||kind==='select'||kind==='textarea'||kind==='contenteditable')return 'focus';return 'pointer';}"
        + "function ftHintDescription(el,family,linkSelector){let kind=family==='scrollables'?'scrollable':'aria';if(family!=='scrollables'){if(el.matches(linkSelector))kind='link';else if(el.matches('button,[role=button]'))kind='button';else if(el.matches('input'))kind='input';else if(el.matches('select'))kind='select';else if(el.matches('textarea'))kind='textarea';else if(el.isContentEditable)kind='contenteditable';else if(el.matches('img'))kind='image';else if(el.matches('audio,video'))kind='media';else if(ftAxisScrollable(el,'x')||ftAxisScrollable(el,'y'))kind='scrollable';}let label=el.getAttribute('aria-label')||el.getAttribute('title')||'';if(!label&&el.labels&&el.labels.length)label=Array.from(el.labels).map(function(item){return item.innerText||item.textContent||'';}).join(' ');if(!label&&!el.matches('input,select,textarea,[contenteditable]:not([contenteditable=false])'))label=el.innerText||'';const text=String(label).replace(/[\\u0000-\\u001f\\u007f-\\u009f]+/g,' ').replace(/\\s+/g,' ').trim().slice(0,512);let href=kind==='link'?String(el.href||el.getAttribute('href')||''):(kind==='image'||kind==='media'?String(el.currentSrc||el.src||''):null);if(href===''||href&&href.length>8192||href&&/[\\u0000-\\u001f\\u007f-\\u009f]/.test(href))href=null;if(kind==='link'&&href===null)kind='aria';return {kind:kind,text:text,href:href};}"
        + "function ftCurrentFramePath(doc){try{const parts=[];let currentDocument=doc,view=doc&&doc.defaultView;if(!view||view.document!==currentDocument)return null;for(let depth=0;view&&view!==window&&depth<8;depth++){const frame=view.frameElement;if(!frame||frame.contentWindow!==view||frame.contentDocument!==currentDocument)return null;const frames=Array.from(frame.ownerDocument.querySelectorAll('iframe,frame')),index=frames.indexOf(frame);if(index<0)return null;parts.push(String(index));currentDocument=frame.ownerDocument;view=currentDocument.defaultView;}if(view!==window)return null;return '0'+(parts.length?'.'+parts.reverse().join('.'): '');}catch(error){return null;}}"
        + "function ftOverflowClips(value){return value==='auto'||value==='scroll'||value==='overlay'||value==='hidden'||value==='clip';}"
        // Root overflow (or body overflow when the HTML root's overflow is visible) is
        // applied to the viewport, not that element's possibly empty box.
        // https://www.w3.org/TR/css-overflow-3/#overflow-propagation
        + "function ftViewportOverflow(node,view){const doc=node.ownerDocument,root=doc&&doc.documentElement;if(node===root)return true;if(!root||node!==doc.body||node.localName!=='body'||root.localName!=='html'||node.parentElement!==root)return false;const rs=view.getComputedStyle(root),bs=view.getComputedStyle(node);return rs.overflowX==='visible'&&rs.overflowY==='visible'&&rs.contain==='none'&&bs.contain==='none'&&bs.display!=='none';}"
        + "function ftClippedRect(element,raw,view){let left=Math.max(0,raw.left),top=Math.max(0,raw.top),right=Math.min(view.innerWidth,raw.right),bottom=Math.min(view.innerHeight,raw.bottom),node=ftParent(element),depth=0;for(;node&&right>left&&bottom>top&&depth<256;node=ftParent(node),depth++){const nodeView=node.ownerDocument&&node.ownerDocument.defaultView||view;if(ftViewportOverflow(node,nodeView))continue;const style=nodeView.getComputedStyle(node),rect=node.getBoundingClientRect();if(ftOverflowClips(style.overflowX)){left=Math.max(left,rect.left);right=Math.min(right,rect.right);}if(ftOverflowClips(style.overflowY)){top=Math.max(top,rect.top);bottom=Math.min(bottom,rect.bottom);}}if(node){right=left;bottom=top;}return {left:left,top:top,right:right,bottom:bottom,width:Math.max(0,right-left),height:Math.max(0,bottom-top)};}"
        + "function ftLocalVisible(element,view,policy){const actionPolicy=policy||'pointer';if(!element||element.nodeType!==1||!view||ftDisabled(element)||ftInert(element)||actionPolicy==='focus'&&!ftFocusEligible(element)||actionPolicy==='scroll'&&!(ftAxisScrollable(element,'x')||ftAxisScrollable(element,'y')))return null;const raw=element.getBoundingClientRect();if(!(raw.width>0&&raw.height>0))return null;let node=element,depth=0;for(;node&&depth<256;node=ftParent(node),depth++){const nodeView=node.ownerDocument&&node.ownerDocument.defaultView||view,style=nodeView.getComputedStyle(node);if(style.display==='none'||style.visibility==='hidden'||style.visibility==='collapse'||Number(style.opacity)===0||(actionPolicy==='pointer'&&style.pointerEvents==='none'))return null;}if(node)return null;const clipped=ftClippedRect(element,raw,view);if(!(clipped.width>0&&clipped.height>0)||actionPolicy==='pointer'&&!ftHit(element,clipped))return null;return {raw:raw,clipped:clipped};}"
        + "function ftProjectedRect(element,local,policy){let view=element&&element.ownerDocument&&element.ownerDocument.defaultView;if(!view||!local)return null;let rect=local.clipped;for(let depth=0;view!==window&&depth<16;depth++){const childView=view,frame=childView.frameElement;if(!frame||!frame.isConnected||frame.contentWindow!==childView||frame.contentDocument!==childView.document)return null;const parentView=frame.ownerDocument&&frame.ownerDocument.defaultView;if(!parentView)return null;const frameLocal=ftLocalVisible(frame,parentView,policy==='pointer'?'pointer':'visible');if(!frameLocal)return null;const raw=frameLocal.raw,content=ftIntersect({left:raw.left+frame.clientLeft,top:raw.top+frame.clientTop,right:raw.left+frame.clientLeft+frame.clientWidth,bottom:raw.top+frame.clientTop+frame.clientHeight},frameLocal.clipped);rect=ftIntersect({left:rect.left+raw.left+frame.clientLeft,top:rect.top+raw.top+frame.clientTop,right:rect.right+raw.left+frame.clientLeft,bottom:rect.bottom+raw.top+frame.clientTop},content);if(!(rect.width>0&&rect.height>0))return null;view=parentView;}if(view!==window)return null;rect=ftIntersect(rect,{left:0,top:0,right:window.innerWidth,bottom:window.innerHeight});return rect.width>0&&rect.height>0?rect:null;}"
        + "function ftMeaningfullyVisible(element,policy){try{if(!element||element.nodeType!==1||!element.isConnected)return false;const actionPolicy=policy||'pointer',local=ftLocalVisible(element,element.ownerDocument&&element.ownerDocument.defaultView,actionPolicy);return !!ftProjectedRect(element,local,actionPolicy);}catch(error){return false;}}"
        + "function ftReachableFromTopDocument(element,capturedDocument){try{if(!element||element.nodeType!==1||!element.isConnected||element.ownerDocument!==capturedDocument)return false;let currentDocument=capturedDocument,view=capturedDocument&&capturedDocument.defaultView;if(!view||view.document!==currentDocument)return false;for(let depth=0;view!==window&&depth<16;depth++){const childView=view,frame=childView.frameElement;if(!frame||!frame.isConnected||childView.document!==currentDocument||frame.contentWindow!==childView||frame.contentDocument!==currentDocument)return false;currentDocument=frame.ownerDocument;view=currentDocument&&currentDocument.defaultView;if(!view)return false;}return view===window;}catch(error){return false;}}"
        + "function ftAutoState(){return {version:1,policy:'auto',elementRef:null,ownerDocumentRef:null};}"
        + "function ftWriteState(next){window.__ferric_browserScrollTargetState=next;return next;}"
        + "function ftState(){try{const raw=window.__ferric_browserScrollTargetState;if(raw===undefined)return ftWriteState(ftAutoState());if(!raw||typeof raw!=='object'||raw.version!==1)return ftWriteState(ftAutoState());if((raw.policy==='auto'||raw.policy==='document')&&raw.elementRef===null&&raw.ownerDocumentRef===null)return raw;if(raw.policy==='element'&&raw.elementRef!=null&&raw.ownerDocumentRef!=null)return raw;return ftWriteState(ftAutoState());}catch(error){const fallback=ftAutoState();try{ftWriteState(fallback);}catch(ignored){}return fallback;}}"
        + "function ftSetPolicy(policy,element){if(policy==='auto'||policy==='document')return {ok:true,policy:ftWriteState({version:1,policy:policy,elementRef:null,ownerDocumentRef:null}).policy};if(policy!=='element'||!element||element.nodeType!==1)return {ok:false,error:'stale-target'};if(typeof WeakRef!=='function'){ftWriteState(ftAutoState());return {ok:false,error:'weak-reference-unavailable'};}try{const elementRef=new WeakRef(element),ownerDocumentRef=new WeakRef(element.ownerDocument);ftWriteState({version:1,policy:'element',elementRef:elementRef,ownerDocumentRef:ownerDocumentRef});return {ok:true,policy:'element'};}catch(error){return {ok:false,error:'script-error'};}}"
        + "function ftValidateElementState(state){if(!state||state.policy!=='element'||typeof WeakRef!=='function')return null;let element=null,capturedDocument=null,failed=false;try{const deref=WeakRef.prototype.deref;if(typeof deref!=='function')return null;try{element=deref.call(state.elementRef);}catch(error){failed=true;}try{capturedDocument=deref.call(state.ownerDocumentRef);}catch(error){failed=true;}}catch(error){return null;}if(failed||!element||!capturedDocument||capturedDocument.nodeType!==9)return null;if(!ftReachableFromTopDocument(element,capturedDocument)||!(ftAxisScrollable(element,'x')||ftAxisScrollable(element,'y'))||!ftMeaningfullyVisible(element,'scroll'))return null;return element;}"
        + "function ftAutomaticTarget(axis){let node=document.activeElement,depth=0;while(node&&node!==document.body&&node!==document.documentElement&&depth<256){if(ftAxisScrollable(node,axis))return node;const shadow=node.shadowRoot&&node.shadowRoot.activeElement;if(shadow){node=shadow;}else{node=ftParent(node);}depth++;}return ftRoot();}"
        + "function ftEffectiveTarget(axis){const current=ftState();if(current.policy==='document')return {target:ftRoot(),policy:'document'};if(current.policy==='element'){const element=ftValidateElementState(current);if(!element){ftSetPolicy('auto');return {target:ftAutomaticTarget(axis),policy:'auto'};}if(!ftAxisScrollable(element,axis))return {target:element,policy:'element',no_op:'unsupported-axis'};return {target:element,policy:'element'};}return {target:ftAutomaticTarget(axis),policy:'auto'};}"
}

function scroll(kind, direction, half, count) {
    return "(function(){"
        + "try{"
        + scrollTargetRuntime()
        + "var kind=" + JSON.stringify(String(kind)) + ";"
        + "var direction=" + JSON.stringify(String(direction)) + ";"
        + "var half=" + (half ? "true" : "false") + ";"
        + "var count=" + String(boundedCount(count)) + ";"
        + "if(!['up','down','left','right','top','bottom'].includes(direction))return {ok:false,error:'script-error'};"
        + "var axis=(direction==='up'||direction==='down'||direction==='top'||direction==='bottom')?'y':'x';"
        + "var resolved=ftEffectiveTarget(axis),target=resolved.target;"
        + "if(!target)return {ok:false,error:'no-scroll-root'};"
        + "if(resolved.no_op)return {ok:true,policy:resolved.policy,no_op:resolved.no_op};"
        + "var root=ftRoot(),isRoot=target===root;"
        + "var width=isRoot?window.innerWidth:target.clientWidth;"
        + "var height=isRoot?window.innerHeight:target.clientHeight;"
        + "if(kind==='scroll-to'){"
        + "var top=direction==='top'?0:Math.max(0,target.scrollHeight-height);"
        + "target.scrollTo(target.scrollLeft||0,top);"
        + "}else{"
        + "var step=kind==='scroll-page'?(half?0.5:0.9):0.15;"
        + "var dx=(direction==='left'?-1:direction==='right'?1:0)*Math.max(40,width*step)*count;"
        + "var dy=(direction==='up'?-1:direction==='down'?1:0)*Math.max(40,height*step)*count;"
        + "target.scrollBy(dx,dy);}"
        + "return {ok:true,policy:resolved.policy};"
        + "}catch(error){return {ok:false,error:'script-error'};}})()"
}

function scrollTargetPolicy(policy) {
    if (policy !== "auto" && policy !== "document")
        return "(function(){return {ok:false,error:'script-error'};})()"
    return "(function(){try{" + scrollTargetRuntime()
        + "return ftSetPolicy(" + JSON.stringify(policy) + ");"
        + "}catch(error){return {ok:false,error:'script-error'};}})()"
}

function scrollTargetStatus() {
    return "(function(){try{" + scrollTargetRuntime()
        + "let current=ftState();if(current.policy==='element'&&!ftValidateElementState(current)){ftSetPolicy('auto');current=ftState();}"
        + "return {ok:true,policy:current.policy};"
        + "}catch(error){return {ok:false,error:'script-error'};}})()"
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

function hintProxyRuntime() {
    return "function proxyIdentity(element,state){let id=state.weak.get(element);if(!id){id=state.next++;state.weak.set(element,id);}return id;}"
        + "function proxyReceiverEligible(element,container){if(!element||!container||!element.isConnected||!container.isConnected||element.ownerDocument!==container.ownerDocument||!container.contains(element)||ftDisabled(element)||ftInert(element)||element.readOnly)return false;const tag=String(element.tagName||'').toLowerCase(),type=String(element.type||'text').toLowerCase();if(tag==='textarea'){if(type!=='textarea')return false;}else if(tag==='input'){if(['text','search','email','url','tel'].indexOf(type)<0)return false;}else return false;let node=element,depth=0;for(;node&&depth<256;node=ftParent(node),depth++){const view=node.ownerDocument&&node.ownerDocument.defaultView;if(!view)return false;const style=view.getComputedStyle(node);if(style.display==='none'||style.visibility==='hidden'||style.visibility==='collapse')return false;}return !node;}"
        + "function proxyEditorValid(container,receiver){if(!container||!receiver||!container.matches('.ace_editor[role=\"textbox\"]'))return false;const receivers=container.querySelectorAll('textarea.ace_text-input,input.ace_text-input');return receivers.length===1&&receivers[0]===receiver&&proxyReceiverEligible(receiver,container);}"
        + "function proxyLabelValid(label,control){if(!label||!control||label.tagName!=='LABEL'||!label.isConnected||!control.isConnected||label.ownerDocument!==control.ownerDocument||label.control!==control||ftInert(label)||ftInert(control)||ftDisabled(control))return false;const type=String(control.type||'').toLowerCase();return control.matches('input')&&(type==='checkbox'||type==='radio');}"
        + "function proxyPairValid(record,candidate,state){if(!record||!candidate||!record.relatedElement||!state||candidate.related_element_id!==record.relatedElementId)return false;const el=record.element,related=record.relatedElement;if(!el||!related||!el.isConnected||!related.isConnected||el.ownerDocument!==related.ownerDocument||record.ownerDocument!==el.ownerDocument||!ftReachableFromTopDocument(related,record.ownerDocument))return false;if(record.kind==='editor-focus')return proxyEditorValid(el,related);if(record.kind==='label-control')return proxyLabelValid(el,related);return false;}"
        + "function proxyText(element,kind){if(kind==='editor-focus')return String(element.getAttribute('aria-label')||element.getAttribute('title')||'Text editor').replace(/[\\u0000-\\u001f\\u007f-\\u009f]+/g,' ').replace(/\\s+/g,' ').trim().slice(0,512);return ftHintDescription(element,'all',linkSelector).text;}"
}

function hintCollector(family, proxyTargets) {
    var selector = hintSelector(family)
    var linkSelector = hintSelector(true)
    return "(function(){"
        + scrollTargetRuntime()
        + "const out=[];const seen=new Set();const activeIds=new Set();const selector=" + JSON.stringify(selector) + ";"
        + "const linkSelector=" + JSON.stringify(linkSelector) + ";"
        + "const family=" + JSON.stringify(family || "all") + ";"
        + "const proxyEnabled=" + (proxyTargets === true || proxyTargets === "validated" ? "true" : "false") + ";"
        + "let state=window.__ferric_browserHintState;if(!state){state={weak:new WeakMap(),elements:new Map(),next:1,revision:1,dirty:true,roots:new WeakSet(),observers:[],windows:[],resize:null};window.__ferric_browserHintState=state;}if(!state.windows)state.windows=[];const elements=state.elements;window.__ferric_browserHintElements=elements;"
        + hintProxyRuntime()
        + "function dirty(){state.dirty=true;state.revision=Math.min(Number.MAX_SAFE_INTEGER,state.revision+1);}if(!state.tracking){state.tracking=true;state.dirtyHandler=dirty;if(typeof ResizeObserver==='function')state.resize=new ResizeObserver(dirty);}"
        + "function track(root){if(state.roots.has(root))return;state.roots.add(root);try{const observer=new MutationObserver(dirty);observer.observe(root,{subtree:true,childList:true,attributes:true,characterData:true});state.observers.push(observer);}catch(error){}const doc=root.nodeType===9?root:root.ownerDocument,view=doc&&doc.defaultView;if(view&&state.windows.indexOf(view)<0){try{view.addEventListener('scroll',dirty,true);view.addEventListener('resize',dirty,true);state.windows.push(view);}catch(error){}}}"
        + "function parentOf(node){return ftParent(node);}"
        + "function linkedSurface(el,r){for(let node=parentOf(el);node;node=parentOf(node))if(node.matches&&node.matches(linkSelector))return node;const doc=el.ownerDocument,points=[[r.left+r.width/2,r.top+r.height/2],[r.left+Math.min(4,r.width/2),r.top+Math.min(4,r.height/2)],[r.right-Math.min(4,r.width/2),r.bottom-Math.min(4,r.height/2)]];for(const point of points)for(const hit of doc.elementsFromPoint(point[0],point[1]))for(let node=hit;node;node=parentOf(node))if(node.matches&&node.matches(linkSelector)){const q=node.getBoundingClientRect();if(q.left<=r.left+1&&q.top<=r.top+1&&q.right>=r.right-1&&q.bottom>=r.bottom-1)return node;}return null;}"
        + "function inert(el){return ftInert(el);}"
        + "function composedContains(parent,node){return ftContains(parent,node);}"
        + "function hit(el,r){return ftHit(el,r);}"
        + "function hasPreferredLink(el,r){if(!el.matches(linkSelector))return false;const scope=parentOf(el);if(!scope||!scope.querySelectorAll)return false;const href=String(el.href||el.getAttribute('href')||''),area=r.width*r.height;for(const candidate of scope.querySelectorAll(linkSelector)){if(candidate===el||String(candidate.href||candidate.getAttribute('href')||'')!==href)continue;const q=candidate.getBoundingClientRect(),view=candidate.ownerDocument&&candidate.ownerDocument.defaultView,s=view&&view.getComputedStyle(candidate);if(q.width>0&&q.height>0&&q.width*q.height+1<area&&s&&s.display!=='none'&&s.visibility!=='hidden'&&s.pointerEvents!=='none'&&Number(s.opacity)!==0&&hit(candidate,q))return true;}return false;}"
        + "const clipped=ftClippedRect;"
        + "function intersect(a,b){const left=Math.max(a.left,b.left),top=Math.max(a.top,b.top),right=Math.min(a.right,b.right),bottom=Math.min(a.bottom,b.bottom);return {left:left,top:top,right:right,bottom:bottom,width:Math.max(0,right-left),height:Math.max(0,bottom-top)};}"
        + "function add(el,ox,oy,framePath,topClip,pairKind,related){if(out.length>=5000||seen.has(el))return;seen.add(el);"
        + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView;if(!ownerView)return;const description=pairKind?{kind:pairKind,text:proxyText(el,pairKind),href:null}:ftHintDescription(el,family,linkSelector),kind=description.kind,policy=ftHintPolicy(kind,family),local=ftLocalVisible(el,ownerView,policy);if(!local)return;const r=local.clipped,projected=ftProjectedRect(el,local,policy);if(!projected)return;const vw=ownerView.innerWidth,vh=ownerView.innerHeight,topRect=intersect(projected,topClip),x=topRect.left,y=topRect.top;"
        + "const scrollable=ftAxisScrollable(el,'x')||ftAxisScrollable(el,'y');if(family==='scrollables'&&!scrollable)return;"
        + "if(r.width<=0||r.height<=0||topRect.width<=0||topRect.height<=0||hasPreferredLink(el,r))return;"
        + "if(r.bottom<=0||r.right<=0||r.top>=vh||r.left>=vw||topRect.right<=0||topRect.bottom<=0||x>=window.innerWidth||y>=window.innerHeight)return;"
        + "if(family==='all'&&(kind==='image'||kind==='media')&&linkedSurface(el,r))return;"
        + "const text=description.text,href=description.href,elementId=proxyIdentity(el,state),relatedElementId=related?proxyIdentity(related,state):null;elements.set(elementId,{element:el,relatedElement:related||null,relatedElementId:relatedElementId,ownerDocument:el.ownerDocument,framePath:framePath,family:family,kind:kind,text:text,href:href});activeIds.add(elementId);if(state.resize)try{state.resize.observe(el);}catch(error){}"
        + "out.push({element_id:elementId,related_element_id:relatedElementId,kind:kind,frame_path:framePath,text:text,href:href,geometry:{x:x,y:y,width:topRect.width,height:topRect.height}});}"
        + "function visit(root,framePath,ox,oy,depth,topClip){if(out.length>=5000||depth>8)return;track(root);"
        + "root.querySelectorAll(selector).forEach(function(el){add(el,ox,oy,framePath,topClip);});if(out.length>=5000)return;"
        + "if(proxyEnabled&&(family==='inputs'||family==='all')){root.querySelectorAll('.ace_editor[role=\"textbox\"]').forEach(function(editor){const receivers=editor.querySelectorAll('textarea.ace_text-input,input.ace_text-input');if(receivers.length===1&&proxyEditorValid(editor,receivers[0]))add(editor,ox,oy,framePath,topClip,'editor-focus',receivers[0]);});root.querySelectorAll('label').forEach(function(label){const control=label.control;if(!proxyLabelValid(label,control))return;const controlId=proxyIdentity(control,state);if(activeIds.has(controlId))return;if(seen.has(label)){const oldId=state.weak.get(label);for(let index=out.length-1;index>=0;index--)if(out[index].element_id===oldId)out.splice(index,1);elements.delete(oldId);activeIds.delete(oldId);seen.delete(label);}add(label,ox,oy,framePath,topClip,'label-control',control);});}if(out.length>=5000)return;"
        + "root.querySelectorAll('*').forEach(function(el){if(family==='all'&&(ftAxisScrollable(el,'x')||ftAxisScrollable(el,'y')))add(el,ox,oy,framePath,topClip);if(el.shadowRoot)visit(el.shadowRoot,framePath,ox,oy,depth+1,topClip);});"
        + "if(root.nodeType!==9)return;root.querySelectorAll('iframe,frame').forEach(function(frame,index){if(out.length>=5000||depth>=8)return;"
        + "const ownerView=frame.ownerDocument&&frame.ownerDocument.defaultView,frameLocal=ftLocalVisible(frame,ownerView,'visible');if(!frameLocal)return;const raw=frameLocal.raw,fr=frameLocal.clipped;"
        + "const vw=ownerView?ownerView.innerWidth:window.innerWidth,vh=ownerView?ownerView.innerHeight:window.innerHeight,content={left:raw.left+frame.clientLeft,top:raw.top+frame.clientTop,right:raw.left+frame.clientLeft+frame.clientWidth,bottom:raw.top+frame.clientTop+frame.clientHeight},frameClip=intersect(intersect({left:content.left+ox,top:content.top+oy,right:content.right+ox,bottom:content.bottom+oy},{left:fr.left+ox,top:fr.top+oy,right:fr.right+ox,bottom:fr.bottom+oy}),topClip);"
        + "if(fr.width<=0||fr.height<=0||frameClip.width<=0||frameClip.height<=0||fr.bottom<=0||fr.right<=0||fr.top>=vh||fr.left>=vw)return;"
        + "try{if(frame.contentDocument)visit(frame.contentDocument,framePath+'.'+index,ox+raw.left+frame.clientLeft,oy+raw.top+frame.clientTop,depth+1,frameClip);}catch(error){}});}"
        + "visit(document,'0',0,0,0,{left:0,top:0,right:innerWidth,bottom:innerHeight});for(const id of Array.from(elements.keys()))if(!activeIds.has(id)){const record=elements.get(id);if(state.resize&&record)try{state.resize.unobserve(record.element);}catch(error){}elements.delete(id);}state.dirty=false;return {candidates:out,revision:state.revision,viewport:{width:innerWidth,height:innerHeight}};})()"
}

function hintDirtyRevision() {
    return "(function(){const s=window.__ferric_browserHintState;return s&&s.tracking?{dirty:s.dirty,revision:s.revision}:{dirty:false,revision:0};})()"
}

function hintStopTracking() {
    return "(function(){const s=window.__ferric_browserHintState;if(!s)return true;if(s.dirtyHandler)for(const view of s.windows||[])try{view.removeEventListener('scroll',s.dirtyHandler,true);view.removeEventListener('resize',s.dirtyHandler,true);}catch(error){}for(const o of s.observers)o.disconnect();if(s.resize)s.resize.disconnect();if(s.elements&&typeof s.elements.clear==='function')s.elements.clear();try{delete window.__ferric_browserHintElements;}catch(error){window.__ferric_browserHintElements=null;}s.observers=[];s.windows=[];s.roots=new WeakSet();s.tracking=false;s.dirty=false;return true;})()"
}

function hintFresh(candidate) {
    var linkSelector = hintSelector(true)
    var relatedElementId = typeof candidate.related_element_id === "number" ? candidate.related_element_id : null
    return "(function(){try{" + scrollTargetRuntime()
        + "const elementId=" + Number(candidate.element_id) + ",path=" + JSON.stringify(candidate.frame_path || "0")
        + ",candidateKind=" + JSON.stringify(candidate.kind || "") + ",candidateRelatedId=" + JSON.stringify(relatedElementId) + ",linkSelector=" + JSON.stringify(linkSelector) + ";"
        + hintProxyRuntime()
        + "const state=window.__ferric_browserHintState,elements=state&&state.elements,record=elements&&elements.get(elementId),el=record&&record.element;"
        + "if(!record||!el||record.framePath!==path||record.ownerDocument!==el.ownerDocument||!ftReachableFromTopDocument(el,record.ownerDocument)||ftCurrentFramePath(el.ownerDocument)!==path)return {visible:false,element_id:elementId};"
        + "const pairKind=record.kind==='editor-focus'||record.kind==='label-control';if(pairKind&&!proxyPairValid(record,{related_element_id:candidateRelatedId},state))return {visible:false,element_id:elementId};"
        + "const maybeScrollable=ftAxisScrollable(el,'x')||ftAxisScrollable(el,'y');if(!pairKind&&!el.matches(" + JSON.stringify(hintSelector("all")) + ")&&!(record.kind==='scrollable'&&maybeScrollable))return {visible:false,element_id:elementId};"
        + "if(record.family==='scrollables'&&!maybeScrollable)return {visible:false,element_id:elementId};"
        + "const description=pairKind?{kind:record.kind,text:proxyText(el,record.kind),href:null}:ftHintDescription(el,record.family,linkSelector);if(description.kind!==record.kind||description.text!==record.text||description.href!==record.href||candidateKind&&candidateKind!==description.kind)return {visible:false,element_id:elementId};"
        + "const policy=ftHintPolicy(description.kind,record.family),ownerView=el.ownerDocument&&el.ownerDocument.defaultView,local=ftLocalVisible(el,ownerView,policy),geometry=ftProjectedRect(el,local,policy);if(!geometry)return {visible:false,element_id:elementId};"
        + "return {visible:true,element_id:elementId,related_element_id:record.relatedElementId,kind:description.kind,frame_path:path,text:description.text,href:description.href,geometry:{x:geometry.left,y:geometry.top,width:geometry.width,height:geometry.height}};"
        + "}catch(error){return {visible:false,element_id:" + Number(candidate.element_id) + "};}})()"
}

function hintPointerPreflight(candidate, viewWidth, viewHeight) {
    var actionCandidate = Object.assign({}, candidate || {})
    if (typeof actionCandidate.href === "undefined")
        actionCandidate.href = null
    if (typeof actionCandidate.related_element_id === "undefined")
        actionCandidate.related_element_id = null
    var elementId = Number(actionCandidate.element_id)
    var width = Number(viewWidth)
    var height = Number(viewHeight)
    var linkSelector = hintSelector(true)
    return "(function(){try{" + scrollTargetRuntime()
        + "const candidate=" + JSON.stringify(actionCandidate) + ",elementId=" + elementId
        + ",viewWidth=" + width + ",viewHeight=" + height
        + ",linkSelector=" + JSON.stringify(linkSelector) + ";"
        + hintProxyRuntime()
        + "if(!Number.isFinite(viewWidth)||!Number.isFinite(viewHeight)||viewWidth<=0||viewHeight<=0||!candidate||!Number.isInteger(candidate.element_id)||candidate.element_id!==elementId)return {ok:false};"
        + "const state=window.__ferric_browserHintState,elements=state&&state.elements,record=elements&&elements.get(elementId),el=record&&record.element;"
        + "if(!record||!el||!el.isConnected||record.framePath!==candidate.frame_path||record.ownerDocument!==el.ownerDocument||!ftReachableFromTopDocument(el,record.ownerDocument)||ftCurrentFramePath(el.ownerDocument)!==candidate.frame_path)return {ok:false};"
        + "const pairKind=record.kind==='editor-focus'||record.kind==='label-control';if(pairKind&&!proxyPairValid(record,candidate,state))return {ok:false};const description=pairKind?{kind:record.kind,text:proxyText(el,record.kind),href:null}:ftHintDescription(el,record.family,linkSelector);if(description.kind!==record.kind||description.text!==record.text||description.href!==record.href||candidate.kind!==description.kind||candidate.text!==description.text||candidate.href!==description.href||candidate.related_element_id!==record.relatedElementId)return {ok:false};"
        + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView,local=ftLocalVisible(el,ownerView,'pointer'),geometry=ftProjectedRect(el,local,'pointer');if(!geometry||!(window.innerWidth>0&&window.innerHeight>0))return {ok:false};"
        + "const insetX=Math.min(12,geometry.width*0.2),insetY=Math.min(12,geometry.height*0.2),points=[[geometry.left+geometry.width/2,geometry.top+geometry.height/2],[geometry.left+insetX,geometry.top+insetY],[geometry.right-insetX,geometry.top+insetY],[geometry.left+insetX,geometry.bottom-insetY],[geometry.right-insetX,geometry.bottom-insetY]];"
        + "let point=null;for(const sample of points){const hit=ftPointerDeepHit(document,sample[0],sample[1]);if(hit&&ftContains(el,hit)){point=sample;break;}}if(!point)return {ok:false};"
        + "const scaleX=viewWidth/window.innerWidth,scaleY=viewHeight/window.innerHeight;if(!Number.isFinite(scaleX)||!Number.isFinite(scaleY)||scaleX<0.2||scaleX>5||scaleY<0.2||scaleY>5)return {ok:false};"
        + "return {ok:true,candidate:{visible:true,element_id:elementId,related_element_id:record.relatedElementId,kind:description.kind,frame_path:candidate.frame_path,text:description.text,href:description.href,geometry:{x:geometry.left,y:geometry.top,width:geometry.width,height:geometry.height}},css_x:point[0],css_y:point[1],x:point[0]*scaleX,y:point[1]*scaleY,viewport_width:window.innerWidth,viewport_height:window.innerHeight,view_width:viewWidth,view_height:viewHeight};"
        + "}catch(error){return {ok:false};}})()"
}

function hintFocus(candidate) {
    var actionCandidate = Object.assign({}, candidate || {})
    if (typeof actionCandidate.href === "undefined")
        actionCandidate.href = null
    if (typeof actionCandidate.related_element_id === "undefined")
        actionCandidate.related_element_id = null
    var elementId = Number(actionCandidate.element_id)
    var freshness = hintFresh(actionCandidate)
    return "(function(){try{const fresh=" + freshness + ",candidate=" + JSON.stringify(actionCandidate) + ";"
        + "if(!fresh.visible||fresh.element_id!==candidate.element_id||fresh.related_element_id!==candidate.related_element_id||fresh.kind!==candidate.kind||fresh.frame_path!==candidate.frame_path||fresh.text!==candidate.text||fresh.href!==candidate.href)return false;"
        + "const state=window.__ferric_browserHintState,record=state&&state.elements&&state.elements.get(" + elementId + "),el=record&&record.element;"
        + scrollTargetRuntime()
        + hintProxyRuntime()
        + "if(candidate.kind==='editor-focus'){if(!proxyPairValid(record,candidate,state))return false;const receiver=record.relatedElement,ownerView=receiver.ownerDocument&&receiver.ownerDocument.defaultView;if(!ownerView)return false;if(typeof ownerView.__ferric_browserAuthorizeExplicitFocus==='function')ownerView.__ferric_browserAuthorizeExplicitFocus();receiver.focus();let active=receiver.ownerDocument.activeElement;for(let depth=0;active&&active.shadowRoot&&depth<32;depth++){const inner=active.shadowRoot.activeElement;if(!inner||inner===active)break;active=inner;}return active===receiver;}"
        + "if(!el||!el.matches('input,select,textarea,[contenteditable]:not([contenteditable=false])'))return false;"
        + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView;if(!ownerView)return false;"
        + "if(typeof ownerView.__ferric_browserAuthorizeExplicitFocus==='function')ownerView.__ferric_browserAuthorizeExplicitFocus();el.focus();"
        + "let active=el.ownerDocument.activeElement;for(let depth=0;active&&active.shadowRoot&&depth<32;depth++){const inner=active.shadowRoot.activeElement;if(!inner||inner===active)break;active=inner;}return active===el;"
        + "}catch(error){return false;}})()"
}

function hintClick(candidate) {
    var actionCandidate = Object.assign({}, candidate || {})
    if (typeof actionCandidate.href === "undefined")
        actionCandidate.href = null
    if (typeof actionCandidate.related_element_id === "undefined")
        actionCandidate.related_element_id = null
    var elementId = Number(actionCandidate.element_id)
    var selector = hintSelector(false)
    var freshness = hintFresh(actionCandidate)
    return "(function(){try{const fresh=" + freshness + ",candidate=" + JSON.stringify(actionCandidate) + ";"
        + "if(!fresh.visible||fresh.element_id!==candidate.element_id||fresh.related_element_id!==candidate.related_element_id||fresh.kind!==candidate.kind||fresh.frame_path!==candidate.frame_path||fresh.text!==candidate.text||fresh.href!==candidate.href)return false;"
        + "const state=window.__ferric_browserHintState,record=state&&state.elements&&state.elements.get(" + elementId + "),el=record&&record.element;"
        + scrollTargetRuntime()
        + hintProxyRuntime()
        + "if(candidate.kind==='label-control'){if(!proxyPairValid(record,candidate,state)||typeof el.click!=='function')return false;const control=record.relatedElement,before=!!control.checked,radio=String(control.type).toLowerCase()==='radio';el.click();return !!control.checked===(radio||!before);}"
        + "if(!el||!el.matches(" + JSON.stringify(selector) + ")||typeof el.click!=='function')return false;el.click();return true;"
        + "}catch(error){return false;}})()"
}

function hintSetScrollTarget(elementId, framePath) {
    return "(function(){try{" + scrollTargetRuntime()
        + "const elementId=" + Number(elementId) + ",path=" + JSON.stringify(String(framePath || "0")) + ";"
        + "const hintState=window.__ferric_browserHintState,elements=hintState&&hintState.elements,record=elements&&elements.get(elementId);"
        + "if(!record||record.framePath!==path||!record.ownerDocument)return {ok:false,error:'stale-target'};"
        + "const element=record.element;if(!element||record.ownerDocument!==element.ownerDocument||!ftReachableFromTopDocument(element,record.ownerDocument)||ftCurrentFramePath(record.ownerDocument)!==path||!ftMeaningfullyVisible(element,'scroll')||!(ftAxisScrollable(element,'x')||ftAxisScrollable(element,'y')))return {ok:false,error:'stale-target'};"
        + "return ftSetPolicy('element',element);"
        + "}catch(error){return {ok:false,error:'script-error'};}})()"
}
