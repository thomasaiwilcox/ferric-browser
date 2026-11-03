#!/usr/bin/python3
"""Disposable pipe fixture: no authenticator, Bluetooth, network or real key."""
import base64
import hashlib
import json
import os
import sys
import time


def encode(value):
    return base64.urlsafe_b64encode(value).decode().rstrip("=")


def record(kind, mode=None):
    with open("/proc/self/stat", encoding="ascii") as status:
        started = status.read().split()[21]
    value = {"pid": os.getpid(), "start": started, "kind": kind}
    if mode is not None:
        value["mode"] = mode
    with open(os.environ["FERRIC_PHONE_ACTOR_RECEIPT"], "a", encoding="utf8") as output:
        output.write(json.dumps(value, separators=(",", ":")) + "\n")


def main():
    record("started")
    line = sys.stdin.buffer.readline(1024 * 1024 + 1)
    if not line or not line.endswith(b"\n") or len(line) > 1024 * 1024:
        return 2
    request = json.loads(line)
    if request["network"] != {"mode": "direct"} or not request["origin"].startswith("https://localhost:"):
        return 3
    challenge = request["options"]["challenge"]
    mode = base64.urlsafe_b64decode(challenge + "=" * (-len(challenge) % 4))[0]
    if request["op"] != ("create" if mode in (17, 18) else "get"):
        return 3
    record("request", mode)

    def frame(kind, body, identity=None):
        print(json.dumps({"v": 1, "id": request["id"] if identity is None else identity, "kind": kind, "body": body}, separators=(",", ":")), flush=True)

    if mode == 8:
        frame("error", {"code": "security_error", "implemented": False})
        return 0
    frame("qr", {"width": 21, "modules": [0] * 441})
    frame("status", {"status": "submitted"})
    if mode == 18:
        if not request["options"].get("excludeCredentials"):
            return 3
        frame("error", {"code": "credential_excluded"})
        return 0
    if mode == 15:
        frame("status", {"status": "submitted"})
    if mode in (6, 10, 11, 12, 13, 16, 19):
        time.sleep(60)  # Native cancellation must terminate/reap this process.
        return 4

    def credential(index):
        client = json.dumps({"type": "webauthn.create" if mode == 17 else "webauthn.get", "challenge": challenge, "origin": request["origin"], "crossOrigin": False}, separators=(",", ":")).encode()
        identity = encode(f"fixture-{index}".encode())
        value = {"id": identity, "rawId": identity, "type": "public-key", "authenticatorAttachment": "cross-platform",
                "clientExtensionResults": {}, "response": {"clientDataJSON": encode(client),
                    "authenticatorData": encode(hashlib.sha256(b"localhost").digest() + b"\x05\x00\x00\x00\x00"),
                    "signature": "AQ", "userHandle": encode(f"user-{index}".encode())}}
        if mode == 17:
            value["response"].pop("signature")
            value["response"].pop("userHandle")
            value["response"].update(attestationObject="oA", publicKey=None, publicKeyAlgorithm=-7, transports=["hybrid"])
        return value

    # These bytes deliberately have no valid signature. They exercise browser
    # delivery/IDL reconstruction only, and never qualify an RP or device.
    credentials = [credential(1), credential(2)] if mode in (2, 14) else [credential(1)]
    frame("status", {"status": "completed"})
    frame("result", credentials[0] if mode == 17 else {"assertions": credentials, "accounts": ["Alice", "Bob"] if mode in (2, 14) else [None]},
          "stale-request" if mode == 7 else None)
    if mode == 3:
        time.sleep(3)
    if mode == 4:
        print("trailing-output", flush=True)
    return 7 if mode == 5 else 0


if __name__ == "__main__":
    raise SystemExit(main())
