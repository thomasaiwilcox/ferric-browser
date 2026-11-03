> **Historical execution pack.** Continue [the active window-mode and phone-passkey plan](../WINDOW_PASSKEYS_PLAN.md). Hot tabs are deferred; the old worker/amendment/attempt framework below does not control current work.

# Ferric Everyday Features — Luna execution pack

## Outcome

Deliver three independent, opt-in capabilities without regressing ordinary internal-tab browsing:

1. One bounded, never-visited hot-page spare with immediate cold fallback and measured benefit.
2. Explicit QR phone passkey registration and sign-in, with document-bound browser authority, a supervised transport helper and native-security-key coexistence.
3. Configurable one-page-per-native-window presentation and separately qualified Hyprland grouping, preserving Ferric's keyboard tools, privacy, sessions and popup semantics.

The repository is not bundled. `START_HERE.md` contains its exact address and clone procedure. Source review was performed through the GitHub connector at the frozen baseline recorded in `BASELINE.md`. No browser code was changed or compiled while authoring this pack. The pack's own local Python tooling is tested separately in `reports/AUTHOR_VALIDATION.md`.

## Where to find things

| File or directory | Purpose |
|---|---|
| `GOAL_PROMPT.txt` | Single short Codex goal, under 1,000 UTF-8 bytes. |
| `START_HERE.md`, `ENVIRONMENT.md` | Clone, current-source reconciliation, SDK/build/test setup. |
| `BASELINE.md`, `control/source_manifest.json` | Inspected code map and frozen file identities, with inspection limits. |
| `contracts/` | Normative architecture, lifecycle, protocol, security and user behaviour. |
| `slices/`, `control/plan.json` | Small dependency-ordered work orders, file allowlists, locks and acceptance. |
| `control/test_matrix.json`, `control/requirements.json` | Traceable positive, negative and native/physical acceptance cases. |
| `ORCHESTRATOR.md`, `WORKER.md`, `REVIEWER.md` | Bounded agent roles, independent review, integration, retry and resume policy. |
| `TESTING.md`, `RESOURCE_POLICY.md`, `RELEASE.md` | Real evidence, build-disk safeguards and capability-specific release gates. |
| `TOOLS.md`, `tools/` | Safe bootstrap, preflight, packet generation, readiness, command receipts, validation and owned-artifact cleanup. |
| `templates/` | Work orders, review, evidence, blockers, checkpoint and capability release records. |
| `reference/ORIGINAL_PROPOSAL.md`, `SOURCES.md` | Historical design input and independently checked references. |

## Precedence

Repository/user/harness instructions take precedence. Within this pack: numbered contracts and explicit BASELINE amendments; requirements/test matrix; slice acceptance; scheduling plan; historical proposal. An actual conflict is a recorded specification blocker. Do not silently choose the weaker rule or delete an existing guard. Current source takes precedence over historical assertions about what is implemented, but does not silently redefine this pack's required behaviour.

## Deliberate constraints

Internal tabs remain the default. Feature settings start disabled (presentation starts `internal`). No daemon/autostart, predictive external prefetch, live mode conversion, credential vault, remembered-phone pairing, general iframe phone support, Chromium fork or automatic compositor reconfiguration. Existing security-sensitive native glue stays intact and version-qualified; its presence is not permission to introduce new private Qt APIs.

The suggested task size limits are adapted to the user’s implementation-pack workflow. The pack is self-contained; it does not claim formal conformance to an external standard or require a proprietary orchestration tool. Where subagents are unavailable, execute the same slices serially with fresh review context.
