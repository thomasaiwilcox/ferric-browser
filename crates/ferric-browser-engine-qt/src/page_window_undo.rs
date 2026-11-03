//! Bounded, process-local undo for actual profile sessions. No live engine
//! state is retained; private descriptors disappear with their final owner.

use crate::{BrowserUiRust, ClosedTabDescriptor, PrivacyKind, TabId, Uuid};
use ferric_browser_application::ProfileInstanceId;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Scope {
    pub(crate) profile: ProfileInstanceId,
    pub(crate) privacy: PrivacyKind,
    pub(crate) context: Option<String>,
    pub(crate) group: String,
}

impl Scope {
    pub(crate) fn capture_page(rust: &BrowserUiRust, page: TabId) -> Option<Self> {
        Self::capture(rust).map(|scope| scope.with_page_metadata(rust, page))
    }

    fn with_page_metadata(mut self, rust: &BrowserUiRust, page: TabId) -> Self {
        if !rust.interaction.prompt_view_identities.contains_key(&page)
            && let Some(metadata) = rust.restored_page_windows.get(&page)
        {
            self.context.clone_from(&metadata.context);
            self.group.clone_from(&metadata.group);
        }
        self
    }
    pub(crate) fn capture(rust: &BrowserUiRust) -> Option<Self> {
        let state = rust.state.as_ref()?;
        if !rust.page_window_ordinary
            || state.configuration().tabs.presentation
                != ferric_browser_config::TabsPresentation::Windows
        {
            return None;
        }
        let window = state.windows().get(&rust.window?)?;
        Some(Self {
            profile: state.profile_instance_id()?,
            privacy: state.profiles().get(&window.profile)?.privacy,
            context: window.context.clone(),
            group: if rust.page_window_group.is_empty() {
                rust.window_token.to_string()
            } else {
                rust.page_window_group.clone()
            },
        })
    }

    pub(crate) fn same_profile(&self, other: &Self) -> bool {
        self.profile == other.profile && self.privacy == other.privacy
    }
}

#[derive(Default)]
struct Store {
    owners: HashMap<String, Scope>,
    entries: Vec<(Option<TabId>, ClosedTabDescriptor)>,
    claims: HashSet<(ProfileInstanceId, Uuid)>,
}

impl Store {
    fn release(&mut self, token: &str) {
        let Some(scope) = self.owners.remove(token) else {
            return;
        };
        if scope.privacy.is_transient()
            && !self.owners.values().any(|owner| owner.same_profile(&scope))
        {
            self.entries.retain(|(_, entry)| {
                entry
                    .scope
                    .as_ref()
                    .is_none_or(|entry| !entry.same_profile(&scope))
            });
        }
    }

    fn record(&mut self, page: Option<TabId>, closed: ClosedTabDescriptor, limit: usize) {
        if !crate::url_safety::is_safe_tab_descriptor(&closed.url) {
            return;
        }
        let Some(scope) = closed.scope.as_ref() else {
            return;
        };
        if scope.privacy.is_transient()
            && !self.owners.values().any(|owner| owner.same_profile(scope))
        {
            return;
        }
        if self.entries.iter().any(|(id, entry)| {
            entry
                .scope
                .as_ref()
                .is_some_and(|entry_scope| entry_scope.same_profile(scope))
                && (entry.id == closed.id || page.is_some() && *id == page)
        }) {
            return;
        }
        let mut retained = 0;
        self.entries.retain(|(_, entry)| {
            if entry
                .scope
                .as_ref()
                .is_some_and(|entry| entry.same_profile(scope))
            {
                retained += 1;
                retained < limit.min(100)
            } else {
                true
            }
        });
        if limit > 0 {
            self.entries.insert(0, (page, closed));
            self.entries.truncate(1_000);
        }
    }

    fn set_limit(&mut self, scope: &Scope, limit: usize) {
        let mut retained = 0;
        self.entries.retain(|(_, entry)| {
            if entry
                .scope
                .as_ref()
                .is_some_and(|entry| entry.same_profile(scope))
            {
                retained += 1;
                retained <= limit.min(100)
            } else {
                true
            }
        });
    }

