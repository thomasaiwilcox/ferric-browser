//! Pure parsing and authorization rules for the switcher.

pub(super) fn default_action(kind: &str) -> Option<&'static str> {
    match kind {
        "tab" | "window" => Some("focus"),
        "context" => Some("enter"),
        "history" | "bookmark" | "quickmark" => Some("open"),
        "session" => Some("load-preview"),
        "download" => Some("show"),
        "command" => Some("help"),
        "closed" => Some("reopen"),
        "action" => Some("execute"),
        _ => None,
    }
}

pub(super) fn parse_command(arguments: &[String]) -> Result<(String, String), String> {
    let mut scope = "all".to_owned();
    let mut scope_set = false;
    let mut query = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--scope" => {
                if scope_set {
                    return Err("switcher accepts --scope at most once".into());
                }
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| "switcher --scope requires a value".to_owned())?;
                if !valid_scope(value) {
                    return Err("switcher --scope must be a valid switcher scope".into());
                }
                scope = value.clone();
                scope_set = true;
                index += 2;
            }
            value if !value.starts_with('-') => {
                query.push(value.to_owned());
                index += 1;
            }
            _ => return Err("switcher accepts optional --scope SCOPE and QUERY".into()),
        }
    }
    let query = query.join(" ");
    if query.len() > 4_096 {
        return Err("switcher query exceeds 4096 bytes".into());
    }
    Ok((scope, query))
}

pub(super) fn parse_generation(value: &str) -> Result<Option<u64>, String> {
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse::<u64>()
        .map(Some)
        .map_err(|_| "switcher target generation is invalid".to_owned())
}

pub(super) fn validate_generation(
    expected: Option<u64>,
    current: Option<u64>,
) -> Result<(), &'static str> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let Some(current) = current else {
        return Err("Switcher target generation is only valid for tabs");
    };
    if current == expected {
        Ok(())
    } else {
        Err("Switcher tab target is stale; refresh the results")
    }
}

pub(super) fn action_allowed(kind: &str, action: &str) -> bool {
    match kind {
        "tab" => matches!(action, "focus" | "open"),
        "window" => action == "focus",
        "context" => action == "enter",
        "history" => action == "open",
        "bookmark" | "quickmark" => matches!(action, "open" | "delete"),
        "session" => matches!(action, "load-preview" | "load"),
        "download" => matches!(action, "show" | "open"),
        "command" => matches!(action, "execute" | "help"),
        "closed" => action == "reopen",
        "action" => action == "execute",
        _ => false,
    }
}

pub(super) fn context_boost(current: Option<&str>, candidate: Option<&str>) -> i64 {
    if current.is_some() && current == candidate {
        25
    } else {
        0
    }
}

pub(super) fn valid_scope(scope: &str) -> bool {
    matches!(
        scope,
        "all"
            | "tabs"
            | "windows"
            | "contexts"
            | "commands"
            | "history"
            | "marks"
            | "sessions"
            | "downloads"
            | "closed"
            | "actions"
    )
}
