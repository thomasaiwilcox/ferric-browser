.pragma library

// Generates only a closed, browser-owned ApplicationWorld program. The page
// receives no path, profile handle, cookie, command or editable-value request.
function geometryScript(request) {
    var input = JSON.stringify(request)
    if (typeof input !== "string" || input.length > 4096) {
        return "(function(){return JSON.stringify({ok:false,error:'invalid-request'})})()"
    }
    return "(" + PAGE_SOURCE + ")(" + input + ")"
}

function mutationProbeScript(request) {
    var input
    try {
        input = JSON.stringify({nonce: request.nonce,
                                capture_fingerprint: request.capture_fingerprint})
    } catch (error) {
        return "(function(){return 'not-observed'})()"
    }
    if (typeof input !== "string" || input.length > 256) {
        return "(function(){return 'not-observed'})()"
    }
    return "(" + MUTATION_PROBE_SOURCE + ")(" + input + ")"
}

// A one-shot geometry-only application. The request comes from a qualified
// native plan, but the page helper still independently validates the live DOM.
function restoreGeometryScript(request) {
    var input
    try {
        input = JSON.stringify(typeof request === "string" ? JSON.parse(request) : request)
    } catch (error) {
        return "(function(){return JSON.stringify({ok:false,error:'invalid-request'})})()"
    }
    if (typeof input !== "string" || input.length > 65536) {
        return "(function(){return JSON.stringify({ok:false,error:'invalid-request'})})()"
    }
    return "(" + PAGE_SOURCE + ")(" + input + ")"
}

// An ApplicationWorld-only stamp distinguishes same-URL document replacement
// across the collector's separate asynchronous frame calls. The probe never
// creates the stamp, so a reload cannot make an old result look current.
function documentChallengeScript(request) {
    if (!request || typeof request.nonce !== "string"
            || !/^[0-9a-f]{32}$/.test(request.nonce)
            || typeof request.capture_fingerprint !== "string"
            || !/^[0-9a-f]{64}$/.test(request.capture_fingerprint)) {
        return "'invalid-request'"
    }
    var token = JSON.stringify(request.nonce + ":" + request.capture_fingerprint)
    return "(function(token){try{return window.__ferricPageStateChallenge===token"
        + "?'same':'changed'}catch(error){return 'changed'}})(" + token + ")"
}

