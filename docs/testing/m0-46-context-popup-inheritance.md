# M0-46 Context popup inheritance

Date: 2026-09-16  
Status: in-progress  
Requirement: CTX-002

## Implemented slice

Popup windows retain the opener's browser-owned UI/profile owner for
permissions, authentication, userscripts, media requests, and context-menu
handling. They now also capture the opener's context name and label when the
popup is created and expose that identity in the native popup title.

The captured context is presentation and request-routing metadata only. Popup
windows remain ephemeral and are not written into durable context membership;
private popups remain excluded from durable context state.

The tab strip now exposes an accessible context-move picker. It lists only
configured contexts for the active profile, marks destinations without a
different live same-profile window as unavailable, and dispatches the typed
`tab-move --context` path for an eligible destination. The picker does not
silently create a window; users can use `context-enter` first when the target
context has no live owner.

## Verification

Commands: `cargo fmt --all -- --check`; `cargo test -p browser-engine-qt
--locked`; `cargo xtask check`; and
`QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir`.
