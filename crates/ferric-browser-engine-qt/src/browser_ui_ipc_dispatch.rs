use super::{
    ActionRegistry, CxxQtType, Pin, PublicError, Request, Response, Uuid, Value,
    action_failure_category, ipc_action_failure, ipc_action_failure_with_context,
    ipc_command_context, ipc_command_count, ipc_command_failure_with_context, ipc_failure,
    is_mutating_ipc_method, parse_external_action_id, qobject, typed_ipc_action, typed_ipc_command,
};

impl qobject::BrowserUi {
    pub(super) fn handle_ipc_request(mut self: Pin<&mut Self>, request: &Request) -> Response {
        if self.as_ref().rust().ipc_shutdown_gate && is_mutating_ipc_method(&request.method) {
            return ipc_failure(
                &request.id,
                "E_BUSY",
                "shutdown is in progress; mutating IPC is temporarily disabled",
            );
        }
        match request.method.as_str() {
            "command.execute" => {
                let (command, route) = match typed_ipc_command(&request.params) {
                    Ok(command) => command,
                    Err(error) => {
                        return Response::failure(
                            request.id.clone(),
                            error.into_public().into_protocol_error(),
                        );
                    }
                };
                let operation_id = format!("op-{}", Uuid::new_v4());
                let count = ipc_command_count(&command);
                let command_context = ipc_command_context(
                    self.as_ref().rust().state.as_ref(),
                    &route,
                    &operation_id,
                    count,
                );
                match self.as_mut().execute_ipc_command_with_operation(
                    command,
                    &route,
                    Some(&operation_id),
                ) {
                    Ok(mut result) => {
                        result["operation_id"] = Value::String(operation_id.clone());
                        result["command_context"] = command_context;
                        self.as_mut()
                            .rust_mut()
                            .as_mut()
                            .get_mut()
                            .operation_states
                            .insert(operation_id, "accepted".into());
                        Response::success(request.id.clone(), result)
                    }
                    Err(error) => {
                        ipc_command_failure_with_context(&request.id, error, command_context)
                    }
                }
            }
            "action.execute" => {
                if let Some(action_id) = request.params.get("action").and_then(Value::as_str)
                    && action_id.starts_with("userscript.")
                {
                    return self
                        .as_mut()
                        .handle_userscript_action_request(request, action_id);
                }
                let audit_action_id = request
                    .params
                    .get("action")
                    .and_then(Value::as_str)
                    .and_then(|action| {
                        let registry = ActionRegistry::default_v1();
                        registry
                            .resolve(action)
                            .map(|definition| definition.id.clone())
                            .or_else(|| parse_external_action_id(action).map(|_| action.to_owned()))
                    })
                    .unwrap_or_else(|| "unknown".into());
                let (command, route, action_id) = match typed_ipc_action(&request.params) {
                    Ok(command) => command,
                    Err(error) => {
                        let error = error.into_public();
                        let category = action_failure_category(error.code());
                        self.as_mut().record_action_audit(
                            &audit_action_id,
                            "unissued",
                            "rejected",
                            Some(category),
                        );
                        return ipc_action_failure(
                            &request.id,
                            &audit_action_id,
                            "unissued",
                            error,
                        );
                    }
                };
                let operation_id = format!("op-{}", Uuid::new_v4());
                let count = ipc_command_count(&command);
                let command_context = ipc_command_context(
                    self.as_ref().rust().state.as_ref(),
                    &route,
                    &operation_id,
                    count,
                );
                if let Err(error) = self
                    .as_ref()
                    .validate_action_availability(&action_id, route.source)
                {
                    let error = PublicError::engine(error);
                    let category = action_failure_category(error.code());
                    self.as_mut().record_action_audit(
                        &action_id,
                        &operation_id,
                        "failed",
                        Some(category),
                    );
                    return ipc_action_failure_with_context(
                        &request.id,
                        &action_id,
                        &operation_id,
                        error,
                        command_context,
                    );
                }
                match self.as_mut().execute_ipc_command_with_operation(
                    command,
                    &route,
                    Some(&operation_id),
                ) {
                    Ok(mut result) => {
                        self.as_mut().record_action_audit(
                            &action_id,
                            &operation_id,
                            "accepted",
                            None,
                        );
                        result["action_id"] = Value::String(action_id);
                        result["operation_id"] = Value::String(operation_id.clone());
                        result["command_context"] = command_context;
                        self.as_mut()
                            .rust_mut()
                            .as_mut()
                            .get_mut()
                            .operation_states
                            .insert(operation_id, "accepted".into());
                        Response::success(request.id.clone(), result)
                    }
                    Err(error) => {
                        let error = PublicError::engine(error);
                        let category = action_failure_category(error.code());
                        self.as_mut().record_action_audit(
                            &action_id,
                            &operation_id,
                            "failed",
                            Some(category),
                        );
                        self.as_mut()
                            .rust_mut()
                            .as_mut()
                            .get_mut()
                            .operation_states
                            .insert(operation_id.clone(), format!("failed ({category})"));
                        ipc_action_failure_with_context(
                            &request.id,
                            &action_id,
                            &operation_id,
                            error,
                            command_context,
                        )
                    }
                }
            }
            "tabs.query" => match self.as_ref().ipc_tabs_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "windows.query" => match self.as_ref().ipc_windows_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "window.focus" => match self.as_mut().ipc_window_focus(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "profiles.query" => match self.as_ref().ipc_profiles_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "actions.query" => match self.as_ref().ipc_actions_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "bindings.query" => match self.as_ref().ipc_bindings_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "bindings.explain" => match self.as_ref().ipc_bindings_explain(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "downloads.query" => {
                if self.as_ref().rust().profile_persistence.is_durable()
                    && (self.as_ref().rust().storage_library.is_none()
                        || self.as_ref().rust().storage_library_dirty.get())
                {
                    self.as_mut().request_storage_library();
                }
                match self.as_ref().ipc_downloads_query(&request.params) {
                    Ok(result) => Response::success(request.id.clone(), result),
                    Err(error) => {
                        let (code, message) = error.into_public_parts();
                        ipc_failure(&request.id, code, message)
                    }
                }
            }
            "permissions.query" => {
                if self.as_ref().rust().profile_persistence.is_durable()
                    && (self.as_ref().rust().storage_library.is_none()
                        || self.as_ref().rust().storage_library_dirty.get())
                {
                    self.as_mut().request_storage_library();
                }
                match self.as_ref().ipc_permissions_query(&request.params) {
                    Ok(result) => Response::success(request.id.clone(), result),
                    Err(error) => {
                        let (code, message) = error.into_public_parts();
                        ipc_failure(&request.id, code, message)
                    }
                }
            }
            "contexts.query" => match self.as_ref().ipc_contexts_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "switcher.query" => {
                if self.as_ref().rust().profile_persistence.is_durable()
                    && (self.as_ref().rust().storage_library.is_none()
                        || self.as_ref().rust().storage_library_dirty.get())
                {
                    self.as_mut().request_storage_library();
                }
                self.as_mut().request_session_names();
                match self.as_ref().ipc_switcher_query(&request.params) {
                    Ok(result) => Response::success(request.id.clone(), result),
                    Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
                }
            }
            "switcher.activate" => match self.as_mut().ipc_switcher_activate(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "site.status" => {
                if self.as_ref().rust().profile_persistence.is_durable()
                    && (self.as_ref().rust().storage_library.is_none()
                        || self.as_ref().rust().storage_library_dirty.get())
                {
                    self.as_mut().request_storage_library();
                }
                match self.as_ref().ipc_site_status(&request.params) {
                    Ok(result) => Response::success(request.id.clone(), result),
                    Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
                }
            }
            "blocking.status" => match self.as_ref().ipc_blocking_status(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "operations.query" => match self.as_ref().ipc_operations_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "operations.cancel" => match self.as_mut().ipc_operation_cancel(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_NOT_FOUND", error),
            },
            "diagnostics.get" => match self.as_ref().ipc_diagnostics(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "config.get" => match self.as_ref().ipc_config_get(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => {
                    let (code, message) = error.into_public_parts();
                    ipc_failure(&request.id, code, message)
                }
            },
            "events.subscribe" => match self.as_ref().ipc_events_subscribe(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            _ => ipc_failure(
                &request.id,
                "E_UNSUPPORTED",
                format!("unsupported IPC method: {}", request.method),
            ),
        }
    }
}
