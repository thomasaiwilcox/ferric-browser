# Ferric Browser

> **Public pre-alpha:** Ferric is not ready for daily or security-sensitive
> browsing. Expect incomplete accessibility, cross-origin hint limitations,
> platform gaps, storage changes, and unqualified native interactions.

Ferric Browser is a keyboard-first, qutebrowser-inspired browser built with
Rust, Qt 6, and QtWebEngine. It keeps navigation, tabs, modes, commands,
profiles, sessions, and desktop workflows behind typed Rust boundaries while
using Qt for WebEngine and native Linux integration.

Ferric currently targets Linux on native Wayland, with Arch Linux and Omarchy
as the first packaging environments. It deliberately follows qutebrowser's
command names and keyboard ergonomics where practical, then documents Ferric
extensions rather than silently repurposing familiar behavior.

## Build and run

Install Rust 1.85 or newer, Qt 6.11 with QtWebEngine, Qt Widgets, Qt Declarative, and Qt
Wayland, KWindowSystem 6.28 or newer, pkg-config, and a C++ toolchain. Then run:

```sh
cargo build --locked
cargo run --locked -p ferric-browser -- open https://example.org
```

For a local release installation, see the
[installation instructions](packaging/README.md#local-source-installation).

Useful development checks:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask check
cargo xtask test engine
```

Ferric makes a clean pre-alpha data break: legacy browser roots are never read,
moved, modified, or deleted. If an incompatible earlier Ferric root is found,
startup refuses to interpret it. To explicitly erase only Ferric-owned data
and start fresh, run:

```sh
ferric-browser reset-data --confirm
```

The command rejects unsafe root symlinks and leaves root directories and their
parents in place. Schema v3 profile databases receive a backed-up, transactional
start-page upgrade; older data remains subject to the clean break. See
[the data policy](packaging/MIGRATIONS.md).

## Using the browser

Ferric opens a local start page with Top Sites, eight pinned Favourites, and a
searchable keyboard Guide. Press `g s` to return to it and `Ctrl+Shift+Space` to
switch views. A focused site grid supports `1–8`, arrows/hjkl, and Enter; Alt+1–8
continues to select tabs. See [the start-page guide](docs/user/start-page.md).
In the favourite editor, Enter moves from the address to its name, then saves.
Saving shows progress and keeps failed or newer edits available to retry.
The saved name is shared with its bookmark, including when reusing one.

Page search uses Chromium's native text search: type `/words`, press Enter to
keep the highlighted match, and use `n`/`N` to move. Press Enter again to activate
the selected page result, including links with wrapped titles and buttons.
Shift+Enter requests a foreground tab for links. Escape clears the search.
Use `:target-search links` for the full link-list interface.

In `:downloads`, use `j`/`k` or arrows to browse, Page Up/Down to read a page,
and Home/End or `g`/`G` to jump. Enter expands the selected file's full name,
location and details. Tab enters its actions; Shift+Tab from the first action
returns to browsing. Escape closes the panel.

Settings keeps unapplied text with its window and profile while you filter,
refresh or reopen the panel. Enter or Apply submits it; Discard edit restores
the current configured value. A newer edit remains intact when an earlier
submission finishes.
Settings shows saving progress and the final result. If saving fails, the
edit stays available to retry; Show details explains the failure. Reset also
keeps an unfinished edit until the reset succeeds.
Saved settings are shared by regular profiles using the same setup. Choose
Session only to limit a change to the current window and session.

Press `i` for transient text entry: ordinary Enter reaches the webpage and
returns to Normal mode, so `j` and `k` immediately navigate again. Shift+Enter
keeps composing. Press `I` (Shift+i) for locked editing with repeated Enter,
such as document paragraphs. Escape exits either insert mode. For webpage
shortcuts, use Ctrl+v for Pass-through or Ctrl+Shift+v / `; v` to forward one
key; Ctrl+Escape returns to Normal. See [Webpage shortcuts](docs/user/webpage-shortcuts.md),
the [text-entry guide](docs/user/insert-modes.md), and
[keyboard bindings](docs/user/bindings.md).

Press `Ctrl+Space` to search browser items, or `T` to search tabs. Use arrows or
`Ctrl+n`/`Ctrl+p` to select results, Page Up/Down to move by a visible page, and
`Ctrl+Home`/`Ctrl+End` for the first/last result. Home, End, and their Shift
variants edit the search text. Enter opens the selection; Escape closes it.
Tab enters the selected result's actions after any search status controls;
Shift+Tab from its first action returns to the preceding search control.
Current results remain usable while background searches load more items.
In the command line, Tab/Shift+Tab or `Ctrl+n`/`Ctrl+p` cycle completions.
At the end of the input, Right inserts the selected suggestion for editing;
Space inserts it and starts the next word. Enter runs the command. Ctrl+z
undoes an insertion while preserving what you typed before it.
History and bookmark lists also page by their rendered rows; expanded details
and controls taller than the viewport scroll before navigation moves past them.
Tab enters a library entry's controls; Shift+Tab from its first control returns
to browsing the selected entry.
From list browsing, Shift+Tab reaches the controls above the results. Those
controls scroll with the results, leaving room to read at large text sizes.
For journey entries, Enter and Reopen use the entry's selected destination:
Current tab, New tab or New window. That choice survives scrolling.
In Relationships, Page Up/Down scroll by a visible page; Home/End or `g`/`G`
jump to the top/bottom. Tab into the map to pan with arrows or navigate its
contents with the same paging and end keys.
Unfinished edits survive scrolling and changing library pages. Save submits the
change; Cancel edit discards the draft.
In Profiles, Enter moves from the new profile name to its display name, then
creates the profile. Enter also saves a profile rename or a named session.
Cancelling or completing a rename returns focus to an available control.

Press `;g` from Normal mode to enter Grid mode, select the numbered region that
contains the target, and repeat until the crosshair is positioned correctly.
`Enter` left-clicks the crosshair. See the
[Grid navigation guide](docs/user/grid-navigation.md) for right-click,
middle-click, hover, correction, cancellation, and current qualification
details.

The complete generated references cover
[keyboard bindings](docs/user/bindings.md), [commands](docs/user/commands.md),
[configuration](docs/user/configuration.md), and
[typed actions](docs/user/actions.md).

## Architecture

Input follows one direction:

```text
QML / Qt callback
  → typed bridge request
  → ferric-browser-runtime
  → core reducer and subsystem orchestration
  → typed runtime effects
  → Qt, WebEngine, desktop, or storage adapter
```

`ferric-browser-core` owns application invariants and exposes read-only state;
`ferric-browser-runtime` owns Qt-independent orchestration; the Qt crate owns
platform integration and effect execution. Command, action, binding, and
setting registries generate the user references under `docs/user/`.

See [architecture](docs/ARCHITECTURE.md), [security model](SECURITY.md),
[testing guide](docs/TESTING.md), [roadmap](docs/ROADMAP.md), and the full
[development specification](docs/DEVELOPMENT_SPEC.md).

## Security and support

QtWebEngine supplies the rendering engine, so Ferric's browser security also
depends on timely Qt/Chromium packages from the distribution. Use GitHub's
private vulnerability reporting rather than a public issue for suspected
security defects. See [SECURITY.md](SECURITY.md) and [support](docs/SUPPORT.md).

Ferric Browser is licensed under
[GPL-3.0-or-later](LICENSE). It is an independent project inspired by
qutebrowser; see [provenance](docs/PROVENANCE.md).
