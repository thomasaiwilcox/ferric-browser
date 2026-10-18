.pragma library

// These scripts run only in a fixed browser-owned document and isolated world.
function canonicalTarget(controller, url) {
    return "(()=>{if(location.href!==" + JSON.stringify(controller) + ")return '';const u=new URL("
        + JSON.stringify(url) + ");u.hash='';return u.href})()"
}
function start(controller, url) {
    return "(()=>{if(location.href!==" + JSON.stringify(controller) + ")return false;"
        + "if(navigator.serviceWorker&&navigator.serviceWorker.controller)return false;"
        + "const s=window.__ferricDownload={phase:'fetching',busy:false,reader:null,pending:null,offset:0,eof:false,error:'',status:0};"
        + "s.abort=new AbortController();fetch(" + JSON.stringify(url)
        + ",{credentials:'include',referrerPolicy:'no-referrer',cache:'no-store',signal:s.abort.signal}).then(r=>{"
        + "s.status=r.status;if(!r.ok){s.phase='error';return}"
        + "s.mime=(r.headers.get('Content-Type')||'').slice(0,256);s.disposition=(r.headers.get('Content-Disposition')||'').slice(0,8192);"
        + "const length=r.headers.get('Content-Length');const encoding=r.headers.get('Content-Encoding');"
        + "s.total=(!encoding||encoding==='identity')&&/^\\d+$/.test(length||'')&&Number.isSafeInteger(Number(length))?Number(length):-1;"
        + "s.reader=r.body?r.body.getReader():null;s.eof=!s.reader;s.phase='headers';"
        + "}).catch(e=>{s.error=e.name;s.phase='error'});return true})()"
}
function metadata(controller) {
    return "(()=>{if(location.href!==" + JSON.stringify(controller) + ")return null;const s=window.__ferricDownload;"
        + "return s?{phase:s.phase,status:s.status,total:s.total,mime:s.mime,disposition:s.disposition,error:s.error}:null})()"
}
function chunk(controller) {
    return "(()=>{if(location.href!==" + JSON.stringify(controller) + ")return {error:'changed'};const s=window.__ferricDownload;"
        + "if(!s||s.phase==='error')return {error:s?s.error:'missing'};"
        + "if(s.pending&&s.offset<s.pending.byteLength){const b=s.pending.slice(s.offset,s.offset+65536).buffer;s.offset+=b.byteLength;return b}"
        + "s.pending=null;s.offset=0;if(s.eof)return new ArrayBuffer(0);"
        + "if(!s.busy){s.busy=true;s.reader.read().then(p=>{s.eof=p.done;s.pending=p.value||null;s.busy=false})"
        + ".catch(e=>{s.error=e.name;s.phase='error';s.busy=false})}return false})()"
}
function abort() {
    return "(()=>{const s=window.__ferricDownload;if(s){s.abort.abort();if(s.reader)s.reader.cancel().catch(()=>{});s.pending=null}return true})()"
}
