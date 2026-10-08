#!/usr/bin/env bash
# Isolated research build. Never installs into, or reconfigures, system Qt.
set -euo pipefail

spike_repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
spike_tools="$spike_repo/tools/versioning-engine-spike"
spike_root="$spike_repo/target/versioning-engine-spike"
spike_src="$spike_root/qtwebengine-everywhere-src-6.11.2"
spike_build="$spike_root/build"
spike_prefix="$spike_root/install"
spike_archive=qtwebengine-everywhere-src-6.11.2.tar.xz
spike_sha=6101c1aa00ff933d1b65ee5d167f76e8d71b9ac5b378b0111277723ebda7c163
spike_patch="$spike_tools/qt-6.11.2-capability-probes.patch"
seed_patch="$spike_tools/qt-6.11.2-seed-if-empty-probe.patch"
rtc_admission_patch="$spike_tools/qt-6.11.2-rtc-admission-probe.patch"
rtc_fake_desktop_patch="$spike_tools/qt-6.11.2-rtc-fake-desktop-probe.patch"
rtc_quick_bridge_patch="$spike_tools/qt-6.11.2-rtc-quick-bridge-probe.patch"
rtc_child_capture_patch="$spike_tools/qt-6.11.2-rtc-child-frame-capture-probe.patch"
rtc_frame_activation_patch="$spike_tools/qt-6.11.2-rtc-frame-activation-probe.patch"
rtc_capture_rejection_patch="$spike_tools/qt-6.11.2-rtc-capture-rejection-diagnostic.patch"
rtc_capture_dispatch_patch="$spike_tools/qt-6.11.2-rtc-capture-dispatch-diagnostic.patch"
spike_jobs=${FERRIC_SPIKE_JOBS:-3}

fail() { printf '%s\n' "$*" >&2; exit 1; }
verify() { printf '%s  %s\n' "$1" "$2" | sha256sum --check --status; }
[[ $spike_jobs =~ ^[1-4]$ ]] || fail 'FERRIC_SPIKE_JOBS must be 1–4 on this build host.'
[[ $(id -u) != 0 ]] || fail 'Do not run this experiment as root.'
mkdir -p "$spike_root"

