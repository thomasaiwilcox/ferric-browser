# C02 — View factory, roles and atomic adoption

## Single allocation seam

Locate the inline `webViewComponent` in the current primary composition and its reuse by secondary windows. Introduce a narrow shared factory only if no equivalent now exists. Extract the production component without changing behaviour first. Preserve stableTabId, prompt identity, profile registry, script runner, native history, download initiator, appearance and all existing callbacks. Do not clone the whole primary window per page.

Required native/view roles are `Spare`, `Claiming`, `Live`, `Closing`, `Retired`; names may match existing types. Assign the initial role during construction, before Component.onCompleted and callback subscription. Do not create an ordinary live view and later hide it. A role plus incarnation/generation accompanies queued completions.

## Spare constraints

A spare has no core TabId or presentation row. It cannot create history/journey/session records, permissions, native prompts, credential channels, downloads, notifications, MPRIS state, switcher results or accessible page entries. It cannot focus or map a native window. Load only a new blank document. Gate every signal's side effects, including profile-level script injection and download routing. Reject unexpected navigation and destroy a contaminated spare; never rehabilitate it into the pool.

Use the existing prepared-transfer fallback only as a reference. That view is a reserve owned by its transfer transaction. Never steal it for the hot pool. No closed or previously navigated user view can become a spare.

## Claim transaction

Resolve destination and settings first. Atomically reserve a ready matching spare; two opens cannot claim the same object. Bind the new page identity, actual profile/partition generation, host and prompt lease. Apply live configuration and interceptor; register the real consumer; attach live browser scripts and the qualified credential machinery; then expose/focus/navigate. If any binding step fails, retire the spare and perform exactly one ordinary allocation. Do not create two user pages through overlapping fallback callbacks.

Old blank-load/script callbacks retain their preparation incarnation and cannot report into the live page. Claim must not leave an extra blank back-history entry. Use native start-page behaviour exactly as on the cold path. Explicit HTTP/new-tab targets load once. Engine-created popups and script-open requests remain on the existing cold request handoff for v1.

## Placement

Internal-mode spare belongs to an inert container in the destination QQuickWindow, outside StackLayout/tabViews. Never make a cross-window scenegraph transfer part of its hot path. Window-mode spare is a separately qualified unmapped host; map only after claim. Failure to benefit from an unmapped host does not justify map/hide flicker.
