# M0-43 Context routes

Date: 2026-09-16  
Status: in-progress  
Requirement: CTX-004

## Implemented slice

`contexts.toml` route definitions now use the narrow CONFIG-006 HTTP(S)
matching grammar: HTTP, HTTPS, or wildcard scheme; exact, `*`, or leading
label-wildcard hosts; default or exact ports; and path globs without query or
fragment matching. File and internal schemes are excluded. Matching filters by
the configured entry point and orders conflicts deterministically by priority,
pattern specificity, then authored order.

Explicit URL/host opens are intercepted before navigation when a route matches.
The browser-owned confirmation surface shows the route, target context/profile,
and safe address. Accepting a same-profile route assigns the context and
navigates; accepting a cross-profile route opens a new window with the target
profile/context. Dismissing the prompt performs the original navigation. The
accepted and dismissed paths bypass route matching for that request, so a route
cannot repeatedly intercept itself.

The route prompt is limited to the pre-navigation explicit-open path. Redirects,
popups, form submissions, same-document navigation, WebAuthn, permissions, and
authentication chains remain on their established opener/profile boundary.

The shared command, CLI, and typed IPC surfaces now support the commands
context-route list, context-route add PATTERN CONTEXT, and context-route remove
ROUTE_ID. Add also accepts the options --priority INTEGER, --behavior
prompt|suggest, and repeatable --entry-point
external-open|explicit-open|typed-initial-url. With no entry-point option, the
route applies to all three browser-controlled pre-navigation entry points.
Changes are validated and atomically persisted to the sibling contexts.toml;
the live browser and secondary windows receive the updated route document.

An add response includes same-pattern conflict metadata and the deterministic
resolution rule (priority, then pattern specificity, then authored order).
Same-pattern routes targeting different contexts must use different priorities
when their entry points overlap; the validator rejects contradictory
equal-priority routes before persistence.

The CLI marks forwarded opens as external-open, and a new primary window
receives its startup entry point explicitly. Thus routes may opt into
external desktop opens or typed-initial-url opens without treating an
interactive address-bar navigation as either source. Browser-created secondary
windows use the preflighted one-shot path and do not re-intercept the same
request.

Typed IPC `open` requests can also target a new normal-profile window while
carrying a context. The route validator checks context/profile affinity before
any caller-window mutation; the pending Qt action carries the target profile
and context into the new window, which attaches the context before its
preflighted navigation.

## Verification

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-config -p ferric-browser-core -p ferric-browser-engine-qt -p ferric-browser --locked
cargo xtask check
QT_QPA_PLATFORM=wayland target/debug/ferric-browser --temp-basedir
```

The Wayland smoke run is bounded by the local test harness timeout and should
exit only because the timeout stops the still-running application, with an
empty diagnostic log.

## Remaining qualification

Full interactive multi-window conflict/OAuth qualification remains for later
slices.
