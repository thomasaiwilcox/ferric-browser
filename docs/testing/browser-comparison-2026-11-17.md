# Ferric and qutebrowser: practical comparison, 17 November 2026

Ferric is competitive on simple pages and ordinary navigation in this sample.
Qutebrowser has a repeatable advantage in dense-feed hint latency and lower
process-tree memory. Ferric offers useful additional interaction features and
filters unusable hint targets more carefully. These results support improving
Ferric's remaining costs without discarding its correctness checks.

This was a bounded comparison of the **installed applications**, using clean,
temporary profiles. No implementation changes, personal profile changes, or
new installations were made. It is not a release qualification or a long-term
stability/battery benchmark.

## Applications and method

- Ferric installed source: `8c27c75a371861dc7e5040fd09226b0461a95486`;
  binary SHA-256: `0f374110d84b4a6e576fe0ebf63a1fefd883b62b7378c2ed6015154f02472e38`.
  This is the strict-hints build after removal of the rejected progressive prototype.
- Ferric launcher: `/home/tom/.local/bin/ferric-browser`; bundled Qt 6.11.2.
- Qutebrowser: `/usr/bin/qutebrowser`, version 3.7.0, system Qt 6.12.0,
  QtWebEngine 6.11.2, Python 3.14.8.
- Most comparisons used the existing owned Wayland test compositor, one
  browser process/profile per application, software rendering and GPU disabled
  for both. App surfaces were 640 × 480, DPR 1. The measured page viewports
  were usually Ferric 640 × 480 and qutebrowser 640 × 444; browser chrome
  changes the available page area and can change visible target counts.
- Separate temporary windows on the actual Hyprland desktop checked focus
  and media policies with real `wtype` keyboard input. Focus/PID guards
  prevented sending input to unrelated windows. Those checks used the normal
  desktop rendering environment rather than the software test compositor.
- Local HTTP fixtures simulated articles, a dense feed, search/reply forms,
  player keyboard controls, canvas controls, and eight loaded mixed-content
  tabs. Fixtures reported DOM events to the local server; production browser
  code was not instrumented.
- The feed had 300 cards, a title link and 20 detail spans per card: roughly
  13,000 DOM nodes with only a few cards visible. This tests the common case
  of a substantial document with relatively few visible targets, rather than
  the deferred 5,000-visible-target stress case.
- Hint timing used two warmups and ten alternating measurements per browser
  in each of two sessions. The endpoint was the first **captured frame with
  labels**, inspected against saved screenshots. A capture itself took about
  40–45 ms. These are coarse capture-return timings, not display presentation
  timestamps or isolated collector CPU timings. The raw `lower_ms` field is
  the previous empty capture return, not a rigorous lower bound on paint.
- This compares the installed stacks as a whole. Runtime versions, browser
  chrome, target families and test-compositor behaviour differ. Results do not
  establish any Rust-versus-Python advantage or isolate a specific bottleneck.

## Responsiveness and resources

| Task / metric | qutebrowser | Ferric | Interpretation |
| --- | ---: | ---: | --- |
| Simple eight-link page, first captured labels, median run 1 / run 2 | 113 / 111 ms | 111 / 111 ms | Indistinguishable at this measurement resolution |
| Dense feed, first captured labels, median run 1 / run 2 | 140 / 138 ms | 217 / 214 ms | Repeatable approximately 76–77 ms Ferric disadvantage |
| Loaded-tab switch, median DOM-visible notification, run 1 / repeat | 23.9 / 26.7 ms | 15.3 / 13.6 ms | Small Ferric advantage in this endpoint; both responsive |
| One-tab process-tree PSS, two sessions | 376 / 342 MiB | 457 / 380 MiB | Ferric higher by approximately 37–81 MiB |
| Eight visited tabs, process-tree PSS, two sessions | 509 / 475 MiB | 591 / 520 MiB | Ferric higher by approximately 45–83 MiB |
| Eight-tab process count | 12 | 13 | Ferric had one additional process |

All 40 recorded hint trials per browser succeeded. All 20 loaded-tab switching
attempts per browser succeeded. Tab switching timings end at a local DOM
visibility-event receipt, not the first painted frame. They do not prove a
perceptible tab-switching advantage.

