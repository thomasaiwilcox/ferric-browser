# Settings save feedback — 2026-10-21

## Reproduction

The Settings panel reported "Change requested" when a background write was
accepted. A later disk failure appeared only in transient browser status;
the panel continued to imply success. Closing and reopening lost progress.
Reset discarded an unfinished edit as soon as its request was accepted,
including when the subsequent write failed. Four focused regressions failed
before the repair.

## Repair

Repeated native runs exposed a misleading scope assumption: persistent runtime
settings use one file per state root, including across named regular profiles.
The panel and scope control now disclose that shared scope. Filtering reserves
space for matching settings rather than repeating the explanatory paragraph. Session-only
changes remain in the current window. Receipts name the actual scope rather
than promising that a saved value affects only the active named profile.

The backend publishes a request ID, state, message and failure detail for
Settings operations. Pending requests follow selector validation and durable
configuration publication to their actual result. Validation rejection,
timeout, replacement, cancellation and writer failure settle their matching
receipt. Synchronous validation and session-only changes publish a terminal
result too. Profile activation clears the receipt, and late completions must
match its ID before publishing.
An applied write whose final directory sync failed keeps the existing crash
durability warning in the panel and exposes its detail.
Saved choices waiting for a configuration reload or browser restart remain
visible in their controls. The receipt identifies the required boundary;
the backend's effective configuration stays unchanged until that boundary.

The window's Settings workspace owns its submitted request. It restores
feedback when reopened and shows pending progress, success or a failure with
plain-text, selectable details. Apply, Reset, toggles and choices wait for the
current request to finish; text remains editable. Held or fresh Return cannot
enqueue another save while one is pending. Changing where the next edit is
saved or receiving an unrelated userscript result preserves the current save
feedback and its original scope.

Successful Apply clears only the text it submitted. Successful Reset clears
only its original draft. Failure leaves the edit available to retry. Newer text
survives either result, including when it happens to equal the old effective
value. Discard during a pending request clears local text while retaining the
submission identity needed to protect a subsequent edit.

## Verification

The focused Settings workspace suite passed **55 checks**. The final full
source QML suite passed **922 checks**, with no failures or skips in 169.907 seconds.
Its log contains no runtime type, reference, assignment, binding-loop or
unknown-signal-handler errors. Lint reports 40 warnings and no error/fatal
diagnostics. The backend and scope changes passed a release build in **8m 17s**.

Production qualification passed **154 native checks**: 14 navigation/history,
40 offscreen keyboard, 12 Wayland editing, 22 compositor-seat keyboard, three
directory-sync warning checks, four backend writer/validation checks,
38 real-download checks and 21 manager/library/window/popup/session/startup
checks. All passed with no failures or skips. Eight consecutive additional
Wayland Settings runs passed **32 checks**, including observed reload-busy
rejections followed by fresh keyboard retries.

Visual review then found the scope paragraph crowded the first filtered
result. The explanation remains visible before filtering and in the scope
control's accessible description; saved receipts also name the shared scope.
Filtering now reserves that space for results. A regression requires the
first matching setting's full name to fit before entering its editor.
The final layout release build passed in **8m 41s**. Its focused Settings
render run passed 55 checks. The relinked compiled journeys passed **four
offscreen checks** in 7.510 seconds and **four owned-seat Wayland checks**
in 7.823 seconds. Their eighteen-resource guard requires exact source parity.
Committed-frame review copies show the retained draft and the actual directory
error in both windows. The 390-pixel/20-point filtered result now shows its full
name, and the 320-pixel/26-point draft/error actions remain reachable.

A repeated source run also exposed a Library fixture's final frame wait after
all its draft assertions had passed. The fixture now explicitly requests a
window update before waiting for that frame. Its focused 71-check suite and
the final 922-check source suite passed after that repair.

The native fixture waits for saved values to synchronize to the primary
window. A retained diagnostic (`native-reload-rejection.txt`) also confirmed
that replacing or repairing the watched runtime file can temporarily reserve
configuration. The fixture retries only the exact reload-busy response, within
five seconds, checking the visible rejection and retained draft first. Other
errors fail normally. The disk-failure phase still requires the real writer's
runtime-override path in its error, before repair and a successful fresh retry.

Logs and renders are retained in `/tmp/ferric-settings-feedback-20261021/`.

All native fixtures use temporary profiles and owned configuration, windows,
D-Bus sessions and compositors. The writer failure is created only inside the
test's temporary profile by replacing its runtime override file with a
directory, then removing that directory before retrying. Wayland navigation
and fresh Return use the owned compositor seat; text entry and explicit repeat
events use Qt. These gates do not qualify a full desktop compositor, physical
hardware, GPU rendering or platform IMEs.
