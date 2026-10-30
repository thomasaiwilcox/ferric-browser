// Credential-only MainWorld adapter. Installation is gated separately from
// the private receiver; this file never grants native or helper authority.
(function () {
    "use strict";
    if (window !== window.top || !isSecureContext || location.protocol !== "https:"
            || typeof CredentialsContainer === "undefined" || !navigator.credentials
            || typeof PublicKeyCredential === "undefined") return;
    const prototype = CredentialsContainer.prototype;
    if (Object.prototype.hasOwnProperty.call(prototype, "__ferricCredentialShimV1")) return;
    Object.defineProperty(prototype, "__ferricCredentialShimV1", {value: true});
    const originals = {create: prototype.create, get: prototype.get};
    const apply = Reflect.apply;
    const byteLength = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength").get;
    const isView = ArrayBuffer.isView;
    const credentialRecords = new WeakMap();
    const responseRecords = new WeakMap();
    let pending = null;

    function bytes(value) {
        let buffer = value, offset = 0, length;
        if (isView(value)) {
            buffer = value.buffer;
            offset = value.byteOffset;
            length = value.byteLength;
        }
        // The intrinsic accepts cross-realm ArrayBuffers and rejects shared
        // buffers and objects that merely imitate BufferSource properties.
        const total = apply(byteLength, buffer, []);
        const size = length === undefined ? total : length;
        if (size > 1048576) throw new TypeError("Credential bytes are too large");
        return new Uint8Array(buffer, offset, size).slice();
    }
    function encode(value) {
        const data = bytes(value);
        let text = "";
        for (let offset = 0; offset < data.length; offset += 4096)
            text += String.fromCharCode.apply(null, data.subarray(offset, offset + 4096));
        return btoa(text).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    }
    function decode(value) {
        if (typeof value !== "string" || !/^[A-Za-z0-9_-]*$/.test(value) || value.length % 4 === 1)
            throw new TypeError("Invalid credential byte encoding");
        const binary = atob(value.replace(/-/g, "+").replace(/_/g, "/"));
        const data = Uint8Array.from(binary, ch => ch.charCodeAt(0));
        if (encode(data) !== value) throw new TypeError("Noncanonical credential byte encoding");
        return data.buffer;
    }
    function clone(value) { return JSON.parse(JSON.stringify(value)); }
    function prfInput(value) {
        const result = clone(value);
        if (value.first !== undefined) result.first = encode(value.first);
        if (value.second !== undefined) result.second = encode(value.second);
        return result;
    }
    function optionsJSON(options, operation) {
        const source = options.publicKey;
        const result = clone(source);
        // Snapshot dictionary values once before crossing the message boundary.
        const fields = operation === "create"
            ? ["rp", "pubKeyCredParams", "timeout", "authenticatorSelection", "hints", "attestation", "attestationFormats"]
            : ["timeout", "rpId", "userVerification", "hints"];
        for (const field of fields) if (source[field] !== undefined) result[field] = clone(source[field]);
        result.challenge = encode(source.challenge);
        if (operation === "create") {
            const user = source.user;
            if (user.name === undefined || user.displayName === undefined) throw new TypeError("User name is required");
            result.user = Object.assign(clone(user), {id: encode(user.id), name: String(user.name), displayName: String(user.displayName)});
        }
        const descriptors = operation === "create" ? "excludeCredentials" : "allowCredentials";
        if (source[descriptors] !== undefined) result[descriptors] = Array.from(source[descriptors], entry => {
            const value = {type: String(entry.type), id: encode(entry.id)};
            if (entry.transports !== undefined) value.transports = Array.from(entry.transports, String);
            return value;
        });
        if (source.extensions !== undefined) {
            const extensions = source.extensions;
            result.extensions = Object.create(null);
            for (const key of Object.keys(extensions)) {
                if (key === "largeBlob") {
                    result.extensions[key] = Object.create(null);
                    for (const field of Object.keys(extensions[key]))
                        result.extensions[key][field] = field === "write" ? encode(extensions[key][field]) : clone(extensions[key][field]);
                } else if (key === "prf") {
                    const prf = extensions[key], value = clone(prf);
                    if (prf.eval !== undefined) value.eval = prfInput(prf.eval);
                    if (prf.evalByCredential !== undefined) {
                        value.evalByCredential = Object.create(null);
                        for (const id of Object.keys(prf.evalByCredential)) value.evalByCredential[id] = prfInput(prf.evalByCredential[id]);
                    }
                    result.extensions[key] = value;
                } else result.extensions[key] = clone(extensions[key]);
            }
        }
        return result;
    }
    function extensionResults(value) {
        const result = clone(value || {});
        if (result.prf && result.prf.results) {
            for (const key of ["first", "second"])
                if (result.prf.results[key] !== undefined) result.prf.results[key] = decode(result.prf.results[key]);
        }
        if (result.largeBlob && result.largeBlob.blob !== undefined) result.largeBlob.blob = decode(result.largeBlob.blob);
        return result;
    }
    function phoneMethod(prototype, name, records, operation, implementation) {
        const original = prototype[name];
        if (typeof original !== "function") return;
        Object.defineProperty(prototype, name, {value: function (...args) {
            const record = records.get(this);
            if (record && (!operation || record.operation === operation)) return implementation(record, this);
            return apply(original, this, args);
        }, writable: true, configurable: true});
    }
    phoneMethod(PublicKeyCredential.prototype, "getClientExtensionResults", credentialRecords, null,
        record => extensionResults(record.json.clientExtensionResults));
    phoneMethod(PublicKeyCredential.prototype, "toJSON", credentialRecords, null, (record, credential) => {
        const value = clone(record.json), response = credential.response;
        value.rawId = encode(credential.rawId);
        value.response.clientDataJSON = encode(response.clientDataJSON);
        if (record.operation === "create") value.response.attestationObject = encode(response.attestationObject);
        else {
            value.response.authenticatorData = encode(response.authenticatorData);
            value.response.signature = encode(response.signature);
            value.response.userHandle = response.userHandle === null ? null : encode(response.userHandle);
        }
        return value;
    });
    phoneMethod(AuthenticatorAttestationResponse.prototype, "getAuthenticatorData", responseRecords, "create",
        record => record.authenticatorData.slice(0));
    phoneMethod(AuthenticatorAttestationResponse.prototype, "getPublicKey", responseRecords, "create",
        record => record.publicKey === null ? null : record.publicKey.slice(0));
    phoneMethod(AuthenticatorAttestationResponse.prototype, "getPublicKeyAlgorithm", responseRecords, "create",
        record => record.data.publicKeyAlgorithm);
    phoneMethod(AuthenticatorAttestationResponse.prototype, "getTransports", responseRecords, "create",
        record => record.data.transports.slice());

    function credentialJSON(input, operation) {
        const value = clone(input);
        if (!value || value.type !== "public-key" || typeof value.id !== "string" || !value.response)
            throw new TypeError("Invalid credential response");
        const rawId = decode(value.rawId);
        if (!rawId.byteLength || encode(rawId) !== value.id) throw new TypeError("Invalid credential identity");
        const response = Object.create(operation === "create"
            ? AuthenticatorAttestationResponse.prototype : AuthenticatorAssertionResponse.prototype);
        const data = value.response;
        const properties = {clientDataJSON: {value: decode(data.clientDataJSON), enumerable: true}};
        const responseRecord = {operation, data};
        if (operation === "create") {
            properties.attestationObject = {value: decode(data.attestationObject), enumerable: true};
            responseRecord.authenticatorData = decode(data.authenticatorData);
            responseRecord.publicKey = data.publicKey == null ? null : decode(data.publicKey);
            if (!Number.isInteger(data.publicKeyAlgorithm) || !Array.isArray(data.transports)
                || !data.transports.every(item => typeof item === "string")) throw new TypeError("Invalid attestation response");
        } else {
            properties.authenticatorData = {value: decode(data.authenticatorData), enumerable: true};
            properties.signature = {value: decode(data.signature), enumerable: true};
            properties.userHandle = {value: data.userHandle == null ? null : decode(data.userHandle), enumerable: true};
        }
        Object.defineProperties(response, properties);
        responseRecords.set(response, responseRecord);
        const credential = Object.create(PublicKeyCredential.prototype);
        Object.defineProperties(credential, {
            id: {value: value.id, enumerable: true}, rawId: {value: rawId, enumerable: true},
            type: {value: "public-key", enumerable: true}, response: {value: response, enumerable: true},
            authenticatorAttachment: {value: value.authenticatorAttachment ?? null, enumerable: true}
        });
        credentialRecords.set(credential, {operation, json: value});
        return credential;
    }
    function clear(request) {
        if (pending !== request) return;
        pending = null;
        clearTimeout(request.timer);
        if (request.signal) request.signal.removeEventListener("abort", request.abort);
    }
    function reject(request, reason) { clear(request); request.reject(reason); }
    function cancel(request, reason) {
        window.postMessage({type: "ferric.credentials.cancel.v1", id: request.id}, location.origin);
        reject(request, reason);
    }
    function invoke(receiver, options, operation) {
        // Preserve the original methods and options for native-only requests.
        const policy = document.permissionsPolicy || document.featurePolicy;
        if (receiver !== navigator.credentials || !options || !options.publicKey
                || !policy || !policy.allowsFeature("publickey-credentials-" + operation)
                || options.password || options.federated || options.identity || options.otp || options.digital
                || (options.publicKey.extensions && ("credBlob" in options.publicKey.extensions
                    || "getCredBlob" in options.publicKey.extensions))
                || (operation === "create" && options.publicKey.attestation === "enterprise")
                || (options.mediation && options.mediation !== "optional" && options.mediation !== "required")
                || (operation === "create" && options.publicKey.authenticatorSelection
                    && options.publicKey.authenticatorSelection.authenticatorAttachment === "platform"))
            return apply(originals[operation], receiver, [options]);
        return new Promise((resolve, rejectPromise) => {
            if (pending) { rejectPromise(new DOMException("Another credential request is active", "NotAllowedError")); return; }
            let encoded;
            try { encoded = optionsJSON(options, operation); }
            catch (error) { rejectPromise(error); return; }
            if (new TextEncoder().encode(JSON.stringify(encoded)).length > 1048576) {
                rejectPromise(new TypeError("Credential request is too large")); return;
            }
            const request = {id: crypto.randomUUID(), operation, receiver, options, resolve, reject: rejectPromise,
                signal: options.signal, abort: null, timer: null};
            pending = request;
            request.abort = () => cancel(request, request.signal.reason);
            if (request.signal) {
                try {
                    const aborted = apply(Object.getOwnPropertyDescriptor(AbortSignal.prototype, "aborted").get, request.signal, []);
                    if (aborted) { reject(request, request.signal.reason); return; }
                    request.signal.addEventListener("abort", request.abort, {once: true});
                } catch (error) { reject(request, error); return; }
            }
            const timeout = Number(encoded.timeout);
            request.timer = setTimeout(() => cancel(request, new DOMException("Credential request timed out", "NotAllowedError")),
                Number.isFinite(timeout) && timeout >= 0 ? Math.min(timeout, 180000) : 180000);
            window.postMessage({type: "ferric.credentials.request.v1", id: request.id, op: operation, options: encoded}, location.origin);
        });
    }
    window.addEventListener("message", event => {
        if (!event.isTrusted || event.source !== window || event.origin !== location.origin || !pending) return;
        const data = event.data, request = pending;
        if (!data || data.type !== "ferric.credentials.response.v1" || data.id !== request.id || request.providerChosen) return;
        if (request.signal && request.signal.aborted) { reject(request, request.signal.reason); return; }
        const response = data.response;
        if (!response || typeof response !== "object") return;
        if (response.native === true) {
            request.providerChosen = true;
            clearTimeout(request.timer);
            Promise.resolve().then(() => {
                if (pending !== request) throw new DOMException("Request was cancelled", "AbortError");
                return apply(originals[request.operation], request.receiver, [request.options]);
            })
                .then(value => { clear(request); request.resolve(value); }, error => reject(request, error));
        } else if (typeof response.error === "string") {
            const names = ["AbortError", "NotAllowedError", "SecurityError", "NotSupportedError", "InvalidStateError", "UnknownError"];
            reject(request, response.error === "TypeError" ? new TypeError("Invalid credential request")
                : new DOMException("Credential request failed", names.includes(response.error) ? response.error : "UnknownError"));
        } else if (response.credential) {
            try { const value = credentialJSON(response.credential, request.operation); clear(request); request.resolve(value); }
            catch (error) { reject(request, error); }
        }
    });
    window.addEventListener("pagehide", () => {
        if (pending) reject(pending, new DOMException("Document left", "AbortError"));
    });
    Object.defineProperties(prototype, {
        create: {value: function create(options) { return invoke(this, options, "create"); }, writable: true, configurable: true},
        get: {value: function get(options) { return invoke(this, options, "get"); }, writable: true, configurable: true}
    });
})();
