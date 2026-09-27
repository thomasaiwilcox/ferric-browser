use std::fmt;

use crate::{ActionId, Mode};

const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_COMMANDS: usize = 64;
const MAX_ARGUMENTS: usize = 256;
const MAX_ALIAS_DEPTH: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseInput {
    Interactive,
    Cli,
    Ipc,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedCommand {
    pub name: String,
    pub arguments: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandError {
    Empty,
    TooLong,
    TooManyCommands,
    TooManyArguments,
    Nul,
    ControlCharacter(char),
    UnterminatedQuote,
    TrailingEscape,
    EmptyCommand,
    InvalidName(String),
    UnknownCommand(String),
    DuplicateAlias(String),
    AliasCycle(String),
    AliasDepth,
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("command input is empty"),
            Self::TooLong => formatter.write_str("command input exceeds 64 KiB"),
            Self::TooManyCommands => formatter.write_str("command chain exceeds 64 commands"),
            Self::TooManyArguments => formatter.write_str("command exceeds 256 arguments"),
            Self::Nul => formatter.write_str("command contains NUL"),
            Self::ControlCharacter(character) => {
                write!(
                    formatter,
                    "command contains disallowed control character {character:?}"
                )
            }
            Self::UnterminatedQuote => formatter.write_str("command has an unterminated quote"),
            Self::TrailingEscape => formatter.write_str("command ends with an incomplete escape"),
            Self::EmptyCommand => formatter.write_str("command chain contains an empty command"),
            Self::InvalidName(name) => write!(formatter, "invalid command name: {name}"),
            Self::UnknownCommand(name) => write!(formatter, "unknown command: {name}"),
            Self::DuplicateAlias(name) => write!(formatter, "duplicate command alias: {name}"),
            Self::AliasCycle(name) => write!(formatter, "command alias cycle at: {name}"),
            Self::AliasDepth => formatter.write_str("command alias expansion exceeds depth 8"),
        }
    }
}

impl std::error::Error for CommandError {}

/// Parses a complete command chain without executing any command.
///
/// `;;` separates commands only outside quotes. Quoting and backslash
/// escaping are decoded into the returned arguments; shell expansion is never
/// attempted, so `$()`, backticks, and glob characters remain ordinary data.
///
/// # Errors
///
/// Returns a typed error for invalid syntax or any documented size limit.
pub fn parse_chain(input: &str, source: ParseInput) -> Result<Vec<ParsedCommand>, CommandError> {
    if input.is_empty() {
        return Err(CommandError::Empty);
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(CommandError::TooLong);
    }

    let mut input = input;
    if input.starts_with(':') {
        if matches!(source, ParseInput::Ipc) {
            return Err(CommandError::InvalidName(":".into()));
        }
        input = &input[1..];
    }
    if input.is_empty() {
        return Err(CommandError::Empty);
    }

    let commands = scan_commands(input)?;
    if commands.is_empty() {
        return Err(CommandError::Empty);
    }
    if commands.len() > MAX_COMMANDS {
        return Err(CommandError::TooManyCommands);
    }
    commands
        .into_iter()
        .map(|words| {
            if words.is_empty() {
                return Err(CommandError::EmptyCommand);
            }
            if words.len() - 1 > MAX_ARGUMENTS {
                return Err(CommandError::TooManyArguments);
            }
            let name = words[0].clone();
            if !is_command_name(&name) {
                return Err(CommandError::InvalidName(name));
            }
            Ok(ParsedCommand {
                name,
                arguments: words.into_iter().skip(1).collect(),
            })
        })
        .collect()
}

fn scan_commands(input: &str) -> Result<Vec<Vec<String>>, CommandError> {
    let mut commands = Vec::new();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut started = false;
    let mut after_boundary = false;
    let mut characters = input.chars().peekable();

    while let Some(character) = characters.next() {
        if escaped {
            validate_character(character)?;
            word.push(character);
            started = true;
            escaped = false;
            after_boundary = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            started = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else {
                validate_character(character)?;
                word.push(character);
            }
            started = true;
            continue;
        }
        match character {
            '\'' | '"' => {
                quote = Some(character);
                started = true;
                after_boundary = false;
            }
            ' ' | '\t' => flush_word(&mut word, &mut words, &mut started),
            ';' if characters.peek() == Some(&';') => {
                characters.next();
                flush_word(&mut word, &mut words, &mut started);
                if words.is_empty() {
                    return Err(CommandError::EmptyCommand);
                }
                commands.push(std::mem::take(&mut words));
                after_boundary = true;
            }
            _ => {
                validate_character(character)?;
                word.push(character);
                started = true;
                after_boundary = false;
            }
        }
    }
    if escaped {
        return Err(CommandError::TrailingEscape);
    }
    if quote.is_some() {
        return Err(CommandError::UnterminatedQuote);
    }
    flush_word(&mut word, &mut words, &mut started);
    if after_boundary {
        return Err(CommandError::EmptyCommand);
    }
    if !words.is_empty() {
        commands.push(words);
    }
    Ok(commands)
}

fn flush_word(word: &mut String, words: &mut Vec<String>, started: &mut bool) {
    if *started {
        words.push(std::mem::take(word));
        *started = false;
    }
}

fn validate_character(character: char) -> Result<(), CommandError> {
    if character == '\0' {
        return Err(CommandError::Nul);
    }
    if character.is_control() {
        return Err(CommandError::ControlCharacter(character));
    }
    Ok(())
}

