//! Authored feature explanations shared by native help and generated references.

#[derive(Clone, Copy, Debug)]
pub struct GuideArticle {
    pub category: &'static str,
    pub explanation: &'static str,
    pub recovery: &'static str,
}

/// Every built-in command must have an authored article. Custom commands use
/// their configured invocation rather than pretending to document external code.
#[must_use]
// This explicit catalogue makes missing authored coverage a test failure.
#[allow(clippy::too_many_lines)]
pub fn guide_article(command: &str) -> Option<GuideArticle> {
    let (category, explanation, recovery) = match command {
        "tab-new" | "start-page" | "start-page-view" | "start-page-cycle" | "favourite-add" => (
            "Getting Started and Modes",
            "The start page has Top Sites, Favourites, and Guide views. Top Sites ranks recent and frequent visits; Favourites keeps eight explicitly chosen, bookmark-backed slots. Select a view to remember it for this profile. With a site grid focused, press 1–8 to open that slot, or move with arrows/hjkl and press Enter. Add the current page from the browser action menu or :favourite-add; choose a slot to replace when all eight are occupied. Private-session choices and favourites disappear when that session closes. Startup and new tabs can be configured independently.",
            "Escape closes an editor or menu and returns focus to the page. Use :open to navigate elsewhere.",
        ),
        "mode-enter" => (
            "Getting Started and Modes",
            "Ferric starts in Normal mode, where keys operate the browser. i enters Transient Insert: ordinary Enter reaches the page and returns to Normal; Shift+Enter keeps composing. I enters Locked Insert for repeated paragraphs. Neither shortcut chooses an arbitrary field: focus the field first, for example with gi. Ctrl+v enters Pass-through for webpage shortcuts; Ctrl+Shift+v or ;v forwards one key gesture. Caret mode navigates and selects document text without editing it. Configured bindings may replace these entry shortcuts.",
            "Escape exits Insert or Caret. Ctrl+Escape is the reserved recovery key for Pass-through; ordinary Escape is sent to the webpage.",
        ),
        "open" | "open-current" | "paste-open" | "tab-open" | "selection-search" => (
            "Opening Pages and Search",
            "Enter a full URL, a hostname, a named quickmark, a search-engine keyword followed by a query, or plain search text. Ferric resolves these in that order and encodes search text as data. open replaces the current tab; tab-open creates another. open-current uses the current safe URL. paste-open reads the clipboard only when invoked, and --primary selects the primary selection. --target tab opens a new tab; tab-bg opens without selecting it. selection-search uses selected page text with the configured search engine. Local paths require trusted local input.",
            "Escape cancels address entry. A normal navigation uses the existing link-cleaning and permission policies.",
        ),
        "back" | "forward" | "reload" | "stop" => (
            "Opening Pages and Search",
            "Back and Forward traverse this tab's navigation history, including native start-page entries. A supported numeric count repeats traversal. Reload loads the current document again; --bypass-cache requests a fresh engine load. Stop cancels a pending load. These are distinct from reopening a saved journey, which makes a new safe GET request rather than restoring form submissions.",
            "Use the opposite traversal direction to return. Stop can cancel a load; failed navigation retains the browser's recovery controls.",
        ),
        "search" | "search-next" => (
            "Opening Pages and Search",
            "Page search finds text within the current web document. / searches forward and ? searches backward; enter a query and press Enter. n advances to the next match and N reverses direction. Smart-case and explicit case settings determine matching. Counts advance through multiple matches where supported. This is separate from an address-bar search, which navigates to a search engine.",
            "Escape cancels search entry or clears the active search through the browser recovery path. Use o for an Internet search.",
        ),
        "url-clean" | "url-explain" | "link-cleaning-update" => (
            "Opening Pages and Search",
            "Ferric can remove recognised tracking parameters using its local cleaning rules. Explain previews the rule decisions; clean produces or opens the cleaned destination through the normal navigation path. Confirm a preview before navigation when confirmation is enabled. Updating rules requires the explicitly configured HTTPS source and checksum; it does not infer an update endpoint from a webpage.",
            "Escape closes the preview. Keep the original URL when the proposed change is unsuitable.",
        ),
        "tab-next" | "tab-prev" | "tab-select" | "tab-focus" | "tab-close" | "tab-undo"
        | "tab-clone" | "tab-move" | "tab-pin" | "tab-mute" | "tab-detach" | "tab-give"
        | "reopen-in-window" => (
            "Tabs and Windows",
            "Tab commands select, arrange, or change a specific tab. next/prev use tab order; select accepts a position, last, or previous where supported. Alt+1–8 selects tabs and Alt+9 selects the last tab. Pinning changes the tab's group, and muting controls audio. Closing follows the configured last-tab policy; undo reopens an eligible closed-tab descriptor. Clone and reopen use safe URL descriptors, not a replay of form submissions. Moving, giving, and detaching preserve a live tab where the native transfer path supports it.",
            "Use tab-undo after an accidental close. Select the previous tab with Ctrl+Tab; inspect command syntax before moving tabs between windows or contexts.",
        ),
        "tab-suspend" | "tab-discard" | "tab-resume" => (
            "Tabs and Windows",
            "Suspend freezes a hidden live tab to reduce activity. Discard releases its page resources while keeping a descriptor; resume restores resources and may reload a safe URL. The browser rejects unsafe targets and tabs with active operations. Activating an inactive tab requests resumption automatically. Native start-page descriptors remain native when resumed.",
            "Select or resume the tab to use it again. A discarded document may require a reload and cannot preserve arbitrary in-page JavaScript state.",
        ),
        "window-new" | "window-close" | "window-focus" | "window-move" | "fullscreen" | "quit" => (
            "Tabs and Windows",
            "Window commands create, activate, arrange, or close browser windows. window-new --private creates a temporary private session. Fullscreen toggles the native window presentation. Closing and quitting use the browser's shutdown decisions for active work and durable metadata; window movement uses the supported desktop integration and reports its actual outcome.",
            "Toggle fullscreen again to leave it. Escape dismisses a cancellable shutdown decision; reopening a window does not recover arbitrary unsaved webpage state.",
        ),
        "scroll" | "scroll-page" | "scroll-to" | "scroll-target" | "zoom" => (
            "Page Movement and Zoom",
            "Normal-mode hjkl scrolls left/down/up/right. Page movement supports full-page and half-page steps; a count multiplies commands that explicitly support counts. gg moves to the top and G to the bottom. scroll-target selects the scrolling container when a page has nested panels; auto restores automatic selection. Zoom changes document scale, with reset returning to the configured baseline. On the start page, focused grids use hjkl for tile selection instead of webpage scrolling.",
            "Select scroll-target auto to leave a manually chosen container. Reset zoom when text or controls become difficult to use.",
        ),
        "hint" => (
            "Hints and Grid",
            "Hints label eligible page elements. Type a label to select the target. f follows in the current tab; F opens a tab. --target tab-bg opens in the background, yank copies a link, and download requests a download. --first chooses the first matching target and supports an index. Rapid hints keep the interaction active for multiple targets. Use Tab/Shift+Tab to move among candidates, Backspace to correct a label, and the available text filter for crowded pages. `hints.proxy_targets = \"validated\"` opts into the narrowly checked editor and label/control pairings; it defaults to off and does not authorize selectors. Cross-origin frame qualification follows the engine's current limitations. For an explicit native pointer attempt, start `hint --target pointer FAMILY` from the local command bar or a direct local keyboard binding. Selecting a label only highlights it; press and release Enter to request one click through the active view. The target is rechecked first, and page response or permission-dependent behavior is not verified. `hint regions` and `hint surfaces` explicitly collect capped, visible structural regions and visual surfaces from the same retained target map; f remains the unchanged all-target family. Regions require named or semantic structure, except genuinely scrollable panes and validated editor representatives. Unnamed visual surfaces must be at least 16 by 16 CSS pixels. Region/surface selection opens a Rust-derived action palette: Hint within and Search within are available for a region or actual scroll pane, and only actually scrollable panes offer Set scroll target; only actually focusable surfaces offer Focus. Hint within and Search within collect visible eligible descendants, exclude the selected region, preserve page focus and scroll, and fail closed for stale or empty scopes without widening to the page. Generic-div size, hidden/offscreen buffers, closed roots and cross-origin frame children do not grant targets or authority; a cross-origin iframe can appear only as its visible exterior.",
            "Escape cancels hints without selecting anything. A rejected or uncertain pointer dispatch is terminal and is never retried; use ordinary hints for semantic activation.",
        ),
        "target-search" => (
            "Hints and Grid",
            "`target-search` opens a native palette over the current page's visible, bounded targets. Enter the query in the palette; optional scopes are links, controls, regions, surfaces, or all. Text ranking uses the same Hint matcher, and a result must be explicitly confirmed with Enter or Confirm before its existing Hint action path runs. Query and selection stay transient. Refresh keeps selection only while the same frame and element identity still has the same action meaning; a removed or virtualised replacement row cannot inherit it. The collector does not scroll the page or claim offscreen content. Search within reuses the same local, weakly retained Region lease and only searches descendants of the selected region.",
            "Escape cancels the palette without changing page selection, focus, or scroll. Run the command from local keyboard or UI input; remote IPC and CLI command execution are rejected.",
        ),
        "grid" | "grid-refine" | "grid-click" | "grid-double-click" | "grid-hover"
        | "grid-back" | "grid-reset" | "grid-help" | "grid-cancel" => (
            "Hints and Grid",
            "Grid navigation targets a visual position when element hints are unsuitable. Enter with ;g, press the number of the region containing your target, and repeat to refine the crosshair. Enter left-clicks, Shift+Enter right-clicks, Ctrl+Enter middle-clicks, and Space hovers. Alt+Enter requests a qualification-gated native double-click; it remains unavailable until native Wayland qualification passes. Backspace undoes one refinement; 0 resets the grid. ? shows the grid advisory. A changed document, geometry, or physical-pointer interaction can invalidate the session to prevent clicking a stale position.",
            "Escape cancels Grid. A held key does not repeatedly refine or click; release it before another action.",
        ),
        "caret-move" | "caret-select" | "caret-yank" | "yank" | "edit-text" => (
            "Text Entry and Webpage Shortcuts",
            "Caret mode moves through document text with directional, word, and line actions. Toggle selection, move to extend it, then yank to copy. The general yank command can copy a safe URL, title, or selection. edit-text sends the focused editable content to the explicitly configured editor and applies a completed result only while its document target is still valid. It does not silently choose a different text field.",
            "Escape exits Caret or cancels a pending text operation. Use i or I for webpage editing; Ctrl+Escape recovers from Pass-through.",
        ),
        "bookmark-add" | "bookmark-delete" | "bookmark-edit" | "bookmark-open"
        | "bookmark-list" | "quickmark-add" | "quickmark-delete" | "quickmark-edit"
        | "quickmark-open" | "quickmark-list" => (
            "Bookmarks and History",
            "Bookmarks save safe URLs under stable IDs with editable labels. Favourites refer to those same bookmarks: editing a favourite changes the bookmark, unpinning retains it, and deleting it clears its favourite slot. Quickmarks are case-sensitive named URL shortcuts; invoke quickmark-open or enter the name as an address. Use the library to inspect IDs before editing or deleting. Adding an ordinary bookmark does not automatically pin it to the start page.",
            "Escape closes a library or entry dialog. Unpinning is reversible by choosing the bookmark again; deleting a bookmark removes the stored entry.",
        ),
        "history" | "history-open" | "journey" | "journey-reopen" | "history-clear" => (
            "Bookmarks and History",
            "History lists eligible recorded visits for this profile. The journey graph connects page visits by navigation relationships and can be searched or expanded. Reopening a history or journey entry performs a new safe navigation. Clearing history requires the existing confirmation and can be scoped by time or origin; it also invalidates affected Top Sites tiles. Private history stays in its own session and is never used by ordinary-profile Top Sites.",
            "Escape closes the library or graph. History clearing cannot be undone by tab-undo; dismissed Top Sites can be restored without altering history.",
        ),
        "profile-list" | "profile-open" | "profile-create" | "profile-delete" | "context-list"
        | "context-create" | "context-delete" | "context-enter" | "context-save"
        | "context-route" => (
            "Profiles and Sessions",
            "Profiles isolate browser data and configuration. Each normal profile owns its own bookmarks, start-page preference, favourites, dismissals, and icons. Contexts select supported profile/workspace workflows and can route eligible explicit opens. Profile deletion uses a preview and the browser's existing safety rules. Private and ephemeral browsing cannot read ordinary-profile start-page state, and temporary choices are destroyed with their owner.",
            "Escape cancels a preview or routing decision. Review the selected profile before destructive operations; deletion does not have a tab-undo recovery.",
        ),
        "session-save" | "session-load" | "session-list" | "session-delete" => (
            "Profiles and Sessions",
            "Sessions save eligible tab descriptors, order, pinning, muting, scale, and supported scroll positions. Loading previews the stored session and restores safe GET destinations, including native start pages; unsafe navigation becomes a placeholder. Lazy restoration delays hidden page loads. Named sessions are distinct from the automatic crash-recovery checkpoint. Private-session state is not exported as durable profile metadata.",
            "Escape cancels the restore preview. Loading a descriptor cannot replay unsaved form data or arbitrary script state.",
        ),
        "download" | "downloads" | "download-open" | "download-show" | "download-cancel"
        | "download-pause" | "download-resume" | "download-retry" | "print-pdf" | "print"
        | "save-page" => (
            "Downloads and Permissions",
            "Downloads use the browser's destination chooser, collision policy, and manager. Pause/resume apply only when the engine supports the requested state; retry starts another eligible request. Open launches a completed file through the desktop, while show reveals its location. Cancel stops the transfer. Printing and saving use explicit user-selected destinations and retain their native progress and failure reporting.",
            "Escape dismisses a destination or print dialog. Cancel a pending download through the manager; opening a file is an explicit desktop action.",
        ),
        "permissions" | "permission-reset" | "blocking-status" | "blocking-toggle"
        | "blocklist-update" | "site-status" | "site-doctor" | "site-doctor-undo"
        | "site-data-clear" => (
            "Downloads and Permissions",
            "Permission prompts identify the requesting origin and requested capability. Inspect and reset decisions in the permission UI. Site Ledger shows active site settings and blocking evidence. Site Doctor applies a supported temporary experiment and can undo it; it is not a permanent security-policy bypass. Blocking updates use configured lists. Site-data clearing previews the affected origin and requires the established confirmation before removing engine-owned data.",
            "Escape cancels a prompt or preview. Use site-doctor-undo to revert an experiment and permission-reset to remove a stored decision.",
        ),
        "set"
        | "get"
        | "unset"
        | "config-export"
        | "config-edit"
        | "config-reload"
        | "config-check"
        | "config-write-defaults"
        | "theme-reload"
        | "bind"
        | "unbind" => (
            "Configuration and Advanced Commands",
            "Configuration is declarative and layered by global, profile, runtime, command-line, and temporary overrides. get inspects a value; set and unset change supported overrides. Validation checks types, scopes, and bindings before application. Some settings apply live and others require navigation or restart. bind assigns a command invocation to a mode/key sequence; unbind masks it. Export, editor, reload, and defaults commands follow their documented file and overwrite rules. The Guide always shows the effective binding map after changes.",
            "Escape cancels command entry or an editor decision. Remove an override with unset or restore a binding using bind; inspect config-check before reloading a changed file.",
        ),
        "spawn" | "script-run" | "jseval" | "send" | "view-source" | "devtools" => (
            "Configuration and Advanced Commands",
            "These are explicit development or external-tool actions. spawn and send use configured typed action targets; userscripts follow the installed script's declared capabilities and target checks. jseval evaluates only the explicitly requested JavaScript in the current web document. View Source opens the eligible source representation; DevTools inspects the engine page. Native start pages do not expose privileged application state through webpage scripts.",
            "Escape exits entry dialogs; close DevTools using its control or command. Inspect external-tool configuration before running it, and never assume an external process can be undone by Escape.",
        ),
        "repeat" | "cancel" | "macro-record" | "macro-stop" | "macro-play" | "command-execute"
        | "action" | "action-list" => (
            "Configuration and Advanced Commands",
            "Repeat reissues the last eligible browser command with supported count behavior. Macros record key gestures into named registers and replay through the normal resolver; stop recording before playback. Typed actions expose supported subject/verb operations with required arguments and availability checks. command-execute and chained commands use Ferric's command parser, not shell syntax. Cancel targets supported pending operations and does not roll back already committed work.",
            "Stop macro recording explicitly with macro-stop. Escape cancels command entry; use cancel for a supported pending operation.",
        ),
        "switcher" | "command-help" | "binding-list" | "binding-explain" | "learning-mode"
        | "help" | "version" | "diagnostics" => (
            "Configuration and Advanced Commands",
            "The switcher searches eligible tabs, commands, and profile library entries; scopes narrow the results. Keyboard help groups features and shows your active key sequences, including custom bindings and unbound commands. Binding Explain reports how a sequence resolves and its contributing layers; prefix conflicts depend on the configured timeout. Learning mode adds command feedback. Diagnostics and version report the installed browser and engine facts. Selecting a Guide explanation reads documentation and never runs its command.",
            "Escape closes the switcher or help and restores prior focus. Ctrl+Escape remains the reserved Pass-through recovery gesture.",
        ),
        _ => return None,
    };
    Some(GuideArticle {
        category,
        explanation,
        recovery,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_registered_feature_has_authored_manual_content() {
        for command in crate::CommandRegistry::default_v1().definitions() {
            let article = super::guide_article(&command.name)
                .unwrap_or_else(|| panic!("missing manual: {}", command.name));
            assert!(
                !command.examples.is_empty(),
                "missing examples: {}",
                command.name
            );
            assert!(!article.explanation.is_empty() && !article.recovery.is_empty());
        }
    }
}
