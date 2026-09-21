# M0-59 — Native journey relationships

The native `journey` library surface now has two views:

- `Outline` shows the bounded chronological node list with sanitized title,
  URL, transition, and source fields.
- `Relationships` shows the retained source-to-target edges using text labels
  such as `navigate`, `reopen`, and `branch-after-back`.

The relationship data is assembled from the profile-local SQLite edge table
for normal profiles and from the bounded memory graph for private and
ephemeral profiles. It is display-only: rendering the view never loads a page,
requests a recorded URL, or exposes renderer/form data. Private and ephemeral
views continue to be labeled memory-only and are not available through
ordinary IPC. Outline rows also provide a native `Reopen` action that submits
only the displayed node identifier to the existing safe recovery command.
