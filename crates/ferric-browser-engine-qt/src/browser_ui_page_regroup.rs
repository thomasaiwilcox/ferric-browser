//! Detach/give change a single-page window's logical group without moving its view.
use crate::browser_ui_page_windows::{
    PageContinuation, PageHandoff, PageWindowSnapshot, continuation_current,
};
use crate::{CxxQtType, ParsedCommand, Pin, PublicError, QString, Value, qobject};
use cxx_qt::Threading;

pub(super) fn handles(command: &ParsedCommand) -> bool {
    matches!(command.name.as_str(), "tab-detach" | "tab-give")
}

fn single_page_owner(ui: &qobject::BrowserUi, page: crate::TabId) -> bool {
    let rust = ui.rust();
    rust.tab_ids.as_slice() == [page]
        && rust
            .state
            .as_ref()
            .and_then(|state| state.windows().get(&rust.window?))
            .is_some_and(|window| window.tabs.as_slice() == [page])
        && PageWindowSnapshot::capture(ui)
            .is_some_and(|window| window.pages.len() == 1 && window.pages[0].id == page)
}

impl qobject::BrowserUi {
    pub(super) fn execute_page_window_ipc_regroup(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &crate::IpcRoute,
        commands: Vec<(ParsedCommand, crate::IpcRoute)>,
        operation: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<Value, PublicError> {
        if route.context.is_some() {
            return Err("context routing is not valid for page group changes".into());
        }
        self.as_ref().validate_ipc_route_public(route)?;
        if Some(self.as_ref().ipc_route_window(route)?) != self.as_ref().rust().window {
            return Err("Choose the page group in its issuing window".into());
        }
        self.as_mut()
            .execute_page_window_regroup(
                command,
                PageContinuation::Ipc {
                    commands,
                    route: route.clone(),
                },
                operation,
                handoff,
            )
            .map_err(Into::into)
    }

    pub(super) fn execute_page_window_regroup(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        continuation: PageContinuation,
        operation: Option<&str>,
        incoming: &mut Option<PageHandoff>,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let source = PageWindowSnapshot::capture_checked(self.as_ref().get_ref())?;
        if source.pages.len() != 1
            || !single_page_owner(self.as_ref().get_ref(), source.pages[0].id)
        {
            return Err("This window contains several live pages. Detach and Give currently require one page per window.".into());
        }
        let document = source.pages[0].target;
        self.as_mut().refresh_ipc_owner();
        let owners = crate::live_ipc_owners::page_window_owners(&source.scope);
        let destination = match (command.name.as_str(), command.arguments.as_slice()) {
            ("tab-detach", []) => None,
            ("tab-give", [id]) => {
                let window = crate::WindowId::from_display(id).ok_or("Use a live window ID")?;
                if Some(window) == self.as_ref().rust().window {
                    return Err("Choose a different window for tab-give".into());
                }
                Some(
                    owners
                        .iter()
                        .find(|owner| owner.window == window)
                        .cloned()
                        .ok_or("Choose a ready window in the same profile, privacy and context")?,
                )
            }
            _ => return Err("Use tab-detach or tab-give WINDOW_ID".into()),
        };
        let group = destination.as_ref().map_or_else(
            || {
                if owners
                    .iter()
                    .filter(|owner| owner.snapshot.group == source.group)
                    .map(|owner| owner.snapshot.pages.len())
                    .sum::<usize>()
                    > 1
                {
                    crate::Uuid::new_v4().to_string()
                } else {
                    source.group.clone()
                }
            },
            |owner| owner.snapshot.group.clone(),
        );
        let handoff = self
            .as_mut()
            .prepare_page_handoff(incoming.clone(), &source, operation)?;
        let thread = self.as_ref().get_ref().qt_thread();
        let token = self.as_ref().rust().window_token.to_string();
        let mut result = serde_json::json!({"status":"accepted", "action":command.name, "pending":true,
            "preserves_live_state":true, "tab_id":document.tab.to_string(), "group":group});
        if let Some(destination) = &destination {
            result["target_window"] = serde_json::json!(destination.window.to_string());
        }
        let expected_destination = destination.clone();
        let resume = move |valid: bool| {
            let _ = thread.queue(move |mut ui| {
                let mut handoff = Some(handoff);
                let source_current = single_page_owner(ui.as_ref().get_ref(), document.tab)
                    && continuation_current(
                        ui.as_ref().get_ref(),
                        &source.scope,
                        &source.group,
                        document,
                    );
                let destination_current = expected_destination.as_ref().is_none_or(|expected| {
                    crate::live_ipc_owners::page_window_owners(&source.scope)
                        .iter()
                        .any(|owner| {
                            owner.token == expected.token
                                && owner.window == expected.window
                                && owner.snapshot.group == expected.snapshot.group
                                && owner.snapshot.pages.iter().any(|page| {
                                    expected
                                        .snapshot
                                        .pages
                                        .iter()
                                        .any(|saved| saved.target == page.target)
                                })
                        })
                });
                let mut valid = valid && source_current && destination_current;
                if valid {
                    valid = handoff
                        .as_ref()
                        .is_some_and(|receipt| receipt.authorize_regroup(&source, &group));
                }
                if valid {
                    {
                        let mut rust = ui.as_mut().rust_mut();
                        let this = rust.as_mut().get_mut();
                        this.page_window_group.clone_from(&group);
                        this.library.switcher_cache = None;
                        this.checkpoint
                            .mark_dirty(crate::elapsed_ms(this.binding_clock));
                    }
                    ui.as_mut().refresh_ipc_owner();
                    if let Some(scope) = crate::page_window_undo::Scope::capture(ui.as_ref().rust())
                    {
                        let pinned = ui
                            .as_ref()
                            .rust()
                            .state
                            .as_ref()
                            .and_then(|state| state.tabs().get(&document.tab))
                            .is_some_and(|page| page.pinned);
                        crate::live_ipc_owners::restore_closed_page_order(
                            &scope,
                            document.tab,
                            pinned,
                            usize::MAX,
                        );
                    }
                    ui.as_mut()
                        .set_status_text(QString::from("Page group updated"));
                    ui.as_mut().sync_macro_status_text();
                    valid = continuation_current(
                        ui.as_ref().get_ref(),
                        &source.scope,
                        &group,
                        document,
                    );
                }
                let succeeded = valid
                    && match continuation {
                        PageContinuation::Interactive { commands, count } => {
                            ui.as_mut().execute_counted_parsed_commands_with_handoff(
                                commands,
                                count,
                                &mut handoff,
                            )
                        }
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
                    handoff.finish(valid, succeeded, token);
                }
            });
        };
        if let Some(destination) = destination {
            let target_thread = destination.thread.clone();
            target_thread
                .queue(move |ui| {
                    let valid =
                        PageWindowSnapshot::capture(ui.as_ref().get_ref()).is_some_and(|current| {
                            ui.as_ref().rust().window == Some(destination.window)
                                && current.scope == destination.snapshot.scope
                                && current.group == destination.snapshot.group
                                && current.pages.iter().any(|page| {
                                    destination
                                        .snapshot
                                        .pages
                                        .iter()
                                        .any(|saved| saved.target == page.target)
                                })
                        });
                    ui.refresh_ipc_owner();
                    resume(valid);
                })
                .map_err(|_| "The destination window closed".to_owned())?;
        } else {
            resume(true);
        }
        incoming.take();
        Ok(result)
    }
}
