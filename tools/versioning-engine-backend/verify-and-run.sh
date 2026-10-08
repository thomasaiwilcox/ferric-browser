#!/usr/bin/env bash
# Disposable integration handoff for one exact research engine build.
# Never use this as a release launcher or against a normal browser profile.
set -euo pipefail

backend_repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
backend_tools="$backend_repo/tools/versioning-engine-backend"
backend_prefix="$backend_repo/target/versioning-engine-spike/install"
backend_binary="$backend_repo/target/debug/ferric-browser"
backend_manifest="$backend_tools/candidate-linux-x86_64.sha256"
backend_tree_manifest="$backend_tools/candidate-linux-x86_64-tree.sha256"

fail() { printf 'pinned engine refused: %s\n' "$*" >&2; exit 1; }

verify_candidate() {
    [[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || fail 'candidate is Linux x86_64 only'
    [[ $(id -u) != 0 ]] || fail 'do not run browser as root'
    [[ -d $backend_prefix && -x $backend_binary ]] || fail 'engine install or Ferric debug binary is absent'
    [[ $(realpath -- "$backend_prefix") == "$backend_prefix" ]] || fail 'engine prefix must not redirect through a symlink'
    [[ -r $backend_manifest && -r $backend_tree_manifest ]] || fail 'candidate manifest is absent'

    # The versioned libraries are files; the SONAME links must resolve to
    # exactly those files inside the verified prefix.
    local library
    for library in Core Quick; do
        [[ $(realpath -- "$backend_prefix/lib/libQt6WebEngine${library}.so.6") == \
            "$backend_prefix/lib/libQt6WebEngine${library}.so.6.11.2" ]] \
            || fail "WebEngine${library} SONAME link points outside the pinned candidate"
    done
    (cd -- "$backend_prefix" && sha256sum --check --status "$backend_manifest") \
        || fail 'a pinned engine library or process differs from the manifest'

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
    [[ $tree_actual == "$tree_expected" ]] || fail 'QML, resource or locale tree differs from the manifest'

    # `ldd` is only a local diagnostic check here, after verifying the chosen
    # libraries. A release bundle needs its own loader-path/security review.
    local resolved
    resolved=$(env LD_LIBRARY_PATH="$backend_prefix/lib" ldd "$backend_binary") \
        || fail 'could not inspect binary dependencies'
    [[ $resolved == *"libQt6WebEngineCore.so.6 => $backend_prefix/lib/libQt6WebEngineCore.so.6"* \
        && $resolved == *"libQt6WebEngineQuick.so.6 => $backend_prefix/lib/libQt6WebEngineQuick.so.6"* ]] \
        || fail 'Ferric would load a different WebEngine library'
    [[ $resolved != *'not found'* ]] || fail 'Ferric has an unresolved library dependency'
}

launch_disposable() {
    verify_candidate
    umask 077
    mkdir -p -- "$backend_repo/target/versioning-engine-backend"
    local backend_runtime
    backend_runtime=$(mktemp -d "$backend_repo/target/versioning-engine-backend/run.XXXXXX")
    mkdir -m 700 -- "$backend_runtime/runtime"
    printf 'Disposable runtime: %s\n' "$backend_runtime"
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

case ${1:-} in
    check)
        [[ $# == 1 ]] || fail 'usage: verify-and-run.sh check | run | test-view-host'
        verify_candidate
        printf 'Pinned disposable engine candidate verified; no capability was enabled.\n'
        ;;
    run)
        [[ $# == 1 ]] || fail 'usage: verify-and-run.sh check | run | test-view-host'
        launch_disposable "$backend_binary" --safe-mode
        ;;
    test-view-host)
        [[ $# == 1 ]] || fail 'usage: verify-and-run.sh check | run | test-view-host'
        launch_disposable timeout --signal=INT --kill-after=5s 90 \
            /usr/lib/qt6/bin/qmltestrunner \
            -input "$backend_repo/crates/ferric-browser-engine-qt/tests/qml/tst_versioning_view_host.qml"
        ;;
    *) fail 'usage: verify-and-run.sh check | run | test-view-host' ;;
esac
