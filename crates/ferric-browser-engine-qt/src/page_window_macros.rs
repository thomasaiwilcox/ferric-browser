//! Memory-only recordings shared by the live owners of one exact page group.
use crate::{BrowserUiRust, RepeatableCommand, page_window_undo::Scope};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Mutex, OnceLock},
};

const MAX_GROUPS: usize = 128;
const MAX_STORED_BYTES: usize = 8 * 1024 * 1024;
const MAX_STORED_COMMANDS: usize = 64_000;

#[derive(Clone, Default)]
pub(crate) struct MacroState {
    pub(crate) last: Option<RepeatableCommand>,
    pub(crate) registers: BTreeMap<String, Vec<RepeatableCommand>>,
    pub(crate) recording: Option<(String, Vec<RepeatableCommand>)>,
}

impl MacroState {
    fn commands(&self) -> impl Iterator<Item = &RepeatableCommand> {
        self.last
            .iter()
            .chain(self.registers.values().flatten())
            .chain(self.recording.iter().flat_map(|(_, commands)| commands))
    }
}

#[derive(Default)]
struct Store {
    owners: HashMap<String, Scope>,
    groups: Vec<(Scope, MacroState)>,
}

pub(super) fn command_bytes(command: &RepeatableCommand) -> usize {
    let payload = &command.command;
    std::mem::size_of::<RepeatableCommand>()
        + payload.name.len()
        + payload
            .arguments
            .iter()
            .map(|v| v.len() + std::mem::size_of::<String>())
            .sum::<usize>()
        + command.route.as_ref().map_or(0, |route| {
            route.profile.as_ref().map_or(0, String::len)
                + route.context.as_ref().map_or(0, String::len)
        })
}

impl Store {
    fn prune(&mut self) {
        self.groups
            .retain(|(scope, _)| self.owners.values().any(|owner| owner == scope));
    }

    fn state_mut(&mut self, scope: &Scope) -> Result<&mut MacroState, String> {
        if let Some(index) = self.groups.iter().position(|(saved, _)| saved == scope) {
            return Ok(&mut self.groups[index].1);
        }
        if self.groups.len() >= MAX_GROUPS || !self.owners.values().any(|owner| owner == scope) {
            return Err("Macro state is unavailable for this page group".into());
        }
        self.groups.push((scope.clone(), MacroState::default()));
        Ok(&mut self.groups.last_mut().expect("inserted group").1)
    }

    fn note(&mut self, scope: &Scope, mut recorded: RepeatableCommand) -> Result<(), String> {
        if let Some(route) = &mut recorded.route {
            route.selector = crate::DispatchTarget::Active;
        }
        let (count, bytes) = self
            .groups
            .iter()
            .flat_map(|(_, state)| state.commands())
            .fold((0, 0), |(count, bytes), command| {
                (count + 1, bytes + command_bytes(command))
            });
        let state = self.state_mut(scope)?;
        let copies = if state.recording.is_some() { 2 } else { 1 };
        let replaced = state.last.as_ref().map_or(0, command_bytes);
        if count - usize::from(state.last.is_some()) + copies > MAX_STORED_COMMANDS
            || bytes - replaced + copies * command_bytes(&recorded) > MAX_STORED_BYTES
        {
            state.recording = None;
            state.last = None;
            return Err("Macro recording stopped at the process memory limit".into());
        }
        state.last = Some(recorded.clone());
        if let Some((_, commands)) = &mut state.recording {
            if commands.len() >= crate::MAX_MACRO_COMMANDS {
                state.recording = None;
                return Err("Macro recording stopped at the 1000-command limit".into());
            }
            commands.push(recorded);
        }
        Ok(())
    }
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(Mutex::default)
}

pub(crate) fn register(rust: &BrowserUiRust) -> bool {
    let token = rust.window_token.to_string();
    let scope = (!rust.page_window_closing && rust.view_alive && !rust.tab_ids.is_empty())
        .then(|| Scope::capture(rust))
        .flatten();
    if let Ok(mut store) = store().lock() {
        let changed = store.owners.get(&token) != scope.as_ref();
        match scope {
            Some(scope) => {
                store.owners.insert(token, scope);
            }
            None => {
                store.owners.remove(&token);
            }
        }
        store.prune();
        changed
    } else {
        false
    }
}

pub(crate) fn release(token: &str) {
    if let Ok(mut store) = store().lock() {
        store.owners.remove(token);
        store.prune();
    }
}

pub(crate) fn snapshot(rust: &BrowserUiRust) -> MacroState {
    if let Some(scope) = Scope::capture(rust) {
        return store()
            .lock()
            .ok()
            .and_then(|store| {
                store
                    .groups
                    .iter()
                    .find(|(saved, _)| *saved == scope)
                    .map(|(_, state)| state.clone())
            })
            .unwrap_or_default();
    }
    MacroState {
        last: rust.last_repeatable.clone(),
        registers: rust.macro_registers.clone(),
        recording: rust.recording_macro.clone(),
    }
}

pub(crate) fn recording_register(rust: &BrowserUiRust) -> Option<String> {
    if let Some(scope) = Scope::capture(rust) {
        return store().lock().ok().and_then(|store| {
            store
                .groups
                .iter()
                .find(|(saved, _)| *saved == scope)
                .and_then(|(_, state)| {
                    state
                        .recording
                        .as_ref()
                        .map(|(register, _)| register.clone())
                })
        });
    }
    rust.recording_macro
        .as_ref()
        .map(|(register, _)| register.clone())
}

