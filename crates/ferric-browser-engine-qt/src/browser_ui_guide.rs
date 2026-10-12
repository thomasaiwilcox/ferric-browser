//! Authored manual articles joined with the effective binding and command registries.
use super::{CxxQtType, Pin, QString, QStringList, ipc_mode_name, qobject};
use ferric_browser_core::{CountPolicy, ParseInput, guide_article, parse_chain};

const CATEGORIES: [&str; 10] = [
    "Getting Started and Modes",
    "Opening Pages and Search",
    "Tabs and Windows",
    "Page Movement and Zoom",
    "Hints and Grid",
    "Text Entry and Webpage Shortcuts",
    "Bookmarks and History",
    "Profiles and Sessions",
    "Downloads and Permissions",
    "Configuration and Advanced Commands",
];

struct GuideRow {
    category: String,
    title: String,
    keys: String,
    mode: String,
    command: String,
    summary: String,
    details: String,
    identity: String,
    explanation: String,
    arguments: String,
    examples: String,
    recovery: String,
    count: String,
    binding_notes: String,
}

fn feature_title(command: &str, description: &str) -> String {
    match command {
        "tab-new" => "New tabs",
        "start-page" => "Your start page",
        "start-page-view" => "Choose a start page view",
        "start-page-cycle" => "Switch start page views",
        "favourite-add" => "Add a favourite",
        "open" => "Open a page or search",
        "open-current" => "Reopen the current address",
        "paste-open" => "Open from the clipboard",
        "back" => "Go back",
        "forward" => "Go forward",
        "reload" => "Reload a page",
        "stop" => "Stop loading",
        "zoom" => "Page zoom",
        "search-next" => "Next and previous search matches",
        "search" => "Find text on a page",
        "scroll" => "Scroll a page",
        "scroll-page" => "Scroll by a page",
        "scroll-to" => "Jump to the top or bottom",
        "scroll-target" => "Choose where to scroll",
        "window-close" => "Close a window",
        "window-new" => "New and private windows",
        "fullscreen" => "Fullscreen",
        "quit" => "Quit Ferric",
        "set" => "Change a setting",
        "get" => "Inspect a setting",
        "unset" => "Reset a setting",
        "config-export" => "Export your configuration",
        "config-edit" => "Edit your configuration",
        "config-reload" => "Reload your configuration",
        "config-check" => "Check your configuration",
        "config-write-defaults" => "Create a default configuration",
        "theme-reload" => "Refresh your theme",
        "bind" => "Customise a shortcut",
        "unbind" => "Remove a shortcut",
        "hint" => "Follow links with hints",
        "grid" => "Spatial Grid mode",
        "grid-refine" => "Refine the Grid region",
        "grid-click" => "Click with Grid",
        "grid-double-click" => "Double-click with Grid (unavailable pending qualification)",
        "grid-hover" => "Hover with Grid",
        "grid-back" => "Go back a Grid step",
        "grid-reset" => "Reset the Grid region",
        "grid-help" => "Grid mode help",
        "grid-cancel" => "Leave Grid mode",
        "download" => "Download a URL",
        "print-pdf" => "Export a PDF",
        "print" => "Print a page",
        "save-page" => "Save a page",
        "view-source" => "View page source",
        "devtools" => "Developer tools",
        "downloads" => "Manage downloads",
        "permissions" => "Site permissions",
        "blocking-status" => "Inspect network blocking",
        "site-status" => "Site privacy and activity",
        "site-doctor" => "Troubleshoot a site",
        "site-doctor-undo" => "Undo a site experiment",
        "site-data-clear" => "Clear a site's data",
        "blocklist-update" => "Update blocking lists",
        "link-cleaning-update" => "Update link cleaning rules",
        "blocking-toggle" => "Turn network blocking on or off",
        "permission-reset" => "Revoke a site permission",
        "download-open" => "Open a download",
        "download-show" => "Show a download in its folder",
        "download-cancel" => "Cancel a download",
        "download-pause" => "Pause a download",
        "download-resume" => "Resume a download",
        "download-retry" => "Retry a download",
        "mode-enter" => "Input modes",
        "caret-move" => "Move through document text",
        "caret-select" => "Select document text",
        "yank" => "Copy an address or browser value",
        "caret-yank" => "Copy selected document text",
        "edit-text" => "Use an external text editor",
        "spawn" => "Run an external program",
        "script-run" => "Run an installed userscript",
        "jseval" => "Run JavaScript",
        "send" => "Send to an external application",
        "repeat" => "Repeat the last command",
        "cancel" => "Cancel pending work",
        "macro-record" => "Record a macro",
        "macro-stop" => "Stop recording a macro",
        "macro-play" => "Play a macro",
        "action" => "Run a browser action",
        "action-list" => "Explore browser actions",
        "switcher" => "The universal switcher",
        "command-help" => "Help for a command",
        "binding-list" => "The Ferric Guide",
        "binding-explain" => "Explain a shortcut",
        "learning-mode" => "Shortcut learning feedback",
        "command-execute" => "Execute a command by ID",
        "url-clean" => "Preview a cleaned address",
        "url-explain" => "Explain link cleaning",
        "tab-undo" => "Reopen a closed tab",
        "tab-clone" => "Duplicate a tab",
        "reopen-in-window" => "Reopen a page in another window",
        "tab-detach" => "Move a tab to a new window",
        "tab-give" => "Move a tab to another window",
        "bookmark-add" => "Bookmark a page",
        "bookmark-delete" => "Delete a bookmark",
        "bookmark-edit" => "Rename a bookmark",
        "bookmark-open" => "Open a bookmark",
        "bookmark-list" => "Your bookmarks",
        "quickmark-add" => "Create a named URL shortcut",
        "quickmark-delete" => "Delete a named URL shortcut",
        "quickmark-edit" => "Edit a named URL shortcut",
        "quickmark-open" => "Open a named URL shortcut",
        "quickmark-list" => "Your named URL shortcuts",
        "history" => "Browsing history",
        "history-open" => "Open a history entry",
        "journey" => "Your browsing journey",
        "journey-reopen" => "Reopen a journey entry",
        "history-clear" => "Clear browsing history",
        "tab-open" => "Open an address in a new tab",
        "tab-next" => "Select the next tab",
        "tab-prev" => "Select the previous tab",
        "tab-select" => "Choose a tab",
        "tab-focus" => "Focus a tab by ID",
        "tab-close" => "Close a tab",
        "tab-suspend" => "Suspend a tab",
        "tab-discard" => "Release a tab's resources",
        "tab-resume" => "Resume a tab",
        "tab-move" => "Reorder a tab",
        "tab-pin" => "Pin a tab",
        "tab-mute" => "Mute a tab",
        "selection-search" => "Search selected text",
        "window-focus" => "Focus another window",
        "window-move" => "Move a window to a workspace",
        "session-save" => "Save a session",
        "session-load" => "Restore a saved session",
        "session-list" => "Your saved sessions",
        "session-delete" => "Delete a saved session",
        "profile-list" => "Your profiles",
        "profile-open" => "Open a profile",
        "profile-create" => "Create a profile",
        "profile-delete" => "Delete a profile",
        "context-list" => "Your browsing contexts",
        "context-create" => "Create a browsing context",
        "context-delete" => "Delete a browsing context",
        "context-enter" => "Switch browsing contexts",
        "context-save" => "Save a browsing context",
        "context-route" => "Route pages to contexts",
        "help" => "Command and setting help",
        "version" => "About Ferric",
        "diagnostics" => "Runtime diagnostics",
        _ => description.trim_end_matches('.'),
    }
    .into()
}

