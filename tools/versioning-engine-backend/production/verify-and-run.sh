#!/usr/bin/env bash
# Exact-tuple development launcher for the minimal owner-ABI engine.
# Never a production release launcher or a strict capability certificate.
set -euo pipefail

backend_repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd -P)
backend_tools="$backend_repo/tools/versioning-engine-backend/production"
backend_prefix="$backend_repo/target/versioning-engine-backend/production/install"
backend_binary="$backend_repo/target/debug/ferric-browser"
backend_manifest="$backend_tools/owner-abi-linux-x86_64.sha256"
backend_tree_manifest="$backend_tools/owner-abi-linux-x86_64-tree.sha256"
backend_patch="$backend_tools/qt-6.11.2-owner-abi.patch"
backend_system_qt_manifest="$backend_tools/system-qt-linux-x86_64.sha256"
backend_patch_sha=7b692e9af2c8eb015c9ca36839b6ef061d95675cd409cea68482ad67d1c7ddb3

fail() { printf 'owner-ABI backend refused: %s\n' "$*" >&2; exit 1; }

verify_backend() {
    [[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || fail 'Linux x86_64 only'
    [[ $(id -u) != 0 ]] || fail 'do not run as root'
    [[ $(/usr/lib/qt6/bin/qtpaths --query QT_VERSION) == 6.11.2 ]] \
        || fail 'system Qt runtime must be 6.11.2'
    sha256sum --check --status "$backend_system_qt_manifest" \
        || fail 'system Qt runtime differs from the pinned build tuple'
    [[ -d $backend_prefix && -x $backend_binary ]] || fail 'engine install or Ferric binary absent'
    [[ $(realpath -- "$backend_prefix") == "$backend_prefix" ]] \
        || fail 'engine prefix must not redirect through a symlink'
    [[ -r $backend_manifest && -r $backend_tree_manifest ]] || fail 'artifact manifest absent'
    printf '%s  %s\n' "$backend_patch_sha" "$backend_patch" | sha256sum --check --status \
        || fail 'source patch differs from the pinned revision'
    local library
    for library in Core Quick Widgets QuickDelegatesQml; do
        [[ $(realpath -- "$backend_prefix/lib/libQt6WebEngine${library}.so.6") == \
            "$backend_prefix/lib/libQt6WebEngine${library}.so.6.11.2" ]] \
            || fail "WebEngine${library} SONAME link points outside this bundle"
    done
    (cd -- "$backend_prefix" && sha256sum --check --status "$backend_manifest") \
        || fail 'engine library or process checksum mismatch'

    local tree_root tree_actual tree_expected
    for tree_root in lib/qt6/qml/QtWebEngine share/qt6/resources \
        share/qt6/translations/qtwebengine_locales; do
        [[ -d $backend_prefix/$tree_root ]] || fail "missing $tree_root"
        [[ -z $(find "$backend_prefix/$tree_root" -type l -print -quit) ]] \
            || fail "unexpected symlink in $tree_root"
        [[ -z $(find "$backend_prefix/$tree_root" ! -type d ! -type f -print -quit) ]] \
            || fail "unexpected file type in $tree_root"
    done
    tree_expected=$(<"$backend_tree_manifest")
    [[ $tree_expected =~ ^[a-f0-9]{64}$ ]] || fail 'invalid resource-tree digest'
    tree_actual=$(
        cd -- "$backend_prefix"
        find lib/qt6/qml/QtWebEngine share/qt6/resources \
            share/qt6/translations/qtwebengine_locales -type f -print0 |
            LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | cut -d ' ' -f 1
    )
    [[ $tree_actual == "$tree_expected" ]] || fail 'QML, resource or locale tree changed'
    bash "$backend_repo/tools/versioning-engine-backend/check-no-probe-api.sh" \
        "$backend_prefix" >/dev/null || fail 'research-only API remains in the bundle'

    local resolved
    resolved=$(env LD_LIBRARY_PATH="$backend_prefix/lib" ldd "$backend_binary") \
        || fail 'could not inspect Ferric dependencies'
    [[ $resolved == *"libQt6WebEngineCore.so.6 => $backend_prefix/lib/libQt6WebEngineCore.so.6"* \
        && $resolved == *"libQt6WebEngineQuick.so.6 => $backend_prefix/lib/libQt6WebEngineQuick.so.6"* ]] \
        || fail 'Ferric would load a different WebEngine library'
    [[ $resolved != *'not found'* ]] || fail 'unresolved Ferric dependency'
}

launch_disposable() {
    verify_backend
    umask 077
    mkdir -p -- "$backend_repo/target/versioning-engine-backend/production"
    local backend_runtime
    backend_runtime=$(mktemp -d "$backend_repo/target/versioning-engine-backend/production/run.XXXXXX")
    mkdir -m 700 -- "$backend_runtime/runtime"
    printf 'Disposable owner-ABI runtime: %s\n' "$backend_runtime"
    env -u QTWEBENGINE_REMOTE_DEBUGGING -u QTWEBENGINE_DISABLE_SANDBOX \
        -u QTWEBENGINE_CHROMIUM_FLAGS -u LD_PRELOAD -u LD_AUDIT \
        -u QT_PLUGIN_PATH -u QML2_IMPORT_PATH \
        LD_LIBRARY_PATH="$backend_prefix/lib" \
        QML_IMPORT_PATH="$backend_prefix/lib/qt6/qml" \
        QTWEBENGINEPROCESS_PATH="$backend_prefix/lib/qt6/QtWebEngineProcess" \
        QTWEBENGINE_RESOURCES_PATH="$backend_prefix/share/qt6/resources" \
        QTWEBENGINE_LOCALES_PATH="$backend_prefix/share/qt6/translations/qtwebengine_locales" \
        XDG_CONFIG_HOME="$backend_runtime/config" XDG_DATA_HOME="$backend_runtime/data" \
        XDG_CACHE_HOME="$backend_runtime/cache" XDG_STATE_HOME="$backend_runtime/state" \
        XDG_RUNTIME_DIR="$backend_runtime/runtime" QT_QPA_PLATFORM=offscreen \
        QT_QPA_PLATFORMTHEME= QT_STYLE_OVERRIDE=Fusion \
        "$@"
}

test_integrity_refusals() {
    verify_backend
    local original_prefix=$backend_prefix
    local negative_root
    negative_root=$(mktemp -d "$backend_repo/target/versioning-engine-backend/production/negative.XXXXXX")
    cp -a --reflink=auto -- "$original_prefix" "$negative_root/install"
    backend_prefix="$negative_root/install"
    verify_backend || fail 'copied baseline bundle did not verify'

    cp -- /usr/lib/libQt6WebEngineQuick.so.6.11.2 \
        "$backend_prefix/lib/libQt6WebEngineQuick.so.6.11.2"
    if (verify_backend >/dev/null 2>&1); then
        fail "wrong-engine fixture was accepted; retained $negative_root"
    fi
    cp --reflink=auto -- "$original_prefix/lib/libQt6WebEngineQuick.so.6.11.2" \
        "$backend_prefix/lib/libQt6WebEngineQuick.so.6.11.2"

    truncate -s +1 -- "$backend_prefix/lib/qt6/QtWebEngineProcess"
    if (verify_backend >/dev/null 2>&1); then
        fail "mixed-process fixture was accepted; retained $negative_root"
    fi
    cp --reflink=auto -- "$original_prefix/lib/qt6/QtWebEngineProcess" \
        "$backend_prefix/lib/qt6/QtWebEngineProcess"

    truncate -s +1 -- "$backend_prefix/lib/qt6/qml/QtWebEngine/qmldir"
    if (verify_backend >/dev/null 2>&1); then
        fail "mixed-QML fixture was accepted; retained $negative_root"
    fi
    cp --reflink=auto -- "$original_prefix/lib/qt6/qml/QtWebEngine/qmldir" \
        "$backend_prefix/lib/qt6/qml/QtWebEngine/qmldir"

    truncate -s +1 -- "$backend_prefix/share/qt6/resources/icudtl.dat"
    if (verify_backend >/dev/null 2>&1); then
        fail "mixed-resource fixture was accepted; retained $negative_root"
    fi
    backend_prefix=$original_prefix
    rm -rf -- "$negative_root"
    printf 'Wrong engine, mixed process, QML and resource were refused.\n'
}

test_renderer_sandbox() {
    verify_backend
    local sandbox_log sandbox_launcher renderer_pid renderer_exe
    sandbox_log=$(mktemp "$backend_repo/target/versioning-engine-backend/production/sandbox.XXXXXX")
    launch_disposable timeout --signal=INT --kill-after=5s 35 \
        /usr/lib/qt6/bin/qml "$backend_tools/sandbox_pid.qml" >"$sandbox_log" 2>&1 &
    sandbox_launcher=$!
    for ((attempt=0; attempt<200; ++attempt)); do
        renderer_pid=$(sed -n 's/.*FERRIC_OWNER_RENDERER_PID=\([0-9][0-9]*\).*/\1/p' \
            "$sandbox_log" | tail -n 1)
        [[ -n $renderer_pid ]] && break
        if ! kill -0 "$sandbox_launcher" 2>/dev/null; then
            break
        fi
        sleep 0.1
    done
    if [[ -z $renderer_pid || ! -r /proc/$renderer_pid/status ]]; then
        fail "renderer PID was not observed; retained $sandbox_log"
    fi
    renderer_exe=$(readlink -- "/proc/$renderer_pid/exe")
    [[ $renderer_exe == "$backend_prefix/lib/qt6/QtWebEngineProcess" ]] \
        || fail "renderer did not use pinned process; retained $sandbox_log"
    for ((attempt=0; attempt<50; ++attempt)); do
        if rg -q '^NoNewPrivs:[[:space:]]+1$' "/proc/$renderer_pid/status" \
                && rg -q '^Seccomp:[[:space:]]+2$' "/proc/$renderer_pid/status"; then
            break
        fi
        sleep 0.1
    done
    rg -q '^NoNewPrivs:[[:space:]]+1$' "/proc/$renderer_pid/status" \
        || fail "renderer NoNewPrivs was not 1; retained $sandbox_log"
    rg -q '^Seccomp:[[:space:]]+2$' "/proc/$renderer_pid/status" \
        || fail "renderer seccomp filter was not active; retained $sandbox_log"
    rg -q '^Seccomp_filters:[[:space:]]+[1-9][0-9]*$' "/proc/$renderer_pid/status" \
        || fail "renderer filter count was zero; retained $sandbox_log"
    rg -Fq "$backend_prefix/lib/libQt6WebEngineCore.so.6.11.2" \
        "/proc/$renderer_pid/maps" \
        || fail "renderer did not map pinned WebEngine Core; retained $sandbox_log"
    wait "$sandbox_launcher" || fail "sandbox fixture failed; retained $sandbox_log"
    rm -- "$sandbox_log"
    printf 'Pinned renderer process observed with NoNewPrivs=1 and Seccomp=2.\n'
}

case ${1:-} in
    check)
        [[ $# == 1 ]] || fail 'usage: verify-and-run.sh check | run | test-owner | test-view-host'
        verify_backend
        printf 'Pinned owner-ABI development bundle verified; strict capabilities remain disabled.\n'
        ;;
    run)
        [[ $# == 1 ]] || fail 'usage: verify-and-run.sh check | run | test-owner | test-view-host'
        launch_disposable "$backend_binary" --safe-mode
        ;;
    test-owner)
        [[ $# == 1 ]] || fail 'usage: verify-and-run.sh check | run | test-owner | test-view-host'
        launch_disposable timeout --signal=INT --kill-after=5s 90 \
            /usr/lib/qt6/bin/qmltestrunner -input "$backend_tools/tst_owner_bridge.qml"
        ;;
    test-view-host)
        [[ $# == 1 ]] || fail 'usage: verify-and-run.sh check | run | test-owner | test-view-host'
        launch_disposable timeout --signal=INT --kill-after=5s 90 \
            /usr/lib/qt6/bin/qmltestrunner \
            -input "$backend_repo/crates/ferric-browser-engine-qt/tests/qml/tst_versioning_view_host.qml"
        ;;
    test-integrity)
        [[ $# == 1 ]] || fail 'usage: verify-and-run.sh test-integrity'
        test_integrity_refusals
        ;;
    test-sandbox)
        [[ $# == 1 ]] || fail 'usage: verify-and-run.sh test-sandbox'
        test_renderer_sandbox
        ;;
    *) fail 'usage: verify-and-run.sh check | run | test-owner | test-view-host | test-integrity | test-sandbox' ;;
esac
