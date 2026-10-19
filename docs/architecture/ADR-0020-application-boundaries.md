# ADR-0020: Application ownership and native request boundaries

Status: implemented on `refactor/application-boundaries`.

## Ownership

| Layer | Owns | Must not own |
| --- | --- | --- |
| Core | Reducer, identities, invariants, command registry, failure categories | Qt, storage, wire serialization |
| Runtime | Command context, event dispatch, typed observations, worker lifecycle | IPC responses, eager UI snapshots |
| Application | Configuration transactions, profile lifetime, operation catalog, userscript catalogs and jobs, durable scheduling | QObject or QML references |
| Storage | SQLite/session formats, payload validation, bounded transport and execution | Window state or command routing |
| IPC | Version 3 schema, codecs and public error projection | Domain failure definitions |
| Qt adapter | Native facilities, presentation projections, shared command executor, operation notifications | A second authoritative configuration or operation catalog |
| QML | Native views, chrome and explicit-host controllers | Reducer mutation or authority inferred from page data |

The Qt adapter's JSON configuration values are presentation projections. Domain
configuration is read from `BrowserApplication::configuration` and mutations
pass through prepared application transactions. Workflow-specific adapters
remain separate from the common command executor because their native consent,
profile and window routing differ.

## Deferred document work

Reducer effects and application `ViewRequest` values retain tab, generation and
document identity and own their arguments. The adapter projects these into
QVariant records. Both window shells call `EngineRequests.js`; a request is
checked when dequeued and again after synchronizing native views. A later tab
switch cannot retarget work, a later script cannot overwrite an earlier payload,
and replacement documents cannot consume a stale request.

Window and feature presentation intents remain adapter-local. They are separate
from document requests and do not grant authority to page content. Site Doctor
reload/fresh-view work uses captured requests; temporary-view cleanup resolves a
stable tab ID rather than a remembered index.

## Configuration and userscripts

Persistent configuration is validated, reserved and written before publication.
The worker owns its profile lock and reservation. A completed rename with
directory-sync uncertainty remains a committed write. State roots share a
persistent-layer authority and revision even across different named profiles;
window-specific temporary and command-line layers remain local. A retired
observer cannot suppress a committed update to a live peer.

Startup configuration is a construction input accepted only before profile
activation. Secondary windows do not consume the primary window's live JSON
projection as startup state. Their file watchers and the shared persistent
layer provide live updates without resetting active interaction sessions.

Userscript discovery, bounded source loading and mutations belong to the
application. Navigation and action lookup use an immutable catalog. Mutation
jobs retain their profile lease and a shared root reservation; their results
publish only into the admitting application. Retiring an owned worker closes
its channels without joining unfinished filesystem work on the UI thread.
Durable storage retains its explicit drain and flush lifecycle.

## Operations and protocol compatibility

Protocol 3 replaces protocol 2. An operation is admitted before its side effects.
The application catalog bounds active work, terminal history, output bytes and
pending completion notifications independently. Terminal transitions are
irreversible. Retention cannot evict active work or erase an undelivered terminal
notification. Queries use bounded pages; cursors identify an owner and admission
sequence. Cross-window aggregation paginates once after collection.

Database, session and configuration file formats are unchanged. Qt properties
and command-line grammar remain the presentation boundary; CLI arguments are
quoted through the browser command grammar before IPC transport.

## Qualification

See [the integration record](../implementation/full-refactoring.md) for executed
checks, performance evidence and remaining platform qualification limits.
