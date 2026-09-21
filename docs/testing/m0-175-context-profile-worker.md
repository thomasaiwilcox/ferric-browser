# M0-175: worker-backed context profile validation

`context-create NAME --profile PROFILE` now requires and validates its
referenced durable profile through the
bounded profile-list worker. The command returns an accepted pending response;
the context registry is mutated only after the worker confirms an exact
profile name. Missing profiles, registry failures, and registry mutation
errors remain status-visible and do not create a partial context.
