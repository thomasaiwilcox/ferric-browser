//! Pure parsing and authorization rules for the switcher.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QueryOrigin {
    NativeUi,
    External,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QueryAccessError {
    PrivateQueriesDisabled,
    MissingNativeScope,
    StalePrivateScope,
}

pub(super) fn authorize_query(
    origin: QueryOrigin,
    include_private: bool,
    private_queries_enabled: bool,
    private_scope: Option<&str>,
    active_private_scope: Option<&str>,
) -> Result<(), QueryAccessError> {
    if include_private && origin == QueryOrigin::External && !private_queries_enabled {
        return Err(QueryAccessError::PrivateQueriesDisabled);
    }
    if include_private && origin == QueryOrigin::NativeUi && private_scope.is_none() {
        return Err(QueryAccessError::MissingNativeScope);
    }
    if private_scope.is_some() && private_scope != active_private_scope {
        return Err(QueryAccessError::StalePrivateScope);
    }
    Ok(())
}

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

#[cfg(test)]
mod query_access_tests {
    use super::{QueryAccessError as Error, QueryOrigin as Origin, authorize_query};

    #[test]
    fn external_scope_fields_never_grant_private_query_authority() {
        for supplied in [None, Some("Private"), Some("another session")] {
            assert_eq!(
                authorize_query(Origin::External, true, false, supplied, Some("Private")),
                Err(Error::PrivateQueriesDisabled)
            );
        }
    }

    #[test]
    fn private_native_search_requires_its_exact_live_session_even_with_external_opt_in() {
        for opted_in in [false, true] {
            assert_eq!(
                authorize_query(Origin::NativeUi, true, opted_in, None, Some("Private")),
                Err(Error::MissingNativeScope)
            );
            for active in [None, Some("another session")] {
                assert_eq!(
                    authorize_query(Origin::NativeUi, true, opted_in, Some("Private"), active),
                    Err(Error::StalePrivateScope)
                );
            }
            assert_eq!(
                authorize_query(
                    Origin::NativeUi,
                    true,
                    opted_in,
                    Some("Private"),
                    Some("Private")
                ),
                Ok(())
            );
        }
    }

    #[test]
    fn explicit_external_opt_in_and_ordinary_search_remain_available() {
        assert_eq!(
            authorize_query(Origin::External, true, true, None, Some("Private")),
            Ok(())
        );
        assert_eq!(
            authorize_query(
                Origin::External,
                true,
                true,
                Some("Private"),
                Some("Private")
            ),
            Ok(())
        );
        for origin in [Origin::External, Origin::NativeUi] {
            assert_eq!(authorize_query(origin, false, false, None, None), Ok(()));
        }
        assert_eq!(
            authorize_query(Origin::External, true, true, Some("old"), Some("new")),
            Err(Error::StalePrivateScope)
        );
    }
}
