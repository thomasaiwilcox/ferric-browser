# Compatibility regression diagnosis

This is the controlled record template for COMPAT-005. It is used when a
qualified site regresses; it does not claim that a live regression has
currently been observed.

## Application and engine identity

- Application commit/package: no active regression recorded
- Qt/QtWebEngine and Chromium security-patch level: record from the installed package
- Distribution/compositor/GPU/portal/PipeWire baseline: record the qualified test machine
- Qualification date: record in UTC

## Observed site and symptom

- Exact site pattern and service area: record only the minimum affected origin/path
- Symptom and expected behavior: record a short, sanitized description
- Status: no-active-regression

## Sanitized reproduction

Record deterministic steps using disposable data, with account names, tokens,
message contents, document contents, and personal URLs removed. Do not record credentials.
Do not record cookies. Preserve the smallest fixture or URL that
reproduces the behavior.

## Minimal Qt reference comparison

Run the same steps in the minimal reference browser built from the same
Qt/QtWebEngine family and record whether the failure reproduces there. If the
reference behavior differs, record the smallest adapter boundary implicated.

## Upstream issue and authorization

Record an upstream issue URL only when filing or linking it is authorized.
Never upload private pages, account data, cookies, credentials, recordings, or
diagnostic archives containing them.

## Scoped workaround

Record either none or the reviewed workaround ID. Any workaround must be
bounded by the exact affected site pattern and engine-version range, have a
reproducible regression fixture, and be reversible. Do not copy a qutebrowser workaround
without independently reproducing and reviewing it.

## Removal condition

State the upstream version/fix and test that removes the workaround. A
workaround without a removal condition is not eligible for the compatibility
registry.
