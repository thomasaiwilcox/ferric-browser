# Start page

`rb://start` is Ferric's local, native start page. It offers **Top Sites**,
**Favourites**, and **Guide**, with Guide as the fresh-profile default. Click a
view button or use `Ctrl+Shift+Space` to cycle. Your selection is saved per
normal profile and used when you next open the start page. `g s` or
`:start-page` opens it in the current tab; `:start-page-view favourites` opens a
specific view. These commands and their bindings are configurable.

Closing the final tab opens the start page in the same profile, using your saved
view. Use `rb://blank` when you want an intentionally empty page.

## Keyboard navigation

In Normal mode, focus a site grid to use `1–8` to open a populated slot, or use
arrows/hjkl to select and Enter to open. Empty numeric slots do nothing. Tab and
Shift+Tab reach view buttons, tile options, and other controls. The shortcuts
are inactive in text fields, menus, dialogs, other modes, and pending browser
key chains/counts. Alt+1–8 still selects browser tabs; Alt+9 selects the last tab.

## Top Sites

Eight tiles are ranked from retained, eligible HTTP(S) visits, combining visit
frequency with exponential recency weighting (a 14-day half-life). HTTP and
HTTPS visits to the same host share a site, as do `www` and bare-host forms;
nondefault ports remain distinct. Each tile opens that site's most recent
eligible address. Score, recency, and site key give deterministic ordering.

Slots stay fixed while you use the page. Background updates can clear a deleted
or dismissed target, but cannot move another target into its number. Reopen the
page or select Refresh to calculate a new arrangement. Existing history
retention, URL privacy exclusions, and history clearing apply. Native pages are
never recorded as browsing history or journey entries.

A tile's Options menu can dismiss its site or add it to Favourites. Manage
dismissed sites restores individual sites; Refresh then fills available slots.
Dismissing hides a suggestion without deleting its browsing history.

## Favourites

Eight fixed, numbered slots hold explicit choices. Add a URL and label, choose
an existing bookmark in the editor, or use Add to Favourites in the bookmark
library. From a web page, `:favourite-add` (also available as a browser action)
opens the slot chooser with the current page filled in. A Top Sites tile has
the same promotion action.

Favourites refer to ordinary bookmarks. Edit bookmark changes its URL/label in
both places. Unpin keeps the bookmark. Deleting the bookmark clears its slot.
Pinning an existing bookmark moves its favourite reference rather than making
a duplicate. The name entered when adding a favourite is saved to that bookmark
too, including when you choose an existing bookmark or type its address.
Replacing an occupied slot keeps its former bookmark. Move to
slot swaps two occupied slots or moves into an empty slot. Slots never compact
after removal. Saving shows errors and commits the change before refreshing
its stored presentation.

In the editor, Enter in Site address moves to Name; Enter in Name saves.
Holding Enter does not advance or submit repeatedly. The focused field scrolls
into view, and the footer wraps in narrow windows or with larger text.
Saving shows progress until the write and its metadata refresh finish. A
failed save leaves the form and its text available to retry; Show details
reveals selectable error text. Text entered during a save remains an unsaved
edit when that earlier save finishes. Close during saving keeps the draft;
Resume edit reopens it. Cancel after completion discards the local draft.
Each tab keeps its draft and save feedback when another tab saves a favourite.

Another Add to Favourites request waits while you edit or save the current
one. Save or cancel that edit, then use Review next in Favourites. Dismiss next
removes just the next waiting request. Up to sixteen requests can wait; another
request is rejected visibly if that list is full. Waiting requests belong to
the current tab and profile and are included in its unfinished-work close
check. Switching profiles clears them.

Bookmark choices follow their identity while metadata refreshes, keeping your
entered name and selection intact. If a selected bookmark is removed or its
address changes, saving waits for a choice: use the entered address, choose
another bookmark, or inspect and use the updated address. The entered name is
kept. The save checks the bookmark and the address shown in the form together.
Narrow windows with larger text use compact view controls and a scrolling
content area so the waiting requests remain reachable.

## Guide and keyboard help

Guide is a searchable manual. On wide windows, selecting a feature immediately
shows its full article beside the index; no separate reveal action is needed.
Input modes is the initial article when browsing the complete manual. Each
command appears once, with all its effective shortcuts and modes from your
configuration. Built-in search and Caret-mode gestures appear alongside the
configurable bindings, with their activation conditions in the binding notes.
Search matches keys, commands, feature names, and explanations;
the section filter narrows the index. Arrows/jk browse entries. Enter, Space,
or Right/l moves keyboard focus into the article; j/k and arrows then scroll,
Space pages down, Shift+Space pages up, and PgUp/PgDn scroll by a page. Left/h
or Escape returns focus to the index, keeping the article visible on wide
windows. On narrow windows, one click or Enter opens a full-width article;
All features returns to the selected entry in the index. Entries describe
the feature, complete configured invocation (including arguments and command
chains), syntax, examples, count
support, and exit/recovery. Command and binding details expand to show argument
syntax, counts, and binding resolution. Commands and examples can be selected
and copied; Escape returns keyboard focus to the reader. Unbound commands
remain discoverable. Binding notes show contributing layers and prefix
conflicts. Reading an entry never executes
its command. `g ?` opens the same guide as keyboard help.

## Startup, new tabs, and privacy

Startup and new tabs are independent settings:

```toml
[navigation]
start_pages = ["rb://start"]
new_tab = "rb://start"
```

Set either to `rb://blank`, `about:blank`, or an eligible URL if preferred.
Ctrl+t, `g a`, and the New Tab controls use `navigation.new_tab`; an explicit
startup address overrides `navigation.start_pages`. Back/Forward includes
native pages, and saved sessions, cloning, closed-tab undo, lazy restoration,
and discarded tabs preserve their native page descriptors.
In Normal mode, `H` (Shift+h) goes back and `L` (Shift+l) goes forward;
`:back` and `:forward` are also available. Pages that move through their own
history keep these shortcuts aligned with the current entry, including
different states that share a URL.

Each normal profile owns its preference, bookmarks, favourite slots, dismissed
sites, and cached icons. Windows in that profile read the shared metadata;
visible Top Sites remain stable. Private sessions begin with Guide and only
use their own temporary favourites, choices, and visits. Windows sharing a
private session share its temporary start-page state. Nothing is copied from
an ordinary profile, and temporary icon files are removed when the session owner
closes. Icons are captured from pages already visited; opening the start page
makes no icon-network requests. A neutral globe covers missing icons; only the
number keycaps represent shortcuts. Empty favourite slots show an add symbol
and a slot label, without a shortcut badge. The local
cache is bounded to 128 icons per profile/session.

Existing schema v3 databases receive the backed-up transactional upgrade
described in [the data policy](../../packaging/MIGRATIONS.md).
