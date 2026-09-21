# M0-177 profile bootstrap worker

Initial profile storage bootstrap now runs on the named
`rustbrowser-profile-bootstrap` worker. Profile registry discovery, profile
creation, lock acquisition, directory creation, SQLite store opening, and
profile-layer and persistent runtime override loading are performed off the Qt
thread. Configuration include/source discovery is also performed there and
the profile-local context registry is opened there as well; all results are
returned through one bounded handoff.

The QML startup sequence waits for the result before clearing recovery
checkpoints, registering the browser window, attaching request interception,
or issuing the initial navigation/context command. This keeps startup storage
ownership deterministic while preserving the existing transient-profile path.

Coverage:

- `profile_setup_worker_opens_profile_store_off_thread` verifies a normal
  profile is opened and its session metadata is returned by the worker.
- `network_policy_worker_loads_cached_policy_off_thread` verifies cached
  blocker policy loading uses the bounded worker handoff.
- Profile-local context registry opening and configured-definition application
  are included in the bootstrap result, preventing duplicate registry reads or
  synchronous definition writes during startup.
- `cargo check -p browser-engine-qt --tests --locked --offline`
- `cargo clippy -p browser-engine-qt --all-targets --locked --offline -- -D warnings`

Native Qt/WebEngine startup still requires the separately documented display
and Wayland qualification environment.
