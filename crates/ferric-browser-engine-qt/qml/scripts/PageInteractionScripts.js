.pragma library

// Versioned ApplicationWorld script builders for scroll and Hint operations.
// BrowserScripts.js remains the stable QML-facing compatibility facade.
var VERSION = "4"

function boundedCount(value) {
    return Math.max(1, Math.min(9999, Number(value) || 1))
}

function scrollTargetRuntime() {
    return "const ftFrameIndexCache=new WeakMap();"
        + "function ftRoot(){return document.scrollingElement||document.documentElement||null;}"
        + "function ftParent(node){if(!node)return null;if(node.assignedSlot)return node.assignedSlot;const root=node.getRootNode&&node.getRootNode();return node.parentElement||(root&&root.host)||null;}"
        + "function ftContains(parent,node){for(let current=node,depth=0;current&&depth<256;current=ftParent(current),depth++)if(current===parent)return true;return false;}"
        + "function ftAxisScrollable(element,axis){if(!element||element.nodeType!==1)return false;const owner=element.ownerDocument,view=owner&&owner.defaultView;if(!view)return false;const root=ftRoot();if(owner===document&&element===root)return axis==='x'?element.scrollWidth>view.innerWidth:element.scrollHeight>view.innerHeight;const style=view.getComputedStyle(element);if(axis==='x')return ['auto','scroll','overlay'].indexOf(style.overflowX)>=0&&element.scrollWidth>element.clientWidth;if(axis==='y')return ['auto','scroll','overlay'].indexOf(style.overflowY)>=0&&element.scrollHeight>element.clientHeight;return false;}"
        + "function ftIntersect(a,b){const left=Math.max(a.left,b.left),top=Math.max(a.top,b.top),right=Math.min(a.right,b.right),bottom=Math.min(a.bottom,b.bottom);return {left:left,top:top,right:right,bottom:bottom,width:Math.max(0,right-left),height:Math.max(0,bottom-top)};}"
        + "function ftDeepHit(doc,x,y){let target=doc&&doc.elementFromPoint(x,y);for(let depth=0;target&&target.shadowRoot&&depth<8;depth++){const inner=target.shadowRoot.elementFromPoint(x,y);if(!inner||inner===target)break;target=inner;}return target&&target.shadowRoot?null:target;}"
        + `function ftPointerDeepHit(doc,x,y){
    let current=doc,pointX=x,pointY=y;
    for(let depth=0;current&&depth<16;depth++){
        const target=ftDeepHit(current,pointX,pointY);if(!target)return null;
        if(target.localName!=='iframe'&&target.localName!=='frame')return target;
        try{
            const view=current.defaultView;
            // Native delivery retains its stricter no-transform/no-zoom contract.
            for(let node=target,guard=0;node&&guard<256;node=ftParent(node),guard++){
                const style=(node.ownerDocument&&node.ownerDocument.defaultView||view).getComputedStyle(node);
                if(style.transform!=='none'||style.perspective!=='none'||Number(style.zoom||1)!==1)return null;
            }
            const child=target.contentDocument,childView=child&&child.defaultView;
            const mapping=ftFrameMapping(target,ftLocalVisible(target,view,'pointer'),null);
            if(!mapping||!childView||!ftReachableFromTopDocument(target,target.ownerDocument))return null;
            const content=mapping.content;
            if(pointX<content.left||pointY<content.top||pointX>=content.right||pointY>=content.bottom)return null;
            pointX=(pointX-mapping.left)/mapping.scaleX;pointY=(pointY-mapping.top)/mapping.scaleY;current=child;
        }catch(error){return null;}
    }
    return null;
}`
        + `function ftLinkAliasHit(element,target,rect){
    // A stretched sibling link can receive the pointer over a visible title.
    // This exception belongs to URL hints only: native delivery stays exact.
    const receiver=target;
    if(element.localName!=='a'||!element.hasAttribute('href')||!receiver||receiver.localName!=='a'||!receiver.hasAttribute('href'))return false;
    const doc=element.ownerDocument,container=receiver.parentElement;
    if(receiver.ownerDocument!==doc||!container||container===doc.body||container===doc.documentElement||!ftContains(container,element)||ftDisabled(receiver)||ftInert(receiver))return false;
    if(!element.href||receiver.href!==element.href)return false;
    const cover=receiver.getBoundingClientRect(),style=doc.defaultView.getComputedStyle(receiver);
    // An opaque replacement or coincident clone is not a visible title alias.
    if(style.backgroundImage!=='none'||!['transparent','rgba(0, 0, 0, 0)'].includes(style.backgroundColor))return false;
    for(const pseudo of ['::before','::after']){
        const content=doc.defaultView.getComputedStyle(receiver,pseudo).content;
        if(content!=='none'&&content!=='normal')return false;
    }
    return cover.left<=rect.left&&cover.top<=rect.top&&cover.right>=rect.right&&cover.bottom>=rect.bottom&&(cover.width>rect.width+1||cover.height>rect.height+1);
}`
        + "function ftHit(element,rect,linkAlias){const doc=element&&element.ownerDocument;if(!doc||!rect||rect.width<=0||rect.height<=0)return false;const points=[[rect.left+rect.width/2,rect.top+rect.height/2],[rect.left+Math.min(4,rect.width/2),rect.top+Math.min(4,rect.height/2)],[rect.right-Math.min(4,rect.width/2),rect.bottom-Math.min(4,rect.height/2)]];return points.some(function(point){const target=ftDeepHit(doc,point[0],point[1]);return ftContains(element,target)||linkAlias&&ftLinkAliasHit(element,target,rect);});}"
        + "function ftInert(element){let node=element,depth=0;for(;node&&depth<256;node=ftParent(node),depth++)if(node.inert||(node.hasAttribute&&node.hasAttribute('inert')))return true;return !!node;}"
        + "function ftDisabled(element){try{return !!(element.matches&&element.matches(':disabled'))||element.getAttribute('aria-disabled')==='true';}catch(error){return true;}}"
        + "function ftFocusEligible(element){if(!element||ftDisabled(element)||ftInert(element))return false;try{return element.matches('input:not([type=hidden]),select,textarea,button,iframe,object,embed,summary,a[href],area[href]')||!!element.isContentEditable||element.tabIndex>=0;}catch(error){return false;}}"
        + "function ftHintPolicy(kind,family){if(family==='scrollables'||kind==='scrollable'||kind==='scrollable-region')return 'scroll';if(kind==='input'||kind==='select'||kind==='textarea'||kind==='contenteditable'||kind==='focusable'||kind==='focusable-surface'||kind==='frame')return 'focus';if(family==='regions'||family==='surfaces')return 'visible';return kind==='link'?'link':'pointer';}"
        + `function ftHintDescription(el,family,linkSelector){
    if(family==='regions'||family==='surfaces')return ftHintFacetDescription(el,family);
    let kind=family==='scrollables'?'scrollable':'aria';
    if(family!=='scrollables'){
        if(el.matches('iframe,frame'))kind='frame';else if(el.matches(linkSelector))kind='link';else if(el.matches('button,[role=button]'))kind='button';else if(el.matches('input'))kind='input';else if(el.matches('select'))kind='select';else if(el.matches('textarea'))kind='textarea';else if(el.isContentEditable)kind='contenteditable';else if(el.matches('img'))kind='image';else if(el.matches('audio,video'))kind='media';else if(ftAxisScrollable(el,'x')||ftAxisScrollable(el,'y'))kind='scrollable';
        else if(ftFocusEligible(el)&&!el.matches('[role=button],[role=link],[role=checkbox],[role=switch],[role=radio],[role=option],[role=tab],[role=treeitem],[role^=menuitem],[onclick],[onmousedown],[aria-haspopup],[ng-click],[ngClick],[data-ng-click],[x-ng-click],summary'))kind='focusable';
    }
    const text=(kind==='scrollable'&&el===el.ownerDocument.scrollingElement?'Page scroll':ftHintName(el,kind!=='scrollable'))||({link:'Link',button:'Button',input:'Text field',select:'Select',textarea:'Text area',contenteditable:'Editable text',image:'Image',media:'Media',scrollable:el===el.ownerDocument.scrollingElement?'Page scroll':'Scrollable area',focusable:'Focus region',frame:'Embedded frame',aria:'Control'})[kind];
    let href=kind==='link'?String(el.href||el.getAttribute('href')||''):(kind==='image'||kind==='media'?String(el.currentSrc||el.src||''):null);
    if(href===''||href&&href.length>8192||href&&/[\\u0000-\\u001f\\u007f-\\u009f]/.test(href))href=null;
    if(kind==='link'&&href===null)kind='aria';
    return {kind:kind,text:text,href:href};
}`
        + `function ftFrameIndex(frame){
    try{
        const doc=frame&&frame.ownerDocument;if(!doc||!doc.createTreeWalker)return -1;
        let frames=ftFrameIndexCache.get(doc);
        if(!frames){
            frames=[];let complete=true;
            function walk(root,depth){if(depth>16)return;const walker=doc.createTreeWalker(root,1);let node;
                while((node=walker.nextNode())){if(typeof inspect==='function'&&!inspect()){complete=false;return;}if(node.matches('iframe,frame'))frames.push(node);if(node.shadowRoot)walk(node.shadowRoot,depth+1);if(!complete)return;}}
            walk(doc,0);if(!complete)return -1;ftFrameIndexCache.set(doc,frames);
        }
        return frames.indexOf(frame);
    }catch(error){return -1;}
}`
        + "function ftCurrentFramePath(doc){try{const parts=[];let currentDocument=doc,view=doc&&doc.defaultView;if(!view||view.document!==currentDocument)return null;for(let depth=0;view&&view!==window&&depth<8;depth++){const frame=view.frameElement;if(!frame||frame.contentWindow!==view||frame.contentDocument!==currentDocument)return null;const index=ftFrameIndex(frame);if(index<0)return null;parts.push(String(index));currentDocument=frame.ownerDocument;view=currentDocument.defaultView;}if(view!==window)return null;return '0'+(parts.length?'.'+parts.reverse().join('.'): '');}catch(error){return null;}}"
        + "function ftOverflowClips(value){return value==='auto'||value==='scroll'||value==='overlay'||value==='hidden'||value==='clip';}"
        // Root overflow (or body overflow when the HTML root's overflow is visible) is
        // applied to the viewport, not that element's possibly empty box.
        // https://www.w3.org/TR/css-overflow-3/#overflow-propagation
        + "function ftComputedStyle(element,view,styleCache){if(!styleCache||!styleCache.styles)return view.getComputedStyle(element);let style=styleCache.styles.get(element);if(!style){style=view.getComputedStyle(element);styleCache.styles.set(element,style);}return style;}"
        + "function ftRememberStyleChain(cache,chain,key,visible){if(!cache||!cache.ancestors)return;for(const node of chain){let state=cache.ancestors.get(node);if(!state){state={all:null,pointer:null};cache.ancestors.set(node,state);}state[key]=visible;}}"
        + "function ftStyleChainVisible(element,view,policy,cache){const key=policy==='pointer'||policy==='link'?'pointer':'all',ancestors=cache&&cache.ancestors;if(!ancestors){let node=element,depth=0;for(;node&&depth<256;node=ftParent(node),depth++){const nodeView=node.ownerDocument&&node.ownerDocument.defaultView||view,style=view&&ftComputedStyle(node,nodeView,null);if(style.display==='none'||style.visibility==='hidden'||style.visibility==='collapse'||Number(style.opacity)===0||(key==='pointer'&&style.pointerEvents==='none'))return false;}return !node;}const chain=[];let node=element,depth=0;for(;node&&depth<256;node=ftParent(node),depth++){const state=ancestors.get(node);if(state&&state[key]!==null){ftRememberStyleChain(cache,chain,key,state[key]);return state[key];}const nodeView=node.ownerDocument&&node.ownerDocument.defaultView||view,style=ftComputedStyle(node,nodeView,cache);if(style.display==='none'||style.visibility==='hidden'||style.visibility==='collapse'||Number(style.opacity)===0||(key==='pointer'&&style.pointerEvents==='none')){chain.push(node);ftRememberStyleChain(cache,chain,key,false);return false;}chain.push(node);}const visible=!node;ftRememberStyleChain(cache,chain,key,visible);return visible;}"
        + "function ftCachedRect(element,cache){if(!cache||!cache.rects)return element.getBoundingClientRect();let rect=cache.rects.get(element);if(!rect){const raw=element.getBoundingClientRect();rect={left:raw.left,top:raw.top,right:raw.right,bottom:raw.bottom};cache.rects.set(element,rect);}return rect;}"
        + "function ftViewportOverflow(node,view,styleCache){const doc=node.ownerDocument,root=doc&&doc.documentElement;if(node===root)return true;if(!root||node!==doc.body||node.localName!=='body'||root.localName!=='html'||node.parentElement!==root)return false;const rs=ftComputedStyle(root,view,styleCache),bs=ftComputedStyle(node,view,styleCache);return rs.overflowX==='visible'&&rs.overflowY==='visible'&&rs.contain==='none'&&bs.contain==='none'&&bs.display!=='none';}"
        + "function ftClippedRect(element,raw,view,styleCache){let left=Math.max(0,raw.left),top=Math.max(0,raw.top),right=Math.min(view.innerWidth,raw.right),bottom=Math.min(view.innerHeight,raw.bottom),node=ftParent(element),depth=0;for(;node&&right>left&&bottom>top&&depth<256;node=ftParent(node),depth++){const nodeView=node.ownerDocument&&node.ownerDocument.defaultView||view;if(ftViewportOverflow(node,nodeView,styleCache))continue;const style=ftComputedStyle(node,nodeView,styleCache),rect=ftCachedRect(node,styleCache);if(ftOverflowClips(style.overflowX)){left=Math.max(left,rect.left);right=Math.min(right,rect.right);}if(ftOverflowClips(style.overflowY)){top=Math.max(top,rect.top);bottom=Math.min(bottom,rect.bottom);}}if(node){right=left;bottom=top;}return {left:left,top:top,right:right,bottom:bottom,width:Math.max(0,right-left),height:Math.max(0,bottom-top)};}"
        + "function ftLocalVisible(element,view,policy,styleCache){const actionPolicy=policy||'pointer';if(!element||element.nodeType!==1||!view||ftDisabled(element)||ftInert(element)||actionPolicy==='focus'&&!ftFocusEligible(element)||actionPolicy==='scroll'&&!(ftAxisScrollable(element,'x')||ftAxisScrollable(element,'y')))return null;const raw=element.getBoundingClientRect();if(!(raw.width>0&&raw.height>0)||!ftStyleChainVisible(element,view,actionPolicy,styleCache))return null;const clipped=ftClippedRect(element,raw,view,styleCache);if(!(clipped.width>0&&clipped.height>0)||(actionPolicy==='pointer'||actionPolicy==='link')&&!ftHit(element,clipped,actionPolicy==='link'))return null;return {raw:raw,clipped:clipped};}"
        + `function ftFrameMapping(frame,local,cache,viewport){
    try{
        if(cache&&cache.frames&&cache.frames.has(frame))return cache.frames.get(frame);
        const child=frame.contentDocument,childView=child&&child.defaultView,parentView=frame.ownerDocument.defaultView;
        if(!local||(!viewport&&(!childView||frame.contentWindow!==childView||childView.document!==child)))return null;
        const childWidth=viewport?viewport.width:childView.innerWidth,childHeight=viewport?viewport.height:childView.innerHeight;
        let node=frame,depth=0;
        for(;node&&depth<256;node=ftParent(node),depth++){
            const view=node.ownerDocument.defaultView,style=ftComputedStyle(node,view,cache);
            if(style.perspective!=='none')return null;
            if(style.transform!=='none'){const matrix=new view.DOMMatrixReadOnly(style.transform);if(!matrix.is2D||matrix.b!==0||matrix.c!==0||!(matrix.a>0&&matrix.d>0))return null;}
        }
        if(node||!(frame.offsetWidth>0&&frame.offsetHeight>0&&childWidth>0&&childHeight>0))return null;
        const sx=local.raw.width/frame.offsetWidth,sy=local.raw.height/frame.offsetHeight;
        if(!(Number.isFinite(sx)&&Number.isFinite(sy)&&sx>0&&sy>0))return null;
        const style=ftComputedStyle(frame,parentView,cache);
        const pl=parseFloat(style.paddingLeft)||0,pr=parseFloat(style.paddingRight)||0,pt=parseFloat(style.paddingTop)||0,pb=parseFloat(style.paddingBottom)||0;
        const width=frame.clientWidth-pl-pr,height=frame.clientHeight-pt-pb;if(!(width>0&&height>0))return null;
        const left=local.raw.left+(frame.clientLeft+pl)*sx,top=local.raw.top+(frame.clientTop+pt)*sy;
        const content={left:left,top:top,right:left+width*sx,bottom:top+height*sy};
        const result={left:left,top:top,scaleX:sx*width/childWidth,scaleY:sy*height/childHeight,content:ftIntersect(content,local.clipped),parentView:parentView};
        if(cache&&cache.frames)cache.frames.set(frame,result);
        return result;
    }catch(error){return null;}
}`
        + `function ftProjectedRect(element,local,policy,styleCache){
    let view=element&&element.ownerDocument&&element.ownerDocument.defaultView;if(!view||!local)return null;
    let rect=local.clipped;
    for(let depth=0;view!==window&&depth<16;depth++){
        const childView=view,frame=childView.frameElement;
        if(!frame||!frame.isConnected||frame.contentWindow!==childView||frame.contentDocument!==childView.document)return null;
        const parentView=frame.ownerDocument.defaultView,frameLocal=ftLocalVisible(frame,parentView,policy==='pointer'||policy==='link'?'pointer':'visible',styleCache),mapping=ftFrameMapping(frame,frameLocal,styleCache);
        if(!mapping)return null;
        rect=ftIntersect({left:mapping.left+rect.left*mapping.scaleX,top:mapping.top+rect.top*mapping.scaleY,right:mapping.left+rect.right*mapping.scaleX,bottom:mapping.top+rect.bottom*mapping.scaleY},mapping.content);
        if(!(rect.width>0&&rect.height>0))return null;view=parentView;
    }
    if(view!==window)return null;
    rect=ftIntersect(rect,{left:0,top:0,right:window.innerWidth,bottom:window.innerHeight});
    return rect.width>0&&rect.height>0?rect:null;
}`
        + "function ftMeaningfullyVisible(element,policy){try{if(!element||element.nodeType!==1||!element.isConnected)return false;const actionPolicy=policy||'pointer',local=ftLocalVisible(element,element.ownerDocument&&element.ownerDocument.defaultView,actionPolicy);return !!ftProjectedRect(element,local,actionPolicy);}catch(error){return false;}}"
        + "function ftReachableFromTopDocument(element,capturedDocument){try{if(!element||element.nodeType!==1||!element.isConnected||element.ownerDocument!==capturedDocument)return false;let currentDocument=capturedDocument,view=capturedDocument&&capturedDocument.defaultView;if(!view||view.document!==currentDocument)return false;for(let depth=0;view!==window&&depth<16;depth++){const childView=view,frame=childView.frameElement;if(!frame||!frame.isConnected||childView.document!==currentDocument||frame.contentWindow!==childView||frame.contentDocument!==currentDocument)return false;currentDocument=frame.ownerDocument;view=currentDocument&&currentDocument.defaultView;if(!view)return false;}return view===window;}catch(error){return false;}}"
        + "function ftWithinScope(scopeElement,scopeDocument,element){try{if(!scopeElement||!scopeDocument||!element||element.nodeType!==1)return false;let currentDocument=element.ownerDocument;if(!currentDocument)return false;for(let depth=0;currentDocument&&currentDocument!==scopeDocument&&depth<16;depth++){const view=currentDocument.defaultView,frame=view&&view.frameElement;if(!view||!frame||!frame.isConnected||view.document!==currentDocument||frame.contentWindow!==view||frame.contentDocument!==currentDocument)return false;currentDocument=frame.ownerDocument;if(!currentDocument)return false;if(currentDocument===scopeDocument)return ftContains(scopeElement,frame);}return currentDocument===scopeDocument&&ftContains(scopeElement,element);}catch(error){return false;}}"
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

function normalizedHintPolicySnapshot(snapshot) {
    if (typeof snapshot === "string") {
        try { snapshot = JSON.parse(snapshot) } catch (error) { return {error:"invalid-snapshot"} }
    }
    snapshot = snapshot && typeof snapshot === "object" ? snapshot : ({})
    var policies = snapshot.policies
    if (typeof policies === "undefined") return snapshot
    if (!policies || typeof policies !== "object" || Array.isArray(policies))
        return {error:"invalid-selector-policy"}
    var families = ["all", "links", "inputs", "buttons", "images", "media", "scrollables"]
    for (var family in policies) {
        var entry = policies[family]
        if (families.indexOf(family) < 0 || !entry || typeof entry !== "object"
                || !Object.prototype.hasOwnProperty.call(entry, "policy"))
            return {error:"invalid-selector-policy"}
        var policy = entry.policy
        if (policy === null) continue
        if (!policy || (policy.mode !== "extend" && policy.mode !== "replace")
                || !Array.isArray(policy.selectors)
                || policy.selectors.length > 32
                || policy.selectors.some(function(selector) { return typeof selector !== "string" || selector.length === 0 }))
            return {error:"invalid-selector-policy"}
    }
    return snapshot
}


function hintNameRuntime() {
    return `function ftNameText(root,hidden){
    if(!root||root.nodeType!==1||root.isContentEditable||root.matches('input,textarea,select'))return '';
    const stack=[root];let text='',visited=0;
    while(stack.length&&visited++<256&&text.length<2048){
        if(typeof inspect==='function'&&!inspect())break;
        const node=stack.pop();
        if(node.nodeType===3){text+=' '+String(node.data||'').slice(0,2048-text.length);continue;}
        if(node.nodeType!==1||node.isContentEditable||node.matches('input,textarea,select,script,style'))continue;
        if(!hidden){const view=node.ownerDocument.defaultView,style=view.getComputedStyle(node);if(style.display==='none'||style.visibility==='hidden'||style.visibility==='collapse')continue;}
        // Do not enqueue an unbounded sibling list. Text traversal is bounded.
        let child=node.lastChild,count=0;
        for(;child&&count<256-visited&&stack.length<256;child=child.previousSibling,count++)stack.push(child);
    }
    return text;
}
function ftHintName(element,visibleText){
    let name='';const root=element.getRootNode(),ids=String(element.getAttribute('aria-labelledby')||'').slice(0,1024).trim().split(/\\s+/).filter(Boolean).slice(0,16);
    for(const id of ids){const named=root&&typeof root.getElementById==='function'?root.getElementById(id):null;if(named&&named.isConnected)name+=' '+ftNameText(named,true);if(name.length>=2048)break;}
    if(!name.trim())name=String(element.getAttribute('aria-label')||'').slice(0,2048);
    if(!name&&element.labels)for(const label of Array.prototype.slice.call(element.labels,0,16)){name+=' '+ftNameText(label,true);if(name.length>=2048)break;}
    if(!name&&element.matches('img,area'))name=String(element.getAttribute('alt')||'').slice(0,2048);
    if(!name&&visibleText)name=ftNameText(element,false);
    if(!name.trim())name=String(element.getAttribute('title')||'').slice(0,2048);
    return Array.from(name.slice(0,2048).replace(/[\\u0000-\\u001f\\u007f-\\u009f]+/g,' ').replace(/\\s+/g,' ').trim()).slice(0,512).map(function(character){const point=character.codePointAt(0);return point>=0xd800&&point<=0xdfff?'\\uFFFD':character;}).join('');
}
function ftHintPresentation(element,local,policy,topClip,cache){
    const rects=element.getClientRects();
    // A single border box has exactly the already validated/projected geometry.
    // Avoid a duplicate projection and payload; wrapped controls keep fragments.
    if(rects.length===1){const rect=rects[0],raw=local.raw;if(rect.left===raw.left&&rect.top===raw.top&&rect.right===raw.right&&rect.bottom===raw.bottom)return null;}
    const fragments=[];let examined=0;
    for(const rect of rects){
        if(examined++>=8)break;
        const clipped=ftIntersect(ftClippedRect(element,rect,element.ownerDocument.defaultView,cache),local.clipped);
        const projected=ftProjectedRect(element,{raw:local.raw,clipped:clipped},policy,cache);
        if(!projected)continue;const visible=ftIntersect(projected,topClip);
        if(visible.width>0&&visible.height>0)fragments.push({x:visible.left,y:visible.top,width:visible.width,height:visible.height});
    }
    if(!fragments.length)return null;
    const first=fragments[0];return {anchor:{x:first.x,y:first.y},fragments:fragments};
}`
}

function hintPolicyRuntime() {
    var defaults = {
        all: hintSelector("all"),
        links: hintSelector("links"),
        inputs: hintSelector("inputs"),
        buttons: hintSelector("buttons"),
        images: hintSelector("images"),
        media: hintSelector("media"),
        scrollables: hintSelector("scrollables")
    }
    return hintNameRuntime() + "const ftHintDefaults=" + JSON.stringify(defaults) + ";"
        + "function ftHintFamilyPolicy(family,policies){const entry=policies&&policies[family];return entry?entry.policy:null;}"
        + "function ftHintSelectors(family,policies){if(family==='regions'||family==='surfaces')return [];const policy=ftHintFamilyPolicy(family,policies);if(!policy)return [ftHintDefaults[family]];const custom=Array.isArray(policy.selectors)?policy.selectors:[];return (policy.mode==='replace'?[]:[ftHintDefaults[family]]).concat(custom); }"
        + "function ftHintMatchesAny(element,selectors){if(!element||!Array.isArray(selectors))return false;for(const selector of selectors){try{if(element.matches(selector))return true;}catch(error){}}return false;}"
        + "function ftHintFacetLabel(element){return ftHintName(element,false);}"
        + "function ftHintFacetDescription(element,family){if(!element||element.nodeType!==1)return null;const label=ftHintFacetLabel(element);if(family==='regions'){const role=String(element.getAttribute('role')||'').toLowerCase(),tag=String(element.localName||'').toLowerCase(),root=ftRoot(),scrollable=element!==root&&(ftAxisScrollable(element,'x')||ftAxisScrollable(element,'y'));let ace=false;if(tag==='div'&&element.matches('.ace_editor[role=\\\"textbox\\\"]')&&typeof proxyEditorReceivers==='function'){const receivers=proxyEditorReceivers(element);ace=!!receivers&&receivers.length===1&&proxyReceiverEligible(receivers[0],element);}const structural=['main','nav','aside','dialog'].includes(tag)||['main','navigation','complementary','dialog','alertdialog','toolbar','tablist','tree','treegrid','listbox','grid'].includes(role)||(tag==='section'&&!!label)||(role==='region'&&!!label);if(!scrollable&&!structural&&!ace)return null;let name=label;if(!name){if(ace)name='Text editor';else if(scrollable&&!structural)name='Scrollable region';else if(tag==='main'||role==='main')name='Main region';else if(tag==='nav'||role==='navigation')name='Navigation region';else if(tag==='aside'||role==='complementary')name='Complementary region';else if(tag==='dialog'||role==='dialog'||role==='alertdialog')name='Dialog region';else name=role?role.replace(/-/g,' ')+' region':'Section';}return {kind:scrollable?'scrollable-region':'region',text:name,href:null,named:!!label};}if(family==='surfaces'){if(!element.matches('canvas,video,iframe,frame,object,embed'))return null;const kind=element.matches('iframe,frame')?'frame':ftFocusEligible(element)?'focusable-surface':'surface',tag=String(element.localName||'').toLowerCase(),name=label||(tag==='iframe'||tag==='frame'?'Frame surface':tag.charAt(0).toUpperCase()+tag.slice(1)+' surface');return {kind:kind,text:name,href:null,named:!!label};}return null;}"
        + "function ftHintRecordMatchesPolicy(record,element){if(!record||!element)return false;if(record.family==='regions'||record.family==='surfaces'){const description=ftHintFacetDescription(element,record.family);return !!description&&description.kind===record.kind;}const policies=record.selectorPolicies||{},families=Array.isArray(record.discoveryFamilies)&&record.discoveryFamilies.length?record.discoveryFamilies:[record.family];if(record.kind==='editor-focus'||record.kind==='label-control'){const policy=ftHintFamilyPolicy(record.family,policies);return !policy||policy.mode!=='replace'||ftHintMatchesAny(element,ftHintSelectors(record.family,policies));}return families.some(function(family){return ftHintMatchesAny(element,ftHintSelectors(family,policies));});}"
        + "function ftHintRevisionMatches(record,revision){return revision===null||typeof revision==='undefined'||String(record&&record.selectorRevision||'')===String(revision);}"
}

function hintProxyRuntime() {
    return "function proxyIdentity(element,state){let id=state.weak.get(element);if(!id){id=state.next++;state.weak.set(element,id);}return id;}"
        + "function proxyReceiverEligible(element,container){if(!element||!container||!element.isConnected||!container.isConnected||element.ownerDocument!==container.ownerDocument||!container.contains(element)||ftDisabled(element)||ftInert(element)||element.readOnly)return false;const tag=String(element.tagName||'').toLowerCase(),type=String(element.type||'text').toLowerCase();if(tag==='textarea'){if(type!=='textarea')return false;}else if(tag==='input'){if(['text','search','email','url','tel'].indexOf(type)<0)return false;}else return false;let node=element,depth=0;for(;node&&depth<256;node=ftParent(node),depth++){const view=node.ownerDocument&&node.ownerDocument.defaultView;if(!view)return false;const style=view.getComputedStyle(node);if(style.display==='none'||style.visibility==='hidden'||style.visibility==='collapse')return false;}return !node;}"
        + "function proxyEditorReceivers(container){const doc=container&&container.ownerDocument;if(!doc||!doc.createTreeWalker)return null;const walker=doc.createTreeWalker(container,0xffffffff);let node,receiver=null,visited=0;while((node=walker.nextNode())){if(typeof inspect==='function'){if(!inspect())return null;}else if(++visited>50000)return null;if(node.nodeType!==1||!node.matches('textarea.ace_text-input,input.ace_text-input'))continue;if(receiver)return null;receiver=node;}return receiver?[receiver]:[];}"
        + "function proxyEditorValid(container,receiver){if(!container||!receiver||!container.matches('.ace_editor[role=\"textbox\"]'))return false;const receivers=proxyEditorReceivers(container);return !!receivers&&receivers.length===1&&receivers[0]===receiver&&proxyReceiverEligible(receiver,container);}"
        + "function proxyLabelValid(label,control){if(!label||!control||label.tagName!=='LABEL'||!label.isConnected||!control.isConnected||label.ownerDocument!==control.ownerDocument||label.control!==control||ftInert(label)||ftInert(control)||ftDisabled(control))return false;const type=String(control.type||'').toLowerCase();return control.matches('input')&&(type==='checkbox'||type==='radio');}"
        + "function proxyPairValid(record,candidate,state){if(!record||!candidate||!record.relatedElement||!state||candidate.related_element_id!==record.relatedElementId)return false;const el=record.element,related=record.relatedElement;if(!el||!related||!el.isConnected||!related.isConnected||el.ownerDocument!==related.ownerDocument||record.ownerDocument!==el.ownerDocument||!ftReachableFromTopDocument(related,record.ownerDocument))return false;if(record.kind==='editor-focus')return proxyEditorValid(el,related);if(record.kind==='label-control')return proxyLabelValid(el,related);return false;}"
        + "function proxyText(element,kind){return kind==='editor-focus'?(ftHintName(element,false)||'Text editor'):ftHintDescription(element,'all',linkSelector).text;}"
}

function hintCollector(family, proxyTargets, selectorSnapshot, scopeLeaseId, includeFacets, budget) {
    var linkSelector = hintSelector(true)
    var snapshot = normalizedHintPolicySnapshot(selectorSnapshot)
    if (snapshot.error)
        return "(function(){return {error:" + JSON.stringify(String(snapshot.error)) + "};})()"
    var selectorRevision = typeof snapshot.selector_revision === "string"
            ? snapshot.selector_revision : ""
    var selectorPolicies = snapshot.policies && typeof snapshot.policies === "object"
            ? snapshot.policies : ({})
    var requestedScopeLeaseId = typeof scopeLeaseId === "string" && scopeLeaseId.length <= 128
            ? scopeLeaseId : ""
    // One synchronous pass can cache shared style/ancestor/clip reads; the
    // per-call caches are discarded so later collections observe page changes.
    return "(function(){"
        + scrollTargetRuntime()
        + hintPolicyRuntime()
        + "const scannedPhysicalNodes=new WeakSet();const preferredScopes=new WeakMap();const out=[];const seen=new Set();const activeIds=new Set();const visibilityStyleCache={styles:new WeakMap(),ancestors:new WeakMap(),rects:new WeakMap(),frames:new WeakMap()};const selectorPolicies=" + JSON.stringify(selectorPolicies) + ";"
        + "const family=" + JSON.stringify(family || "all") + ";"
        + "const requestedScopeLeaseId=" + JSON.stringify(requestedScopeLeaseId) + ";"
        + "const includeFacets=" + (includeFacets === true ? "true" : "false") + ";"
        + "const facet=family==='regions'||family==='surfaces',selectors=ftHintSelectors(family,selectorPolicies),scrollSelectors=ftHintSelectors('scrollables',selectorPolicies),candidateLimit=Math.min(facet?256:5000," + Math.max(0, Math.min(5000, Number(budget && budget.candidates !== undefined ? budget.candidates : 5000))) + ");"
        + "const linkSelector=" + JSON.stringify(linkSelector) + ";"
        + "const selectorRevision=" + JSON.stringify(selectorRevision) + ";"
        + "const proxyEnabled=" + (proxyTargets === true || proxyTargets === "validated" ? "true" : "false") + ";"
        + "const traversalDepthLimit=" + Math.max(0, Math.min(8, Number(budget && budget.depth !== undefined ? budget.depth : 8))) + ";"
        + "const inspectionLimit=" + Math.max(0, Math.min(50000, Number(budget && budget.inspections !== undefined ? budget.inspections : 50000))) + ";let inspectedNodes=0,truncated=false;function inspect(){if(inspectedNodes>=inspectionLimit){truncated=true;return false;}inspectedNodes++;return true;}"
        + "let state=window.__ferric_browserHintState;if(!state){state={weak:new WeakMap(),elements:new Map(),next:1,revision:1,dirty:true,roots:new WeakSet(),observers:[],windows:[],resize:null,scopeLease:null};window.__ferric_browserHintState=state;}if(!state.windows)state.windows=[];if(typeof state.scopeLease==='undefined')state.scopeLease=null;const elements=state.elements;window.__ferric_browserHintElements=elements;"
        + hintProxyRuntime()
        + "function dirty(){state.dirty=true;state.revision=Math.min(Number.MAX_SAFE_INTEGER,state.revision+1);}if(!state.tracking){state.tracking=true;state.dirtyHandler=dirty;if(typeof ResizeObserver==='function')state.resize=new ResizeObserver(dirty);}"
        + "function track(root){if(state.roots.has(root))return;state.roots.add(root);try{const observer=new MutationObserver(dirty);observer.observe(root,{subtree:true,childList:true,attributes:true,characterData:true});state.observers.push(observer);}catch(error){}const doc=root.nodeType===9?root:root.ownerDocument,view=doc&&doc.defaultView;if(view&&state.windows.indexOf(view)<0){try{view.addEventListener('scroll',dirty,true);view.addEventListener('resize',dirty,true);state.windows.push(view);}catch(error){}}}"
        + "function parentOf(node){return ftParent(node);}"
        + "function linkedSurface(el,r){for(let node=parentOf(el);node;node=parentOf(node))if(node.matches&&node.matches(linkSelector))return node;const doc=el.ownerDocument,points=[[r.left+r.width/2,r.top+r.height/2],[r.left+Math.min(4,r.width/2),r.top+Math.min(4,r.height/2)],[r.right-Math.min(4,r.width/2),r.bottom-Math.min(4,r.height/2)]];for(const point of points)for(const hit of doc.elementsFromPoint(point[0],point[1]))for(let node=hit;node;node=parentOf(node))if(node.matches&&node.matches(linkSelector)){const q=node.getBoundingClientRect();if(q.left<=r.left+1&&q.top<=r.top+1&&q.right>=r.right-1&&q.bottom>=r.bottom-1)return node;}return null;}"
        + "function hit(el,r){return ftHit(el,r);}"
        + `function hasPreferredLink(el,r){
    if(!el.matches(linkSelector))return false;
    const scope=parentOf(el),doc=el.ownerDocument;
    if(!scope||!doc||!doc.createTreeWalker)return false;
    let summary=preferredScopes.get(scope);
    if(typeof summary==='undefined'){
        summary=new Map();const walker=doc.createTreeWalker(scope,0xffffffff);let candidate,complete=true;
        while((candidate=walker.nextNode())){
            if(!inspect()){complete=false;break;}
            if(candidate.nodeType!==1||!candidate.matches(linkSelector))continue;
            const q=ftCachedRect(candidate,visibilityStyleCache),view=candidate.ownerDocument.defaultView,s=ftComputedStyle(candidate,view,visibilityStyleCache);
            const width=q.right-q.left,height=q.bottom-q.top;
            if(width<=0||height<=0||s.display==='none'||s.visibility==='hidden'||s.pointerEvents==='none'||Number(s.opacity)===0||!hit(candidate,{left:q.left,top:q.top,right:q.right,bottom:q.bottom,width:width,height:height}))continue;
            const href=String(candidate.href||candidate.getAttribute('href')||''),area=width*height;
            if(!summary.has(href)||area<summary.get(href))summary.set(href,area);
        }
        if(!complete)summary=null;
        preferredScopes.set(scope,summary);
    }
    const href=String(el.href||el.getAttribute('href')||'');
    return !!summary&&summary.has(href)&&summary.get(href)+1<r.width*r.height;
}`
        + "function intersect(a,b){const left=Math.max(a.left,b.left),top=Math.max(a.top,b.top),right=Math.min(a.right,b.right),bottom=Math.min(a.bottom,b.bottom);return {left:left,top:top,right:right,bottom:bottom,width:Math.max(0,right-left),height:Math.max(0,bottom-top)};}"
        + "function add(el,ox,oy,framePath,topClip,pairKind,related,discoveryFamily,recordFamily){const candidateFamily=recordFamily||family;if(!facet&&out.length>=candidateLimit){truncated=true;return;}if(!pairKind&&candidateFamily!=='regions'&&candidateFamily!=='surfaces'&&!ftHintMatchesAny(el,ftHintSelectors(discoveryFamily||candidateFamily,selectorPolicies)))return;if(seen.has(el)){const id=state.weak.get(el),old=elements.get(id),source=discoveryFamily||candidateFamily;if(old&&old.discoveryFamilies.indexOf(source)<0)old.discoveryFamilies.push(source);return;}seen.add(el);"
        + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView;if(!ownerView)return;const description=pairKind?{kind:pairKind,text:proxyText(el,pairKind),href:null}:ftHintDescription(el,candidateFamily,linkSelector);if(!description)return;const kind=description.kind,policy=ftHintPolicy(kind,candidateFamily),local=ftLocalVisible(el,ownerView,policy,visibilityStyleCache);if(!local)return;const r=local.clipped,projected=ftProjectedRect(el,local,policy,visibilityStyleCache);if(!projected)return;if(candidateFamily==='surfaces'&&!description.named&&(r.width<16||r.height<16))return;const vw=ownerView.innerWidth,vh=ownerView.innerHeight,topRect=intersect(projected,topClip),x=topRect.left,y=topRect.top;"
        + "const scrollable=ftAxisScrollable(el,'x')||ftAxisScrollable(el,'y');if(candidateFamily==='scrollables'&&!scrollable)return;"
        + "if(r.width<=0||r.height<=0||topRect.width<=0||topRect.height<=0||hasPreferredLink(el,r))return;"
        + "if(r.bottom<=0||r.right<=0||r.top>=vh||r.left>=vw||topRect.right<=0||topRect.bottom<=0||x>=window.innerWidth||y>=window.innerHeight)return;if(facet&&out.length>=candidateLimit){truncated=true;return;}"
        + "if(candidateFamily==='all'&&(kind==='image'||kind==='media')&&linkedSurface(el,r))return;"
        + "const text=description.text,href=description.href,elementId=proxyIdentity(el,state),relatedElementId=related?proxyIdentity(related,state):null,source=discoveryFamily||candidateFamily,policyEntry=selectorPolicies[candidateFamily];elements.set(elementId,{element:el,relatedElement:related||null,relatedElementId:relatedElementId,ownerDocument:el.ownerDocument,framePath:framePath,family:candidateFamily,kind:kind,text:text,href:href,selectorPolicies:selectorPolicies,selectorRevision:selectorRevision,scopeLeaseId:requestedScopeLeaseId||null,discoveryFamilies:[source],provenance:policyEntry&&policyEntry.provenance||['built-in']});activeIds.add(elementId);if(state.resize)try{state.resize.observe(el);}catch(error){}"
        + "out.push({element_id:elementId,related_element_id:relatedElementId,kind:kind,frame_path:framePath,text:text,href:href,presentation:ftHintPresentation(el,local,policy,topClip,visibilityStyleCache),geometry:{x:x,y:y,width:topRect.width,height:topRect.height}});}"
        + "function visit(root,framePath,ox,oy,depth,topClip,includeRoot){if(!facet&&out.length>=candidateLimit){truncated=true;return;}if(facet&&truncated||depth>traversalDepthLimit)return;track(root);const doc=root.nodeType===9?root:root.ownerDocument;if(!doc||!doc.createTreeWalker)return;const walker=doc.createTreeWalker(root,0xffffffff),normal=[],facetNodes=[],scrollableNodes=[],editors=[],editorReceivers=new Map(),labels=[],shadows=[],frames=[],slots=[];let node;"
        + "function scan(el){if(family==='regions'||family==='surfaces'){if(ftHintFacetDescription(el,family))normal.push(el);}else if(ftHintMatchesAny(el,selectors))normal.push(el);if(includeFacets&&family==='all'){if(ftHintFacetDescription(el,'regions'))facetNodes.push([el,'regions']);if(ftHintFacetDescription(el,'surfaces'))facetNodes.push([el,'surfaces']);}const allPolicy=ftHintFamilyPolicy('all',selectorPolicies),allPolicyAllowsSecondary=!allPolicy||allPolicy.mode!=='replace'||ftHintMatchesAny(el,selectors);if(family==='all'&&allPolicyAllowsSecondary&&ftHintMatchesAny(el,scrollSelectors)&&(ftAxisScrollable(el,'x')||ftAxisScrollable(el,'y')))scrollableNodes.push(el);if(proxyEnabled&&(family==='inputs'||family==='all')){if(el.matches('.ace_editor[role=\"textbox\"]'))editors.push(el);if(el.matches('textarea.ace_text-input,input.ace_text-input')){for(let parent=ftParent(el),guard=0;parent&&guard<256;parent=ftParent(parent),guard++)if(parent.matches&&parent.matches('.ace_editor[role=\"textbox\"]')){let pair=editorReceivers.get(parent);if(!pair){pair={count:0,receiver:null};editorReceivers.set(parent,pair);}pair.count++;pair.receiver=el;}}if(el.tagName==='LABEL')labels.push(el);}if(el.shadowRoot)shadows.push(el.shadowRoot);if(el.matches('slot'))slots.push(el);if(el.matches('iframe,frame'))frames.push(el);}if(includeRoot&&root.nodeType===1&&inspect()){scannedPhysicalNodes.add(root);scan(root);}while((node=walker.nextNode())){if(!inspect())break;scannedPhysicalNodes.add(node);if(node.nodeType===1)scan(node);}"
        + "for(const target of normal){if(facet&&truncated)break;if(!facet&&out.length>=candidateLimit){truncated=true;break;}add(target,ox,oy,framePath,topClip,null,null,family);}"
        + "for(const facetEntry of facetNodes){if(out.length>=5000){truncated=true;break;}add(facetEntry[0],ox,oy,framePath,topClip,null,null,facetEntry[1],facetEntry[1]);}"
        + "if(proxyEnabled&&(family==='inputs'||family==='all')&&out.length<5000){const familyPolicy=ftHintFamilyPolicy(family,selectorPolicies),replacePolicy=familyPolicy&&familyPolicy.mode==='replace';for(const editor of editors){if(out.length>=5000){truncated=true;break;}const pair=editorReceivers.get(editor);if(pair&&pair.count===1&&proxyReceiverEligible(pair.receiver,editor)&&(!replacePolicy||ftHintMatchesAny(editor,selectors)))add(editor,ox,oy,framePath,topClip,'editor-focus',pair.receiver,family);}for(const label of labels){if(out.length>=5000){truncated=true;break;}const control=label.control;if(!proxyLabelValid(label,control)||replacePolicy&&!ftHintMatchesAny(label,selectors))continue;const controlId=proxyIdentity(control,state);if(activeIds.has(controlId))continue;if(seen.has(label)){const oldId=state.weak.get(label);for(let index=out.length-1;index>=0;index--)if(out[index].element_id===oldId)out.splice(index,1);elements.delete(oldId);activeIds.delete(oldId);seen.delete(label);}add(label,ox,oy,framePath,topClip,'label-control',control,family);}}"
        + "if(family==='all')for(const target of scrollableNodes){if(out.length>=5000){truncated=true;break;}add(target,ox,oy,framePath,topClip,null,null,'scrollables');}"
        + "for(const shadow of shadows){if(out.length>=5000||inspectedNodes>=inspectionLimit){if(inspectedNodes>=inspectionLimit)truncated=true;break;}visit(shadow,framePath,ox,oy,depth+1,topClip,false);}"
        + "for(const slot of slots){let assigned=[];try{assigned=slot.assignedElements({flatten:true});}catch(error){}for(const assignedElement of assigned){if(scannedPhysicalNodes.has(assignedElement))continue;if(out.length>=5000||inspectedNodes>=inspectionLimit){if(inspectedNodes>=inspectionLimit)truncated=true;break;}visit(assignedElement,framePath,ox,oy,depth+1,topClip,true);}}"
        + "for(let index=0;index<frames.length;index++){const frame=frames[index],frameIndex=ftFrameIndex(frame);if(frameIndex<0)continue;if(out.length>=5000||inspectedNodes>=inspectionLimit||depth>=traversalDepthLimit){if(inspectedNodes>=inspectionLimit)truncated=true;break;}"
        + "const ownerView=frame.ownerDocument&&frame.ownerDocument.defaultView,frameLocal=ftLocalVisible(frame,ownerView,'visible',visibilityStyleCache);if(!frameLocal)continue;const raw=frameLocal.raw,fr=frameLocal.clipped;"
        + "const vw=ownerView.innerWidth,vh=ownerView.innerHeight,mapping=ftFrameMapping(frame,frameLocal,visibilityStyleCache);if(!mapping)continue;const content=ftProjectedRect(frame,{raw:raw,clipped:mapping.content},'visible',visibilityStyleCache);if(!content)continue;const frameClip=intersect(content,topClip);"
        + "if(fr.width<=0||fr.height<=0||frameClip.width<=0||frameClip.height<=0||fr.bottom<=0||fr.right<=0||fr.top>=vh||fr.left>=vw)continue;"
        + "try{if(frame.contentDocument)visit(frame.contentDocument,framePath+'.'+frameIndex,ox+raw.left+frame.clientLeft,oy+raw.top+frame.clientTop,depth+1,frameClip,false);}catch(error){}}}"
        + "let startRoot=document,startPath='0',startX=0,startY=0,startClip={left:0,top:0,right:innerWidth,bottom:innerHeight};if(requestedScopeLeaseId){const lease=state.scopeLease;let scopeRoot=null,scopeDocument=null;try{if(typeof WeakRef!=='function'||!lease||lease.leaseId!==requestedScopeLeaseId)return {error:'stale-scope',candidates:[]};scopeRoot=WeakRef.prototype.deref.call(lease.elementRef);scopeDocument=WeakRef.prototype.deref.call(lease.ownerDocumentRef);}catch(error){return {error:'stale-scope',candidates:[]};}if(!scopeRoot||!scopeDocument||scopeRoot.nodeType!==1||scopeRoot.ownerDocument!==scopeDocument||!scopeRoot.isConnected||!ftReachableFromTopDocument(scopeRoot,scopeDocument)||ftCurrentFramePath(scopeDocument)!==lease.framePath)return {error:'stale-scope',candidates:[]};const scopeDescription=ftHintFacetDescription(scopeRoot,'regions'),scopeLocal=ftLocalVisible(scopeRoot,scopeDocument.defaultView,'visible',visibilityStyleCache),scopeRect=ftProjectedRect(scopeRoot,scopeLocal,'visible',visibilityStyleCache);if(!scopeDescription||scopeDescription.kind!==lease.kind||!scopeRect)return {error:'stale-scope',candidates:[]};startRoot=scopeRoot;startPath=lease.framePath;startClip=scopeRect;let scopeView=scopeDocument.defaultView;for(let scopeDepth=0;scopeView&&scopeView!==window&&scopeDepth<16;scopeDepth++){const frame=scopeView.frameElement;if(!frame||frame.contentWindow!==scopeView||frame.contentDocument!==scopeView.document)return {error:'stale-scope',candidates:[]};const rect=frame.getBoundingClientRect();startX+=rect.left+frame.clientLeft;startY+=rect.top+frame.clientTop;scopeView=frame.ownerDocument&&frame.ownerDocument.defaultView;}if(scopeView!==window)return {error:'stale-scope',candidates:[]};}else if(state.scopeLease)return {error:'scope-lease-required',candidates:[]};visit(startRoot,startPath,startX,startY,0,startClip,false);for(const id of Array.from(elements.keys()))if(!activeIds.has(id)){const record=elements.get(id);if(state.resize&&record)try{state.resize.unobserve(record.element);}catch(error){}elements.delete(id);}state.dirty=false;return {candidates:out,revision:state.revision,selector_revision:selectorRevision,scope_lease_id:requestedScopeLeaseId||null,inspected_nodes:inspectedNodes,truncated:truncated,viewport:{width:innerWidth,height:innerHeight}};})()"
}

// Retains an exact parent-visible iframe independently of the semantic
// collector. The native broker supplies the child viewport only after focus
// establishes a unique association. This script never reads a foreign DOM.
function hintFrameHost(leaseId, viewport, point) {
    return "(()=>{try{" + scrollTargetRuntime()
        + "const lease=" + JSON.stringify(String(leaseId || ""))
        + ",viewport=" + JSON.stringify(viewport || null)
        + ",point=" + JSON.stringify(point || null) + ";"
        + `let hosts=window.__ferric_hintFrameHosts;if(!hosts){hosts=new Map();window.__ferric_hintFrameHosts=hosts;}
        let active=document.activeElement;
        for(let depth=0;active&&active.shadowRoot&&depth<8;depth++){const inner=active.shadowRoot.activeElement;if(!inner||inner===active)break;active=inner;}
        if(!document.hasFocus())return {error:'frame-focus-lost'};
        let frame=active,key=lease;
        if(key){const record=hosts.get(key);frame=record&&record.element.deref();if(!frame||record.document.deref()!==document||active!==frame)return {error:'frame-focus-changed'};}
        if(!frame||!frame.matches('iframe,frame'))return {none:true};
        if(frame.ownerDocument!==document||!frame.isConnected||ftDisabled(frame)||ftInert(frame))return {error:'invalid-frame-host'};
        if(frame.hasAttribute('sandbox')&&!frame.sandbox.contains('allow-scripts'))return {error:'sandbox-script-restriction'};
        const local=ftLocalVisible(frame,window,'pointer');if(!local)return {error:'frame-host-covered'};
        if(!key){for(const entry of hosts){if(entry[1].element.deref()===frame){key=entry[0];break;}}
            if(!key){if(hosts.size>=64||typeof WeakRef!=='function')return {error:'frame-host-limit'};key=String(hosts.size+1);hosts.set(key,{element:new WeakRef(frame),document:new WeakRef(document)});}}
        const mapping=viewport?ftFrameMapping(frame,local,null,viewport):null;
        if(viewport&&!mapping)return {error:'unsupported-frame-geometry'};
        if(point){if(!mapping||point.x<mapping.content.left||point.y<mapping.content.top||point.x>=mapping.content.right||point.y>=mapping.content.bottom||!ftContains(frame,ftDeepHit(document,point.x,point.y)))return {error:'frame-target-covered'};}
        let modal=false;for(let node=frame,depth=0;node&&depth<256;node=ftParent(node),depth++)if(node.matches('[aria-modal=true],dialog[open],[role=dialog],[role=alertdialog]')){modal=true;break;}
        if(mapping){delete mapping.parentView;mapping.viewportWidth=viewport.width;mapping.viewportHeight=viewport.height;}
        return {host:key,cross_origin:frame.contentDocument===null,modal:modal,mapping:mapping,viewport:{width:innerWidth,height:innerHeight}};
        }catch(error){return {error:'frame-host-unavailable'};}})()`
}

function hintDirtyRevision() {
    return "(function(){const s=window.__ferric_browserHintState;return s&&s.tracking?{dirty:s.dirty,revision:s.revision}:{dirty:false,revision:0};})()"
}

function hintStopTracking(preserveScopeLeaseId) {
    var preserve = typeof preserveScopeLeaseId === "string" && preserveScopeLeaseId.length <= 128
            ? preserveScopeLeaseId : ""
    return "(function(){const s=window.__ferric_browserHintState;if(!s)return true;const keepScope=!!(s.scopeLease&&s.scopeLease.leaseId===" + JSON.stringify(preserve) + ");if(s.dirtyHandler)for(const view of s.windows||[])try{view.removeEventListener('scroll',s.dirtyHandler,true);view.removeEventListener('resize',s.dirtyHandler,true);}catch(error){}for(const o of s.observers||[])try{o.disconnect();}catch(error){}if(s.resize)try{s.resize.disconnect();}catch(error){}if(s.elements&&typeof s.elements.clear==='function')s.elements.clear();try{delete window.__ferric_browserHintElements;}catch(error){window.__ferric_browserHintElements=null;}s.observers=[];s.windows=[];s.roots=new WeakSet();s.resize=null;s.dirtyHandler=null;s.tracking=false;s.dirty=false;if(!keepScope&&s.scopeLease){try{const target=WeakRef.prototype.deref.call(s.scopeLease.elementRef);if(target)target.removeEventListener('scroll',s.scopeLease.scrollHandler,true);}catch(error){}for(const view of s.scopeLease.scrollViews||[])try{view.removeEventListener('scroll',s.scopeLease.scrollHandler,true);view.removeEventListener('resize',s.scopeLease.scrollHandler,true);}catch(error){}for(const doc of s.scopeLease.scrollDocuments||[])try{doc.removeEventListener('scroll',s.scopeLease.scrollHandler,true);}catch(error){}s.scopeLease=null;}return true;})()"
}

function hintAcquireScopeLease(request, candidate, selectorSnapshot) {
    var snapshot = normalizedHintPolicySnapshot(selectorSnapshot)
    if (snapshot.error)
        return "(function(){return {ok:false,error:'invalid-selector-policy'};})()"
    var linkSelector = hintSelector(true)
    var freshness = hintFresh(candidate || {}, selectorSnapshot)
    return "(function(){try{" + scrollTargetRuntime() + hintPolicyRuntime() + hintProxyRuntime()
        + "const linkSelector=" + JSON.stringify(linkSelector) + ";const request=" + JSON.stringify(request || {})
        + ",candidate=" + JSON.stringify(candidate || {}) + ",fresh=" + freshness
        + ";const state=window.__ferric_browserHintState,elements=state&&state.elements,record=elements&&elements.get(Number(request.element_id)),element=record&&record.element;"
        + "const fields=['lease_id','element_id','frame_path','kind','selector_revision','configuration_revision','parent_scope_lease_id'];if(!request||typeof request!=='object'||Object.keys(request).length!==fields.length||Object.keys(request).some(function(key){return fields.indexOf(key)<0;})||typeof request.lease_id!=='string'||request.lease_id.length<1||request.lease_id.length>128||!Number.isInteger(request.element_id)||request.element_id<=0||typeof request.frame_path!=='string'||request.frame_path.length<1||request.frame_path.length>256||!['region','scrollable-region','surface','focusable-surface','frame'].includes(request.kind)||typeof request.selector_revision!=='string'||request.selector_revision.length!==16||!Number.isSafeInteger(request.configuration_revision)||request.configuration_revision<0||!(request.parent_scope_lease_id===null||typeof request.parent_scope_lease_id==='string'&&request.parent_scope_lease_id.length<=128))return {ok:false,error:'invalid-scope-request'};"
        + "const activeId=state&&state.scopeLease?state.scopeLease.leaseId:null,scopeFamily=request.kind==='surface'||request.kind==='focusable-surface'||request.kind==='frame'?'surfaces':'regions';if((request.parent_scope_lease_id||null)!==activeId||(record&&record.scopeLeaseId||null)!==activeId||!record||(record.family!==scopeFamily&&!(request.kind==='frame'&&record.family==='all'))||record.kind!==request.kind||record.framePath!==request.frame_path||record.selectorRevision!==request.selector_revision||!fresh.visible||fresh.element_id!==request.element_id||fresh.kind!==request.kind||fresh.frame_path!==request.frame_path||candidate.element_id!==request.element_id||candidate.kind!==request.kind||candidate.frame_path!==request.frame_path||fresh.text!==record.text||fresh.href!==record.href)return {ok:false,error:'stale-scope'};"
        + "if(typeof WeakRef!=='function'||typeof WeakRef.prototype.deref!=='function')return {ok:false,error:'weak-reference-unavailable'};const description=ftHintFacetDescription(element,scopeFamily);if(!element||!element.isConnected||!description||description.kind!==request.kind||!ftMeaningfullyVisible(element,'visible')||!ftReachableFromTopDocument(element,record.ownerDocument)||ftCurrentFramePath(record.ownerDocument)!==request.frame_path)return {ok:false,error:'stale-scope'};"
        + "let elementRef,ownerDocumentRef;try{elementRef=new WeakRef(element);ownerDocumentRef=new WeakRef(record.ownerDocument);}catch(error){return {ok:false,error:'weak-reference-unavailable'};}const releasePreviousScope=function(lease){if(!lease)return;try{const target=WeakRef.prototype.deref.call(lease.elementRef);if(target)target.removeEventListener('scroll',lease.scrollHandler,true);}catch(error){}for(const view of lease.scrollViews||[])try{view.removeEventListener('scroll',lease.scrollHandler,true);view.removeEventListener('resize',lease.scrollHandler,true);}catch(error){}for(const doc of lease.scrollDocuments||[])try{doc.removeEventListener('scroll',lease.scrollHandler,true);}catch(error){}};releasePreviousScope(state.scopeLease);state.scopeLease={leaseId:request.lease_id,elementRef:elementRef,ownerDocumentRef:ownerDocumentRef,elementId:request.element_id,framePath:request.frame_path,kind:request.kind,family:scopeFamily,selectorRevision:request.selector_revision,configurationRevision:request.configuration_revision,parentScopeLeaseId:request.parent_scope_lease_id,scrollRevision:0,scrollViews:[],scrollDocuments:[],scrollHandler:null};const bumpScroll=function(){state.scopeLease.scrollRevision=Math.min(Number.MAX_SAFE_INTEGER,state.scopeLease.scrollRevision+1);};state.scopeLease.scrollHandler=bumpScroll;try{element.addEventListener('scroll',bumpScroll,true);}catch(error){return {ok:false,error:'scope-listener-unavailable'};}let scopeView=record.ownerDocument.defaultView;for(let scopeDepth=0;scopeView&&scopeDepth<16;scopeDepth++){const scopeDocument=scopeView.document;if(scopeDocument&&state.scopeLease.scrollDocuments.indexOf(scopeDocument)<0){try{scopeDocument.addEventListener('scroll',bumpScroll,true);state.scopeLease.scrollDocuments.push(scopeDocument);}catch(error){return {ok:false,error:'scope-listener-unavailable'};}}if(state.scopeLease.scrollViews.indexOf(scopeView)<0){try{scopeView.addEventListener('scroll',bumpScroll,true);scopeView.addEventListener('resize',bumpScroll,true);state.scopeLease.scrollViews.push(scopeView);}catch(error){return {ok:false,error:'scope-listener-unavailable'};}}if(scopeView===window)break;const frame=scopeView.frameElement;if(!frame||frame.contentWindow!==scopeView||frame.contentDocument!==scopeView.document)return {ok:false,error:'stale-scope'};scopeView=frame.ownerDocument&&frame.ownerDocument.defaultView;}if(scopeView!==window)return {ok:false,error:'stale-scope'};return {ok:true,lease_id:request.lease_id,element_id:request.element_id,frame_path:request.frame_path,kind:request.kind,selector_revision:request.selector_revision,configuration_revision:request.configuration_revision,parent_scope_lease_id:request.parent_scope_lease_id};"
        + "}catch(error){return {ok:false,error:'scope-lease-invalid'};}})()"
}

function hintClearScopeLease(scopeLeaseId) {
    var leaseId = typeof scopeLeaseId === "string" && scopeLeaseId.length <= 128
            ? scopeLeaseId : ""
    return "(function(){const s=window.__ferric_browserHintState;if(!s||!s.scopeLease||s.scopeLease.leaseId!=="
        + JSON.stringify(leaseId) + ")return false;try{const target=WeakRef.prototype.deref.call(s.scopeLease.elementRef);if(target)target.removeEventListener('scroll',s.scopeLease.scrollHandler,true);}catch(error){}for(const view of s.scopeLease.scrollViews||[])try{view.removeEventListener('scroll',s.scopeLease.scrollHandler,true);view.removeEventListener('resize',s.scopeLease.scrollHandler,true);}catch(error){}for(const doc of s.scopeLease.scrollDocuments||[])try{doc.removeEventListener('scroll',s.scopeLease.scrollHandler,true);}catch(error){}s.scopeLease=null;return true;})()"
}

function spatialScopeGeometry(scopeLeaseId) {
    var leaseId = typeof scopeLeaseId === "string" && scopeLeaseId.length <= 128
            ? scopeLeaseId : ""
    return "(function(){try{" + scrollTargetRuntime() + hintPolicyRuntime()
        + "const requestedLeaseId=" + JSON.stringify(leaseId)
        + ";const state=window.__ferric_browserHintState,lease=state&&state.scopeLease;"
        + "if(!requestedLeaseId||!lease||lease.leaseId!==requestedLeaseId||typeof WeakRef!=='function'||typeof WeakRef.prototype.deref!=='function')return {ok:false};"
        + "let element,ownerDocument;try{element=WeakRef.prototype.deref.call(lease.elementRef);ownerDocument=WeakRef.prototype.deref.call(lease.ownerDocumentRef);}catch(error){return {ok:false};}"
        + "if(!element||!ownerDocument||element.nodeType!==1||element.ownerDocument!==ownerDocument||!element.isConnected||!ftReachableFromTopDocument(element,ownerDocument)||ftCurrentFramePath(ownerDocument)!==lease.framePath||!Number.isSafeInteger(lease.elementId)||lease.elementId<=0)return {ok:false};"
        + "const description=ftHintFacetDescription(element,lease.family);if(!description||description.kind!==lease.kind||!['regions','surfaces'].includes(lease.family))return {ok:false};"
        + "let currentView=ownerDocument.defaultView,currentElement=element,guard=0;while(currentView&&guard<16){for(let node=currentElement;node;node=ftParent(node)){if(node.nodeType===1){const nodeView=node.ownerDocument&&node.ownerDocument.defaultView||currentView,style=nodeView.getComputedStyle(node);if(style.transform!=='none'||style.perspective!=='none')return {ok:false,error:'unsupported-geometry'};}}if(currentView===window)break;const frame=currentView.frameElement;if(!frame||frame.contentWindow!==currentView||frame.contentDocument!==currentView.document)return {ok:false};currentElement=frame;currentView=frame.ownerDocument&&frame.ownerDocument.defaultView;guard++;}if(currentView!==window)return {ok:false};"
        + "const local=ftLocalVisible(element,ownerDocument.defaultView,'visible');if(!local)return {ok:false};let projected=local.clipped;"
        + "if((element.localName==='iframe'||element.localName==='frame')&&lease.family==='surfaces'){const raw=local.raw,content={left:raw.left+element.clientLeft,top:raw.top+element.clientTop,right:raw.left+element.clientLeft+element.clientWidth,bottom:raw.top+element.clientTop+element.clientHeight};projected=ftIntersect(projected,content);if(!(projected.width>0&&projected.height>0))return {ok:false};}"
        + "let view=ownerDocument.defaultView;for(let depth=0;view!==window&&depth<16;depth++){const childView=view,frame=childView.frameElement,parentView=frame&&frame.ownerDocument&&frame.ownerDocument.defaultView;if(!frame||!parentView||frame.contentWindow!==childView||frame.contentDocument!==childView.document)return {ok:false};const frameLocal=ftLocalVisible(frame,parentView,'visible');if(!frameLocal||!(childView.innerWidth>0&&childView.innerHeight>0&&frame.clientWidth>0&&frame.clientHeight>0&&frame.offsetWidth>0&&frame.offsetHeight>0))return {ok:false};const raw=frameLocal.raw,frameScaleX=raw.width/frame.offsetWidth,frameScaleY=raw.height/frame.offsetHeight;if(!Number.isFinite(frameScaleX)||!Number.isFinite(frameScaleY)||frameScaleX<0.2||frameScaleX>5||frameScaleY<0.2||frameScaleY>5)return {ok:false,error:'unsupported-geometry'};const sx=frame.clientWidth*frameScaleX/childView.innerWidth,sy=frame.clientHeight*frameScaleY/childView.innerHeight;if(!Number.isFinite(sx)||!Number.isFinite(sy)||sx<0.2||sx>5||sy<0.2||sy>5)return {ok:false,error:'unsupported-geometry'};const left=raw.left+frame.clientLeft*frameScaleX,top=raw.top+frame.clientTop*frameScaleY,content={left:left,top:top,right:left+frame.clientWidth*frameScaleX,bottom:top+frame.clientHeight*frameScaleY},mapped={left:left+projected.left*sx,top:top+projected.top*sy,right:left+projected.right*sx,bottom:top+projected.bottom*sy};projected=ftIntersect(ftIntersect(mapped,content),frameLocal.clipped);if(!(projected.width>0&&projected.height>0))return {ok:false};view=parentView;}"
        + "projected=ftIntersect(projected,{left:0,top:0,right:window.innerWidth,bottom:window.innerHeight});if(!(projected.width>0&&projected.height>0))return {ok:false,error:'empty-scope-geometry',geometry:{x:projected.left,y:projected.top,width:projected.width,height:projected.height},viewport:{width:window.innerWidth,height:window.innerHeight}};if(!Number.isSafeInteger(lease.configurationRevision)||!Number.isSafeInteger(lease.scrollRevision)||lease.scrollRevision<0)return {ok:false,error:'invalid-scope-revision'};"
        + "return {ok:true,lease_id:lease.leaseId,element_id:lease.elementId,frame_path:lease.framePath,kind:lease.kind,selector_revision:lease.selectorRevision,configuration_revision:lease.configurationRevision,scroll_revision:lease.scrollRevision,viewport_width:window.innerWidth,viewport_height:window.innerHeight,geometry:{x:projected.left,y:projected.top,width:projected.width,height:projected.height}};"
        + "}catch(error){return {ok:false};}})()"
}

function hintFresh(candidate, selectorSnapshot) {
    var linkSelector = hintSelector(true)
    var relatedElementId = typeof candidate.related_element_id === "number" ? candidate.related_element_id : null
    var snapshot = normalizedHintPolicySnapshot(selectorSnapshot)
    if (snapshot.error)
        return "(function(){return {visible:false};})()"
    var activeSelectorRevision = typeof snapshot.selector_revision === "string"
            ? snapshot.selector_revision : null
    return "(function(){try{" + scrollTargetRuntime()
        + "const elementId=" + Number(candidate.element_id) + ",path=" + JSON.stringify(candidate.frame_path || "0")
        + ",candidateKind=" + JSON.stringify(candidate.kind || "") + ",candidateRelatedId=" + JSON.stringify(relatedElementId) + ",linkSelector=" + JSON.stringify(linkSelector)
        + ",activeSelectorRevision=" + JSON.stringify(activeSelectorRevision) + ";"
        + hintPolicyRuntime()
        + hintProxyRuntime()
        + "const state=window.__ferric_browserHintState,elements=state&&state.elements,record=elements&&elements.get(elementId),el=record&&record.element;"
        + "if(!record||!el||record.framePath!==path||record.ownerDocument!==el.ownerDocument||!ftReachableFromTopDocument(el,record.ownerDocument)||ftCurrentFramePath(el.ownerDocument)!==path)return {visible:false,element_id:elementId};const activeScopeLeaseId=state.scopeLease?state.scopeLease.leaseId:null;if((record.scopeLeaseId||null)!==activeScopeLeaseId)return {visible:false,element_id:elementId};if(activeScopeLeaseId){let scopeRoot=null,scopeDocument=null;try{scopeRoot=WeakRef.prototype.deref.call(state.scopeLease.elementRef);scopeDocument=WeakRef.prototype.deref.call(state.scopeLease.ownerDocumentRef);}catch(error){return {visible:false,element_id:elementId};}const scopeDescription=scopeRoot&&ftHintFacetDescription(scopeRoot,'regions');if(!scopeRoot||!scopeDocument||scopeRoot.ownerDocument!==scopeDocument||!scopeRoot.isConnected||!scopeDescription||scopeDescription.kind!==state.scopeLease.kind||!ftReachableFromTopDocument(scopeRoot,scopeDocument)||ftCurrentFramePath(scopeDocument)!==state.scopeLease.framePath||!ftWithinScope(scopeRoot,scopeDocument,el))return {visible:false,element_id:elementId};}"
        + "if(!ftHintRevisionMatches(record,activeSelectorRevision)||!ftHintRecordMatchesPolicy(record,el))return {visible:false,element_id:elementId};"
        + "const pairKind=record.kind==='editor-focus'||record.kind==='label-control';if(pairKind&&!proxyPairValid(record,{related_element_id:candidateRelatedId},state))return {visible:false,element_id:elementId};"
        + "const maybeScrollable=ftAxisScrollable(el,'x')||ftAxisScrollable(el,'y');if((record.kind==='scrollable'||record.kind==='scrollable-region')&&!maybeScrollable)return {visible:false,element_id:elementId};"
        + "if(record.family==='scrollables'&&!maybeScrollable)return {visible:false,element_id:elementId};"
        + "const description=pairKind?{kind:record.kind,text:proxyText(el,record.kind),href:null}:ftHintDescription(el,record.family,linkSelector);if(description.kind!==record.kind||description.text!==record.text||description.href!==record.href||candidateKind&&candidateKind!==description.kind)return {visible:false,element_id:elementId};"
        + "const policy=ftHintPolicy(description.kind,record.family),ownerView=el.ownerDocument&&el.ownerDocument.defaultView,local=ftLocalVisible(el,ownerView,policy),geometry=ftProjectedRect(el,local,policy);if(!geometry)return {visible:false,element_id:elementId};"
        + "return {visible:true,element_id:elementId,related_element_id:record.relatedElementId,kind:description.kind,frame_path:path,text:description.text,href:description.href,geometry:{x:geometry.left,y:geometry.top,width:geometry.width,height:geometry.height}};"
        + "}catch(error){return {visible:false,element_id:" + Number(candidate.element_id) + "};}})()"
}

function hintPointerPreflight(candidate, viewWidth, viewHeight, selectorSnapshot) {
    var actionCandidate = Object.assign({}, candidate || {})
    if (typeof actionCandidate.href === "undefined")
        actionCandidate.href = null
    if (typeof actionCandidate.related_element_id === "undefined")
        actionCandidate.related_element_id = null
    var elementId = Number(actionCandidate.element_id)
    var width = Number(viewWidth)
    var height = Number(viewHeight)
    var linkSelector = hintSelector(true)
    var actionSelector = hintSelector(false)
    var snapshot = normalizedHintPolicySnapshot(selectorSnapshot)
    if (snapshot.error)
        return "(function(){return {ok:false};})()"
    var activeSelectorRevision = typeof snapshot.selector_revision === "string"
            ? snapshot.selector_revision : null
    return "(function(){try{" + scrollTargetRuntime()
        + "const candidate=" + JSON.stringify(actionCandidate) + ",elementId=" + elementId
        + ",viewWidth=" + width + ",viewHeight=" + height
        + ",linkSelector=" + JSON.stringify(linkSelector)
        + ",actionSelector=" + JSON.stringify(actionSelector)
        + ",activeSelectorRevision=" + JSON.stringify(activeSelectorRevision) + ";"
        + hintPolicyRuntime()
        + hintProxyRuntime()
        + "if(!Number.isFinite(viewWidth)||!Number.isFinite(viewHeight)||viewWidth<=0||viewHeight<=0||!candidate||!Number.isInteger(candidate.element_id)||candidate.element_id!==elementId)return {ok:false};"
        + "const state=window.__ferric_browserHintState,elements=state&&state.elements,record=elements&&elements.get(elementId),el=record&&record.element;"
        + "if(!record||!el||!el.isConnected||record.family==='regions'||record.family==='surfaces'||record.framePath!==candidate.frame_path||record.ownerDocument!==el.ownerDocument||!ftReachableFromTopDocument(el,record.ownerDocument)||ftCurrentFramePath(el.ownerDocument)!==candidate.frame_path)return {ok:false};"
        + "if(!ftHintRevisionMatches(record,activeSelectorRevision)||!ftHintRecordMatchesPolicy(record,el))return {ok:false};"
        + "const pairKind=record.kind==='editor-focus'||record.kind==='label-control';if(pairKind&&!proxyPairValid(record,candidate,state))return {ok:false};const description=pairKind?{kind:record.kind,text:proxyText(el,record.kind),href:null}:ftHintDescription(el,record.family,linkSelector);if(description.kind!==record.kind||description.text!==record.text||description.href!==record.href||candidate.kind!==description.kind||candidate.text!==description.text||candidate.href!==description.href||candidate.related_element_id!==record.relatedElementId)return {ok:false};"
        + "if(!pairKind&&!el.matches(actionSelector))return {ok:false};"
        + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView,local=ftLocalVisible(el,ownerView,'pointer'),geometry=ftProjectedRect(el,local,'pointer');if(!geometry||!(window.innerWidth>0&&window.innerHeight>0))return {ok:false};"
        + "const insetX=Math.min(12,geometry.width*0.2),insetY=Math.min(12,geometry.height*0.2),points=[[geometry.left+geometry.width/2,geometry.top+geometry.height/2],[geometry.left+insetX,geometry.top+insetY],[geometry.right-insetX,geometry.top+insetY],[geometry.left+insetX,geometry.bottom-insetY],[geometry.right-insetX,geometry.bottom-insetY]];"
        + "let point=null;for(const sample of points){const hit=ftPointerDeepHit(document,sample[0],sample[1]);if(hit&&ftContains(el,hit)){point=sample;break;}}if(!point)return {ok:false};"
        + "const scaleX=viewWidth/window.innerWidth,scaleY=viewHeight/window.innerHeight;if(!Number.isFinite(scaleX)||!Number.isFinite(scaleY)||scaleX<0.2||scaleX>5||scaleY<0.2||scaleY>5)return {ok:false};"
        + "return {ok:true,candidate:{visible:true,element_id:elementId,related_element_id:record.relatedElementId,kind:description.kind,frame_path:candidate.frame_path,text:description.text,href:description.href,geometry:{x:geometry.left,y:geometry.top,width:geometry.width,height:geometry.height}},css_x:point[0],css_y:point[1],x:point[0]*scaleX,y:point[1]*scaleY,viewport_width:window.innerWidth,viewport_height:window.innerHeight,view_width:viewWidth,view_height:viewHeight};"
        + "}catch(error){return {ok:false};}})()"
}

function hintFocus(candidate, selectorSnapshot) {
    var actionCandidate = Object.assign({}, candidate || {})
    if (typeof actionCandidate.href === "undefined")
        actionCandidate.href = null
    if (typeof actionCandidate.related_element_id === "undefined")
        actionCandidate.related_element_id = null
    var elementId = Number(actionCandidate.element_id)
    var freshness = hintFresh(actionCandidate, selectorSnapshot)
    return "(function(){try{const fresh=" + freshness + ",candidate=" + JSON.stringify(actionCandidate) + ";"
        + "if(!fresh.visible||fresh.element_id!==candidate.element_id||fresh.related_element_id!==candidate.related_element_id||fresh.kind!==candidate.kind||fresh.frame_path!==candidate.frame_path||fresh.text!==candidate.text||fresh.href!==candidate.href)return false;"
        + "const state=window.__ferric_browserHintState,record=state&&state.elements&&state.elements.get(" + elementId + "),el=record&&record.element;"
        + scrollTargetRuntime()
        + hintProxyRuntime()
        + "if(candidate.kind==='editor-focus'){if(!proxyPairValid(record,candidate,state))return false;const receiver=record.relatedElement,ownerView=receiver.ownerDocument&&receiver.ownerDocument.defaultView;if(!ownerView)return false;if(typeof ownerView.__ferric_browserAuthorizeExplicitFocus==='function')ownerView.__ferric_browserAuthorizeExplicitFocus();receiver.focus();let active=receiver.ownerDocument.activeElement;for(let depth=0;active&&active.shadowRoot&&depth<32;depth++){const inner=active.shadowRoot.activeElement;if(!inner||inner===active)break;active=inner;}return active===receiver;}"
        + "if(!el||(candidate.kind==='focusable-surface'||candidate.kind==='focusable'||candidate.kind==='frame')&&!ftFocusEligible(el)||(candidate.kind!=='focusable-surface'&&candidate.kind!=='focusable'&&candidate.kind!=='frame')&&!el.matches('input,select,textarea,[contenteditable]:not([contenteditable=false])'))return false;"
        + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView;if(!ownerView)return false;"
        + "if(typeof ownerView.__ferric_browserAuthorizeExplicitFocus==='function')ownerView.__ferric_browserAuthorizeExplicitFocus();el.focus();"
        + "let active=el.ownerDocument.activeElement;for(let depth=0;active&&active.shadowRoot&&depth<32;depth++){const inner=active.shadowRoot.activeElement;if(!inner||inner===active)break;active=inner;}return active===el;"
        + "}catch(error){return false;}})()"
}

function hintClick(candidate, selectorSnapshot) {
    var actionCandidate = Object.assign({}, candidate || {})
    if (typeof actionCandidate.href === "undefined")
        actionCandidate.href = null
    if (typeof actionCandidate.related_element_id === "undefined")
        actionCandidate.related_element_id = null
    var elementId = Number(actionCandidate.element_id)
    var selector = hintSelector(false)
    var freshness = hintFresh(actionCandidate, selectorSnapshot)
    return "(function(){try{const fresh=" + freshness + ",candidate=" + JSON.stringify(actionCandidate) + ";"
        + "if(!fresh.visible||fresh.element_id!==candidate.element_id||fresh.related_element_id!==candidate.related_element_id||fresh.kind!==candidate.kind||fresh.frame_path!==candidate.frame_path||fresh.text!==candidate.text||fresh.href!==candidate.href)return false;"
        + "const state=window.__ferric_browserHintState,record=state&&state.elements&&state.elements.get(" + elementId + "),el=record&&record.element;"
        + scrollTargetRuntime()
        + hintProxyRuntime()
        + "if(candidate.kind==='label-control'){if(!proxyPairValid(record,candidate,state)||typeof el.click!=='function')return false;const control=record.relatedElement,before=!!control.checked,radio=String(control.type).toLowerCase()==='radio';el.click();return !!control.checked===(radio||!before);}"
        + "if(!el||record.family==='regions'||record.family==='surfaces'||!el.matches(" + JSON.stringify(selector) + ")||typeof el.click!=='function'||candidate.kind==='link'&&!ftMeaningfullyVisible(el,'pointer'))return false;el.click();return true;"
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
