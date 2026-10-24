use super::{
    CxxQtType, Event, Mode, ParseInput, ParsedCommand, Pin, QString, is_safe_history_url,
    parse_chain, qobject,
};

fn durable_history_commands(commands: &[ParsedCommand]) -> bool {
    commands.iter().all(|command| {
        command.arguments.is_empty()
            || (matches!(command.name.as_str(), "open" | "tab-open")
                && command.arguments.len() == 1
                && (command.arguments[0].starts_with("https://")
                    || command.arguments[0].starts_with("http://"))
                && is_safe_history_url(&command.arguments[0])
                && !command.arguments[0].contains('?'))
    })
}

fn reconcile_history_cursor(
    history: &[String],
    cursor: Option<usize>,
    draft: Option<&str>,
    current: &str,
) -> Option<usize> {
    let draft = draft?;
    if let Some(cursor) = cursor
        && history.get(cursor).is_some_and(|entry| entry == current)
    {
        return Some(cursor);
    }
    // Native Undo/Redo can restore another recalled line without moving the
    // history cursor, even after Down returned to the draft. The saved draft
    // stays outside recorded entries; other exact lines resume from their
    // own positions.
    if current == draft {
        return None;
    }
    history.iter().position(|entry| entry == current)
}

fn secondary_command_supported(command: &ParsedCommand) -> bool {
    if command.name == "help" {
        return matches!(command.arguments.as_slice(), [topic] if super::setting_metadata(topic).is_some());
    }
    if command.name == "open" {
        return !command.arguments.iter().any(|argument| {
            matches!(argument.as_str(), "--ephemeral" | "--profile" | "--context")
        }) && !command
            .arguments
            .windows(2)
            .any(|arguments| arguments[0] == "--target" && arguments[1] != "current");
    }
    if command.name == "open-current" {
        return command.arguments.is_empty();
    }
    if command.name == "paste-open" {
        return !command
            .arguments
            .windows(2)
            .any(|arguments| arguments[0] == "--target" && arguments[1] != "current");
    }
    !matches!(
        command.name.as_str(),
        "action"
            | "action-list"
            | "blocking-toggle"
            | "blocklist-update"
            | "bookmark-list"
            | "command-execute"
            | "command-help"
            | "context-create"
            | "context-delete"
            | "context-enter"
            | "context-list"
            | "context-route"
            | "context-save"
            | "history"
            | "journey"
            | "link-cleaning-update"
            | "macro-play"
            | "permissions"
            | "quickmark-list"
            | "repeat"
            | "site-data-clear"
            | "site-doctor"
            | "site-doctor-undo"
            | "site-status"
            | "window-focus"
            | "window-move"
    )
}

#[cfg(test)]
mod settings_surface_tests {
    use super::*;

    #[test]
    fn secondary_commands_require_a_supported_surface() {
        let command = |name: &str, arguments: &[&str]| ParsedCommand {
            name: name.into(),
            arguments: arguments.iter().map(|value| (*value).to_owned()).collect(),
        };
        assert!(secondary_command_supported(&command("settings", &[])));
        assert!(secondary_command_supported(&command("binding-list", &[])));
        assert!(secondary_command_supported(&command(
            "binding-explain",
            &["x"]
        )));
        for arguments in [vec![], vec!["--private"], vec!["--profile", "work"]] {
            assert!(secondary_command_supported(&command(
                "window-new",
                &arguments
            )));
        }
        assert!(secondary_command_supported(&command(
            "url-clean",
            &["https://example.test/?utm_source=demo"]
        )));
        assert!(secondary_command_supported(&command("url-explain", &[])));
        assert!(secondary_command_supported(&command(
            "open",
            &["--clean-link", "https://example.test/?utm_source=demo"]
        )));
        assert!(!secondary_command_supported(&command(
            "open",
            &[
                "--clean-link",
                "--target",
                "new-window",
                "https://example.test/"
            ]
        )));
        for arguments in [
            vec!["links"],
            vec!["--target", "pointer", "links"],
            vec!["--target", "choose"],
        ] {
            assert!(secondary_command_supported(&command("hint", &arguments)));
        }
        assert!(secondary_command_supported(&command("grid", &[])));
        assert!(secondary_command_supported(&command("target-search", &[])));
        assert!(secondary_command_supported(&command(
            "help",
            &["ui.font_size_pt"]
        )));
        for unavailable in [
            command("help", &[]),
            command("help", &["open"]),
            command("help", &["ui.font_size_pt", "extra"]),
        ] {
            assert!(!secondary_command_supported(&unavailable));
        }
    }
}

