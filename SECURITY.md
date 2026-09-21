# Security policy

Do not report vulnerabilities in public issues, pull requests, or the bug
template. Do not upload profiles, cookies, credentials, or unredacted logs.

Use GitHub Security Advisories for private vulnerability reporting:
`https://github.com/ferricbrowser/ferric-browser/security/advisories/new`.
The public issue process is only for sanitized, non-sensitive behavior reports.

QtWebEngine supplies Chromium and the rendering sandbox. Ferric Browser's
security therefore depends on timely, supported Qt/Chromium packages from the
distribution; a passing Rust dependency audit does not establish browser-engine
patch status. Releases are blocked by known applicable critical engine issues.