prepare() {
    command -v uv >/dev/null || fail 'Install/locate uv before preparing the local venv.'
    if [[ ! -f "$spike_root/$spike_archive" ]]; then
        curl --fail --location --retry 2 --output "$spike_root/$spike_archive.part" \
            "https://download.qt.io/official_releases/qt/6.11/6.11.2/submodules/$spike_archive"
        verify "$spike_sha" "$spike_root/$spike_archive.part" || fail 'Qt archive checksum mismatch.'
        mv "$spike_root/$spike_archive.part" "$spike_root/$spike_archive"
    fi
    verify "$spike_sha" "$spike_root/$spike_archive" || fail 'Qt archive checksum mismatch.'
    if [[ ! -d "$spike_src" ]]; then
        tar -xJf "$spike_root/$spike_archive" -C "$spike_root"
    fi
    # Refuse ambiguous or partially applied patches. No checkout/reset/cleanup.
    if patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --reverse -i "$spike_patch" >/dev/null 2>&1; then
        printf 'Capability probe patch is already applied.\n'
    else
        patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --forward -i "$spike_patch"
        patch -d "$spike_src" -p1 --fuzz=0 --batch --forward -i "$spike_patch"
    fi
    if patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --reverse -i "$seed_patch" >/dev/null 2>&1; then
        printf 'Experimental seed-if-empty patch is already applied.\n'
    else
        patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --forward -i "$seed_patch"
        patch -d "$spike_src" -p1 --fuzz=0 --batch --forward -i "$seed_patch"
    fi
    if patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --reverse -i "$rtc_admission_patch" >/dev/null 2>&1; then
        printf 'Experimental RTC admission patch is already applied.\n'
    else
        patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --forward -i "$rtc_admission_patch"
        patch -d "$spike_src" -p1 --fuzz=0 --batch --forward -i "$rtc_admission_patch"
    fi
    if patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --reverse -i "$rtc_fake_desktop_patch" >/dev/null 2>&1; then
        printf 'Experimental RTC synthetic-frame patch is already applied.\n'
    else
        patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --forward -i "$rtc_fake_desktop_patch"
        patch -d "$spike_src" -p1 --fuzz=0 --batch --forward -i "$rtc_fake_desktop_patch"
    fi
    if patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --reverse -i "$rtc_quick_bridge_patch" >/dev/null 2>&1; then
        printf 'Experimental Qt Quick RTC bridge patch is already applied.\n'
    else
        patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --forward -i "$rtc_quick_bridge_patch"
        patch -d "$spike_src" -p1 --fuzz=0 --batch --forward -i "$rtc_quick_bridge_patch"
    fi
    if patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --reverse -i "$rtc_child_capture_patch" >/dev/null 2>&1; then
        printf 'Experimental child-frame RTC capture-state patch is already applied.\n'
    else
        patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --forward -i "$rtc_child_capture_patch"
        patch -d "$spike_src" -p1 --fuzz=0 --batch --forward -i "$rtc_child_capture_patch"
    fi
    if patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --reverse -i "$rtc_frame_activation_patch" >/dev/null 2>&1; then
        printf 'Experimental frame-activation diagnostics patch is already applied.\n'
    else
        patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --forward -i "$rtc_frame_activation_patch"
        patch -d "$spike_src" -p1 --fuzz=0 --batch --forward -i "$rtc_frame_activation_patch"
    fi
    if patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --reverse -i "$rtc_capture_rejection_patch" >/dev/null 2>&1; then
        printf 'Experimental capture-rejection diagnostic patch is already applied.\n'
    else
        patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --forward -i "$rtc_capture_rejection_patch"
        patch -d "$spike_src" -p1 --fuzz=0 --batch --forward -i "$rtc_capture_rejection_patch"
    fi
    if patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --reverse -i "$rtc_capture_dispatch_patch" >/dev/null 2>&1; then
        printf 'Experimental capture-dispatch diagnostic patch is already applied.\n'
    else
        patch -d "$spike_src" -p1 --fuzz=0 --dry-run --batch --forward -i "$rtc_capture_dispatch_patch"
        patch -d "$spike_src" -p1 --fuzz=0 --batch --forward -i "$rtc_capture_dispatch_patch"
    fi
    if [[ ! -x "$spike_root/venv/bin/python" ]]; then
        uv venv --python /usr/bin/python3 "$spike_root/venv"
    fi
    uv pip install --python "$spike_root/venv/bin/python" \
        html5lib==1.1 six==1.17.0 webencodings==0.5.1
    if ! command -v gperf >/dev/null; then
        # This host's missing tool is unpacked locally, never pacman-installed.
        local package=gperf-3.3-2-x86_64.pkg.tar.zst
        local digest=87c8fb6323a2609b77767d2820f26b4c0d3337367a753d0b84180a983a6a1aa3
        if [[ ! -f "$spike_root/$package" ]]; then
            curl --fail --location --retry 2 --output "$spike_root/$package" \
                "https://stable-mirror.omarchy.org/extra/os/x86_64/$package"
        fi
        verify "$digest" "$spike_root/$package" || fail 'gperf package checksum mismatch.'
        mkdir -p "$spike_root/local-tools"
        tar -xf "$spike_root/$package" -C "$spike_root/local-tools"
    fi
}

configure() {
    [[ -x "$spike_root/venv/bin/python" ]] || fail 'Run prepare first.'
    env PATH="$spike_root/local-tools/usr/bin:$spike_root/venv/bin:$PATH" \
        NINJAFLAGS="-j$spike_jobs" /usr/lib/qt6/bin/qt-cmake \
        -S "$spike_src" -B "$spike_build" -G Ninja \
        -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
        -DCMAKE_INSTALL_PREFIX="$spike_prefix" \
        -DPython3_EXECUTABLE="$spike_root/venv/bin/python" \
        -DQT_BUILD_EXAMPLES=OFF -DQT_BUILD_TESTS=OFF -DQT_SHOW_EXTRA_IDE_SOURCES=OFF \
        -DFEATURE_qtwebengine_build=ON -DFEATURE_qtwebengine_core_build=ON \
        -DFEATURE_qtwebengine_widgets_build=ON -DFEATURE_qtwebengine_quick_build=ON \
        -DFEATURE_qtpdf_build=OFF -DFEATURE_webengine_jumbo_build=8 \
        -DFEATURE_webengine_printing_and_pdf=OFF -DFEATURE_webengine_spellchecker=OFF \
        -DFEATURE_webengine_webrtc=ON
    # Qt can exit successfully while disabling WebEngine after a failed check.
    grep -Eq 'Build QtWebEngineCore .*yes' "$spike_build/config.summary" \
        || fail 'Qt configuration disabled WebEngineCore; inspect config.summary.'
    grep -Eq 'WebRTC .*yes' "$spike_build/config.summary" \
        || fail 'WebRTC must remain enabled.'
}

