# Text entry

Normal mode is home. Focus the field you want to edit, then choose how long
Ferric should keep sending your keys to the page. Neither entry command
automatically chooses an input.

Insert also forwards keys outside text fields. For video players, games, and
other webpage shortcuts, use [Pass-through](webpage-shortcuts.md), where
ordinary Escape reaches the page and Ctrl+Escape returns to Normal.

| Key in Normal mode | Indicator | Behavior |
| --- | --- | --- |
| `i` | `INSERT` | Transient: ordinary Enter returns to Normal after reaching the page. |
| `I` (Shift+i) | `INSERT LOCKED` | Locked: Enter keeps editing until you press Escape. |

For a search, press `i`, type your query, and press Enter. The page submits the
search using its normal behavior; you can immediately press `j` or `k`, or
start Grid or hint navigation. The input may still have DOM focus: returning to
Normal does not blur it or send a synthetic Escape.

For a multiline prompt, press `i`, type the first line, and use Shift+Enter to
continue composing. Ordinary Enter finishes transient entry. Ferric passes
these keys through unchanged; the website decides whether they submit,
insert a newline, or accept an autocomplete suggestion. Ctrl+Enter, Alt+Enter
and Meta+Enter also keep transient editing active.

For a document or long reply, press `I`. Enter and Shift+Enter keep editing,
and moving between editable fields preserves Locked Insert. Press Escape to
return to browsing. A second typed `i` is ordinary text, not a mode shortcut.
If focusing a field has already entered Insert automatically, press Escape
before choosing `I`; while editing, `i` and `I` are ordinary text.

Transient Insert also ends when an observed field loses focus or disappears.
Both modes reset when changing tabs, navigating the active page, or closing
its view. Enter used to confirm an active input-method composition keeps
transient editing active when Qt exposes that composition.

Focus observation has asynchronous latency. Closed shadow roots, deeply
nested fields, embedded-frame removal without a focus notification, and
editor focus proxies have limited detection. Platform IMEs which clear
composition before forwarding their confirmation Enter may appear to Ferric
as ordinary Enter. Use Locked Insert for extended editing; Escape always
provides an explicit exit.
