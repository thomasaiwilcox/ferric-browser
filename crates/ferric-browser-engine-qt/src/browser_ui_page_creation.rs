//! Creation owns an explicit new-page transition. Its first document authority
//! and original operation receipt are installed before either owner publishes.
use crate::browser_ui_page_windows::{
    PageAnchor, PageContinuation, PageHandoff, PageScope, PageWindowSnapshot, continuation_current,
};
use crate::{CxxQtType, ParsedCommand, Pin, QString, TabId, Target, qobject};
use cxx_qt::Threading;
use ferric_browser_application::NativeOperationOutcome;
use std::time::{Duration, Instant};

pub(super) struct CreationPresentation {
    target: Target,
    scope: PageScope,
    group: String,
    generation: u64,
    terminal_focus: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

pub(super) struct CreationPublication {
    window: crate::WindowId,
    owner: QString,
    target: Target,
    scope: PageScope,
    group: String,
    effects: Vec<crate::RuntimeEffect>,
    command: ParsedCommand,
    route: Option<crate::IpcRoute>,
    count: u32,
}

pub(super) struct PendingCreation {
    scope: PageScope,
    group: String,
    issuer: Target,
    issuer_view: String,
    pub(super) expected: Target,
    pub(super) foreground: bool,
    pub(super) awaiting_prepared_navigation: bool,
    committed: bool,
    initial_url: crate::ValidatedUrl,
    navigation_key: Option<String>,
    view: Option<String>,
    load_receipt: Option<String>,
    expires: Instant,
    continuation: Option<PageContinuation>,
    handoff: Option<PageHandoff>,
}
impl Drop for PendingCreation {
    fn drop(&mut self) {
        if let Some(handoff) = self.handoff.take() {
            handoff.owner_lost();
        }
    }
}

pub(super) fn handles(command: &ParsedCommand, route: Option<&crate::IpcRoute>) -> bool {
    command.name == "tab-open"
        || command.name == "open"
            && !command.arguments.iter().any(|arg| arg == "--clean-link")
            && route.is_some_and(|route| {
                matches!(
                    route.open_target,
                    crate::IpcOpenTarget::Tab | crate::IpcOpenTarget::BackgroundTab
                )
            })
}

impl qobject::BrowserUi {
    pub(super) fn execute_page_window_ipc_creation(
        self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &crate::IpcRoute,
        commands: Vec<(ParsedCommand, crate::IpcRoute)>,
        operation: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<crate::Value, crate::PublicError> {
        let mut continuation_route = route.clone();
        continuation_route.selector = crate::DispatchTarget::Active;
        let commands = commands
            .into_iter()
            .map(|(command, mut route)| {
                route.selector = crate::DispatchTarget::Active;
                (command, route)
            })
            .collect();
        self.execute_page_window_creation(
            command,
            1,
            Some(route),
            PageContinuation::Ipc {
                commands,
                route: continuation_route,
            },
            operation,
            handoff,
        )
        .map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn execute_page_window_creation(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        count: u32,
        route: Option<&crate::IpcRoute>,
        continuation: PageContinuation,
        operation: Option<&str>,
        incoming: &mut Option<PageHandoff>,
    ) -> Result<crate::Value, String> {
        if command.name == "open"
            && route.is_some_and(|route| self.as_ref().creation_requires_routing(command, route))
        {
            if continuation.has_commands() || incoming.is_some() {
                return Err(
                    "Complete the context-routed open separately before chaining page commands"
                        .into(),
                );
            }
            return self
                .execute_adapter_ipc_command(
                    command.clone(),
                    route.expect("routed open"),
                    operation,
                )
                .map_err(|error| error.to_string());
        }
        if count != 1 {
            return Err("Page opening does not accept a count".into());
        }
        let source = PageWindowSnapshot::capture_checked(self.as_ref().get_ref())?;
        let issuer = source
            .pages
            .iter()
            .find(|page| Some(page.id) == source.selected)
            .ok_or("The issuing page is unavailable")?
            .clone();
        if self.as_ref().rust().page_creations.len() >= 32 {
            return Err("Too many pages are still opening. Try again shortly.".into());
        }
        if let Some(route) = route {
            if route.context.is_some() || route.profile.is_some() {
                return Err("Open grouped pages without context or profile routing".into());
            }
            self.as_ref()
                .validate_ipc_route_public(route)
                .map_err(|e| e.to_string())?;
            if Some(
                self.as_ref()
                    .ipc_route_window(route)
                    .map_err(|e| e.to_string())?,
            ) != self.as_ref().rust().window
            {
                return Err("Open pages in their issuing window".into());
            }
        }
        let command = if command.name == "open" {
            let [input] = command.arguments.as_slice() else {
                return Err("Open requires one input".into());
            };
            crate::browser_ui_ipc_runtime_dispatch::planned_tab_open_command(
                input,
                route.is_none_or(|route| route.open_target != crate::IpcOpenTarget::BackgroundTab),
            )
        } else {
            command.clone()
        };
        let foreground = !command
            .arguments
            .iter()
            .take_while(|arg| arg.as_str() != "--")
            .any(|arg| arg == "--background");
        let dispatch = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let state = rust.state.as_ref().ok_or("Core state unavailable")?;
            crate::RuntimeDispatch {
                invocation: route.map_or_else(
                    || crate::CommandInvocation::with_count(command.clone(), count),
                    |route| {
                        crate::ipc_command_invocation(
                            state,
                            command.clone(),
                            route.selector,
                            route.source,
                            operation,
                        )
                    },
                ),
                navigation: route.map_or_else(
                    || binding.navigation_context(),
                    |route| binding.navigation_context_for_source(route.source),
                ),
                target: route.map_or(crate::DispatchTarget::Active, |route| route.selector),
            }
        };
        let handoff = self
            .as_mut()
            .prepare_page_handoff(incoming.clone(), &source, operation)?;
        if !handoff.can_create_from(&source) {
            return Err("The issuing operation changed before opening the page".into());
        }
        let effects = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .state
                .as_mut()
                .expect("captured state")
                .dispatch_runtime(crate::RuntimeInput::Command(
                    crate::RuntimeCommand::Dispatch(Box::new(dispatch)),
                ))
        };
        let effects = match effects {
            Ok(effects) => effects,
            Err(error) => {
                handoff.finish_close(NativeOperationOutcome::Failed);
                return Err(error.to_string());
            }
        };
        crate::browser_ui_page_get::observe(self.as_mut().rust_mut().as_mut().get_mut(), &effects);
        crate::live_ipc_owners::commit_owner_session(self.as_ref().get_ref());
        let created = effects.iter().find_map(|effect| match effect {
            crate::RuntimeEffect::Engine(crate::EngineEffect::Navigate { target, url })
                if !source.reserved.contains(&target.tab) =>
            {
                Some((*target, url.clone()))
            }
            _ => None,
        });
        let Some((target, initial_url)) = created else {
            handoff.finish_close(NativeOperationOutcome::Failed);
            return Err("The new page did not receive a navigation".into());
        };
        if foreground {
            let anchor = PageAnchor::prepared(
                self.as_ref().get_ref(),
                source.scope.clone(),
                source.group.clone(),
                target.tab,
            )
            .expect("pure creation retains its captured owner");
            // No Qt or worker callback runs between the preflight and this commit.
            if !handoff.accept_created_anchor(&source, anchor) {
                handoff.finish_close(NativeOperationOutcome::Stale);
                self.as_mut()
                    .queue_creation_publication(target, effects, command, route, count);
                incoming.take();
                return Err("The new page is open, but its command could not continue".into());
            }
            handoff.request_focus();
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_creations
            .insert(
                target.tab,
                Box::new(PendingCreation {
                    scope: source.scope,
                    group: source.group,
                    issuer: issuer.target,
                    issuer_view: issuer.view_identity,
                    expected: target,
                    foreground,
                    awaiting_prepared_navigation: false,
                    committed: false,
                    navigation_key: crate::canonical_engine_url(QString::from(
                        initial_url.as_str(),
                    ))
                    .ok(),
                    initial_url,
                    view: None,
                    load_receipt: None,
                    expires: Instant::now() + Duration::from_secs(60),
                    continuation: Some(continuation),
                    handoff: Some(handoff),
                }),
            );
        incoming.take();
        // Queue publication after this Rust reference and the issuing command
        // stack unwind. The native publisher blocks signals, then guards every
        // notification, including actual synchronous issuer destruction.
        self.as_mut()
            .queue_creation_publication(target, effects, command, route, count);
        Ok(serde_json::json!({"status":"accepted", "pending":true,
            "tab_id":target.tab.to_string()}))
    }

    fn creation_requires_routing(&self, command: &ParsedCommand, route: &crate::IpcRoute) -> bool {
        if route.context.is_some() || route.profile.is_some() {
            return true;
        }
        if self.active_profile_is_transient() {
            return false;
        }
        let Some(input) = command.arguments.first() else {
            return false;
        };
        let Ok(result) =
            crate::resolve_input(input, &self.navigation_context_for_source(route.source))
        else {
            return false;
        };
        if !matches!(
            result.source,
            crate::NavigationSource::ExplicitUrl | crate::NavigationSource::Host
        ) {
            return false;
        }
        let Ok(contexts) =
            serde_json::from_str::<crate::ContextsConfig>(&self.rust().contexts_json.to_string())
        else {
            return false;
        };
        crate::matching_context_routes(
            &contexts,
            result.url.as_str(),
            if route.external_open {
                "external-open"
            } else {
                "explicit-open"
            },
        )
        .iter()
        .any(|route| {
            contexts
                .contexts
                .iter()
                .any(|context| context.name == route.context)
        })
    }

    fn queue_creation_publication(
        mut self: Pin<&mut Self>,
        target: Target,
        effects: Vec<crate::RuntimeEffect>,
        command: ParsedCommand,
        route: Option<&crate::IpcRoute>,
        count: u32,
    ) {
        let snapshot =
            PageWindowSnapshot::capture_checked(self.as_ref().get_ref()).expect("captured source");
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.page_creation_publications
            .push_back(CreationPublication {
                window: this.window.expect("captured owner"),
                owner: this.window_token.clone(),
                target,
                scope: snapshot.scope,
                group: snapshot.group,
                effects,
                command,
                route: route.cloned(),
                count,
            });
        drop(rust);
        qobject::ferric_queue_page_creation_publication(self.as_ref().get_ref());
    }

    // Called only under the native publisher's signal blocker.
    pub(super) fn publish_page_creation(mut self: Pin<&mut Self>) -> bool {
        let publication = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_creation_publications
            .pop_front();
        let Some(publication) = publication else {
            return false;
        };
        let valid = self.as_ref().rust().window == Some(publication.window)
            && self.as_ref().rust().window_token == publication.owner
            && PageWindowSnapshot::capture(self.as_ref().get_ref()).is_some_and(|snapshot| {
                snapshot.scope == publication.scope && snapshot.group == publication.group
            })
            && !self.as_ref().rust().profile_bootstrap_pending
            && !self.as_ref().rust().ipc_shutdown_gate
            && self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(|state| state.capture_target(publication.target.tab))
                == Some(publication.target);
        if valid {
            let effects = self.as_mut().observe_runtime_effects(publication.effects);
            self.as_mut().sync_tab_order_from_core();
            self.as_mut().set_pending_engine_action(&effects);
            self.as_mut().note_repeatable_invocation(
                &publication.command,
                publication.route.as_ref(),
                publication.count,
            );
        } else {
            if let Some(pending) = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .page_creations
                .remove(&publication.target.tab)
            {
                fail(pending, NativeOperationOutcome::Stale);
            }
            self.as_mut().sync_tab_order_from_core();
        }
        true
    }

    // Native commit calls this before either owner is observable. A background
    // tail stays with the issuer; a foreground tail and anchor travel together.
    pub(super) fn move_page_creation(
        mut self: Pin<&mut Self>,
        mut child: Pin<&mut Self>,
        tab: TabId,
    ) {
        let Some(mut pending) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_creations
            .remove(&tab)
        else {
            return;
        };
        if !pending.foreground {
            pending.committed = true;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .page_creations
                .insert(tab, pending);
            return;
        }
        let destination = PageAnchor::prepared(
            child.as_ref().get_ref(),
            pending.scope.clone(),
            pending.group.clone(),
            tab,
        );
        let moved = destination.is_some_and(|destination| {
            pending.handoff.as_ref().is_some_and(|handoff| {
                handoff.move_created_anchor(self.as_ref().get_ref(), destination)
            })
        });
        if !moved {
            return;
        }
        pending.awaiting_prepared_navigation = true;
        pending.committed = true;
        let mut rust = child.as_mut().rust_mut();
        let child = rust.as_mut().get_mut();
        child.page_creation_focus_owned = true;
        child.page_creation_presentation = Some(CreationPresentation {
            target: pending.expected,
            scope: pending.scope.clone(),
            group: pending.group.clone(),
            generation: pending
                .handoff
                .as_ref()
                .expect("owned handoff")
                .focus_generation(),
            terminal_focus: pending
                .handoff
                .as_ref()
                .expect("owned handoff")
                .terminal_focus_guard(),
        });
        child.page_creations.insert(tab, pending);
    }

    pub(super) fn page_creation_controls_focus(&self) -> bool {
        self.rust().page_creation_focus_owned
    }

    pub(super) fn created_page_presentation_intent(&self) -> QString {
        QString::from(
            self.rust()
                .page_creation_presentation
                .as_ref()
                .filter(|presentation| {
                    !presentation
                        .terminal_focus
                        .load(std::sync::atomic::Ordering::Acquire)
                })
                .map_or_else(String::new, |presentation| {
                    presentation.generation.to_string()
                }),
        )
    }

    // The guarded native publisher consumes this immediately before show().
    // Loading/error presentation has no authority to run document commands.
    pub(super) fn take_created_page_presentation(mut self: Pin<&mut Self>) -> bool {
        let presentation = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_creation_presentation
            .take();
        presentation.is_some_and(|presentation| {
            !presentation
                .terminal_focus
                .load(std::sync::atomic::Ordering::Acquire)
                && crate::page_window_focus::current(presentation.generation)
                && continuation_current(
                    self.as_ref().get_ref(),
                    &presentation.scope,
                    &presentation.group,
                    presentation.target,
                )
        })
    }

    pub(super) fn bind_prepared_creation(mut self: Pin<&mut Self>, old: Target, new: Target) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if let Some(presentation) = &mut this.page_creation_presentation
            && presentation.target == old
        {
            presentation.target = new;
        }
        if let Some(pending) = this.page_creations.get_mut(&old.tab)
            && pending.foreground
            && pending.awaiting_prepared_navigation
            && pending.expected == old
            && old.tab == new.tab
        {
            pending.expected = new;
            pending.awaiting_prepared_navigation = false;
        }
    }

