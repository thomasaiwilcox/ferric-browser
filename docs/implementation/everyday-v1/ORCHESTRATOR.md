# Coordinator execution contract

## Objective and authority

Deliver the three capabilities and their verification, not a fresh proposal. Work from the checked-out source reconciled in S00. This pack specifies desired outcomes and bounded authority; it does not override higher-priority repository or platform instructions. The historical proposal is context, not a second competing specification.

Coordinator owns the execution map, plan amendments, task scheduling, shared registration edits, acceptance records and serial integration branch. Workers implement. Fresh reviewers review. A worker cannot promote its own work to accepted. An independent agent is useful scrutiny, not a substitute for a human credential-security review where required.

Default execution is sequential if the harness has no real subagent tool. Do not simulate parallel agents by pretending separate identities. A single harness can run implementation and then a new independent review session; record the actual session identities. At most six non-coordinator agents: normally four implementers and two reviewers. At most two builds overall, one native Qt/C++ link/test pipeline, and one physical/compositor qualification session. Start with one build on a constrained machine.

## Bootstrap and baseline

1. Read START_HERE. Establish the exact repository, safe workspace and branch. Read applicable AGENTS.md and CONTRIBUTING.md. The pack must not replace them.
2. Fetch current main once, record fetched SHA and HEAD, compare with authoring baseline and source map. Code search is not a source-of-truth checkout. For shallow history, fetch the required reviewed base or record the absent history; never report an empty diff merely because the base is absent.
3. S00 resolves proposed/current paths and all actual command names. Record file ownership, SDK versions, existing test inventory and unmodified baseline failures. Latest means fetched main, not an unmerged branch chosen by name.
4. Freeze `execution_baseline_sha`. Branch work starts from a reviewed integration commit. Do not pull/rebase under running workers. If main advances, checkpoint, review the diff and schedule a separate rebase/reconciliation; never overwrite existing work.
5. Import/commit this pack separately using configured author identity. Keep mutable RUN_CONTEXT, state, attempts, raw logs and working checkpoints outside the product checkout. This makes a test receipt's clean product SHA meaningful. Sanitized final reports may be copied into the repository later; record the source revision they describe.

## Worktree and ownership rules

Use one isolated git worktree per implementation worker or execute sequentially. A task worktree is created from the recorded integration SHA on a local branch such as `work/everyday-v1-H02-a1`. Record worktree and branch before dispatch. No `git reset --hard`, `git clean -fdx`, force push, automatic stash, replacement of a dirty checkout or deletion of unrelated worktrees.

Both named locks **and exact allowed-path overlap** exclude simultaneous writing. Shared `lib.rs`, `bridge.rs`, build.rs, Cargo manifests, CMakeLists, xtask, root shells, settings registries and generated references require serial ownership. `tools/ready.py` suggests non-overlapping candidates; it does not acquire locks or grant acceptance. The coordinator writes claims atomically before dispatch. A lock records slice, worker, branch, start and lease heartbeat. Expiry is not permission to steal a live worker's files: first verify its state and owned processes.

Worker scope is one slice, up to eight changed product files, no recursive delegation, and only named contracts. Its packet is at most 24,000 UTF-8 bytes; source reading has 48,000 soft / 96,000 hard bytes per attempt. Read targeted symbols and callers before full files. If the packet/source/edit budget is insufficient, stop and split into real child slices. Do not truncate a safety contract to fit.

## Plan amendment and registration protocol

A moved file or required ninth file triggers an explicit amendment before editing. Record old/new path, reason, dependency effect and test impact in `control/AMENDMENTS.md`; amend execution plan and locks, then validate. A task too large becomes `F02a/F02b`, with separate outcomes/tests and dependencies replacing the parent. Parent acceptance is an aggregate of accepted children, not a second invented pass. The shipped validator supports only flat task IDs, so flatten amended tasks into the execution plan and carry parent grouping as metadata. Keep the original pack intact for provenance.