    fn list(&self, scope: &Scope, all_contexts: bool) -> Vec<ClosedTabDescriptor> {
        let mut entries = self
            .entries
            .iter()
            .filter_map(|(_, entry)| {
                let saved = entry.scope.as_ref()?;
                (saved.same_profile(scope)
                    && (all_contexts || saved.context == scope.context)
                    && !self.claims.contains(&(scope.profile, entry.id)))
                .then(|| entry.clone())
            })
            .collect::<Vec<_>>();
        if !all_contexts {
            entries.sort_by_key(|entry| {
                entry
                    .scope
                    .as_ref()
                    .is_none_or(|entry| entry.group != scope.group)
            });
        }
        entries
    }

    fn claim(&mut self, id: Uuid, scope: &Scope) -> bool {
        self.entries.iter().any(|(_, entry)| {
            entry.id == id
                && entry.scope.as_ref().is_some_and(|saved| {
                    saved.same_profile(scope) && saved.context == scope.context
                })
        }) && self.claims.insert((scope.profile, id))
    }
}

pub(crate) struct UndoClaim {
    id: Uuid,
    scope: Scope,
}

impl Drop for UndoClaim {
    fn drop(&mut self) {
        if let Ok(mut store) = store().lock() {
            store.claims.remove(&(self.scope.profile, self.id));
        }
        crate::live_ipc_owners::page_undo_changed(&self.scope);
    }
}

pub(crate) fn claim(id: Uuid, scope: &Scope) -> Option<UndoClaim> {
    store().lock().ok()?.claim(id, scope).then(|| UndoClaim {
        id,
        scope: scope.clone(),
    })
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(Mutex::default)
}

pub(crate) fn register(rust: &BrowserUiRust) {
    if rust.page_window_closing || !rust.view_alive || rust.tab_ids.is_empty() {
        release(&rust.window_token.to_string());
        return;
    }
    let Some(scope) = Scope::capture(rust) else {
        release(&rust.window_token.to_string());
        return;
    };
    if let Ok(mut store) = store().lock() {
        let token = rust.window_token.to_string();
        if store
            .owners
            .get(&token)
            .is_some_and(|old| !old.same_profile(&scope))
        {
            store.release(&token);
        }
        store.set_limit(&scope, crate::configured_undo_limit(&rust.config));
        store.owners.insert(token, scope);
    }
}

pub(crate) fn release(token: &str) {
    if let Ok(mut store) = store().lock() {
        store.release(token);
    }
}

pub(crate) fn record(page: Option<TabId>, closed: ClosedTabDescriptor, limit: usize) {
    let scope = closed.scope.clone();
    if let Ok(mut store) = store().lock() {
        store.record(page, closed, limit);
    }
    if let Some(scope) = scope {
        crate::live_ipc_owners::page_undo_changed(&scope);
    }
}

pub(crate) fn list(rust: &BrowserUiRust, all_contexts: bool) -> Vec<ClosedTabDescriptor> {
    let mut entries = Scope::capture(rust)
        .and_then(|scope| {
            store()
                .lock()
                .ok()
                .map(|store| store.list(&scope, all_contexts))
        })
        .unwrap_or_default();
    entries.truncate(crate::configured_undo_limit(&rust.config));
    entries
}

