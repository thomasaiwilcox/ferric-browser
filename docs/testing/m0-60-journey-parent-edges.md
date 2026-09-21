# M0-60 — Journey parent edges and transition tagging

The journey commit boundary now accepts an optional validated parent node.
Popup navigation captures the opener's current core node and links the newly
committed popup node with a typed `popup` edge when both durable mappings are
available. The same relationship is retained in the memory graph for private
and ephemeral profiles.

Hint link navigation is tagged `hint`, and the selected session-recovery
navigation is tagged `session-restore`. These are still safe descriptors only:
no opener headers, renderer state, form data, or page-script output crosses
the journey boundary. Redirect observation remains a subsequent slice;
bounded search and one-hop neighborhood behavior is covered by
`m0-61-journey-search-expansion.md`.
