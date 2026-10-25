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
The fixture adds an RP counter policy: each new authenticator counter must be
strictly greater than the registered/last accepted count. That is a deliberate
fixture policy for the counter replay negative; it is stricter than authenticators
whose counters are absent or permanently zero. Errors returned to the browser
are generic and request bodies are never logged.

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
  -s . -p 'test_verification.py'
```

The unit server binds only to `127.0.0.1` on an OS-assigned port. Each test
creates a unique temporary TLS key/certificate below `FERRIC_P13_TEST_TMP`; its
HTTPS client context trusts that certificate only. The key, cert, in-memory
registration, credential, challenges, and counters are removed by fixture
cleanup. No global CA store, DNS, firewall, browser, desktop, or profile is
modified. The test asserts account state is empty, temporary trust files are
gone, and Python's default CA paths are unchanged.

## Guided browser route and future physical test

`../fixtures/phone-auth.html` is a small guided `navigator.credentials.create`
and `navigator.credentials.get` page served from `/`. It creates one disposable
local account and then authenticates it. It does not contact a phone or pair
with Ferric. Opening it on localhost with the temporary self-signed certificate
requires an explicitly scoped local browser trust action; the unit tests avoid
browser trust changes by using a private `SSLContext`.

For a later operator-approved physical test, use only a domain you control, a
certificate/key that the test phone already trusts, an RP ID valid for that
domain, and an explicit P17 consent/fixture plan. Example shape (replace every
placeholder with owned values; do not expose an untrusted test service):

```sh
ARTIFACT=/absolute/path/to/builds/rp-P13-a1
"$ARTIFACT/venv/bin/python" server.py --serve --bind 0.0.0.0 --port 8443 \
  --rp-id auth.example.net --origin https://auth.example.net:8443 \
  --tls-cert /owned/path/fullchain.pem --tls-key /owned/path/private-key.pem \
  --temp-root "$ARTIFACT/fixture-manual"
```

Non-loopback binds require an explicit HTTPS origin and operator-provided
certificate and key; the fixture never installs their trust. Stop the process
with Ctrl-C. This is a future manual route only: no remote listener, phone,
personal account, native provider, or physical credential was used or qualified
for P13.