PSS apportions shared memory and is more useful here than adding RSS across
Chromium processes. The two resource sessions had somewhat different preceding
workflows; the samples are indicative, not a matched fresh-start memory
benchmark. Eight tabs used the same local origin and were each visited before
measurement; this is not eight heavy cross-origin production sites. Shared
library accounting and the different runtime packaging also affect PSS.

A three-second idle sample recorded 0 ms qutebrowser / 20 ms Ferric process-tree
CPU time. That interval is too short, and tick resolution too coarse, to draw
battery or idle-efficiency conclusions. No cold-start speed ranking is made.

## Where Ferric demonstrated a benefit

### More selective hints

The eligibility page contained an enabled button, disabled button, inert
button, transparent button, fully covered button and open-shadow-root button.
Ferric labelled only the enabled and shadow-root buttons. Qutebrowser labelled
all six.

Actual qutebrowser hint activation confirmed that disabled, inert and covered
controls produced no target click, while the invisible transparent control did
receive a trusted click. Ferric avoided presenting those four unusable or
invisible choices. That is a concrete reduction in misleading labels, not just
an abstract correctness claim. It does not prove that every Ferric filtering
decision on arbitrary sites is preferable; both browsers successfully exposed
the open-shadow-root control.

### Policy-based interaction

A separate configured Ferric profile displayed a reserved `h` home label and
numbered feed labels. Immediate `f1` activated the intended first article in
3/3 trials. A separate **key-only** policy on the real desktop mapped `f`, then
`p`, to a trusted page `k` event in 2/2 trials, with 100 ms and 700 ms gaps.
The equivalent tested qutebrowser default workflow used `i`, `k`, `Escape`.
This demonstrates the value of integrating the configured page gesture with
ordinary hints. It is not a claim that qutebrowser cannot be configured with
custom bindings or scripts.

There is a measurable cost: the configured Ferric feed's first captured labels
had a 401 ms median across five trials, versus 214 ms for the unconfigured
Ferric feed in that session. Immediate `f1` action receipts took 615–644 ms.
This is not a like-for-like comparison against vanilla qutebrowser because
Ferric is doing additional policy/collection work. Nevertheless, users experience
that cost, so policies deserve their own performance budget and profiling.

### Canvas interaction through Grid

Neither browser's ordinary element hints exposed the button drawn *inside*
the canvas. Ferric's `;g`, four refinements (`2898`), then `Enter` reached that
button and produced a trusted, in-bounds click. This is a useful escape route
for custom visual controls without using the mouse. Only this left-click case
was exercised here; Grid remains documented as pre-alpha, and this result does
not qualify every scale, frame or race condition. It compares the tested
built-in workflows, not the limits of qutebrowser scripting.

### Transient editing and early input

With Ferric's documented transient `i` entry, submitting the local SPA search
form with Enter let the following `j` scroll the page. Qutebrowser's default
insert workflow instead appended `j` to the field until Escape was pressed.
Ferric's behaviour saves a mode-exit step for short searches and submissions.

On the simple page, sending `f` and the known first label with **zero deliberate
gap** succeeded in Ferric 3/3 times and qutebrowser 0/3. With a 50 ms or 250 ms
gap, both succeeded 3/3. This is a deliberately extreme automation case: it
supports Ferric preserving immediate intent, not a claim that normal human
qutebrowser hint usage loses input.

## Where qutebrowser demonstrated a benefit

1. **Dense-feed hints were faster.** The roughly 76 ms capture-time gap repeated
   across both paired sessions. Simple pages did not show a meaningful gap.
   Further work should target dense-document costs rather than assuming every
   navigation path is slow.
2. **Memory use was lower in both sampled sessions.** The difference varied,
   but its direction was consistent. A matched fresh-start memory breakdown is
   a sensible next investigation before changing process architecture.
3. **Default multiline editing needed less mode knowledge.** Qutebrowser kept
   `first`, Enter, `jjj` as `first\njjj`. Ferric's transient `i` returned to
   browsing on Enter, so the `j` keys scrolled instead. Ferric's documented
   Locked Insert `I` preserved `first\njjj` correctly. This is a UX trade-off:
   Ferric is convenient for short submissions, while users writing comments
   need to know when to use Locked Insert. The case does not justify globally
   removing transient Insert.

## Everyday workflows and live-page checks