    pub(super) fn bind_creation_view(mut self: Pin<&mut Self>, tab: TabId, identity: &str) {
        let mut rust = self.as_mut().rust_mut();
        if let Some(pending) = rust.as_mut().get_mut().page_creations.get_mut(&tab)
            && pending.foreground
            && pending.view.is_none()
        {
            pending.view = Some(identity.to_owned());
        }
    }

    pub(super) fn begin_page_creation_load(
        mut self: Pin<&mut Self>,
        index: i32,
        identity: &QString,
        url: &QString,
    ) -> QString {
        let Some(target) = self.as_ref().target_for_index(index) else {
            return QString::default();
        };
        let identity = identity.to_string();
        let valid = self
            .as_ref()
            .rust()
            .page_creations
            .get(&target.tab)
            .is_some_and(|pending| {
                pending.foreground
                    && !pending.awaiting_prepared_navigation
                    && pending.expected == target
                    && pending.view.as_deref() == Some(&identity)
                    && pending.load_receipt.is_none()
                    && pending.navigation_key.is_some()
                    && crate::canonical_engine_url(url.clone()).ok() == pending.navigation_key
            })
            && self
                .as_ref()
                .rust()
                .interaction
                .prompt_view_identities
                .get(&target.tab)
                == Some(&identity);
        if !valid {
            return QString::default();
        }
        let receipt = crate::Uuid::new_v4().to_string();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_creations
            .get_mut(&target.tab)
            .expect("captured pending navigation")
            .load_receipt = Some(receipt.clone());
        QString::from(receipt)
    }

