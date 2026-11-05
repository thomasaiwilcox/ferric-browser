#!/usr/bin/env bash
# Select the qualified SDK for one command; never change host packages.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
workspace_dir=$(dirname -- "$repo_dir")
qt_prefix=${FERRIC_QT_PREFIX:-${XDG_DATA_HOME:-$HOME/.local/share}/ferric-browser-runtime/qt/6.11.2/gcc_64}
wayland_prefix=${FERRIC_QTWAYLAND_PREFIX:-$workspace_dir/builds/qtwayland-qt6112-F01b/build}
if [[ ! -x "$qt_prefix/bin/qmake6" ]]; then
    echo 'Set FERRIC_QT_PREFIX to the complete qualified Qt 6.11.2 SDK, including matching Gui private headers.' >&2
    exit 1
fi
[[ $("$qt_prefix/bin/qmake6" -query QT_VERSION) == 6.11.2 ]] || { echo 'Qt 6.11.2 is required.' >&2; exit 1; }
export PATH="$qt_prefix/bin:$PATH"
command -v wayland-scanner >/dev/null
pkg-config --exists wayland-client wayland-scanner
export QMAKE="$qt_prefix/bin/qmake6"
export PKG_CONFIG_PATH="$qt_prefix/lib/pkgconfig"
export CMAKE_PREFIX_PATH="$wayland_prefix:$qt_prefix"
if [[ -f "$wayland_prefix/lib/cmake/Qt6WaylandCompositor/Qt6WaylandCompositorConfig.cmake" ]]; then
    export QT_ADDITIONAL_PACKAGES_PREFIX_PATH="$wayland_prefix"
fi
export LD_LIBRARY_PATH="$qt_prefix/lib:$wayland_prefix/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export QT_PLUGIN_PATH="$qt_prefix/plugins"
export QML_IMPORT_PATH="$qt_prefix/qml"
export QTWEBENGINEPROCESS_PATH="$qt_prefix/libexec/QtWebEngineProcess"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$repo_dir/target/qt611}
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
exec "$@"
