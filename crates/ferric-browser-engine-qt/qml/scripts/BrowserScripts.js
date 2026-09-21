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
