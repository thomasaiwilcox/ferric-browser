#!/usr/bin/env python3
"""Disposable HTTPS relying-party fixture using Yubico's independent fido2 verifier."""

from __future__ import annotations

import argparse
import http.server
import ipaddress
import json
import os
import secrets
import ssl
import tempfile
import threading
from collections.abc import Mapping
from datetime import datetime, timedelta, timezone
from enum import Enum
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import ExtendedKeyUsageOID, NameOID
from fido2.server import Fido2Server
from fido2.utils import websafe_encode
from fido2.webauthn import (
    AttestationConveyancePreference,
    AuthenticationResponse,
    PublicKeyCredentialRpEntity,
    PublicKeyCredentialUserEntity,
    UserVerificationRequirement,
)

MAX_BODY_BYTES = 65_536
FIXTURE_HTML = Path(__file__).parents[1] / "fixtures" / "phone-auth.html"


def _origin_is_valid_for_rp(origin: str, rp_id: str) -> bool:
    parsed = urlsplit(origin)
    origin_host = (parsed.hostname or "").lower().rstrip(".")
    rp_host = rp_id.lower().rstrip(".")
    return bool(
        parsed.scheme == "https"
        and origin_host
        and parsed.username is None
        and parsed.password is None
        and parsed.path in ("", "/")
        and not parsed.query
        and not parsed.fragment
        and (origin_host == rp_host or origin_host.endswith("." + rp_host))
    )