Workers who need a shared registration edit submit the exact diff and test registration name. Coordinator either serializes that worker's access or adds the reviewed patch during integration. Never have a worker write to another worktree. A file rename must update the current execution source map, not resurrect an obsolete Runtime*.qml filename.

## Ready → implement → review → integrate loop

- Ready only when dependencies are accepted, locks/files are free, relevant proof gates passed and resources are available. `not_started` and `ready` are eligible. Review-only work also needs real source access.
- Create a packet using make_packet.py; attach the current integration SHA, resolved path map, relevant caller snippets, test IDs, task-specific commands, branch and environment constraints. Never attach the whole conversation or all implementation packs.
- Worker reproduces relevant baseline and adds behavioural assertions before/following its code change. It returns a <=6,000-byte WORKER_RESULT with exact candidate commit, changed files, results and blockers. No broad cleanup/refactor or permissive fallback outside scope.
- Fresh reviewer reads the actual diff and relevant contract/callers, checks positive and negative tests, and writes REVIEW.json. Source-string checks are architecture hints, never runtime proof. For credential work, trace all authority and delivery edges; for compositor work, trace exact target and focus races.
- Coordinator integrates approved commits serially onto the integration branch. Before cherry-picking, check source/base/changed-path overlap and current branch cleanliness. Conflicts require a bounded repair, not an automatic "ours"/"theirs" decision.
- Rerun affected gates on the integrated source. A worker receipt for a different SHA cannot simply be relabelled as integration evidence. Update the acceptance record only after the integrated SHA and reviewed tests match.
- Release locks, update state, append the checkpoint and schedule the next ready work. Continue until all executable work is finished or every remaining dependency has a concrete blocker.

## Status and repair limits

Allowed execution states: `not_started`, `ready`, `in_progress`, `submitted`, `reviewing`, `accepted`, `blocked_environment`, `blocked_specification`, `implemented_unqualified`, `failed`. Initial state is not implementation evidence. `accepted` requires independent review and all slice cases evidenced. Maximum three attempts total (initial plus two repair rounds). If repeated repairs fail, preserve work/logs, record the exact failure and split or block it; do not weaken tests, expand scope indefinitely or ask another agent to rubber-stamp it.

An implementation can be present while native evidence is missing. Store that as `implemented_unqualified`; dependent code that truly needs only the implementation may be split from a separate qualification gate through an explicit plan amendment. Do not change a declared dependency merely to turn a dashboard green. Keep working on unrelated helper, policy, fixture and generic-window tasks.

A failed public-API proof is not successful delivery of phone support/grouping. Record the missing primitive, reproduction, rejected alternatives and next technical decision. No Qt fork or weaker security exception is implicitly authorized. Physical tests require actual available devices and operator consent; never access the user's real accounts to satisfy them.

## Evidence, integration changes and resume

`tools/run_gate.py` records a command and result, not the correctness of every assertion. Reviewers map acceptance cases to actual tests and expected outcomes in an EVIDENCE record. Native and physical records state the real execution environment. Acceptance records point to reviewed candidate and integrated SHAs and log digests. Final qualification must be refreshed on the final source, or explicitly justified as unchanged by a reviewed source-dependency analysis; the default is rerun.

After each integration or blocker, update external `control/CHECKPOINT.md`: integration SHA, completed/current tasks, locks, branches, build ownership, resource state, last commands, blockers and next actions. Before resuming, inspect Git status and task records, confirm processes actually running, invalidate stale source evidence, then continue. Never start a second compiler simply because the previous chat stopped responding.

Only S00, X04–X06 and scheduled milestone checks read the whole plan. Normal workers read their own task. The work ends with a truthful local handover: implementation status, qualification status, tests run, measured performance, current source and remaining blockers. No remote push/PR/merge, release publication, privileged installation, user-profile migration or compositor configuration edits are included in the user's request.
