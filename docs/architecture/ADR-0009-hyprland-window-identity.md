# ADR-0009: Optional Hyprland identity and workspace adapter

- Status: accepted for the M4 desktop integration boundary; live compositor
  qualification pending
- Date: 2026-09-18
- Scope: Hyprland window identity, context workspace intent, and Omarchy
  integration

## Decision

Hyprland and Omarchy integration remains an optional adapter outside
`browser-core`. The adapter uses bounded public compositor IPC/JSON facts and
the browser's exact window owner token to identify, focus, and request
workspace intent. Context workspace/accent metadata is presentation intent;
denial, absence, or version drift leaves the browser usable and is surfaced as
an explicit capability result.

The adapter never assigns a browser-owned split view, moves windows by title
heuristics, executes arbitrary configuration text, or makes core correctness
depend on Hyprland being installed.

## Consequences and evidence

Exact activation can be qualified on Hyprland without making Sway, Plasma, or
GNOME claim the same integration promise. Live multi-window routing,
workspace placement, and versioned Omarchy layouts remain platform evidence.

- `crates/browser-engine-qt/src/hyprland.rs`
- `crates/browser-engine-qt/src/lib.rs`
- `docs/architecture/capabilities.md`
- `docs/testing/m0-44-context-visibility-workspace.md`
- `docs/testing/m4-03-hyprland.md`

## Reconsideration trigger

Revisit only if Hyprland's public identity/IPC contract changes or a required
desktop capability cannot be represented without weakening the core boundary.
