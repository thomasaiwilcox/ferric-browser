use super::{
    CxxQtType, ParseInput, ParsedCommand, Pin, QString, is_safe_history_url, parse_chain, qobject,
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

impl qobject::BrowserUi {
    pub(super) fn execute_interactive_command(mut self: Pin<&mut Self>, input: &QString) -> bool {
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
                this.command_history_draft.clear();
            }
            if durable && !self.as_ref().active_profile_is_transient() {
                self.as_mut().queue_command_history_write(text.to_owned());
            }
        }
        true
    }

    pub(super) fn command_history_previous(mut self: Pin<&mut Self>, current: &QString) -> QString {
        let current = current.to_string();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if let Some(index) = this.command_history_index
            && this
                .command_history
                .get(index)
                .is_none_or(|entry| entry != &current)
        {
            this.command_history_index = None;
        }
        let next = this
            .command_history_index
            .map_or(0, |index| index.saturating_add(1));
        if let Some(entry) = this.command_history.get(next) {
            if this.command_history_index.is_none() {
                this.command_history_draft = current;
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
        let Some(index) = this.command_history_index else {
            return QString::from(current);
        };
        if this
            .command_history
            .get(index)
            .is_none_or(|entry| entry != &current)
        {
            this.command_history_index = None;
            return QString::from(current);
        }
        if index == 0 {
            this.command_history_index = None;
            QString::from(this.command_history_draft.as_str())
        } else {
            this.command_history_index = Some(index - 1);
            QString::from(this.command_history[index - 1].as_str())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ParsedCommand, durable_history_commands};

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
}