build() {
    local seconds=${FERRIC_SPIKE_BUILD_SECONDS:-43200}
    [[ $seconds =~ ^[1-9][0-9]*$ ]] || fail 'Build duration must be a positive integer.'
    # A bounded foreground build, with ordinary Ninja incremental continuation.
    # Exit 124 means the chunk timed out, not that a native test passed/failed.
    env PATH="$spike_root/local-tools/usr/bin:$spike_root/venv/bin:$PATH" \
        NINJAFLAGS="-j$spike_jobs" nice -n 10 timeout --signal=INT --kill-after=20s "$seconds" \
        cmake --build "$spike_build" --parallel "$spike_jobs"
}

install_probe() {
    # The fixed local prefix is explicit even if a CMake cache was modified.
    # An inherited packaging destination must not relocate this experiment.
    env -u DESTDIR cmake --install "$spike_build" --prefix "$spike_prefix"
    /usr/lib/qt6/bin/qt-cmake -S "$spike_tools" -B "$spike_root/probe-build" -G Ninja \
        -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="$spike_prefix" \
        -DQt6WebEngineCore_DIR="$spike_prefix/lib/cmake/Qt6WebEngineCore"
    cmake --build "$spike_root/probe-build" --parallel "$spike_jobs"
}

probe() {
    [[ -x "$spike_prefix/lib/qt6/QtWebEngineProcess" ]] || fail 'Build/install the isolated engine first.'
    local run_root
    run_root=$(mktemp -d "$spike_root/probe-runtime.XXXXXX")
    mkdir -p "$run_root/config" "$run_root/data" "$run_root/cache" "$run_root/state" "$run_root/runtime"
    chmod 700 "$run_root/runtime"
    printf 'Disposable runtime directory: %s\n' "$run_root"
    local executable="$spike_root/probe-build/ferric-engine-cookie-probe"
    local -a extra_arguments=()
    local -a isolated_environment=()
    if [[ ${1:-baseline} == https || ${1:-baseline} == rtc || ${1:-baseline} == rtc-oop || ${1:-baseline} == rtc-quick || ${1:-baseline} == rtc-quick-oop || ${1:-baseline} == rtc-quick-sw || ${1:-baseline} == rtc-quick-capture || ${1:-baseline} == rtc-quick-capture-oop || ${1:-baseline} == rtc-quick-capture-same-oop || ${1:-baseline} == rtc-quick-capture-grant-oop || ${1:-baseline} == rtc-quick-capture-deny-oop ]]; then
        command -v openssl >/dev/null || fail 'The owned HTTPS fixture requires openssl.'
        openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
            -subj /CN=ferric-owned-loopback-fixture \
            -addext 'subjectAltName=DNS:cookie.localhost,DNS:sub.cookie.localhost,DNS:top-a.localhost,DNS:top-b.localhost' \
            -keyout "$run_root/private-key.pem" -out "$run_root/certificate.pem" \
            >"$run_root/certificate-generation.log" 2>&1
        executable="$spike_root/probe-build/ferric-engine-cookie-request-probe"
        if [[ ${1:-baseline} == rtc || ${1:-baseline} == rtc-oop ]]; then
            executable="$spike_root/probe-build/ferric-engine-rtc-registry-probe"
            isolated_environment=(QTWEBENGINE_CHROMIUM_FLAGS=--use-fake-device-for-media-stream)
        fi
        if [[ ${1:-baseline} == rtc-quick || ${1:-baseline} == rtc-quick-oop || ${1:-baseline} == rtc-quick-sw || ${1:-baseline} == rtc-quick-capture || ${1:-baseline} == rtc-quick-capture-oop || ${1:-baseline} == rtc-quick-capture-same-oop || ${1:-baseline} == rtc-quick-capture-grant-oop || ${1:-baseline} == rtc-quick-capture-deny-oop ]]; then
            executable="$spike_root/probe-build/ferric-engine-rtc-quick-bridge-probe"
            isolated_environment=(QTWEBENGINE_CHROMIUM_FLAGS=--use-fake-device-for-media-stream
                FERRIC_SPIKE_QML_IMPORT_PATH="$spike_prefix/lib/qt6/qml")
        fi
        if [[ ${1:-baseline} == rtc-quick-oop ]]; then
            isolated_environment=(QTWEBENGINE_CHROMIUM_FLAGS='--use-fake-device-for-media-stream --site-per-process'
                FERRIC_SPIKE_QML_IMPORT_PATH="$spike_prefix/lib/qt6/qml")
        fi
        if [[ ${1:-baseline} == rtc-quick-capture ]]; then
            isolated_environment=(QTWEBENGINE_CHROMIUM_FLAGS=--use-fake-device-for-media-stream
                FERRIC_SPIKE_QML_IMPORT_PATH="$spike_prefix/lib/qt6/qml"
                FERRIC_SPIKE_CROSS_FRAME_CAPTURE_PROBE=1)
        fi
        if [[ ${1:-baseline} == rtc-quick-capture-oop ]]; then
            isolated_environment=(QTWEBENGINE_CHROMIUM_FLAGS='--use-fake-device-for-media-stream --site-per-process'
                FERRIC_SPIKE_QML_IMPORT_PATH="$spike_prefix/lib/qt6/qml"
                FERRIC_SPIKE_CROSS_FRAME_CAPTURE_PROBE=1)
        fi
        if [[ ${1:-baseline} == rtc-quick-capture-same-oop ]]; then
            isolated_environment=(QTWEBENGINE_CHROMIUM_FLAGS='--use-fake-device-for-media-stream --site-per-process'
                FERRIC_SPIKE_QML_IMPORT_PATH="$spike_prefix/lib/qt6/qml"
                FERRIC_SPIKE_CROSS_FRAME_CAPTURE_PROBE=1
                FERRIC_SPIKE_CROSS_FRAME_CAPTURE_SAME_ORIGIN_PROBE=1)
        fi
        if [[ ${1:-baseline} == rtc-quick-capture-grant-oop ]]; then
            isolated_environment=(QTWEBENGINE_CHROMIUM_FLAGS='--use-fake-device-for-media-stream --site-per-process'
                FERRIC_SPIKE_QML_IMPORT_PATH="$spike_prefix/lib/qt6/qml"
                FERRIC_SPIKE_CROSS_FRAME_CAPTURE_PROBE=1
                FERRIC_SPIKE_FORCE_CHILD_VIDEO_GRANT_PROBE=1)
        fi
        if [[ ${1:-baseline} == rtc-quick-capture-deny-oop ]]; then
            isolated_environment=(QTWEBENGINE_CHROMIUM_FLAGS='--use-fake-device-for-media-stream --site-per-process'
                FERRIC_SPIKE_QML_IMPORT_PATH="$spike_prefix/lib/qt6/qml"
                FERRIC_SPIKE_CROSS_FRAME_CAPTURE_PROBE=1
                FERRIC_SPIKE_FORCE_CHILD_VIDEO_DENY_PROBE=1)
        fi
        if [[ ${1:-baseline} == rtc-quick-sw ]]; then
            isolated_environment=(QTWEBENGINE_CHROMIUM_FLAGS='--use-fake-device-for-media-stream --disable-gpu'
                FERRIC_SPIKE_QML_IMPORT_PATH="$spike_prefix/lib/qt6/qml")
        fi
        if [[ ${1:-baseline} == rtc-oop ]]; then
            # Qt's default disables strict site isolation. This stronger, fixed
            # configuration is a separate fixture tuple, never flag passthrough.
            isolated_environment=(QTWEBENGINE_CHROMIUM_FLAGS='--use-fake-device-for-media-stream --site-per-process')
        fi
        extra_arguments=("$run_root/certificate.pem" "$run_root/private-key.pem")
    fi
    # Keep the disposable directory for inspection, never point at a normal profile.
    # Use no debugger, preloads or sandbox overrides. RTC probes use only the
    # fixed fake-device fixture switch; the OOP variant adds strict isolation.
    # Inherited browser flags are removed.
    env -u QTWEBENGINE_REMOTE_DEBUGGING -u QTWEBENGINE_DISABLE_SANDBOX \
        -u QTWEBENGINE_CHROMIUM_FLAGS -u LD_PRELOAD \
        -u FERRIC_SPIKE_CROSS_FRAME_CAPTURE_PROBE \
        -u FERRIC_SPIKE_CROSS_FRAME_CAPTURE_SAME_ORIGIN_PROBE \
        -u FERRIC_SPIKE_FORCE_CHILD_VIDEO_GRANT_PROBE \
        -u FERRIC_SPIKE_FORCE_CHILD_VIDEO_DENY_PROBE \
        LD_LIBRARY_PATH="$spike_prefix/lib" \
        QTWEBENGINEPROCESS_PATH="$spike_prefix/lib/qt6/QtWebEngineProcess" \
        QTWEBENGINE_RESOURCES_PATH="$spike_prefix/share/qt6/resources" \
        QTWEBENGINE_LOCALES_PATH="$spike_prefix/share/qt6/translations/qtwebengine_locales" \
        XDG_CONFIG_HOME="$run_root/config" XDG_DATA_HOME="$run_root/data" \
        XDG_CACHE_HOME="$run_root/cache" XDG_STATE_HOME="$run_root/state" \
        XDG_RUNTIME_DIR="$run_root/runtime" QT_QPA_PLATFORM=offscreen \
        QT_QPA_PLATFORMTHEME= QT_STYLE_OVERRIDE=Fusion \
        "${isolated_environment[@]}" \
        timeout --signal=INT --kill-after=10s 180 \
        "$executable" "${extra_arguments[@]}"
}