- Back and forward (`H`, `L`) returned to the expected article in both browsers.
- In-page search and repeated `n` selected the intended later text in both;
  different viewport/scroll choices are not treated as a performance ranking.
- Closing and undoing a tab restored a page in both, but **neither preserved the
  unsent textarea draft in this test**. Do not rely on close/undo as draft
  protection. No history/session-restore superiority is established here.
- Hacker News and Wikipedia loaded and displayed hints in both browsers.
- YouTube and Reddit loaded and displayed hints on their anonymous consent
  screens. Consent was not dismissed and no account was used. These checks
  establish basic live-page compatibility only: **they do not compare signed-in
  home feeds, actual video playback, or long scrolling sessions**. The dense-feed
  latency measurements and media gesture checks above are local simulations.
- No spontaneous crash was observed during the completed workflows. This is
  a short smoke result, not proof of comparative long-term stability.

## Excluded or limited observations

The first pilot used temporary paths too long for UNIX-domain IPC sockets;
startup failures from that setup were excluded. Short temporary paths fixed it.

In the owned compositor, click-to-type focus observation failed for Ferric,
although explicitly entering `i` worked. On the actual desktop, Tab-focusing
the input and typing `xyz` worked in Ferric. Therefore the isolated focus result
is **not reported as a confirmed Ferric regression**. The real-desktop keyboard
focus check left qutebrowser's value empty; this narrow Tab-focus difference
is not a broad claim about mouse focus or form compatibility.

A deliberately broad fixture policy combined feed-only anchors/collections
with a media page that did not contain them. Its media attempt failed with
`interaction-collection-policy-seal-unavailable`. The independent key-only
policy succeeded on the actual desktop. The broad-policy failure remains a
configuration/diagnostic follow-up, not evidence that the installed YouTube
shortcut is broken. Policy composition and user-facing failure feedback are
worth checking before generalising that setup.

The invalid initial Ferric form trials that omitted explicit mode entry inside
the owned compositor were not used to judge its editing behaviour. Subsequent
documented `i`/`I` trials and the real-desktop check are reported separately.

## Recommended priorities

1. Profile **configured versus unconfigured dense feeds** separately, including
   policy sealing, collection validation and candidate eligibility. Preserve the
   filtering and trusted-input properties demonstrated here. Pursue the largest
   measured costs first; do not revive the rejected progressive prototype on
   the strength of these results.
2. Attribute the memory difference using matched fresh sessions and process/
   allocation breakdowns. Establish whether the additional process, SDK/runtime,
   renderer settings or application state accounts for it before redesigning.
3. Make transient versus Locked Insert clear at the point of use, especially for
   multiline comments. Retain the short-form benefit and the working `I` path.
4. Give configured policies a small, representative regression set: numbered
   feeds, key-only media pages, and rules whose collection targets are absent.
   Improve diagnostics if users cannot tell why an otherwise valid gesture is
   unavailable. Do not add website-specific production exceptions from this test.

Ordinary tab navigation does not warrant optimisation on these measurements.
A larger full-size, GPU-enabled, real-desktop benchmark would be needed before
turning these observations into numerical product claims. This report does not
propose another broad implementation project.

## Evidence

[Recorded observations](browser-comparison-2026-11-17/observations.jsonl) include
successful measurements and excluded cases, identified by run.
[Provenance manifest](browser-comparison-2026-11-17/manifest.json) records the
installed build and hashes of the local harness/logs. Full local fixtures,
scripts, logs, event receipts and screenshots remain under
`target/browser-comparison-20261117/` (ignored build/test artifacts).

Local reproduction uses `/usr/bin/python` (system PyQt6 and dbus), not the
mise Python. The owned-compositor runs used:

```sh
dbus-run-session \
  --config-file=crates/ferric-browser-engine-qt/tests/application/notification-test-bus.conf \
  -- /usr/bin/python target/browser-comparison-20261117/compare.py
```

`extend.py` and `finish.py` use the same invocation; `native_check.py` runs
separately in the actual desktop session and temporarily focuses its own windows.
The archived first run used `run1-source.py`; the working fixture module was
subsequently extended. They are experimental comparison scripts, not a new
supported benchmark framework.

For qutebrowser's configurable commands and hint settings, see its
[command reference](https://www.qutebrowser.org/doc/help/commands.html) and
[settings reference](https://www.qutebrowser.org/doc/help/settings.html).
