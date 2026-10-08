#!/usr/bin/env bash
# Disposable bound-but-disconnected CookieManager pipe experiment.
set -euo pipefail

disconnect_repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
disconnect_tools="$disconnect_repo/tools/versioning-engine-spike"
disconnect_source="$disconnect_repo/target/versioning-engine-spike/qtwebengine-everywhere-src-6.11.2"
disconnect_source_relative="target/versioning-engine-spike/qtwebengine-everywhere-src-6.11.2"
disconnect_patch="$disconnect_tools/qt-6.11.2-cookie-service-loss-probe.patch"

[[ $(id -u) != 0 ]] || { printf '%s\n' 'Do not run this fixture as root.' >&2; exit 1; }

apply_overlay() {
    bash "$disconnect_tools/run-unpublished.sh" apply
    if rg -q 'FerricServiceLossProbeStage' \
            "$disconnect_source/src/core/api/qwebenginecookiestore.h" \
            && rg -q 'FerricProbeDisconnectAfterNextGetAllCookies' \
                "$disconnect_source/src/3rdparty/chromium/services/network/cookie_manager.cc"; then
        printf 'Cookie service-loss overlay is already applied.\n'
        return
    fi
    git -C "$disconnect_repo" apply --check --recount --unidiff-zero \
        --directory="$disconnect_source_relative" "$disconnect_patch"
    git -C "$disconnect_repo" apply --recount --unidiff-zero \
        --directory="$disconnect_source_relative" "$disconnect_patch"
}

case ${1:-help} in
    apply) apply_overlay ;;
    build) apply_overlay; bash "$disconnect_tools/run.sh" build ;;
    install) apply_overlay; bash "$disconnect_tools/run.sh" install-probe ;;
    probe) bash "$disconnect_tools/run.sh" probe-service-loss ;;
    baseline) apply_overlay; bash "$disconnect_tools/run.sh" build; bash "$disconnect_tools/run.sh" install-probe; bash "$disconnect_tools/run.sh" probe-service-loss ;;
    *) printf 'Usage: bash tools/versioning-engine-spike/run-network-disconnect.sh {apply|build|install|probe|baseline}\n' ;;
esac
