# M0-21 Arch packaging evidence

Status: in progress, implementation slice recorded 2026-09-20.

## Scope

`packaging/PKGBUILD` is a system-Qt Arch recipe for the Ferric Browser release
archive. It builds with the locked Cargo dependency graph, runs the workspace
tests and desktop-file validation, then installs:

- `/usr/bin/ferric-browser`;
- the stable desktop entry under `/usr/share/applications`;
- the scalable hicolor icon; and
- the README, specification, generated user documentation, dependency and
  license inventories, migration/known-issue notes, and support/security/
  provenance/release-policy documentation under package docs.

`packaging/dependencies.toml` separates mandatory Qt/QML/Wayland/CA trust
inputs from optional portal backends, PipeWire session components, spellcheck
dictionaries, and external tools. The browser does not treat an absent portal
or media service as a compile failure; diagnostics report the capability.
The build dependency set also includes `desktop-file-utils`, because the
recipe validates its desktop entry during the package check phase.
The recipe's check-time dependencies also include `dbus` and `weston`, which
provide `dbus-run-session` and the disposable nested compositor used by the
native Wayland and cached-list blocking qualification harnesses; they are not
runtime browser dependencies. The package `check()` phase runs
`cargo xtask package artifacts`, `cargo xtask test wayland`, and
`cargo xtask test blocking`, so release inputs and these check-time components
are exercised during a source-package check rather than merely declared.
The default `xdg-downloads` policy reads the bounded
`$XDG_CONFIG_HOME/user-dirs.dirs` entry (with `$HOME/Downloads` fallback),
without shell evaluation or directory creation.

## Verification

```text
(cargo xtask package arch)
(cargo xtask package artifacts)
(cd packaging && makepkg --printsrcinfo -p PKGBUILD)
desktop-file-validate packaging/io.github.ferricbrowser.FerricBrowser.desktop
```

The `xtask` package check also verifies the locked release build command and
the standard installed binary, desktop-file, icon, and documentation paths,
plus the system-dynamic and optional-component dependency boundary. Both the
primary and optional Omarchy recipes are checked against bounded installed-file
manifests, including duplicate, unsafe, omitted, or empty path failures.

The recipe intentionally expects a release source archive and leaves its
checksum as a publication-time placeholder. This workspace is not a clean
Arch chroot, so installed-package qualification, icon-cache behavior, live
desktop-handler launch, and the optional Omarchy integration package remain
release work.

On 2026-09-19, the initial disposable runs exposed two packaging issues: the
`/tmp` tmpfs was too small for the Qt archive, and makepkg's default LTO mode
produced unresolved CXX-Qt bridge symbols at the Rust final link. The recipe
now disables makepkg LTO for this mixed Rust/C++ package and adds a trailing
`--no-as-needed` while preserving Arch's other linker hardening flags. The
isolated release build then reached package assembly; that pass exposed a
third issue where `package()` referenced `SUPPORT.md` at the repository root
even though the checked-in file is `docs/SUPPORT.md`. The recipe now installs
the correct path. On 2026-09-20, a fakeroot repackage using the existing
release binary passed package issue checks and produced
`ferric-browser-0.1.0-1-x86_64.pkg.tar.zst` plus its debug package. Inspection
confirmed the executable, desktop entry, icon, and all declared documentation
files, including `SUPPORT.md`. Neither artifact was installed or published.
Weston was present as a check dependency and was not the cause of the earlier
failures.

The same corrected package was unpacked into an isolated temporary root on
2026-09-20, with the build tree absent from the runtime path. The installed
`usr/bin/ferric-browser`, desktop entry, scalable icon, and documentation manifest
were present; the installed binary's `--help` and JSON diagnostics commands
also completed successfully. This validates the package layout and CLI/runtime
resource boundary without installing into the host system. Dependency,
desktop-handler, upgrade, and live graphical qualification remain release
gates.

The release profile now enables DWARF level 2 and the Arch recipe enables its
`debug` option. A 2026-09-20 optimized release rebuild contained `.debug_info`,
`.debug_line`, and `.debug_str`; a subsequent fakeroot package assembly
produced a 63 MiB split debug package whose `usr/bin/ferric-browser.debug`
contains the same DWARF sections. This satisfies the available packaging
debug-symbol requirement while keeping the installed runtime package stripped.

On 2026-09-20, two locally built revisions (`pkgrel` 1 and 2) were installed
and upgraded inside a mapped-root temporary pacman database. The database was
seeded from a read-only copy of the host's installed-package records, so normal
dependency resolution ran without touching the host database or filesystem.
Pacman completed both transactions and reported `ferric-browser 0.1.0-2`; the
executable, desktop entry, and `SUPPORT.md` were still present afterward. This
is isolated package transaction evidence, not a substitute for a clean Arch
image installation from repository metadata.

The distribution decision is recorded in
`docs/architecture/ADR-0003-system-qt-distribution.md`: the supported V1
baseline is dynamically linked system Qt, with optional portals, PipeWire,
dictionaries, external tools, and Omarchy resources kept outside the mandatory
browser package.

The checked-in `packaging/release-artifacts.toml` manifest enumerates the
Cargo lockfile and build manifest, package/dependency/engine inputs,
compatibility and capability inputs, a license inventory, migration notes,
known issues, changelog, primary and Omarchy installed-file manifests, requirements traceability,
and release documentation. `cargo xtask package artifacts` validates that each
bounded manifest path exists. Source archives, generated checksums, and
detached signatures remain publication outputs.