pub(crate) fn note(scope: &Scope, recorded: RepeatableCommand) -> Result<(), String> {
    store()
        .lock()
        .map_err(|_| "Macro state is unavailable".to_owned())?
        .note(scope, recorded)
}

pub(crate) fn begin(scope: &Scope, register: &str) -> Result<(), String> {
    store()
        .lock()
        .map_err(|_| "Macro state is unavailable".to_owned())?
        .state_mut(scope)?
        .recording = Some((register.into(), Vec::new()));
    Ok(())
}

pub(crate) fn stop(scope: &Scope) -> Result<String, String> {
    let mut store = store()
        .lock()
        .map_err(|_| "Macro state is unavailable".to_owned())?;
    let state = store.state_mut(scope)?;
    let (register, commands) = state.recording.take().ok_or("No macro is recording")?;
    state.registers.insert(register.clone(), commands);
    Ok(register)
}

pub(crate) fn abort(rust: &BrowserUiRust) {
    if let Some(scope) = Scope::capture(rust)
        && let Ok(mut store) = store().lock()
        && let Ok(state) = store.state_mut(&scope)
    {
        state.recording = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrivacyKind;
    use ferric_browser_application::{BrowserApplication, ProfileOpenRequest, ProfileStorage};
    use ferric_browser_config::{Config, ContextsConfig, RuntimeOverrides};
    use std::time::{Duration, Instant};
    use uuid::Uuid;
    fn scope(privacy: PrivacyKind) -> Scope {
        let (mut application, _, _) =
            BrowserApplication::bootstrap(Config::default(), privacy, "undo").unwrap();
        application
            .request_profile_open(ProfileOpenRequest {
                name: "undo".into(),
                label: "Undo".into(),
                privacy,
                storage: ProfileStorage::Transient,
                base_configuration: Config::default(),
                initial_profile_overrides: RuntimeOverrides::default(),
                command_line_overrides: RuntimeOverrides::default(),
                config_path: None,
                script_catalog_root: None,
                contexts: ContextsConfig::default(),
                context_configuration_error: None,
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(result) = application.poll_profile_open() {
                result.unwrap();
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        Scope {
            profile: application.profile_instance_id().unwrap(),
            privacy,
            context: None,
            group: Uuid::new_v4().to_string(),
        }
    }

    fn recorded() -> RepeatableCommand {
        RepeatableCommand {
            command: crate::ParsedCommand {
                name: "tab-next".into(),
                arguments: Vec::new(),
            },
            count: 3,
            route: None,
        }
    }

    #[test]
    fn recordings_share_only_the_exact_scope_and_expire_with_its_last_owner() {
        let current = scope(PrivacyKind::Private);
        let mut store = Store::default();
        store.owners.insert("first".into(), current.clone());
        store.owners.insert("second".into(), current.clone());
        store.state_mut(&current).unwrap().recording = Some(("a".into(), Vec::new()));
        store.note(&current, recorded()).unwrap();
        assert_eq!(
            store
                .state_mut(&current)
                .unwrap()
                .recording
                .as_ref()
                .unwrap()
                .1
                .len(),
            1
        );
        assert_eq!(
            store
                .state_mut(&current)
                .unwrap()
                .last
                .as_ref()
                .unwrap()
                .count,
            3
        );
        for dimension in 0..4 {
            let mut foreign = current.clone();
            match dimension {
                0 => foreign.profile = scope(PrivacyKind::Private).profile,
                1 => foreign.privacy = PrivacyKind::Normal,
                2 => foreign.context = Some("other".into()),
                _ => foreign.group = "other".into(),
            }
            store.owners.insert("foreign".into(), foreign.clone());
            assert!(store.state_mut(&foreign).unwrap().recording.is_none());
        }
        store.owners.remove("first");
        store.prune();
        assert!(store.state_mut(&current).unwrap().last.is_some());
        store.owners.remove("second");
        store.prune();
        assert!(store.state_mut(&current).is_err());
        store.owners.insert("new".into(), current.clone());
        assert!(store.state_mut(&current).unwrap().last.is_none());
    }

    #[test]
    fn oversized_recordings_stop_without_evicting_another_group() {
        let current = scope(PrivacyKind::Ephemeral);
        let mut peer = current.clone();
        peer.group = "peer".into();
        let mut store = Store::default();
        store.owners.insert("current".into(), current.clone());
        store.owners.insert("peer".into(), peer.clone());
        store.note(&peer, recorded()).unwrap();
        store.state_mut(&current).unwrap().recording = Some(("a".into(), Vec::new()));
        let mut huge = recorded();
        huge.command.arguments.push("x".repeat(MAX_STORED_BYTES));
        assert!(store.note(&current, huge).is_err());
        assert!(store.state_mut(&current).unwrap().recording.is_none());
        assert!(store.state_mut(&peer).unwrap().last.is_some());
        store.state_mut(&current).unwrap().recording = Some(("a".into(), Vec::new()));
        for _ in 0..crate::MAX_MACRO_COMMANDS {
            store.note(&current, recorded()).unwrap();
        }
        assert!(store.note(&current, recorded()).is_err());
        assert!(store.state_mut(&current).unwrap().recording.is_none());
    }
}
