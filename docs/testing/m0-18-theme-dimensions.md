# M0-18 theme and dimensions evidence

Status: in progress, implementation slice recorded 2026-09-16.

## Scope

The native chrome now consumes the already validated `[ui]` settings from the
active config. `font_family` and `font_size_pt` are applied to the
`ApplicationWindow` font, including secondary and popup windows through the
shared root font. Status, command/search, toolbar, and completion-row heights
are derived from `FontMetrics` and logical padding rather than fixed physical
pixel values. The chrome exposes semantic background, surface, panel, text,
border, accent, warning, error, and success tokens; the surfaces remain fully
opaque.

The default presentation follows UI-001's content-first layout. There is no
persistent navigation toolbar: normal browsing shows only the compact tab line
when the configured tab policy calls for it and the bottom status line. The
status line gives the current mode a stable left edge, prioritizes the safe
committed URL, and keeps profile/context plus compact activity indicators
visible. Command and search modes replace that line with flat, monospace input
instead of adding another row of controls. Tabs are edge-aligned, numbered,
keyboard-oriented, and retain small mouse close/new-tab targets. Secondary
browser windows also omit the conventional address/back/zoom toolbar.

The browser's content views continue to use WebEngine settings. The default
`content.force_dark` value remains false, so the desktop theme does not force
darkening on websites. No optional chrome animations are currently defined;
the validated `reduced_motion = on` setting is retained as an explicit state
for future motion-bearing surfaces. In `system` mode, a bounded worker probes
the GNOME-compatible animation preference; a second worker probes
`text-scaling-factor` and applies a validated 0.5–3.0 scale to the native
chrome font, with conservative fallbacks when desktop settings are unavailable.

## Verification

```text
cargo xtask check
cargo test --workspace --locked --offline
```

The workspace check is the build-time QML/resource validation. Rendered-pixel
contrast review and imported theme provider qualification remain manual
follow-up work; operating-system motion and font-scale probes are covered by
bounded worker/parser tests and offscreen adapter smoke.
