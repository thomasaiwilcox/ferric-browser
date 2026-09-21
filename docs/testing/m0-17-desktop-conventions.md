# Desktop conventions evidence

Date: 2026-09-16  
Status: in-progress

## Scope

The repository now contains a stable freedesktop desktop entry and scalable
hicolor icon using the application identity io.github.ferricbrowser.FerricBrowser.
The Qt bootstrap sets the same desktop-file basename and display name before
creating QML windows, so native Wayland app identity does not depend on the
window title.
The entry declares HTTP(S), HTML/XHTML, and PDF associations, uses the typed
open target for normal/private window actions, and does not silently claim
default-browser ownership.

Default-browser ownership is opt-in through the CLI only:

    ferric-browser default-browser status
    ferric-browser default-browser set

`set` invokes `xdg-settings` without a shell and verifies the resulting
desktop entry. Installation and normal GUI startup do not claim ownership.

Desktop `%U` launches preserve multiple URL-like arguments as separate browser
tabs (including Unicode and `file:` URLs); ordinary multi-word `open` input
continues to be treated as one search/navigation input.

## Verification

    desktop-file-validate packaging/io.github.ferricbrowser.FerricBrowser.desktop

The validator is optional on minimal development images. Arch installation,
icon-cache refresh, package dependency qualification, and live handler
launches remain open.
