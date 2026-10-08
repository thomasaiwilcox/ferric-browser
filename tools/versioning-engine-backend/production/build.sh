#!/usr/bin/env bash
# Isolated, source-pinned Linux x86_64 development build. Never touches system Qt.
set -euo pipefail

backend_repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd -P)
backend_tools="$backend_repo/tools/versioning-engine-backend/production"
backend_root="$backend_repo/target/versioning-engine-backend/production"
backend_source="$backend_root/source"
backend_build="$backend_root/build"
backend_install="$backend_root/install"
backend_patch="$backend_tools/qt-6.11.2-owner-abi.patch"
backend_system_qt_manifest="$backend_tools/system-qt-linux-x86_64.sha256"
backend_archive_name=qtwebengine-everywhere-src-6.11.2.tar.xz
backend_archive="$backend_root/$backend_archive_name"
backend_archive_sha=6101c1aa00ff933d1b65ee5d167f76e8d71b9ac5b378b0111277723ebda7c163
backend_patch_sha=7b692e9af2c8eb015c9ca36839b6ef061d95675cd409cea68482ad67d1c7ddb3
backend_gperf_name=gperf-3.3-2-x86_64.pkg.tar.zst
backend_gperf_sha=87c8fb6323a2609b77767d2820f26b4c0d3337367a753d0b84180a983a6a1aa3
backend_gperf="$backend_root/$backend_gperf_name"
backend_config_summary_sha=bd56dc4129f213453dd00b695d92b289c6ef73c115d6d3d1415986b59c8a8e02
backend_args_gn_sha=a5f0185ad97667572e1af2664c217b2c19ec544eb56fedcefa4e2f8b92143fb8
backend_jobs=${FERRIC_BACKEND_JOBS:-3}

fail() { printf 'pinned backend build refused: %s\n' "$*" >&2; exit 1; }
verify_hash() { printf '%s  %s\n' "$1" "$2" | sha256sum --check --status; }
verify_configuration() {
    verify_hash "$backend_config_summary_sha" "$backend_build/config.summary" \
        && verify_hash "$backend_args_gn_sha" "$backend_build/src/core/Release/x86_64/args.gn"
}

