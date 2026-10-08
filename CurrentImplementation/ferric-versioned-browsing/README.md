# Ferric Versioned Browsing — proposal pack

**Read `PROPOSAL.md` for the complete design. Give `AGENT_HANDOFF.md` to an implementation agent first.**

For transfer to another machine, fetch the `feature/versioned-browsing` branch
and begin with the **Transfer checkpoint** at the top of `AGENT_HANDOFF.md`.
The source and ledger are committed there; ignored local Qt/Chromium build
outputs are not. VB-099 remains in progress and no strict production
capability is enabled.

Prepared against main `deea458d92bcf6d8fbea7b04af17823b5882cc1b` on 27 September 2026 and rechecked before packaging.

The proposal contains the complete retained vision, an exact repository integration map, public Qt API boundaries, state/storage/command designs, security and failure handling, and **103 dependency-ordered slices across nine phases**. VB-096–098 separate current-window replacement, ordinary viewless downloads and hidden-parent branch retention from independently feasible routes. Owner-approved VB-099–102 add the maintained pinned-backend implementation and its separately qualified RTC, cookie and ordinary-download primitives; none of those additions enables a production capability.

`IMPLEMENTATION_BACKLOG.json` is the resumable work ledger. `IMPLEMENTATION_SLICES.md` is a standalone rendering of the same task specifications. `CAPABILITY_GATES.json` separates primitive API support from production feature enablement.

The central scope is structural commits/restore/diff, qualified live stashes, archived and reconstructable page state, and clean isolated branches. Snapshot-inherited cookies and cookie write-back retain their full design but remain capability-gated: the current public Qt cookie API is not treated as an atomic, fully lossless Chromium storage API.

The original packaged proposal performed no Ferric build or native runtime test. Subsequent implementation and focused/native evidence are recorded in `AGENT_HANDOFF.md`, the backlog and `docs/testing/versioned-browsing/`. The included validator still checks proposal-pack integrity only, not a Ferric build.

Run:

```sh
python3 validate_pack.py
python3 validate_pack.py --next
```

If adopting these files into the repository, use the repository's current documentation conventions and keep the relative pack files together. Do not assume the suggested new ADR number or source module already exists; VB-000/001 handle alignment and architectural adoption.