def _json_value(value: Any) -> Any:
    """Adapt python-fido2's WebAuthnData mappings for browser JSON options."""
    if isinstance(value, Enum):
        return value.value
    if isinstance(value, bytes):
        return websafe_encode(value)
    if isinstance(value, Mapping):
        return {key: _json_value(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_json_value(item) for item in value]
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    raise TypeError(f"Unsupported WebAuthn option value: {type(value).__name__}")


def _write_private(path: Path, data: bytes) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as output:
        output.write(data)


def _make_loopback_certificate(directory: Path) -> tuple[Path, Path]:
    """Create a one-use self-signed localhost trust anchor inside the temp root."""
    private_key = ec.generate_private_key(ec.SECP256R1())
    name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "P13 disposable localhost")])
    now = datetime.now(timezone.utc)
    certificate = (
        x509.CertificateBuilder()
        .subject_name(name)
        .issuer_name(name)
        .public_key(private_key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(now - timedelta(minutes=1))
        .not_valid_after(now + timedelta(days=1))
        .add_extension(x509.BasicConstraints(ca=True, path_length=0), critical=True)
        .add_extension(
            x509.SubjectAlternativeName(
                [x509.DNSName("localhost"), x509.IPAddress(ipaddress.ip_address("127.0.0.1"))]
            ),
            critical=False,
        )
        .add_extension(x509.ExtendedKeyUsage([ExtendedKeyUsageOID.SERVER_AUTH]), critical=False)
        .sign(private_key, hashes.SHA256())
    )
    cert_path = directory / "loopback-cert.pem"
    key_path = directory / "loopback-key.pem"
    _write_private(
        key_path,
        private_key.private_bytes(
            serialization.Encoding.PEM,
            serialization.PrivateFormat.PKCS8,
            serialization.NoEncryption(),
        ),
    )
    _write_private(cert_path, certificate.public_bytes(serialization.Encoding.PEM))
    return cert_path, key_path


class VerifierApplication:
    def __init__(self, rp_id: str, origin: str):
        self.rp = PublicKeyCredentialRpEntity(id=rp_id, name="Disposable P13 RP")
        self.origin = origin
        self.verifier = Fido2Server(
            self.rp,
            attestation=AttestationConveyancePreference.NONE,
            verify_origin=lambda observed: observed == self.origin,
        )
        self.registration_states: dict[str, Any] = {}
        self.assertion_states: dict[str, Any] = {}
        self.credential = None
        self.signature_counter = 0
        self.account_id: str | None = None

    def clear_account(self) -> None:
        self.registration_states.clear()
        self.assertion_states.clear()
        self.credential = None
        self.signature_counter = 0
        self.account_id = None


class _RequestHandler(http.server.BaseHTTPRequestHandler):
    server_version = "P13VerifierFixture/1"
    sys_version = ""

    @property
    def app(self) -> VerifierApplication:
        return self.server.app  # type: ignore[attr-defined]

    def log_message(self, _format: str, *args: object) -> None:
        # Requests contain challenges, credential material and signatures.
        # The fixture deliberately emits no request-body or URL logs.
        return

    def _respond(self, status: int, payload: dict[str, Any]) -> None:
        body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header(
            "Content-Security-Policy",
            "default-src 'self'; script-src 'self' 'unsafe-inline'; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'",
        )
        self.end_headers()
        self.wfile.write(body)

    def _read_json(self) -> dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        if length <= 0 or length > MAX_BODY_BYTES:
            raise ValueError("invalid request size")
        payload = json.loads(self.rfile.read(length))
        if not isinstance(payload, dict):
            raise ValueError("request must be an object")
        return payload

    def do_GET(self) -> None:  # noqa: N802 - stdlib handler API
        if self.path not in ("/", "/index.html"):
            self._respond(404, {"error": "not_found"})
            return
        try:
            body = FIXTURE_HTML.read_bytes()
        except OSError:
            self._respond(500, {"error": "fixture_unavailable"})
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header(
            "Content-Security-Policy",
            "default-src 'self'; script-src 'self' 'unsafe-inline'; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'",
        )
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self) -> None:  # noqa: N802 - stdlib handler API
        try:
            request = self._read_json()
            if self.path == "/rp/register/begin":
                if self.app.credential is not None:
                    self._respond(409, {"error": "account_exists"})
                    return
                self.app.account_id = secrets.token_urlsafe(12)
                user = PublicKeyCredentialUserEntity(
                    id=secrets.token_bytes(32),
                    name=self.app.account_id,
                    display_name="Disposable test account",
                )
                options, state = self.app.verifier.register_begin(
                    user,
                    user_verification=UserVerificationRequirement.REQUIRED,
                )
                transaction = secrets.token_urlsafe(18)
                self.app.registration_states[transaction] = state
                self._respond(
                    200,
                    {"transaction": transaction, "publicKey": _json_value(options["publicKey"])},
                )
                return

            if self.path == "/rp/register/complete":
                transaction = request.get("transaction")
                state = self.app.registration_states.pop(transaction, None)
                if state is None:
                    self._respond(400, {"error": "verification_failed"})
                    return
                auth_data = self.app.verifier.register_complete(state, request.get("credential"))
                self.app.credential = auth_data.credential_data
                self.app.signature_counter = auth_data.counter
                self._respond(200, {"ok": True})
                return

            if self.path == "/rp/assertion/begin":
                if self.app.credential is None:
                    self._respond(409, {"error": "no_credential"})
                    return
                options, state = self.app.verifier.authenticate_begin(
                    credentials=[self.app.credential],
                    user_verification=UserVerificationRequirement.REQUIRED,
                )
                transaction = secrets.token_urlsafe(18)
                self.app.assertion_states[transaction] = state
                self._respond(
                    200,
                    {"transaction": transaction, "publicKey": _json_value(options["publicKey"])},
                )
                return

            if self.path == "/rp/assertion/complete":
                transaction = request.get("transaction")
                state = self.app.assertion_states.pop(transaction, None)
                if state is None or self.app.credential is None:
                    self._respond(400, {"error": "verification_failed"})
                    return
                response = request.get("credential")
                verified = self.app.verifier.authenticate_complete(
                    state,
                    [self.app.credential],
                    response,
                )
                parsed = AuthenticationResponse.from_dict(response)
                count = parsed.response.authenticator_data.counter
                # python-fido2 validates signature, client data, RP hash, UP
                # and required UV. Counter freshness is RP state policy here.
                if verified.credential_id != self.app.credential.credential_id:
                    raise ValueError("unexpected credential")
                if count <= self.app.signature_counter:
                    raise ValueError("signature counter did not increase")
                self.app.signature_counter = count
                self._respond(200, {"ok": True})
                return

            if self.path == "/rp/cleanup":
                self.app.clear_account()
                self._respond(200, {"ok": True})
                return

            self._respond(404, {"error": "not_found"})
        except (KeyError, TypeError, ValueError, json.JSONDecodeError):
            # Do not reflect library exception text or WebAuthn response data.
            self._respond(400, {"error": "verification_failed"})


