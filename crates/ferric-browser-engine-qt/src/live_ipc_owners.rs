//! Native owner routing. The directory holds lifetime-safe Qt thread handles,
//! never `QObject` pointers or presentation-supplied command authority.

use super::qobject;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use ferric_browser_application::ContextCatalog;
use ferric_browser_core::WindowId;
use ferric_browser_ipc::{ErrorCode, PendingRequest, PublicError, Request, Response};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone)]
struct Owner<H = CxxQtThread<qobject::BrowserUi>> {
    token: String,
    window: Option<WindowId>,
    operations: Vec<String>,
    operation_revision: u64,
    thread: H,
    active: bool,
    focused: u64,
    order: u64,
    context_catalog: Option<(u64, Arc<ContextCatalog>)>,
}

struct Directory<H = CxxQtThread<qobject::BrowserUi>> {
    owners: BTreeMap<String, Owner<H>>,
    clock: u64,
    generation: u64,
    controller: Option<String>,
    shutdown: bool,
}

impl<H> Default for Directory<H> {
    fn default() -> Self {
        Self {
            owners: BTreeMap::new(),
            clock: 0,
            generation: 0,
            controller: None,
            shutdown: false,
        }
    }
}

fn directory() -> &'static Mutex<Directory> {
    static DIRECTORY: OnceLock<Mutex<Directory>> = OnceLock::new();
    DIRECTORY.get_or_init(Default::default)
}

pub(super) fn has_window_target(selector: &str) -> bool {
    directory()
        .lock()
        .ok()
        .and_then(|directory| directory.selected(Some(selector)))
        .is_some_and(|owner| owner.window.is_some())
}

pub(super) fn context_catalog_revision() -> u64 {
    directory().lock().map_or(0, |directory| directory.clock)
}

pub(super) struct OwnerLease(String);

impl Drop for OwnerLease {
    fn drop(&mut self) {
        if let Ok(mut directory) = directory().lock() {
            directory.owners.remove(&self.0);
            directory.generation = directory.generation.saturating_add(1);
            if directory.controller.as_deref() == Some(self.0.as_str()) {
                directory.controller = None;
                directory.shutdown = false;
            }
        }
    }
}

fn failure(request: &Request, code: ErrorCode, message: &str) -> Response {
    Response::failure(
        request.id.clone(),
        PublicError::new(code, message, message).into_protocol_error(),
    )
}

fn selector_error(message: &str) -> PublicError {
    PublicError::new(ErrorCode::InvalidArgument, message, message)
}

fn unavailable(request: &Request) -> Response {
    failure(
        request,
        ErrorCode::StaleTarget,
        "The requested browser window is no longer available. Refresh the window list.",
    )
}

fn missing_owner(request: &Request, selector: Option<&str>) -> Response {
    if request.method == "operations.cancel" {
        failure(
            request,
            ErrorCode::NotFound,
            "The requested operation is no longer available. Refresh the operation list.",
        )
    } else if matches!(selector, None | Some("active" | "last-focused")) {
        failure(
            request,
            ErrorCode::Busy,
            "No browser window is ready. Try again shortly.",
        )
    } else {
        unavailable(request)
    }
}

fn instance_shutdown(request: &Request) -> bool {
    super::is_mutating_ipc_method(&request.method)
        && directory()
            .lock()
            .ok()
            .is_none_or(|directory| directory.shutdown)
}

/// A queued callback can be discarded when Qt destroys its owner. Dropping the
/// delivery still resolves the authenticated request and its outstanding slot.
struct Reply(Option<PendingRequest>);

impl Reply {
    fn finish(mut self, response: &Response) {
        if let Some(pending) = self.0.take() {
            let _ = pending.respond(response);
        }
    }
}

struct QueryCollection {
    reply: Option<Reply>,
    request: Request,
    key: &'static str,
    remaining: usize,
    generation: u64,
    sequence: u64,
    responses: Vec<(u64, Response)>,
}

struct QueryPart {
    collection: Arc<Mutex<QueryCollection>>,
    request: Request,
    finished: bool,
    order: u64,
}