// These gestures are handled directly by the window input controller, rather
// than the configurable binding resolver. Keep them visible in the same reader.
fn built_in_gestures(command: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    match command {
        "search" => &[
            ("search <query>", "/", "normal"),
            ("search <query> --backward", "?", "normal"),
        ],
        "search-next" => &[
            ("search-next", "n", "normal"),
            ("search-next --backward", "N", "normal"),
        ],
        "caret-move" => &[
            ("caret-move left", "h", "caret"),
            ("caret-move down", "j", "caret"),
            ("caret-move up", "k", "caret"),
            ("caret-move right", "l", "caret"),
            ("caret-move word-next", "w", "caret"),
            ("caret-move word-prev", "b", "caret"),
            ("caret-move line-start", "0", "caret"),
            ("caret-move line-end", "$", "caret"),
        ],
        "caret-select" => &[("caret-select toggle", "v", "caret")],
        "caret-yank" => &[("caret-yank", "y", "caret")],
        _ => &[],
    }
}

impl qobject::BrowserUi {
    pub(super) fn refresh_guide(mut self: Pin<&mut Self>, search: &QString) {
        let query = search.to_string().to_ascii_lowercase();
        let bindings = self
            .as_ref()
            .rust()
            .bindings
            .as_ref()
            .map(super::BindingResolver::definitions)
            .unwrap_or_default();
        let registry = self.as_ref().rust().registry.clone();
        let mut rows: Vec<GuideRow> = Vec::new();
        for command in registry.definitions() {
            let Some(article) = guide_article(&command.name) else {
                continue;
            };
            let title = feature_title(&command.name, &command.description);
            let variants: Vec<_> = bindings
                .iter()
                .filter(|binding| {
                    parse_chain(&binding.command, ParseInput::Interactive).is_ok_and(|chain| {
                        chain.iter().any(|invocation| {
                            registry
                                .resolve(&invocation.name)
                                .is_ok_and(|resolved| resolved.name == command.name)
                        })
                    })
                })
                .collect();
            let mut invocations: Vec<(String, String, String)> = variants
                .iter()
                .map(|binding| {
                    (
                        binding.command.clone(),
                        binding.keys.join(" "),
                        ipc_mode_name(binding.mode).to_owned(),
                    )
                })
                .collect();
            let gestures = built_in_gestures(&command.name);
            invocations.extend(gestures.iter().map(|(invocation, keys, mode)| {
                ((*invocation).into(), (*keys).into(), (*mode).into())
            }));
            if invocations.is_empty() {
                invocations.push((
                    command.name.clone(),
                    "Unbound — use command entry".into(),
                    command
                        .modes
                        .iter()
                        .map(|mode| ipc_mode_name(*mode))
                        .collect::<Vec<_>>()
                        .join(", "),
                ));
            }
            let mut feature_rows = Vec::new();
            let mut matches_query = query.is_empty();
            for (invocation, keys, mode) in invocations {
                let count = match command.count {
                    CountPolicy::NotSupported => {
                        "This command does not accept a numeric count.".into()
                    }
                    CountPolicy::Supported { maximum } => {
                        format!("A numeric prefix repeats or scales this action, up to {maximum}.")
                    }
                };
                let examples = command.examples.join("\n");
                let arguments = command
                    .arguments
                    .iter()
                    .map(|argument| {
                        format!(
                            "{}: {:?}{}",
                            argument.name,
                            argument.kind,
                            if argument.required { " (required)" } else { "" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let mut binding_notes = variants.iter().filter(|binding| binding.command == invocation && ipc_mode_name(binding.mode) == mode).map(|binding| {
                    let mut notes = vec![format!("Active keys: {}", binding.keys.join(" "))];
                    let layers = self.as_ref().binding_layer_provenance(&mode, &binding.keys.concat());
                    if let Some(layers) = layers.as_array() {
                        for layer in layers { notes.push(format!("{} layer: {}", layer["layer"].as_str().unwrap_or("configuration"), layer["status"].as_str().unwrap_or("contributing"))); }
                    }
                    for other in &bindings {
                        if other.mode == binding.mode && other.keys != binding.keys && (other.keys.starts_with(&binding.keys) || binding.keys.starts_with(&other.keys)) {
                            notes.push(format!("Prefix conflict with {}: the configured keychain timeout decides the shorter match.", other.keys.join(" ")));
                        }
                    }
                    notes.join("\n")
                }).collect::<Vec<_>>().join("\n");
                if gestures.iter().any(|gesture| {
                    gesture.0 == invocation && gesture.1 == keys && gesture.2 == mode
                }) {
                    if !binding_notes.is_empty() {
                        binding_notes.push('\n');
                    }
                    binding_notes.push_str(match command.name.as_str() {
                        "search" => "Built-in search-entry gesture. Type a query and press Enter. A question mark continues a pending key chain instead when one is active.",
                        "search-next" => "Built-in gesture while a page search is active. Numeric counts are available through command entry with --count.",
                        _ => "Built-in Caret-mode gesture, handled before configurable bindings.",
                    });
                }
                let details = format!(
                    "{}\n\nConfigured invocation\n:{invocation}\n\nModes: {mode}\n{count}\n\nArguments\n{}\n\nExamples\n{}\n\nExit and recovery\n{}",
                    article.explanation,
                    if arguments.is_empty() {
                        "No arguments"
                    } else {
                        &arguments
                    },
                    if examples.is_empty() {
                        &invocation
                    } else {
                        &examples
                    },
                    article.recovery
                );
                let details = format!("{details}\n\nBinding resolution\n{binding_notes}");
                let searchable = format!(
                    "{} {title} {invocation} {keys} {mode} {} {details}",
                    article.category, command.description
                )
                .to_ascii_lowercase();
                matches_query |= searchable.contains(&query);
                feature_rows.push(GuideRow {
                    category: article.category.into(),
                    title: title.clone(),
                    keys,
                    mode,
                    command: invocation,
                    summary: command.description.clone(),
                    details,
                    identity: command.name.clone(),
                    explanation: article.explanation.into(),
                    arguments: arguments.clone(),
                    examples: examples.clone(),
                    recovery: article.recovery.into(),
                    count,
                    binding_notes,
                });
            }
            if matches_query {
                rows.extend(feature_rows);
            }
        }
        let recovery = GuideRow {
            category: "Getting Started and Modes".into(),
            title: "Recover from webpage shortcuts".into(),
            keys: "Ctrl+Escape".into(),
            mode: "pass-through, pass-through-once".into(),
            command: "Reserved recovery gesture".into(),
            summary: "Return to Normal mode from Pass-through.".into(),
            details: "Ctrl+Escape is reserved and always returns to Normal. Ordinary Escape is sent to the webpage while Pass-through is active. Pass Next Key forwards one complete key gesture, including repeats and release, then returns to Normal.".into(),
            identity: "reserved-recovery".into(),
            explanation: "Ctrl+Escape always returns to Normal mode. Ordinary Escape is sent to the webpage while Pass-through is active. Pass Next Key forwards one complete key gesture, including repeats and release, then returns to Normal.".into(),
            arguments: String::new(),
            examples: String::new(),
            recovery: "Press Ctrl+Escape to return to Normal mode.".into(),
            count: "This gesture does not accept a numeric count.".into(),
            binding_notes: "This recovery gesture is reserved, even when bindings are customised.".into(),
        };
        if query.is_empty()
            || format!(
                "{} {} {} {} {}",
                recovery.category,
                recovery.title,
                recovery.keys,
                recovery.mode,
                recovery.explanation
            )
            .to_ascii_lowercase()
            .contains(&query)
        {
            rows.push(recovery);
        }
        rows.sort_by(|left, right| {
            CATEGORIES
                .iter()
                .position(|category| *category == left.category)
                .cmp(
                    &CATEGORIES
                        .iter()
                        .position(|category| *category == right.category),
                )
                .then_with(|| {
                    (left.identity != "mode-enter").cmp(&(right.identity != "mode-enter"))
                })
                .then_with(|| left.title.cmp(&right.title))
                .then_with(|| left.mode.cmp(&right.mode))
        });
        macro_rules! project {
            ($setter:ident, $field:tt) => {
                self.as_mut().$setter(
                    rows.iter()
                        .map(|row| QString::from(&row.$field))
                        .collect::<QStringList>(),
                );
            };
        }
        project!(set_guide_categories, category);
        project!(set_guide_titles, title);
        project!(set_guide_keys, keys);
        project!(set_guide_modes, mode);
        project!(set_guide_commands, command);
        project!(set_guide_summaries, summary);
        project!(set_guide_ids, identity);
        project!(set_guide_explanations, explanation);
        project!(set_guide_arguments, arguments);
        project!(set_guide_examples, examples);
        project!(set_guide_recoveries, recovery);
        project!(set_guide_counts, count);
        project!(set_guide_binding_notes, binding_notes);
        project!(set_guide_details, details);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_input_gestures_have_registered_manual_commands() {
        let registry = ferric_browser_core::CommandRegistry::default_v1();
        for command in [
            "search",
            "search-next",
            "caret-move",
            "caret-select",
            "caret-yank",
        ] {
            let gestures = built_in_gestures(command);
            assert_ne!(gestures, []);
            for (invocation, _, mode) in gestures {
                let chain = parse_chain(invocation, ParseInput::Interactive).unwrap();
                assert_eq!(chain.len(), 1);
                let definition = registry.resolve(&chain[0].name).unwrap();
                assert_eq!(definition.name, command);
                assert!(
                    definition
                        .modes
                        .iter()
                        .any(|value| ipc_mode_name(*value) == *mode)
                );
                assert!(guide_article(command).is_some());
            }
        }
        // Prompt and document-selection shortcuts are browser gestures even
        // when no configurable binding exists for their command.
        assert_eq!(built_in_gestures("search")[0].1, "/");
        assert_eq!(built_in_gestures("caret-select")[0].1, "v");
    }
}