class VerifierFixture:
    """One HTTPS RP whose account, challenges, key and trust anchor are disposable."""

    def __init__(
        self,
        temporary_root: Path,
        *,
        bind: str = "127.0.0.1",
        port: int = 0,
        rp_id: str = "localhost",
        origin: str | None = None,
        certificate: Path | None = None,
        private_key: Path | None = None,
    ):
        if bind not in ("127.0.0.1", "::1") and (
            certificate is None or private_key is None or origin is None
        ):
            raise ValueError("non-loopback serving requires operator TLS files and explicit origin")
        if (certificate is None) != (private_key is None):
            raise ValueError("certificate and private key must be supplied together")
        if origin is not None and not _origin_is_valid_for_rp(origin, rp_id):
            raise ValueError("origin must be an HTTPS origin valid for the configured RP ID")

        self.temp = tempfile.TemporaryDirectory(prefix="p13-rp-", dir=temporary_root)
        self.temp_path = Path(self.temp.name)
        self.httpd = http.server.ThreadingHTTPServer((bind, port), _RequestHandler)
        self.httpd.daemon_threads = True
        local_host = "localhost" if bind in ("127.0.0.1", "::1") else bind
        self.origin = origin or f"https://{local_host}:{self.httpd.server_port}"
        if not _origin_is_valid_for_rp(self.origin, rp_id):
            self.close()
            raise ValueError("origin must be an HTTPS origin valid for the configured RP ID")
        if certificate is None:
            certificate, private_key = _make_loopback_certificate(self.temp_path)
        assert private_key is not None
        self.certificate_path = Path(certificate)
        self.private_key_path = Path(private_key)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.minimum_version = ssl.TLSVersion.TLSv1_2
        context.load_cert_chain(self.certificate_path, self.private_key_path)
        self.httpd.socket = context.wrap_socket(self.httpd.socket, server_side=True)
        self.httpd.app = VerifierApplication(rp_id, self.origin)
        self.thread = threading.Thread(target=self.httpd.serve_forever, name="p13-rp-fixture", daemon=True)
        self.closed = False

    @property
    def port(self) -> int:
        return self.httpd.server_port

    @property
    def client_context(self) -> ssl.SSLContext:
        context = ssl.create_default_context(cafile=str(self.certificate_path))
        context.minimum_version = ssl.TLSVersion.TLSv1_2
        return context

    def start(self) -> "VerifierFixture":
        self.thread.start()
        return self

    def close(self) -> None:
        if getattr(self, "closed", False):
            return
        if hasattr(self, "httpd"):
            try:
                self.httpd.app.clear_account()
            except AttributeError:
                pass
            if getattr(self, "thread", None) and self.thread.is_alive():
                self.httpd.shutdown()
                self.thread.join(timeout=5)
            self.httpd.server_close()
        if hasattr(self, "temp"):
            self.temp.cleanup()
        self.closed = True

    def __enter__(self) -> "VerifierFixture":
        return self.start()

    def __exit__(self, *_exc: object) -> None:
        self.close()


def _serve(args: argparse.Namespace) -> None:
    root = args.temp_root.resolve()
    root.mkdir(parents=True, exist_ok=True)
    with VerifierFixture(
        root,
        bind=args.bind,
        port=args.port,
        rp_id=args.rp_id,
        origin=args.origin,
        certificate=args.tls_cert,
        private_key=args.tls_key,
    ) as fixture:
        print(json.dumps({"origin": fixture.origin, "rp_id": args.rp_id}), flush=True)
        try:
            fixture.thread.join()
        except KeyboardInterrupt:
            pass


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serve", action="store_true", help="serve the guided HTTPS WebAuthn route")
    parser.add_argument("--bind", default="127.0.0.1", help="defaults to loopback; remote bind needs explicit TLS/origin")
    parser.add_argument("--port", type=int, default=8443)
    parser.add_argument("--rp-id", default="localhost")
    parser.add_argument("--origin", help="exact public HTTPS origin (required for a non-loopback bind)")
    parser.add_argument("--tls-cert", type=Path, help="operator-controlled certificate for remote manual route")
    parser.add_argument("--tls-key", type=Path, help="matching operator-controlled private key")
    parser.add_argument("--temp-root", type=Path, required=True, help="owned disposable artifact directory")
    args = parser.parse_args()
    if not args.serve:
        parser.error("choose --serve")
    _serve(args)


if __name__ == "__main__":
    main()
