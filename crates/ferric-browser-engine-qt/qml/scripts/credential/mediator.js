// Runs only in the centrally reserved credential world, in the main frame.
// Channel delivery supplies no caller identity. Native code challenges this
// private main-frame state and corroborates the native owner before use.
(function () {
    "use strict";
    if (window !== window.top || !window.isSecureContext || location.protocol !== "https:") return;
    const documentNonce = crypto.randomUUID();
    let pending = null;
    let receiver = null;
    const sendNotification = function () { if (receiver) receiver.notify(); };
    new QWebChannel(qt.webChannelTransport, function (channel) {
        receiver = channel.objects.credentialNotification;
        if (pending) sendNotification();
    });
    window.addEventListener("message", function (event) {
        if (!event.isTrusted || event.source !== window || event.origin !== location.origin) return;
        const data = event.data;
        if (!data || typeof data !== "object" || typeof data.id !== "string"
                || !/^[\x21-\x7e]{1,128}$/.test(data.id)) return;
        if (data.type === "ferric.credentials.cancel.v1") {
            if (pending && pending.id === data.id && !pending.cancelled) {
                pending.cancelled = true;
                sendNotification();
            }
            return;
        }
        if (data.type !== "ferric.credentials.request.v1" || pending
                || (data.op !== "get" && data.op !== "create")) return;
        let encoded;
        try { encoded = JSON.stringify(data.options); } catch (_) { return; }
        if (typeof encoded !== "string" || new TextEncoder().encode(encoded).length > 1048576) return;
        pending = {id: data.id, op: data.op, options: JSON.parse(encoded),
                   requestNonce: crypto.randomUUID(), cancelled: false};
        sendNotification();
    });
    window.addEventListener("pagehide", function () { pending = null; });
    Object.defineProperty(globalThis, "__ferricCredentialMediator", {value: Object.freeze({
        challenge: function (challenge) {
            if (!pending) return "";
            const policy = document.permissionsPolicy || document.featurePolicy;
            const feature = "publickey-credentials-" + pending.op;
            return JSON.stringify({challenge: challenge, documentNonce: documentNonce,
                requestNonce: pending.requestNonce, id: pending.id, op: pending.op,
                options: pending.options, cancelled: pending.cancelled,
                origin: location.origin, secureContext: window.isSecureContext,
                policyAllowed: !!policy && policy.allowsFeature(feature)});
        },
        complete: function (requestNonce, expectedDocument, expectedOrigin, response) {
            if (!pending || pending.requestNonce !== requestNonce || documentNonce !== expectedDocument
                    || location.origin !== expectedOrigin || !window.isSecureContext) return false;
            if (pending.cancelled && (!response || response.error !== "AbortError")) return false;
            const policy = document.permissionsPolicy || document.featurePolicy;
            if (!policy || !policy.allowsFeature("publickey-credentials-" + pending.op)) return false;
            const id = pending.id;
            pending = null;
            window.postMessage({type: "ferric.credentials.response.v1", id: id,
                                response: response}, location.origin);
            return true;
        }
    }), writable: false, configurable: false});
})();
