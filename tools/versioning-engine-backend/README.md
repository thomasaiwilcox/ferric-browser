# Pinned WebEngine backend integration boundary

The `production/` subdirectory is the **separate clean-source owner-ABI
candidate** for VB-099. It pins Qt WebEngine 6.11.2 by archive digest, applies
only the reviewable three-file Qt Quick patch in
`production/qt-6.11.2-owner-abi.patch`, and keeps its source, build and install
under `target/versioning-engine-backend/production/`. The v1 ABI exposes a
per-view ID and an adapter-generation ID. The latter rotates on profile
replacement, adapter adoption and renderer termination; it does not certify
RTC, cookies or downloads. Ferric compares that witness with its exact
QObject, profile, retained-view ID and navigation epoch before staging a live
view. The Rust stage ticket separately enforces its operation generation.

On the declared Linux x86_64 development tuple, run
`bash tools/versioning-engine-backend/production/build.sh all` followed by
local artifact-manifest generation and
`bash tools/versioning-engine-backend/production/verify-and-run.sh check`.
`test-integrity`, `test-owner`, `test-view-host`, `test-sandbox` and `run` then
use the exact bundle. The last command is an
offscreen, disposable `--safe-mode` launch; it cannot select a normal profile.
The scripts refuse mismatched libraries, QML/resources/locales, process binary
or research-only probe exports and do not silently fall back to public Qt.
Before build and installation, `production/verify-source.py` compares every archived
source member with the extracted tree, reconstructs the three patched files
from the archive plus the checked-in zero-fuzz patch, and refuses unexpected
extra source paths apart from generated Python bytecode. This is source
provenance for the local build, not a reproducible-binary or signed-release
claim.
Normal Ferric startup still resolves the public Qt library. This candidate is
not a release certificate, and no strict capability is enabled by selecting it.
The development tuple also pins the currently installed Qt base/QML runtime
hashes; it is not a self-contained redistribution and must be rebuilt after a
system Qt update.

## Transfer and resume from another machine

Fetch branch `feature/versioned-browsing` and read
`CurrentImplementation/ferric-versioned-browsing/AGENT_HANDOFF.md` and
`docs/testing/versioned-browsing/VB-099.md`. On 2026-10-08 the clean build was
interrupted at Ninja edge 2017/4105; it was not linked, installed or natively
qualified. The `target/` tree, downloaded source archive and research install
are intentionally not committed. `build.sh all` downloads the exact archive
when necessary, verifies its hash and the source patch, configures, builds and
installs into the ignored local tree. It requires Linux x86_64, the exact
installed Qt 6.11.2 development/runtime tuple recorded by
`production/system-qt-linux-x86_64.sha256`, the tools/versions in
`production/source-lock.json`, and sufficient CPU, memory and disk for a full
Qt WebEngine build. It fails closed if the generated configuration differs.
A different host is not automatically the same tuple: reproduce it or review,
update and requalify the locks. Do not copy a research install or disable a
failed fingerprint check.
Build the Ferric binary before running the verifier:
`cargo build -p ferric-browser --features ferric-browser-engine-qt/unqualified-clean-branch-test --locked`.
The opt-in test feature preserves the existing native journey fixture; it does
not enable a production capability.

The clean install has no pre-generated artifact manifests. After inspecting
the installed paths and confirming the build/install and
`check-no-probe-api.sh` succeeded, generate host-local manifests from the
installed bundle, then review them before using the verifier:

```sh
cd target/versioning-engine-backend/production/install
sha256sum lib/libQt6WebEngineCore.so.6.11.2 lib/libQt6WebEngineQuick.so.6.11.2 lib/libQt6WebEngineWidgets.so.6.11.2 lib/libQt6WebEngineQuickDelegatesQml.so.6.11.2 lib/qt6/QtWebEngineProcess > ../../../../tools/versioning-engine-backend/production/owner-abi-linux-x86_64.sha256
find lib/qt6/qml/QtWebEngine share/qt6/resources share/qt6/translations/qtwebengine_locales -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | cut -d ' ' -f 1 > ../../../../tools/versioning-engine-backend/production/owner-abi-linux-x86_64-tree.sha256
cd ../../../..
bash tools/versioning-engine-backend/production/verify-and-run.sh check
bash tools/versioning-engine-backend/production/verify-and-run.sh test-integrity
bash tools/versioning-engine-backend/production/verify-and-run.sh test-owner
bash tools/versioning-engine-backend/production/verify-and-run.sh test-view-host
bash tools/versioning-engine-backend/production/verify-and-run.sh test-sandbox
bash tools/versioning-engine-backend/production/verify-and-run.sh run
```

