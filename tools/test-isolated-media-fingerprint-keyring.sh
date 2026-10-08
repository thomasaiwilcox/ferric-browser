#!/usr/bin/env bash
set -euo pipefail

for required_tool in dbus-run-session gnome-keyring-daemon busctl cargo timeout; do
    if ! command -v "$required_tool" >/dev/null 2>&1; then
        printf 'required isolated keyring test tool is missing: %s\n' "$required_tool" >&2
        exit 127
    fi
done

ferric_isolated_root=$(mktemp -d /tmp/ferric-secret-service.XXXXXX)
trap 'rm -rf -- "$ferric_isolated_root"' EXIT
mkdir -p "$ferric_isolated_root/runtime" "$ferric_isolated_root/data" \
    "$ferric_isolated_root/config" "$ferric_isolated_root/cache"
chmod 700 "$ferric_isolated_root/runtime"

# Scope XDG and desktop-session variables before D-Bus starts: activated
# services must not inherit the interactive desktop's compositor or keyring.
env \
    -u GNOME_KEYRING_CONTROL \
    -u GNOME_KEYRING_PID \
    -u WAYLAND_DISPLAY \
    -u DISPLAY \
    -u XAUTHORITY \
    -u HYPRLAND_INSTANCE_SIGNATURE \
    -u XDG_CURRENT_DESKTOP \
    -u XDG_SESSION_TYPE \
    -u DBUS_SESSION_BUS_ADDRESS \
    -u DBUS_STARTER_ADDRESS \
    -u DBUS_STARTER_BUS_TYPE \
    XDG_RUNTIME_DIR="$ferric_isolated_root/runtime" \
    XDG_DATA_HOME="$ferric_isolated_root/data" \
    XDG_CONFIG_HOME="$ferric_isolated_root/config" \
    XDG_CACHE_HOME="$ferric_isolated_root/cache" \
    FERRIC_TEST_SECRET_SERVICE_ROOT="$ferric_isolated_root" \
    FERRIC_TEST_SECRET_SERVICE_ISOLATED=yes \
    dbus-run-session -- \
    bash -c '
        set -euo pipefail
        printf "\n" | gnome-keyring-daemon --unlock --components=secrets >/dev/null
        if [[ "${FERRIC_TEST_NATIVE_MEDIA_JOURNEY:-no}" == yes ]]; then
            # Run while the disposable collection is unlocked; the focused
            # lifecycle test below deliberately leaves it locked afterward.
            FERRIC_NATIVE_ARCHIVE_JOURNEY=native_qt_committed_paused_media_position_restores_without_playback \
                timeout --signal=TERM --kill-after=10s 900s \
                cargo run -p xtask --locked --offline -- test archive
        fi
        timeout --signal=TERM --kill-after=5s 30s \
            cargo test -p ferric-browser-storage --locked --offline --lib \
            versioning::media_fingerprint::tests::isolated_secret_service_round_trip_rotation_and_locked_refusal \
            -- --ignored --exact --nocapture
        DBUS_SESSION_BUS_ADDRESS="unix:path=$FERRIC_TEST_SECRET_SERVICE_ROOT/missing-session-bus" \
            timeout --signal=TERM --kill-after=5s 30s \
            cargo test -p ferric-browser-storage --locked --offline --lib \
            versioning::media_fingerprint::tests::isolated_missing_secret_service_fails_closed_without_marker \
            -- --ignored --exact --nocapture
        dbus-run-session --config-file tools/test-isolated-no-activation-session.conf -- \
            bash -c '\''
                set -euo pipefail
                owner_state=$(busctl --user call org.freedesktop.DBus \
                    /org/freedesktop/DBus org.freedesktop.DBus NameHasOwner \
                    s org.freedesktop.secrets)
                if [[ "$owner_state" != "b false" ]]; then
                    printf "private no-activation bus unexpectedly has a Secret Service owner: %s\\n" \
                        "$owner_state" >&2
                    exit 1
                fi
                timeout --signal=TERM --kill-after=5s 30s \
                    cargo test -p ferric-browser-storage --locked --offline --lib \
                    versioning::media_fingerprint::tests::isolated_active_bus_without_service_owner_fails_closed_without_marker \
                    -- --ignored --exact --nocapture
            '\''
    '
