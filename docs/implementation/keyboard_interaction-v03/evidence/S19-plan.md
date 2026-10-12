# S19 experiment and implementation plan

## Gate question

Can one request through `FerricPagePointerAdapter` deliver Qt's documented
double-click event sequence into the owning Qt Quick/WebEngine view and produce
one DOM double-click with the expected pointer/mouse/click counters? The
feature must remain an explicit local Grid action, never a DOM-click fallback,
never retry an uncertain request, and never become a default binding.

## Intended patch outline

1. Extend the existing adapter only for a single explicit double-click request;
   retain its current target/surface checks, request deduplication, and one
   terminal acknowledgement. Mirror Qt's documented
   Press-Release-Press-DoubleClick-Release ordering and check the captured view
   and window before subsequent sends.
2. Add a native-insert WebEngine fixture that calls the production adapter once
   and records trusted DOM pointer/mouse/click/dblclick events, event details,
   and bounded navigation counters. Verify duplicate request IDs do not
   redeliver and a failed/uncertain transaction is terminal.
3. Only if the Qt event experiment passes, offer the action through the existing
   local Grid owner path with a new in-Grid chord; preserve Enter,
   Shift+Enter, Ctrl+Enter and all default modes. Exercise the adapter boundary
   directly and record separately if a composed Grid-to-Rust-to-QML handoff is
   unavailable in the current harness.
4. Keep the action unavailable until required native Wayland qualification
   passes. If Qt cannot produce the expected sequence or adapter ownership
   checks cannot bound the transaction, add no production action and record
   the evidence-based no-go.

## Test mapping

- T046/native-qt-offscreen: production adapter request, observed DOM order and
  event cardinality, one acknowledgement, duplicate-ID rejection, and no DOM
  fallback.
- T046/native-wayland: physical native input qualification; run only if the
  existing Wayland harness supports the required input path.
- T010/pure-unit and native-qt-offscreen: retain the existing permit/owner
  tests and exercise the explicit Grid chord through the production UI path if
  the gated action is implemented.
- T014/native-qt-offscreen: record event trust and transient user-activation
  facts without inferring permission approval; qualify actual APIs separately
  where the fixture can exercise them.
- T045: full Rust, WebEngine, Qt offscreen and required project gates after any
  production changes. T055/pure-unit: command registry/help and docs parity.

The native Wayland result remains separate from offscreen Qt evidence. No live
site or account changes are part of this slice.

## Outcome

Qt 6.11.2 / QtWebEngine 6.11.2 offscreen proof passed through one enabled
production-adapter request. The DOM trace was two trusted click sequences
followed by one trusted `dblclick`, with `detail` values 1 and 2 on the clicks
and 2 on `dblclick`; transient user activation was active for each observed
event. URL stayed unchanged, one request was acknowledged, a duplicate request
ID was rejected, and the adapter's default-disabled path emitted no events.
The full native suite initially exposed Qt's window-wide click counter sharing
the same coordinates between neighboring fixtures; moving the double-click
target outside that distance made the full-suite sequence deterministic.

The Grid command/binding, Rust terminal-request handling, remote-source
rejection, QML unavailable feedback and command docs are implemented. The
BrowserUi local Grid-to-Rust-to-QML-to-adapter handoff has no composed native
fixture in this repository, so T010/native-qt-offscreen remains not-run;
existing pure-unit permit and source-boundary tests pass. Specific
fullscreen/popup/clipboard/chooser outcomes were not exercised, so T014 remains
not-run even though trust and activation facts were observed in the fixture.
The Wayland harness passed its bounded Weston startup smoke but could not send
native input because its compositor lacks the virtual-keyboard protocol.
Accordingly the runtime `doubleClickEnabled` property remains hardcoded false;
no desktop or live-site qualification is claimed.