impl QueryPart {
    fn finish(mut self, response: Response) {
        self.finished = true;
        self.collect(response);
    }

    fn collect(&self, response: Response) {
        let completed = {
            let Ok(mut collection) = self.collection.lock() else {
                return;
            };
            collection.responses.push((self.order, response));
            collection.remaining -= 1;
            if collection.remaining != 0 {
                return;
            }
            collection.responses.sort_by_key(|(order, _)| *order);
            let response = if directory()
                .lock()
                .ok()
                .is_none_or(|directory| directory.generation != collection.generation)
            {
                failure(
                    &collection.request,
                    ErrorCode::Busy,
                    "The window list changed. Refresh it and try again.",
                )
            } else if let Some(response) = collection
                .responses
                .iter()
                .map(|(_, response)| response)
                .find(|response| response.error.is_some())
            {
                response.clone()
            } else {
                let mut rows = Vec::new();
                let mut registry = Vec::new();
                for (_, response) in &collection.responses {
                    if let Some(result) = response.result.as_ref() {
                        if let Some(part) = result.get(collection.key).and_then(Value::as_array) {
                            rows.extend(part.iter().cloned());
                        }
                        if let Some(part) = result.get("live_registry").and_then(Value::as_array) {
                            registry.extend(part.iter().cloned());
                        }
                    }
                }
                let mut result = json!({"sequence": collection.sequence});
                result[collection.key] = Value::Array(rows);
                if collection.key == "windows" {
                    result["live_registry"] = Value::Array(registry);
                }
                if collection.key == "operations" {
                    match super::operation_projection::page_operations(
                        result["operations"].as_array().cloned().unwrap_or_default(),
                        &collection.request.params,
                        collection.sequence,
                    ) {
                        Ok(page) => Response::success(collection.request.id.clone(), page),
                        Err(error) => {
                            failure(&collection.request, ErrorCode::InvalidArgument, &error)
                        }
                    }
                } else {
                    Response::success(collection.request.id.clone(), result)
                }
            };
            collection.reply.take().map(|reply| (reply, response))
        };
        if let Some((reply, response)) = completed {
            reply.finish(&response);
        }
    }
}

impl Drop for QueryPart {
    fn drop(&mut self) {
        if !self.finished {
            self.collect(unavailable(&self.request));
        }
    }
}

fn query_key(request: &Request) -> Option<&'static str> {
    match request.method.as_str() {
        "tabs.query"
            if request.params.get("window").is_none()
                && request.params.get("active_only").and_then(Value::as_bool) != Some(true) =>
        {
            Some("tabs")
        }
        "windows.query" if request.params.get("window").is_none() => Some("windows"),
        "operations.query" if request.params.get("window").is_none() => Some("operations"),
        "profiles.query" => Some("profiles"),
        _ => None,
    }
}

fn collect_query(pending: PendingRequest, key: &'static str, owners: Vec<Owner>, generation: u64) {
    let request = pending.request().clone();
    let collection = Arc::new(Mutex::new(QueryCollection {
        reply: Some(Reply(Some(pending))),
        request: request.clone(),
        key,
        remaining: owners.len(),
        generation,
        sequence: super::ipc_transport::current_sequence(),
        responses: Vec::new(),
    }));
    for owner in owners {
        let part = QueryPart {
            collection: Arc::clone(&collection),
            request: request.clone(),
            finished: false,
            order: owner.order,
        };
        let request = request.clone();
        let _ = owner.thread.queue(move |mut ui| {
            if ui.as_ref().rust().window != owner.window
                || !ui.as_ref().get_ref().profile_available()
                || ui.as_ref().rust().profile_bootstrap_pending
                || ui.as_ref().rust().ipc_owner.is_none()
            {
                part.finish(unavailable(&request));
                return;
            }
            ui.as_mut().refresh_operations();
            let response = if request.method == "operations.query" {
                match super::operation_projection::operations_collection_value(
                    ui.as_ref().rust(),
                    &request.params,
                ) {
                    Ok(result) => Response::success(request.id.clone(), result),
                    Err(error) => failure(&request, ErrorCode::InvalidArgument, &error),
                }
            } else {
                ui.as_mut().handle_ipc_request(&request)
            };
            part.finish(response);
            ui.as_mut().refresh_ipc_owner();
        });
    }
}

