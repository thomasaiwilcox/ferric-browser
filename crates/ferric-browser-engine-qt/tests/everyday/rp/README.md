# Independent WebAuthn relying-party fixture

This is a disposable test RP for the P13 verifier cases. It is not the Ferric
phone broker, a helper integration, or evidence of a supported phone provider.
The verifier is Yubico's established [`python-fido2`](https://pypi.org/project/fido2/2.2.1/)
2.2.1 `Fido2Server`, independent of Ferric's `libwebauthn`. The upstream server
API source is [Yubico/python-fido2 `fido2/server.py`](https://github.com/Yubico/python-fido2/blob/main/fido2/server.py).
`requirements.in` pins the direct dependencies; `requirements.lock` records all
resolved versions and SHA-256 hashes, fixes the index to official PyPI, and
requires wheels only. The tested interpreter is CPython 3.13.9.

## What the verifier checks

The fixture selects `attestation="none"`. `register_complete` parses the
registration response and checks WebAuthn create type, exact expected origin,
server-issued challenge, configured RP ID hash, user presence, and required
user verification. With none attestation it does not authenticate an
attestation certificate chain or manufacturer identity. It retains the
credential public key from authenticator data.

`authenticate_complete` parses assertion bytes and checks get type, exact
origin, server-issued challenge, RP ID hash, user presence, required UV, known
credential ID, and the assertion signature using the registered public key.
The fixture follows WebAuthn's signature-counter condition: if either the new
or stored counter is nonzero, the new value must increase. Counterless
authenticators may keep both values at zero; their assertions still require
fresh server challenges and valid signatures. This supports physical passkeys
without disguising a non-increasing nonzero counter. See
[WebAuthn §7.2](https://www.w3.org/TR/webauthn-3/#sctn-verifying-assertion).
Errors returned to the browser are generic and request bodies are never logged.

The test authenticator makes fresh P-256 keys, credential IDs, authenticator
data, a `none` attestation object, client data, and ECDSA signatures for each
test. `python-fido2` data classes encode the WebAuthn bytes and its server
implementation parses and verifies them. Tests submit those response mappings
over the same HTTPS endpoints as the guided page. Negative mutations therefore
reach the actual independent verifier (or the explicitly documented RP counter
policy); no fake signature success or libwebauthn validation is used.

## Run in an isolated environment

Commands below use the assigned external artifact directory. Recreate the
managed interpreter and venv there; do not install packages into system Python.

```sh
ARTIFACT=/absolute/path/to/builds/rp-P13-a1
export UV_CACHE_DIR="$ARTIFACT/uv-cache"
export UV_PYTHON_INSTALL_DIR="$ARTIFACT/python"
export UV_PYTHON_BIN_DIR="$ARTIFACT/bin"
uv python install 3.13.9
uv venv --managed-python --python 3.13.9 "$ARTIFACT/venv"
cd /path/to/ferric-browser/crates/ferric-browser-engine-qt/tests/everyday/rp
uv pip compile requirements.in --output-file requirements.lock --generate-hashes \
  --only-binary :all: --emit-index-url --emit-build-options \
  --index-url https://pypi.org/simple --python-version 3.13.9
uv pip sync --python "$ARTIFACT/venv/bin/python" --require-hashes \
  --only-binary :all: --index-url https://pypi.org/simple requirements.lock
mkdir -p "$ARTIFACT/fixture-tests"
FERRIC_P13_TEST_TMP="$ARTIFACT/fixture-tests" \
  "$ARTIFACT/venv/bin/python" -B -m unittest discover -v \
  -s . -p 'test_*.py'
```

The unit server binds only to `127.0.0.1` on an OS-assigned port. Each test
creates a unique temporary TLS key/certificate below `FERRIC_P13_TEST_TMP`; its
HTTPS client context trusts that certificate only. The key, cert, in-memory
registration, credential, challenges, and counters are removed by fixture
cleanup. No global CA store, DNS, firewall, browser, desktop, or profile is
modified. The test asserts account state is empty, temporary trust files are
gone, and Python's default CA paths are unchanged.

## Guided browser route and physical test preparation

`../fixtures/phone-auth.html` is a small guided `navigator.credentials.create`
and `navigator.credentials.get` page served from `/`. It creates one disposable
local account and then authenticates it. It does not contact a phone or pair
with Ferric. Opening it on localhost with the temporary self-signed certificate
requires an explicitly scoped local browser trust action; the unit tests avoid
browser trust changes by using a private `SSLContext`.

The local hybrid test can use the reserved hostname `passkeys.ferric.test`,
an exact matching RP ID, and a loopback-only listener. The independent review
confirmed that the packaged PSL/library admits this exact RP and rejects a
parent `ferric.test` or unrelated RP. Supply a certificate with the exact
fixture hostname in its SAN and trust its test CA only in the disposable
browser profile. Qt 6.11's
[`WebEngineProfilePrototype.additionalTrustedCertificateFiles`](https://doc.qt.io/qt-6/qml-qtwebengine-webengineprofileprototype.html#additionalTrustedCertificateFiles-prop)
supports this without changing the system trust store. A dedicated fixture
process may map only this hostname to loopback; do not change global DNS or
disable certificate validation.

The phone is the CTAP hybrid authenticator, so it need not browse or trust the
RP's HTTPS certificate. The hybrid relay still uses normal public network/TLS
and Bluetooth proximity. This tests a real authenticator and independently
verified local create/get results; it does not qualify public deployment or
replace native-fallback checks against normal browser trust decisions.

Before scanning, explain that the disposable RP/account creates a real passkey
on the phone's selected provider and that the provider may sync it. Deleting
the RP fixture does not delete that passkey; remove it from the phone/provider
after the run. Never use a personal website account for this check.

The native `public-page-host` build also provides `phone_rp_browser`, which
loads the actual production Main Root. Its wrapper creates the exact-SAN
fixture CA, inserts trust before profile creation, and copies the real helper
and PSL into an owned temporary prefix. Build the helper's `ferric-phone-auth`
binary separately and supply both artifact paths:

```sh
"$ARTIFACT/venv/bin/python" ../../public-page-host/run_phone_rp.py \
  --browser /owned/build/native/phone_rp_browser \
  --helper /owned/build/helper-target/debug/ferric-phone-auth \
  --scratch-root /owned/existing/scratch
```

The default check displays the disposable browser, confirms the Phone choice
is enabled, and cancels it without selecting the helper. It uses an explicit
direct route for that process and appends only the exact RP loopback mapping;
proxy conflicts remain errors. It prints verification/cleanup counts without
credential bodies. TLS handshakes and process cleanup have deadlines. Failed
cleanup keeps ownership evidence and fails the run.

Add `--interactive` only for the attended device run after explaining synced
passkey cleanup. Create and verify using Phone in both native choices, delete
the RP account, then remove the passkey from the phone/provider before closing
the disposable browser. Independent RP counters alone do not identify the
physical authenticator; retain the operator's device/provider observations.

For a separately requested remote test, use a domain you control and an
operator-provided certificate/key. Example shape (replace every placeholder
with owned values; do not expose an untrusted test service):

```sh
ARTIFACT=/absolute/path/to/builds/rp-P13-a1
"$ARTIFACT/venv/bin/python" server.py --serve --bind 0.0.0.0 --port 8443 \
  --rp-id auth.example.net --origin https://auth.example.net:8443 \
  --tls-cert /owned/path/fullchain.pem --tls-key /owned/path/private-key.pem \
  --temp-root "$ARTIFACT/fixture-manual"
```

Non-loopback binds require an explicit HTTPS origin and operator-provided
certificate and key; the fixture never installs their trust. Stop the process
with Ctrl-C. No physical credential is qualified by the automated verifier
tests or by these preparation instructions.
