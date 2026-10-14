# S02 patch outline and test mapping

## Intended changes

1. Extend the existing `scrollTargetRuntime()` primitives in `BrowserScripts.js` with a bounded composed-parent walk (`assignedSlot` before parent/host), exact open-shadow hit descent, and safe failure when ancestry exceeds its bound. Preserve the root/body overflow propagation and keep focus, pointer, and scroll eligibility policies distinct.
2. Make hint collection, freshness, focus, click, and scroll-target selection use that shared ancestry/visibility machinery. Retain the exact candidate element and owner document in the existing hint map; reject adoption/replacement, disabled-fieldset, ancestor opacity, and covering siblings. Project frame candidates through the iframe content origin and clip to its content box. Remove value-based labels for controls and secret buffers.
3. Tighten the bounded Rust candidate decoder: explicit outer/geometry key allowlists, optional boolean `visible`, finite positive geometry, and a UTF-8 text byte bound. Add pure unit cases for rejected authority fields and malformed bounds.
4. Guard QML async final-action callbacks with the captured WebView and hint epoch, and confirm HintSession selection cardinality before adding any latch. Test final deep focus, one click, frame/document identity, clipping, labels, and existing viewport behavior.

## Test-to-production mapping

- T002/T008: `tst_webapp_interaction.qml` → shared hit test, exact retained-record final focus, deep active element.
- T003/T005/T007: same fixture → assigned-slot composed ancestry, bounded walk, `:disabled`, ancestor opacity/inert, exact hit identity, adoption/cover rejection.
- T004: same fixture → frame content-origin projection, content-box clipping, retained owner-document freshness.
- T006: `tst_hint_geometry.qml` and existing QML suite → unchanged root/body overflow semantics.
- T009: `hint_payload.rs` unit tests plus fixture label assertions → key allowlists, finite geometry, UTF-8 byte bound, no control value.
- T020: QML callback lifecycle and retained map behavior; mark unqualified if a real owner/handoff proof remains outside S02.
- T045: native insert and Qt QML legacy suites after focused cases pass.

Only the four planned implementation anchors, their focused existing tests, progress/evidence files, and the S02 record will be edited. No new registry or transport is planned.