These hashes are observations of a local build, not signed provenance or
independent proof that the bundle is safe. Commit them only alongside the
corresponding qualified tuple evidence. If any check fails, retain the
failure and keep VB-099 in progress. The full workspace/release campaign and
the separate WebRTC, cookie and ordinary-viewless-download slices still follow.

The project owner approved pursuing a maintained, version-pinned Qt/Chromium
backend for the full Versioned Browsing vision on 2026-10-08. This directory is
the first **disposable Linux x86_64 integration boundary**, not that backend's
release certificate. The default public-Qt build and every production capability
flag remain unchanged and fail closed.

`bash tools/versioning-engine-backend/verify-and-run.sh check` verifies the
current local research install against exact library/process hashes and a
deterministic digest of its QML, resource and locale tree. It also checks the
Ferric binary's direct WebEngine library resolution. `run` repeats the checks,
then launches only `--safe-mode` with a fresh offscreen XDG environment. It
removes inherited remote-debugging, sandbox-disabling, Chromium-flag and loader
injection variables. It never points to a normal profile. A mismatch refuses
launch; it does not fall back to system WebEngine. The disposable runtime path is
printed and retained for local inspection.
`test-view-host` uses the same verified, disposable environment to run the
existing exact-view QML fixture. On 2026-10-08 it passed 18/18 tests on the
pinned candidate. Chromium emitted three `blink.mojom.Widget` rejected-message
diagnostics during the passing run. The same disposable fixture also passed
18/18 against system Qt and emitted five such diagnostics, so the message class
is not unique to this patch. The diagnostics have not been adjudicated as a
release-safe condition.
One disposable smoke run showed the pinned WebEngine child processes loading
the pinned Core library and resources. The actual renderer process reported
`NoNewPrivs: 1`, `Seccomp: 2` and one seccomp filter in `/proc/<pid>/status`;
an intermediate zygote reported `Seccomp: 0`. This is a bounded host observation,
not a complete renderer/network-service sandbox audit or evidence about other
release platforms.

The manifest pins the **observed 2026-10-08 local research artifacts**, including
experimental diagnostic overlays. It is deliberately separate from
[`source-lock.json`](../versioning-engine-spike/source-lock.json): that record
contains many earlier, separately exercised patch/binary tuples and none of
their passing probe claims transfer merely because the version string matches.
This runner is not a release packaging recipe, integrity signature, sandbox
audit, reproducible-build claim or permission to expose its test-only
`ferric*ForProbe` methods in production.
`check-no-probe-api.sh ENGINE_INSTALL_PREFIX` is a separate negative release
hygiene check: it refuses exported `ferric*ForProbe` or `FERRIC_SPIKE_` markers
in the WebEngine libraries and process binary. It intentionally **fails** on
this research install. Passing it on another build would not prove that build
has the required production API or any capability certificate.

The next backend work is ordered by proof boundary:

1. Extract the minimal production extension from the research overlays, remove
   test/diagnostic methods and connect its exact Qt Quick view/profile handles
   to Ferric's owner/generation model. Public Qt stays the default backend.
2. Qualify WebRTC absence with a retained, invalidatable exact-view lease over
   all live frames, PeerConnections, media and capture paths, app-script entry
   points, frame/process changes and owner loss. Unknown always refuses stash.
3. Qualify complete canonical cookie snapshots and seed/read-back into an
   unpublished empty child under explicit source/child ownership. Keep
   live-profile merge behind a separate all-writer conditional-apply gate.
4. Provide a qualified exact-profile, null-view ordinary-download origin and
   prove installed bytes and durable ledger completion; exact Save Page is a
   different already-supported path.
5. Rebuild and test each supported OS/architecture tuple, the full native
   journeys and fault campaign, sandbox, licences/notices, security updates,
   rollback and artifact signatures before promoting any capability.

Linux x86_64 is an initial development tuple only. The release platform matrix,
named backend maintainer, security-update service level, CI capacity and
independent security review are still explicit release gates. A newer Qt or
Chromium release invalidates this manifest and all engine-bound certificates
until rebuilt and requalified. The [backend ADR](../../docs/architecture/ADR-0016-versioning-engine-capability-backend.md)
owns the product decision; this tool only makes the development handoff
fail-closed.
