#!/usr/bin/env bash
# Narrow release hygiene gate; this alone never qualifies a production engine.
set -euo pipefail

if [[ $# != 1 ]]; then
    printf 'usage: check-no-probe-api.sh ENGINE_INSTALL_PREFIX\n' >&2
    exit 2
fi

backend_prefix=$(realpath -- "$1") || exit 2
for backend_artifact in \
    "$backend_prefix/lib/libQt6WebEngineCore.so.6" \
    "$backend_prefix/lib/libQt6WebEngineQuick.so.6" \
    "$backend_prefix/lib/libQt6WebEngineWidgets.so.6" \
    "$backend_prefix/lib/libQt6WebEngineQuickDelegatesQml.so.6" \
    "$backend_prefix/lib/qt6/QtWebEngineProcess"; do
    if [[ ! -f $backend_artifact ]]; then
        printf 'missing WebEngine artifact: %s\n' "$backend_artifact" >&2
        exit 1
    fi
    backend_symbols=$(nm -D -C -- "$backend_artifact") || exit 1
    if rg -q 'ferric[^[:space:]]*ForProbe|FERRIC_SPIKE_' <<<"$backend_symbols" \
            || LC_ALL=C rg -a -q 'ferric[A-Za-z0-9_]*ForProbe|FERRIC_SPIKE_' \
                "$backend_artifact"; then
        printf 'research-only Ferric probe marker remains in %s\n' "$backend_artifact" >&2
        exit 1
    fi
done

printf 'No Ferric probe symbol or binary marker found in the WebEngine libraries or process; all other release gates remain required.\n'