pub(crate) fn remove(id: Uuid, scope: &Scope) {
    if let Ok(mut store) = store().lock() {
        store.entries.retain(|(_, entry)| {
            entry.id != id
                || entry
                    .scope
                    .as_ref()
                    .is_none_or(|entry| !entry.same_profile(scope))
        });
    }
    crate::live_ipc_owners::page_undo_changed(scope);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_application::{BrowserApplication, ProfileOpenRequest, ProfileStorage};
    use ferric_browser_config::{Config, ContextsConfig, RuntimeOverrides};
    use std::time::{Duration, Instant};

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

    fn closed(scope: &Scope, title: &str) -> ClosedTabDescriptor {
        ClosedTabDescriptor {
            id: Uuid::new_v4(),
            url: "https://undo.test/page".into(),
            title: title.into(),
            profile: "Undo".into(),
            private: scope.privacy.is_transient(),
            closed_at: 1,
            group_index: Some(0),
            pinned: false,
            muted: false,
            zoom_hundredths: 100,
            scope: Some(scope.clone()),
        }
    }

    #[test]
    fn undo_is_session_and_context_scoped_and_prefers_the_current_group() {
        let current = scope(PrivacyKind::Private);
        let mut peer = current.clone();
        peer.group = Uuid::new_v4().to_string();
        let mut context = current.clone();
        context.context = Some("work".into());
        let foreign = scope(PrivacyKind::Private);
        let mut store = Store::default();
        store.owners.insert("current".into(), current.clone());
        store.owners.insert("foreign".into(), foreign.clone());
        for (scope, name) in [
            (&current, "current"),
            (&peer, "peer"),
            (&context, "work"),
            (&foreign, "foreign"),
        ] {
            store.record(None, closed(scope, name), 100);
        }
        assert_eq!(
            store
                .list(&current, false)
                .iter()
                .map(|entry| entry.title.as_str())
                .collect::<Vec<_>>(),
            ["current", "peer"]
        );
        assert_eq!(store.list(&context, false)[0].title, "work");
        assert_eq!(store.list(&current, true).len(), 3);
        let mut wrong_privacy = current;
        wrong_privacy.privacy = PrivacyKind::Normal;
        assert!(store.list(&wrong_privacy, true).is_empty());
    }

    #[test]
    fn undo_survives_one_owner_but_is_erased_after_the_final_session_owner() {
        let current = scope(PrivacyKind::Private);
        let foreign = scope(PrivacyKind::Private);
        let mut store = Store::default();
        store.owners.insert("first".into(), current.clone());
        store.owners.insert("second".into(), current.clone());
        store.owners.insert("foreign".into(), foreign.clone());
        store.record(None, closed(&current, "current"), 100);
        store.record(None, closed(&foreign, "foreign"), 100);
        store.release("first");
        assert_eq!(store.list(&current, false).len(), 1);
        store.release("second");
        assert!(store.list(&current, true).is_empty());
        store.record(None, closed(&current, "retired primary"), 100);
        assert!(store.list(&current, true).is_empty());
        assert_eq!(store.list(&foreign, true).len(), 1);
    }

    #[test]
    fn staged_page_scope_applies_until_its_actual_view_is_registered() {
        let current = scope(PrivacyKind::Ephemeral);
        let page = TabId::from_display("tabid-1988").unwrap();
        let mut owner = BrowserUiRust::default();
        let restored_group = Uuid::new_v4().to_string();
        owner.restored_page_windows.insert(
            page,
            crate::browser_ui_page_sessions::RestoredPageWindow {
                context: Some("work".into()),
                group: restored_group.clone(),
            },
        );
        let staged = current.clone().with_page_metadata(&owner, page);
        assert_eq!(staged.context.as_deref(), Some("work"));
        assert_eq!(staged.group, restored_group);
        assert!(staged.same_profile(&current));
        owner
            .interaction
            .prompt_view_identities
            .insert(page, "actual native view".into());
        assert_eq!(current.clone().with_page_metadata(&owner, page), current);
    }

    #[test]
    fn a_reentrant_reopen_cannot_consume_the_same_descriptor_twice() {
        let current = scope(PrivacyKind::Ephemeral);
        let mut store = Store::default();
        store.owners.insert("current".into(), current.clone());
        let entry = closed(&current, "reopen");
        let id = entry.id;
        store.record(None, entry, 100);
        assert!(store.claim(id, &current));
        assert!(!store.claim(id, &current));
        assert!(store.list(&current, true).is_empty());
        store.claims.remove(&(current.profile, id));
        assert_eq!(store.list(&current, false).len(), 1);
        assert!(store.claim(id, &current));
    }

    #[test]
    fn undo_deduplicates_close_acknowledgements_and_bounds_the_whole_profile() {
        let current = scope(PrivacyKind::Ephemeral);
        let page = TabId::from_display("tabid-1987").unwrap();
        let mut store = Store::default();
        store.owners.insert("current".into(), current.clone());
        store.record(Some(page), closed(&current, "first"), 2);
        store.record(Some(page), closed(&current, "duplicate"), 2);
        assert_eq!(store.list(&current, true).len(), 1);
        let mut other = current.clone();
        other.context = Some("work".into());
        store.record(None, closed(&other, "work"), 2);
        store.record(None, closed(&current, "last"), 2);
        assert_eq!(
            store
                .list(&current, true)
                .iter()
                .map(|entry| entry.title.as_str())
                .collect::<Vec<_>>(),
            ["last", "work"]
        );
        let mut unsafe_entry = closed(&current, "unsafe");
        unsafe_entry.url = "javascript:alert(1)".into();
        store.record(None, unsafe_entry, 2);
        assert_eq!(store.list(&current, true).len(), 2);
        store.record(None, closed(&current, "disabled"), 0);
        assert!(store.list(&current, true).is_empty());
    }
}
