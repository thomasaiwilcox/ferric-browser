# M1-09 semantic theme evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: THEME-001.
- Files: `crates/browser-config/src/lib.rs` and
  `crates/browser-engine-qt/qml/Main.qml`.
- Observable result: browser-owned chrome consumes semantic theme tokens with
  deterministic fallbacks; page content is not styled by the browser theme.

## Evidence

- `ThemePalette` exposes `background`, `surface`, `foreground`, `muted`,
  `accent`, `border`, `selection_background`, `selection_foreground`, `error`,
  `warning`, `success`, `private`, and `mode_insert`.
- Builtin fallback values use the specified Catppuccin-style background,
  foreground, accent, error, warning, and success colors; remaining tokens are
  derived deterministically. Legacy provider names remain aliases.
- Theme files accept six- or eight-digit hex colors. QML clamps translucent
  chrome surfaces to readable opacity and keeps security prompt surfaces
  opaque; mode and private-window state retain explicit labels and colors.
- Browser-owned panels, overlays, status bars, borders, selections, and
  decision indicators use semantic bindings. No page CSS or document content
  is modified by theme loading.
- `cargo test -p browser-config -p browser-engine-qt --locked`, strict
  workspace Clippy, `cargo xtask check`, and native Wayland startup smoke pass.

## Limitations

Full theme-provider qualification, richer semantic contrast checks, and the
remaining legacy hardcoded colors in specialized diagnostic states are future
polish slices. Theme changes remain chrome-only by default.
