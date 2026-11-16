//! Incremental raw Core facts. Parsing and whole-owner capture are cold work.
use super::ProfileContext;
use crate::{BrowserApplication, configuration::CapturedInteractionContexts};
use ferric_browser_core::{Event, ExistenceState, ProfileId, TabId, TabState, WindowId};
use ferric_browser_runtime::{RuntimeCommand, RuntimeEffect, RuntimeInput, RuntimeObservation};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Eq, PartialEq)]
pub(super) struct ContextFact {
    window: WindowId,
    profile: ProfileId,
    url: String,
}

impl ContextFact {
    fn matches(&self, page: &TabState) -> bool {
        self.window == page.window
            && self.profile == page.profile
            && Some(self.url.as_str()) == page.url.as_deref()
    }
}

pub(super) struct ContextStore {
    pub(super) epoch: Arc<()>,
    coherent: bool,
    facts: BTreeMap<TabId, Arc<ContextFact>>,
}

pub(super) struct ContextCapture {
    pub(super) epoch: Arc<()>,
    coherent: bool,
    facts: Vec<(TabId, Arc<ContextFact>)>,
}

impl ContextCapture {
    pub(super) fn compile_inputs(&self) -> CapturedInteractionContexts {
        CapturedInteractionContexts::from_raw_pages(
            self.coherent,
            self.facts.iter().map(|(id, fact)| (*id, fact.url.as_str())),
        )
    }
}

impl ContextStore {
    pub(super) fn unavailable() -> Self {
        Self {
            epoch: Arc::new(()),
            coherent: false,
            facts: BTreeMap::new(),
        }
    }

    pub(super) fn capture(app: &BrowserApplication, profile: Option<&ProfileContext>) -> Self {
        let scope = profile.and_then(|profile| profile.scope);
        let facts = app
            .tabs()
            .values()
            .filter(|page| relevant(page, scope))
            .filter_map(|page| {
                page.url.as_ref().map(|url| {
                    (
                        page.id,
                        Arc::new(ContextFact {
                            window: page.window,
                            profile: page.profile,
                            url: url.clone(),
                        }),
                    )
                })
            })
            .collect();
        Self {
            epoch: Arc::new(()),
            coherent: CapturedInteractionContexts::membership_is_coherent(app, scope),
            facts,
        }
    }

    pub(super) fn snapshot(&self) -> ContextCapture {
        ContextCapture {
            epoch: Arc::clone(&self.epoch),
            coherent: self.coherent,
            facts: self
                .facts
                .iter()
                .map(|(id, fact)| (*id, Arc::clone(fact)))
                .collect(),
        }
    }

    fn update(&mut self, app: &BrowserApplication, id: TabId, scope: Option<ProfileId>) -> bool {
        let current = app
            .tabs()
            .get(&id)
            .filter(|page| relevant(page, scope) && page.url.is_some());
        match (self.facts.get(&id), current) {
            (Some(old), Some(page)) if old.matches(page) => false,
            (_, Some(page)) => {
                self.facts.insert(
                    id,
                    Arc::new(ContextFact {
                        window: page.window,
                        profile: page.profile,
                        url: page.url.as_ref().expect("URL checked above").clone(),
                    }),
                );
                true
            }
            (Some(_), None) => {
                self.facts.remove(&id);
                true
            }
            (None, None) => false,
        }
    }

    /// Only actual accepted observations are used. Stale events have no such
    /// observation; accepted no-ops compare equal without allocation or epochs.
    pub(super) fn apply(
        &mut self,
        app: &BrowserApplication,
        profile: Option<&ProfileContext>,
        effects: &[RuntimeEffect],
    ) {
        let scope = profile.and_then(|profile| profile.scope);
        let mut changed = false;
        for effect in effects {
            let RuntimeEffect::Observed(observation) = effect else {
                continue;
            };
            if !context_event(&observation.event) {
                continue;
            }
            changed |= self.apply_observation(app, scope, observation);
        }
        if changed {
            self.epoch = Arc::new(());
        }
    }

    fn apply_observation(
        &mut self,
        app: &BrowserApplication,
        scope: Option<ProfileId>,
        observation: &RuntimeObservation,
    ) -> bool {
        let mut changed = false;
        // Removed pages are absent from TabObservation, so use their event ID.
        let removed = match &observation.event {
            Event::CloseTab { tab }
            | Event::TransferTabOut { tab }
            | Event::ViewClosed { tab, .. } => Some(*tab),
            _ => None,
        };
        if let Some(id) = removed {
            // Closing a last tab can create a browser start page. Its address
            // is not HTTP, but keep the actual replacement without a tab scan.
            let window = self.facts.get(&id).map(|fact| fact.window);
            changed |= self.update(app, id, scope);
            if let Some(next) = window
                .and_then(|id| app.windows().get(&id))
                .and_then(|window| window.active_tab)
            {
                changed |= self.update(app, next, scope);
            }
        }
        if let Some(page) = &observation.tab {
            changed |= self.update(app, page.id, scope);
        }
        match &observation.event {
            Event::TransferTabIn { window, .. } => {
                // An identity-preserving import can replace an unused, URL-less
                // placeholder. Such a placeholder contributes no stored fact.
                if let Some(tab) = app
                    .windows()
                    .get(window)
                    .and_then(|window| window.active_tab)
                {
                    changed |= self.update(app, tab, scope);
                }
            }
            Event::OpenPopup { opener, .. } => {
                if let Some(tab) = app
                    .tabs()
                    .get(&opener.tab)
                    .and_then(|page| app.windows().get(&page.window))
                    .and_then(|window| window.tabs.last())
                {
                    changed |= self.update(app, *tab, scope);
                }
            }
            _ => {}
        }
        changed
    }
}

fn relevant(page: &TabState, scope: Option<ProfileId>) -> bool {
    scope.is_none_or(|scope| page.profile == scope)
        && matches!(
            page.existence,
            ExistenceState::Constructing | ExistenceState::Live
        )
}

fn context_event(event: &Event) -> bool {
    matches!(
        event,
        Event::CreateProfile { .. }
            | Event::CreateWindow { .. }
            | Event::OpenTab { .. }
            | Event::OpenTabWithNavigation { .. }
            | Event::OpenPopup { .. }
            | Event::MoveTab { .. }
            | Event::TransferTabOut { .. }
            | Event::TransferTabIn { .. }
            | Event::StartNavigation { .. }
            | Event::RestoreTabAddress { .. }
            | Event::CommitNavigation { .. }
            | Event::CommitNavigationWithTransition { .. }
            | Event::CommitNavigationAtVisit { .. }
            | Event::SameDocumentNavigation { .. }
            | Event::CloseTab { .. }
            | Event::ViewClosed { .. }
            | Event::SuspendTab { .. }
            | Event::DiscardTab { .. }
            | Event::ResumeTab { .. }
            | Event::RendererTerminated { .. }
            | Event::RequestShutdown
    )
}

pub(super) fn may_change_context(input: &RuntimeInput) -> bool {
    match input {
        RuntimeInput::Command(
            RuntimeCommand::EnterMode { .. } | RuntimeCommand::LeaveMode { .. },
        )
        | RuntimeInput::WorkerCompletion { .. } => false,
        RuntimeInput::Command(_) => true,
        RuntimeInput::EngineFact(event)
        | RuntimeInput::Lifecycle(event)
        | RuntimeInput::UserDecision(event) => context_event(event),
    }
}
