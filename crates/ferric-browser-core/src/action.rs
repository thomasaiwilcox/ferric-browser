use crate::{ArgumentDefinition, ArgumentKind, CommandRegistry, EffectClass};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionSubject {
    Url,
    Link,
    Tab,
    Selection,
    Download,
    Context,
    Window,
    HistoryEntry,
    Bookmark,
    Quickmark,
    Session,
    Command,
}

impl ActionSubject {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::Link => "link",
            Self::Tab => "tab",
            Self::Selection => "selection",
            Self::Download => "download",
            Self::Context => "context",
            Self::Window => "window",
            Self::HistoryEntry => "history-entry",
            Self::Bookmark => "bookmark",
            Self::Quickmark => "quickmark",
            Self::Session => "session",
            Self::Command => "command",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionSource {
    Ui,
    Hint,
    Ipc,
    Switcher,
    Userscript,
}

impl ActionSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ui => "ui",
            Self::Hint => "hint",
            Self::Ipc => "ipc",
            Self::Switcher => "switcher",
            Self::Userscript => "userscript",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionConfirmation {
    Never,
    RequiredWhenChanged,
}

impl ActionConfirmation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::RequiredWhenChanged => "when-changed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionDefinition {
    pub id: String,
    pub subject: ActionSubject,
    pub verb: String,
    pub label: String,
    pub description: String,
    pub command: String,
    pub arguments: Vec<ArgumentDefinition>,
    pub examples: Vec<String>,
    pub sources: Vec<ActionSource>,
    pub effect: EffectClass,
    pub confirmation: ActionConfirmation,
    pub sensitive: bool,
}

