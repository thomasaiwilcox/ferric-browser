# Provenance and branding

Ferric Browser is an independent implementation inspired by documented keyboard
browser behavior. It is not an official qutebrowser, Google, Hyprland, or
Omarchy product. The project must not imply endorsement or affiliation without
authorization.

The implementation may learn from public behavior and documentation without
copying qutebrowser source or branding. Any reused source snippet, asset, font,
icon, dictionary, blocklist, script, or generated data must retain its notice,
license, and source reference in the release inventory. QtWebEngine and
Chromium notices remain distribution obligations separate from the Ferric Browser
application license.

`io.github.ferricbrowser.FerricBrowser` and the `ferric-browser` executable are the
current working identifiers, not a claim that the final project namespace,
repository, or trademark has been cleared. Before public V1, maintainers must
check name/namespace availability and record the owned identifiers.

The separate `helpers/ferric-phone-auth` workspace pins
[`linux-credentials/libwebauthn`](https://github.com/linux-credentials/libwebauthn/tree/9c0e5f78ac445d35dca05170efbb0afefc5a89ae)
at `9c0e5f78ac445d35dca05170efbb0afefc5a89ae`. That upstream revision is
LGPL-2.1-or-later and carries its license text as `COPYING`; static helper
distribution must preserve the corresponding source and relinking obligations.
The dependency is not part of Ferric's root Cargo workspace.