    pub(super) fn complete_page_creation_load(
        mut self: Pin<&mut Self>,
        receipt: &QString,
        identity: &QString,
        success: bool,
    ) {
        let receipt = receipt.to_string();
        if receipt.is_empty() {
            return;
        }
        let tab = self
            .as_ref()
            .rust()
            .page_creations
            .iter()
            .find_map(|(tab, pending)| {
                (pending.load_receipt.as_deref() == Some(&receipt)
                    && pending.view.as_deref() == Some(&identity.to_string()))
                .then_some(*tab)
            });
        let Some(tab) = tab else {
            return;
        };
        let pending = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_creations
            .remove(&tab)
            .expect("receipt");
        let target = pending.expected;
        let view = pending.view.clone().expect("bound view");
        if success {
            self.queue_creation_tail(pending, target, view);
        } else {
            fail(pending, NativeOperationOutcome::Failed);
        }
    }

    pub(super) fn page_creation_internal_ready(
        mut self: Pin<&mut Self>,
        target: Target,
        url: &str,
    ) {
        let ready = self
            .as_ref()
            .rust()
            .page_creations
            .get(&target.tab)
            .is_some_and(|pending| {
                pending.foreground
                    && !pending.awaiting_prepared_navigation
                    && pending.expected == target
                    && pending.initial_url.as_str() == url
                    && pending.view.as_ref().is_some_and(|view| {
                        self.as_ref()
                            .rust()
                            .interaction
                            .prompt_view_identities
                            .get(&target.tab)
                            == Some(view)
                    })
            });
        if !ready {
            return;
        }
        let pending = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_creations
            .remove(&target.tab)
            .expect("internal page");
        let view = pending.view.clone().expect("bound view");
        self.queue_creation_tail(pending, target, view);
    }

