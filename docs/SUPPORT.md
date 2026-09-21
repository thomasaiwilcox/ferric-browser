# Support and issue triage

Ferric Browser reports must be reproducible and privacy-safe. Do not attach a
profile directory, cookies, authentication headers, browsing-history database,
clipboard contents, unredacted logs, or page dumps.

## Required report facts

Include the application version/commit, Qt and QtWebEngine versions, operating
system and compositor, GPU/driver, portal and PipeWire availability, the exact
entry point, reproducible steps, expected behavior, actual behavior, whether a
fresh temporary profile reproduces it, and a sanitized diagnostics report.

Replace URLs, titles, account names, paths, tokens, and form contents with
stable placeholders before sharing diagnostics. If a minimal local fixture can
reproduce the issue, prefer that over a real site.

## Triage categories

Use exactly one primary category and add secondary labels only when justified:

- `core`: command parsing, reducer state, storage-independent behavior;
- `qt-adapter`: Qt/QML bridge, lifecycle, focus, or browser-owned UI;
- `upstream-engine`: QtWebEngine/Chromium behavior reproducible outside the
  Ferric Browser adapter;
- `desktop-portal`: Wayland, compositor, portal, PipeWire, or desktop launch;
- `graphics-driver`: GPU, rendering, or compositor-driver failures;
- `packaging`: package recipe, installed files, dependencies, or launchers;
- `site-policy`: blocking, cleaning, compatibility, or site-specific policy;
- `user-configuration`: profile, context, binding, theme, or local override.

Document workarounds with their scope, tradeoffs, and whether they change
privacy or security behavior. Escalation to an upstream project requires a
minimal reproduction and explicit authorization; posting an external issue is
separate from diagnosing the local problem.

## Security reports

Do not use the public issue templates for vulnerabilities. A private security
reporting route must be configured and published before a public V1 release;
until then, keep sensitive details out of public channels and contact the
maintainer through the project owner’s separately managed private route.
