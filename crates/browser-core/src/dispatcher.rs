use std::fmt;

use crate::{
    ApplicationState, CommandError, CommandRegistry, Event, Mode, NavigationContext,
    NavigationError, ParsedCommand, ReduceError, TabId, Target, WindowId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandSource {
    Keyboard,
    Ui,
    Cli,
    Ipc,
    Switcher,
    Userscript,
    Macro,
}

impl CommandSource {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "keyboard" => Some(Self::Keyboard),
            "ui" => Some(Self::Ui),
            "cli" => Some(Self::Cli),
            "ipc" => Some(Self::Ipc),
            "switcher" => Some(Self::Switcher),
            "userscript" => Some(Self::Userscript),
            "macro" => Some(Self::Macro),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Keyboard => "keyboard",
            Self::Ui => "ui",
            Self::Cli => "cli",
            Self::Ipc => "ipc",
            Self::Switcher => "switcher",
            Self::Userscript => "userscript",
            Self::Macro => "macro",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandContext {
    pub source: CommandSource,
    pub count: u32,
    pub window: Option<WindowId>,
    pub tab: Option<TabId>,
    pub profile: Option<crate::ProfileId>,
    pub context: Option<String>,
    pub operation_id: Option<String>,
}

impl Default for CommandContext {
    fn default() -> Self {
        Self {
            source: CommandSource::Keyboard,
            count: 1,
            window: None,
            tab: None,
            profile: None,
            context: None,
            operation_id: None,
        }
    }
}

impl CommandContext {
    #[must_use]
    pub fn capture(
        state: &ApplicationState,
        selector: DispatchTarget,
        source: CommandSource,
        count: u32,
        operation_id: Option<String>,
    ) -> Self {
        let window = selected_window(state, selector);
        let tab = window.and_then(|window| selected_tab(state, selector, window).map(|tab| tab.id));
        let profile =
            window.and_then(|window| state.windows.get(&window).map(|window| window.profile));
        let context = window
            .and_then(|window| state.windows.get(&window))
            .and_then(|window| window.context.clone());
        Self {
            source,
            count,
            window,
            tab,
            profile,
            context,
            operation_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandInvocation {
    pub command: ParsedCommand,
    pub count: u32,
    pub context: CommandContext,
}

/// Selects the browser object against which a command is evaluated.
///
/// The selector is resolved at dispatch time. This keeps a forwarded command
/// deterministic if focus changes while its effects are being applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchTarget {
    Active,
    LastFocused,
    Window(WindowId),
    Tab(TabId),
}

impl CommandInvocation {
    #[must_use]
    pub fn new(command: ParsedCommand) -> Self {
        Self {
            command,
            count: 1,
            context: CommandContext::default(),
        }
    }

    #[must_use]
    pub fn with_count(command: ParsedCommand, count: u32) -> Self {
        Self {
            command,
            count,
            context: CommandContext {
                count,
                ..CommandContext::default()
            },
        }
    }

    #[must_use]
    pub fn with_context(mut self, mut context: CommandContext) -> Self {
        context.count = self.count;
        self.context = context;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchError {
    Command(CommandError),
    Navigation(NavigationError),
    Reduce(ReduceError),
    NoActiveTab,
    MissingArgument { command: String, argument: String },
    UnexpectedArguments { command: String },
    CountNotSupported { command: String },
    CountTooLarge { command: String, maximum: u32 },
    UnsupportedCommand(String),
}

impl fmt::Display for DispatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(error) => write!(formatter, "command parse/registry error: {error}"),
            Self::Navigation(error) => write!(formatter, "navigation error: {error}"),
            Self::Reduce(error) => write!(formatter, "state transition error: {error}"),
            Self::NoActiveTab => formatter.write_str("no active tab"),
            Self::MissingArgument { command, argument } => {
                write!(formatter, "{command} requires argument {argument}")
            }
            Self::UnexpectedArguments { command } => {
                write!(formatter, "{command} does not accept arguments")
            }
            Self::CountNotSupported { command } => {
                write!(formatter, "{command} does not accept a count")
            }
            Self::CountTooLarge { command, maximum } => {
                write!(formatter, "count for {command} exceeds {maximum}")
            }
            Self::UnsupportedCommand(command) => {
                write!(formatter, "unsupported command: {command}")
            }
        }
    }
}

impl std::error::Error for DispatchError {}

/// Converts one parsed command into typed reducer events without mutating
/// application state or performing engine/desktop side effects.
///
/// The active tab is captured once at dispatch time. Returned events retain
/// that target even if focus changes before an adapter executes them.
///
/// # Errors
///
/// Returns a typed error for mode/registry violations, malformed command
/// arguments, missing current state, invalid URL resolution, or unsupported
/// commands.
pub fn dispatch_command(
    state: &ApplicationState,
    registry: &CommandRegistry,
    invocation: CommandInvocation,
    context: &NavigationContext,
) -> Result<Vec<Event>, DispatchError> {
    dispatch_command_for(state, registry, invocation, context, DispatchTarget::Active)
}

/// Dispatches a command against an explicit window/tab selector.
///
/// # Errors
///
/// Returns the same validation, navigation, and state-target errors as
/// [`dispatch_command`].
#[allow(clippy::too_many_lines)]
pub fn dispatch_command_for(
    state: &ApplicationState,
    registry: &CommandRegistry,
    invocation: CommandInvocation,
    context: &NavigationContext,
    selector: DispatchTarget,
) -> Result<Vec<Event>, DispatchError> {
    let CommandInvocation {
        command,
        count,
        context: invocation_context,
    } = invocation;
    let expanded = registry
        .expand_command(command)
        .map_err(DispatchError::Command)?;
    if expanded.len() > 1 {
        let mut events = Vec::new();
        for command in expanded {
            events.extend(dispatch_command_for(
                state,
                registry,
                CommandInvocation {
                    command,
                    count,
                    context: invocation_context.clone(),
                },
                context,
                selector,
            )?);
        }
        return Ok(events);
    }
    let parsed = expanded
        .into_iter()
        .next()
        .ok_or(DispatchError::Command(CommandError::EmptyCommand))?;
    let window = selected_window(state, selector).ok_or(DispatchError::NoActiveTab)?;
    let mode = mode_for_window(state, window);
    registry
        .validate(&parsed, mode)
        .map_err(DispatchError::Command)?;
    let definition = registry
        .resolve(&parsed.name)
        .map_err(DispatchError::Command)?;
    let command = definition.name.as_str();
    let count = validate_count(command, count, definition.count)?;

    let target = selected_tab(state, selector, window).map(|tab| Target {
        tab: tab.id,
        generation: tab.generation,
        document: tab.document,
    });

    match command {
        "tab-open" => {
            let (input, background) = tab_open_arguments(&parsed, command)?;
            let navigation =
                crate::resolve_input(&input, context).map_err(DispatchError::Navigation)?;
            Ok(vec![Event::OpenTabWithNavigation {
                window,
                url: navigation.url,
                background,
            }])
        }
        "tab-next" => {
            let count = history_count(&parsed, command, count)?;
            let tab = adjacent_tab(state, window, count, true).ok_or(DispatchError::NoActiveTab)?;
            Ok(vec![Event::ActivateTab { window, tab }])
        }
        "tab-prev" => {
            let count = history_count(&parsed, command, count)?;
            let tab =
                adjacent_tab(state, window, count, false).ok_or(DispatchError::NoActiveTab)?;
            Ok(vec![Event::ActivateTab { window, tab }])
        }
        "tab-close" => {
            let (selector, requested_count) = parse_tab_close_arguments(&parsed, command)?;
            let close_count = requested_count.unwrap_or(count);
            let candidates = if let Some(selector) = selector {
                vec![
                    state
                        .windows
                        .get(&window)
                        .and_then(|window| {
                            window.tabs.iter().find_map(|tab| {
                                state.tabs.get(tab).filter(|tab| {
                                    tab.id.to_string() == selector
                                        && tab.existence == crate::ExistenceState::Live
                                })
                            })
                        })
                        .ok_or(DispatchError::NoActiveTab)?
                        .id,
                ]
            } else {
                close_candidates(state, window, close_count)
            };
            if candidates.is_empty() {
                return Err(DispatchError::NoActiveTab);
            }
            Ok(candidates
                .into_iter()
                .map(|tab| Event::CloseTab { tab })
                .collect())
        }
        "tab-select" => {
            if parsed.arguments.len() != 1 {
                return Err(if parsed.arguments.is_empty() {
                    DispatchError::MissingArgument {
                        command: command.into(),
                        argument: "selector".into(),
                    }
                } else {
                    DispatchError::UnexpectedArguments {
                        command: command.into(),
                    }
                });
            }
            let selector = &parsed.arguments[0];
            let tab = if selector == "last" {
                state
                    .windows
                    .get(&window)
                    .and_then(|window| window.tabs.last())
                    .copied()
            } else if let Ok(displayed_index) = selector.parse::<usize>() {
                if displayed_index == 0 {
                    return Err(DispatchError::NoActiveTab);
                }
                state
                    .windows
                    .get(&window)
                    .and_then(|window| window.tabs.get(displayed_index - 1))
                    .copied()
            } else {
                state
                    .tabs
                    .values()
                    .find(|tab| tab.window == window && tab.id.to_string() == *selector)
                    .map(|tab| tab.id)
            }
            .filter(|tab| {
                state
                    .tabs
                    .get(tab)
                    .is_some_and(|tab| tab.existence == crate::ExistenceState::Live)
            })
            .ok_or(DispatchError::NoActiveTab)?;
            Ok(vec![Event::ActivateTab { window, tab }])
        }
        "open" => {
            let target = target.ok_or(DispatchError::NoActiveTab)?;
            let input = joined_input(&parsed, command, "input")?;
            let navigation =
                crate::resolve_input(&input, context).map_err(DispatchError::Navigation)?;
            Ok(vec![Event::StartNavigation {
                target,
                url: navigation.url,
            }])
        }
        "back" | "forward" => {
            let requested_count = history_count(&parsed, command, count)?;
            let offset = i32::try_from(requested_count).unwrap_or(i32::MAX);
            Ok(vec![Event::TraverseHistory {
                target: target.ok_or(DispatchError::NoActiveTab)?,
                offset: if command == "back" { -offset } else { offset },
            }])
        }
        "reload" => {
            let bypass_cache = match parsed.arguments.as_slice() {
                [] => false,
                [argument] if argument == "--bypass-cache" => true,
                _ => {
                    return Err(DispatchError::UnexpectedArguments {
                        command: command.into(),
                    });
                }
            };
            Ok(vec![Event::Reload {
                target: target.ok_or(DispatchError::NoActiveTab)?,
                bypass_cache,
            }])
        }
        "stop" => {
            ensure_no_arguments(&parsed, command)?;
            Ok(vec![Event::Stop {
                target: target.ok_or(DispatchError::NoActiveTab)?,
            }])
        }
        "search-next" => {
            let target = target.ok_or(DispatchError::NoActiveTab)?;
            let (requested_backward, requested_count) =
                search_next_arguments(&parsed, command, count)?;
            let backward = requested_backward.unwrap_or_else(|| {
                state
                    .tabs
                    .get(&target.tab)
                    .and_then(|tab| tab.search.as_ref())
                    .is_some_and(|search| search.backward)
            });
            Ok((0..requested_count)
                .map(|_| Event::SearchNext { target, backward })
                .collect())
        }
        "scroll" | "scroll-page" | "scroll-to" => {
            Err(DispatchError::UnsupportedCommand(command.into()))
        }
        "window-close" | "quit" => {
            ensure_no_arguments(&parsed, command)?;
            Ok(vec![Event::RequestShutdown])
        }
        other => Err(DispatchError::UnsupportedCommand(other.into())),
    }
}

fn adjacent_tab(
    state: &ApplicationState,
    window: crate::WindowId,
    count: u32,
    forward: bool,
) -> Option<crate::TabId> {
    let window = state.windows.get(&window)?;
    let live = window
        .tabs
        .iter()
        .copied()
        .filter(|tab| {
            state
                .tabs
                .get(tab)
                .is_some_and(|tab| tab.existence == crate::ExistenceState::Live)
        })
        .collect::<Vec<_>>();
    if live.is_empty() {
        return None;
    }
    let active = window
        .active_tab
        .and_then(|active| live.iter().position(|tab| *tab == active))
        .unwrap_or(0);
    let distance = usize::try_from(count).unwrap_or(usize::MAX) % live.len();
    let offset = if forward {
        distance
    } else {
        live.len() - distance
    };
    live.get((active + offset) % live.len()).copied()
}

fn parse_tab_close_arguments(
    parsed: &ParsedCommand,
    command: &str,
) -> Result<(Option<String>, Option<u32>), DispatchError> {
    let mut selector = None;
    let mut count = None;
    let mut index = 0;
    while index < parsed.arguments.len() {
        match parsed.arguments[index].as_str() {
            "--id" => {
                if selector.is_some() {
                    return Err(DispatchError::UnexpectedArguments {
                        command: command.into(),
                    });
                }
                let value = parsed.arguments.get(index + 1).ok_or_else(|| {
                    DispatchError::MissingArgument {
                        command: command.into(),
                        argument: "id".into(),
                    }
                })?;
                selector = Some(value.clone());
                index += 2;
            }
            "--count" => {
                if count.is_some() {
                    return Err(DispatchError::UnexpectedArguments {
                        command: command.into(),
                    });
                }
                let value = parsed.arguments.get(index + 1).ok_or_else(|| {
                    DispatchError::MissingArgument {
                        command: command.into(),
                        argument: "count".into(),
                    }
                })?;
                let value = value
                    .parse::<u32>()
                    .ok()
                    .filter(|value| (1..=100).contains(value));
                count = Some(value.ok_or_else(|| DispatchError::UnexpectedArguments {
                    command: command.into(),
                })?);
                index += 2;
            }
            value if !value.starts_with('-') && selector.is_none() => {
                selector = Some(value.to_owned());
                index += 1;
            }
            _ => {
                return Err(DispatchError::UnexpectedArguments {
                    command: command.into(),
                });
            }
        }
    }
    if selector.is_some() && count.is_some() {
        return Err(DispatchError::UnexpectedArguments {
            command: command.into(),
        });
    }
    Ok((selector, count))
}

fn search_next_arguments(
    parsed: &ParsedCommand,
    command: &str,
    default_count: u32,
) -> Result<(Option<bool>, u32), DispatchError> {
    let mut backward = None;
    let mut requested_count = None;
    let mut index = 0;
    while index < parsed.arguments.len() {
        match parsed.arguments[index].as_str() {
            "--backward" if backward.is_none() => {
                backward = Some(true);
                index += 1;
            }
            "--count" if requested_count.is_none() => {
                let value = parsed.arguments.get(index + 1).ok_or_else(|| {
                    DispatchError::MissingArgument {
                        command: command.into(),
                        argument: "count".into(),
                    }
                })?;
                let value = value
                    .parse::<u32>()
                    .ok()
                    .filter(|value| (1..=100).contains(value))
                    .ok_or_else(|| DispatchError::UnexpectedArguments {
                        command: command.into(),
                    })?;
                requested_count = Some(value);
                index += 2;
            }
            _ => {
                return Err(DispatchError::UnexpectedArguments {
                    command: command.into(),
                });
            }
        }
    }
    Ok((backward, requested_count.unwrap_or(default_count)))
}

fn close_candidates(
    state: &ApplicationState,
    window: crate::WindowId,
    count: u32,
) -> Vec<crate::TabId> {
    let Some(window_state) = state.windows.get(&window) else {
        return Vec::new();
    };
    let mut ordered = Vec::with_capacity(window_state.tabs.len());
    if let Some(active) = window_state.active_tab {
        ordered.push(active);
    }
    ordered.extend(
        window_state
            .tabs
            .iter()
            .copied()
            .filter(|tab| Some(*tab) != window_state.active_tab),
    );
    ordered
        .into_iter()
        .filter(|tab| {
            state
                .tabs
                .get(tab)
                .is_some_and(|tab| tab.existence == crate::ExistenceState::Live && !tab.pinned)
        })
        .take(usize::try_from(count).unwrap_or(usize::MAX))
        .collect()
}

fn mode_for_window(state: &ApplicationState, window: WindowId) -> Mode {
    state
        .windows
        .get(&window)
        .and_then(|window| window.modes.last().copied())
        .unwrap_or(Mode::Normal)
}

fn selected_window(state: &ApplicationState, selector: DispatchTarget) -> Option<WindowId> {
    match selector {
        DispatchTarget::Active => state.active_window,
        DispatchTarget::LastFocused => state.last_focused_window.or(state.active_window),
        DispatchTarget::Window(window) => state.windows.contains_key(&window).then_some(window),
        DispatchTarget::Tab(tab) => state.tabs.get(&tab).map(|tab| tab.window),
    }
}

fn selected_tab(
    state: &ApplicationState,
    selector: DispatchTarget,
    window: WindowId,
) -> Option<&crate::TabState> {
    match selector {
        DispatchTarget::Tab(tab) => state
            .tabs
            .get(&tab)
            .filter(|tab| tab.window == window && tab.existence == crate::ExistenceState::Live),
        DispatchTarget::Active | DispatchTarget::LastFocused | DispatchTarget::Window(_) => state
            .windows
            .get(&window)
            .and_then(|window| window.active_tab)
            .and_then(|tab| state.tabs.get(&tab))
            .filter(|tab| tab.existence == crate::ExistenceState::Live),
    }
}

fn validate_count(
    command: &str,
    count: u32,
    policy: crate::CountPolicy,
) -> Result<u32, DispatchError> {
    match policy {
        crate::CountPolicy::NotSupported if count != 1 => Err(DispatchError::CountNotSupported {
            command: command.into(),
        }),
        crate::CountPolicy::Supported { maximum } if count > maximum => {
            Err(DispatchError::CountTooLarge {
                command: command.into(),
                maximum,
            })
        }
        _ => Ok(count),
    }
}

fn joined_input(
    command: &ParsedCommand,
    name: &str,
    argument: &str,
) -> Result<String, DispatchError> {
    let arguments = if command.arguments.first().is_some_and(|value| value == "--") {
        &command.arguments[1..]
    } else {
        if command
            .arguments
            .first()
            .is_some_and(|value| value.starts_with('-'))
        {
            return Err(DispatchError::UnexpectedArguments {
                command: name.into(),
            });
        }
        &command.arguments
    };
    if arguments.is_empty() {
        return Err(DispatchError::MissingArgument {
            command: name.into(),
            argument: argument.into(),
        });
    }
    Ok(arguments.join(" "))
}

fn tab_open_arguments(
    command: &ParsedCommand,
    name: &str,
) -> Result<(String, bool), DispatchError> {
    let mut background = false;
    let mut input = Vec::new();
    let mut options_ended = false;
    for argument in &command.arguments {
        if !options_ended {
            match argument.as_str() {
                "--background" if !background => {
                    background = true;
                    continue;
                }
                "--background" => {
                    return Err(DispatchError::UnexpectedArguments {
                        command: name.into(),
                    });
                }
                "--" => {
                    options_ended = true;
                    continue;
                }
                value if value.starts_with('-') => {
                    return Err(DispatchError::UnexpectedArguments {
                        command: name.into(),
                    });
                }
                _ => {}
            }
        }
        input.push(argument.clone());
    }
    if input.is_empty() {
        return Err(DispatchError::MissingArgument {
            command: name.into(),
            argument: "input".into(),
        });
    }
    Ok((input.join(" "), background))
}

fn history_count(
    command: &ParsedCommand,
    name: &str,
    default_count: u32,
) -> Result<u32, DispatchError> {
    match command.arguments.as_slice() {
        [] => Ok(default_count),
        [flag, value] if flag == "--count" => value
            .parse::<u32>()
            .ok()
            .filter(|value| (1..=100).contains(value))
            .ok_or_else(|| DispatchError::UnexpectedArguments {
                command: name.into(),
            }),
        _ => Err(DispatchError::UnexpectedArguments {
            command: name.into(),
        }),
    }
}

fn ensure_no_arguments(command: &ParsedCommand, name: &str) -> Result<(), DispatchError> {
    if command.arguments.is_empty() {
        Ok(())
    } else {
        Err(DispatchError::UnexpectedArguments {
            command: name.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, ParseInput, PrivacyKind, reduce};

    fn state() -> ApplicationState {
        let mut state = ApplicationState::new();
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "default".into(),
                privacy: PrivacyKind::Normal,
            },
        )
        .unwrap();
        let profile = *state.profiles.keys().next().unwrap();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows.keys().next().unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        state
    }

    fn dispatch_cli(
        state: &ApplicationState,
        registry: &CommandRegistry,
        input: &str,
    ) -> Result<Vec<Event>, DispatchError> {
        let command = crate::parse_chain(input, ParseInput::Cli)
            .unwrap()
            .remove(0);
        dispatch_command(
            state,
            registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
        )
    }

    #[test]
    fn open_resolves_to_a_start_navigation_event() {
        let state = state();
        let registry = CommandRegistry::default_v1();
        let command = crate::parse_chain("open example.test", ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
        )
        .unwrap();
        assert!(
            matches!(events.as_slice(), [Event::StartNavigation { url, .. }] if url.as_str() == "https://example.test")
        );
    }

    #[test]
    fn command_context_captures_the_selected_identity_once() {
        let state = state();
        let window = state.active_window.expect("active window");
        let tab = state.windows[&window].active_tab.expect("active tab");
        let profile = state.windows[&window].profile;
        let context = CommandContext::capture(
            &state,
            DispatchTarget::Active,
            CommandSource::Ipc,
            4,
            Some("op-1".into()),
        );
        assert_eq!(context.source.as_str(), "ipc");
        assert_eq!(context.count, 4);
        assert_eq!(context.window, Some(window));
        assert_eq!(context.tab, Some(tab));
        assert_eq!(context.profile, Some(profile));
        assert_eq!(context.context, None);
        assert_eq!(context.operation_id.as_deref(), Some("op-1"));
    }

    #[test]
    fn open_option_terminator_keeps_the_url_as_data() {
        let state = state();
        let registry = CommandRegistry::default_v1();
        let command = crate::parse_chain("open -- https://example.test/a", ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
        )
        .unwrap();
        assert!(
            matches!(events.as_slice(), [Event::StartNavigation { url, .. }] if url.as_str() == "https://example.test/a")
        );
    }

    #[test]
    fn open_rejects_unknown_flags_before_effects() {
        let state = state();
        let registry = CommandRegistry::default_v1();
        let command = crate::parse_chain("open --unknown https://example.test", ParseInput::Cli)
            .unwrap()
            .remove(0);
        assert_eq!(
            dispatch_command(
                &state,
                &registry,
                CommandInvocation::new(command),
                &NavigationContext::default(),
            ),
            Err(DispatchError::UnexpectedArguments {
                command: "open".into()
            })
        );
    }

    #[test]
    fn dispatch_expands_alias_arguments_and_command_chains() {
        let state = state();
        let base = CommandRegistry::default_v1();
        let registry = CommandRegistry::new_with_aliases(
            base.definitions().to_vec(),
            vec![
                crate::CommandAlias::from_text("background", "tab-open --background").unwrap(),
                crate::CommandAlias::from_text("history-pair", "back;;forward").unwrap(),
            ],
        )
        .unwrap();

        let command = crate::parse_chain("background https://example.test/path", ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
        )
        .unwrap();
        assert!(matches!(
            events.as_slice(),
            [Event::OpenTabWithNavigation {
                background: true,
                url,
                ..
            }] if url.as_str() == "https://example.test/path"
        ));

        let command = crate::parse_chain("history-pair", ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
        )
        .unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                Event::TraverseHistory { offset: -1, .. },
                Event::TraverseHistory { offset: 1, .. }
            ]
        ));
    }

    #[test]
    fn history_and_counts_are_typed_and_arguments_are_checked() {
        let state = state();
        let registry = CommandRegistry::default_v1();
        let command = crate::parse_chain("back", ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::with_count(command, 3),
            &NavigationContext::default(),
        )
        .unwrap();
        assert!(matches!(
            events.as_slice(),
            [Event::TraverseHistory { offset: -3, .. }]
        ));

        let events = dispatch_cli(&state, &registry, "forward --count 4").unwrap();
        assert!(matches!(
            events.as_slice(),
            [Event::TraverseHistory { offset: 4, .. }]
        ));

        for input in [
            "back --count 2 --count 3",
            "forward 2",
            "forward --count 0",
            "tab-next --count 101",
        ] {
            assert_eq!(
                dispatch_cli(&state, &registry, input),
                Err(DispatchError::UnexpectedArguments {
                    command: input.split_whitespace().next().unwrap().into(),
                }),
                "navigation command unexpectedly accepted: {input}"
            );
        }

        let mut tab_state = state.clone();
        let tab_window = tab_state.active_window.unwrap();
        reduce(&mut tab_state, Event::OpenTab { window: tab_window }).unwrap();
        reduce(&mut tab_state, Event::OpenTab { window: tab_window }).unwrap();
        let expected = tab_state.windows[&tab_window].tabs[1];
        let events = dispatch_cli(&tab_state, &registry, "tab-next --count 2").unwrap();
        assert_eq!(
            events,
            vec![Event::ActivateTab {
                window: tab_window,
                tab: expected
            }]
        );

        assert_eq!(
            dispatch_cli(&state, &registry, "reload extra"),
            Err(DispatchError::UnexpectedArguments {
                command: "reload".into()
            })
        );

        let command = crate::parse_chain("search-next", ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::with_count(command, 3),
            &NavigationContext::default(),
        )
        .unwrap();
        assert_eq!(events.len(), 3);
        assert!(events.iter().all(|event| matches!(
            event,
            Event::SearchNext {
                backward: false,
                ..
            }
        )));
    }

    #[test]
    fn search_next_accepts_typed_direction_and_count_flags() {
        let state = state();
        let registry = CommandRegistry::default_v1();
        let events = dispatch_cli(&state, &registry, "search-next --backward --count 3")
            .expect("typed search-next flags");
        assert!(matches!(
            events.as_slice(),
            [
                Event::SearchNext { backward: true, .. },
                Event::SearchNext { backward: true, .. },
                Event::SearchNext { backward: true, .. }
            ]
        ));
        assert!(dispatch_cli(&state, &registry, "search-next --backward --backward").is_err());
        assert!(dispatch_cli(&state, &registry, "search-next --count 0").is_err());
    }

    #[test]
    fn tab_commands_use_ordered_live_window_tabs() {
        let mut state = state();
        let window = state.active_window.unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let registry = CommandRegistry::default_v1();
        let first_tab = state.windows[&window].tabs[0];
        let command = crate::parse_chain("tab-select 1", ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
        )
        .unwrap();
        assert_eq!(
            events,
            vec![Event::ActivateTab {
                window,
                tab: first_tab
            }]
        );
        let last_tab = *state.windows[&window].tabs.last().expect("last tab");
        let events = dispatch_cli(&state, &registry, "tab-select last")
            .expect("last-tab selector should resolve");
        assert_eq!(
            events,
            vec![Event::ActivateTab {
                window,
                tab: last_tab
            }]
        );
        let command = crate::parse_chain(&format!("tab-select {first_tab}"), ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
        )
        .unwrap();
        assert_eq!(
            events,
            vec![Event::ActivateTab {
                window,
                tab: first_tab
            }]
        );
        let command = crate::parse_chain("tab-select 0", ParseInput::Cli)
            .unwrap()
            .remove(0);
        assert!(matches!(
            dispatch_command(
                &state,
                &registry,
                CommandInvocation::new(command),
                &NavigationContext::default(),
            ),
            Err(DispatchError::NoActiveTab)
        ));
        let command = crate::parse_chain("tab-prev", ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
        )
        .unwrap();
        let active = state.windows[&window].active_tab.unwrap();
        assert!(matches!(events.as_slice(), [Event::ActivateTab { tab, .. }] if *tab != active));
        let command = crate::parse_chain("tab-open example.test", ParseInput::Cli)
            .unwrap()
            .remove(0);
        assert_eq!(
            dispatch_command(
                &state,
                &registry,
                CommandInvocation::new(command),
                &NavigationContext::default(),
            )
            .unwrap(),
            vec![Event::OpenTabWithNavigation {
                window,
                url: crate::ValidatedUrl::parse("https://example.test").unwrap(),
                background: false,
            }]
        );
    }

    #[test]
    fn tab_open_resolves_input_and_preserves_focus_for_background_tabs() {
        let mut state = state();
        let window = state.active_window.unwrap();
        let original_tab = state.windows[&window].active_tab.unwrap();
        let registry = CommandRegistry::default_v1();
        let command = crate::parse_chain(
            "tab-open --background https://example.test/path",
            ParseInput::Cli,
        )
        .unwrap()
        .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
        )
        .unwrap();
        assert!(matches!(
            events.as_slice(),
            [Event::OpenTabWithNavigation {
                background: true,
                url,
                ..
            }] if url.as_str() == "https://example.test/path"
        ));
        let effects = reduce(&mut state, events.into_iter().next().unwrap()).unwrap();
        assert_eq!(state.windows[&window].active_tab, Some(original_tab));
        assert_eq!(state.windows[&window].tabs.len(), 2);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            crate::Effect::Engine(crate::EngineEffect::Navigate { url, .. })
                if url.as_str() == "https://example.test/path"
        )));
    }

    #[test]
    fn tab_close_supports_targeted_and_unpinned_bulk_forms() {
        let mut state = state();
        let window = state.active_window.unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tabs = state.windows[&window].tabs.clone();
        state.tabs.get_mut(&tabs[0]).unwrap().pinned = true;
        let registry = CommandRegistry::default_v1();

        let command = crate::parse_chain("tab-close --count 2", ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
        )
        .unwrap();
        assert_eq!(
            events,
            vec![
                Event::CloseTab { tab: tabs[2] },
                Event::CloseTab { tab: tabs[1] }
            ]
        );

        let command = crate::parse_chain(&format!("tab-close --id {}", tabs[0]), ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command(
            &state,
            &registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
        )
        .unwrap();
        assert_eq!(events, vec![Event::CloseTab { tab: tabs[0] }]);
    }

    #[test]
    fn explicit_dispatch_target_uses_last_focused_window() {
        let mut state = state();
        let profile = *state.profiles.keys().next().unwrap();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let last_focused = state.last_focused_window.unwrap();
        reduce(
            &mut state,
            Event::OpenTab {
                window: last_focused,
            },
        )
        .unwrap();
        let active = state
            .windows
            .keys()
            .copied()
            .find(|window| *window != last_focused)
            .unwrap();
        state.active_window = Some(active);
        let registry = CommandRegistry::default_v1();
        let command = crate::parse_chain("reload", ParseInput::Cli)
            .unwrap()
            .remove(0);
        let events = dispatch_command_for(
            &state,
            &registry,
            CommandInvocation::new(command),
            &NavigationContext::default(),
            DispatchTarget::LastFocused,
        )
        .unwrap();
        assert!(matches!(
            events.as_slice(),
            [Event::Reload { target, .. }]
                if state.tabs[&target.tab].window == last_focused
        ));
    }

    #[test]
    fn quit_dispatches_a_shutdown_request_without_a_page_target() {
        let state = state();
        let registry = CommandRegistry::default_v1();
        let command = crate::parse_chain("quit", ParseInput::Cli)
            .unwrap()
            .remove(0);
        assert_eq!(
            dispatch_command(
                &state,
                &registry,
                CommandInvocation::new(command),
                &NavigationContext::default(),
            )
            .unwrap(),
            vec![Event::RequestShutdown]
        );
        let command = crate::parse_chain("window-close", ParseInput::Cli)
            .unwrap()
            .remove(0);
        assert_eq!(
            dispatch_command(
                &state,
                &registry,
                CommandInvocation::new(command),
                &NavigationContext::default(),
            )
            .unwrap(),
            vec![Event::RequestShutdown]
        );
    }
}