run_legacy_stage() {
    local run_root=$1 fixture=$2 stage=$3
    local executable="$spike_root/probe-build/ferric-engine-legacy-cookie-probe"
    local source_dir="$run_root/$fixture-source"
    env -u QTWEBENGINE_REMOTE_DEBUGGING -u QTWEBENGINE_DISABLE_SANDBOX \
        -u QTWEBENGINE_CHROMIUM_FLAGS -u LD_PRELOAD \
        LD_LIBRARY_PATH="$spike_prefix/lib" \
        QTWEBENGINEPROCESS_PATH="$spike_prefix/lib/qt6/QtWebEngineProcess" \
        QTWEBENGINE_RESOURCES_PATH="$spike_prefix/share/qt6/resources" \
        QTWEBENGINE_LOCALES_PATH="$spike_prefix/share/qt6/translations/qtwebengine_locales" \
        XDG_CONFIG_HOME="$run_root/config" XDG_DATA_HOME="$run_root/data" \
        XDG_CACHE_HOME="$run_root/cache" XDG_STATE_HOME="$run_root/state" \
        XDG_RUNTIME_DIR="$run_root/runtime" QT_QPA_PLATFORM=offscreen \
        QT_QPA_PLATFORMTHEME= QT_STYLE_OVERRIDE=Fusion \
        FERRIC_SPIKE_DISPOSABLE_ROOT="$run_root" \
        timeout --signal=INT --kill-after=10s 180 \
        "$executable" "$stage" "$source_dir" "$fixture"
}

