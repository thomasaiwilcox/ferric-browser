# M0-57 — Private and ephemeral journey memory boundary

Private and ephemeral profiles already use the bounded core `JourneyGraph`,
but have no `ProfileStore`. The native `journey` command now renders their
profile-local in-memory nodes through the same accessible library manager and
reports `durability: memory-only`. `journey-reopen` can resolve only a matching
live in-memory node and still performs a fresh safe GET; it cannot cross into a
normal profile.

Ordinary IPC refuses `journey` and `journey-reopen` for transient profiles, so
the memory-only topology is not exposed through the control socket. The
explicit final-owner release boundary clears the core state, journey durable-ID
mapping, pending journey work, session permissions, and durable-store handles;
the QML owner-release path invokes it before the transient profile is dropped.
Session snapshots remain excluded because transient profiles have no session
path, and native export presents a memory-only warning plus an explicit local
save boundary. Remaining work is runtime/marker qualification across real
close and crash sequences.