fn is_command_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CountPolicy {
    NotSupported,
    Supported { maximum: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentKind {
    Text,
    Url,
    Boolean,
    Integer,
    Enum,
    Object,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentDefinition {
    pub name: String,
    pub kind: ArgumentKind,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandScope {
    Application,
    Window,
    Tab,
    Profile,
    Context,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectClass {
    Query,
    Navigation,
    Mutating,
    Sensitive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionProvider {
    None,
    Commands,
    Urls,
    Profiles,
    Tabs,
    Settings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandDefinition {
    pub action: ActionId,
    pub name: String,
    pub aliases: Vec<String>,
    pub modes: Vec<Mode>,
    pub count: CountPolicy,
    pub sensitive: bool,
    pub description: String,
    pub arguments: Vec<ArgumentDefinition>,
    pub scope: CommandScope,
    pub effect: EffectClass,
    pub completion: CompletionProvider,
    pub examples: Vec<String>,
}

/// A parsed-token command alias.
///
/// Aliases deliberately store parsed commands rather than a command string.
/// Expanding one therefore cannot introduce a second shell-like parse pass or
/// turn URL punctuation into command syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandAlias {
    pub name: String,
    pub expansion: Vec<ParsedCommand>,
}

impl CommandAlias {
    #[must_use]
    pub fn new(name: impl Into<String>, expansion: Vec<ParsedCommand>) -> Self {
        Self {
            name: name.into(),
            expansion,
        }
    }

    /// Creates an alias from command text at registration time.
    ///
    /// The text is parsed once and retained as tokens for every later use.
    ///
    /// # Errors
    ///
    /// Returns the same syntax and limit errors as [`parse_chain`].
    pub fn from_text(name: &str, text: &str) -> Result<Self, CommandError> {
        Ok(Self::new(name, parse_chain(text, ParseInput::Cli)?))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandRegistry {
    definitions: Vec<CommandDefinition>,
    expansions: Vec<CommandAlias>,
}

impl CommandRegistry {
    #[must_use]
    pub fn new(mut definitions: Vec<CommandDefinition>) -> Self {
        for (index, definition) in definitions.iter_mut().enumerate() {
            definition.action = ActionId::from_raw((index as u64) + 1);
        }
        Self {
            definitions,
            expansions: Vec::new(),
        }
    }

    /// Builds a registry with parsed-token aliases.
    ///
    /// # Errors
    ///
    /// Returns an error when an alias name is invalid or its expansion is
    /// empty/contains an invalid or unknown command name.
    pub fn new_with_aliases(
        definitions: Vec<CommandDefinition>,
        aliases: Vec<CommandAlias>,
    ) -> Result<Self, CommandError> {
        let mut registry = Self::new(definitions);
        let alias_names = aliases
            .iter()
            .map(|alias| alias.name.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for alias in aliases {
            if !is_command_name(&alias.name) {
                return Err(CommandError::InvalidName(alias.name));
            }
            if registry.definitions.iter().any(|definition| {
                definition.name == alias.name
                    || definition.aliases.iter().any(|name| name == &alias.name)
            }) || registry
                .expansions
                .iter()
                .any(|candidate| candidate.name == alias.name)
            {
                return Err(CommandError::DuplicateAlias(alias.name));
            }
            if alias.expansion.is_empty() {
                return Err(CommandError::EmptyCommand);
            }
            if alias.expansion.len() > MAX_COMMANDS {
                return Err(CommandError::TooManyCommands);
            }
            if alias.expansion.iter().any(|command| {
                !is_command_name(&command.name)
                    || (!registry.definitions.iter().any(|definition| {
                        definition.name == command.name
                            || definition.aliases.iter().any(|name| name == &command.name)
                    }) && !alias_names.contains(command.name.as_str()))
            }) {
                let name = alias
                    .expansion
                    .iter()
                    .find(|command| {
                        !is_command_name(&command.name)
                            || (!registry.definitions.iter().any(|definition| {
                                definition.name == command.name
                                    || definition.aliases.iter().any(|name| name == &command.name)
                            }) && !alias_names.contains(command.name.as_str()))
                    })
                    .map_or_else(String::new, |command| command.name.clone());
                return Err(if is_command_name(&name) {
                    CommandError::UnknownCommand(name)
                } else {
                    CommandError::InvalidName(name)
                });
            }
            if alias
                .expansion
                .iter()
                .any(|command| command.arguments.len() > MAX_ARGUMENTS)
            {
                return Err(CommandError::TooManyArguments);
            }
            registry.expansions.push(alias);
        }
        Ok(registry)
    }

    #[must_use]
    #[allow(clippy::too_many_lines)]
    ///
    /// # Panics
    ///
    /// Panics only if a built-in alias literal fails the command parser; the
    /// bundled aliases are fixed, validated command text.
    pub fn default_v1() -> Self {
        let normal = vec![Mode::Normal, Mode::Command];
        let mut registry = Self::new(vec![
            definition(
                "open",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                false,
                "Navigate to an address or search query.",
            ),
            definition(
                "open-current",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                false,
                "Open the current safe URL in the active tab or a new tab.",
            ),
            definition(
                "paste-open",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                true,
                "Read the user clipboard and navigate only when it contains valid text.",
            ),
            definition(
                "back",
                &[],
                normal.clone(),
                CountPolicy::Supported { maximum: 100 },
                false,
                "Go back in engine history.",
            ),
            definition(
                "forward",
                &[],
                normal.clone(),
                CountPolicy::Supported { maximum: 100 },
                false,
                "Go forward in engine history.",
            ),
            definition(
                "reload",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                false,
                "Reload the current page.",
            ),
            definition(
                "stop",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                false,
                "Stop the current load.",
            ),
            definition(
                "zoom",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                false,
                "Set or adjust the current tab's page zoom factor.",
            ),
            definition(
                "search-next",
                &[],
                normal.clone(),
                CountPolicy::Supported { maximum: 100 },
                false,
                "Traverse retained in-page search matches.",
            ),
            definition(
                "search",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                false,
                "Search the current page without navigating.",
            ),
            definition(
                "scroll",
                &[],
                normal.clone(),
                CountPolicy::Supported { maximum: 9_999 },
                false,
                "Scroll the resolved page scroll target in one direction.",
            ),
            definition(
                "scroll-page",
                &[],
                normal.clone(),
                CountPolicy::Supported { maximum: 9_999 },
                false,
                "Scroll the resolved page scroll target by viewport pages.",
            ),
            definition(
                "scroll-to",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                false,
                "Scroll the resolved page scroll target to its top or bottom.",
            ),
            definition(
                "scroll-target",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                false,
                "Select, clear, inspect, or pin the active page's keyboard scroll target.",
            ),
            definition(
                "window-close",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                false,
                "Request close of the owning browser window.",
            ),
            definition(
                "window-new",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                false,
                "Create a normal or private browser window.",
            ),
            definition(
                "fullscreen",
                &[],
                normal.clone(),
                CountPolicy::NotSupported,
                false,
                "Enter, leave, or toggle the owning browser window fullscreen state.",
            ),
            definition(
                "quit",
                &[],
                normal,
                CountPolicy::NotSupported,
                false,
                "Request coordinated application shutdown.",
            ),
            definition(
                "set",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Set a typed runtime configuration value.",
            ),
            definition(
                "get",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Show the effective configuration value and its source.",
            ),
            definition(
                "unset",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Remove a generated runtime configuration value.",
            ),
            definition(
                "config-export",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Write a reviewed merged configuration to a new file.",
            ),
            definition(
                "config-edit",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open the trusted file-backed configuration in the configured editor.",
            ),
            definition(
                "config-reload",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Reload the configured file-backed configuration atomically.",
            ),
            definition(
                "config-check",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Validate the configured files without applying changes.",
            ),
            definition(
                "config-write-defaults",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Write a default configuration template to a new path.",
            ),
            definition(
                "theme-reload",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Refresh the configured theme palette and provider status.",
            ),
            definition(
                "bind",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Bind a key sequence to a registered command.",
            ),
            definition(
                "unbind",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Write an explicit unbound marker for a key sequence.",
            ),
            definition(
                "hint",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::Supported { maximum: 5_000 },
                false,
                "Label visible page links and controls for validated activation.",
            ),
            definition(
                "grid",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Enter keyboard-controlled spatial Grid mode for the active page view.",
            ),
            definition(
                "grid-refine",
                &[],
                vec![Mode::Grid],
                CountPolicy::NotSupported,
                false,
                "Refine the active spatial region by one row-major grid cell.",
            ),
            definition(
                "grid-click",
                &[],
                vec![Mode::Grid],
                CountPolicy::NotSupported,
                false,
                "Request one native pointer click at the spatial crosshair.",
            ),
            definition(
                "grid-hover",
                &[],
                vec![Mode::Grid],
                CountPolicy::NotSupported,
                false,
                "Request one native pointer hover at the spatial crosshair.",
            ),
            definition(
                "grid-back",
                &[],
                vec![Mode::Grid],
                CountPolicy::NotSupported,
                false,
                "Restore the previous spatial region.",
            ),
            definition(
                "grid-reset",
                &[],
                vec![Mode::Grid],
                CountPolicy::NotSupported,
                false,
                "Restore the root spatial region without dispatching input.",
            ),
            definition(
                "grid-help",
                &[],
                vec![Mode::Grid],
                CountPolicy::NotSupported,
                false,
                "Toggle concise Grid mode help.",
            ),
            definition(
                "grid-cancel",
                &[],
                vec![Mode::Grid],
                CountPolicy::NotSupported,
                false,
                "Cancel Grid mode without dispatching input.",
            ),
            definition(
                "download",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Download an explicit URL through the browser download manager.",
            ),
            definition(
                "print-pdf",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Export the current page to a collision-safe PDF path.",
            ),
            definition(
                "print",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Print the current page through the configured desktop print path.",
            ),
            definition(
                "save-page",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Save the current page to a collision-safe local path.",
            ),
            definition(
                "view-source",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open the current safe HTTP(S) document in the engine source viewer.",
            ),
            definition(
                "devtools",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Toggle the local QtWebEngine developer tools for the current tab.",
            ),
            definition(
                "downloads",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open the browser download manager.",
            ),
            definition(
                "permissions",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Inspect exact-origin permission rules.",
            ),
            definition(
                "blocking-status",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Show active network-blocking lists and blocked-request totals.",
            ),
            definition(
                "site-status",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open a privacy-safe ledger for the active tab and document.",
            ),
            definition(
                "site-doctor",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                true,
                "Run one registered, bounded diagnostic experiment for the active site.",
            ),
            definition(
                "site-doctor-undo",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                true,
                "Undo the active Site Doctor experiment by ID.",
            ),
            definition(
                "site-data-clear",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                true,
                "Preview or clear only the supported data categories for one exact site origin.",
            ),
            definition(
                "blocklist-update",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Fetch configured blocklists over HTTPS and retain the last good snapshot on failure.",
            ),
            definition(
                "link-cleaning-update",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Fetch the explicitly configured clean-link rule manifest and retain the last accepted snapshot on failure.",
            ),
            definition(
                "blocking-toggle",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Toggle network blocking globally or for the current site only.",
            ),
            definition(
                "permission-reset",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Revoke one exact-origin permission rule.",
            ),
            definition(
                "download-open",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open a completed download with the desktop handler.",
            ),
            definition(
                "download-show",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Reveal a completed download in its containing directory.",
            ),
            definition(
                "download-cancel",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Cancel an active download.",
            ),
            definition(
                "download-pause",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Pause an in-progress download when supported by the engine.",
            ),
            definition(
                "download-resume",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Resume a paused download when supported by the engine.",
            ),
            definition(
                "download-retry",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Retry an interrupted or cancelled download.",
            ),
            definition(
                "mode-enter",
                &[],
                vec![Mode::Normal, Mode::Command, Mode::Insert, Mode::Caret],
                CountPolicy::NotSupported,
                false,
                "Enter an explicit browser input mode.",
            ),
            definition(
                "caret-move",
                &[],
                vec![Mode::Normal, Mode::Command, Mode::Caret],
                CountPolicy::Supported { maximum: 9_999 },
                false,
                "Move the document caret by a Unicode-aware browser granularity.",
            ),
            definition(
                "caret-select",
                &[],
                vec![Mode::Normal, Mode::Command, Mode::Caret],
                CountPolicy::NotSupported,
                false,
                "Toggle or set document caret selection extension.",
            ),
            definition(
                "yank",
                &[],
                vec![Mode::Normal, Mode::Command, Mode::Caret],
                CountPolicy::NotSupported,
                true,
                "Copy a URL or other supported browser value to the clipboard.",
            ),
            definition(
                "caret-yank",
                &[],
                vec![Mode::Normal, Mode::Command, Mode::Caret],
                CountPolicy::NotSupported,
                true,
                "Copy the current visible document selection to the clipboard.",
            ),
            definition(
                "edit-text",
                &[],
                vec![Mode::Normal, Mode::Command, Mode::Insert, Mode::Caret],
                CountPolicy::NotSupported,
                true,
                "Edit the focused plain-text control with a configured external editor.",
            ),
            definition(
                "spawn",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                true,
                "Run an explicitly selected executable with resolved browser values.",
            ),
            definition(
                "script-run",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                true,
                "Run one explicitly installed userscript with the current context.",
            ),
            definition(
                "jseval",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                true,
                "Run a bounded explicitly requested JavaScript expression in the current tab.",
            ),
            definition(
                "send",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                true,
                "Send a validated browser value to a configured external target.",
            ),
            definition(
                "repeat",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::Supported { maximum: 100 },
                false,
                "Repeat the last eligible browser command.",
            ),
            definition(
                "cancel",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Cancel pending browser work.",
            ),
            definition(
                "macro-record",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Record eligible commands into a named macro register.",
            ),
            definition(
                "macro-stop",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Stop recording the current macro.",
            ),
            definition(
                "macro-play",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::Supported { maximum: 8 },
                false,
                "Replay a named macro register.",
            ),
            definition(
                "action",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Invoke a stable typed action on an explicit subject.",
            ),
            definition(
                "action-list",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "List available stable typed actions.",
            ),
            definition(
                "switcher",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open the universal switcher over typed browser targets.",
            ),
            definition(
                "command-help",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Show help for a registered command by stable ID.",
            ),
            definition(
                "binding-list",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "List generated key bindings, commands, and conflicts.",
            ),
            definition(
                "binding-explain",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Explain how a keychain resolves in the current binding map.",
            ),
            definition(
                "learning-mode",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Show or toggle per-window binding learning feedback.",
            ),
            definition(
                "command-execute",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Execute a registered command by stable ID with typed arguments.",
            ),
            definition(
                "url-clean",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Preview a conservative cleaned URL without navigating.",
            ),
            definition(
                "url-explain",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Explain retained and removed URL components without navigating.",
            ),
            definition(
                "tab-undo",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Reopen the most recent eligible closed tab.",
            ),
            definition(
                "tab-clone",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open a safe URL descriptor from the current tab in a new tab.",
            ),
            definition(
                "reopen-in-window",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open the current tab's safe URL descriptor in a same-profile window; live state is not preserved.",
            ),
            definition(
                "tab-detach",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Move the live current tab into a same-profile window when supported by the engine.",
            ),
            definition(
                "tab-give",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Move the live current tab to a same-profile target window when supported by the engine.",
            ),
            definition(
                "bookmark-add",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Save the current safe URL as a bookmark.",
            ),
            definition(
                "bookmark-delete",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Delete a bookmark by ID.",
            ),
            definition(
                "bookmark-edit",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Update a bookmark title by ID.",
            ),
            definition(
                "bookmark-open",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open a bookmark by ID.",
            ),
            definition(
                "bookmark-list",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "List bookmarks for the current profile.",
            ),
            definition(
                "quickmark-add",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Save a named URL shortcut.",
            ),
            definition(
                "quickmark-delete",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Delete a named URL shortcut.",
            ),
            definition(
                "quickmark-edit",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Update a named URL shortcut destination.",
            ),
            definition(
                "quickmark-open",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open a named URL shortcut.",
            ),
            definition(
                "quickmark-list",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "List named URL shortcuts for the current profile.",
            ),
            definition(
                "history",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open profile-local browsing history.",
            ),
            definition(
                "history-open",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open a history entry by ID.",
            ),
            definition(
                "journey",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open the profile-local navigation journey.",
            ),
            definition(
                "journey-reopen",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Reopen a safe journey node with a new GET navigation.",
            ),
            definition(
                "history-clear",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Clear profile-local history after confirmation.",
            ),
            definition(
                "tab-open",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open a new tab and navigate to an address or search query.",
            ),
            definition(
                "tab-next",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::Supported { maximum: 100 },
                false,
                "Select the next tab.",
            ),
            definition(
                "tab-prev",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::Supported { maximum: 100 },
                false,
                "Select the previous tab.",
            ),
            definition(
                "tab-select",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Select a live tab by displayed index, stable ID, last position, or previous focus.",
            ),
            definition(
                "tab-focus",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Focus a specific live tab by stable ID.",
            ),
            definition(
                "tab-close",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::Supported { maximum: 100 },
                false,
                "Close the active tab or a bounded number of unpinned tabs.",
            ),
            definition(
                "tab-suspend",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Freeze an engine-approved hidden tab until it is resumed.",
            ),
            definition(
                "tab-discard",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Discard an engine-approved hidden tab until it is resumed.",
            ),
            definition(
                "tab-resume",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Resume a frozen tab and restore its engine lifecycle.",
            ),
            definition(
                "tab-move",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Move a specific live tab left or right.",
            ),
            definition(
                "tab-pin",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Set or toggle the pinned state of a specific live tab.",
            ),
            definition(
                "tab-mute",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Set or toggle the muted state of a specific live tab.",
            ),
            definition(
                "selection-search",
                &[],
                vec![Mode::Normal, Mode::Command, Mode::Caret],
                CountPolicy::NotSupported,
                true,
                "Search the current visible document selection.",
            ),
            definition(
                "window-focus",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Focus a specific live browser window by stable ID.",
            ),
            definition(
                "window-move",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Move a specific live browser window to a compositor workspace.",
            ),
            definition(
                "session-save",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Save the current profile's safe session snapshot.",
            ),
            definition(
                "session-load",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Load a validated named session snapshot.",
            ),
            definition(
                "session-list",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "List named sessions in the current profile.",
            ),
            definition(
                "session-delete",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Delete a named session after confirmation.",
            ),
            definition(
                "profile-list",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "List durable profiles.",
            ),
            definition(
                "profile-open",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Focus or create a window for a named profile.",
            ),
            definition(
                "profile-create",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Create a durable normal profile.",
            ),
            definition(
                "profile-delete",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Delete a durable profile after confirmation.",
            ),
            definition(
                "context-list",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "List durable browsing contexts.",
            ),
            definition(
                "context-create",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Create a durable browsing context.",
            ),
            definition(
                "context-delete",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Delete a durable browsing context after confirmation.",
            ),
            definition(
                "context-enter",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Enter a durable browsing context.",
            ),
            definition(
                "context-save",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Save the current window membership to a browsing context.",
            ),
            definition(
                "context-route",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Add, remove, or list explicit context routing rules.",
            ),
            definition(
                "help",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Show generated browser command and setting help.",
            ),
            definition(
                "version",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Show the Ferric Browser application and protocol version.",
            ),
            definition(
                "diagnostics",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "Open the privacy-safe runtime diagnostics report.",
            ),
        ]);
        for name in ["set", "unset"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.arguments = vec![
                    ArgumentDefinition {
                        name: if name == "set" { "key=value" } else { "key" }.into(),
                        kind: ArgumentKind::Text,
                        required: true,
                    },
                    ArgumentDefinition {
                        name: "pattern".into(),
                        kind: ArgumentKind::Text,
                        required: false,
                    },
                    ArgumentDefinition {
                        name: "temporary".into(),
                        kind: ArgumentKind::Boolean,
                        required: false,
                    },
                ];
                command.examples = vec![
                    if name == "set" {
                        "set content.zoom=1.25".into()
                    } else {
                        "unset content.zoom".into()
                    },
                    if name == "set" {
                        "set --pattern https://docs.example/* input.entry_mode=insert".into()
                    } else {
                        "unset --pattern https://docs.example/* input.entry_mode".into()
                    },
                ];
                command.completion = CompletionProvider::Settings;
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "get")
        {
            command.effect = EffectClass::Query;
            command.arguments = vec![
                ArgumentDefinition {
                    name: "key".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "url".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "explain".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
            ];
            command.examples = vec![
                "get content.zoom".into(),
                "get content.zoom --url https://example.test/docs --explain".into(),
            ];
            command.completion = CompletionProvider::Settings;
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "help")
        {
            command.effect = EffectClass::Query;
            command.arguments = vec![ArgumentDefinition {
                name: "topic".into(),
                kind: ArgumentKind::Text,
                required: false,
            }];
            command.examples = vec!["help".into(), "help content.zoom".into()];
        }
        for name in ["version", "diagnostics"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.effect = EffectClass::Query;
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "config-export")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "path".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            command.examples = vec!["config-export ~/reviewed-config.toml".into()];
        }
        for name in [
            "config-edit",
            "config-reload",
            "config-check",
            "theme-reload",
        ] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.examples = vec![name.into()];
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "config-write-defaults")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "path".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            command.examples = vec!["config-write-defaults ~/config-template.toml".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "binding-list")
        {
            command.effect = EffectClass::Query;
            command.arguments = vec![ArgumentDefinition {
                name: "mode".into(),
                kind: ArgumentKind::Enum,
                required: false,
            }];
            command.examples = vec!["binding-list".into(), "binding-list --mode normal".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "binding-explain")
        {
            command.effect = EffectClass::Query;
            command.arguments = vec![
                ArgumentDefinition {
                    name: "keychain".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "mode".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
            ];
            command.examples = vec![
                "binding-explain g".into(),
                "binding-explain gg --mode normal".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "learning-mode")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "state".into(),
                kind: ArgumentKind::Enum,
                required: false,
            }];
            command.examples = vec![
                "learning-mode".into(),
                "learning-mode on".into(),
                "learning-mode toggle".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "bind")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "keychain".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "command".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
            ];
            command.examples = vec!["bind g,g tab-next".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "unbind")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "keychain".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            command.examples = vec!["unbind g,g".into()];
        }
        if let Some(open) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "open")
        {
            open.arguments = vec![
                ArgumentDefinition {
                    name: "input".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "target".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
                ArgumentDefinition {
                    name: "profile".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "context".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "ephemeral".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
                ArgumentDefinition {
                    name: "clean_link".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
            ];
            open.completion = CompletionProvider::Urls;
            open.examples = vec![
                "open https://example.test".into(),
                "open --target tab https://example.test".into(),
                "open --profile work --context project https://example.test".into(),
                "open --clean-link https://example.test/?utm_source=demo".into(),
            ];
        }
        if let Some(open_current) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "open-current")
        {
            open_current.arguments = vec![ArgumentDefinition {
                name: "target".into(),
                kind: ArgumentKind::Enum,
                required: false,
            }];
            open_current.examples = vec!["open-current".into(), "open-current --target tab".into()];
        }
        if let Some(tab_open) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "tab-open")
        {
            tab_open.arguments = vec![
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
            ];
            tab_open.completion = CompletionProvider::Urls;
            tab_open.examples = vec![
                "tab-open https://example.test".into(),
                "tab-open --background https://example.test".into(),
            ];
        }
        if let Some(paste_open) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "paste-open")
        {
            paste_open.effect = EffectClass::Sensitive;
            paste_open.arguments = vec![
                ArgumentDefinition {
                    name: "target".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
                ArgumentDefinition {
                    name: "primary".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
            ];
            paste_open.examples = vec![
                "paste-open".into(),
                "paste-open --target tab".into(),
                "paste-open --primary".into(),
            ];
        }
        if let Some(download) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "download")
        {
            download.effect = EffectClass::Mutating;
            download.arguments = vec![ArgumentDefinition {
                name: "input".into(),
                kind: ArgumentKind::Url,
                required: true,
            }];
            download.completion = CompletionProvider::Urls;
            download.examples = vec!["download https://example.test/file.zip".into()];
        }
        if let Some(print_pdf) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "print-pdf")
        {
            print_pdf.effect = EffectClass::Mutating;
            print_pdf.arguments = vec![ArgumentDefinition {
                name: "path".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            print_pdf.examples = vec!["print-pdf /tmp/page.pdf".into()];
        }
        if let Some(save_page) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "save-page")
        {
            save_page.effect = EffectClass::Mutating;
            save_page.arguments = vec![ArgumentDefinition {
                name: "path".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            save_page.examples = vec!["save-page /tmp/page.html".into()];
        }
        if let Some(print) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "print")
        {
            print.examples = vec!["print".into()];
        }
        if let Some(view_source) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "view-source")
        {
            view_source.effect = EffectClass::Navigation;
            view_source.examples = vec!["view-source".into()];
        }
        if let Some(devtools) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "devtools")
        {
            devtools.effect = EffectClass::Mutating;
            devtools.arguments = vec![ArgumentDefinition {
                name: "detach".into(),
                kind: ArgumentKind::Boolean,
                required: false,
            }];
            devtools.examples = vec!["devtools".into(), "devtools --detach".into()];
        }
        if let Some(downloads) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "downloads")
        {
            downloads.effect = EffectClass::Query;
            downloads.examples = vec!["downloads".into()];
        }
        if let Some(permissions) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "permissions")
        {
            permissions.effect = EffectClass::Query;
            permissions.arguments = vec![ArgumentDefinition {
                name: "origin".into(),
                kind: ArgumentKind::Text,
                required: false,
            }];
            permissions.examples = vec![
                "permissions".into(),
                "permissions https://example.test".into(),
            ];
        }
        if let Some(site_status) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "site-status")
        {
            site_status.effect = EffectClass::Query;
            site_status.arguments = vec![ArgumentDefinition {
                name: "tab".into(),
                kind: ArgumentKind::Text,
                required: false,
            }];
            site_status.examples = vec!["site-status".into(), "site-status --tab TAB_ID".into()];
        }
        if let Some(site_doctor) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "site-doctor")
        {
            site_doctor.arguments = vec![ArgumentDefinition {
                name: "experiment".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            site_doctor.examples = vec!["site-doctor EXPERIMENT".into()];
        }
        if let Some(site_doctor_undo) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "site-doctor-undo")
        {
            site_doctor_undo.arguments = vec![ArgumentDefinition {
                name: "id".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            site_doctor_undo.examples = vec!["site-doctor-undo EXPERIMENT_ID".into()];
        }
        if let Some(site_data_clear) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "site-data-clear")
        {
            site_data_clear.arguments = vec![
                ArgumentDefinition {
                    name: "origin".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "confirmed".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
            ];
            site_data_clear.examples = vec![
                "site-data-clear https://example.test".into(),
                "site-data-clear https://example.test --confirm".into(),
            ];
        }
        if let Some(blocking_toggle) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "blocking-toggle")
        {
            blocking_toggle.arguments = vec![ArgumentDefinition {
                name: "site".into(),
                kind: ArgumentKind::Boolean,
                required: false,
            }];
            blocking_toggle.examples =
                vec!["blocking-toggle".into(), "blocking-toggle --site".into()];
        }
        if let Some(blocking_status) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "blocking-status")
        {
            blocking_status.effect = EffectClass::Query;
        }
        if let Some(send) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "send")
        {
            send.arguments = vec![
                ArgumentDefinition {
                    name: "target".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "input".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "url".into(),
                    kind: ArgumentKind::Url,
                    required: false,
                },
                ArgumentDefinition {
                    name: "selection".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
                ArgumentDefinition {
                    name: "send_subject".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
            ];
            send.examples = vec!["send mpv".into(), "send mpv --selection".into()];
        }
        if let Some(action) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "action")
        {
            action.arguments = vec![
                ArgumentDefinition {
                    name: "subject".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "verb".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "input".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "url".into(),
                    kind: ArgumentKind::Url,
                    required: false,
                },
            ];
            action.examples = vec!["action link open".into(), "action selection search".into()];
        }
        if let Some(action_list) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "action-list")
        {
            action_list.effect = EffectClass::Query;
            action_list.arguments = vec![ArgumentDefinition {
                name: "subject".into(),
                kind: ArgumentKind::Text,
                required: false,
            }];
            action_list.examples = vec!["action-list".into(), "action-list link".into()];
        }
        if let Some(command_help) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "command-help")
        {
            command_help.effect = EffectClass::Query;
            command_help.arguments = vec![ArgumentDefinition {
                name: "id".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            command_help.examples = vec!["command-help open".into()];
        }
        if let Some(command_execute) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "command-execute")
        {
            command_execute.arguments = vec![
                ArgumentDefinition {
                    name: "id".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "arguments".into(),
                    kind: ArgumentKind::Object,
                    required: false,
                },
            ];
            command_execute.examples = vec!["command-execute open".into()];
        }
        if let Some(permission_reset) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "permission-reset")
        {
            permission_reset.arguments = vec![
                ArgumentDefinition {
                    name: "origin".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "permission".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
            ];
            permission_reset.examples =
                vec!["permission-reset https://example.test notifications".into()];
        }
        for name in [
            "download-open",
            "download-show",
            "download-cancel",
            "download-pause",
            "download-resume",
            "download-retry",
        ] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.arguments = vec![ArgumentDefinition {
                    name: "id".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                }];
                command.examples = vec![format!("{name} DOWNLOAD_ID")];
            }
        }
        for name in ["back", "forward", "reload", "stop", "tab-next", "tab-prev"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.effect = EffectClass::Navigation;
                command.examples = vec![name.into()];
            }
        }
        for name in ["back", "forward"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.examples = vec![name.into(), format!("{name} --count 3")];
            }
        }
        for name in ["tab-next", "tab-prev"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.arguments = vec![ArgumentDefinition {
                    name: "count".into(),
                    kind: ArgumentKind::Integer,
                    required: false,
                }];
                command.examples = vec![name.into(), format!("{name} --count 3")];
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "window-close")
        {
            command.scope = CommandScope::Window;
            command.examples = vec!["window-close".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "window-new")
        {
            command.scope = CommandScope::Window;
            command.arguments = vec![
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
            ];
            command.examples = vec![
                "window-new".into(),
                "window-new --profile work".into(),
                "window-new --private".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "fullscreen")
        {
            command.scope = CommandScope::Window;
            command.arguments = vec![ArgumentDefinition {
                name: "state".into(),
                kind: ArgumentKind::Enum,
                required: false,
            }];
            command.examples = vec![
                "fullscreen toggle".into(),
                "fullscreen on".into(),
                "fullscreen off".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "search")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "query".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "backward".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
                ArgumentDefinition {
                    name: "case".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
            ];
            command.examples = vec![
                "search example".into(),
                "search --backward example".into(),
                "search --case sensitive example".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "reload")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "bypass_cache".into(),
                kind: ArgumentKind::Boolean,
                required: false,
            }];
            command.examples = vec!["reload".into(), "reload --bypass-cache".into()];
        }
        for name in ["back", "forward"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.arguments = vec![ArgumentDefinition {
                    name: "count".into(),
                    kind: ArgumentKind::Integer,
                    required: false,
                }];
                command.examples = vec![name.into(), format!("{name} --count 3")];
            }
        }
        for name in ["tab-focus", "tab-suspend", "tab-discard", "tab-resume"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.arguments = vec![ArgumentDefinition {
                    name: "id".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                }];
                command.examples = vec![format!("{name} TAB_ID")];
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "tab-close")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "id".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "count".into(),
                    kind: ArgumentKind::Integer,
                    required: false,
                },
            ];
            command.examples = vec![
                "tab-close".into(),
                "tab-close --id TAB_ID".into(),
                "tab-close --count 3".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "tab-select")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "selector".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            command.effect = EffectClass::Navigation;
            command.examples = vec![
                "tab-select 2".into(),
                "tab-select last".into(),
                "tab-select previous".into(),
                "tab-select TAB_ID".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "tab-mute")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "id".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "state".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
            ];
            command.examples = vec![
                "tab-mute".into(),
                "tab-mute toggle".into(),
                "tab-mute TAB_ID".into(),
                "tab-mute TAB_ID on".into(),
                "tab-mute TAB_ID off".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "tab-move")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "id".into(),
                    kind: ArgumentKind::Text,
                    required: false,
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
            ];
            command.examples = vec![
                "tab-move TAB_ID left".into(),
                "tab-move --context work".into(),
                "tab-move 2 --context work".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "tab-pin")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "id".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "state".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
            ];
            command.examples = vec![
                "tab-pin".into(),
                "tab-pin toggle".into(),
                "tab-pin TAB_ID".into(),
                "tab-pin TAB_ID on".into(),
                "tab-pin TAB_ID off".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "selection-search")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "engine".into(),
                kind: ArgumentKind::Text,
                required: false,
            }];
            command.examples = vec!["selection-search --engine ddg".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "window-focus")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "id".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            command.examples = vec!["window-focus WINDOW_ID".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "window-move")
        {
            command.arguments = vec![
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
            ];
            command.examples = vec!["window-move WINDOW_ID WORKSPACE".into()];
        }
        for name in ["url-clean", "url-explain"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.effect = EffectClass::Query;
                command.arguments = vec![ArgumentDefinition {
                    name: "url".into(),
                    kind: ArgumentKind::Url,
                    required: false,
                }];
                command.examples = vec![format!("{name} https://example.test/?utm_source=demo")];
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "yank")
        {
            command.effect = EffectClass::Sensitive;
            command.arguments = vec![
                ArgumentDefinition {
                    name: "source".into(),
                    kind: ArgumentKind::Enum,
                    required: true,
                },
                ArgumentDefinition {
                    name: "clean".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
                ArgumentDefinition {
                    name: "primary".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
            ];
            command.examples = vec!["yank url".into(), "yank url --clean".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "caret-yank")
        {
            command.effect = EffectClass::Sensitive;
            command.examples = vec!["caret-yank".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "edit-text")
        {
            command.examples = vec!["edit-text".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "hint")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "kind".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
                ArgumentDefinition {
                    name: "target".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
                ArgumentDefinition {
                    name: "rapid".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
                ArgumentDefinition {
                    name: "script".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "first".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
                ArgumentDefinition {
                    name: "index".into(),
                    kind: ArgumentKind::Integer,
                    required: false,
                },
            ];
            command.examples = vec![
                "hint links".into(),
                "hint --rapid --target tab-bg links".into(),
                "hint --target choose all".into(),
                "hint --first --index 3 inputs".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "spawn")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "argv".into(),
                    kind: ArgumentKind::Object,
                    required: false,
                },
                ArgumentDefinition {
                    name: "userscript".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
            ];
            command.examples = vec![
                "spawn -- mpv {url}".into(),
                "spawn --userscript video".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "script-run")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "name".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            command.examples = vec!["script-run video".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "jseval")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "world".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
                ArgumentDefinition {
                    name: "script".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
            ];
            command.examples = vec![
                "jseval \"document.body.dataset.rb = '1'\"".into(),
                "jseval --world page \"document.title\"".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "mode-enter")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "mode".into(),
                kind: ArgumentKind::Enum,
                required: true,
            }];
            command.examples = vec!["mode-enter caret".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "caret-move")
        {
            command.arguments = vec![
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
            ];
            command.examples = vec!["caret-move word-next".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "caret-select")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "state".into(),
                kind: ArgumentKind::Enum,
                required: false,
            }];
            command.examples = vec!["caret-select toggle".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "tab-undo")
        {
            command.examples = vec!["tab-undo".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "zoom")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "factor".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            command.examples = vec![
                "zoom in".into(),
                "zoom out".into(),
                "zoom reset".into(),
                "zoom 1.25".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "search-next")
        {
            command.arguments = vec![
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
            ];
            command.effect = EffectClass::Navigation;
            command.examples = vec![
                "search-next".into(),
                "search-next --backward".into(),
                "search-next --count 3".into(),
            ];
        }
        for name in ["scroll", "scroll-page", "scroll-to"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.effect = EffectClass::Navigation;
                let argument_name = if name == "scroll-to" {
                    "edge"
                } else {
                    "direction"
                };
                command.arguments = match name {
                    "scroll" => vec![
                        ArgumentDefinition {
                            name: argument_name.into(),
                            kind: ArgumentKind::Enum,
                            required: true,
                        },
                        ArgumentDefinition {
                            name: "count".into(),
                            kind: ArgumentKind::Integer,
                            required: false,
                        },
                    ],
                    "scroll-page" => vec![
                        ArgumentDefinition {
                            name: argument_name.into(),
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
                    _ => vec![ArgumentDefinition {
                        name: argument_name.into(),
                        kind: ArgumentKind::Enum,
                        required: true,
                    }],
                };
                command.examples = match name {
                    "scroll" => vec![
                        "scroll down".into(),
                        "scroll up --count 3".into(),
                        "scroll left".into(),
                        "scroll right".into(),
                    ],
                    "scroll-page" => {
                        vec!["scroll-page down".into(), "scroll-page up --half".into()]
                    }
                    _ => vec!["scroll-to top".into(), "scroll-to bottom".into()],
                };
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "scroll-target")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "action".into(),
                kind: ArgumentKind::Enum,
                required: true,
            }];
            command.examples = vec![
                "scroll-target select".into(),
                "scroll-target auto".into(),
                "scroll-target document".into(),
                "scroll-target status".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "tab-clone")
        {
            command.examples = vec!["tab-clone".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "reopen-in-window")
        {
            command.examples = vec!["reopen-in-window".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "tab-detach")
        {
            command.examples = vec!["tab-detach".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "tab-give")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "window_id".into(),
                kind: ArgumentKind::Text,
                required: true,
            }];
            command.examples = vec!["tab-give WINDOW_ID".into()];
        }
        for name in [
            "bookmark-list",
            "quickmark-list",
            "history",
            "journey",
            "switcher",
        ] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.effect = EffectClass::Query;
                command.examples = vec![name.into()];
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "switcher")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "scope".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
                ArgumentDefinition {
                    name: "query".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
            ];
            command.examples = vec!["switcher".into(), "switcher --scope tabs example".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "journey")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "current".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
                ArgumentDefinition {
                    name: "search".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "expand".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
            ];
            command.examples = vec![
                "journey".into(),
                "journey --current".into(),
                "journey --search example".into(),
                "journey --expand 123e4567-e89b-12d3-a456-426614174000".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "journey-reopen")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "node".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "target".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
            ];
            command.examples = vec!["journey-reopen 123e4567-e89b-12d3-a456-426614174000".into()];
        }
        for name in [
            "bookmark-delete",
            "bookmark-open",
            "bookmark-edit",
            "quickmark-delete",
            "quickmark-open",
            "quickmark-edit",
            "history-open",
        ] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.arguments = vec![ArgumentDefinition {
                    name: if name == "bookmark-delete"
                        || name == "bookmark-open"
                        || name == "bookmark-edit"
                        || name == "history-open"
                    {
                        "id"
                    } else {
                        "name"
                    }
                    .into(),
                    kind: ArgumentKind::Text,
                    required: true,
                }];
                command.examples = vec![format!("{name} value")];
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "bookmark-edit")
        {
            command.arguments.push(ArgumentDefinition {
                name: "title".into(),
                kind: ArgumentKind::Text,
                required: true,
            });
            command.examples = vec!["bookmark-edit BOOKMARK_ID --title \"New title\"".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "quickmark-edit")
        {
            command.arguments.push(ArgumentDefinition {
                name: "url".into(),
                kind: ArgumentKind::Url,
                required: true,
            });
            command.examples = vec!["quickmark-edit work https://work.example".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "bookmark-add")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "title".into(),
                kind: ArgumentKind::Text,
                required: false,
            }];
            command.examples = vec!["bookmark-add".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "quickmark-add")
        {
            command.arguments = vec![
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
            ];
            command.examples = vec!["quickmark-add work https://work.example".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "history-clear")
        {
            command.arguments = vec![
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
            ];
            command.examples = vec![
                "history-clear --origin https://example.test --since 1757894400 --confirm".into(),
            ];
        }
        for name in ["session-save", "session-load", "session-delete"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.arguments = vec![ArgumentDefinition {
                    name: "name".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                }];
                command.examples = if name == "profile-create" {
                    vec![
                        "profile-create work".into(),
                        "profile-create task --ephemeral".into(),
                    ]
                } else {
                    vec![format!("{name} work")]
                };
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "session-list")
        {
            command.effect = EffectClass::Query;
            command.examples = vec!["session-list".into()];
        }
        for name in ["profile-create", "profile-delete"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.arguments = vec![ArgumentDefinition {
                    name: "name".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                }];
                if name == "profile-create" {
                    command.arguments.push(ArgumentDefinition {
                        name: "ephemeral".into(),
                        kind: ArgumentKind::Boolean,
                        required: false,
                    });
                }
                command.examples = vec![format!("{name} work")];
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "profile-list")
        {
            command.effect = EffectClass::Query;
            command.examples = vec!["profile-list".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "profile-open")
        {
            command.arguments = vec![
                ArgumentDefinition {
                    name: "name".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "input".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
            ];
            command.examples = vec![
                "profile-open work".into(),
                "profile-open work https://example.test".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "context-list")
        {
            command.scope = CommandScope::Context;
            command.effect = EffectClass::Query;
            command.examples = vec!["context-list".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "context-create")
        {
            command.scope = CommandScope::Context;
            command.arguments = vec![
                ArgumentDefinition {
                    name: "name".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "label".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "profile".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "workspace".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
            ];
            command.examples = vec!["context-create work --profile work".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "context-delete")
        {
            command.scope = CommandScope::Context;
            command.arguments = vec![
                ArgumentDefinition {
                    name: "name".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "confirmed".into(),
                    kind: ArgumentKind::Boolean,
                    required: false,
                },
            ];
            command.examples = vec!["context-delete work --confirm".into()];
        }
        for name in ["context-enter", "context-save"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.scope = CommandScope::Context;
                command.arguments = vec![ArgumentDefinition {
                    name: "name".into(),
                    kind: ArgumentKind::Text,
                    required: name == "context-enter",
                }];
                command.examples = vec![format!("{name} work")];
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "context-route")
        {
            command.scope = CommandScope::Context;
            command.arguments = vec![
                ArgumentDefinition {
                    name: "action".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                },
                ArgumentDefinition {
                    name: "pattern".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "context".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "id".into(),
                    kind: ArgumentKind::Text,
                    required: false,
                },
                ArgumentDefinition {
                    name: "priority".into(),
                    kind: ArgumentKind::Integer,
                    required: false,
                },
                ArgumentDefinition {
                    name: "behavior".into(),
                    kind: ArgumentKind::Enum,
                    required: false,
                },
                ArgumentDefinition {
                    name: "entry_points".into(),
                    kind: ArgumentKind::Object,
                    required: false,
                },
            ];
            command.examples = vec![
                "context-route list".into(),
                "context-route add https://*.company.test/* work --priority 10 --behavior prompt --entry-point explicit-open".into(),
                "context-route remove company-work".into(),
            ];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "repeat")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "count".into(),
                kind: ArgumentKind::Integer,
                required: false,
            }];
            command.examples = vec!["repeat".into(), "repeat --count 3".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "cancel")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "operation_id".into(),
                kind: ArgumentKind::Text,
                required: false,
            }];
            command.examples = vec!["cancel".into(), "cancel op-123".into()];
        }
        for name in ["macro-record", "macro-play"] {
            if let Some(command) = registry
                .definitions
                .iter_mut()
                .find(|definition| definition.name == name)
            {
                command.arguments = vec![ArgumentDefinition {
                    name: "register".into(),
                    kind: ArgumentKind::Text,
                    required: true,
                }];
                command.examples = vec![format!("{name} a")];
            }
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "macro-stop")
        {
            command.examples = vec!["macro-stop".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "grid-refine")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "cell".into(),
                kind: ArgumentKind::Enum,
                required: true,
            }];
            command.examples = vec!["grid-refine 5".into()];
        }
        if let Some(command) = registry
            .definitions
            .iter_mut()
            .find(|definition| definition.name == "grid-click")
        {
            command.arguments = vec![ArgumentDefinition {
                name: "button".into(),
                kind: ArgumentKind::Enum,
                required: true,
            }];
            command.examples = vec![
                "grid-click left".into(),
                "grid-click right".into(),
                "grid-click middle".into(),
            ];
        }
        registry.expansions = [
            ("o", "open"),
            ("t", "tab-open"),
            ("q", "quit"),
            ("open-history", "history"),
        ]
        .into_iter()
        .map(|(name, expansion)| {
            CommandAlias::from_text(name, expansion).expect("built-in command alias is valid")
        })
        .collect();
        registry
    }

    /// Resolves a canonical command name or alias.
    ///
    /// # Errors
    ///
    /// Returns `UnknownCommand` when the registry has no matching definition.
    pub fn resolve(&self, name: &str) -> Result<&CommandDefinition, CommandError> {
        let mut current = name;
        let mut visited = Vec::new();
        loop {
            if let Some(definition) = self.definitions.iter().find(|definition| {
                definition.name == current
                    || definition.aliases.iter().any(|alias| alias == current)
            }) {
                return Ok(definition);
            }
            let Some(alias) = self.expansions.iter().find(|alias| alias.name == current) else {
                return Err(CommandError::UnknownCommand(name.into()));
            };
            if visited.contains(&current) {
                return Err(CommandError::AliasCycle(current.into()));
            }
            if visited.len() >= MAX_ALIAS_DEPTH {
                return Err(CommandError::AliasDepth);
            }
            visited.push(current);
            current = &alias.expansion[0].name;
        }
    }

    /// Expands one parsed command into parsed commands without reparsing it.
    /// Invocation arguments are appended to the final command in the alias
    /// expansion, matching ordinary command-alias behavior.
    ///
    /// # Errors
    ///
    /// Returns an alias cycle/depth error, or an unknown command error when an
    /// expansion points at no registered command.
    pub fn expand_command(
        &self,
        command: ParsedCommand,
    ) -> Result<Vec<ParsedCommand>, CommandError> {
        self.expand_command_inner(command, &mut Vec::new(), 0)
    }

    /// Expands every command in a parsed chain before execution.
    ///
    /// # Errors
    ///
    /// Returns the first alias or registry error encountered.
    pub fn expand_chain(
        &self,
        commands: Vec<ParsedCommand>,
    ) -> Result<Vec<ParsedCommand>, CommandError> {
        let mut expanded = Vec::new();
        for command in commands {
            expanded.extend(self.expand_command(command)?);
            if expanded.len() > MAX_COMMANDS {
                return Err(CommandError::TooManyCommands);
            }
        }
        Ok(expanded)
    }

    fn expand_command_inner(
        &self,
        command: ParsedCommand,
        stack: &mut Vec<String>,
        depth: usize,
    ) -> Result<Vec<ParsedCommand>, CommandError> {
        let Some(alias) = self
            .expansions
            .iter()
            .find(|alias| alias.name == command.name)
        else {
            self.resolve(&command.name)?;
            return Ok(vec![command]);
        };
        if stack.iter().any(|name| name == &alias.name) {
            return Err(CommandError::AliasCycle(alias.name.clone()));
        }
        if depth >= MAX_ALIAS_DEPTH {
            return Err(CommandError::AliasDepth);
        }
        stack.push(alias.name.clone());
        let mut commands = alias.expansion.clone();
        if let Some(last) = commands.last_mut() {
            last.arguments.extend(command.arguments);
            if last.arguments.len() > MAX_ARGUMENTS {
                return Err(CommandError::TooManyArguments);
            }
        }
        let mut expanded = Vec::new();
        for command in commands {
            expanded.extend(self.expand_command_inner(command, stack, depth + 1)?);
        }
        stack.pop();
        Ok(expanded)
    }

    /// Validates that a parsed command is available in the requested mode.
    ///
    /// # Errors
    ///
    /// Returns a typed registry error for an unknown command or unavailable
    /// mode.
    pub fn validate(&self, command: &ParsedCommand, mode: Mode) -> Result<(), CommandError> {
        for command in self.expand_command(command.clone())? {
            let definition = self.resolve(&command.name)?;
            if !definition.modes.contains(&mode) {
                return Err(CommandError::UnknownCommand(command.name));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn definitions(&self) -> &[CommandDefinition] {
        &self.definitions
    }

    #[must_use]
    pub fn alias_definitions(&self) -> &[CommandAlias] {
        &self.expansions
    }
}

fn definition(
    name: &str,
    aliases: &[&str],
    modes: Vec<Mode>,
    count: CountPolicy,
    sensitive: bool,
    description: &str,
) -> CommandDefinition {
    CommandDefinition {
        action: ActionId::from_raw(0),
        name: name.into(),
        aliases: aliases.iter().map(|alias| (*alias).into()).collect(),
        modes,
        count,
        sensitive,
        description: description.into(),
        arguments: Vec::new(),
        scope: CommandScope::Tab,
        effect: if sensitive {
            EffectClass::Sensitive
        } else if matches!(name, "open" | "open-current") {
            EffectClass::Navigation
        } else {
            EffectClass::Mutating
        },
        completion: CompletionProvider::None,
        examples: Vec::new(),
    }
}

/// Rejects the one open-option combination whose semantics cannot be preserved
/// by the native clean-link preview surface. Every open entry point should use
/// this shared invariant before it mutates a tab or window.
///
/// # Errors
///
/// Returns an error when clean-link navigation is requested for a background
/// tab, because the preview must remain attached to the foreground target.
pub fn validate_open_target(target: Option<&str>, clean_link: bool) -> Result<(), &'static str> {
    if clean_link && target == Some("tab-bg") {
        Err("open --clean-link cannot be combined with --target tab-bg")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_target_validation_preserves_clean_link_semantics() {
        assert!(validate_open_target(Some("tab"), true).is_ok());
        assert!(validate_open_target(Some("tab-bg"), false).is_ok());
        assert_eq!(
            validate_open_target(Some("tab-bg"), true),
            Err("open --clean-link cannot be combined with --target tab-bg")
        );
    }

    #[test]
    fn parses_quotes_escapes_and_chain_boundaries_without_shell_expansion() {
        let commands = parse_chain(
            r#":open "https://example.test/a;;b?q=$()";;set ui.font "JetBrains Mono""#,
            ParseInput::Interactive,
        )
        .unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].name, "open");
        assert_eq!(commands[0].arguments[0], "https://example.test/a;;b?q=$()");
        assert_eq!(commands[1].arguments, vec!["ui.font", "JetBrains Mono"]);
    }

    #[test]
    fn leading_colon_is_source_specific() {
        assert!(parse_chain(":reload", ParseInput::Interactive).is_ok());
        assert!(parse_chain(":reload", ParseInput::Cli).is_ok());
        assert_eq!(
            parse_chain(":reload", ParseInput::Ipc),
            Err(CommandError::InvalidName(":".into()))
        );
    }

    #[test]
    fn rejects_incomplete_syntax_and_limits() {
        assert_eq!(
            parse_chain("open 'url", ParseInput::Cli),
            Err(CommandError::UnterminatedQuote)
        );
        assert_eq!(
            parse_chain("open url\\", ParseInput::Cli),
            Err(CommandError::TrailingEscape)
        );
        assert_eq!(
            parse_chain("open\0url", ParseInput::Cli),
            Err(CommandError::Nul)
        );
        let chain = std::iter::repeat_n("stop", MAX_COMMANDS + 1)
            .collect::<Vec<_>>()
            .join(";;");
        assert_eq!(
            parse_chain(&chain, ParseInput::Cli),
            Err(CommandError::TooManyCommands)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn registry_is_the_single_resolution_source() {
        let registry = CommandRegistry::default_v1();
        let command = parse_chain("open https://example.test", ParseInput::Cli)
            .unwrap()
            .remove(0);
        assert_eq!(registry.resolve(&command.name).unwrap().name, "open");
        assert!(registry.validate(&command, Mode::Normal).is_ok());
        assert_eq!(registry.definitions().len(), 138);
        let open = registry.resolve("open").unwrap();
        assert_ne!(open.action, ActionId::from_raw(0));
        assert_eq!(open.effect, EffectClass::Navigation);
        assert_eq!(open.scope, CommandScope::Tab);
        let scroll_target = registry.resolve("scroll-target").unwrap();
        assert!(scroll_target.aliases.is_empty());
        assert_eq!(scroll_target.modes, [Mode::Normal, Mode::Command]);
        assert_eq!(scroll_target.count, CountPolicy::NotSupported);
        assert!(!scroll_target.sensitive);
        assert_eq!(
            scroll_target.description,
            "Select, clear, inspect, or pin the active page's keyboard scroll target."
        );
        assert_eq!(scroll_target.scope, CommandScope::Tab);
        assert_eq!(scroll_target.effect, EffectClass::Mutating);
        assert_eq!(scroll_target.completion, CompletionProvider::None);
        assert_eq!(scroll_target.arguments.len(), 1);
        assert_eq!(scroll_target.arguments[0].name, "action");
        assert_eq!(scroll_target.arguments[0].kind, ArgumentKind::Enum);
        assert!(scroll_target.arguments[0].required);
        assert_eq!(
            scroll_target.examples,
            [
                "scroll-target select",
                "scroll-target auto",
                "scroll-target document",
                "scroll-target status",
            ]
        );
        for mode in [Mode::Normal, Mode::Command] {
            assert!(
                registry
                    .validate(
                        &ParsedCommand {
                            name: "scroll-target".into(),
                            arguments: vec!["status".into()],
                        },
                        mode
                    )
                    .is_ok()
            );
        }
        for name in ["tab-suspend", "tab-discard", "tab-resume"] {
            let definition = registry.resolve(name).expect("suspension command");
            assert_eq!(definition.arguments.len(), 1);
            assert_eq!(definition.scope, CommandScope::Tab);
        }
        for name in ["tab-next", "tab-prev"] {
            let definition = registry.resolve(name).expect("tab traversal command");
            assert_eq!(definition.arguments.len(), 1);
            assert_eq!(definition.arguments[0].name, "count");
            assert_eq!(definition.scope, CommandScope::Tab);
        }
        assert_eq!(
            registry
                .resolve("binding-list")
                .unwrap()
                .arguments
                .iter()
                .map(|argument| argument.name.as_str())
                .collect::<Vec<_>>(),
            vec!["mode"]
        );
        for source in ["tab-pin", "tab-pin toggle"] {
            let command = parse_chain(source, ParseInput::Interactive)
                .expect("tab-pin shorthand parses")
                .remove(0);
            assert!(
                registry.validate(&command, Mode::Normal).is_ok(),
                "{source} should target the active tab"
            );
        }
        for source in ["tab-mute", "tab-mute toggle"] {
            let command = parse_chain(source, ParseInput::Interactive)
                .expect("tab-mute shorthand parses")
                .remove(0);
            assert!(
                registry.validate(&command, Mode::Normal).is_ok(),
                "{source} should target the active tab"
            );
        }
        assert_eq!(
            registry
                .resolve("binding-explain")
                .unwrap()
                .arguments
                .iter()
                .map(|argument| argument.name.as_str())
                .collect::<Vec<_>>(),
            vec!["keychain", "mode"]
        );
        assert_eq!(
            registry
                .resolve("get")
                .unwrap()
                .arguments
                .iter()
                .map(|argument| argument.name.as_str())
                .collect::<Vec<_>>(),
            vec!["key", "url", "explain"]
        );
        assert_eq!(
            registry
                .resolve("help")
                .unwrap()
                .arguments
                .iter()
                .map(|argument| argument.name.as_str())
                .collect::<Vec<_>>(),
            vec!["topic"]
        );
        assert_eq!(
            registry
                .resolve("learning-mode")
                .unwrap()
                .arguments
                .iter()
                .map(|argument| argument.name.as_str())
                .collect::<Vec<_>>(),
            vec!["state"]
        );
        assert_eq!(open.arguments[0].kind, ArgumentKind::Text);
        assert_eq!(open.completion, CompletionProvider::Urls);
        assert_eq!(
            registry.resolve("set").unwrap().arguments[0].name,
            "key=value"
        );
        assert_eq!(registry.resolve("unset").unwrap().arguments[0].name, "key");
        assert_eq!(
            registry.resolve("context-route").unwrap().arguments[0].name,
            "action"
        );
        assert_eq!(
            registry.resolve("config-export").unwrap().arguments[0].name,
            "path"
        );
        for name in [
            "config-edit",
            "config-reload",
            "config-check",
            "theme-reload",
        ] {
            assert!(registry.resolve(name).unwrap().arguments.is_empty());
        }
        assert_eq!(
            registry.resolve("config-write-defaults").unwrap().arguments[0].name,
            "path"
        );
        assert_eq!(
            registry
                .resolve("scroll-page")
                .unwrap()
                .arguments
                .iter()
                .map(|argument| argument.name.as_str())
                .collect::<Vec<_>>(),
            vec!["direction", "half", "count"]
        );
        assert_eq!(
            registry.resolve("bind").unwrap().arguments[0].name,
            "keychain"
        );
        assert_eq!(
            registry.resolve("unbind").unwrap().arguments[0].name,
            "keychain"
        );
        assert_eq!(registry.resolve("o").unwrap().name, "open");
        assert_eq!(registry.resolve("t").unwrap().name, "tab-open");
        assert_eq!(registry.resolve("q").unwrap().name, "quit");
        assert_eq!(registry.resolve("open-history").unwrap().name, "history");
        assert_eq!(
            registry
                .expand_command(ParsedCommand {
                    name: "open-history".into(),
                    arguments: Vec::new(),
                })
                .unwrap()[0]
                .name,
            "history"
        );
        let switcher = registry.resolve("switcher").expect("switcher command");
        assert_eq!(switcher.effect, EffectClass::Query);
        assert_eq!(
            switcher
                .arguments
                .iter()
                .map(|argument| argument.name.as_str())
                .collect::<Vec<_>>(),
            vec!["scope", "query"]
        );
    }

    #[test]
    fn aliases_expand_parsed_tokens_and_append_invocation_arguments() {
        let registry = CommandRegistry::new_with_aliases(
            vec![definition(
                "open",
                &[],
                vec![Mode::Normal, Mode::Command],
                CountPolicy::NotSupported,
                false,
                "open",
            )],
            vec![CommandAlias::from_text("go", "open --target tab").unwrap()],
        )
        .unwrap();
        let command = parse_chain(r#"go "https://example.test/a;;b""#, ParseInput::Cli)
            .unwrap()
            .remove(0);
        let expanded = registry.expand_command(command).unwrap();
        assert_eq!(
            expanded,
            vec![ParsedCommand {
                name: "open".into(),
                arguments: vec![
                    "--target".into(),
                    "tab".into(),
                    "https://example.test/a;;b".into()
                ],
            }]
        );
        assert!(registry.validate(&expanded[0], Mode::Normal).is_ok());
    }

    #[test]
    fn aliases_report_cycles_and_depth_limits() {
        let cycle = CommandRegistry::new_with_aliases(
            vec![definition(
                "stop",
                &[],
                vec![Mode::Normal],
                CountPolicy::NotSupported,
                false,
                "stop",
            )],
            vec![
                CommandAlias::from_text("first", "second").unwrap(),
                CommandAlias::from_text("second", "first").unwrap(),
            ],
        )
        .unwrap();
        assert_eq!(
            cycle.expand_command(ParsedCommand {
                name: "first".into(),
                arguments: Vec::new(),
            }),
            Err(CommandError::AliasCycle("first".into()))
        );

        let definitions = vec![definition(
            "stop",
            &[],
            vec![Mode::Normal],
            CountPolicy::NotSupported,
            false,
            "stop",
        )];
        assert_eq!(
            CommandRegistry::new_with_aliases(
                definitions.clone(),
                vec![CommandAlias::from_text("broken", "missing").unwrap()],
            )
            .unwrap_err(),
            CommandError::UnknownCommand("missing".into())
        );
        assert_eq!(
            CommandRegistry::new_with_aliases(
                definitions,
                vec![
                    CommandAlias::from_text("same", "stop").unwrap(),
                    CommandAlias::from_text("same", "stop").unwrap(),
                ],
            )
            .unwrap_err(),
            CommandError::DuplicateAlias("same".into())
        );

        let aliases = (0..=MAX_ALIAS_DEPTH)
            .map(|index| {
                let name = format!("alias-{index}");
                let target = if index == MAX_ALIAS_DEPTH {
                    "stop".into()
                } else {
                    format!("alias-{}", index + 1)
                };
                CommandAlias::new(
                    name,
                    vec![ParsedCommand {
                        name: target,
                        arguments: Vec::new(),
                    }],
                )
            })
            .collect();
        let deep = CommandRegistry::new_with_aliases(
            vec![definition(
                "stop",
                &[],
                vec![Mode::Normal],
                CountPolicy::NotSupported,
                false,
                "stop",
            )],
            aliases,
        )
        .unwrap();
        assert_eq!(
            deep.expand_command(ParsedCommand {
                name: "alias-0".into(),
                arguments: Vec::new(),
            }),
            Err(CommandError::AliasDepth)
        );
    }
}
