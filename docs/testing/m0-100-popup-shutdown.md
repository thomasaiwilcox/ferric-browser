# M0-100: popup shutdown ownership

Requirements: STATE-009, AUTH-001, and JOURNEY-001

Approved popup windows now intercept their native close request and use the
opener-owned, token-bound `close_popup_tab` operation. Rust resolves the exact
popup journey target, transitions it through `CloseTab` and `ViewClosed`, and
removes the popup tab/token bookkeeping exactly once; it never requests
shutdown of the opener's BrowserUi.

Popup close also has browser-owned active-work and page-state prompts. The
page-state check uses the same bounded, value-free form/editor probe as normal
window shutdown, and late callbacks are invalidated by a per-popup generation.
Unexpected popup destruction performs the same token-bound close acknowledgement
before releasing journey metadata.

Compilation and static Qt tests cover the typed bridge and QML path. Interactive
popup creation, download ownership, and multiwindow shutdown ordering remain
native qualification work.
