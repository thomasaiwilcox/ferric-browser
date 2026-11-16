//! Typed binding-help projection for the QML list view.

use super::{
    BindingResolver, CxxQtType, ParseInput, Pin, QString, ipc_mode_name, parse_chain, qobject,
};

const BINDING_HELP_MODES: [&str; 9] = [
    "normal",
    "insert",
    "insert-locked",
    "command",
    "search",
    "hint",
    "caret",
    "pass-through",
    "pass-through-once",
];

struct BindingHelpRow {
    kind: &'static str,
    title: String,
    mode: String,
    command: String,
    description: String,
    keys: String,
    source: String,
    count: String,
}

impl BindingHelpRow {
    fn heading(title: String) -> Self {
        Self {
            kind: "heading",
            title,
            mode: String::new(),
            command: String::new(),
            description: String::new(),
            keys: String::new(),
            source: String::new(),
            count: String::new(),
        }
    }
}

impl qobject::BrowserUi {
    pub fn refresh_binding_help(mut self: Pin<&mut Self>, search: &QString) -> bool {
        let query = search.to_string().trim().to_ascii_lowercase();
        let definitions = self
            .as_ref()
            .rust()
            .bindings
            .as_ref()
            .map(BindingResolver::definitions)
            .unwrap_or_default();
        let this = self.as_ref();
        let sources = super::binding_source_index::BindingSources::from_config(&this.rust().config);
        let definitions = definitions
            .iter()
            .map(|binding| (binding, binding.canonical_keys()))
            .collect::<Vec<_>>();
        let mut rows = Vec::new();

        for mode in BINDING_HELP_MODES {
            let mut mode_rows = Vec::new();
            for command in self.as_ref().rust().registry.definitions() {
                if !command
                    .modes
                    .iter()
                    .any(|candidate| ipc_mode_name(*candidate) == mode)
                {
                    continue;
                }

                let bindings = definitions
                    .iter()
                    .filter(|(binding, _)| {
                        ipc_mode_name(binding.mode) == mode
                            && canonical_binding_command_name(
                                &self.as_ref().rust().registry,
                                &binding.command,
                            )
                            .as_deref()
                                == Some(command.name.as_str())
                    })
                    .collect::<Vec<_>>();
                let keys = if bindings.is_empty() {
                    "unbound".to_owned()
                } else {
                    bindings
                        .iter()
                        .map(|(binding, _)| binding.keys.join(" "))
                        .collect::<Vec<_>>()
                        .join(" · ")
                };
                let source = if bindings.is_empty() {
                    "—".to_owned()
                } else {
                    bindings
                        .iter()
                        .map(|(binding, keys)| {
                            if sources.get(binding.mode, keys).is_some() {
                                "user"
                            } else {
                                "built-in"
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" · ")
                };
                let count = match command.count {
                    ferric_browser_core::CountPolicy::NotSupported => "no count".to_owned(),
                    ferric_browser_core::CountPolicy::Supported { maximum } => {
                        format!("count ≤ {maximum}")
                    }
                };
                let search_text = format!(
                    "{mode} {} {} {keys} {source}",
                    command.name, command.description
                )
                .to_ascii_lowercase();
                if !query.is_empty() && !search_text.contains(&query) {
                    continue;
                }
                mode_rows.push(BindingHelpRow {
                    kind: "command",
                    title: String::new(),
                    mode: mode.to_owned(),
                    command: command.name.clone(),
                    description: command.description.clone(),
                    keys,
                    source,
                    count,
                });
            }
            if matches!(mode, "pass-through" | "pass-through-once") {
                let description = "Return to Normal; ordinary Escape goes to the webpage.";
                let search_text = format!("{mode} mode-enter normal Ctrl+Escape {description}")
                    .to_ascii_lowercase();
                if query.is_empty() || search_text.contains(&query) {
                    mode_rows.push(BindingHelpRow {
                        kind: "command",
                        title: String::new(),
                        mode: mode.to_owned(),
                        command: "mode-enter normal".into(),
                        description: description.into(),
                        keys: "Ctrl+Escape".into(),
                        source: "built-in (reserved)".into(),
                        count: "no count".into(),
                    });
                }
            }
            if !mode_rows.is_empty() {
                rows.push(BindingHelpRow::heading(format!("{mode} mode")));
                rows.extend(mode_rows);
            }
        }

        let mut conflict_rows = Vec::new();
        for (left_index, (left, left_keys)) in definitions.iter().enumerate() {
            for (right, right_keys) in definitions.iter().skip(left_index + 1) {
                if left.mode != right.mode
                    || left_keys == right_keys
                    || (!right_keys.starts_with(left_keys) && !left_keys.starts_with(right_keys))
                {
                    continue;
                }
                let mode = ipc_mode_name(left.mode);
                let command = [left.keys.concat(), right.keys.concat()].join(" / ");
                let keys = [left.command.clone(), right.command.clone()].join(" · ");
                let description = "A shorter binding is also a prefix of a longer binding; the configured chord timeout decides the exact-prefix case.".to_owned();
                let search_text =
                    format!("{mode} {command} {keys} {description}").to_ascii_lowercase();
                if !query.is_empty() && !search_text.contains(&query) {
                    continue;
                }
                conflict_rows.push(BindingHelpRow {
                    kind: "conflict",
                    title: String::new(),
                    mode: mode.to_owned(),
                    command,
                    description,
                    keys,
                    source: "prefix ambiguity".to_owned(),
                    count: format!(
                        "timeout {} ms",
                        ferric_browser_core::DEFAULT_CHORD_TIMEOUT_MS
                    ),
                });
            }
        }
        if !conflict_rows.is_empty() {
            rows.push(BindingHelpRow::heading("Binding conflicts".to_owned()));
            rows.extend(conflict_rows);
        }

        self.as_mut()
            .set_binding_help_row_kinds(rows.iter().map(|row| QString::from(row.kind)).collect());
        self.as_mut().set_binding_help_row_titles(
            rows.iter().map(|row| QString::from(&row.title)).collect(),
        );
        self.as_mut()
            .set_binding_help_row_modes(rows.iter().map(|row| QString::from(&row.mode)).collect());
        self.as_mut().set_binding_help_row_commands(
            rows.iter().map(|row| QString::from(&row.command)).collect(),
        );
        self.as_mut().set_binding_help_row_descriptions(
            rows.iter()
                .map(|row| QString::from(&row.description))
                .collect(),
        );
        self.as_mut()
            .set_binding_help_row_keys(rows.iter().map(|row| QString::from(&row.keys)).collect());
        self.as_mut().set_binding_help_row_sources(
            rows.iter().map(|row| QString::from(&row.source)).collect(),
        );
        self.as_mut().set_binding_help_row_counts(
            rows.iter().map(|row| QString::from(&row.count)).collect(),
        );
        true
    }
}

fn canonical_binding_command_name(
    registry: &ferric_browser_core::CommandRegistry,
    command: &str,
) -> Option<String> {
    parse_chain(command, ParseInput::Interactive)
        .ok()
        .and_then(|mut commands| commands.drain(..).next())
        .and_then(|command| registry.resolve(&command.name).ok())
        .map(|definition| definition.name.clone())
}
