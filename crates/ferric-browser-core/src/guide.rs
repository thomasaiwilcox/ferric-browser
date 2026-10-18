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
        "edit" => (
            "Copy, Paste and External Tools",
            "edit copy, edit cut, edit paste, and edit paste-plain request the native edit operation in the retained focused document. Native Copy preserves supported formats; yank selection intentionally copies plain text. Standard clipboard shortcuts reach native editors by default even in Normal. Unavailable or stale sources are rejected without parent-document fallback. Requested means dispatched, not an engine acknowledgement of completion.",
            "Focus the desired control or select text again before retrying. binding-explain Ctrl+v identifies configuration overrides of the standard paste shortcut.",
        ),
        "mode-enter" => (
            "Getting Started and Modes",
            "Ferric starts in Normal mode, where keys operate the browser. i enters Transient Insert: ordinary Enter reaches the page and returns to Normal; Shift+Enter keeps composing. I enters Locked Insert for repeated paragraphs. Neither shortcut chooses an arbitrary field: focus the field first, for example with gi. Ctrl+v pastes and Ctrl+Shift+v pastes plain text. ;V enters Pass-through for webpage shortcuts; ;v forwards one key gesture. Caret mode navigates and selects document text without editing it. Configured bindings may replace these entry shortcuts.",
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
            "Suspend freezes a hidden tab to reduce activity. Discard releases its page resources while keeping its title, address and tab settings; Resume may reload a safe address. The browser protects tabs with active operations. Selecting a suspended or discarded tab requests resumption automatically. The start page stays a browser page when restored.",
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
            "Hints label visible links and controls. Type a label to select its target. f follows in the current tab; F opens a new tab. --target tab-bg opens in the background, yank copies a link, and download requests a download. --first chooses the first matching target and supports an index. Rapid hints let you select several targets.\n\nUse Tab/Shift+Tab to preview candidates, Backspace to correct a label, and the text filter on crowded pages. Hover over a label for a description. Space cycles overlapping badges in label mode; Ctrl+Space does the same in text mode. Labels stay stable while the page changes. Restart hints to include new controls that could not receive a label.\n\nUse hint regions to choose a named section or scrolling panel, or hint surfaces to choose a visible surface. Hint within and Search within restrict the next step to that section. Scrollable panels also offer Set scroll target; focusable surfaces offer Focus. These choices do not search offscreen content.\n\nFor embedded content, choose Hint inside frame when offered. If its controls cannot be labelled, use Focus with pass-through or Grid. The optional hints.proxy_targets = \"validated\" setting supports checked editor and label/control pairings; it defaults to off.\n\nFor a pointer click, run hint --target pointer FAMILY in the local command bar or through a keyboard binding. Selecting a label highlights the target; press and release Enter to request one click. Pointer hints may be unavailable in embedded content. A click request does not confirm that the page accepted it.",
            "Escape cancels without selecting a target. If a pointer click is rejected or its outcome is uncertain, Ferric stops without retrying it. Start ordinary hints to follow the link or activate the control.",
        ),
        "target-search" => (
            "Hints and Grid",
            "target-search opens a palette of visible page targets. Enter your query there; optional scopes are links, controls, regions, surfaces, or all. Select a result and confirm it with Enter or Confirm. Search text and selection are forgotten when the palette closes. Refresh retains your choice only while the same target and action are still available; otherwise choose again. The search does not scroll the page or include offscreen content. Search within restricts results to the section you chose.",
            "Escape cancels without changing page selection, focus or scroll. Open target-search from the browser's command bar or keyboard; CLI/IPC cannot interact with page targets.",
        ),
        "grid" | "grid-refine" | "grid-click" | "grid-double-click" | "grid-hover"
        | "grid-back" | "grid-reset" | "grid-help" | "grid-cancel" => (
            "Hints and Grid",
            "Grid navigation chooses a position when element hints are unsuitable. Enter with ;g, press the number of the region containing your target, and repeat to refine the crosshair. Enter left-clicks, Shift+Enter right-clicks, Ctrl+Enter middle-clicks, and Space hovers. Backspace undoes one refinement; 0 resets the grid. ? shows the controls. Navigation, resizing or using the mouse can cancel the grid to prevent a click at the wrong position. Double-click is currently unavailable; use a mouse or the page's keyboard controls.",
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
        "profiles" | "profile-list" | "profile-open" | "profile-create" | "profile-delete" => (
            "Profiles and Sessions",
            "Use profiles to open the profile manager; profile-list lists profile names for scripts. Profiles keep cookies, browsing history, bookmarks and settings separate. Contexts combine a profile with supported window and workspace workflows. Review the deletion preview before removing a profile. Private browsing keeps its start-page choices separate and forgets them when the private session closes.",
            "Escape cancels a profile preview. Review the profile before deleting it; deletion cannot be undone with tab-undo.",
        ),
        "context-list" | "context-create" | "context-enter" | "context-save" => (
            "Profiles and Sessions",
            "Contexts group a task's tabs with an existing profile and optional workspace. Use :context-list to choose one, or :context-create NAME --profile PROFILE to create one. :context-enter NAME enters it; :context-save saves the current window's tab membership. A context never combines cookies or storage from separate profiles. You can also define contexts in contexts.toml and apply changes with :config-reload.",
            "Escape cancels a context choice. Saved membership cannot restore unsaved form entries or a running web app. Use :context-delete NAME --confirm to remove a command-created context; remove file-defined contexts in contexts.toml.",
        ),
        "context-delete" => (
            "Profiles and Sessions",
            "Use :context-delete NAME --confirm to remove a context created with :context-create. Open pages, the profile and named sessions are kept. For a context defined in contexts.toml, remove its definition and any routes that use it from that file, then run :config-reload. Direct deletion keeps a context defined in that file and its saved tabs, and explains how to remove it.",
            "Context deletion removes saved tab membership and cannot be undone with tab-undo. Review the context name before confirming.",
        ),
        "context-route" => (
            "Profiles and Sessions",
            "Context routes suggest or prompt for a context when opening a matching address through a supported browser command. Use :context-route list to review rules, :context-route add PATTERN CONTEXT to add one, and :context-route remove ROUTE_ID to remove one. Route changes are saved in contexts.toml and apply to pages opened next. Redirects, submitted forms and authentication already in progress are not moved to another context.",
            "Escape dismisses a routing choice. Removing a route keeps its context and pages already open. When removing a context from contexts.toml, also remove routes that refer to it before reloading settings.",
        ),
        "sessions" | "session-save" | "session-load" | "session-list" | "session-delete" => (
            "Profiles and Sessions",
            "Use sessions to preview, restore or delete saved groups of tabs; session-list lists their names for scripts. Sessions retain tab order, pinned and muted state, zoom, and supported scroll positions. Restoring opens saved page addresses and start pages without replaying forms. Hidden pages can load when selected. Named sessions are separate from automatic crash recovery. Private browsing is not saved here.",
            "Escape cancels the restore preview. Unsaved form entries and the running state of a web app cannot be restored from a saved session.",
        ),
        "settings" => (
            "Configuration and Troubleshooting",
            "Open the settings panel to search by setting name or configuration key. Choose a readable value or edit a field, then apply it. Session-only changes are forgotten when Ferric closes; saved changes keep your config.toml intact. Settings show when a change takes effect. Use site-rule commands for settings that apply to one site.",
            "Escape closes settings. Reset returns a setting to its inherited value.",
        ),
        "download" | "download-retry" => (
            "Downloads and Permissions",
            "Download an HTTP or HTTPS link without leaving the current page. Retry starts a new request for an interrupted or cancelled item using this window's profile. Downloads shows preparation and transfer progress. A retry stays unavailable while that item's replacement is preparing or running.",
            "Use Cancel request in Downloads while waiting for a site to start, or Cancel once a transfer is active. Private browsing keeps downloaded files but does not save download history.",
        ),
        "downloads" => (
            "Downloads and Permissions",
            "View saved and active downloads for this window's profile. Known transfers stay usable while saved history is updating. Open or Show in folder requires a completed file. Details reveals the download ID for commands.",
            "Close or Escape dismisses the panel. Cancel request stops preparation; Cancel stops an active transfer. Retry is available after interruption or cancellation.",
        ),
        "download-open" | "download-show" | "download-cancel" | "download-pause"
        | "download-resume" => (
            "Downloads and Permissions",
            "Open a completed file or reveal its containing folder with the desktop. Pause, resume and cancel apply to active transfers when their current state permits the action. Use Downloads to see progress and available controls.",
            "Closing Downloads keeps transfers running. Opening a file is an explicit desktop action.",
        ),
        "print-pdf" | "print" => (
            "Downloads and Permissions",
            "Print the current page, or use print-pdf to write it to an explicit PDF destination. The browser reports completion or a failure so you can check the result.",
            "Escape dismisses a print dialog without printing.",
        ),
        "save-page" => (
            "Downloads and Permissions",
            "Save the current page to the chosen destination. The browser reports progress and any failure through Downloads.",
            "Escape dismisses a destination dialog. Cancel an active save in Downloads.",
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
