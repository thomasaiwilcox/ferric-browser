# M4-05 Omarchy launcher/shell integration

Status: in-progress. This slice keeps Omarchy optional and uses only the
browser's public CLI/IPC surface.

## Delivered

- `rustbrowser query switcher --scope tabs --format json` returns normal-profile
  tab rows with stable tab IDs and sanitized display fields.
- `rustbrowser activate tab TAB_ID` sends `switcher.activate` to the running
  instance and focuses the exact tab represented by that ID.
- Private tabs are excluded from the default query and rejected by the public
  activation method.
- The primary desktop entry retains explicit normal/private window actions.
- `packaging/omarchy/` contains an optional community package with a declarative
  launcher contract, public-CLI wrappers, a data-only theme template, a
  version-qualified Hyprland 0.51–0.54 and 0.55+ examples, and a read-only
  doctor.

The package does not depend on Walker, Waybar, or a particular shell. It does
not inspect browser databases, scrape titles, source Omarchy shell files, or
write user configuration. Its wrappers pass stable IDs through the public
interface with direct argv.

## Verification

```text
cargo fmt --all -- --check
cargo test -p rustbrowser --locked --offline
cargo test -p browser-engine-qt --locked --offline
cargo build -p rustbrowser --locked
desktop-file-validate packaging/io.github.rustbrowser.RustBrowser.desktop
sh -n packaging/omarchy/usr/bin/rustbrowser-omarchy-doctor
sh -n packaging/omarchy/usr/bin/rustbrowser-omarchy-query-tabs
sh -n packaging/omarchy/usr/bin/rustbrowser-omarchy-activate-tab
```

Clean Arch package build/install, a real launcher selection, and live
Hyprland/Omarchy qualification remain release work.

On 2026-09-20, the optional recipe was assembled from the release archive,
all three installed wrapper scripts passed `sh -n`, and revisions `0.1.0-1`
and `0.1.0-2` were installed/upgraded in an isolated pacman database seeded
from a copy of the host package records. Normal dependency resolution accepted
the `rustbrowser` dependency, and the final package contained every declared
wrapper, launcher contract, theme template, both Hyprland examples, and README
path.
No host configuration or user-owned file was modified. This closes the local
package transaction check; live launcher selection, Hyprland workspace routing,
and checksum/publication review remain open.

The source wrapper files were also corrected to executable mode (`0755`) after
a direct live smoke caught that source-tree invocation otherwise failed even
though the package recipe installed them correctly. In the disposable native
Wayland session on 2026-09-20, `rustbrowser-omarchy-query-tabs` returned the
real normal-profile tab, `rustbrowser-omarchy-activate-tab tabid-3` returned a
typed `status: accepted` response, and an invalid `bad/id` value was rejected
locally with exit 2. The doctor reported four available capabilities and one
unavailable Omarchy-palette capability, with the promised no-files-changed
summary. The temporary process and all temporary XDG roots were removed after
the run; no user configuration was touched.
