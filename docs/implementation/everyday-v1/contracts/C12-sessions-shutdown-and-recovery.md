# C12 — Sessions, closure, restoration and privacy

Preserve the existing session model and current schema/data-protection rules. Add optional logical presentation/group hints only through deliberate compatible schema handling and tests. Native client addresses, ephemeral compositor IDs, auth leases and spare state are never durable session authority.

On startup into windows mode, translate saved pages into the presentation without modifying the source backup destructively. Keep restore deferred until the existing crash-recovery decision has been made. Restoring >12 native page windows requires a preview/confirmation; 12 is an explicit chosen product threshold. Materialize in bounded batches and preserve recoverable unmaterialized entries if canceled or interrupted. Do not lose unvisited lazy tabs or use a URL-only replacement to claim full native history restoration.

Unknown/unsupported group hints are ignored while preserving page data. Internal mode remains able to open the session. No replayed POST, authentication ceremony, download, permission grant or page-generated side effect is restored. Maintain privacy/context partition separation and visible private identity. Private session data follows existing nonpersistence policy.

Last-page closure obeys the existing configured last-close semantics within its correct scope. If policy creates a fresh start page, it is a real new page, not the speculative spare. Undo can reuse only an untouched automatic start page per current behaviour; unfinished drafts and existing user pages must survive. Do not force application quit because a one-page host closed, or force a blank replacement where the chosen policy closes the host.

Closing the first visible host while others survive keeps shared resources and per-window keyboard tools alive. Credentials are canceled only for their true owner/invalidated document; no result is delivered to a replacement. Downloads, portal requests, storage tickets and renderers follow existing graceful closure rules. Do not broaden the scope of a Quit confirmation inadvertently.

On final application shutdown, evict all spares, cancel outstanding new operations, asynchronously terminate/reap owned auth helpers and drain accepted persistence work through current application semantics. No lingering spare/transport thread keeps the browser alive. Save evidence of forced failure and recovery using disposable profile roots only; never run reset-data against the user’s normal browser.