probe_legacy() {
    local seed_child=${1:-false}
    local child_seed_stage=${2:-seed-child}
    local owner_loss=${3:-false}
    [[ $seed_child == true || $seed_child == false ]] || fail 'Invalid seed fixture mode.'
    [[ $child_seed_stage == seed-child || $child_seed_stage == seed-child-unpublished ]] \
        || fail 'Invalid child seed fixture stage.'
    [[ $owner_loss == true || $owner_loss == false ]] || fail 'Invalid owner-loss fixture mode.'
    [[ -x "$spike_prefix/lib/qt6/QtWebEngineProcess" ]] || fail 'Build/install the isolated engine first.'
    [[ -x "$spike_root/probe-build/ferric-engine-legacy-cookie-probe" ]] \
        || fail 'Build the isolated legacy-cookie probe first.'
    command -v sqlite3 >/dev/null || fail 'The disposable backing-store fixture requires sqlite3.'
    local run_root
    run_root=$(mktemp -d "$spike_root/probe-runtime.XXXXXX")
    mkdir -p "$run_root/config" "$run_root/data" "$run_root/cache" \
        "$run_root/state" "$run_root/runtime" \
        "$run_root/legacy-source" "$run_root/older-source"
    chmod 700 "$run_root/runtime"
    printf 'Disposable backing-store fixture: %s\n' "$run_root"
    local fixture database
    for fixture in legacy older; do
        if [[ $seed_child == true ]]; then
            mkdir -p "$run_root/$fixture-source-child"
            mkdir -p "$run_root/$fixture-source-nonempty-child"
        fi
        if [[ $owner_loss == true ]]; then
            mkdir -p "$run_root/$fixture-source-owner-loss-child"
            mkdir -p "$run_root/$fixture-source-owner-loss-source-child"
        fi
        run_legacy_stage "$run_root" "$fixture" seed
        run_legacy_stage "$run_root" "$fixture" control
        if [[ $owner_loss == true ]]; then
            run_legacy_stage "$run_root" "$fixture" owner-loss
            run_legacy_stage "$run_root" "$fixture" source-owner-loss
        fi
        if [[ $seed_child == true ]]; then
            run_legacy_stage "$run_root" "$fixture" seed-nonempty
        fi
        database="$run_root/$fixture-source/Cookies"
        [[ -f "$database" && ! -L "$database" ]] \
            || fail "Disposable cookie database was not persisted: $database"
        # Research-only mutation of a closed, synthetic Qt profile. No cookie
        # database rows are read, exported, copied or used by production code.
        if [[ $fixture == legacy ]]; then
            sqlite3 -bail "$database" \
                "BEGIN IMMEDIATE; UPDATE cookies SET source_type=0 WHERE host_key='fixture.test' AND name='ferric_legacy_fixture' AND path='/'; COMMIT;"
        else
            # Chromium stores base::Time as microseconds. The reopened record
            # has a 600-day creation-to-expiry span but remains unexpired.
            sqlite3 -bail "$database" \
                "BEGIN IMMEDIATE; UPDATE cookies SET creation_utc=creation_utc-25920000000000, expires_utc=creation_utc+25920000000000 WHERE host_key='fixture.test' AND name='ferric_older_fixture' AND path='/'; COMMIT;"
        fi
        run_legacy_stage "$run_root" "$fixture" observe
        if [[ $seed_child == true ]]; then
            run_legacy_stage "$run_root" "$fixture" "$child_seed_stage"
            run_legacy_stage "$run_root" "$fixture" verify-child
        fi
    done
}

