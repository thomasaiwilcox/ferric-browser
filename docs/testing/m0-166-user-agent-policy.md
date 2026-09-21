# User-agent policy

The browser does not set an application-specific QtWebEngine user agent. The
engine's default identity is retained so compatibility reports describe the
actual Qt/Chromium build rather than a newer Chrome version. Site-specific
compatibility changes must use the reviewed, engine-version-scoped registry;
they must not be implemented as global user-agent spoofing.

`ferric-browser diagnostics --format json` reports this as
`build.user_agent.mode = engine-default`, with `override = false` and
`provenance = qtwebengine-default`. The policy is also covered by the
`compatibility::tests::user_agent_policy_keeps_the_engine_identity` test.
