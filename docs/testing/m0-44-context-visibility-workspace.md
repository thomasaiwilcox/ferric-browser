# M0-44 Context visibility and workspace intent

Date: 2026-09-16  
Status: in-progress  
Requirements: CTX-006, CTX-007

## Implemented slice

Every configured browser UI now publishes the active context name, display
label, workspace selector, and validated semantic accent separately from its
profile. Primary and secondary status bars show the profile and context as
distinct values; the context label uses the configured accent when valid.
Private windows do not acquire durable context metadata.

Changing context membership updates these properties after the reducer state
change. If the context has a workspace selector, the owning UI requests that
workspace through the exact window token and leaves the window usable when the
Hyprland adapter reports disabled, denied, missing, or ambiguous support.
Context metadata remains browser chrome state and is never sent to page
content.

## Verification

Commands: cargo fmt --all -- --check; cargo test -p browser-engine-qt --locked;
cargo xtask check; QT_QPA_PLATFORM=wayland timeout 10s
target/debug/rustbrowser --temp-basedir.

Context entry now prefers an existing live member in another browser window,
emitting the exact stable window-focus action before falling back to safe saved
descriptors or a new window. Remaining qualification is interactive
multi-window workspace routing and screen-reader verification of the distinct
profile/context labels.