function parseHttpAddress(value) {
    if (typeof value !== "string" || value.length > 4096) return null
    var parts = /^(https?):\/\/([^/?#]+)(\/[^?#]*)?(\?[^#]*)?(?:#.*)?$/i.exec(value)
    if (!parts || parts[2].indexOf("@") >= 0) return null
    var scheme = parts[1].toLowerCase()
    var authority = parts[2].toLowerCase()
    if (/\s/.test(authority)) return null
    if ((scheme === "http" && /:80$/.test(authority))
            || (scheme === "https" && /:443$/.test(authority))) {
        authority = authority.replace(/:(80|443)$/, "")
    }
    return {origin: scheme + "://" + authority,
            route: (parts[3] || "/") + (parts[4] || "")}
}

// Renderer JSON is untrusted even in ApplicationWorld. Do not forward fields
// outside the reviewed sidecar schema through the QML callback boundary.
function hasExactKeys(value, keys) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return false
    var actual = Object.keys(value)
    if (actual.length !== keys.length) return false
    return keys.every(function(key) { return actual.indexOf(key) >= 0 })
}

function frameResultHasExpectedShape(result) {
    if (!hasExactKeys(result, ["schema_version", "helper_version", "nonce",
                               "capture_fingerprint", "requested_categories", "frames",
                               "candidates", "positions", "controls", "coverage",
                               "exclusions", "sensitive_editable", "nodes_visited"])
            || !Array.isArray(result.requested_categories)) return false
    return result.frames.every(function(frame) {
        return hasExactKeys(frame, ["id", "parent", "origin", "route_fingerprint"])
    }) && result.candidates.every(function(candidate) {
        return hasExactKeys(candidate, ["frame_id", "candidate_id", "kind",
                                        "id_fingerprint", "name_fingerprint",
                                        "label_fingerprint", "context_fingerprint",
                                        "source_fingerprint"])
    }) && result.positions.every(function(position) {
        return hasExactKeys(position, ["frame_id", "candidate_id", "kind", "value", "paused"])
    }) && result.controls.every(function(control) {
        if (!hasExactKeys(control, ["frame_id", "candidate_id", "value"])) return false
        var value = control.value
        return value && (value.kind === "details"
            ? hasExactKeys(value, ["kind", "open"])
            : value.kind === "select"
                && hasExactKeys(value, ["kind", "option_fingerprints"]))
    }) && result.coverage.every(function(coverage) {
        return hasExactKeys(coverage, ["category", "requested", "captured", "truncated"])
    }) && result.exclusions.every(function(exclusion) {
        return hasExactKeys(exclusion, ["frame_id", "reason"])
    }) && result.sensitive_editable.every(function(editable) {
        return hasExactKeys(editable, ["frame_id", "candidate_id", "value"])
            && hasExactKeys(editable.value, ["kind", "value"])
    })
}

// Collect one bounded top-level frame tree. Numeric frame IDs are capture-local
// foreign keys only: no frame-order mapping survives this call or authorizes a
// later restore. Child enumeration is serial so aggregate budgets are enforced
// before another renderer result is requested.
function collectFrameTree(view, request, callback) {
    var completed = false
    function finish(value) {
        if (completed) return
        completed = true
        callback(JSON.stringify(value), probeCapturedMutation)
    }
    function boundedInteger(value, minimum, maximum) {
        return Number.isInteger(value) && value >= minimum && value <= maximum
    }
    function utf8ByteLength(value) {
        var bytes = 0
        for (var index = 0; index < value.length; index++) {
            var code = value.charCodeAt(index)
            if (code >= 0xd800 && code <= 0xdbff
                    && index + 1 < value.length) {
                var low = value.charCodeAt(index + 1)
                if (low >= 0xdc00 && low <= 0xdfff) {
                    bytes += 4
                    index++
                    continue
                }
            }
            bytes += code <= 0x7f ? 1 : code <= 0x7ff ? 2 : 3
        }
        return bytes
    }
    if (!view || !request || typeof request !== "object"
            || typeof callback !== "function") {
        finish({ok: false, error: "invalid-request"})
        return
    }
    var maxFrames = request.max_frames === undefined ? 16 : request.max_frames
    var maxNodes = request.max_nodes === undefined ? 4096 : request.max_nodes
    var maxCandidates = request.max_total_candidates === undefined
        ? (request.max_candidates === undefined ? 256 : request.max_candidates)
        : request.max_total_candidates
    var maxPositions = request.max_total_positions === undefined ? 512 : request.max_total_positions
    var maxControls = request.max_total_controls === undefined ? 256 : request.max_total_controls
    var maxEditableValues = request.max_total_editable_values === undefined
        ? 128 : request.max_total_editable_values
    var maxEditableBytes = request.max_total_editable_bytes === undefined
        ? 65536 : request.max_total_editable_bytes
    var maxExclusions = request.max_total_exclusions === undefined ? 128 : request.max_total_exclusions
    if (!boundedInteger(maxFrames, 1, 16) || !boundedInteger(maxNodes, 1, 4096)
            || !boundedInteger(maxCandidates, 1, 256) || !boundedInteger(maxPositions, 1, 512)
            || !boundedInteger(maxControls, 1, 256) || !boundedInteger(maxEditableValues, 1, 128)
            || !boundedInteger(maxEditableBytes, 1, 65536) || !boundedInteger(maxExclusions, 1, 128)
            || (request.include_media !== undefined && typeof request.include_media !== "boolean")) {
        finish({ok: false, error: "invalid-request"})
        return
    }
    var rootFrame
    try { rootFrame = view.mainFrame } catch (error) {}
    if (!rootFrame || !rootFrame.isValid || typeof rootFrame.children === "undefined") {
        finish({ok: false, error: "frame-api-unavailable"})
        return
    }
    var capturedViewUrl = String(view.url)
    var capturedRootFrameUrl = String(rootFrame.url)
    var expectedOrigin = String(request.expected_origin || "")
    // loadHtml(url) intentionally keeps the caller's URL on WebEngineView,
    // while Chromium exposes its generated data: document URL on mainFrame.
    // Use the view URL for qualified origin/route and retain the actual frame
    // URL separately for native document identity checks.
    var rootAddress = parseHttpAddress(capturedViewUrl)
    if (!rootAddress || rootAddress.origin !== expectedOrigin
            || rootAddress.route !== request.expected_route) {
        finish({ok: false, error: "root-route-changed"})
        return
    }

    var sidecar = {
        schema_version: 1, helper_version: "ferric-page-state-5",
        nonce: request.nonce, capture_fingerprint: request.capture_fingerprint,
        requested_categories: ["geometry"], frames: [], candidates: [], positions: [],
        controls: [], coverage: [], exclusions: [], sensitive_editable: []
    }
    if (request.include_controls) sidecar.requested_categories.push("controls")
    if (request.include_editable) sidecar.requested_categories.push("editable")
    if (request.include_selection) sidecar.requested_categories.push("selection")
    if (request.include_media) sidecar.requested_categories.push("media")
    var queue = [{frame: rootFrame, id: 0, parent: null, depth: 0,
                  url: capturedViewUrl, frameUrl: capturedRootFrameUrl, root: true}]
    var capturedFrames = []
    var nextFrameId = 1, totalFrameCount = 0, nodesVisited = 0
    var candidateCount = 0, positionCount = 0, controlCount = 0
    var editableCount = 0, editableBytes = 0, exclusionCount = 0
    var wasTruncated = false

    function rootStillCurrent() {
        try {
            return String(view.url) === capturedViewUrl && view.mainFrame
                && view.mainFrame.isValid && String(view.mainFrame.url) === capturedRootFrameUrl
        } catch (error) {
            return false
        }
    }

    // The handles and URLs are capture-local only. This one-shot callback
    // probes the same frames after archive completion; it never returns page
    // content or grants restore authority.
    var mutationProbeStarted = false
    function probeCapturedMutation(probeCallback) {
        if (typeof probeCallback !== "function") return
        if (mutationProbeStarted || !Array.isArray(capturedFrames)
                || capturedFrames.length === 0) {
            probeCallback("not-observed")
            return
        }
        mutationProbeStarted = true
        var index = 0
        var changed = false
        var documentChanged = !rootStillCurrent()
        var allObserved = true
        function next() {
            if (index >= capturedFrames.length) {
                if (!rootStillCurrent()) documentChanged = true
                probeCallback(documentChanged ? "document-changed"
                                      : changed ? "observed-changed"
                                      : allObserved ? "observed-stable" : "not-observed")
                return
            }
            var captured = capturedFrames[index++]
            var frameStillCurrent = false
            try {
                frameStillCurrent = !!captured.frame && captured.frame.isValid
                    && String(captured.frame.url) === captured.url
            } catch (error) {}
            if (!frameStillCurrent) {
                documentChanged = true
                next()
                return
            }
            var script = mutationProbeScript(request)
            try {
                captured.frame.runJavaScript(script, 1, function(result) {
                    if (result === "observed-changed") changed = true
                    else if (result === "document-changed") documentChanged = true
                    else if (result !== "observed-stable") allObserved = false
                    next()
                })
            } catch (error) {
                allObserved = false
                next()
            }
        }
        next()
    }

    function addExclusion(reason) {
        wasTruncated = true
        if (exclusionCount < maxExclusions) {
            sidecar.exclusions.push({frame_id: null, reason: reason})
            exclusionCount++
        } else {
            wasTruncated = true
        }
    }
    function mergeCoverage(entries) {
        for (var index = 0; index < entries.length; index++) {
            var incoming = entries[index], found = null
            for (var existingIndex = 0; existingIndex < sidecar.coverage.length; existingIndex++) {
                if (sidecar.coverage[existingIndex].category === incoming.category) {
                    found = sidecar.coverage[existingIndex]
                    break
                }
            }
            if (!found) {
                found = {category: incoming.category, requested: 0, captured: 0, truncated: false}
                sidecar.coverage.push(found)
            }
            found.requested = Math.min(65535, found.requested + incoming.requested)
            found.captured = Math.min(65535, found.captured + incoming.captured)
            found.truncated = found.truncated || incoming.truncated
        }
    }
    function failureReason(error) {
        if (error === "unsupported-origin") return "opaque-origin"
        if (error === "origin-changed") return "cross-origin"
        if (error === "document-changed" || error === "route-changed") return "navigated-frame"
        if (error === "limit" || error === "result-too-large") return "limit"
        return "unsupported"
    }
    function processNext() {
        if (completed) return
        if (!rootStillCurrent()) {
            finish({ok: false, error: "root-document-changed"})
            return
        }
        if (exclusionCount >= maxExclusions && queue.length > 0) {
            queue = []
            wasTruncated = true
        }
        if (queue.length === 0) {
            if (wasTruncated) {
                for (var coverageIndex = 0; coverageIndex < sidecar.coverage.length; coverageIndex++) {
                    sidecar.coverage[coverageIndex].truncated = true
                }
            }
            verifyCapturedDocuments(0)
            return
        }
        var item = queue.shift()
        if (!item.frame || !item.frame.isValid
                || String(item.frame.url) !== (item.frameUrl || item.url)) {
            addExclusion(item.root ? "navigated-frame" : "deleted-frame")
            processNext()
            return
        }
        var address = parseHttpAddress(item.url)
        if (!address) {
            if (!item.root) addExclusion("opaque-origin")
            else finish({ok: false, error: "unsupported-origin"})
            processNext()
            return
        }
        if (address.origin !== expectedOrigin) {
            addExclusion("cross-origin")
            processNext()
            return
        }
        if (!item.root && item.depth > 8) {
            addExclusion("limit")
            wasTruncated = true
            processNext()
            return
        }
        if (totalFrameCount >= maxFrames || nextFrameId > 15) {
            addExclusion("limit")
            wasTruncated = true
            processNext()
            return
        }
        if (candidateCount >= maxCandidates || positionCount >= maxPositions
                || controlCount >= maxControls
                || (request.include_editable
                    && (editableCount >= maxEditableValues || editableBytes >= maxEditableBytes))) {
            addExclusion("limit")
            wasTruncated = true
            processNext()
            return
        }
        if (nodesVisited >= maxNodes) {
            addExclusion("limit")
            wasTruncated = true
            processNext()
            return
        }

        var frameId = item.root ? 0 : nextFrameId++
        var frameRequest = {}
        Object.keys(request).forEach(function(key) { frameRequest[key] = request[key] })
        frameRequest.frame_id = frameId
        frameRequest.parent_frame_id = item.parent
        frameRequest.expected_origin = expectedOrigin
        frameRequest.expected_route = address.route
        frameRequest.max_nodes = Math.min(4096, maxNodes - nodesVisited)
        frameRequest.report_nodes = true
        frameRequest.max_candidates = Math.min(64, maxCandidates - candidateCount)
        frameRequest.max_positions = Math.min(512, maxPositions - positionCount)
        frameRequest.max_controls = Math.min(256, maxControls - controlCount)
        frameRequest.max_editable_values = Math.min(128, maxEditableValues - editableCount)
        frameRequest.max_editable_total_bytes = Math.min(65536, maxEditableBytes - editableBytes)
        frameRequest.max_exclusions = Math.max(1, Math.min(128, maxExclusions - exclusionCount))
        var script = geometryScript(frameRequest)
        if (!script || frameRequest.max_candidates < 1 || frameRequest.max_positions < 1
                || frameRequest.max_controls < 1 || frameRequest.max_editable_values < 1
                || frameRequest.max_editable_total_bytes < 1) {
            addExclusion("limit")
            wasTruncated = true
            processNext()
            return
        }
        var capturedFrameUrl = String(item.frame.url)
        var capturedFrame = item.frame
        try {
        capturedFrame.runJavaScript(script, 1, function(rawResult) {
            if (completed) return
            if (!rootStillCurrent()) {
                finish({ok: false, error: "root-document-changed"})
                return
            }
            if (!capturedFrame.isValid) {
                addExclusion("deleted-frame")
                processNext()
                return
            }
            if (String(capturedFrame.url) !== capturedFrameUrl) {
                addExclusion("navigated-frame")
                processNext()
                return
            }
            if (typeof rawResult !== "string" || rawResult.length > 256 * 1024) {
                if (item.root) finish({ok: false, error: "result-too-large"})
                else {
                    addExclusion("limit")
                    processNext()
                }
                return
            }
            var result
            try { result = JSON.parse(rawResult) } catch (error) {}
            if (!result || result.ok === false) {
                addExclusion(failureReason(result && result.error))
                processNext()
                return
            }
            var frameRecord = result.frames && result.frames.length === 1 ? result.frames[0] : null
            if (result.schema_version !== 1 || result.nonce !== request.nonce
                    || result.capture_fingerprint !== request.capture_fingerprint
                    || !frameRecord || frameRecord.id !== frameId
                    || frameRecord.parent !== item.parent || frameRecord.origin !== expectedOrigin
                    || !/^[0-9a-f]{64}$/.test(frameRecord.route_fingerprint)
                    || !boundedInteger(result.nodes_visited, 0, frameRequest.max_nodes)) {
                addExclusion("unsupported")
                processNext()
                return
            }
            if (!Array.isArray(result.candidates)
                    || result.candidates.length > frameRequest.max_candidates
                    || !Array.isArray(result.positions)
                    || result.positions.length > frameRequest.max_positions
                    || !Array.isArray(result.controls)
                    || result.controls.length > frameRequest.max_controls
                    || !Array.isArray(result.coverage) || result.coverage.length > 5
                    || !Array.isArray(result.exclusions)
                    || result.exclusions.length > frameRequest.max_exclusions
                    || !Array.isArray(result.sensitive_editable)
                    || result.sensitive_editable.length > frameRequest.max_editable_values) {
                if (item.root) finish({ok: false, error: "invalid-frame-result"})
                else {
                    addExclusion("unsupported")
                    processNext()
                }
                return
            }
            if (!frameResultHasExpectedShape(result)) {
                if (item.root) finish({ok: false, error: "invalid-frame-result"})
                else {
                    addExclusion("unsupported")
                    processNext()
                }
                return
            }
            var incomingEditableBytes = 0
            for (var editableIndex = 0;
                    editableIndex < result.sensitive_editable.length; editableIndex++) {
                var editable = result.sensitive_editable[editableIndex]
                if (!editable || !editable.value
                        || (editable.value.kind !== "text"
                            && editable.value.kind !== "checked")
                        || (editable.value.kind === "text"
                            && typeof editable.value.value !== "string")
                        || (editable.value.kind === "checked"
                            && typeof editable.value.value !== "boolean")) {
                    if (item.root) finish({ok: false, error: "invalid-frame-result"})
                    else {
                        addExclusion("unsupported")
                        processNext()
                    }
                    return
                }
                if (editable.value.kind === "text") {
                    incomingEditableBytes += utf8ByteLength(editable.value.value)
                }
            }
            if (incomingEditableBytes > frameRequest.max_editable_total_bytes) {
                if (item.root) finish({ok: false, error: "invalid-frame-result"})
                else {
                    addExclusion("unsupported")
                    processNext()
                }
                return
            }
            nodesVisited += result.nodes_visited
            delete result.nodes_visited
            capturedFrames.push({frame: capturedFrame, url: capturedFrameUrl,
                                 root: item.root})
            sidecar.frames.push(frameRecord)
            totalFrameCount++
            Array.prototype.push.apply(sidecar.candidates, result.candidates || [])
            Array.prototype.push.apply(sidecar.positions, result.positions || [])
            Array.prototype.push.apply(sidecar.controls, result.controls || [])
            Array.prototype.push.apply(sidecar.exclusions, result.exclusions || [])
            exclusionCount += (result.exclusions || []).length
            Array.prototype.push.apply(sidecar.sensitive_editable, result.sensitive_editable || [])
            mergeCoverage(result.coverage || [])
            candidateCount += (result.candidates || []).length
            positionCount += (result.positions || []).length
            controlCount += (result.controls || []).length
            var editableValues = result.sensitive_editable || []
            editableCount += editableValues.length
            editableBytes += incomingEditableBytes
            var children
            var childrenLength = -1
            try {
                children = capturedFrame.children
                childrenLength = children && children.length
            } catch (error) {}
            if (!Number.isInteger(childrenLength)
                    || childrenLength < 0 || childrenLength > 256) {
                addExclusion("limit")
                wasTruncated = true
            } else {
                var remainingFrameSlots = Math.max(0, maxFrames - totalFrameCount - queue.length)
                var childrenToInspect = Math.min(childrenLength, remainingFrameSlots)
                if (childrenToInspect < childrenLength) {
                    addExclusion("limit")
                    wasTruncated = true
                }
                for (var childIndex = 0; childIndex < childrenToInspect; childIndex++) {
                    var child
                    try { child = children[childIndex] } catch (error) {}
                    if (!child || !child.isValid) {
                        addExclusion("deleted-frame")
                        continue
                    }
                    queue.push({frame: child, id: -1, parent: frameId,
                                depth: item.depth + 1, url: String(child.url), root: false})
                }
            }
            processNext()
        })
        } catch (error) {
            addExclusion("deleted-frame")
            processNext()
        }
    }
    function verifyCapturedDocuments(index) {
        if (completed) return
        if (!rootStillCurrent()) {
            finish({ok: false, error: "root-document-changed"})
            return
        }
        if (index >= capturedFrames.length) {
            var encoded = JSON.stringify(sidecar)
            finish(encoded.length <= 256 * 1024
                ? sidecar : {ok: false, error: "result-too-large"})
            return
        }
        var captured = capturedFrames[index]
        if (!captured.frame || !captured.frame.isValid
                || String(captured.frame.url) !== captured.url) {
            finish({ok: false, error: captured.root
                ? "root-document-changed" : "frame-document-changed"})
            return
        }
        try {
            captured.frame.runJavaScript(documentChallengeScript(request), 1, function(result) {
                if (completed) return
                if (result !== "same") {
                    finish({ok: false, error: captured.root
                        ? "root-document-changed" : "frame-document-changed"})
                    return
                }
                verifyCapturedDocuments(index + 1)
            })
        } catch (error) {
            finish({ok: false, error: captured.root
                ? "root-document-changed" : "frame-document-changed"})
        }
    }
    processNext()
}

// String.raw keeps regex and string escapes unchanged in the reviewed page
// source. QML-imported functions stringify as native wrappers, so their
// toString() output cannot be used as a page script.
var PAGE_SOURCE = String.raw`function collectGeometry(input) {
    "use strict"
    var MAX_NODES = 4096
    var MAX_CANDIDATES = 64
    var MAX_POSITIONS = 512
    var MAX_CONTROLS = 256
    var MAX_EDITABLE_VALUES = 128
    var MAX_EDITABLE_VALUE_BYTES = 4096
    var MAX_EDITABLE_TOTAL_BYTES = 65536
    var MAX_EXCLUSIONS = 128
    var MAX_MS = 50
    var MAX_COORDINATE = 1000000000
    var MAX_MEDIA_SECONDS = 604800

    function fail(code) { return JSON.stringify({ok: false, error: code}) }
    function hex(value, length) {
        return typeof value === "string" && value.length === length
            && /^[0-9a-f]+$/.test(value)
    }
    function numberInRange(value, lower, upper) {
        return Number.isFinite(value) && value >= lower && value <= upper
    }
    function integerInRange(value, lower, upper) {
        return Number.isInteger(value) && value >= lower && value <= upper
    }
    function objectHasExactKeys(value, keys) {
        if (!value || typeof value !== "object" || Array.isArray(value)) return false
        var actual = Object.keys(value)
        if (actual.length !== keys.length) return false
        for (var index = 0; index < keys.length; index++) {
            if (actual.indexOf(keys[index]) < 0) return false
        }
        return true
    }

    // Candidate hints are hashes of bounded DOM identity/structure, never
    // selectors or authority. This small synchronous SHA-256 has no network,
    // storage, async callback or page-world dependency.
    function sha256Hex(text) {
        var bytes = new TextEncoder().encode(text)
        if (bytes.length > 512) return null
        var size = Math.ceil((bytes.length + 9) / 64) * 64
        var buffer = new Uint8Array(size)
        buffer.set(bytes)
        buffer[bytes.length] = 0x80
        var view = new DataView(buffer.buffer)
        var bitLength = bytes.length * 8
        view.setUint32(size - 8, 0, false)
        view.setUint32(size - 4, bitLength >>> 0, false)
        var state = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                     0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19]
        var round = [
            0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
            0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,0xe49b69c1,
            0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,0x983e5152,0xa831c66d,
            0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,0x27b70a85,0x2e1b2138,0x4d2c6dfc,
            0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,
            0xd192e819,0xd6990624,0xf40e3585,0x106aa070,0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,
            0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,
            0xbef9a3f7,0xc67178f2
        ]
        var words = new Uint32Array(64)
        function rotate(value, count) { return (value >>> count) | (value << (32 - count)) }
        for (var offset = 0; offset < size; offset += 64) {
            for (var i = 0; i < 16; i++) words[i] = view.getUint32(offset + i * 4, false)
            for (var next = 16; next < 64; next++) {
                var a = words[next - 15], b = words[next - 2]
                var small0 = rotate(a, 7) ^ rotate(a, 18) ^ (a >>> 3)
                var small1 = rotate(b, 17) ^ rotate(b, 19) ^ (b >>> 10)
                words[next] = (words[next - 16] + small0 + words[next - 7] + small1) >>> 0
            }
            var h = state.slice(0)
            for (var step = 0; step < 64; step++) {
                var choice = (h[4] & h[5]) ^ (~h[4] & h[6])
                var majority = (h[0] & h[1]) ^ (h[0] & h[2]) ^ (h[1] & h[2])
                var big1 = rotate(h[4], 6) ^ rotate(h[4], 11) ^ rotate(h[4], 25)
                var big0 = rotate(h[0], 2) ^ rotate(h[0], 13) ^ rotate(h[0], 22)
                var t1 = (h[7] + big1 + choice + round[step] + words[step]) >>> 0
                var t2 = (big0 + majority) >>> 0
                h = [(t1 + t2) >>> 0, h[0], h[1], h[2], (h[3] + t1) >>> 0, h[4], h[5], h[6]]
            }
            for (var slot = 0; slot < 8; slot++) state[slot] = (state[slot] + h[slot]) >>> 0
        }
        return state.map(function(word) { return ("00000000" + word.toString(16)).slice(-8) }).join("")
    }

    function contextHint(element) {
        var parts = []
        for (var current = element, depth = 0; current && depth < 6; depth++) {
            var previous = current.previousElementSibling
            var index = 0
            while (previous && index < 32) { index++; previous = previous.previousElementSibling }
            if (previous) return null
            parts.push(current.tagName.toLowerCase() + ":" + index)
            if (current.parentElement) {
                current = current.parentElement
            } else {
                var root = current.getRootNode && current.getRootNode()
                current = root && root.host ? root.host : null
            }
        }
        return sha256Hex(parts.join("/"))
    }

    function candidate(element, kind, id, sourceFingerprint) {
        var rawId = typeof element.id === "string" && element.id.length <= 256 ? element.id : ""
        var rawName = typeof element.getAttribute === "function" ? element.getAttribute("name") : null
        if (typeof rawName !== "string" || rawName.length > 256) rawName = ""
        var context = contextHint(element)
        if (!context) return null
        return {
            frame_id: frameId, candidate_id: id,
            kind: kind === "contenteditable" ? "content-editable" : kind,
            id_fingerprint: rawId ? sha256Hex("id:" + rawId) : null,
            name_fingerprint: rawName ? sha256Hex("name:" + rawName) : null,
            label_fingerprint: null,
            context_fingerprint: context,
            source_fingerprint: sourceFingerprint || null
        }
    }


    function restoreTargetValid(target) {
        return objectHasExactKeys(target, ["candidate_id", "id_fingerprint",
                                           "name_fingerprint", "label_fingerprint",
                                           "context_fingerprint", "x", "y"])
            && integerInRange(target.candidate_id, 1, MAX_CANDIDATES)
            && (target.id_fingerprint === null || hex(target.id_fingerprint, 64))
            && (target.name_fingerprint === null || hex(target.name_fingerprint, 64))
            && target.label_fingerprint === null
            && hex(target.context_fingerprint, 64)
            && (target.id_fingerprint !== null || target.name_fingerprint !== null)
            && (target.x === null || numberInRange(target.x, -MAX_COORDINATE, MAX_COORDINATE))
            && (target.y === null || numberInRange(target.y, -MAX_COORDINATE, MAX_COORDINATE))
            && (target.x !== null || target.y !== null)
    }

    function restoreMediaTargetValid(target) {
        if (!target || typeof target !== "object" || Array.isArray(target)) return false
        var expectedSource = ""
        var sourceFingerprint = typeof target.source_fingerprint === "string"
                ? /^hmac-sha256-v1-k([1-9][0-9]{0,4}):([0-9a-f]{64})$/
                    .exec(target.source_fingerprint)
                : null
        if (!sourceFingerprint || Number(sourceFingerprint[1]) > 65535) return false
        try {
            if (typeof target.expected_source_url !== "string"
                    || target.expected_source_url.length === 0
                    || target.expected_source_url.length > 2048) return false
            expectedSource = new URL(target.expected_source_url)
            if ((expectedSource.protocol !== "http:" && expectedSource.protocol !== "https:")
                    || expectedSource.username.length > 0 || expectedSource.password.length > 0
                    || expectedSource.href !== target.expected_source_url) return false
        } catch (error) { return false }
        return objectHasExactKeys(target, ["candidate_id", "id_fingerprint",
                                           "name_fingerprint", "label_fingerprint",
                                           "context_fingerprint", "source_fingerprint",
                                           "time_seconds", "expected_source_url"])
            && integerInRange(target.candidate_id, 1, MAX_CANDIDATES)
            && (target.id_fingerprint === null || hex(target.id_fingerprint, 64))
            && (target.name_fingerprint === null || hex(target.name_fingerprint, 64))
            && target.label_fingerprint === null
            && hex(target.context_fingerprint, 64)
            && (target.id_fingerprint !== null || target.name_fingerprint !== null)
            && numberInRange(target.time_seconds, 0, MAX_MEDIA_SECONDS)
    }

    function restoreEditableTargetValid(target) {
        if (!objectHasExactKeys(target, ["candidate_id", "id_fingerprint",
                                         "name_fingerprint", "label_fingerprint",
                                         "context_fingerprint", "kind", "value"])
                || !integerInRange(target.candidate_id, 1, MAX_CANDIDATES)
                || (target.id_fingerprint !== null && !hex(target.id_fingerprint, 64))
                || (target.name_fingerprint !== null && !hex(target.name_fingerprint, 64))
                || target.label_fingerprint !== null
                || !hex(target.context_fingerprint, 64)
                || ["text", "textarea", "checkbox"].indexOf(target.kind) < 0
                || (target.id_fingerprint === null && target.name_fingerprint === null)
                || !target.value || typeof target.value !== "object") {
            return false
        }
        if (target.kind === "checkbox") {
            return objectHasExactKeys(target.value, ["kind", "value"])
                && target.value.kind === "checked" && target.value.value === true
        }
        return objectHasExactKeys(target.value, ["kind", "value"])
            && target.value.kind === "text"
            && typeof target.value.value === "string"
            && target.value.value.length > 0
            && new TextEncoder().encode(target.value.value).length <= MAX_EDITABLE_VALUE_BYTES
    }

    function sameCandidate(target, item) {
        return item.kind === "scroll-area"
            && item.id_fingerprint === target.id_fingerprint
            && item.name_fingerprint === target.name_fingerprint
            && item.label_fingerprint === target.label_fingerprint
            && item.context_fingerprint === target.context_fingerprint
    }

    try {
        if (!input || typeof input !== "object"
                || !hex(input.nonce, 32)
                || !hex(input.route_fingerprint, 64)
                || !hex(input.capture_fingerprint, 64)
                || typeof input.expected_origin !== "string" || input.expected_origin.length > 512
                || typeof input.expected_route !== "string" || input.expected_route.length > 2048
                || !integerInRange(input.max_nodes, 1, MAX_NODES)
                || !integerInRange(input.max_candidates, 1, MAX_CANDIDATES)
                || !integerInRange(input.max_ms, 1, MAX_MS)
                || (input.frame_id !== undefined && !integerInRange(input.frame_id, 0, 15))
                || (input.parent_frame_id !== undefined && input.parent_frame_id !== null
                    && !integerInRange(input.parent_frame_id, 0, 15))
                || (input.frame_id === 0 && input.parent_frame_id !== null
                    && input.parent_frame_id !== undefined)
                || (input.frame_id > 0 && !integerInRange(input.parent_frame_id, 0, 15))
                || (input.max_positions !== undefined
                    && !integerInRange(input.max_positions, 1, MAX_POSITIONS))
                || (input.max_controls !== undefined
                    && !integerInRange(input.max_controls, 1, MAX_CONTROLS))
                || (input.max_editable_values !== undefined
                    && !integerInRange(input.max_editable_values, 1, MAX_EDITABLE_VALUES))
                || (input.max_editable_total_bytes !== undefined
                    && !integerInRange(input.max_editable_total_bytes, 1, MAX_EDITABLE_TOTAL_BYTES))
                || (input.max_exclusions !== undefined
                    && !integerInRange(input.max_exclusions, 1, MAX_EXCLUSIONS))
                || (input.report_nodes !== undefined && typeof input.report_nodes !== "boolean")
                || (input.include_controls !== undefined && typeof input.include_controls !== "boolean")
                || (input.include_editable !== undefined && typeof input.include_editable !== "boolean")
                || (input.include_selection !== undefined && typeof input.include_selection !== "boolean")
                || (input.include_media !== undefined && typeof input.include_media !== "boolean")) {
            return fail("invalid-request")
        }
        var frameId = input.frame_id === undefined ? 0 : input.frame_id
        var parentFrameId = frameId === 0 ? null : input.parent_frame_id
        var maxPositions = input.max_positions === undefined ? MAX_POSITIONS : input.max_positions
        var maxControls = input.max_controls === undefined ? MAX_CONTROLS : input.max_controls
        var maxEditableValues = input.max_editable_values === undefined
            ? MAX_EDITABLE_VALUES : input.max_editable_values
        var maxEditableTotalBytes = input.max_editable_total_bytes === undefined
            ? MAX_EDITABLE_TOTAL_BYTES : input.max_editable_total_bytes
        var maxExclusions = input.max_exclusions === undefined ? MAX_EXCLUSIONS : input.max_exclusions
        var restore = input.restore_geometry
        if (restore !== undefined
                && (!restore || typeof restore !== "object" || input.include_controls !== false
                    || input.include_selection === true
                    || (restore.top_x !== null && !numberInRange(restore.top_x, -MAX_COORDINATE, MAX_COORDINATE))
                    || (restore.top_y !== null && !numberInRange(restore.top_y, -MAX_COORDINATE, MAX_COORDINATE))
                    || !Array.isArray(restore.targets) || restore.targets.length > MAX_CANDIDATES
                    || restore.targets.some(function(target) { return !restoreTargetValid(target) })
                    || (restore.media_targets !== undefined
                        && (!Array.isArray(restore.media_targets)
                            || restore.media_targets.length > MAX_CANDIDATES
                            || restore.media_targets.some(function(target) {
                                return !restoreMediaTargetValid(target)
                            })))
                    || (input.include_media !== true
                        && restore.media_targets !== undefined
                        && restore.media_targets.length > 0)
                    || (restore.editable_targets !== undefined
                        && (!Array.isArray(restore.editable_targets)
                            || restore.editable_targets.length > 64
                            || restore.editable_targets.some(function(target) {
                                return !restoreEditableTargetValid(target)
                            })))
                    || (input.include_editable !== true
                        && restore.editable_targets !== undefined
                        && restore.editable_targets.length > 0))) {
            return fail("invalid-request")
        }
        var origin = String(location.origin)
        if (origin === "null" || !/^https?:\/\//.test(origin)) return fail("unsupported-origin")
        if (origin !== input.expected_origin) return fail("origin-changed")
        var route = location.pathname + location.search
        if (route !== input.expected_route) return fail("route-changed")
        // INVARIANT: a same-URL reload gets a fresh ApplicationWorld and loses
        // this per-capture stamp; the native frame collector probes it again
        // after all asynchronous reads before admitting the combined result.
        window.__ferricPageStateChallenge = input.nonce + ":" + input.capture_fingerprint
        var frameRouteFingerprint = frameId === 0
            ? input.route_fingerprint
            : sha256Hex(origin + String.fromCharCode(0) + route)
        if (!frameRouteFingerprint) return fail("route-fingerprint-failed")
        var capturedDocument = document
        var capturedUrl = location.href
        // INVARIANT: retain only a one-bit change signal in this isolated
        // world. Mutation records and page data never cross into the sidecar.
        var mutationTracker = null
        try {
            var mutationKey = "__ferricVersionedPageMutation"
            var existingMutationTracker = window[mutationKey]
            if (input.report_nodes === true && existingMutationTracker
                    && existingMutationTracker.nonce === input.nonce
                    && existingMutationTracker.captureFingerprint === input.capture_fingerprint
                    && existingMutationTracker.document === capturedDocument
                    && existingMutationTracker.observer) {
                mutationTracker = existingMutationTracker
            } else if (input.report_nodes === true
                       && typeof MutationObserver === "function") {
                if (existingMutationTracker) {
                    if (existingMutationTracker.observer)
                        existingMutationTracker.observer.disconnect()
                    if (existingMutationTracker.expiry !== null
                            && existingMutationTracker.expiry !== undefined)
                        window.clearTimeout(existingMutationTracker.expiry)
                }
                mutationTracker = {
                    nonce: input.nonce,
                    captureFingerprint: input.capture_fingerprint,
                    document: capturedDocument,
                    changed: false,
                    observer: null,
                    expiry: null
                }
                mutationTracker.observer = new MutationObserver(function(records) {
                    if (records.length > 0) {
                        mutationTracker.changed = true
                        mutationTracker.observer.disconnect()
                    }
                })
                mutationTracker.observer.observe(capturedDocument, {
                    subtree: true,
                    childList: true,
                    attributes: true,
                    characterData: true
                })
                window[mutationKey] = mutationTracker
                mutationTracker.expiry = window.setTimeout(function() {
                    if (window[mutationKey] !== mutationTracker) return
                    mutationTracker.observer.disconnect()
                    delete window[mutationKey]
                }, 120000)
            }
        } catch (mutationSetupError) {
            mutationTracker = null
        }
        var started = performance.now()
        var root = document.scrollingElement || document.documentElement
        if (!root || !document.documentElement) return fail("document-unavailable")
        var positions = [], candidates = [], candidateElements = [], controls = [], exclusions = []
        var sensitiveEditable = []
        var editableRequested = 0, editableBytes = 0, selectionCaptured = 0
        var selectionTruncated = false
        var shadowScopeIncomplete = false
        var mediaRequested = 0, mediaCaptured = 0
        var truncated = false, visited = 0
        function exclude(reason) {
            if (exclusions.length < maxExclusions) exclusions.push({frame_id: frameId, reason: reason})
        }
    function position(id, kind, value) {
            if (!numberInRange(value, -MAX_COORDINATE, MAX_COORDINATE)) {
                exclude("unsupported")
                return
            }
            if (positions.length >= maxPositions) { truncated = true; return }
            positions.push({frame_id: frameId, candidate_id: id, kind: kind, value: value, paused: null})
        }
        function hasLiveSeekWindow(element) {
            try {
                var ranges = element.seekable
                return ranges && ranges.length > 0
                    && Number.isFinite(Number(ranges.start(0)))
                    && Number(ranges.start(0)) > 0.5
            } catch (error) {
                return false
            }
        }
        function fingerprintMediaSource(element) {
            var raw = String(element.currentSrc || "")
            var url = null
            try { url = new URL(raw) } catch (error) {}
            var path = url ? url.pathname.toLowerCase() : ""
            if (!url || raw.length > 2048
                    || (url.protocol !== "http:" && url.protocol !== "https:")
                    || url.username.length > 0 || url.password.length > 0
                    || /\.(m3u8?|mpd)$/.test(path) || /\.ism(?:\/|$)/.test(path)) {
                return null
            }
            // This bounded canonical URL is transient only. The native adapter
            // replaces it with a profile-keyed HMAC before parsing or storage;
            // no key is passed into ApplicationWorld.
            return url.href
        }
        function editableKind(element) {
            var editableAttribute = typeof element.getAttribute === "function"
                ? String(element.getAttribute("contenteditable") || "").toLowerCase() : ""
            if ((editableAttribute === "true" || editableAttribute === "plaintext-only")
                    && (!element.parentElement || !element.parentElement.isContentEditable)) {
                return "contenteditable"
            }
            if (element.tagName === "TEXTAREA") return "textarea"
            if (element.tagName !== "INPUT") return ""
            var type = String(element.type || "text").toLowerCase()
            if (type === "checkbox") return "checkbox"
            if (type === "radio") return "radio"
            if (type === "text" || type === "search") return "text"
            return ""
        }
        function sensitiveEditableField(element) {
            var type = element.tagName === "INPUT"
                ? String(element.type || "text").toLowerCase() : ""
            if (["password", "file", "hidden"].indexOf(type) >= 0) return true
            var fields
            try {
                fields = [element.id, element.name, element.autocomplete,
                          typeof element.getAttribute === "function"
                              ? element.getAttribute("autocomplete") : "",
                          typeof element.getAttribute === "function"
                              ? element.getAttribute("aria-label") : ""]
            } catch (error) {
                return true
            }
            for (var index = 0; index < fields.length; index++) {
                if (fields[index] === null || fields[index] === undefined) {
                    fields[index] = ""
                } else if (typeof fields[index] !== "string" || fields[index].length > 256) {
                    return true
                }
            }
            var metadata = fields.join(" ").toLowerCase()
            return /(^|[-_.\s])(pass(word)?|token|secret|auth|session|csrf|xsrf|otp|one[-_. ]?time|totp|pin|recovery|verification|security[-_. ]?code|credit[-_. ]?card|cc[-_. ]?number)([-_.\s]|$)/i.test(metadata)
        }
        function addEditableValue(item, element, kind) {
            editableRequested++
            if (sensitiveEditableField(element)) {
                exclude("sensitive-control")
                return
            }
            if (sensitiveEditable.length >= maxEditableValues) {
                truncated = true
                exclude("limit")
                return
            }
            var value
            if (kind === "checkbox" || kind === "radio") {
                value = {kind: "checked", value: !!element.checked}
            } else {
                var text = ""
                if (kind === "contenteditable") {
                    var textWalker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT)
                    var textParts = [], textLength = 0, textNode
                    while ((textNode = textWalker.nextNode())) {
                        if (performance.now() - started >= input.max_ms) {
                            truncated = true
                            exclude("timeout")
                            return
                        }
                        var part = String(textNode.nodeValue || "")
                        textLength += part.length
                        if (textLength > MAX_EDITABLE_VALUE_BYTES) {
                            truncated = true
                            exclude("limit")
                            return
                        }
                        textParts.push(part)
                    }
                    text = textParts.join("")
                } else {
                    var rawValue = element.value
                    if (typeof rawValue !== "string"
                            || rawValue.length > MAX_EDITABLE_VALUE_BYTES) {
                        truncated = true
                        exclude("limit")
                        return
                    }
                    text = rawValue
                }
                var byteLength = new TextEncoder().encode(text).length
                if (byteLength > MAX_EDITABLE_VALUE_BYTES
                        || editableBytes + byteLength > maxEditableTotalBytes) {
                    truncated = true
                    exclude("limit")
                    return
                }
                editableBytes += byteLength
                value = {kind: "text", value: text}
            }
            sensitiveEditable.push({frame_id: frameId, candidate_id: item.candidate_id, value: value})
        }
        // The restore plan currently owns only the top-level document scroll
        // position. Child-frame elements remain observable by frame ID, but
        // their document viewport offsets must not be mislabeled as the page's
        // top-scroll coordinates.
        if (frameId === 0) {
            position(null, "top-scroll-x", Number(root.scrollLeft))
            position(null, "top-scroll-y", Number(root.scrollTop))
        }
        var walkers = [document.createTreeWalker(document.documentElement, NodeFilter.SHOW_ELEMENT)]
        var openShadowRootsVisited = 0
        function nextElement() {
            while (walkers.length > 0) {
                var next = walkers[walkers.length - 1].nextNode()
                if (next) return next
                walkers.pop()
            }
            return null
        }
        while (!truncated) {
            if (visited >= input.max_nodes || performance.now() - started >= input.max_ms) {
                truncated = true
                break
            }
            var element = nextElement()
            if (!element) break
            visited++
            if (element === root || !element.isConnected || element.ownerDocument !== capturedDocument) continue
            // Publicly accessible open roots stay inside the same frame,
            // document and aggregate node budget. Closed roots cannot be
            // inspected through DOM APIs; this collector never guesses their
            // contents or treats the host as proof that a field was saved.
            var openShadowRoot = null
            try { openShadowRoot = element.shadowRoot } catch (shadowError) {}
            if (openShadowRoot) {
                if (openShadowRootsVisited >= input.max_nodes) {
                    truncated = true
                    exclude("limit")
                    break
                }
                try {
                    walkers.push(document.createTreeWalker(openShadowRoot, NodeFilter.SHOW_ELEMENT))
                    openShadowRootsVisited++
                } catch (shadowError) {
                    exclude("unsupported")
                }
            } else {
                if ((input.include_editable || input.include_selection)
                        && element.localName.indexOf("-") >= 0) {
                    // ApplicationWorld may not expose the page world's
                    // customElements registry. Treat a dashed custom-element
                    // host without a public open root conservatively; the
                    // public DOM surface cannot prove it has no closed root.
                    shadowScopeIncomplete = true
                    if (input.include_editable) editableRequested++
                    exclude("uninspectable-component")
                }
            }
            var mediaElement = element.tagName === "AUDIO" || element.tagName === "VIDEO"
            if (input.include_media && mediaElement) mediaRequested++
            var editable = editableKind(element)
            var editableElement = editable || element.tagName === "INPUT"
                || element.tagName === "TEXTAREA"
            if (input.include_editable && editableElement && sensitiveEditableField(element)) {
                editableRequested++
                exclude("sensitive-control")
                continue
            }
            if (input.include_editable && element.tagName === "INPUT" && !editable) {
                editableRequested++
                exclude("unsupported")
            }
            var style = getComputedStyle(element)
            if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse") {
                if (input.include_media && mediaElement) exclude("unsupported")
                continue
            }
            var rect = element.getBoundingClientRect()
            if (!(rect.width > 0 && rect.height > 0)) {
                if (input.include_media && mediaElement) exclude("unsupported")
                continue
            }
            var scrollX = ["auto", "scroll", "overlay"].indexOf(style.overflowX) >= 0
                && element.scrollWidth > element.clientWidth + 1
            var scrollY = ["auto", "scroll", "overlay"].indexOf(style.overflowY) >= 0
                && element.scrollHeight > element.clientHeight + 1
            var kind = scrollX || scrollY ? "scroll-area" : ""
            if (!kind && input.include_controls && element.tagName === "DETAILS") kind = "details"
            if (!kind && input.include_controls && element.tagName === "SELECT") kind = "select"
            if (!kind && (input.include_editable || input.include_selection) && editable) kind = editable
            if (!kind && input.include_media && mediaElement) kind = "media"
            if (!kind) continue
            var mediaSourceFingerprint = null
            var mediaDuration = 0, mediaTime = 0, mediaPaused = null
            if (kind === "media") {
                mediaSourceFingerprint = fingerprintMediaSource(element)
                mediaDuration = Number(element.duration)
                mediaTime = Number(element.currentTime)
                mediaPaused = element.paused
                if (!mediaSourceFingerprint || !!element.srcObject || !!element.mediaKeys
                        || element.readyState < 1 || hasLiveSeekWindow(element)
                        || !Number.isFinite(mediaDuration) || mediaDuration <= 0
                        || mediaDuration > MAX_MEDIA_SECONDS
                        || !numberInRange(mediaTime, 0, Math.min(mediaDuration, MAX_MEDIA_SECONDS))
                        || typeof mediaPaused !== "boolean") {
                    exclude("unsupported")
                    continue
                }
            }
            if (candidates.length >= input.max_candidates) { truncated = true; break }
            var item = candidate(element, kind, candidates.length + 1, mediaSourceFingerprint)
            if (!item) { exclude("unsupported"); continue }
            candidates.push(item)
            candidateElements.push(element)
            if (kind === "scroll-area") {
                if (scrollX) position(item.candidate_id, "element-scroll-x", Number(element.scrollLeft))
                if (scrollY) position(item.candidate_id, "element-scroll-y", Number(element.scrollTop))
            } else if (kind === "details" || kind === "select") {
                if (controls.length >= maxControls) { truncated = true; continue }
                if (kind === "details") {
                    controls.push({frame_id: frameId, candidate_id: item.candidate_id,
                                   value: {kind: "details", open: !!element.open}})
                } else {
                    var selected = [], options = element.selectedOptions
                    var validOptions = options.length <= 16
                    for (var optionIndex = 0; validOptions && optionIndex < options.length; optionIndex++) {
                        var raw = String(options[optionIndex].value)
                        var fingerprint = raw.length <= 256 ? sha256Hex("option:" + raw) : null
                        if (!fingerprint || selected.indexOf(fingerprint) >= 0) {
                            validOptions = false
                            break
                        }
                        selected.push(fingerprint)
                    }
                    if (validOptions) {
                        controls.push({frame_id: frameId, candidate_id: item.candidate_id,
                                       value: {kind: "select", option_fingerprints: selected}})
                    } else {
                        exclude(options.length > 16 ? "limit" : "unsupported")
                        if (options.length > 16) truncated = true
                    }
                }
            } else if (kind === "media") {
                position(item.candidate_id, "media-time", mediaTime)
                var mediaPosition = positions[positions.length - 1]
                if (mediaPosition && mediaPosition.kind === "media-time"
                        && mediaPosition.candidate_id === item.candidate_id) {
                    mediaPosition.paused = mediaPaused
                    mediaCaptured++
                }
            }
            // A scrollable control can contribute both geometry and editable
            // state. Keep that opt-in independent of the candidate's primary
            // geometry kind instead of silently dropping its value.
            if (input.include_editable && editable) addEditableValue(item, element, editable)
        }
        if (capturedDocument !== document || capturedUrl !== location.href) return fail("document-changed")
        if (input.include_selection) {
            var selection = window.getSelection()
            var active = document.activeElement
            var activeIndex = candidateElements.indexOf(active)
            var activeCandidate = activeIndex >= 0 ? candidates[activeIndex] : null
            var contentEditableCandidate = activeCandidate
                && activeCandidate.kind === "content-editable"
            var selectionStart = null, selectionEnd = null
            var selectionExclusion = "unsupported"
            if (activeCandidate && (["text", "textarea"].indexOf(activeCandidate.kind) >= 0
                                    || contentEditableCandidate)) {
                if (!contentEditableCandidate
                        && Number.isInteger(active.selectionStart)
                        && Number.isInteger(active.selectionEnd)) {
                    selectionStart = active.selectionStart
                    selectionEnd = active.selectionEnd
                } else if (contentEditableCandidate && selection && selection.rangeCount === 1) {
                    var range
                    try { range = selection.getRangeAt(0) } catch (rangeError) {}
                    if (range && active.contains(range.startContainer)
                            && active.contains(range.endContainer)
                            && range.startContainer.nodeType === Node.TEXT_NODE
                            && range.endContainer.nodeType === Node.TEXT_NODE) {
                        var startNode = range.startContainer
                        var endNode = range.endContainer
                        var startOffset = range.startOffset
                        var endOffset = range.endOffset
                        var textWalker = document.createTreeWalker(active, NodeFilter.SHOW_TEXT)
                        var textNode, textTotal = 0, textNodes = 0
                        var absoluteStart = null, absoluteEnd = null
                        var offsetFailure = null
                        while ((textNode = textWalker.nextNode())) {
                            if (visited + textNodes >= input.max_nodes) {
                                offsetFailure = "limit"
                                break
                            }
                            if (performance.now() - started >= input.max_ms) {
                                offsetFailure = "timeout"
                                break
                            }
                            textNodes++
                            var textNodeLength = String(textNode.nodeValue || "").length
                            if (textTotal + textNodeLength > MAX_EDITABLE_TOTAL_BYTES) {
                                offsetFailure = "limit"
                                break
                            }
                            if (textNode === startNode) {
                                if (startOffset > textNodeLength) {
                                    offsetFailure = "unsupported"
                                    break
                                }
                                absoluteStart = textTotal + startOffset
                            }
                            if (textNode === endNode) {
                                if (endOffset > textNodeLength) {
                                    offsetFailure = "unsupported"
                                    break
                                }
                                absoluteEnd = textTotal + endOffset
                            }
                            textTotal += textNodeLength
                            if (absoluteStart !== null && absoluteEnd !== null) break
                        }
                        visited += textNodes
                        if (absoluteStart !== null && absoluteEnd !== null
                                && absoluteStart <= MAX_EDITABLE_TOTAL_BYTES
                                && absoluteEnd <= MAX_EDITABLE_TOTAL_BYTES) {
                            selectionStart = absoluteStart
                            selectionEnd = absoluteEnd
                        } else {
                            selectionExclusion = offsetFailure || "unsupported"
                        }
                    }
                }
            }
            if (selectionStart !== null && selectionEnd !== null
                    && integerInRange(selectionStart, 0, MAX_EDITABLE_TOTAL_BYTES)
                    && integerInRange(selectionEnd, 0, MAX_EDITABLE_TOTAL_BYTES)
                    && positions.length + 2 <= maxPositions) {
                positions.push({frame_id: frameId, candidate_id: activeCandidate.candidate_id,
                                kind: "selection-start", value: selectionStart, paused: null})
                positions.push({frame_id: frameId, candidate_id: activeCandidate.candidate_id,
                                kind: "selection-end", value: selectionEnd, paused: null})
                selectionCaptured = 2
            } else {
                if (selectionExclusion === "limit" || selectionExclusion === "timeout") {
                    selectionTruncated = true
                }
                exclude(selectionExclusion)
            }
        }
        if (restore) {
            if (truncated || performance.now() - started >= input.max_ms) return fail("limit")
            if (String(location.origin) !== input.expected_origin
                    || location.pathname + location.search !== input.expected_route) {
                return fail("route-changed")
            }
            var applied = 0, skipped = 0
            var mediaApplied = 0, mediaSkipped = 0
            var seenTargetIds = new Set(), seenTargetDescriptors = new Set()
            for (var uniqueIndex = 0; uniqueIndex < restore.targets.length; uniqueIndex++) {
                var uniqueTarget = restore.targets[uniqueIndex]
                var uniqueId = uniqueTarget.candidate_id
                // INVARIANT: scan numbers are hints, not identity. A changed
                // traversal order may not select a different live element.
                var descriptor = [uniqueTarget.id_fingerprint,
                                  uniqueTarget.name_fingerprint,
                                  uniqueTarget.label_fingerprint,
                                  uniqueTarget.context_fingerprint].join(":")
                if (seenTargetIds.has(uniqueId) || seenTargetDescriptors.has(descriptor)) {
                    return fail("invalid-request")
                }
                seenTargetIds.add(uniqueId)
                seenTargetDescriptors.add(descriptor)
            }
            var mediaTargets = restore.media_targets || []
            var seenMediaTargetIds = new Set(), seenMediaTargetDescriptors = new Set()
            for (var mediaUniqueIndex = 0; mediaUniqueIndex < mediaTargets.length; mediaUniqueIndex++) {
                var mediaUniqueTarget = mediaTargets[mediaUniqueIndex]
                var mediaDescriptor = [mediaUniqueTarget.id_fingerprint,
                                       mediaUniqueTarget.name_fingerprint,
                                       mediaUniqueTarget.label_fingerprint,
                                       mediaUniqueTarget.context_fingerprint].join(":")
                if (seenMediaTargetIds.has(mediaUniqueTarget.candidate_id)
                        || seenMediaTargetDescriptors.has(mediaDescriptor)) {
                    return fail("invalid-request")
                }
                seenMediaTargetIds.add(mediaUniqueTarget.candidate_id)
                seenMediaTargetDescriptors.add(mediaDescriptor)
            }
            var editableTargets = restore.editable_targets || []
            var seenEditableTargetIds = new Set(), seenEditableTargetDescriptors = new Set()
            var editableBytesToApply = 0
            for (var editableUniqueIndex = 0; editableUniqueIndex < editableTargets.length; editableUniqueIndex++) {
                var editableUniqueTarget = editableTargets[editableUniqueIndex]
                var editableDescriptor = [editableUniqueTarget.id_fingerprint,
                                          editableUniqueTarget.name_fingerprint,
                                          editableUniqueTarget.label_fingerprint,
                                          editableUniqueTarget.context_fingerprint].join(":")
                if (seenEditableTargetIds.has(editableUniqueTarget.candidate_id)
                        || seenEditableTargetDescriptors.has(editableDescriptor)) {
                    return fail("invalid-request")
                }
                seenEditableTargetIds.add(editableUniqueTarget.candidate_id)
                seenEditableTargetDescriptors.add(editableDescriptor)
                if (editableUniqueTarget.value.kind === "text") {
                    editableBytesToApply += new TextEncoder().encode(
                        editableUniqueTarget.value.value).length
                }
            }
            if (editableBytesToApply > 16 * 1024) return fail("invalid-request")
            function fits(value, maximum) {
                return value === null || (value >= 0 && value <= maximum + 1)
            }
            function liveScrollArea(element, target) {
                if (!element.isConnected || element.ownerDocument !== capturedDocument) return false
                var style = getComputedStyle(element)
                if (style.display === "none" || style.visibility === "hidden"
                        || style.visibility === "collapse") return false
                var rect = element.getBoundingClientRect()
                if (!(rect.width > 0 && rect.height > 0)) return false
                var horizontal = ["auto", "scroll", "overlay"].indexOf(style.overflowX) >= 0
                    && element.scrollWidth > element.clientWidth + 1
                var vertical = ["auto", "scroll", "overlay"].indexOf(style.overflowY) >= 0
                    && element.scrollHeight > element.clientHeight + 1
                return (target.x === null || horizontal) && (target.y === null || vertical)
            }
            for (var targetIndex = 0; targetIndex < restore.targets.length; targetIndex++) {
                if (performance.now() - started >= input.max_ms) {
                    return JSON.stringify({ok: true, applied: applied,
                                           skipped: skipped + restore.targets.length - targetIndex,
                                           partial: true, reason: "limit"})
                }
                var target = restore.targets[targetIndex]
                var matching = []
                for (var candidateIndex = 0; candidateIndex < candidates.length; candidateIndex++) {
                    var item = candidates[candidateIndex]
                    // A repeated ID/name is ambiguous even if structure differs.
                    if (item.kind === "scroll-area"
                            && ((target.id_fingerprint !== null && item.id_fingerprint === target.id_fingerprint)
                                || (target.name_fingerprint !== null && item.name_fingerprint === target.name_fingerprint))) {
                        matching.push(candidateIndex)
                    }
                }
                if (matching.length !== 1 || !sameCandidate(target, candidates[matching[0]])) {
                    skipped++; continue
                }
                var live = candidateElements[matching[0]]
                if (!liveScrollArea(live, target)
                        || !fits(target.x, live.scrollWidth - live.clientWidth)
                        || !fits(target.y, live.scrollHeight - live.clientHeight)) {
                    skipped++; continue
                }
                if (target.x !== null) live.scrollLeft = target.x
                if (target.y !== null) live.scrollTop = target.y
                applied++
            }
            function sameMediaCandidate(target, item) {
                return item.kind === "media"
                    && item.id_fingerprint === target.id_fingerprint
                    && item.name_fingerprint === target.name_fingerprint
                    && item.label_fingerprint === target.label_fingerprint
                    && item.context_fingerprint === target.context_fingerprint
                    && typeof target.expected_source_url === "string"
            }
            for (var mediaIndex = 0; mediaIndex < mediaTargets.length; mediaIndex++) {
                if (performance.now() - started >= input.max_ms) {
                    return JSON.stringify({ok: true, applied: applied,
                                           skipped: skipped + restore.targets.length,
                                           media_applied: mediaApplied,
                                           media_skipped: mediaSkipped + mediaTargets.length - mediaIndex,
                                           partial: true, reason: "limit"})
                }
                var mediaTarget = mediaTargets[mediaIndex]
                var mediaMatching = []
                for (var mediaCandidateIndex = 0; mediaCandidateIndex < candidates.length; mediaCandidateIndex++) {
                    var mediaItem = candidates[mediaCandidateIndex]
                    if (mediaItem.kind === "media"
                            && ((mediaTarget.id_fingerprint !== null
                                 && mediaItem.id_fingerprint === mediaTarget.id_fingerprint)
                                || (mediaTarget.name_fingerprint !== null
                                    && mediaItem.name_fingerprint === mediaTarget.name_fingerprint))) {
                        mediaMatching.push(mediaCandidateIndex)
                    }
                }
                if (mediaMatching.length !== 1
                        || !sameMediaCandidate(mediaTarget, candidates[mediaMatching[0]])) {
                    mediaSkipped++
                    continue
                }
                var mediaElement = candidateElements[mediaMatching[0]]
                var mediaDuration = Number(mediaElement.duration)
                if (!mediaElement.isConnected || mediaElement.ownerDocument !== capturedDocument
                        || mediaElement.paused !== true || mediaElement.readyState < 1
                        || !!mediaElement.mediaKeys || !!mediaElement.srcObject
                        || fingerprintMediaSource(mediaElement) !== mediaTarget.expected_source_url
                        || hasLiveSeekWindow(mediaElement)
                        || !Number.isFinite(mediaDuration) || mediaDuration <= 0
                        || mediaDuration > MAX_MEDIA_SECONDS
                        || mediaTarget.time_seconds > mediaDuration) {
                    mediaSkipped++
                    continue
                }
                try {
                    // Explicitly opted-in, one-shot position assignment only.
                    // INVARIANT: never call play(), pause(), load(), or any capture API.
                    mediaElement.currentTime = mediaTarget.time_seconds
                    if (Number.isFinite(Number(mediaElement.currentTime))
                            && Math.abs(Number(mediaElement.currentTime)
                                        - mediaTarget.time_seconds) <= 1) {
                        mediaApplied++
                    } else {
                        mediaSkipped++
                    }
                } catch (mediaSeekError) {
                    mediaSkipped++
                }
            }
            function sameEditableCandidate(target, item) {
                return item.kind === target.kind
                    && item.id_fingerprint === target.id_fingerprint
                    && item.name_fingerprint === target.name_fingerprint
                    && item.label_fingerprint === target.label_fingerprint
                    && item.context_fingerprint === target.context_fingerprint
            }
            function liveEditableTargetValid(element, target) {
                if (!element.isConnected || element.ownerDocument !== capturedDocument
                        || sensitiveEditableField(element)) return false
                var style = getComputedStyle(element)
                if (style.display === "none" || style.visibility === "hidden"
                        || style.visibility === "collapse") return false
                var rect = element.getBoundingClientRect()
                if (!(rect.width > 0 && rect.height > 0)) return false
                if (target.kind === "text") {
                    return editableKind(element) === "text" && element.value === ""
                }
                if (target.kind === "textarea") {
                    return editableKind(element) === "textarea" && element.value === ""
                }
                return target.kind === "checkbox" && editableKind(element) === "checkbox"
                    && element.checked === false
            }
            var editableApplied = 0, editableSkipped = 0, editableUnaccepted = 0
            for (var editableIndex = 0; editableIndex < editableTargets.length; editableIndex++) {
                if (performance.now() - started >= input.max_ms) {
                    return JSON.stringify({ok: true, applied: applied,
                                           skipped: skipped + restore.targets.length,
                                           media_applied: mediaApplied,
                                           media_skipped: mediaSkipped,
                                           editable_applied: editableApplied,
                                           editable_skipped: editableSkipped
                                               + editableTargets.length - editableIndex,
                                           editable_unaccepted: editableUnaccepted,
                                           partial: true, reason: "limit"})
                }
                var editableTarget = editableTargets[editableIndex]
                var editableMatching = []
                for (var editableCandidateIndex = 0; editableCandidateIndex < candidates.length; editableCandidateIndex++) {
                    var editableItem = candidates[editableCandidateIndex]
                    if ((editableTarget.id_fingerprint !== null
                         && editableItem.id_fingerprint === editableTarget.id_fingerprint)
                            || (editableTarget.name_fingerprint !== null
                                && editableItem.name_fingerprint === editableTarget.name_fingerprint)) {
                        editableMatching.push(editableCandidateIndex)
                    }
                }
                if (editableMatching.length !== 1
                        || !sameEditableCandidate(editableTarget, candidates[editableMatching[0]])) {
                    editableSkipped++
                    continue
                }
                var editableElement = candidateElements[editableMatching[0]]
                if (!liveEditableTargetValid(editableElement, editableTarget)) {
                    editableSkipped++
                    continue
                }
                try {
                    // INVARIANT: assign a native property only; never focus or
                    // dispatch input/change/submit events or inject HTML.
                    if (editableTarget.value.kind === "text") {
                        editableElement.value = editableTarget.value.value
                        if (editableElement.value === editableTarget.value.value) editableApplied++
                        else { editableSkipped++; editableUnaccepted++ }
                    } else {
                        editableElement.checked = true
                        if (editableElement.checked === true) editableApplied++
                        else { editableSkipped++; editableUnaccepted++ }
                    }
                } catch (editableError) {
                    editableSkipped++
                    editableUnaccepted++
                }
            }
            if (capturedDocument !== document || capturedUrl !== location.href) return fail("document-changed")
            if (restore.top_x !== null || restore.top_y !== null) {
                if (performance.now() - started >= input.max_ms) {
                    return JSON.stringify({ok: true, applied: applied, skipped: skipped + 1,
                                           partial: true, reason: "limit"})
                }
                if (fits(restore.top_x, root.scrollWidth - root.clientWidth)
                        && fits(restore.top_y, root.scrollHeight - root.clientHeight)) {
                    if (restore.top_x !== null) root.scrollLeft = restore.top_x
                    if (restore.top_y !== null) root.scrollTop = restore.top_y
                    applied++
                } else { skipped++ }
            }
            return JSON.stringify({ok: true, applied: applied, skipped: skipped,
                                   media_applied: mediaApplied, media_skipped: mediaSkipped,
                                   editable_applied: editableApplied,
                                   editable_skipped: editableSkipped,
                                   editable_unaccepted: editableUnaccepted})
        }
        if (truncated) exclude("limit")
        var requestedCategories = ["geometry"]
        var geometryPositionCount = positions.filter(function(position) {
            return ["top-scroll-x", "top-scroll-y", "element-scroll-x", "element-scroll-y"]
                .indexOf(position.kind) >= 0
        }).length
        var coverage = [{category: "geometry",
                         requested: Math.max(1, geometryPositionCount + (truncated ? 1 : 0)),
                         captured: geometryPositionCount, truncated: truncated}]
        if (input.include_controls) {
            requestedCategories.push("controls")
            coverage.push({category: "controls", requested: Math.max(1, controls.length + (truncated ? 1 : 0)),
                           captured: controls.length, truncated: truncated})
        }
        if (input.include_editable) {
            requestedCategories.push("editable")
            coverage.push({category: "editable", requested: Math.max(1, editableRequested),
                           captured: sensitiveEditable.length,
                           truncated: truncated || shadowScopeIncomplete})
        }
        if (input.include_selection) {
            requestedCategories.push("selection")
            coverage.push({category: "selection", requested: 2,
                           captured: selectionCaptured,
                           truncated: truncated || shadowScopeIncomplete || selectionTruncated})
        }
        if (input.include_media) {
            requestedCategories.push("media")
            coverage.push({category: "media", requested: Math.max(1, mediaRequested),
                           captured: mediaCaptured, truncated: truncated})
        }
        var result = {
            schema_version: 1, helper_version: "ferric-page-state-5", nonce: input.nonce,
            capture_fingerprint: input.capture_fingerprint,
            requested_categories: requestedCategories,
            frames: [{id: frameId, parent: parentFrameId, origin: origin,
                      route_fingerprint: frameRouteFingerprint}],
            candidates: candidates, positions: positions, controls: controls,
            coverage: coverage, exclusions: exclusions, sensitive_editable: sensitiveEditable
        }
        if (input.report_nodes) result.nodes_visited = visited
        var encoded = JSON.stringify(result)
        return encoded.length <= 256 * 1024 ? encoded : fail("result-too-large")
    } catch (error) {
        return fail("script-error")
    }
}`

var MUTATION_PROBE_SOURCE = String.raw`function(input) {
    "use strict"
    var key = "__ferricVersionedPageMutation"
    var state = window[key]
    if (window.__ferricPageStateChallenge
            !== input.nonce + ":" + input.capture_fingerprint) {
        if (state) {
            try { state.observer.disconnect() } catch (ignored) {}
            try { if (state.expiry !== null) window.clearTimeout(state.expiry) } catch (ignored) {}
            try { delete window[key] } catch (ignored) {}
        }
        return "document-changed"
    }
    if (!state || state.nonce !== input.nonce
            || state.captureFingerprint !== input.capture_fingerprint
            || state.document !== document || !state.observer) return "not-observed"
    try {
        if (state.observer.takeRecords().length > 0) state.changed = true
        state.observer.disconnect()
        if (state.expiry !== null) window.clearTimeout(state.expiry)
        delete window[key]
        return state.changed ? "observed-changed" : "observed-stable"
    } catch (error) {
        try { state.observer.disconnect() } catch (ignored) {}
        try { delete window[key] } catch (ignored) {}
        return "not-observed"
    }
}`