probe_service_loss() {
    [[ -x "$spike_prefix/lib/qt6/QtWebEngineProcess" ]] || fail 'Build/install the isolated engine first.'
    [[ -x "$spike_root/probe-build/ferric-engine-legacy-cookie-probe" ]] \
        || fail 'Build the isolated legacy-cookie probe first.'
    local stage iteration run_root
    for stage in service-loss-source service-loss-destination; do
        for iteration in 1 2 3; do
            run_root=$(mktemp -d "$spike_root/probe-runtime.XXXXXX")
            mkdir -p "$run_root/config" "$run_root/data" "$run_root/cache" \
                "$run_root/state" "$run_root/runtime" "$run_root/legacy-source" \
                "$run_root/legacy-source-service-loss-child"
            chmod 700 "$run_root/runtime"
            printf 'Disposable cookie service-loss campaign %s/%s: %s\n' \
                "$stage" "$iteration" "$run_root"
            run_legacy_stage "$run_root" legacy seed
            run_legacy_stage "$run_root" legacy control
            run_legacy_stage "$run_root" legacy "$stage"
        done
    done
}

case ${1:-help} in
    prepare) prepare ;;
    configure) configure ;;
    build) build ;;
    install-probe) install_probe ;;
    probe) probe ;;
    probe-https) probe https ;;
    probe-rtc) probe rtc ;;
    probe-rtc-oop) probe rtc-oop ;;
    probe-rtc-quick) probe rtc-quick ;;
    probe-rtc-quick-oop) probe rtc-quick-oop ;;
    probe-rtc-quick-capture) probe rtc-quick-capture ;;
    probe-rtc-quick-capture-oop) probe rtc-quick-capture-oop ;;
    probe-rtc-quick-capture-same-oop) probe rtc-quick-capture-same-oop ;;
    probe-rtc-quick-capture-grant-oop) probe rtc-quick-capture-grant-oop ;;
    probe-rtc-quick-capture-deny-oop) probe rtc-quick-capture-deny-oop ;;
    probe-rtc-quick-sw) probe rtc-quick-sw ;;
    probe-legacy) probe_legacy false ;;
    probe-seed) probe_legacy true ;;
    probe-unpublished) probe_legacy true seed-child-unpublished ;;
    probe-owner-loss) probe_legacy false seed-child true ;;
    probe-service-loss) probe_service_loss ;;
    baseline) prepare; configure; build; install_probe; probe ;;
    *) printf 'Usage: bash tools/versioning-engine-spike/run.sh {prepare|configure|build|install-probe|probe|probe-https|probe-rtc|probe-rtc-oop|probe-rtc-quick|probe-rtc-quick-oop|probe-rtc-quick-capture|probe-rtc-quick-capture-oop|probe-rtc-quick-capture-same-oop|probe-rtc-quick-capture-grant-oop|probe-rtc-quick-capture-deny-oop|probe-rtc-quick-sw|probe-legacy|probe-seed|probe-unpublished|probe-owner-loss|probe-service-loss|baseline}\n' ;;
esac
