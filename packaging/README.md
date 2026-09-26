# Desktop integration assets

The stable desktop identity is io.github.ferricbrowser.FerricBrowser, matching
the Qt QML module identity used by the application. Install the desktop entry
as:

    /usr/share/applications/io.github.ferricbrowser.FerricBrowser.desktop

and the icon under the matching hicolor application-icon name:

    /usr/share/icons/hicolor/scalable/apps/io.github.ferricbrowser.FerricBrowser.svg

The desktop entry registers HTTP(S), HTML/XHTML, and PDF associations without
claiming default-browser ownership. Its explicit actions open a new normal or
private window through the typed CLI target.

Default-browser ownership is an explicit user operation: `ferric-browser
default-browser status` reports the current XDG handler and `ferric-browser
default-browser set` opts in after setting and verifying the stable desktop
entry. Installation and first launch never change the handler.

`PKGBUILD` is the primary system-Qt recipe. It expects a release source archive
named `ferric-browser-${pkgver}.tar.gz`; release automation must replace its
placeholder checksum before publishing. `dependencies.toml` records the Qt,
QML, portal, PipeWire, and optional feature inputs without claiming that every
optional service is present.

The recipe installs the binary, desktop entry, scalable icon, generated user
documentation, the Grid navigation guide, dependency/license inventory,
migration and known-issue notes, and support, security, provenance,
release-policy, and contributor documentation, including the current
release-identity decision record. The optional, community-maintained Omarchy
integration recipe
is in `packaging/omarchy/`; it is built from the same release archive and only
installs package-owned resources plus public-CLI wrappers. Clean Arch
build/install qualification, icon-cache refresh, and live handler/launcher
qualification remain release work.
