"""End-to-end HTTPS tests using a real disposable authenticator key and python-fido2."""

from __future__ import annotations

import base64
import hashlib
import json
import os
import ssl
import socket
import tempfile
import threading
import time
import unittest
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import ec
from fido2.cose import ES256
from fido2.utils import websafe_encode
from fido2.webauthn import (
    AttestationObject,
    AttestedCredentialData,
    AuthenticatorData,
    CollectedClientData,
)

from server import VerifierFixture


def _decode(text: str) -> bytes:
    return base64.urlsafe_b64decode(text + "=" * (-len(text) % 4))


def _encode(raw: bytes) -> str:
    return websafe_encode(raw)


def _json_bytes(value: dict) -> bytes:
    return json.dumps(value, separators=(",", ":")).encode("utf-8")


class TestAuthenticator:
    """A disposable software authenticator using independent crypto primitives."""

    def __init__(self):
        self.private_key = ec.generate_private_key(ec.SECP256R1())
        self.credential_id = os.urandom(32)
        cose_public = ES256.from_cryptography_key(self.private_key.public_key())
        self.credential_data = AttestedCredentialData.create(bytes(16), self.credential_id, cose_public)
        self.registration_counter = 1

    @staticmethod
    def _credential_response(credential_id: bytes, response: dict) -> dict:
        encoded_id = _encode(credential_id)
        return {
            "id": encoded_id,
            "rawId": encoded_id,
            "type": "public-key",
            "response": response,
            "clientExtensionResults": {},
            "authenticatorAttachment": "platform",
        }

    def create_response(self, options: dict, origin: str, *, uv: bool = True, client_type: str = "webauthn.create") -> dict:
        challenge = _decode(options["challenge"])
        client_data = CollectedClientData.create(client_type, challenge, origin)
        flags = AuthenticatorData.FLAG.UP | AuthenticatorData.FLAG.AT
        if uv:
            flags |= AuthenticatorData.FLAG.UV
        auth_data = AuthenticatorData.create(
            hashlib.sha256(options["rp"]["id"].encode("utf-8")).digest(),
            flags,
            self.registration_counter,
            bytes(self.credential_data),
        )
        attestation = AttestationObject.create("none", auth_data, {})
        return self._credential_response(
            self.credential_id,
            {
                "clientDataJSON": _encode(bytes(client_data)),
                "attestationObject": _encode(bytes(attestation)),
                "transports": ["internal"],
            },
        )

    def get_response(
        self,
        options: dict,
        origin: str,
        *,
        counter: int = 2,
        uv: bool = True,
        rp_id: str | None = None,
        client_type: str = "webauthn.get",
        corrupt_signature: bool = False,
    ) -> dict:
        client_data = CollectedClientData.create(client_type, _decode(options["challenge"]), origin)
        flags = AuthenticatorData.FLAG.UP
        if uv:
            flags |= AuthenticatorData.FLAG.UV
        bound_rp = rp_id or options["rpId"]
        auth_data = AuthenticatorData.create(
            hashlib.sha256(bound_rp.encode("utf-8")).digest(),
            flags,
            counter,
        )
        signature = self.private_key.sign(bytes(auth_data) + client_data.hash, ec.ECDSA(hashes.SHA256()))
        if corrupt_signature:
            signature = signature[:-1] + bytes([signature[-1] ^ 1])
        return self._credential_response(
            self.credential_id,
            {
                "clientDataJSON": _encode(bytes(client_data)),
                "authenticatorData": _encode(bytes(auth_data)),
                "signature": _encode(signature),
                "userHandle": None,
            },
        )


