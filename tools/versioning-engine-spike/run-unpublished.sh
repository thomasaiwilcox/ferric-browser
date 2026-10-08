#!/usr/bin/env bash
# Disposable page-free child cookie-manager fixture. Never use a normal profile.
set -euo pipefail

unpublished_repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
unpublished_tools="$unpublished_repo/tools/versioning-engine-spike"
unpublished_source="$unpublished_repo/target/versioning-engine-spike/qtwebengine-everywhere-src-6.11.2"
unpublished_patch="$unpublished_tools/qt-6.11.2-unpublished-child-probe.patch"

[[ $(id -u) != 0 ]] || { printf '%s\n' 'Do not run this fixture as root.' >&2; exit 1; }

apply_overlay() {
    if patch -d "$unpublished_source" -p1 --fuzz=0 --dry-run --batch --reverse \
            -i "$unpublished_patch" >/dev/null 2>&1; then
        printf 'Page-free child overlay is already applied.\n'
        return
    fi
    bash "$unpublished_tools/run-t0.sh" apply
    patch -d "$unpublished_source" -p1 --fuzz=0 --dry-run --batch --forward \
        -i "$unpublished_patch"
    patch -d "$unpublished_source" -p1 --fuzz=0 --batch --forward \
        -i "$unpublished_patch"
}

case ${1:-help} in
    apply) apply_overlay ;;
    build) bash "$unpublished_tools/run.sh" build ;;
    install) bash "$unpublished_tools/run.sh" install-probe ;;
    probe) bash "$unpublished_tools/run.sh" probe-unpublished ;;
    owner-loss) bash "$unpublished_tools/run.sh" probe-owner-loss ;;
    *) printf 'Usage: bash tools/versioning-engine-spike/run-unpublished.sh {apply|build|install|probe|owner-loss}\n' ;;
esac
