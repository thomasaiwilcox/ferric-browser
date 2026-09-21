# M0-99: shutdown page-state safety

Requirement: STATE-009

Primary, secondary-window, and popup close requests now inspect active page views before
calling the reducer shutdown boundary. A bounded page-world probe checks form
controls, contenteditable state, `beforeunload`, and bounded select options;
dirty or unknown results show a browser-owned prompt with explicit “Keep open”
and “Close anyway” choices. The probe never returns page content or form
values to Rust, logs, or persistence.

Download cancellation re-enters the same page-state check, so cancelling a
download cannot bypass the unsaved-state decision. Closing a discarded or
otherwise inactive view skips the probe because its page state is not live.
Probe generations invalidate late callbacks after cancellation or approval. Every window
type has a 2.5-second deadline, after which the same explicit choice is shown instead of
leaving an unresponsive page able to block window closure indefinitely.

The application build and native Wayland startup smoke cover QML loading. The
remaining qualification is process-wide shutdown coordination across several
secondary/popup windows, interactive unsaved-form fixtures, and durable flush
ordering during forced exit.