[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || fail 'this tuple is Linux x86_64 only'
[[ $(id -u) != 0 ]] || fail 'do not build as root'
[[ $(/usr/lib/qt6/bin/qtpaths --query QT_VERSION) == 6.11.2 ]] \
    || fail 'system Qt build SDK must be 6.11.2'
[[ $backend_jobs =~ ^[1-4]$ ]] || fail 'FERRIC_BACKEND_JOBS must be 1–4'
verify_hash "$backend_patch_sha" "$backend_patch" || fail 'production patch checksum changed'
sha256sum --check --status "$backend_system_qt_manifest" \
    || fail 'system Qt runtime differs from the pinned build tuple'

prepare() {
    mkdir -p -- "$backend_root"
    if [[ ! -f $backend_archive ]]; then
        local research_archive="$backend_repo/target/versioning-engine-spike/$backend_archive_name"
        if [[ -f $research_archive ]] && verify_hash "$backend_archive_sha" "$research_archive"; then
            cp -- "$research_archive" "$backend_archive"
        else
            curl --fail --location --retry 2 --max-time 600 \
                --output "$backend_archive.part" \
                "https://download.qt.io/official_releases/qt/6.11/6.11.2/submodules/$backend_archive_name"
            verify_hash "$backend_archive_sha" "$backend_archive.part" \
                || fail 'Qt source archive checksum mismatch'
            mv -- "$backend_archive.part" "$backend_archive"
        fi
    fi
    verify_hash "$backend_archive_sha" "$backend_archive" \
        || fail 'Qt source archive checksum mismatch'
    if [[ ! -d $backend_source ]]; then
        mkdir -p -- "$backend_source"
        tar -xJf "$backend_archive" -C "$backend_source" --strip-components=1
    fi
    [[ -f $backend_source/src/webenginequick/api/qquickwebengineview.cpp ]] \
        || fail 'source tree is missing the Qt Quick view implementation'
    if patch -d "$backend_source" -p1 --fuzz=0 --dry-run --batch --reverse \
        -i "$backend_patch" >/dev/null 2>&1; then
        printf 'Production owner-ABI patch already applied.\n'
    else
        patch -d "$backend_source" -p1 --fuzz=0 --dry-run --batch --forward \
            -i "$backend_patch" >/dev/null \
            || fail 'source is neither clean nor exactly patched'
        patch -d "$backend_source" -p1 --fuzz=0 --batch --forward -i "$backend_patch"
    fi
    if [[ ! -f $backend_gperf ]]; then
        local research_package="$backend_repo/target/versioning-engine-spike/$backend_gperf_name"
        if [[ -f $research_package ]] && verify_hash "$backend_gperf_sha" "$research_package"; then
            cp -- "$research_package" "$backend_gperf"
        else
            curl --fail --location --retry 2 --max-time 120 \
                --output "$backend_gperf.part" \
                "https://stable-mirror.omarchy.org/extra/os/x86_64/$backend_gperf_name"
            verify_hash "$backend_gperf_sha" "$backend_gperf.part" \
                || fail 'gperf package checksum mismatch'
            mv -- "$backend_gperf.part" "$backend_gperf"
        fi
    fi
    verify_hash "$backend_gperf_sha" "$backend_gperf" || fail 'gperf package checksum mismatch'
    mkdir -p -- "$backend_root/local-tools"
    tar -xf "$backend_gperf" -C "$backend_root/local-tools"
    command -v uv >/dev/null || fail 'uv is required for the pinned local Python dependencies'
    if [[ ! -x $backend_root/venv/bin/python ]]; then
        uv venv --python /usr/bin/python3 "$backend_root/venv"
    fi
    uv pip install --python "$backend_root/venv/bin/python" \
        html5lib==1.1 six==1.17.0 webencodings==0.5.1
}

configure() {
    [[ -x $backend_root/venv/bin/python && -x $backend_root/local-tools/usr/bin/gperf ]] \
        || fail 'run prepare first'
    env PYTHONDONTWRITEBYTECODE=1 \
        PATH="$backend_root/local-tools/usr/bin:$backend_root/venv/bin:$PATH" \
        NINJAFLAGS="-j$backend_jobs" /usr/lib/qt6/bin/qt-cmake \
        -S "$backend_source" -B "$backend_build" -G Ninja \
        -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
        -DCMAKE_INSTALL_PREFIX="$backend_install" \
        -DPython3_EXECUTABLE="$backend_root/venv/bin/python" \
        -DQT_BUILD_EXAMPLES=OFF -DQT_BUILD_TESTS=OFF -DQT_SHOW_EXTRA_IDE_SOURCES=OFF \
        -DFEATURE_qtwebengine_build=ON -DFEATURE_qtwebengine_core_build=ON \
        -DFEATURE_qtwebengine_widgets_build=ON -DFEATURE_qtwebengine_quick_build=ON \
        -DFEATURE_qtpdf_build=OFF -DFEATURE_webengine_jumbo_build=8 \
        -DFEATURE_webengine_printing_and_pdf=OFF -DFEATURE_webengine_spellchecker=OFF \
        -DFEATURE_webengine_webrtc=ON
    rg -q 'Build QtWebEngineCore .*yes' "$backend_build/config.summary" \
        || fail 'Qt configuration disabled WebEngineCore'
    rg -q 'WebRTC .*yes' "$backend_build/config.summary" \
        || fail 'WebRTC must remain enabled'
    verify_configuration || fail 'generated build configuration differs from the pinned tuple'
}

build() {
    [[ -f $backend_build/build.ninja ]] || fail 'run configure first'
    verify_configuration || fail 'generated build configuration differs from the pinned tuple'
    verify_hash "$backend_archive_sha" "$backend_archive" \
        || fail 'Qt source archive checksum changed'
    patch -d "$backend_source" -p1 --fuzz=0 --dry-run --batch --reverse \
        -i "$backend_patch" >/dev/null \
        || fail 'patched source no longer matches the pinned owner-ABI patch'
    python3 "$backend_tools/verify-source.py" \
        || fail 'build source differs from the pinned archive plus exact patch'
    env PYTHONDONTWRITEBYTECODE=1 \
        PATH="$backend_root/local-tools/usr/bin:$backend_root/venv/bin:$PATH" \
        NINJAFLAGS="-j$backend_jobs" nice -n 10 \
        cmake --build "$backend_build" --parallel "$backend_jobs"
}

install_build() {
    [[ -f $backend_build/build.ninja ]] || fail 'run configure first'
    verify_configuration || fail 'generated build configuration differs from the pinned tuple'
    patch -d "$backend_source" -p1 --fuzz=0 --dry-run --batch --reverse \
        -i "$backend_patch" >/dev/null \
        || fail 'patched source no longer matches the pinned owner-ABI patch'
    python3 "$backend_tools/verify-source.py" \
        || fail 'built source differs from the pinned archive plus exact patch'
    env -u DESTDIR cmake --install "$backend_build" --prefix "$backend_install"
    bash "$backend_repo/tools/versioning-engine-backend/check-no-probe-api.sh" "$backend_install"
}

case ${1:-} in
    prepare) prepare ;;
    configure) configure ;;
    build) build ;;
    install) install_build ;;
    all) prepare; configure; build; install_build ;;
    *) fail 'usage: build.sh prepare | configure | build | install | all' ;;
esac
