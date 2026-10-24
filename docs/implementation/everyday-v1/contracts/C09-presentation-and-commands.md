# C09 — Two presentations, one browser

`tabs.presentation` is `internal` by default or `windows`; global startup/restart scoped. Existing hosts do not live-convert. A pending settings change is displayed honestly; it does not partially change open routing. All profiles/private windows inherit the process presentation. Do not reinterpret tab-strip visibility as the presentation setting.

In windows mode, one ordinary page occupies one native window. Preserve TabId/page state and the proper core window model; no secret tab list under the original primary page. Engine renderer processes remain Qt-managed; no browser process per tab. Internal mode, its shortcuts and defaults remain unchanged.

## Command semantics

| Intent | Windows presentation |
|---|---|
| Ordinary new-tab/open target tab | New page host, optional grouping relative to captured source; foreground only when requested. |
| tab-bg | No activation/focus theft. Exact backend positive path must be qualified; otherwise create a standalone nonactivated page only if that path is supported, or report unavailable rather than silently foreground it. |
| Explicit new window | Standalone page host; no group mutation on unrelated windows. |
| current | Navigate exact captured current page, not another active window. |
| tab-next/prev/index | Current observed Ferric group/order; when ungrouped, documented same profile/privacy/context scope fallback, never all desktop apps. |
| close tab | Close exactly one browser page through existing beforeunload/download decisions. Never close the group or application implicitly. |
| reopen/undo | Existing saved page state; no unsupported DOM/form/POST restoration claim. |
| pin | Browser protection/pinning meaning, not compositor stickiness. |
| move tab | Defined movement within Ferric scope or a validated backend operation; unsupported movement is explicit, not a shell guess. |
| quit/window-close | Preserve explicitly documented existing scopes; new mode does not silently widen destructive targets. |

Respect source provenance: direct keyboard, explicit browser UI, external CLI and engine popup are not interchangeable. Use the captured request schema. Popup requests use actual Qt request handoff and opener/context/profile; they do not join arbitrary groups or consume the hot spare in v1. Native dialogs and DevTools are never mistaken for ordinary page windows.

Use reusable existing HintWorkspace, PageSearch, modal input and tab controllers. Window mode must support normal hints, grid, target search, scrolling, editing, native prompts and everyday keyboard chrome, not merely page rendering. Compare behaviour in primary/secondary/popup hosts and after closing the original visible host.
