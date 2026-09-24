//! Userscript-specific action dispatch for Qt IPC.

use super::{
    CxxQtType, Pin, PublicError, Request, Response, Value, action_failure_category,
    ipc_action_failure, qobject, userscript, userscript_action_argument_name,
    userscript_action_is_hint_only,
};

impl qobject::BrowserUi {
    pub(super) fn handle_userscript_action_request(
        mut self: Pin<&mut Self>,
        request: &Request,
        action_id: &str,
    ) -> Response {
        let Some(object) = request.params.as_object() else {
            return ipc_action_failure(
                &request.id,
                action_id,
                "unissued",
                "userscript action parameters must be an object",
            );
        };
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "action" | "arguments"))
        {
            return ipc_action_failure(
                &request.id,
                action_id,
                "unissued",
                "userscript action parameters contain an unknown field",
            );
        }
        let expected_argument = match self
            .as_ref()
            .rust()
            .userscript_roots
            .as_ref()
            .map(|roots| userscript::registered_actions(&roots.config))
        {
            None => {
                return ipc_action_failure(
                    &request.id,
                    action_id,
                    "unissued",
                    "userscripts are unavailable without configured storage",
                );
            }
            Some(Err(error)) => {
                return ipc_action_failure(&request.id, action_id, "unissued", error);
            }
            Some(Ok(actions)) => {
                let Some(action) = actions.iter().find(|action| action.id == action_id) else {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        "userscript action is not installed",
                    );
                };
                if userscript_action_is_hint_only(action) {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        "hint-only userscript action must be activated from a validated hint",
                    );
                }
                userscript_action_argument_name(&action.subject)
            }
        };
        let value = match object.get("arguments") {
            None => Ok(String::new()),
            Some(arguments) => {
                let Some(arguments) = arguments.as_object() else {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        "userscript action arguments must be an object",
                    );
                };
                if arguments.len() > 1
                    || expected_argument.is_none() && !arguments.is_empty()
                    || expected_argument.is_some_and(|name| !arguments.contains_key(name))
                {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        expected_argument.map_or_else(
                            || "userscript action does not accept subject arguments".to_owned(),
                            |name| format!("userscript action requires argument {name}"),
                        ),
                    );
                }
                if arguments
                    .keys()
                    .any(|key| Some(key.as_str()) != expected_argument)
                {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        format!(
                            "userscript action arguments must use {}",
                            expected_argument.unwrap_or("no subject field")
                        ),
                    );
                }
                let mut values = arguments.values();
                let value = values.next();
                if values.next().is_some() {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        "userscript action accepts one subject value",
                    );
                }
                value
                    .map(|value| {
                        value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                            "userscript action subject value must be a string".to_owned()
                        })
                    })
                    .unwrap_or(Ok(String::new()))
            }
        };
        let value = match value {
            Ok(value) => value,
            Err(error) => {
                return ipc_action_failure(&request.id, action_id, "unissued", error);
            }
        };
        match self
            .as_mut()
            .execute_registered_userscript_action(action_id, &value)
        {
            Ok(mut result) => {
                let operation_id = result
                    .get("operation_id")
                    .and_then(Value::as_str)
                    .unwrap_or("untracked")
                    .to_owned();
                self.as_mut()
                    .record_action_audit(action_id, &operation_id, "accepted", None);
                result["action_id"] = Value::String(action_id.to_owned());
                Response::success(request.id.clone(), result)
            }
            Err(error) => {
                let error = PublicError::engine(error);
                let category = action_failure_category(error.code());
                self.as_mut().record_action_audit(
                    action_id,
                    "unissued",
                    "rejected",
                    Some(category),
                );
                ipc_action_failure(&request.id, action_id, "unissued", error)
            }
        }
    }
}