impl Drop for Reply {
    fn drop(&mut self) {
        if let Some(pending) = self.0.take() {
            let response = unavailable(pending.request());
            let _ = pending.respond(&response);
        }
    }
}

impl<H: Clone> Directory<H> {
    fn selected(&self, selector: Option<&str>) -> Option<Owner<H>> {
        match selector {
            Some("active") | None => self
                .owners
                .values()
                .filter(|owner| owner.active && owner.window.is_some())
                .max_by_key(|owner| owner.focused)
                .or_else(|| self.last_focused())
                .cloned(),
            Some("last-focused") => self.last_focused().cloned(),
            Some(selector) => {
                let window = WindowId::from_display(selector);
                let token = uuid::Uuid::parse_str(selector)
                    .ok()
                    .map(|id| id.to_string());
                let mut matches = self.owners.values().filter(|owner| {
                    token.as_deref() == Some(owner.token.as_str())
                        || (window.is_some() && owner.window == window)
                });
                let owner = matches.next()?;
                matches.next().is_none().then(|| owner.clone())
            }
        }
    }

    fn last_focused(&self) -> Option<&Owner<H>> {
        self.owners
            .values()
            .filter(|owner| owner.window.is_some())
            .max_by_key(|owner| (owner.focused, std::cmp::Reverse(owner.order)))
    }
}

fn selector(request: &Request) -> Result<Option<&str>, PublicError> {
    let value = match request.method.as_str() {
        "command.execute" | "action.execute" => request
            .params
            .get("context")
            .and_then(|context| context.get("window")),
        "tabs.query" | "windows.query" | "operations.query" => request.params.get("window"),
        _ => None,
    };
    let Some(value) = value else { return Ok(None) };
    let Some(value) = value.as_str() else {
        return Err(selector_error("The window selector must be a string."));
    };
    if !matches!(value, "active" | "last-focused")
        && WindowId::from_display(value).is_none()
        && uuid::Uuid::parse_str(value).is_err()
    {
        return Err(selector_error(
            "Choose active, last-focused, or an ID from the window list.",
        ));
    }
    Ok(Some(value))
}

fn target_request(mut request: Request, window: WindowId) -> Request {
    match request.method.as_str() {
        "command.execute" | "action.execute" => {
            if let Some(params) = request.params.as_object_mut() {
                let context = params.entry("context").or_insert_with(|| json!({}));
                if let Some(context) = context.as_object_mut() {
                    context.insert("window".into(), Value::String(window.to_string()));
                }
            }
        }
        "tabs.query" | "windows.query" | "operations.query" => {
            if let Some(params) = request.params.as_object_mut() {
                params.insert("window".into(), Value::String(window.to_string()));
            }
        }
        _ => {}
    }
    request
}

impl qobject::BrowserUi {
    pub(super) fn remember_context_catalog(
        self: Pin<&mut Self>,
        revision: u64,
        catalog: Arc<ContextCatalog>,
    ) {
        let rust = self.as_ref();
        let Some(lease) = rust.rust().ipc_owner.as_ref() else {
            return;
        };
        if let Ok(mut directory) = directory().lock()
            && let Some(owner) = directory.owners.get_mut(&lease.0)
        {
            owner.context_catalog = Some((revision, catalog));
        }
    }