impl ActionDefinition {
    /// Returns the bounded completion provider name for this action's typed
    /// arguments. Providers are descriptive registry metadata; they never
    /// parse or execute command text.
    #[must_use]
    pub fn completion_provider(&self) -> &'static str {
        if self.arguments.is_empty() {
            return "none";
        }
        if self
            .arguments
            .iter()
            .any(|argument| argument.kind == ArgumentKind::Url)
        {
            return "url";
        }
        match self.subject {
            ActionSubject::Tab => "live-tab",
            ActionSubject::Window => "live-window",
            ActionSubject::Context => "profile-context",
            ActionSubject::HistoryEntry => "history-entry",
            ActionSubject::Bookmark => "bookmark",
            ActionSubject::Quickmark => "quickmark",
            ActionSubject::Session => "session",
            ActionSubject::Download => "download",
            ActionSubject::Command => "command",
            ActionSubject::Selection | ActionSubject::Url | ActionSubject::Link => "text",
        }
    }

    /// Returns the stable availability predicate category used by discovery.
    /// The Qt adapter evaluates the category against current state and
    /// capability data immediately before execution.
    #[must_use]
    pub const fn availability_predicate(&self) -> &'static str {
        match self.subject {
            ActionSubject::Url => "current-document",
            ActionSubject::Link => "captured-link",
            ActionSubject::Selection => "live-selection",
            ActionSubject::Tab => "live-tab",
            ActionSubject::Window => "live-window",
            ActionSubject::Context => "profile-context",
            ActionSubject::Download => "download-state",
            ActionSubject::HistoryEntry => "stored-history",
            ActionSubject::Bookmark => "stored-bookmark",
            ActionSubject::Quickmark => "stored-quickmark",
            ActionSubject::Session => "stored-session",
            ActionSubject::Command => "command-registry",
        }
    }

    /// Returns the static capabilities required before this action can be
    /// offered or executed. Runtime availability remains adapter-owned, but
    /// the capability vocabulary is shared with every action consumer.
    #[must_use]
    pub fn required_capabilities(&self) -> &'static [&'static str] {
        match self.command.as_str() {
            "send" => &["configured-action-target"],
            "selection-search" => &["live-document"],
            "download-open" | "download-show" | "download-cancel" | "download-pause"
            | "download-resume" | "download-retry" => &["durable-download-index"],
            "history-open" | "history-clear" | "bookmark-add" | "bookmark-open"
            | "bookmark-delete" | "bookmark-edit" | "quickmark-add" | "quickmark-open"
            | "quickmark-delete" | "quickmark-edit" | "bookmark-list" | "quickmark-list"
            | "session-save" | "session-load" | "session-list" | "session-delete" => {
                &["durable-profile-storage"]
            }
            "context-enter" | "context-save" => &["durable-context-registry"],
            "window-move" => &["compositor-window-move"],
            "tab-detach" | "tab-give" => &["live-window-reparent"],
            "command-help" | "command-execute" => &["command-registry"],
            _ => &[],
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActionRegistry {
    definitions: Vec<ActionDefinition>,
}

impl ActionRegistry {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn default_v1() -> Self {
        let text_url = ArgumentDefinition {
            name: "url".into(),
            kind: ArgumentKind::Url,
            required: false,
        };
        let open_target = ArgumentDefinition {
            name: "target".into(),
            kind: ArgumentKind::Enum,
            required: false,
        };
        let sources = vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher];
        Self {
            definitions: vec![
                ActionDefinition {
                    id: "browser.url.open".into(),
                    subject: ActionSubject::Url,
                    verb: "open".into(),
                    label: "Open URL".into(),
                    description: "Navigate to a captured URL or search input.".into(),
                    command: "open".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "input".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }, open_target.clone()],
                    examples: vec![
                        "action url open https://example.test".into(),
                        "action url open --target tab-bg https://example.test".into(),
                    ],
                    sources: sources.clone(),
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.link.open".into(),
                    subject: ActionSubject::Link,
                    verb: "open".into(),
                    label: "Open link".into(),
                    description: "Navigate to a validated captured page link.".into(),
                    command: "open".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "url".into(),
                            kind: ArgumentKind::Url,
                            required: true,
                        },
                        open_target,
                    ],
                    examples: vec![
                        "action link open https://example.test/docs".into(),
                        "action link open --target tab-bg https://example.test/docs".into(),
                    ],
                    sources: vec![
                        ActionSource::Ui,
                        ActionSource::Hint,
                        ActionSource::Ipc,
                        ActionSource::Switcher,
                    ],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.url.copy".into(),
                    subject: ActionSubject::Url,
                    verb: "copy".into(),
                    label: "Copy URL".into(),
                    description: "Copy the current URL in safe display form.".into(),
                    command: "yank".into(),
                    arguments: Vec::new(),
                    examples: vec!["action url copy".into()],
                    sources: sources.clone(),
                    effect: EffectClass::Sensitive,
                    confirmation: ActionConfirmation::Never,
                    sensitive: true,
                },
                ActionDefinition {
                    id: "browser.url.clean-copy".into(),
                    subject: ActionSubject::Url,
                    verb: "clean-copy".into(),
                    label: "Copy cleaned URL".into(),
                    description: "Copy the current URL after conservative cleaning.".into(),
                    command: "yank".into(),
                    arguments: Vec::new(),
                    examples: vec!["action url clean-copy".into()],
                    sources: sources.clone(),
                    effect: EffectClass::Sensitive,
                    confirmation: ActionConfirmation::Never,
                    sensitive: true,
                },
                ActionDefinition {
                    id: "browser.link.copy".into(),
                    subject: ActionSubject::Link,
                    verb: "copy".into(),
                    label: "Copy link".into(),
                    description: "Copy a validated page link in safe display form.".into(),
                    command: "yank".into(),
                    arguments: vec![text_url.clone()],
                    examples: vec!["action link copy https://example.test/docs".into()],
                    sources: vec![
                        ActionSource::Ui,
                        ActionSource::Hint,
                        ActionSource::Ipc,
                        ActionSource::Switcher,
                    ],
                    effect: EffectClass::Sensitive,
                    confirmation: ActionConfirmation::Never,
                    sensitive: true,
                },
                ActionDefinition {
                    id: "browser.link.clean-copy".into(),
                    subject: ActionSubject::Link,
                    verb: "clean-copy".into(),
                    label: "Copy cleaned link".into(),
                    description: "Copy a validated page link after conservative cleaning.".into(),
                    command: "yank".into(),
                    arguments: vec![text_url.clone()],
                    examples: vec!["action link clean-copy https://example.test/docs".into()],
                    sources: vec![
                        ActionSource::Ui,
                        ActionSource::Hint,
                        ActionSource::Ipc,
                        ActionSource::Switcher,
                    ],
                    effect: EffectClass::Sensitive,
                    confirmation: ActionConfirmation::Never,
                    sensitive: true,
                },
                ActionDefinition {
                    id: "browser.link.download".into(),
                    subject: ActionSubject::Link,
                    verb: "download".into(),
                    label: "Download link".into(),
                    description: "Download a validated page link.".into(),
                    command: "download".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "url".into(),
                        kind: ArgumentKind::Url,
                        required: true,
                    }],
                    examples: vec!["action link download https://example.test/file.zip".into()],
                    sources: vec![
                        ActionSource::Ui,
                        ActionSource::Hint,
                        ActionSource::Ipc,
                        ActionSource::Switcher,
                    ],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.link.send".into(),
                    subject: ActionSubject::Link,
                    verb: "send".into(),
                    label: "Send link to target".into(),
                    description: "Send a validated link to a configured external target.".into(),
                    command: "send".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "target".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "url".into(),
                            kind: ArgumentKind::Url,
                            required: false,
                        },
                    ],
                    examples: vec!["action link send --to mpv".into()],
                    sources: vec![
                        ActionSource::Ui,
                        ActionSource::Hint,
                        ActionSource::Ipc,
                        ActionSource::Switcher,
                    ],
                    effect: EffectClass::Sensitive,
                    confirmation: ActionConfirmation::Never,
                    sensitive: true,
                },
                ActionDefinition {
                    id: "browser.url.send".into(),
                    subject: ActionSubject::Url,
                    verb: "send".into(),
                    label: "Send URL to target".into(),
                    description: "Send the current or explicitly supplied URL to a configured external target.".into(),
                    command: "send".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "target".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "url".into(),
                            kind: ArgumentKind::Url,
                            required: false,
                        },
                    ],
                    examples: vec!["action url send --to mpv".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Sensitive,
                    confirmation: ActionConfirmation::Never,
                    sensitive: true,
                },
                ActionDefinition {
                    id: "browser.selection.send".into(),
                    subject: ActionSubject::Selection,
                    verb: "send".into(),
                    label: "Send selection to target".into(),
                    description:
                        "Send the current visible selection to a configured external target.".into(),
                    command: "send".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "target".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action selection send --to mpv".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Sensitive,
                    confirmation: ActionConfirmation::Never,
                    sensitive: true,
                },
                ActionDefinition {
                    id: "browser.tab.send".into(),
                    subject: ActionSubject::Tab,
                    verb: "send".into(),
                    label: "Send tab to target".into(),
                    description: "Send the active tab URL and title to a configured external target.".into(),
                    command: "send".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "target".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action tab send --to mpv".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Sensitive,
                    confirmation: ActionConfirmation::Never,
                    sensitive: true,
                },
                ActionDefinition {
                    id: "browser.selection.copy".into(),
                    subject: ActionSubject::Selection,
                    verb: "copy".into(),
                    label: "Copy selection".into(),
                    description: "Copy the current visible document selection.".into(),
                    command: "yank".into(),
                    arguments: Vec::new(),
                    examples: vec!["action selection copy".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Sensitive,
                    confirmation: ActionConfirmation::Never,
                    sensitive: true,
                },
                ActionDefinition {
                    id: "browser.selection.search".into(),
                    subject: ActionSubject::Selection,
                    verb: "search".into(),
                    label: "Search selection".into(),
                    description: "Search the current visible document selection.".into(),
                    command: "selection-search".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "engine".into(),
                        kind: ArgumentKind::Text,
                        required: false,
                    }],
                    examples: vec!["action selection search --engine ddg".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: true,
                },
                ActionDefinition {
                    id: "browser.url.clean".into(),
                    subject: ActionSubject::Url,
                    verb: "clean".into(),
                    label: "Preview cleaned URL".into(),
                    description: "Preview conservative reviewed URL cleaning.".into(),
                    command: "url-clean".into(),
                    arguments: vec![text_url.clone()],
                    examples: vec!["action url clean https://example.test/?utm_source=demo".into()],
                    sources: sources.clone(),
                    effect: EffectClass::Query,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.url.explain".into(),
                    subject: ActionSubject::Url,
                    verb: "explain".into(),
                    label: "Explain URL cleaning".into(),
                    description: "Explain retained and removed URL components.".into(),
                    command: "url-explain".into(),
                    arguments: vec![text_url],
                    examples: vec![
                        "action url explain https://example.test/?utm_source=demo".into(),
                    ],
                    sources,
                    effect: EffectClass::Query,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.undo".into(),
                    subject: ActionSubject::Tab,
                    verb: "undo".into(),
                    label: "Reopen closed tab".into(),
                    description: "Reopen the most recent eligible closed tab.".into(),
                    command: "tab-undo".into(),
                    arguments: Vec::new(),
                    examples: vec!["action tab undo".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.open".into(),
                    subject: ActionSubject::Tab,
                    verb: "open".into(),
                    label: "Open tab".into(),
                    description: "Open a resolved input in a foreground or background tab."
                        .into(),
                    command: "tab-open".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "input".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "background".into(),
                            kind: ArgumentKind::Boolean,
                            required: false,
                        },
                    ],
                    examples: vec![
                        "action tab open https://example.test".into(),
                        "action tab open --background https://example.test".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.next".into(),
                    subject: ActionSubject::Tab,
                    verb: "next".into(),
                    label: "Next tab".into(),
                    description: "Select the next tab in deterministic window order.".into(),
                    command: "tab-next".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "count".into(),
                        kind: ArgumentKind::Integer,
                        required: false,
                    }],
                    examples: vec!["action tab next".into(), "action tab next 2".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.previous".into(),
                    subject: ActionSubject::Tab,
                    verb: "previous".into(),
                    label: "Previous tab".into(),
                    description: "Select the previous tab in deterministic window order.".into(),
                    command: "tab-prev".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "count".into(),
                        kind: ArgumentKind::Integer,
                        required: false,
                    }],
                    examples: vec![
                        "action tab previous".into(),
                        "action tab previous 2".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.clone".into(),
                    subject: ActionSubject::Tab,
                    verb: "clone".into(),
                    label: "Clone tab".into(),
                    description: "Open the active tab's safe URL in a new same-profile tab.".into(),
                    command: "tab-clone".into(),
                    arguments: Vec::new(),
                    examples: vec!["action tab clone".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.back".into(),
                    subject: ActionSubject::Tab,
                    verb: "back".into(),
                    label: "Go back".into(),
                    description: "Traverse the active tab's engine history backward.".into(),
                    command: "back".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "count".into(),
                        kind: ArgumentKind::Integer,
                        required: false,
                    }],
                    examples: vec!["action tab back".into(), "action tab back 2".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.forward".into(),
                    subject: ActionSubject::Tab,
                    verb: "forward".into(),
                    label: "Go forward".into(),
                    description: "Traverse the active tab's engine history forward.".into(),
                    command: "forward".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "count".into(),
                        kind: ArgumentKind::Integer,
                        required: false,
                    }],
                    examples: vec!["action tab forward".into(), "action tab forward 2".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.reload".into(),
                    subject: ActionSubject::Tab,
                    verb: "reload".into(),
                    label: "Reload tab".into(),
                    description: "Reload the active tab while preserving form-submission safety.".into(),
                    command: "reload".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "bypass_cache".into(),
                        kind: ArgumentKind::Boolean,
                        required: false,
                    }],
                    examples: vec![
                        "action tab reload".into(),
                        "action tab reload --bypass-cache".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.stop".into(),
                    subject: ActionSubject::Tab,
                    verb: "stop".into(),
                    label: "Stop loading".into(),
                    description: "Cancel the active tab's current load.".into(),
                    command: "stop".into(),
                    arguments: Vec::new(),
                    examples: vec!["action tab stop".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.context.enter".into(),
                    subject: ActionSubject::Context,
                    verb: "enter".into(),
                    label: "Enter context".into(),
                    description: "Enter a captured profile-affine context.".into(),
                    command: "context-enter".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "name".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action context enter CONTEXT".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.context.save".into(),
                    subject: ActionSubject::Context,
                    verb: "save".into(),
                    label: "Save context".into(),
                    description: "Save the current tab membership as a context.".into(),
                    command: "context-save".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "name".into(),
                        kind: ArgumentKind::Text,
                        required: false,
                    }],
                    examples: vec!["action context save CONTEXT".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.window.focus".into(),
                    subject: ActionSubject::Window,
                    verb: "focus".into(),
                    label: "Focus window".into(),
                    description: "Focus a captured live browser window.".into(),
                    command: "window-focus".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action window focus WINDOW_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.window.new".into(),
                    subject: ActionSubject::Window,
                    verb: "new".into(),
                    label: "Open window".into(),
                    description: "Create a normal or private browser window.".into(),
                    command: "window-new".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "profile".into(),
                            kind: ArgumentKind::Text,
                            required: false,
                        },
                        ArgumentDefinition {
                            name: "private".into(),
                            kind: ArgumentKind::Boolean,
                            required: false,
                        },
                    ],
                    examples: vec![
                        "action window new".into(),
                        "action window new --profile work".into(),
                        "action window new --private".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.window.move".into(),
                    subject: ActionSubject::Window,
                    verb: "move".into(),
                    label: "Move window".into(),
                    description: "Move a captured live browser window to a validated compositor workspace.".into(),
                    command: "window-move".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "id".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "workspace".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                    ],
                    examples: vec!["action window move WINDOW_ID WORKSPACE".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.window.close".into(),
                    subject: ActionSubject::Window,
                    verb: "close".into(),
                    label: "Close window".into(),
                    description: "Request close of the owning browser window.".into(),
                    command: "window-close".into(),
                    arguments: Vec::new(),
                    examples: vec!["action window close".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.window.fullscreen".into(),
                    subject: ActionSubject::Window,
                    verb: "fullscreen".into(),
                    label: "Toggle fullscreen".into(),
                    description: "Set or toggle fullscreen for the owning browser window.".into(),
                    command: "fullscreen".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "state".into(),
                        kind: ArgumentKind::Enum,
                        required: false,
                    }],
                    examples: vec![
                        "action window fullscreen".into(),
                        "action window fullscreen on".into(),
                        "action window fullscreen off".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.history-entry.open".into(),
                    subject: ActionSubject::HistoryEntry,
                    verb: "open".into(),
                    label: "Open history entry".into(),
                    description: "Navigate to a captured eligible history entry.".into(),
                    command: "history-open".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action history-entry open HISTORY_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.history-entry.clear".into(),
                    subject: ActionSubject::HistoryEntry,
                    verb: "clear".into(),
                    label: "Clear history".into(),
                    description: "Clear eligible profile history after explicit confirmation."
                        .into(),
                    command: "history-clear".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "since".into(),
                            kind: ArgumentKind::Integer,
                            required: false,
                        },
                        ArgumentDefinition {
                            name: "origin".into(),
                            kind: ArgumentKind::Text,
                            required: false,
                        },
                        ArgumentDefinition {
                            name: "confirmed".into(),
                            kind: ArgumentKind::Boolean,
                            required: false,
                        },
                    ],
                    examples: vec![
                        "action history-entry clear".into(),
                        "action history-entry clear --since UNIX_SECONDS --confirm".into(),
                        "action history-entry clear --origin https://example.test --confirm".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::RequiredWhenChanged,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.bookmark.add".into(),
                    subject: ActionSubject::Bookmark,
                    verb: "add".into(),
                    label: "Add bookmark".into(),
                    description: "Bookmark the current page with an optional title.".into(),
                    command: "bookmark-add".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "title".into(),
                        kind: ArgumentKind::Text,
                        required: false,
                    }],
                    examples: vec![
                        "action bookmark add".into(),
                        "action bookmark add --title Reference".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.bookmark.open".into(),
                    subject: ActionSubject::Bookmark,
                    verb: "open".into(),
                    label: "Open bookmark".into(),
                    description: "Navigate to a captured eligible bookmark.".into(),
                    command: "bookmark-open".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action bookmark open BOOKMARK_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.bookmark.delete".into(),
                    subject: ActionSubject::Bookmark,
                    verb: "delete".into(),
                    label: "Delete bookmark".into(),
                    description: "Delete a captured bookmark by stable ID.".into(),
                    command: "bookmark-delete".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action bookmark delete BOOKMARK_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.bookmark.edit".into(),
                    subject: ActionSubject::Bookmark,
                    verb: "edit".into(),
                    label: "Edit bookmark".into(),
                    description: "Update a captured bookmark title by stable ID.".into(),
                    command: "bookmark-edit".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "id".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "title".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                    ],
                    examples: vec![
                        "action bookmark edit BOOKMARK_ID --title TITLE".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.bookmark.list".into(),
                    subject: ActionSubject::Bookmark,
                    verb: "list".into(),
                    label: "List bookmarks".into(),
                    description: "List bookmarks for the current profile.".into(),
                    command: "bookmark-list".into(),
                    arguments: Vec::new(),
                    examples: vec!["action bookmark list".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Query,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.quickmark.add".into(),
                    subject: ActionSubject::Quickmark,
                    verb: "add".into(),
                    label: "Add quickmark".into(),
                    description: "Save a named shortcut to the current or supplied URL.".into(),
                    command: "quickmark-add".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "name".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "url".into(),
                            kind: ArgumentKind::Url,
                            required: false,
                        },
                    ],
                    examples: vec![
                        "action quickmark add NAME".into(),
                        "action quickmark add NAME https://example.test".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::RequiredWhenChanged,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.quickmark.open".into(),
                    subject: ActionSubject::Quickmark,
                    verb: "open".into(),
                    label: "Open quickmark".into(),
                    description: "Navigate to a captured eligible quickmark.".into(),
                    command: "quickmark-open".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "name".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action quickmark open NAME".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.quickmark.delete".into(),
                    subject: ActionSubject::Quickmark,
                    verb: "delete".into(),
                    label: "Delete quickmark".into(),
                    description: "Delete a captured quickmark by stable name.".into(),
                    command: "quickmark-delete".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "name".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action quickmark delete NAME".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.quickmark.edit".into(),
                    subject: ActionSubject::Quickmark,
                    verb: "edit".into(),
                    label: "Edit quickmark".into(),
                    description: "Update a captured quickmark destination.".into(),
                    command: "quickmark-edit".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "name".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "url".into(),
                            kind: ArgumentKind::Url,
                            required: true,
                        },
                    ],
                    examples: vec![
                        "action quickmark edit NAME https://example.test".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.quickmark.list".into(),
                    subject: ActionSubject::Quickmark,
                    verb: "list".into(),
                    label: "List quickmarks".into(),
                    description: "List named URL shortcuts for the current profile.".into(),
                    command: "quickmark-list".into(),
                    arguments: Vec::new(),
                    examples: vec!["action quickmark list".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Query,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.session.save".into(),
                    subject: ActionSubject::Session,
                    verb: "save".into(),
                    label: "Save session".into(),
                    description: "Capture the current browser state as a named session.".into(),
                    command: "session-save".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "name".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action session save NAME".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::RequiredWhenChanged,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.session.load".into(),
                    subject: ActionSubject::Session,
                    verb: "load".into(),
                    label: "Load session".into(),
                    description: "Preview and load a captured named session.".into(),
                    command: "session-load".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "name".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "append".into(),
                            kind: ArgumentKind::Boolean,
                            required: false,
                        },
                    ],
                    examples: vec![
                        "action session load NAME".into(),
                        "action session load --append NAME".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::RequiredWhenChanged,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.session.delete".into(),
                    subject: ActionSubject::Session,
                    verb: "delete".into(),
                    label: "Delete session".into(),
                    description: "Delete a captured named session.".into(),
                    command: "session-delete".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "name".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action session delete NAME".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::RequiredWhenChanged,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.session.list".into(),
                    subject: ActionSubject::Session,
                    verb: "list".into(),
                    label: "List sessions".into(),
                    description: "List named sessions in the current profile.".into(),
                    command: "session-list".into(),
                    arguments: Vec::new(),
                    examples: vec!["action session list".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Query,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.command.help".into(),
                    subject: ActionSubject::Command,
                    verb: "help".into(),
                    label: "Show command help".into(),
                    description: "Show help for a captured registered command.".into(),
                    command: "command-help".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action command help COMMAND_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Query,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.command.execute".into(),
                    subject: ActionSubject::Command,
                    verb: "execute".into(),
                    label: "Execute command".into(),
                    description: "Execute a captured registered command with typed arguments."
                        .into(),
                    command: "command-execute".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "id".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "arguments".into(),
                            kind: ArgumentKind::Object,
                            required: false,
                        },
                    ],
                    examples: vec!["action command execute COMMAND_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.focus".into(),
                    subject: ActionSubject::Tab,
                    verb: "focus".into(),
                    label: "Focus tab".into(),
                    description: "Focus a live tab captured by stable ID.".into(),
                    command: "tab-focus".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action tab focus TAB_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.select".into(),
                    subject: ActionSubject::Tab,
                    verb: "select".into(),
                    label: "Select tab".into(),
                    description: "Select a live tab by displayed index or stable ID.".into(),
                    command: "tab-select".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "selector".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action tab select 2".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.close".into(),
                    subject: ActionSubject::Tab,
                    verb: "close".into(),
                    label: "Close tab".into(),
                    description: "Close a live tab captured by stable ID.".into(),
                    command: "tab-close".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action tab close TAB_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.give".into(),
                    subject: ActionSubject::Tab,
                    verb: "give".into(),
                    label: "Move tab to window".into(),
                    description:
                        "Move the live tab to a validated same-profile browser window.".into(),
                    command: "tab-give".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "window_id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action tab give WINDOW_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.suspend".into(),
                    subject: ActionSubject::Tab,
                    verb: "suspend".into(),
                    label: "Suspend tab".into(),
                    description:
                        "Freeze an engine-approved hidden tab without discarding its page.".into(),
                    command: "tab-suspend".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action tab suspend TAB_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.resume".into(),
                    subject: ActionSubject::Tab,
                    verb: "resume".into(),
                    label: "Resume tab".into(),
                    description: "Resume a frozen tab's engine lifecycle.".into(),
                    command: "tab-resume".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action tab resume TAB_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.discard".into(),
                    subject: ActionSubject::Tab,
                    verb: "discard".into(),
                    label: "Discard tab".into(),
                    description: "Discard an engine-approved hidden tab until it is resumed.".into(),
                    command: "tab-discard".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action tab discard TAB_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.move".into(),
                    subject: ActionSubject::Tab,
                    verb: "move".into(),
                    label: "Move tab".into(),
                    description: "Move a live tab left or right, or into a same-profile context window.".into(),
                    command: "tab-move".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "id".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "direction".into(),
                            kind: ArgumentKind::Enum,
                            required: false,
                        },
                        ArgumentDefinition {
                            name: "context".into(),
                            kind: ArgumentKind::Text,
                            required: false,
                        },
                    ],
                    examples: vec![
                        "action tab move TAB_ID left".into(),
                        "action tab move TAB_ID --context research".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.zoom".into(),
                    subject: ActionSubject::Tab,
                    verb: "zoom".into(),
                    label: "Set tab zoom".into(),
                    description: "Adjust the current tab's page zoom factor.".into(),
                    command: "zoom".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "factor".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec![
                        "action tab zoom in".into(),
                        "action tab zoom out".into(),
                        "action tab zoom reset".into(),
                        "action tab zoom 1.25".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.search-next".into(),
                    subject: ActionSubject::Tab,
                    verb: "search-next".into(),
                    label: "Find next match".into(),
                    description: "Traverse retained in-page search matches.".into(),
                    command: "search-next".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "direction".into(),
                            kind: ArgumentKind::Enum,
                            required: false,
                        },
                        ArgumentDefinition {
                            name: "count".into(),
                            kind: ArgumentKind::Integer,
                            required: false,
                        },
                    ],
                    examples: vec![
                        "action tab search-next".into(),
                        "action tab search-next backward".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.scroll".into(),
                    subject: ActionSubject::Tab,
                    verb: "scroll".into(),
                    label: "Scroll tab".into(),
                    description: "Scroll the active page in a bounded direction.".into(),
                    command: "scroll".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "direction".into(),
                            kind: ArgumentKind::Enum,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "count".into(),
                            kind: ArgumentKind::Integer,
                            required: false,
                        },
                    ],
                    examples: vec![
                        "action tab scroll down".into(),
                        "action tab scroll down 3".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.scroll-page".into(),
                    subject: ActionSubject::Tab,
                    verb: "scroll-page".into(),
                    label: "Scroll page".into(),
                    description: "Scroll the active page by a viewport or half viewport.".into(),
                    command: "scroll-page".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "direction".into(),
                            kind: ArgumentKind::Enum,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "half".into(),
                            kind: ArgumentKind::Boolean,
                            required: false,
                        },
                        ArgumentDefinition {
                            name: "count".into(),
                            kind: ArgumentKind::Integer,
                            required: false,
                        },
                    ],
                    examples: vec![
                        "action tab scroll-page down".into(),
                        "action tab scroll-page up --half".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.scroll-to".into(),
                    subject: ActionSubject::Tab,
                    verb: "scroll-to".into(),
                    label: "Scroll to page edge".into(),
                    description: "Scroll the active page to its top or bottom edge.".into(),
                    command: "scroll-to".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "edge".into(),
                        kind: ArgumentKind::Enum,
                        required: true,
                    }],
                    examples: vec![
                        "action tab scroll-to top".into(),
                        "action tab scroll-to bottom".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Navigation,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.reopen-window".into(),
                    subject: ActionSubject::Tab,
                    verb: "reopen-window".into(),
                    label: "Reopen tab in window".into(),
                    description:
                        "Open the current tab's safe URL in a same-profile window; live state is lost."
                            .into(),
                    command: "reopen-in-window".into(),
                    arguments: Vec::new(),
                    examples: vec!["action tab reopen-window".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::RequiredWhenChanged,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.detach".into(),
                    subject: ActionSubject::Tab,
                    verb: "detach".into(),
                    label: "Detach live tab".into(),
                    description:
                        "Move a live tab to a same-profile window without navigation when supported."
                            .into(),
                    command: "tab-detach".into(),
                    arguments: Vec::new(),
                    examples: vec!["action tab detach".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.mute".into(),
                    subject: ActionSubject::Tab,
                    verb: "mute".into(),
                    label: "Mute tab".into(),
                    description: "Set or toggle muted state for a live tab captured by stable ID."
                        .into(),
                    command: "tab-mute".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "id".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "state".into(),
                            kind: ArgumentKind::Enum,
                            required: false,
                        },
                    ],
                    examples: vec![
                        "action tab mute TAB_ID".into(),
                        "action tab mute TAB_ID on".into(),
                        "action tab mute TAB_ID off".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.download.open".into(),
                    subject: ActionSubject::Download,
                    verb: "open".into(),
                    label: "Open download".into(),
                    description: "Open a completed download in its default handler.".into(),
                    command: "download-open".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action download open DOWNLOAD_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.download.show".into(),
                    subject: ActionSubject::Download,
                    verb: "show".into(),
                    label: "Show download".into(),
                    description: "Reveal a completed download in its containing folder.".into(),
                    command: "download-show".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action download show DOWNLOAD_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.download.cancel".into(),
                    subject: ActionSubject::Download,
                    verb: "cancel".into(),
                    label: "Cancel download".into(),
                    description: "Cancel an active download.".into(),
                    command: "download-cancel".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action download cancel DOWNLOAD_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.download.retry".into(),
                    subject: ActionSubject::Download,
                    verb: "retry".into(),
                    label: "Retry download".into(),
                    description: "Retry an interrupted or cancelled HTTP(S) download.".into(),
                    command: "download-retry".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action download retry DOWNLOAD_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.download.pause".into(),
                    subject: ActionSubject::Download,
                    verb: "pause".into(),
                    label: "Pause download".into(),
                    description: "Pause an in-progress download when supported by the engine."
                        .into(),
                    command: "download-pause".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action download pause DOWNLOAD_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.download.resume".into(),
                    subject: ActionSubject::Download,
                    verb: "resume".into(),
                    label: "Resume download".into(),
                    description: "Resume a paused download when supported by the engine.".into(),
                    command: "download-resume".into(),
                    arguments: vec![ArgumentDefinition {
                        name: "id".into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    }],
                    examples: vec!["action download resume DOWNLOAD_ID".into()],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
                ActionDefinition {
                    id: "browser.tab.pin".into(),
                    subject: ActionSubject::Tab,
                    verb: "pin".into(),
                    label: "Pin tab".into(),
                    description: "Set or toggle pinned state for a live tab.".into(),
                    command: "tab-pin".into(),
                    arguments: vec![
                        ArgumentDefinition {
                            name: "id".into(),
                            kind: ArgumentKind::Text,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "state".into(),
                            kind: ArgumentKind::Enum,
                            required: false,
                        },
                    ],
                    examples: vec![
                        "action tab pin TAB_ID".into(),
                        "action tab pin TAB_ID on".into(),
                        "action tab pin TAB_ID off".into(),
                    ],
                    sources: vec![ActionSource::Ui, ActionSource::Ipc, ActionSource::Switcher],
                    effect: EffectClass::Mutating,
                    confirmation: ActionConfirmation::Never,
                    sensitive: false,
                },
            ],
        }
    }

    #[must_use]
    pub fn definitions(&self) -> &[ActionDefinition] {
        &self.definitions
    }

    /// Validates the registry schema before it is exposed to a consumer.
    ///
    /// # Errors
    ///
    /// Returns a descriptive error for duplicate identities, malformed
    /// definitions, duplicate argument/source names, or missing examples.
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        let mut subjects_and_verbs = BTreeSet::new();
        for definition in &self.definitions {
            if definition.id.is_empty() || !definition.id.starts_with("browser.") {
                return Err(format!(
                    "action ID is not a browser namespace: {:?}",
                    definition.id
                ));
            }
            let expected_id = format!(
                "browser.{}.{}",
                definition.subject.as_str(),
                definition.verb
            );
            if definition.id != expected_id {
                let subject_verb = format!("{} {}", definition.subject.as_str(), definition.verb);
                return Err(format!(
                    "action {} does not match its subject/verb identity {}; expected {}",
                    definition.id, subject_verb, expected_id
                ));
            }
            if !ids.insert(definition.id.as_str()) {
                return Err(format!("duplicate action ID: {}", definition.id));
            }
            if definition.verb.is_empty() || definition.command.is_empty() {
                return Err(format!(
                    "action {} has an empty verb or command",
                    definition.id
                ));
            }
            if definition.label.is_empty() || definition.description.is_empty() {
                return Err(format!(
                    "action {} has an empty label or description",
                    definition.id
                ));
            }
            if !subjects_and_verbs.insert((definition.subject.as_str(), definition.verb.as_str())) {
                return Err(format!(
                    "duplicate action subject/verb: {} {}",
                    definition.subject.as_str(),
                    definition.verb
                ));
            }
            let mut arguments = BTreeSet::new();
            for argument in &definition.arguments {
                if argument.name.is_empty() || !arguments.insert(argument.name.as_str()) {
                    return Err(format!(
                        "action {} has a duplicate or empty argument",
                        definition.id
                    ));
                }
            }
            if definition.sources.is_empty() {
                return Err(format!(
                    "action {} has no invocation sources",
                    definition.id
                ));
            }
            let mut sources = BTreeSet::new();
            for source in &definition.sources {
                if !sources.insert(source.as_str()) {
                    return Err(format!(
                        "action {} has a duplicate invocation source",
                        definition.id
                    ));
                }
            }
            if definition.examples.is_empty() || definition.examples.iter().any(String::is_empty) {
                return Err(format!("action {} has no valid examples", definition.id));
            }
        }
        Ok(())
    }

    /// Validates the registry schema and confirms that every action executor
    /// resolves through the canonical command registry.
    ///
    /// # Errors
    ///
    /// Returns a descriptive error when the action schema is invalid or an
    /// action refers to an unknown command.
    pub fn validate_against_commands(&self, commands: &CommandRegistry) -> Result<(), String> {
        self.validate()?;
        for definition in &self.definitions {
            let command = commands.resolve(&definition.command).map_err(|_| {
                format!(
                    "action {} refers to unknown executor command {}",
                    definition.id, definition.command
                )
            })?;
            if definition.sensitive != command.sensitive {
                return Err(format!(
                    "action {} sensitivity {} disagrees with executor {} sensitivity {}",
                    definition.id, definition.sensitive, definition.command, command.sensitive
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn resolve(&self, id: &str) -> Option<&ActionDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.id == id)
    }

    #[must_use]
    pub fn resolve_subject_verb(&self, subject: &str, verb: &str) -> Option<&ActionDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.subject.as_str() == subject && definition.verb == verb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn builtins_have_stable_namespaced_ids_and_commands() {
        let registry = ActionRegistry::default_v1();
        registry.validate().expect("valid action registry");
        assert_eq!(registry.definitions().len(), 72);
        assert_eq!(
            registry
                .resolve("browser.url.clean")
                .map(|action| action.command.as_str()),
            Some("url-clean")
        );
        assert_eq!(
            registry
                .resolve("browser.link.download")
                .map(|action| action.command.as_str()),
            Some("download")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("url", "send")
                .map(|action| action.id.as_str()),
            Some("browser.url.send")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("tab", "send")
                .map(|action| action.id.as_str()),
            Some("browser.tab.send")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("url", "explain")
                .map(|action| action.id.as_str()),
            Some("browser.url.explain")
        );
        for (verb, command) in [
            ("back", "back"),
            ("forward", "forward"),
            ("reload", "reload"),
            ("stop", "stop"),
        ] {
            assert_eq!(
                registry
                    .resolve_subject_verb("tab", verb)
                    .map(|action| action.command.as_str()),
                Some(command)
            );
        }
        for (verb, command) in [
            ("scroll", "scroll"),
            ("scroll-page", "scroll-page"),
            ("scroll-to", "scroll-to"),
        ] {
            assert_eq!(
                registry
                    .resolve_subject_verb("tab", verb)
                    .map(|action| action.command.as_str()),
                Some(command)
            );
        }
        for (verb, command) in [
            ("next", "tab-next"),
            ("previous", "tab-prev"),
            ("clone", "tab-clone"),
        ] {
            assert_eq!(
                registry
                    .resolve_subject_verb("tab", verb)
                    .map(|action| action.command.as_str()),
                Some(command)
            );
        }
        assert_eq!(
            registry
                .resolve_subject_verb("window", "fullscreen")
                .map(|action| action.command.as_str()),
            Some("fullscreen")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("download", "retry")
                .map(|action| action.command.as_str()),
            Some("download-retry")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("download", "pause")
                .map(|action| action.command.as_str()),
            Some("download-pause")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("tab", "move")
                .map(|action| action.command.as_str()),
            Some("tab-move")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("tab", "suspend")
                .map(|action| action.command.as_str()),
            Some("tab-suspend")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("tab", "resume")
                .map(|action| action.command.as_str()),
            Some("tab-resume")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("tab", "discard")
                .map(|action| action.command.as_str()),
            Some("tab-discard")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("tab", "pin")
                .map(|action| action.command.as_str()),
            Some("tab-pin")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("tab", "mute")
                .map(|action| action
                    .arguments
                    .iter()
                    .map(|argument| argument.name.as_str())
                    .collect::<Vec<_>>()),
            Some(vec!["id", "state"])
        );
        assert_eq!(
            registry
                .resolve_subject_verb("tab", "undo")
                .map(|action| action.command.as_str()),
            Some("tab-undo")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("bookmark", "delete")
                .map(|action| action.command.as_str()),
            Some("bookmark-delete")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("command", "execute")
                .map(|action| action.command.as_str()),
            Some("command-execute")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("session", "load")
                .map(|action| action.command.as_str()),
            Some("session-load")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("link", "open")
                .map(|action| action.id.as_str()),
            Some("browser.link.open")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("url", "clean-copy")
                .map(|action| action.command.as_str()),
            Some("yank")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("selection", "copy")
                .map(|action| action.id.as_str()),
            Some("browser.selection.copy")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("selection", "search")
                .map(|action| action.command.as_str()),
            Some("selection-search")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("context", "enter")
                .map(|action| action.command.as_str()),
            Some("context-enter")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("window", "focus")
                .map(|action| action.command.as_str()),
            Some("window-focus")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("window", "close")
                .map(|action| action.command.as_str()),
            Some("window-close")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("link", "send")
                .map(|action| action.command.as_str()),
            Some("send")
        );
        assert_eq!(
            registry
                .resolve_subject_verb("selection", "send")
                .map(|action| action.command.as_str()),
            Some("send")
        );
        assert_eq!(
            registry
                .resolve("browser.link.open")
                .and_then(|action| action.examples.first())
                .map(String::as_str),
            Some("action link open https://example.test/docs")
        );
        assert_eq!(
            registry.resolve("browser.link.open").map(|action| action
                .arguments
                .iter()
                .map(|argument| argument.name.as_str())
                .collect::<Vec<_>>()),
            Some(vec!["url", "target"])
        );
        assert!(
            registry
                .definitions()
                .iter()
                .all(|action| action.id.starts_with("browser."))
        );
    }

    #[test]
    fn builtins_declare_completion_and_availability_metadata() {
        let registry = ActionRegistry::default_v1();
        for action in registry.definitions() {
            assert!(!action.completion_provider().is_empty());
            assert!(!action.availability_predicate().is_empty());
        }
        assert_eq!(
            registry
                .resolve("browser.link.open")
                .expect("link action")
                .completion_provider(),
            "url"
        );
        assert_eq!(
            registry
                .resolve("browser.tab.focus")
                .expect("tab action")
                .availability_predicate(),
            "live-tab"
        );
        assert_eq!(
            registry
                .resolve("browser.command.execute")
                .expect("command action")
                .completion_provider(),
            "command"
        );
    }

    #[test]
    fn registry_validation_rejects_duplicate_identity_and_schema_fields() {
        let mut definitions = ActionRegistry::default_v1().definitions.clone();
        definitions.push(definitions[0].clone());
        let registry = ActionRegistry { definitions };
        assert_eq!(
            registry.validate(),
            Err("duplicate action ID: browser.url.open".into())
        );

        let mut definition = ActionRegistry::default_v1().definitions[0].clone();
        definition.arguments.push(definition.arguments[0].clone());
        let registry = ActionRegistry {
            definitions: vec![definition],
        };
        assert_eq!(
            registry.validate(),
            Err("action browser.url.open has a duplicate or empty argument".into())
        );

        let mut definition = ActionRegistry::default_v1().definitions[0].clone();
        definition.id = "browser.link.open".into();
        let registry = ActionRegistry {
            definitions: vec![definition],
        };
        assert_eq!(
            registry.validate(),
            Err("action browser.link.open does not match its subject/verb identity url open; expected browser.url.open".into())
        );

        let mut definition = ActionRegistry::default_v1().definitions[0].clone();
        definition.sources.clear();
        let registry = ActionRegistry {
            definitions: vec![definition],
        };
        assert_eq!(
            registry.validate(),
            Err("action browser.url.open has no invocation sources".into())
        );

        let mut definition = ActionRegistry::default_v1().definitions[0].clone();
        definition.command = "unknown-action-command".into();
        let registry = ActionRegistry {
            definitions: vec![definition],
        };
        assert_eq!(
            registry.validate_against_commands(&CommandRegistry::default_v1()),
            Err(
                "action browser.url.open refers to unknown executor command unknown-action-command"
                    .into()
            )
        );
    }

    #[test]
    fn registry_validation_rejects_executor_metadata_drift() {
        let commands = CommandRegistry::default_v1();
        let mut registry = ActionRegistry::default_v1();
        registry.definitions[0].sensitive = true;
        let error = registry
            .validate_against_commands(&commands)
            .expect_err("sensitivity drift must be rejected");
        assert!(error.contains("sensitivity"));
        assert!(error.contains("disagrees with executor"));
    }
}