    pub(super) fn cancel_page_creations(mut self: Pin<&mut Self>) {
        let pending =
            std::mem::take(&mut self.as_mut().rust_mut().as_mut().get_mut().page_creations);
        for (_, pending) in pending {
            fail(pending, NativeOperationOutcome::Stale);
        }
    }

    pub(super) fn poll_page_creations(mut self: Pin<&mut Self>) {
        let tabs = self
            .as_ref()
            .rust()
            .page_creations
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for tab in tabs {
            let (stale, background_ready) = {
                let binding = self.as_ref();
                let rust = binding.rust();
                let pending = &rust.page_creations[&tab];
                let stale = Instant::now() >= pending.expires
                    || rust.ipc_shutdown_gate
                    || rust.profile_bootstrap_pending
                    || pending.view.as_ref().is_some_and(|view| {
                        rust.interaction.prompt_view_identities.get(&tab) != Some(view)
                    })
                    || !pending.scope.matches_owner(binding.get_ref())
                    || if rust.page_window_group.is_empty() {
                        rust.window_token.to_string() != pending.group
                    } else {
                        rust.page_window_group != pending.group
                    }
                    || rust.state.as_ref().is_none_or(|state| {
                        state.shutdown() != ferric_browser_core::ShutdownState::Running
                            || (pending.foreground || !pending.committed)
                                && state.capture_target(tab) != Some(pending.expected)
                    });
                let ready = !pending.foreground
                    && (pending.committed
                        || rust.interaction.prompt_view_identities.contains_key(&tab));
                (stale, ready)
            };
            if stale || background_ready {
                let pending = self
                    .as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .page_creations
                    .remove(&tab)
                    .expect("listed");
                if stale {
                    fail(pending, NativeOperationOutcome::Stale);
                } else {
                    let target = pending.issuer;
                    let view = pending.issuer_view.clone();
                    self.as_mut().queue_creation_tail(pending, target, view);
                }
            }
        }
    }