    /// Reuse lifetime-safe native owner delivery for committed catalog updates.
    /// Application-owned durable revisions reject older commits, irrespective
    /// of the order in which native callbacks are delivered.
    pub(super) fn publish_context_catalog(mut self: Pin<&mut Self>) {
        let Some(catalog) = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(ferric_browser_application::BrowserApplication::context_catalog)
        else {
            return;
        };
        let token = self.as_ref().rust().window_token.to_string();
        let catalog = Arc::new(catalog);
        let Some((revision, owners)) = directory().lock().ok().map(|mut directory| {
            directory.clock = directory.clock.saturating_add(1);
            let revision = directory.clock;
            if let Some(owner) = directory.owners.get_mut(&token) {
                owner.context_catalog = Some((revision, Arc::clone(&catalog)));
            }
            (
                revision,
                directory
                    .owners
                    .values()
                    .filter(|owner| owner.token != token && owner.window.is_some())
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        }) else {
            return;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .context_catalog_revision = revision;
        for owner in owners {
            let catalog = Arc::clone(&catalog);
            let _ = owner.thread.queue(move |mut ui| {
                if ui.as_ref().rust().window == owner.window
                    && ui.as_ref().rust().ipc_owner.is_some()
                {
                    ui.as_mut().receive_context_catalog(catalog, revision);
                }
            });
        }
    }

    pub(super) fn register_ipc_controller(mut self: Pin<&mut Self>) {
        if !self.as_mut().register_ipc_owner() {
            return;
        }
        let token = self.as_ref().rust().window_token.to_string();
        if let Ok(mut directory) = directory().lock() {
            directory.controller = Some(token);
            directory.shutdown = self.as_ref().rust().ipc_shutdown_gate;
        }
    }

    pub(super) fn register_ipc_owner(mut self: Pin<&mut Self>) -> bool {
        if self.as_ref().rust().ipc_owner.is_some() {
            self.as_mut().refresh_ipc_owner();
            return true;
        }
        let token = self.as_ref().rust().window_token.to_string();
        let thread = self.as_ref().get_ref().qt_thread();
        let Ok(mut directory) = directory().lock() else {
            return false;
        };
        if directory.owners.contains_key(&token) {
            return false;
        }
        directory.clock = directory.clock.saturating_add(1);
        directory.generation = directory.generation.saturating_add(1);
        let order = directory.clock;
        directory.owners.insert(
            token.clone(),
            Owner {
                token: token.clone(),
                window: None,
                operations: Vec::new(),
                operation_revision: 0,
                thread,
                active: false,
                focused: 0,
                order,
                context_catalog: None,
            },
        );
        drop(directory);
        self.as_mut().rust_mut().as_mut().get_mut().ipc_owner = Some(OwnerLease(token));
        self.as_mut().refresh_ipc_owner();
        true
    }

    pub(super) fn refresh_ipc_owner(self: Pin<&mut Self>) {
        let profile_available = self.as_ref().get_ref().profile_available();
        let rust = self.as_ref().get_ref().rust();
        let Some(lease) = rust.ipc_owner.as_ref() else {
            return;
        };
        let Ok(mut directory) = directory().lock() else {
            return;
        };
        let Some(owner) = directory.owners.get_mut(&lease.0) else {
            return;
        };
        let window = (!rust.profile_bootstrap_pending && profile_available)
            .then_some(rust.window)
            .flatten();
        let changed = owner.window != window;
        owner.window = window;
        // Admission and retention both change membership. Compare the catalog revision
        // so equal-size eviction cannot leave routing pointed at forgotten IDs.
        if changed || owner.operation_revision != rust.operation_revision() {
            owner.operations = rust
                .operation_records()
                .map(|record| record.id.clone())
                .collect();
            owner.operation_revision = rust.operation_revision();
        }
        if changed {
            directory.generation = directory.generation.saturating_add(1);
        }
        if directory.controller.as_deref() == Some(lease.0.as_str()) {
            directory.shutdown = rust.ipc_shutdown_gate;
        }
        let catalogs = if changed && window.is_some() {
            directory
                .owners
                .values()
                .filter(|owner| owner.token != lease.0)
                .filter_map(|owner| owner.context_catalog.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        drop(directory);
        // A window may finish bootstrap after the original delivery. Catch up
        // from committed envelopes retained by live owners, not from GUI JSON.
        for (revision, catalog) in catalogs {
            let _ = self.as_ref().get_ref().qt_thread().queue(move |mut ui| {
                if ui.as_ref().rust().window == window && ui.as_ref().rust().ipc_owner.is_some() {
                    ui.as_mut().receive_context_catalog(catalog, revision);
                }
            });
        }
    }

    pub(super) fn set_ipc_window_active(self: Pin<&mut Self>, active: bool) {
        let rust = self.as_ref().get_ref().rust();
        let Some(lease) = rust.ipc_owner.as_ref() else {
            return;
        };
        let Ok(mut directory) = directory().lock() else {
            return;
        };
        directory.clock = directory.clock.saturating_add(1);
        let focused = directory.clock;
        if active {
            for owner in directory.owners.values_mut() {
                owner.active = false;
            }
        }
        if let Some(owner) = directory.owners.get_mut(&lease.0) {
            owner.active = active;
            if active {
                owner.focused = focused;
            }
        }
    }

    pub(super) fn release_ipc_owner(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().as_mut().get_mut().ipc_owner.take();
    }

    pub(super) fn route_pending_ipc(mut self: Pin<&mut Self>, pending: PendingRequest) {
        let request = pending.request().clone();
        if request.method == "operations.cancel"
            && request
                .params
                .get("operation_id")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        {
            let response = failure(
                &request,
                ErrorCode::InvalidArgument,
                "Provide an operation ID to cancel.",
            );
            let _ = pending.respond(&response);
            return;
        }
        if instance_shutdown(&request) {
            let response = failure(
                &request,
                ErrorCode::Busy,
                "The browser is closing. Try again after shutdown is cancelled.",
            );
            let _ = pending.respond(&response);
            return;
        }
        let selector = match selector(&request) {
            Ok(selector) => selector,
            Err(error) => {
                let response = Response::failure(request.id.clone(), error.into_protocol_error());
                let _ = pending.respond(&response);
                return;
            }
        };
        if let Some(key) = query_key(&request) {
            let snapshot = directory().lock().ok().map(|directory| {
                (
                    directory
                        .owners
                        .values()
                        .filter(|owner| owner.window.is_some())
                        .cloned()
                        .collect::<Vec<_>>(),
                    directory.generation,
                )
            });
            if let Some((owners, generation)) = snapshot
                && !owners.is_empty()
            {
                if owners.len() > 64 {
                    let response = failure(
                        &request,
                        ErrorCode::Busy,
                        "Too many windows are open for this query. Close some windows and try again.",
                    );
                    let _ = pending.respond(&response);
                    return;
                }
                collect_query(pending, key, owners, generation);
                return;
            }
        }
        let selected = directory().lock().ok().and_then(|directory| {
            if request.method == "operations.cancel" {
                let operation = request.params.get("operation_id").and_then(Value::as_str)?;
                directory
                    .owners
                    .values()
                    .find(|owner| owner.operations.iter().any(|id| id == operation))
                    .cloned()
            } else {
                directory.selected(selector)
            }
        });
        let Some(owner) = selected else {
            let _ = pending.respond(&missing_owner(&request, selector));
            return;
        };
        let Some(window) = owner.window else {
            let response = failure(
                &request,
                ErrorCode::Busy,
                "The browser is still starting. Try again shortly.",
            );
            let _ = pending.respond(&response);
            return;
        };
        let request = target_request(request, window);
        if owner.token == self.as_ref().rust().window_token.to_string() {
            self.as_mut().deliver_pending_ipc(pending, &request);
            return;
        }
        let mut reply = Reply(Some(pending));
        let _ = owner.thread.queue(move |mut ui| {
            if instance_shutdown(&request) {
                reply.finish(&failure(
                    &request,
                    ErrorCode::Busy,
                    "The browser is closing. Try again after shutdown is cancelled.",
                ));
                return;
            }
            if ui.as_ref().rust().window != Some(window)
                || !ui.as_ref().get_ref().profile_available()
                || ui.as_ref().rust().profile_bootstrap_pending
                || ui.as_ref().rust().ipc_owner.is_none()
            {
                reply.finish(&unavailable(&request));
                return;
            }
            if let Some(pending) = reply.0.take() {
                ui.as_mut().deliver_pending_ipc(pending, &request);
            }
            ui.as_mut().runtime_work_available();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::net::UnixStream, time::Duration};

    fn owner(number: u64, order: u64, focused: u64, active: bool) -> Owner<()> {
        Owner {
            token: uuid::Uuid::from_u128(u128::from(number)).to_string(),
            window: WindowId::from_display(&format!("windowid-{number}")),
            operations: Vec::new(),
            operation_revision: 0,
            thread: (),
            active,
            focused,
            order,
            context_catalog: None,
        }
    }

    fn request(method: &str, params: Value) -> Request {
        Request {
            id: "owner-request".into(),
            method: method.into(),
            params,
        }
    }

    #[test]
    fn explicit_window_and_token_never_fall_back_to_another_owner() {
        let first = owner(11, 1, 7, true);
        let second = owner(22, 2, 6, false);
        let token = second.token.clone();
        let mut directory = Directory::default();
        directory.owners.insert(first.token.clone(), first);
        directory.owners.insert(second.token.clone(), second);
        assert_eq!(
            directory.selected(Some("windowid-22")).unwrap().token,
            token
        );
        assert_eq!(
            directory.selected(Some("WINDOWID-22")).unwrap().token,
            token
        );
        assert_eq!(
            directory.selected(Some(&token)).unwrap().window,
            WindowId::from_display("windowid-22")
        );
        directory.owners.remove(&token);
        assert!(directory.selected(Some("windowid-22")).is_none());
        assert!(directory.selected(Some(&token)).is_none());
        assert!(directory.selected(Some("windowid-999")).is_none());
        assert!(directory.selected(Some("active")).is_some());
    }

    #[test]
    fn focus_selectors_track_the_window_and_preserve_last_focus_on_deactivation() {
        let first = owner(11, 1, 4, false);
        let second = owner(22, 2, 9, true);
        let token = second.token.clone();
        let mut directory = Directory::default();
        directory.owners.insert(first.token.clone(), first);
        directory.owners.insert(second.token.clone(), second);
        assert_eq!(directory.selected(None).unwrap().token, token);
        directory.owners.get_mut(&token).unwrap().active = false;
        assert_eq!(
            directory.selected(Some("last-focused")).unwrap().token,
            token
        );
        assert_eq!(directory.selected(Some("active")).unwrap().token, token);
    }

    #[test]
    fn missing_implicit_window_is_retryable_but_explicit_stale_identity_is_preserved() {
        let query = request("windows.query", json!({}));
        for selector in [None, Some("active"), Some("last-focused")] {
            assert_eq!(
                missing_owner(&query, selector).error.unwrap().code,
                "E_BUSY"
            );
        }
        assert_eq!(
            missing_owner(&query, Some("windowid-99"))
                .error
                .unwrap()
                .code,
            "E_STALE_TARGET"
        );
        let cancel = request("operations.cancel", json!({"operation_id":"missing"}));
        assert_eq!(
            missing_owner(&cancel, None).error.unwrap().code,
            "E_NOT_FOUND"
        );
    }

    #[test]
    fn unadmitted_controller_leases_are_never_implicit_window_targets() {
        let ready = owner(11, 1, 4, false);
        let token = ready.token.clone();
        let mut starting = owner(22, 2, 99, true);
        starting.window = None;
        let starting_token = starting.token.clone();
        let mut directory = Directory::default();
        directory.owners.insert(token.clone(), ready);
        directory.owners.insert(starting_token.clone(), starting);
        for selector in [None, Some("active"), Some("last-focused")] {
            assert_eq!(directory.selected(selector).unwrap().token, token);
        }
        // Explicitly naming an unavailable owner keeps that identity. The
        // dispatcher will reject it instead of selecting a different window.
        assert!(
            directory
                .selected(Some(&starting_token))
                .unwrap()
                .window
                .is_none()
        );
        directory.owners.remove(&token);
        for selector in [None, Some("active"), Some("last-focused")] {
            assert!(directory.selected(selector).is_none());
        }
    }

    #[test]
    fn ambiguous_legacy_window_ids_are_rejected() {
        let first = owner(11, 1, 1, true);
        let mut second = owner(22, 2, 2, false);
        second.window = first.window;
        let mut directory = Directory::default();
        directory.owners.insert(first.token.clone(), first);
        directory.owners.insert(second.token.clone(), second);
        assert!(directory.selected(Some("windowid-11")).is_none());
    }

    #[test]
    fn resolving_owner_preserves_profile_context_source_and_unknown_fields() {
        let original = request(
            "command.execute",
            json!({"command":"open", "arguments":{"input":"rb://blank"},
            "context":{"window":"last-focused","profile":"work","context":"research","source":"cli","untrusted":true}}),
        );
        let resolved = target_request(
            original.clone(),
            WindowId::from_display("windowid-22").unwrap(),
        );
        let mut expected = original;
        expected.params["context"]["window"] = json!("windowid-22");
        assert_eq!(resolved, expected);
        let malformed = request("command.execute", json!({"context":"invalid"}));
        assert_eq!(
            target_request(
                malformed.clone(),
                WindowId::from_display("windowid-22").unwrap()
            ),
            malformed
        );
        assert!(selector(&request("tabs.query", json!({"window":32}))).is_err());
        assert!(selector(&request("tabs.query", json!({"window":"missing"}))).is_err());
    }

    #[test]
    fn active_tab_query_targets_one_owner_while_all_tabs_are_collected() {
        assert_eq!(query_key(&request("tabs.query", json!({}))), Some("tabs"));
        assert_eq!(
            query_key(&request("tabs.query", json!({"active_only":true}))),
            None
        );
        assert_eq!(
            query_key(&request("tabs.query", json!({"window":"active"}))),
            None
        );
        assert_eq!(
            query_key(&request("windows.query", json!({"window":"last-focused"}))),
            None
        );
    }

    #[test]
    fn discarded_qt_delivery_resolves_the_client_with_a_stale_target_error() {
        let (server, mut client) = UnixStream::pair().expect("pair");
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("timeout");
        drop(Reply(Some(PendingRequest::new(
            request("command.execute", json!({})),
            server,
        ))));
        let frame = ferric_browser_ipc::read_frame(&mut client)
            .expect("frame")
            .expect("response");
        let response: Response = serde_json::from_slice(&frame).expect("response JSON");
        assert_eq!(response.id, "owner-request");
        assert_eq!(response.error.expect("failure").code, "E_STALE_TARGET");
        assert!(
            ferric_browser_ipc::read_frame(&mut client)
                .expect("EOF")
                .is_none()
        );
    }

    #[test]
    fn collected_queries_keep_creation_order_and_resolve_discarded_parts() {
        for discard in [false, true] {
            let (server, mut client) = UnixStream::pair().expect("pair");
            client
                .set_read_timeout(Some(Duration::from_secs(1)))
                .expect("timeout");
            let request = request("windows.query", json!({}));
            let collection = Arc::new(Mutex::new(QueryCollection {
                reply: Some(Reply(Some(PendingRequest::new(request.clone(), server)))),
                request: request.clone(),
                key: "windows",
                remaining: 2,
                generation: directory().lock().unwrap().generation,
                sequence: 41,
                responses: Vec::new(),
            }));
            let first = QueryPart {
                collection: Arc::clone(&collection),
                request: request.clone(),
                finished: false,
                order: 1,
            };
            let second = QueryPart {
                collection: Arc::clone(&collection),
                request: request.clone(),
                finished: false,
                order: 2,
            };
            second.finish(Response::success(
                request.id.clone(),
                json!({"windows":[{"id":"second"}]}),
            ));
            if discard {
                drop(first);
            } else {
                first.finish(Response::success(
                    request.id.clone(),
                    json!({"windows":[{"id":"first"}]}),
                ));
            }
            let frame = ferric_browser_ipc::read_frame(&mut client)
                .expect("frame")
                .expect("response");
            let response: Response = serde_json::from_slice(&frame).expect("response JSON");
            if discard {
                assert_eq!(response.error.expect("failure").code, "E_STALE_TARGET");
            } else {
                let result = response.result.expect("result");
                assert_eq!(result["windows"], json!([{"id":"first"},{"id":"second"}]));
                assert_eq!(result["sequence"], 41);
            }
            assert!(
                ferric_browser_ipc::read_frame(&mut client)
                    .expect("EOF")
                    .is_none()
            );
        }
    }
}
