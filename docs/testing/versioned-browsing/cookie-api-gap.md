# Public Qt cookie API gap (VB-005)

This is a capability finding for the installed Qt/QtWebEngine `6.11.2` build, not a recipe for copying an authenticated profile. The native probe uses an off-the-record profile, a `data:` page and synthetic cookies only. No raw cookie value is written to this document or the test's output.

| Required fact | Public representation / observation | Finding |
| --- | --- | --- |
| Raw name and value bytes | `QNetworkCookie::name()` / `value()` | Available as opaque bytes; must remain secret in production. |
| Domain and path | `domain()` / `path()` | Available. Host-only construction requires an empty domain plus an explicit origin to `setCookie`; a non-dot domain is broadened by Qt's setter. The native Quick-profile probe verified host-only request isolation for newly written cookies. **Source-inspection correction, 2026-09-30:** canonical domain strings encode host-only versus domain scope by absence/presence of a leading dot, and Qt's callback conversion preserves that string. An explicit flag is not inherently necessary. Arbitrary-source reconstruction remains unqualified by the existing native fixture; it is not proved impossible. |
| Expiration/session | `expirationDate()` / `isSessionCookie()` | Synthetic persistent expiry was exposed. |
| Secure and HttpOnly | `isSecure()` / `isHttpOnly()` | Survived the synthetic native store round-trip. |
| SameSite | `sameSitePolicy()` | Synthetic `Strict`, `Lax` and `None` inputs were all observed as `Default` in `cookieAdded` on this build. Exact attribute read-back is therefore not established for any of those tested inputs. `Default` itself remains only an observed policy value, not a proof that the source explicitly requested an unspecified attribute. |
| Partition identity | No partition-key field on `QNetworkCookie` | Parsing synthetic `Partitioned` and unpartitioned headers produced the same public identifier; `toRawForm()` omitted `Partitioned`. A map keyed only by the public identifier could collide. No actual partitioned Chromium jar was inspected. |
| Complete enumeration | `loadAllCookies()` is asynchronous and emits `cookieAdded` | No completion signal, snapshot token, count or atomic boundary is exposed. A quiet interval or sentinel cannot prove completeness. |
| Controlled apply | `setCookie` / `deleteCookie` are asynchronous `void` calls | No public transaction/CAS or exclusive writer boundary was found. Notifications do not turn a batch into an atomic apply. |

The probe preserves each notification in a list. It does not overwrite a record under a lossy `(name, domain, path)` key. Its classification regression asserts that zero observations without a completion proof are `Unknown`, not `KnownEmpty`, and that observations with identity loss are still not a strict snapshot. The application mirror also invalidates on conflicting records under a public key rather than silently overwriting them; identical duplicate enumeration remains diagnostic-only. The `loadAllCookies` rescan emitted zero additional events in one 250-ms diagnostic window; that number is explicitly **not** a completeness marker. The additional loopback server binds only 127.0.0.1 and records synthetic cookie presence, not cookie headers or values.

The strict general-cookie gates remain unsupported: lossless identity, complete snapshot and controlled merge apply. General destination seed/read-back is also unsupported on this tested public backend because all three tested non-default SameSite inputs lost that attribute in the callback. A future backend or narrower scope would need its own exact engine fingerprint, attribute/partition coverage, completion proof and native round-trip tests before changing those gates. The tested positive scope is only synthetic public-store conversion of raw bytes, domain/path, session/expiry and Secure/HttpOnly, plus host-only request isolation for a newly set cookie on a fresh off-record profile. It does not prove arbitrary source provenance, partition keys, complete reads or credential continuity. None of these findings authorises Chromium database reads, private APIs, remote debugging or profile copying.

Primary API references: [QWebEngineCookieStore](https://doc.qt.io/qt-6/qwebenginecookiestore.html), [QNetworkCookie](https://doc.qt.io/qt-6/qnetworkcookie.html). Native behavior and exact package fingerprint are recorded in [VB-005.md](VB-005.md).

The approved [isolated native-engine prototype](engine-capability-spike.md)
investigates a different backend. Chromium retains canonical fields and has a
complete-list callback that Qt's public wrapper discards. That promising route
does not change any negative result or capability flag for `qt-public-default`.