impl qobject::BrowserUi {
    pub(super) fn execute_interactive_command(mut self: Pin<&mut Self>, input: &QString) -> bool {
        self.as_mut()
            .execute_interactive_command_for_surface(input, true)
    }

    pub(super) fn execute_secondary_interactive_command(
        mut self: Pin<&mut Self>,
        input: &QString,
    ) -> bool {
        self.as_mut()
            .execute_interactive_command_for_surface(input, false)
    }

    fn execute_interactive_command_for_surface(
        mut self: Pin<&mut Self>,
        input: &QString,
        full_chrome: bool,
    ) -> bool {
        self.as_mut().set_command_retryable(false);
        let text = input.to_string();
        let commands = match parse_chain(&text, ParseInput::Interactive) {
            Ok(commands) => commands,
            Err(error) => {
                self.set_status_text(QString::from(error.to_string()));
                return false;
            }
        };
        let expanded = self
            .as_ref()
            .rust()
            .registry
            .expand_chain(commands.clone())
            .ok();
        if !full_chrome
            && let Some(command) = expanded.as_ref().and_then(|commands| {
                commands
                    .iter()
                    .find(|command| !secondary_command_supported(command))
            })
        {
            self.set_status_text(QString::from(format!(
                "Use {} in the main browser window for this request.",
                command.name
            )));
            return false;
        }
        let eligible = expanded.as_ref().is_some_and(|expanded| {
            expanded.iter().all(|command| {
                self.as_ref()
                    .rust()
                    .registry
                    .resolve(&command.name)
                    .is_ok_and(|definition| !definition.sensitive)
            })
        });
        let durable = expanded
            .as_ref()
            .is_some_and(|expanded| durable_history_commands(expanded));
        if !self.as_mut().execute_parsed_commands(commands) {
            return false;
        }
        let text = text.trim_start_matches(':').trim();
        let limit = self
            .as_ref()
            .rust()
            .config
            .get("history")
            .and_then(|history| history.get("command_limit"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1_000)
            .min(1_000) as usize;
        if eligible && limit > 0 && !text.is_empty() {
            {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.command_history.retain(|entry| entry != text);
                this.command_history.insert(0, text.to_owned());
                this.command_history.truncate(limit);
                this.command_history_index = None;
                this.command_history_draft = None;
            }
            if durable && !self.as_ref().active_profile_is_transient() {
                self.as_mut().queue_command_history_write(text.to_owned());
            }
        }
        self.as_mut().finish_command_entry();
        self.as_mut().runtime_work_available();
        true
    }

    /// Close successful command entry without cancelling accepted work. The
    /// window also uses this when focusing an already open context consumes
    /// submission before command dispatch.
    pub(super) fn finish_command_entry(mut self: Pin<&mut Self>) {
        if self.as_ref().rust().core_mode == Mode::Command {
            if let Some(window) = self.as_ref().rust().window {
                let _ = self.as_mut().reduce_event(Event::Escape { window });
            }
            self.as_mut().set_core_mode(Mode::Normal);
            self.as_mut().set_mode(QString::from("normal"));
            self.as_mut().clear_completion();
        }
    }

    pub(super) fn command_history_previous(mut self: Pin<&mut Self>, current: &QString) -> QString {
        let current = current.to_string();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.command_history_index = reconcile_history_cursor(
            &this.command_history,
            this.command_history_index,
            this.command_history_draft.as_deref(),
            &current,
        );
        let next = this
            .command_history_index
            .map_or(0, |index| index.saturating_add(1));
        if let Some(entry) = this.command_history.get(next) {
            if this.command_history_index.is_none() {
                this.command_history_draft = Some(current);
            }
            this.command_history_index = Some(next);
            QString::from(entry.as_str())
        } else {
            QString::from(current)
        }
    }

    pub(super) fn command_history_next(mut self: Pin<&mut Self>, current: &QString) -> QString {
        let current = current.to_string();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.command_history_index = reconcile_history_cursor(
            &this.command_history,
            this.command_history_index,
            this.command_history_draft.as_deref(),
            &current,
        );
        let Some(index) = this.command_history_index else {
            return QString::from(current);
        };
        if index == 0 {
            this.command_history_index = None;
            QString::from(this.command_history_draft.as_deref().unwrap_or(&current))
        } else {
            this.command_history_index = Some(index - 1);
            QString::from(this.command_history[index - 1].as_str())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ParsedCommand, durable_history_commands, reconcile_history_cursor,
        secondary_command_supported,
    };

    #[test]
    fn history_cursor_follows_recovered_recorded_lines() {
        let history = vec!["recent".into(), "middle".into(), "older 🦀".into()];
        for (cursor, current, expected) in [
            (Some(2), "recent", Some(0)),
            (Some(0), "older 🦀", Some(2)),
            (Some(1), "middle", Some(1)),
            (Some(8), "middle", Some(1)),
            (None, "middle", Some(1)),
        ] {
            assert_eq!(
                reconcile_history_cursor(&history, cursor, Some("draft"), current),
                expected,
                "cursor={cursor:?}, current={current}"
            );
        }
    }

    #[test]
    fn history_cursor_preserves_drafts_and_detaches_unrecorded_edits() {
        let history = vec!["recent".into(), "older".into()];
        assert_eq!(
            reconcile_history_cursor(&history, Some(1), Some("draft"), "draft"),
            None
        );
        assert_eq!(
            reconcile_history_cursor(&history, Some(1), Some("draft"), "new edit"),
            None
        );
        assert_eq!(
            reconcile_history_cursor(&history, Some(1), Some("draft"), "recently edited"),
            None
        );
        // A recovered draft wins over an equal historical value. An explicit
        // recall already at that entry remains a valid history selection.
        assert_eq!(
            reconcile_history_cursor(&history, Some(1), Some("recent"), "recent"),
            None
        );
        assert_eq!(
            reconcile_history_cursor(&history, Some(0), Some("recent"), "recent"),
            Some(0)
        );
        assert_eq!(
            reconcile_history_cursor(&[], Some(0), Some("draft"), "recent"),
            None
        );
        assert_eq!(
            reconcile_history_cursor(&history, None, None, "recent"),
            None
        );
        assert_eq!(
            reconcile_history_cursor(&history, Some(0), None, "recent"),
            None
        );
        assert_eq!(
            reconcile_history_cursor(&history, None, Some(""), "recent"),
            Some(0)
        );
        assert_eq!(reconcile_history_cursor(&history, None, Some(""), ""), None);
    }

    fn command(name: &str, arguments: &[&str]) -> ParsedCommand {
        ParsedCommand {
            name: name.into(),
            arguments: arguments
                .iter()
                .map(|argument| (*argument).into())
                .collect(),
        }
    }

    #[test]
    fn durable_history_excludes_queries_files_and_arbitrary_arguments() {
        assert!(durable_history_commands(&[command("reload", &[])]));
        assert!(durable_history_commands(&[command(
            "open",
            &["https://example.test/page"]
        )]));
        for input in [
            command("open", &["https://example.test/?q=private"]),
            command("open", &["https://user:secret@example.test/"]),
            command("open", &["file:///home/example/private.txt"]),
            command("set", &["password=secret"]),
        ] {
            assert!(!durable_history_commands(&[input]));
        }
    }

    #[test]
    fn secondary_windows_reject_commands_their_chrome_cannot_present() {
        for name in ["history", "help"] {
            assert!(!secondary_command_supported(&command(name, &[])), "{name}");
        }
        for name in [
            "downloads",
            "download-pause",
            "download-resume",
            "download-cancel",
            "download-retry",
            "reload",
            "zoom",
            "get",
            "version",
            "diagnostics",
            "switcher",
            "history-open",
            "tab-close",
            "tab-open",
            "tab-next",
            "tab-prev",
        ] {
            assert!(secondary_command_supported(&command(name, &[])), "{name}");
        }
        assert!(secondary_command_supported(&command(
            "scroll-target",
            &["auto"]
        )));
        assert!(!secondary_command_supported(&command(
            "open",
            &["--target", "window", "https://example.test"]
        )));
        assert!(secondary_command_supported(&command(
            "open",
            &["https://example.test"]
        )));
    }
}
