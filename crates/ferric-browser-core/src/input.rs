use std::collections::BTreeMap;

use crate::{
    CommandError, CommandRegistry, CountPolicy, Mode, ParseInput, ParsedCommand, parse_chain,
};

pub const DEFAULT_CHORD_TIMEOUT_MS: u64 = 1_000;
const MAX_COUNT: u32 = 9_999;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingDefinition {
    pub mode: Mode,
    pub keys: Vec<String>,
    pub command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingError {
    EmptySequence,
    Duplicate(Vec<String>),
    UnknownCommand(CommandError),
}

fn parse_binding_command(command: &str) -> Result<ParsedCommand, BindingError> {
    let mut commands =
        parse_chain(command, ParseInput::Interactive).map_err(BindingError::UnknownCommand)?;
    if commands.len() != 1 {
        return Err(BindingError::UnknownCommand(CommandError::TooManyCommands));
    }
    Ok(commands.remove(0))
}

impl std::fmt::Display for BindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySequence => formatter.write_str("binding sequence is empty"),
            Self::Duplicate(keys) => write!(formatter, "duplicate binding: {keys:?}"),
            Self::UnknownCommand(error) => write!(formatter, "binding command is invalid: {error}"),
        }
    }
}

impl std::error::Error for BindingError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingOutcome {
    Execute {
        command: String,
        count: u32,
    },
    Pending {
        prefix: Vec<String>,
        continuations: Vec<String>,
        count: u32,
        deadline_ms: u64,
    },
    Consumed,
    Rejected {
        reason: String,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TrieNode {
    command: Option<String>,
    children: BTreeMap<String, TrieNode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingTrie {
    roots: BTreeMap<Mode, TrieNode>,
    registry: CommandRegistry,
}

/// The authoritative catalog of keyboard bindings.
///
/// A [`BindingTrie`] is the optimized runtime representation; this wrapper
/// keeps the declarative catalog as the public boundary consumed by generated
/// documentation and presentation code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingRegistry {
    trie: BindingTrie,
}

impl BindingRegistry {
    /// Builds the qutebrowser-compatible default binding catalog.
    ///
    /// # Errors
    ///
    /// Returns [`BindingError`] when a built-in binding no longer resolves
    /// through the supplied command registry.
    pub fn default_v1(commands: CommandRegistry) -> Result<Self, BindingError> {
        BindingTrie::default_v1(commands).map(|trie| Self { trie })
    }

    /// Returns the declarative bindings in deterministic mode/key order.
    #[must_use]
    pub fn definitions(&self) -> Vec<BindingDefinition> {
        self.trie.definitions()
    }

    /// Creates an input resolver for one current browser mode.
    #[must_use]
    pub fn resolver(&self, mode: Mode) -> BindingResolver {
        BindingResolver::new(self.trie.clone(), mode)
    }
}

impl BindingTrie {
    /// Builds a binding trie after validating every command against the shared
    /// command registry and its declared mode.
    ///
    /// # Errors
    ///
    /// Returns an error for empty or duplicate sequences, or a command that is
    /// unknown/unavailable in the binding's mode.
    pub fn new(
        registry: CommandRegistry,
        definitions: impl IntoIterator<Item = BindingDefinition>,
    ) -> Result<Self, BindingError> {
        let mut trie = Self {
            roots: BTreeMap::new(),
            registry,
        };
        for definition in definitions {
            if definition.keys.is_empty() || definition.keys.iter().any(String::is_empty) {
                return Err(BindingError::EmptySequence);
            }
            let command = parse_binding_command(&definition.command)?;
            trie.registry
                .validate(&command, definition.mode)
                .map_err(BindingError::UnknownCommand)?;
            let mut node = trie.roots.entry(definition.mode).or_default();
            for key in &definition.keys {
                node = node.children.entry(key.clone()).or_default();
            }
            if node.command.is_some() {
                return Err(BindingError::Duplicate(definition.keys));
            }
            node.command = Some(definition.command);
        }
        Ok(trie)
    }

    ///
    /// # Errors
    ///
    /// Returns [`BindingError`] if the built-in bindings do not match the
    /// supplied command registry.
    #[allow(clippy::too_many_lines)]
    pub fn default_v1(registry: CommandRegistry) -> Result<Self, BindingError> {
        Self::new(
            registry,
            [
                binding(Mode::Normal, &["o"], "open"),
                binding(Mode::Normal, &["O"], "tab-open"),
                binding(Mode::Normal, &["g", "o"], "open-current"),
                binding(Mode::Normal, &["g", "O"], "open-current --target tab"),
                binding(Mode::Normal, &["p", "p"], "paste-open"),
                binding(Mode::Normal, &["p", "P"], "paste-open --primary"),
                binding(Mode::Normal, &["P", "p"], "paste-open --target tab"),
                binding(
                    Mode::Normal,
                    &["P", "P"],
                    "paste-open --target tab --primary",
                ),
                binding(Mode::Normal, &["H"], "back"),
                binding(Mode::Normal, &["L"], "forward"),
                binding(Mode::Normal, &["Back"], "back"),
                binding(Mode::Normal, &["Forward"], "forward"),
                binding(Mode::Normal, &["r"], "reload"),
                binding(Mode::Normal, &["R"], "reload --bypass-cache"),
                binding(Mode::Normal, &["F5"], "reload"),
                binding(Mode::Normal, &["Ctrl+F5"], "reload --bypass-cache"),
                binding(Mode::Normal, &["s"], "stop"),
                binding(Mode::Normal, &["Ctrl+s"], "stop"),
                binding(Mode::Normal, &["h"], "scroll left"),
                binding(Mode::Normal, &["j"], "scroll down"),
                binding(Mode::Normal, &["k"], "scroll up"),
                binding(Mode::Normal, &["l"], "scroll right"),
                binding(Mode::Normal, &["Ctrl+d"], "scroll-page down --half"),
                binding(Mode::Normal, &["Ctrl+u"], "scroll-page up --half"),
                binding(Mode::Normal, &["Ctrl+f"], "scroll-page down"),
                binding(Mode::Normal, &["Ctrl+b"], "scroll-page up"),
                binding(Mode::Normal, &["g", "g"], "scroll-to top"),
                binding(Mode::Normal, &["G"], "scroll-to bottom"),
                binding(Mode::Normal, &["f"], "hint all"),
                binding(Mode::Normal, &["F"], "hint --target tab all"),
                binding(Mode::Normal, &[";", "b"], "hint --target tab-bg all"),
                binding(Mode::Normal, &[";", "f"], "hint --target tab all"),
                binding(Mode::Normal, &[";", "d"], "hint --target download links"),
                binding(
                    Mode::Normal,
                    &[";", "r"],
                    "hint --rapid --target tab-bg links",
                ),
                binding(Mode::Normal, &[";", "y"], "hint --target yank links"),
                binding(Mode::Normal, &[";", "a"], "hint --target choose all"),
                binding(Mode::Normal, &["g", "i"], "hint --first inputs"),
                binding(Mode::Normal, &["i"], "mode-enter insert"),
                binding(Mode::Normal, &["v"], "mode-enter caret"),
                binding(Mode::Normal, &["y", "y"], "yank url"),
                binding(Mode::Normal, &["y", "t"], "yank title"),
                binding(Mode::Normal, &["m"], "quickmark-add"),
                binding(Mode::Normal, &["b"], "quickmark-open"),
                binding(Mode::Normal, &["M"], "bookmark-add"),
                binding(Mode::Normal, &["g", "b"], "bookmark-open"),
                binding(Mode::Normal, &["g", "C"], "tab-clone"),
                binding(Mode::Normal, &["g", "f"], "view-source"),
                binding(Mode::Normal, &["g", "J"], "tab-move right"),
                binding(Mode::Normal, &["g", "K"], "tab-move left"),
                binding(Mode::Normal, &["."], "repeat"),
                binding(Mode::Normal, &["S", "h"], "history"),
                binding(Mode::Normal, &["S", "q"], "bookmark-list"),
                binding(Mode::Normal, &["T"], "switcher --scope tabs"),
                binding(Mode::Normal, &["g", "a"], "tab-open about:blank"),
                binding(Mode::Normal, &["Ctrl+t"], "tab-open about:blank"),
                binding(Mode::Normal, &["J"], "tab-next"),
                binding(Mode::Normal, &["K"], "tab-prev"),
                binding(Mode::Normal, &["g", "t"], "switcher --scope tabs"),
                binding(Mode::Normal, &["g", "$"], "tab-select last"),
                binding(Mode::Normal, &["g", "0"], "tab-select 1"),
                binding(Mode::Normal, &["g", "^"], "tab-select 1"),
                binding(Mode::Normal, &["Ctrl+PgDown"], "tab-next"),
                binding(Mode::Normal, &["Ctrl+PgUp"], "tab-prev"),
                binding(Mode::Normal, &["Ctrl+Tab"], "tab-select previous"),
                binding(Mode::Normal, &["Ctrl+^"], "tab-select previous"),
                binding(Mode::Normal, &["Alt+1"], "tab-select 1"),
                binding(Mode::Normal, &["Alt+2"], "tab-select 2"),
                binding(Mode::Normal, &["Alt+3"], "tab-select 3"),
                binding(Mode::Normal, &["Alt+4"], "tab-select 4"),
                binding(Mode::Normal, &["Alt+5"], "tab-select 5"),
                binding(Mode::Normal, &["Alt+6"], "tab-select 6"),
                binding(Mode::Normal, &["Alt+7"], "tab-select 7"),
                binding(Mode::Normal, &["Alt+8"], "tab-select 8"),
                binding(Mode::Normal, &["Alt+9"], "tab-select last"),
                binding(Mode::Normal, &["Alt+m"], "tab-mute"),
                binding(Mode::Normal, &["g", "?"], "binding-list"),
                binding(Mode::Normal, &["Ctrl+p"], "tab-pin"),
                binding(Mode::Normal, &["Ctrl+Space"], "switcher"),
                binding(Mode::Normal, &["Ctrl+v"], "mode-enter passthrough"),
                binding(Mode::Normal, &["d"], "tab-close"),
                binding(Mode::Normal, &["u"], "tab-undo"),
                binding(Mode::Normal, &["Ctrl+w"], "tab-close"),
                binding(Mode::Normal, &["Ctrl+Shift+t"], "tab-undo"),
                binding(Mode::Normal, &["Ctrl+n"], "window-new"),
                binding(Mode::Normal, &["Ctrl+Shift+n"], "window-new --private"),
                binding(Mode::Normal, &["Ctrl+Shift+w"], "window-close"),
                binding(Mode::Normal, &["Ctrl+q"], "quit"),
                binding(Mode::Normal, &["Ctrl+Alt+p"], "print"),
                binding(Mode::Normal, &["w", "i"], "devtools"),
                binding(Mode::Normal, &["Z", "Q"], "quit"),
                binding(Mode::Normal, &["F11"], "fullscreen"),
                binding(Mode::Normal, &["+"], "zoom in"),
                binding(Mode::Normal, &["-"], "zoom out"),
                binding(Mode::Normal, &["="], "zoom reset"),
            ],
        )
    }

    #[must_use]
    fn root(&self, mode: Mode) -> Option<&TrieNode> {
        self.roots.get(&mode)
    }

    #[must_use]
    fn definition(&self, command: &str) -> Option<&crate::CommandDefinition> {
        command
            .split_ascii_whitespace()
            .next()
            .and_then(|name| self.registry.resolve(name).ok())
    }

    /// Returns the effective bindings in deterministic mode/key order.
    #[must_use]
    pub fn definitions(&self) -> Vec<BindingDefinition> {
        fn collect(
            mode: Mode,
            keys: &mut Vec<String>,
            node: &TrieNode,
            output: &mut Vec<BindingDefinition>,
        ) {
            if let Some(command) = node.command.as_ref() {
                output.push(BindingDefinition {
                    mode,
                    keys: keys.clone(),
                    command: command.clone(),
                });
            }
            for (key, child) in &node.children {
                keys.push(key.clone());
                collect(mode, keys, child, output);
                keys.pop();
            }
        }

        let mut output = Vec::new();
        for (mode, root) in &self.roots {
            collect(*mode, &mut Vec::new(), root, &mut output);
        }
        output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingResolver {
    trie: BindingTrie,
    mode: Mode,
    prefix: Vec<String>,
    count_digits: String,
    deadline_ms: Option<u64>,
}

impl BindingResolver {
    #[must_use]
    pub fn new(trie: BindingTrie, mode: Mode) -> Self {
        Self {
            trie,
            mode,
            prefix: Vec::new(),
            count_digits: String::new(),
            deadline_ms: None,
        }
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if self.mode != mode {
            self.reset();
            self.mode = mode;
        }
    }

    pub fn reset(&mut self) {
        self.prefix.clear();
        self.count_digits.clear();
        self.deadline_ms = None;
    }

    #[must_use]
    pub fn definitions(&self) -> Vec<BindingDefinition> {
        self.trie.definitions()
    }

    /// Feeds one logical key token into the current mode's trie.
    #[must_use]
    pub fn feed(&mut self, key: &str, now_ms: u64) -> BindingOutcome {
        if key.is_empty() {
            return BindingOutcome::Consumed;
        }

        if self.prefix.is_empty() && self.mode == Mode::Normal && self.is_count_key(key) {
            self.count_digits.push_str(key);
            let count = self.count();
            self.deadline_ms = Some(now_ms.saturating_add(DEFAULT_CHORD_TIMEOUT_MS));
            return BindingOutcome::Pending {
                prefix: Vec::new(),
                continuations: self.continuations(),
                count,
                deadline_ms: self.deadline_ms.unwrap_or(now_ms),
            };
        }

        let mut candidate = self.prefix.clone();
        candidate.push(key.into());
        let Some(node) = self.node(&candidate) else {
            self.reset();
            return BindingOutcome::Consumed;
        };
        let command = node.command.clone();
        let continuations: Vec<String> = node.children.keys().cloned().collect();

        self.prefix = candidate;
        self.deadline_ms = Some(now_ms.saturating_add(DEFAULT_CHORD_TIMEOUT_MS));
        if let Some(command) = command.as_deref()
            && continuations.is_empty()
        {
            return self.execute(command);
        }

        BindingOutcome::Pending {
            prefix: self.prefix.clone(),
            continuations,
            count: self.count(),
            deadline_ms: self.deadline_ms.unwrap_or(now_ms),
        }
    }

    /// Resolves an exact prefix once its ambiguity timeout has elapsed.
    #[must_use]
    pub fn tick(&mut self, now_ms: u64) -> Option<BindingOutcome> {
        if self.deadline_ms.is_none_or(|deadline| now_ms < deadline) {
            return None;
        }
        let command = self
            .node(&self.prefix)
            .and_then(|node| node.command.clone());
        let outcome = command.map(|command| self.execute(&command));
        if outcome.is_none() {
            self.reset();
        }
        outcome
    }

    #[must_use]
    fn node(&self, keys: &[String]) -> Option<&TrieNode> {
        let mut node = self.trie.root(self.mode)?;
        for key in keys {
            node = node.children.get(key)?;
        }
        Some(node)
    }

    #[must_use]
    fn continuations(&self) -> Vec<String> {
        self.node(&self.prefix)
            .map_or_else(Vec::new, |node| node.children.keys().cloned().collect())
    }

    #[must_use]
    fn is_count_key(&self, key: &str) -> bool {
        (key.len() == 1 && key != "0" && key.chars().all(|character| character.is_ascii_digit()))
            || (!self.count_digits.is_empty()
                && key.len() == 1
                && key.chars().all(|character| character.is_ascii_digit()))
    }

    #[must_use]
    fn count(&self) -> u32 {
        if self.count_digits.is_empty() {
            1
        } else {
            self.count_digits
                .parse()
                .unwrap_or(MAX_COUNT)
                .min(MAX_COUNT)
        }
    }

    #[must_use]
    fn execute(&mut self, command: &str) -> BindingOutcome {
        let count = self.count();
        let outcome = match self
            .trie
            .definition(command)
            .map(|definition| definition.count)
        {
            Some(CountPolicy::NotSupported) if !self.count_digits.is_empty() => {
                BindingOutcome::Rejected {
                    reason: format!("command {command} does not accept a count"),
                }
            }
            Some(CountPolicy::Supported { maximum }) => BindingOutcome::Execute {
                command: command.into(),
                count: count.min(maximum),
            },
            Some(CountPolicy::NotSupported) | None => BindingOutcome::Execute {
                command: command.into(),
                count: 1,
            },
        };
        self.reset();
        outcome
    }
}

fn binding(mode: Mode, keys: &[&str], command: &str) -> BindingDefinition {
    BindingDefinition {
        mode,
        keys: keys.iter().map(|key| (*key).into()).collect(),
        command: command.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_registry_exposes_default_definitions_and_resolvers() {
        let registry = BindingRegistry::default_v1(CommandRegistry::default_v1())
            .expect("default bindings resolve through command registry");
        assert!(
            registry
                .definitions()
                .iter()
                .any(|binding| binding.keys == vec!["o"] && binding.command == "open")
        );
        let mut resolver = registry.resolver(Mode::Normal);
        assert!(matches!(
            resolver.feed("o", 0),
            BindingOutcome::Execute { command, count: 1 } if command == "open"
        ));
    }

    #[test]
    fn exact_leaf_executes_and_count_limits_are_applied() {
        let trie = BindingTrie::default_v1(CommandRegistry::default_v1()).unwrap();
        let mut resolver = BindingResolver::new(trie, Mode::Normal);
        assert_eq!(
            resolver.feed("r", 0),
            BindingOutcome::Execute {
                command: "reload".into(),
                count: 1,
            }
        );
        assert!(matches!(
            resolver.feed("y", 0),
            BindingOutcome::Pending { .. }
        ));
        assert_eq!(
            resolver.feed("y", 1),
            BindingOutcome::Execute {
                command: "yank url".into(),
                count: 1,
            }
        );
        assert!(matches!(
            resolver.feed("3", 0),
            BindingOutcome::Pending { count: 3, .. }
        ));
        assert_eq!(
            resolver.feed("H", 10),
            BindingOutcome::Execute {
                command: "back".into(),
                count: 3,
            }
        );
        for _ in 0..5 {
            let _ = resolver.feed("9", 20);
        }
        assert_eq!(
            resolver.feed("H", 30),
            BindingOutcome::Execute {
                command: "back".into(),
                count: 100,
            }
        );
    }

    #[test]
    fn ambiguous_prefix_waits_then_runs_exact_leaf() {
        let registry = CommandRegistry::default_v1();
        let trie = BindingTrie::new(
            registry,
            [
                binding(Mode::Normal, &["g"], "open"),
                binding(Mode::Normal, &["g", "g"], "reload"),
            ],
        )
        .unwrap();
        let mut resolver = BindingResolver::new(trie, Mode::Normal);
        assert!(matches!(
            resolver.feed("g", 0),
            BindingOutcome::Pending { .. }
        ));
        assert_eq!(resolver.tick(999), None);
        assert_eq!(
            resolver.tick(1_000),
            Some(BindingOutcome::Execute {
                command: "open".into(),
                count: 1,
            })
        );
    }

    #[test]
    fn invalid_and_unmatched_bindings_are_safe() {
        let registry = CommandRegistry::default_v1();
        assert_eq!(
            BindingTrie::new(
                registry.clone(),
                [BindingDefinition {
                    mode: Mode::Normal,
                    keys: Vec::new(),
                    command: "open".into(),
                }],
            ),
            Err(BindingError::EmptySequence)
        );
        let trie = BindingTrie::default_v1(registry).unwrap();
        let mut resolver = BindingResolver::new(trie, Mode::Normal);
        assert_eq!(resolver.feed("unexpected", 0), BindingOutcome::Consumed);
    }

    #[test]
    fn binding_commands_may_include_registered_arguments() {
        let trie = BindingTrie::new(
            CommandRegistry::default_v1(),
            [binding(Mode::Normal, &["g"], "open https://example.test")],
        )
        .expect("argument-bearing binding");
        let mut resolver = BindingResolver::new(trie, Mode::Normal);
        assert_eq!(
            resolver.feed("g", 0),
            BindingOutcome::Execute {
                command: "open https://example.test".into(),
                count: 1,
            }
        );
    }

    #[test]
    fn default_bindings_include_half_page_control_keys() {
        let trie = BindingTrie::default_v1(CommandRegistry::default_v1()).unwrap();
        let definitions = trie.definitions();
        assert!(definitions.iter().any(|binding| {
            binding.mode == Mode::Normal
                && binding.keys == vec!["Ctrl+d".to_owned()]
                && binding.command == "scroll-page down --half"
        }));
        assert!(definitions.iter().any(|binding| {
            binding.mode == Mode::Normal
                && binding.keys == vec!["Ctrl+u".to_owned()]
                && binding.command == "scroll-page up --half"
        }));
    }

    #[test]
    fn default_bindings_include_choose_and_indexable_first_input_hint() {
        let trie = BindingTrie::default_v1(CommandRegistry::default_v1()).unwrap();
        let definitions = trie.definitions();
        assert!(definitions.iter().any(|binding| {
            binding.keys == [";", "a"] && binding.command == "hint --target choose all"
        }));
        assert!(definitions.iter().any(|binding| {
            binding.keys == ["g", "i"] && binding.command == "hint --first inputs"
        }));

        let mut resolver = BindingResolver::new(trie, Mode::Normal);
        assert_eq!(
            resolver.feed("f", 0),
            BindingOutcome::Execute {
                command: "hint all".into(),
                count: 1,
            }
        );
        assert!(matches!(
            resolver.feed("g", 1),
            BindingOutcome::Pending { count: 1, .. }
        ));
        assert_eq!(
            resolver.feed("i", 2),
            BindingOutcome::Execute {
                command: "hint --first inputs".into(),
                count: 1,
            }
        );
        assert!(matches!(
            resolver.feed("3", 3),
            BindingOutcome::Pending { count: 3, .. }
        ));
        assert!(matches!(
            resolver.feed("g", 4),
            BindingOutcome::Pending { count: 3, .. }
        ));
        assert_eq!(
            resolver.feed("i", 5),
            BindingOutcome::Execute {
                command: "hint --first inputs".into(),
                count: 3,
            }
        );
    }

    #[test]
    fn default_bindings_cover_documented_navigation_and_copy_actions() {
        let trie = BindingTrie::default_v1(CommandRegistry::default_v1()).unwrap();
        let definitions = trie.definitions();
        for (keys, command) in [
            (vec!["O".to_owned()], "tab-open"),
            (vec!["g".to_owned(), "o".to_owned()], "open-current"),
            (
                vec!["g".to_owned(), "O".to_owned()],
                "open-current --target tab",
            ),
            (vec!["R".to_owned()], "reload --bypass-cache"),
            (vec!["f".to_owned()], "hint all"),
            (vec!["F".to_owned()], "hint --target tab all"),
            (vec!["i".to_owned()], "mode-enter insert"),
            (vec!["v".to_owned()], "mode-enter caret"),
            (
                vec!["P".to_owned(), "p".to_owned()],
                "paste-open --target tab",
            ),
            (
                vec![";".to_owned(), "f".to_owned()],
                "hint --target tab all",
            ),
            (vec!["y".to_owned(), "y".to_owned()], "yank url"),
            (vec!["y".to_owned(), "t".to_owned()], "yank title"),
            (vec!["m".to_owned()], "quickmark-add"),
            (vec!["b".to_owned()], "quickmark-open"),
            (vec!["T".to_owned()], "switcher --scope tabs"),
            (vec!["J".to_owned()], "tab-next"),
            (vec!["K".to_owned()], "tab-prev"),
            (vec!["d".to_owned()], "tab-close"),
            (vec!["u".to_owned()], "tab-undo"),
            (vec!["+".to_owned()], "zoom in"),
            (vec!["-".to_owned()], "zoom out"),
            (vec!["=".to_owned()], "zoom reset"),
            (vec!["Ctrl+p".to_owned()], "tab-pin"),
            (vec!["Ctrl+Space".to_owned()], "switcher"),
            (vec!["Ctrl+v".to_owned()], "mode-enter passthrough"),
            (vec!["g".to_owned(), "?".to_owned()], "binding-list"),
        ] {
            assert_eq!(
                definitions
                    .iter()
                    .find(|binding| binding.keys == keys)
                    .map(|binding| binding.command.as_str()),
                Some(command),
                "missing documented binding {keys:?}"
            );
        }
    }

    #[test]
    fn default_bindings_follow_qutebrowser_compatibility_baseline() {
        let trie = BindingTrie::default_v1(CommandRegistry::default_v1()).unwrap();
        let definitions = trie.definitions();
        for (keys, command) in [
            (vec!["f".to_owned()], "hint all"),
            (vec!["g".to_owned(), "o".to_owned()], "open-current"),
            (
                vec!["g".to_owned(), "O".to_owned()],
                "open-current --target tab",
            ),
            (vec!["g".to_owned(), "a".to_owned()], "tab-open about:blank"),
            (vec!["g".to_owned(), "b".to_owned()], "bookmark-open"),
            (vec!["g".to_owned(), "C".to_owned()], "tab-clone"),
            (vec!["g".to_owned(), "f".to_owned()], "view-source"),
            (vec!["g".to_owned(), "J".to_owned()], "tab-move right"),
            (vec!["g".to_owned(), "K".to_owned()], "tab-move left"),
            (vec!["M".to_owned()], "bookmark-add"),
            (vec!["S".to_owned(), "h".to_owned()], "history"),
            (vec!["S".to_owned(), "q".to_owned()], "bookmark-list"),
            (vec!["Alt+1".to_owned()], "tab-select 1"),
            (vec!["Alt+9".to_owned()], "tab-select last"),
            (vec!["Alt+m".to_owned()], "tab-mute"),
            (vec!["g".to_owned(), "$".to_owned()], "tab-select last"),
            (vec!["Ctrl+t".to_owned()], "tab-open about:blank"),
            (vec!["Ctrl+w".to_owned()], "tab-close"),
            (vec!["Ctrl+Shift+t".to_owned()], "tab-undo"),
            (vec!["Ctrl+n".to_owned()], "window-new"),
            (vec!["Ctrl+Shift+n".to_owned()], "window-new --private"),
            (vec!["Ctrl+Shift+w".to_owned()], "window-close"),
            (vec!["Ctrl+q".to_owned()], "quit"),
            (vec!["Ctrl+Alt+p".to_owned()], "print"),
            (vec!["F5".to_owned()], "reload"),
            (vec!["Ctrl+F5".to_owned()], "reload --bypass-cache"),
            (vec!["F11".to_owned()], "fullscreen"),
            (vec!["w".to_owned(), "i".to_owned()], "devtools"),
            (vec!["Z".to_owned(), "Q".to_owned()], "quit"),
        ] {
            assert_eq!(
                definitions
                    .iter()
                    .find(|binding| binding.keys == keys)
                    .map(|binding| binding.command.as_str()),
                Some(command),
                "missing qutebrowser-compatible binding {keys:?}"
            );
        }
    }
}
