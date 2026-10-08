#!/usr/bin/env bash
# Separate, disposable post-snapshot race probe. Never use a normal profile.
set -euo pipefail

t0_repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
t0_tools="$t0_repo/tools/versioning-engine-spike"
t0_root="$t0_repo/target/versioning-engine-spike"
t0_source="$t0_root/qtwebengine-everywhere-src-6.11.2"
t0_prefix="$t0_root/install"
t0_patch="$t0_tools/qt-6.11.2-t0-race-probe.patch"

[[ $(id -u) != 0 ]] || { printf '%s\n' 'Do not run this fixture as root.' >&2; exit 1; }

apply_overlay() {
    if patch -d "$t0_source" -p1 --fuzz=0 --dry-run --batch --reverse -i "$t0_patch" >/dev/null 2>&1; then
        printf 'Post-snapshot race overlay is already applied.\n'
        return
    fi
    bash "$t0_tools/run.sh" prepare
    patch -d "$t0_source" -p1 --fuzz=0 --dry-run --batch --forward -i "$t0_patch"
    patch -d "$t0_source" -p1 --fuzz=0 --batch --forward -i "$t0_patch"
}

run_stage() {
    local run_root=$1 stage=$2
    env -u QTWEBENGINE_REMOTE_DEBUGGING -u QTWEBENGINE_DISABLE_SANDBOX \
        -u QTWEBENGINE_CHROMIUM_FLAGS -u LD_PRELOAD \
        LD_LIBRARY_PATH="$t0_prefix/lib" \
        QTWEBENGINEPROCESS_PATH="$t0_prefix/lib/qt6/QtWebEngineProcess" \
        QTWEBENGINE_RESOURCES_PATH="$t0_prefix/share/qt6/resources" \
        QTWEBENGINE_LOCALES_PATH="$t0_prefix/share/qt6/translations/qtwebengine_locales" \
        XDG_CONFIG_HOME="$run_root/config" XDG_DATA_HOME="$run_root/data" \
        XDG_CACHE_HOME="$run_root/cache" XDG_STATE_HOME="$run_root/state" \
        XDG_RUNTIME_DIR="$run_root/runtime" QT_QPA_PLATFORM=offscreen \
        QT_QPA_PLATFORMTHEME= QT_STYLE_OVERRIDE=Fusion \
        FERRIC_SPIKE_DISPOSABLE_ROOT="$run_root" \
        timeout --signal=INT --kill-after=10s 180 \
        "$t0_root/probe-build/ferric-engine-legacy-cookie-probe" \
        "$stage" "$run_root/t0-source" t0
}

probe_t0() {
    [[ -x "$t0_prefix/lib/qt6/QtWebEngineProcess" \
            && -x "$t0_root/probe-build/ferric-engine-legacy-cookie-probe" ]] \
        || { printf '%s\n' 'Build/install the isolated engine and probe first.' >&2; exit 1; }
    local run_root
    run_root=$(mktemp -d "$t0_root/probe-runtime.XXXXXX")
    mkdir -p "$run_root/config" "$run_root/data" "$run_root/cache" \
        "$run_root/state" "$run_root/runtime" \
        "$run_root/t0-source" "$run_root/t0-source-race-child"
    chmod 700 "$run_root/runtime"
    printf 'Disposable post-snapshot fixture: %s\n' "$run_root"
    run_stage "$run_root" seed
    run_stage "$run_root" race
}

case ${1:-help} in
    apply) apply_overlay ;;
    build) bash "$t0_tools/run.sh" build ;;
    install) bash "$t0_tools/run.sh" install-probe ;;
    probe) probe_t0 ;;
    *) printf 'Usage: bash tools/versioning-engine-spike/run-t0.sh {apply|build|install|probe}\n' ;;
esac