    fn queue_creation_tail(
        self: Pin<&mut Self>,
        mut pending: Box<PendingCreation>,
        target: Target,
        view: String,
    ) {
        let _ = self.as_ref().get_ref().qt_thread().queue(move |mut ui| {
            let valid = Instant::now() < pending.expires
                && continuation_current(
                    ui.as_ref().get_ref(),
                    &pending.scope,
                    &pending.group,
                    target,
                )
                && ui
                    .as_ref()
                    .rust()
                    .interaction
                    .prompt_view_identities
                    .get(&target.tab)
                    == Some(&view);
            if !valid {
                fail(pending, NativeOperationOutcome::Stale);
                return;
            }
            let mut handoff = pending.handoff.take();
            let succeeded = match pending.continuation.take().expect("single tail") {
                PageContinuation::Interactive { commands, count } => ui
                    .as_mut()
                    .execute_counted_parsed_commands_with_handoff(commands, count, &mut handoff),
                PageContinuation::Ipc { commands, .. } => ui
                    .as_mut()
                    .execute_routed_ipc_chain_with_handoff(commands, None, &mut handoff)
                    .is_ok(),
                PageContinuation::Recorded { commands, .. } => ui
                    .as_mut()
                    .execute_page_recorded_commands(commands, None, &mut handoff)
                    .is_ok(),
            };
            ui.as_mut().runtime_work_available();
            if let Some(handoff) = handoff {
                handoff.finish(true, succeeded, ui.as_ref().get_ref());
            }
        });
    }
}
fn fail(mut pending: Box<PendingCreation>, outcome: NativeOperationOutcome) {
    if let Some(handoff) = pending.handoff.take() {
        handoff.finish_close(outcome);
    }
}
