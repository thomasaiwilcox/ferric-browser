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
documentation, the Grid navigation and text-entry guides, dependency/license
inventory, migration and known-issue notes, and support, security, provenance,
release-policy, and contributor documentation, including the current
release-identity decision record. The optional, community-maintained Omarchy
integration recipe
is in `packaging/omarchy/`; it is built from the same release archive and only
installs package-owned resources plus public-CLI wrappers. Clean Arch
build/install qualification, icon-cache refresh, and live handler/launcher
qualification remain release work.

## Local source installation

For a per-user development installation, build from the repository root and
copy the release executable, desktop entry, icon and user guides:

```sh
cargo build --release --locked -p ferric-browser
install -Dm755 target/release/ferric-browser "$HOME/.local/bin/ferric-browser.new"
mv -f "$HOME/.local/bin/ferric-browser.new" "$HOME/.local/bin/ferric-browser"
install -Dm644 packaging/io.github.ferricbrowser.FerricBrowser.desktop \
  "$HOME/.local/share/applications/io.github.ferricbrowser.FerricBrowser.desktop"
install -Dm644 packaging/io.github.ferricbrowser.FerricBrowser.svg \
  "$HOME/.local/share/icons/hicolor/scalable/apps/io.github.ferricbrowser.FerricBrowser.svg"
for guide in docs/user/*.md; do
  install -Dm644 "$guide" "$HOME/.local/share/doc/ferric-browser/${guide##*/}"
done
install -Dm644 packaging/CHANGELOG.md "$HOME/.local/share/doc/ferric-browser/CHANGELOG.md"
install -Dm644 packaging/KNOWN-ISSUES.md "$HOME/.local/share/doc/ferric-browser/KNOWN-ISSUES.md"
update-desktop-database "$HOME/.local/share/applications"
gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor"
ferric-browser --help
ferric-browser --temp-basedir diagnostics --format json
```

Ensure `~/.local/bin` is on `PATH`. Qt/QML resources are embedded in the
executable; the matching system Qt libraries and QtWebEngine helper/resources
are still required. The atomic executable replacement also works while an
older Ferric process is running. Restart Ferric to use the new build. This
installation does not change default-browser ownership or browser data.

Use the Arch package for package-manager ownership and system-wide installation.