class RelyingPartyVerificationTests(unittest.TestCase):
    def setUp(self):
        temporary_root = Path(os.environ["FERRIC_P13_TEST_TMP"])
        temporary_root.mkdir(parents=True, exist_ok=True)
        self.root = temporary_root
        self.default_trust_paths = ssl_default_paths()
        self.fixture = VerifierFixture(temporary_root).start()
        self.assertEqual(self.fixture.httpd.server_address[0], "127.0.0.1")
        self.authenticator = TestAuthenticator()

    def tearDown(self):
        if self.fixture is not None:
            response = self._post("/rp/cleanup", {})
            self.assertEqual(response["ok"], True)
            self.assertEqual(self.fixture.httpd.app.account_id, None)
            self.fixture.close()
            self.assertFalse(self.fixture.temp_path.exists())
        self.assertEqual(list(self.root.iterdir()), [])
        self.assertEqual(ssl_default_paths(), self.default_trust_paths)

    def _post_raw(self, path: str, payload: dict) -> tuple[int, dict]:
        request = Request(
            f"https://127.0.0.1:{self.fixture.port}{path}",
            data=_json_bytes(payload),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urlopen(request, context=self.fixture.client_context, timeout=5) as response:
                return response.status, json.loads(response.read())
        except HTTPError as error:
            return error.code, json.loads(error.read())

    def _post(self, path: str, payload: dict) -> dict:
        status, response = self._post_raw(path, payload)
        self.assertEqual(status, 200, response)
        return response

    def _begin_registration(self) -> tuple[str, dict]:
        response = self._post("/rp/register/begin", {})
        return response["transaction"], response["publicKey"]

    def _register_disposable_credential(self) -> None:
        transaction, options = self._begin_registration()
        credential = self.authenticator.create_response(options, self.fixture.origin)
        self._post("/rp/register/complete", {"transaction": transaction, "credential": credential})

    def _begin_assertion(self) -> tuple[str, dict]:
        response = self._post("/rp/assertion/begin", {})
        return response["transaction"], response["publicKey"]

    def _assertion_result(self, *, origin: str | None = None, **mutations) -> tuple[int, dict]:
        transaction, options = self._begin_assertion()
        credential = self.authenticator.get_response(
            options,
            origin or self.fixture.origin,
            **mutations,
        )
        return self._post_raw(
            "/rp/assertion/complete",
            {"transaction": transaction, "credential": credential},
        )

    def test_guided_route_is_served_only_by_the_fixture_https_origin(self):
        request = Request(f"https://127.0.0.1:{self.fixture.port}/")
        with urlopen(request, context=self.fixture.client_context, timeout=5) as response:
            self.assertEqual(response.status, 200)
            self.assertIn("text/html", response.headers["Content-Type"])
            self.assertIn("no-store", response.headers["Cache-Control"])
            page = response.read()
        self.assertIn(b"navigator.credentials.create", page)
        self.assertIn(b"navigator.credentials.get", page)

    def test_t085_real_create_then_get_passes_independent_verifier_over_https(self):
        self._register_disposable_credential()
        status, response = self._assertion_result()
        self.assertEqual((status, response), (200, {"ok": True}))
        self.assertEqual(self.fixture.httpd.app.signature_counter, 2)
        self.assertEqual(self.fixture.httpd.app.verified_registrations, 1)
        self.assertEqual(self.fixture.httpd.app.verified_assertions, 1)

    def test_counterless_credentials_pass_with_fresh_challenges(self):
        self.authenticator.registration_counter = 0
        self._register_disposable_credential()
        self.assertEqual(self._assertion_result(counter=0), (200, {"ok": True}))
        self.assertEqual(self._assertion_result(counter=0), (200, {"ok": True}))
        self.assertEqual(self.fixture.httpd.app.signature_counter, 0)

    def test_counter_cannot_return_to_zero_after_incrementing(self):
        self._register_disposable_credential()
        self.assertEqual(self._assertion_result(counter=0)[0], 400)

    def test_t086_wrong_challenge_fails_fido2_assertion_verifier(self):
        self._register_disposable_credential()
        transaction, options = self._begin_assertion()
        bad_options = {**options, "challenge": _encode(os.urandom(32))}
        credential = self.authenticator.get_response(bad_options, self.fixture.origin)
        self.assertEqual(
            self._post_raw("/rp/assertion/complete", {"transaction": transaction, "credential": credential})[0],
            400,
        )

    def test_t086_wrong_origin_fails_fido2_assertion_verifier(self):
        self._register_disposable_credential()
        self.assertEqual(self._assertion_result(origin="https://attacker.invalid")[0], 400)

    def test_t086_wrong_rp_hash_fails_fido2_assertion_verifier(self):
        self._register_disposable_credential()
        self.assertEqual(self._assertion_result(rp_id="attacker.invalid")[0], 400)

    def test_t086_corrupt_signature_fails_fido2_assertion_verifier(self):
        self._register_disposable_credential()
        self.assertEqual(self._assertion_result(corrupt_signature=True)[0], 400)
        self.assertEqual(self.fixture.httpd.app.verified_assertions, 0)

    def test_t086_missing_required_uv_fails_fido2_assertion_verifier(self):
        self._register_disposable_credential()
        self.assertEqual(self._assertion_result(uv=False)[0], 400)

    def test_t086_wrong_client_data_type_fails_fido2_assertion_verifier(self):
        self._register_disposable_credential()
        self.assertEqual(self._assertion_result(client_type="webauthn.create")[0], 400)

    def test_t086_replayed_counter_fails_relying_party_counter_policy(self):
        self._register_disposable_credential()
        self.assertEqual(self._assertion_result(counter=self.authenticator.registration_counter)[0], 400)

    def test_t087_cleanup_removes_in_memory_account_and_scoped_trust_files(self):
        self._register_disposable_credential()
        temporary_directory = self.fixture.temp_path
        trust_file = self.fixture.certificate_path
        private_key = self.fixture.private_key_path
        self.assertTrue(trust_file.is_relative_to(temporary_directory))
        self.assertTrue(private_key.is_relative_to(temporary_directory))
        self.assertEqual(private_key.stat().st_mode & 0o777, 0o600)
        self.assertTrue(self.fixture.client_context.get_ca_certs())
        self.assertEqual(len(self.fixture.httpd.app.registration_states), 0)
        self.assertEqual(len(self.fixture.httpd.app.assertion_states), 0)
        self.assertIsNotNone(self.fixture.httpd.app.credential)

        self._post("/rp/cleanup", {})
        self.assertIsNone(self.fixture.httpd.app.credential)
        self.assertIsNone(self.fixture.httpd.app.account_id)
        self.assertEqual(self.fixture.httpd.app.signature_counter, 0)


class RelyingPartyTeardownTests(unittest.TestCase):
    def test_stalled_pre_tls_connection_cannot_hold_shutdown(self):
        with tempfile.TemporaryDirectory(prefix="stalled-rp-", dir=os.environ["FERRIC_P13_TEST_TMP"]) as temporary:
            fixture = VerifierFixture(Path(temporary))
            entered = threading.Event()
            original = fixture.httpd.get_request

            def observed_accept():
                entered.set()
                return original()

            fixture.httpd.get_request = observed_accept
            fixture.start()
            peer = socket.create_connection(("127.0.0.1", fixture.port), timeout=2)
            closer = threading.Thread(target=fixture.close, daemon=True)
            try:
                self.assertTrue(entered.wait(2))
                started = time.monotonic()
                closer.start()
                closer.join(timeout=5)
                self.assertFalse(closer.is_alive(), "stalled TLS handshake blocked RP shutdown")
                self.assertLess(time.monotonic() - started, 5)
                self.assertTrue(fixture.closed)
            finally:
                peer.close()
                if closer.ident is not None:
                    closer.join(timeout=5)
                if not closer.is_alive():
                    fixture.close()


def ssl_default_paths() -> tuple[str | None, str | None]:
    paths = ssl.get_default_verify_paths()
    return (paths.cafile, paths.capath)


if __name__ == "__main__":
    unittest.main(verbosity=2)
