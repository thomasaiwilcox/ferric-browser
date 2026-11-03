# C08 — Credential actor and browser-owned UI

Use an explicit actor/state machine: Received → Validating → AwaitingProvider → Starting → AwaitingPhone → Completing → Terminal. Validation/protocol/startup failures, AbortSignal, deadline, document replacement, renderer loss, profile/window teardown and shutdown take an exactly-once terminal path. Every transition binds the captured request ID, document lease, profile generation and owning host. An old helper/JS callback cannot change a new ceremony. Same URL is not same document.

Allow only one bridge-managed chooser/phone ceremony per engine process initially. Refuse additional requests with bounded errors rather than an invisible queue. Preserve unrelated native baseline authentication paths. Reject new phone admission from an ineligible background page. Once admitted, losing compositor focus does not retarget the operation; hide/suspend its owned UI as appropriate, leave cancellation available and maintain deadline/owner. Completion can go only to the still-valid originating document.

Compose provider choice, QR/progress and cancellation using current modal input/focus lease infrastructure. Show a sanitized verified site/RP identity as plain text. Never render page/helper HTML. QR uses fixed high contrast and correct quiet zone independent of theme. It is selectable neither for clipboard nor accessibility raw-secret output. Accessible labels explain scan/phone approval without encoding the secret.

Default keyboard focus is the safe cancel/choice control, not an irreversible registration action. Esc cancels. Tab navigation, zoom/large text and private identity remain usable. Page shortcuts, macros and generic IPC cannot choose a provider or approve the prompt on behalf of a human. Test visual owner/focus on primary, secondary and popup hosts.

Provider capability status is coarse and explicit: available, disabled, missing-helper, missing-PSL, Bluetooth-unavailable, unsupported-network-policy, unqualified-boundary, busy, unsupported-context. Distinguish product diagnostics from page-facing WebAuthn error semantics. Never send helper stderr or raw internals to the site.

Physical qualification requires an independent test relying party to verify a create then get with challenge/origin/RP hash/signature/flags checks. Do not treat phone UI approval as proof. Record actual iPhone and Android results separately, plus a native security-key non-regression. No registration in real personal accounts without explicit user action; supply a controlled test site and manual instructions.
