# Public Qt cookie source-snapshot boundary (VB-074)

Scope: a complete, consistent read of an arbitrary persistent parent profile's cookies at one defensible T0, including identity and supported attributes. This is **not** the same as seeing some cookies in a fresh fixture or receiving monotonically numbered callbacks.

## Public mechanisms inspected

| Mechanism | Public guarantee | Why it cannot establish strict T0 |
| --- | --- | --- |
| `QQuickWebEngineProfile::cookieStore()` and `QWebEngineCookieStore::loadAllCookies()` | Per-profile store access; asynchronous loaded cookies emit `cookieAdded` | No enumeration-complete signal, count, revision token, atomic read or pause of concurrent setters is exposed. An initial callback may be a load or live mutation. |
| `cookieAdded` / `cookieRemoved` | Individual notifications | No source label or global ordering against all renderer/network writes; an absent notification does not prove an absent cookie. Local epoch/sequence only order observations received by Ferric. |
| `QNetworkCookie` | Name/value, domain/path, expiry/session, Secure/HttpOnly and SameSite enum | No Chromium partition key or full source identity. VB-073 also measured SameSite Strict/Lax/None returning `Default` through callbacks. A complete count under this descriptor could still collapse distinct Chromium cookies. |
| Page JavaScript `document.cookie` | The current document's script-visible cookies | Not a full-profile read; HttpOnly and other origins are excluded, while script, network and service-worker mutations remain possible. |
| `QWebEngineCookieStore::setCookieFilter` | Intercepts cookie access decisions | It is not an enumeration or transaction API. Blocking browser cookie access would change the source being measured and still supplies no complete read boundary. |
| Timer, sentinel, two equal rescans, page freeze, local observation sequence | Ferric-side heuristics | None is an engine-issued completion or consistent revision. Other tabs, network responses, expiration and non-visible work may continue. |

The installed Qt/QtWebEngine 6.11.2 public headers match the documented store methods and two cookie signals; no alternate public snapshot method was found. Primary API contracts: [QWebEngineCookieStore](https://doc.qt.io/qt-6/qwebenginecookiestore.html), [QNetworkCookie](https://doc.qt.io/qt-6/qnetworkcookie.html), and [WebEngineProfile](https://doc.qt.io/qt-6/qml-qtwebengine-webengineprofile.html). This is a finding for the pinned public backend, not a claim about private Chromium internals or all future Qt versions.

## Native stress evidence

`cargo xtask test versioning-cookie-boundary` runs a native Quick-profile probe in two separate processes with a shared disposable XDG root. The first process seeds a persistent synthetic cookie into a disk-backed profile and exits. The second attaches the bounded adapter before its first fixture navigation, requests enumeration, immediately writes a second cookie, serves a loopback page whose JavaScript writes another cookie, then sets and waits out a short-lived cookie. A 127.0.0.1-only server records only synthetic presence bits from actual browser requests. The first request proves the seed survived process restart; the later request proves the seed and JavaScript cookie remain while the short-lived cookie is absent. The adapter's epoch/sequence remains valid and its queue does not overflow.

In three observed successful runs, the persisted seed was present in the browser request but no corresponding initial callback arrived within the 3-second diagnostic window; the concurrent setter callback did arrive. That observation is **not** a general claim that `loadAllCookies` never reports persistent cookies. It illustrates that a quiet or partially populated observation window cannot close the source read. The test does not assert a particular callback arrival order or use a timeout as proof of completion. A later rescan count is diagnostic only.

The application contract regression constructs observed reads after a long quiet interval, an observed sentinel, and repeated byte-equal enumerations while granting *synthetic* positive backend capability claims. Every read still has `Unknown` completeness and `ObservedOnly` consistency, and `try_consistent_snapshot` returns `MissingReadBoundary`. Real public Qt capabilities are weaker still: lossy identity and observed-only snapshots. No exact T0, branch seed, merge base or production flag can be obtained from this fixture.

## Maintained decision

`versioning.cookie_snapshot` remains `unsupported` for the `qt-public-default` backend and full-profile scope. The native experiment compares limited request truth to observation, not a reference snapshot, because no public snapshot mechanism exists to compare. It does not access Chromium's cookie database, install a debugging endpoint, use a private API or modify a normal user profile.

Unblocking requires a separately reviewed backend and exact scope with full identity/attribute fidelity, a complete consistent source boundary, and native concurrency/expiry/persistent-profile tests proving that guarantee. An ordinary-cookie fixture, an observed count, a timer, or a locally assigned revision is insufficient. The separate seed/read-back and controlled-apply gates would still need their own certificates.

## Native-engine follow-up — 2026-09-30

The owner has authorized an isolated compiled-engine prototype. Source inspection
of the pinned Chromium `CookieMonster` found a promising existing read boundary:
its complete-list operation waits for store loading and copies both ordinary and
partitioned records on the cookie-store sequence. Therefore the earlier public
Qt finding must not be generalized to “Chromium cannot snapshot without a new
transaction system.” The [prototype evidence record](engine-capability-spike.md)
tracks the build and the native tests still required. No source inspection or
synthetic read-back result alone qualifies arbitrary-parent T0; the public
backend above remains unsupported, and merge remains a separate gate.
