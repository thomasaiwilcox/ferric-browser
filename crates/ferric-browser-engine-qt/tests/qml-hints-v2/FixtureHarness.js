.pragma library

var BASELINE_COMMIT = "30e9daf34079186dd5775d5f9c18b26c40176344"
var PAGE_INTERACTION_BLOB = "4a744811798d440970e1d16844e72813b69f2e89"
var BROWSER_SCRIPTS_BLOB = "c1a001f901fb441c8638014a85d8d52ba65a5d90"


function fileUrl(path) {
    var value = String(path)
    if (/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(value))
        return value
    if (value.charAt(0) !== "/")
        value = "/" + value
    return "file://" + value
}

function readText(url, callback) {
    var request = new XMLHttpRequest()
    var finished = false
    request.onreadystatechange = function() {
        if (finished || request.readyState !== 4)
            return
        finished = true
        if ((request.status === 0 || request.status === 200) && request.responseText.length > 0)
            callback(null, request.responseText)
        else
            callback("read failed (status " + request.status + ") for " + url, "")
    }
    try {
        request.open("GET", String(url), true)
        request.send()
    } catch (error) {
        if (!finished) {
            finished = true
            callback(String(error), "")
        }
    }
}

function escapeAttribute(value) {
    return String(value).replace(/&/g, "&amp;")
            .replace(/"/g, "&quot;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
}

function composeFixture(mainHtml, frameHtml) {
    var marker = "__HV2_FRAME_SRC_DOC__"
    var source = String(mainHtml)
    if (source.indexOf(marker) < 0 || source.indexOf(marker) !== source.lastIndexOf(marker))
        throw new Error("basic.html must contain exactly one frame fixture placeholder")
    return source.replace(marker, escapeAttribute(frameHtml))
}

function utf8Bytes(value) {
    var text = String(value)
    var bytes = []
    for (var index = 0; index < text.length; index++) {
        var code = text.charCodeAt(index)
        if (code >= 0xd800 && code <= 0xdbff) {
            if (index + 1 >= text.length)
                throw new Error("unpaired UTF-16 surrogate in oracle source")
            var low = text.charCodeAt(++index)
            if (low < 0xdc00 || low > 0xdfff)
                throw new Error("unpaired UTF-16 surrogate in oracle source")
            code = 0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00)
        } else if (code >= 0xdc00 && code <= 0xdfff) {
            throw new Error("unpaired UTF-16 surrogate in oracle source")
        }
        if (code <= 0x7f) {
            bytes.push(code)
        } else if (code <= 0x7ff) {
            bytes.push(0xc0 | (code >> 6), 0x80 | (code & 0x3f))
        } else if (code <= 0xffff) {
            bytes.push(0xe0 | (code >> 12), 0x80 | ((code >> 6) & 0x3f), 0x80 | (code & 0x3f))
        } else {
            bytes.push(0xf0 | (code >> 18), 0x80 | ((code >> 12) & 0x3f),
                       0x80 | ((code >> 6) & 0x3f), 0x80 | (code & 0x3f))
        }
    }
    return bytes
}

function rotateLeft(value, count) {
    return (value << count) | (value >>> (32 - count))
}

function gitBlobSha1(text) {
    var bytes = utf8Bytes(text)
    var header = utf8Bytes("blob " + bytes.length + "\u0000")
    bytes = header.concat(bytes)
    var bitLength = bytes.length * 8
    bytes.push(0x80)
    while ((bytes.length % 64) !== 56)
        bytes.push(0)
    var high = Math.floor(bitLength / 0x100000000)
    var low = bitLength >>> 0
    bytes.push((high >>> 24) & 0xff, (high >>> 16) & 0xff, (high >>> 8) & 0xff, high & 0xff,
               (low >>> 24) & 0xff, (low >>> 16) & 0xff, (low >>> 8) & 0xff, low & 0xff)

    var h0 = 0x67452301
    var h1 = 0xefcdab89
    var h2 = 0x98badcfe
    var h3 = 0x10325476
    var h4 = 0xc3d2e1f0
    for (var offset = 0; offset < bytes.length; offset += 64) {
        var words = new Array(80)
        for (var word = 0; word < 16; word++) {
            var start = offset + word * 4
            words[word] = ((bytes[start] << 24) | (bytes[start + 1] << 16)
                           | (bytes[start + 2] << 8) | bytes[start + 3]) >>> 0
        }
        for (word = 16; word < 80; word++)
            words[word] = rotateLeft(words[word - 3] ^ words[word - 8] ^ words[word - 14] ^ words[word - 16], 1) >>> 0
        var a = h0, b = h1, c = h2, d = h3, e = h4
        for (var round = 0; round < 80; round++) {
            var f, k
            if (round < 20) {
                f = (b & c) | ((~b) & d); k = 0x5a827999
            } else if (round < 40) {
                f = b ^ c ^ d; k = 0x6ed9eba1
            } else if (round < 60) {
                f = (b & c) | (b & d) | (c & d); k = 0x8f1bbcdc
            } else {
                f = b ^ c ^ d; k = 0xca62c1d6
            }
            var temp = (rotateLeft(a, 5) + f + e + k + words[round]) >>> 0
            e = d; d = c; c = rotateLeft(b, 30) >>> 0; b = a; a = temp
        }
        h0 = (h0 + a) >>> 0; h1 = (h1 + b) >>> 0; h2 = (h2 + c) >>> 0
        h3 = (h3 + d) >>> 0; h4 = (h4 + e) >>> 0
    }
    function hex(value) {
        var text = (value >>> 0).toString(16)
        while (text.length < 8)
            text = "0" + text
        return text
    }
    return hex(h0) + hex(h1) + hex(h2) + hex(h3) + hex(h4)
}

