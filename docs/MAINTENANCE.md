# Maintenance discipline

Before V1, maintainers must record the actual maintainers and support window;
the project does not promise a durable release cadence while those are
unstaffed. The primary branch and signed/tagged release process are the source
of truth. Public CLI, configuration, and IPC changes use semantic versioning
after V1, with IPC major-version negotiation and migration guidance for
deprecations.

Avoid speculative framework extraction, engine-generalization layers, and
unowned feature flags. Every new registry concept needs an owner, focused
tests, a requirement record, and an evidence update. Release reviews must
refresh the dependency/license inventory, engine freshness policy,
compatibility report, known issues, and support status.