function snapshotScript(collectorSource) {
    return "(function(){const result=" + collectorSource + ";"
            + "if(!result||!Array.isArray(result.candidates))return {error:'collector-result-invalid'};"
            + "const registry=window.__ferric_browserHintElements;"
            + "if(!registry||typeof registry.get!=='function')return {error:'collector-registry-missing'};"
            + "function fixtureId(node){return node&&node.getAttribute?node.getAttribute('data-hv2-id'):null;}"
            + "function ownerFacts(doc){const ids=[],indices=[];let current=doc;"
            + "for(let depth=0;current&&current!==document&&depth<8;depth++){"
            + "const view=current.defaultView,frame=view&&view.frameElement;"
            + "if(!frame||frame.contentDocument!==current)return null;"
            + "const owner=frame.ownerDocument,frames=Array.prototype.slice.call(owner.querySelectorAll('iframe,frame'));"
            + "const index=frames.indexOf(frame);if(index<0)return null;"
            + "ids.unshift(fixtureId(frame));indices.unshift(String(index));current=owner;}"
            + "if(current!==document)return null;return {ids:ids,path:'0'+(indices.length?'.'+indices.join('.'):'' )};}"
            + "const candidates=[];for(const item of result.candidates){"
            + "const record=registry.get(item.element_id),element=record&&record.element;"
            + "if(!record||!element)return {error:'candidate-record-missing',element_id:item.element_id};"
            + "const related=record.relatedElement||null,owner=ownerFacts(record.ownerDocument);"
            + "candidates.push({fixture_id:fixtureId(element),related_fixture_id:fixtureId(related),"
            + "kind:item.kind,family:record.family,discovery_families:(record.discoveryFamilies||[]).slice(),"
            + "frame_path:item.frame_path,owner_frame_ids:owner?owner.ids:null,"
            + "ownership_valid:record.ownerDocument===element.ownerDocument&&!!owner&&owner.path===item.frame_path,"
            + "related_identity_valid:record.relatedElementId===item.related_element_id,"
            + "text:item.text,href:item.href,presentation:item.presentation,geometry:item.geometry,"
            + "local_element_id:item.element_id,local_related_element_id:item.related_element_id});}"
            + "return {family:'all',candidates:candidates,inspected_nodes:result.inspected_nodes,"
            + "truncated:result.truncated,viewport:result.viewport};})()"
}

function canonical(snapshot) {
    if (!snapshot || snapshot.error)
        return snapshot
    return {
        family: snapshot.family,
        inspected_nodes: snapshot.inspected_nodes,
        truncated: snapshot.truncated,
        viewport: snapshot.viewport,
        candidates: snapshot.candidates.map(function(item) {
            return {
                fixture_id: item.fixture_id,
                related_fixture_id: item.related_fixture_id,
                kind: item.kind,
                family: item.family,
                discovery_families: item.discovery_families,
                frame_path: item.frame_path,
                owner_frame_ids: item.owner_frame_ids,
                ownership_valid: item.ownership_valid,
                related_identity_valid: item.related_identity_valid,
                text: item.text,
                href: item.href,
                presentation: item.presentation,
                geometry: item.geometry
            }
        })
    }
}
