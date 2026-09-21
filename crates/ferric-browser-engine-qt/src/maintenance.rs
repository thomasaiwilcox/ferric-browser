use serde_json::{Value, json};

const MAX_CONFIGURED_LISTS: usize = 64;

/// Describes browser-owned network activity without enumerating page traffic,
/// URLs, accounts, or browsing history. `config` is optional because the
/// display-free diagnostics command does not open a user profile.
#[must_use]
pub fn snapshot(config: Option<&Value>, normal_profile: bool, durable_storage: bool) -> Value {
    let blocking = config
        .and_then(|value| value.get("blocking"))
        .and_then(Value::as_object);
    let privacy = config
        .and_then(|value| value.get("privacy"))
        .and_then(Value::as_object);
    let update_interval_hours = blocking
        .and_then(|value| value.get("update_interval_hours"))
        .and_then(Value::as_u64)
        .unwrap_or(24)
        .clamp(1, 168);
    let configured_lists = blocking
        .and_then(|value| value.get("lists"))
        .and_then(Value::as_array)
        .map_or(0, |lists| lists.len().min(MAX_CONFIGURED_LISTS));
    let push_configured = privacy
        .and_then(|value| value.get("push_service"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let remote_suggestions_configured = privacy
        .and_then(|value| value.get("remote_suggestions"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    json!({
        "status": "available",
        "config_loaded": config.is_some(),
        "telemetry": {
            "enabled": false,
            "automatic": false,
            "network": "none",
            "reason": "Ferric Browser has no telemetry client"
        },
        "crash_upload": {
            "enabled": false,
            "automatic": false,
            "network": "none",
            "reason": "crash markers and diagnostics remain local until explicitly copied"
        },
        "remote_suggestions": {
            "enabled": remote_suggestions_configured,
            "available": false,
            "network": "none",
            "reason": "no remote suggestion provider is implemented"
        },
        "history_sync": {
            "enabled": false,
            "automatic": false,
            "network": "none",
            "reason": "profiles and history have no browser-owned sync service"
        },
        "push_service": {
            "enabled": normal_profile && push_configured,
            "explicit_opt_in": true,
            "private_profiles_disabled": true,
            "provider": "QtWebEngine push service",
            "reason": "only the normal-profile configuration can enable the engine service"
        },
        "blocklist_updates": {
            "enabled": normal_profile && durable_storage && configured_lists > 0,
            "automatic": normal_profile && durable_storage && configured_lists > 0,
            "explicit_command": true,
            "interval_hours": update_interval_hours,
            "configured_list_count": configured_lists,
            "transport": "HTTPS",
            "private_profiles_trigger": false,
            "reason": "validated last-known-good lists are refreshed only for ordinary profiles"
        },
        "user_configured_pages_and_scripts": {
            "browser_owned": false,
            "automatic": false,
            "reason": "traffic comes from explicit user configuration or the loaded site"
        },
        "automatic_upload": false,
        "automatic_history_sync": false,
        "page_traffic": "not_browser_owned"
    })
}

#[cfg(test)]
mod tests {
    use super::snapshot;
    use serde_json::json;

    #[test]
    fn default_policy_has_no_hidden_browser_network() {
        let value = snapshot(None, false, false);
        assert_eq!(value["telemetry"]["enabled"], false);
        assert_eq!(value["crash_upload"]["enabled"], false);
        assert_eq!(value["remote_suggestions"]["available"], false);
        assert_eq!(value["history_sync"]["enabled"], false);
        assert_eq!(value["push_service"]["enabled"], false);
        assert_eq!(value["blocklist_updates"]["enabled"], false);
    }

    #[test]
    fn normal_profile_report_distinguishes_opt_in_push_and_blocklist_refresh() {
        let config = json!({
            "privacy": {"push_service": true, "remote_suggestions": false},
            "blocking": {
                "lists": ["easylist"],
                "update_interval_hours": 24
            }
        });
        let value = snapshot(Some(&config), true, true);
        assert_eq!(value["push_service"]["enabled"], true);
        assert_eq!(value["push_service"]["explicit_opt_in"], true);
        assert_eq!(value["blocklist_updates"]["enabled"], true);
        assert_eq!(value["blocklist_updates"]["explicit_command"], true);
        assert_eq!(value["blocklist_updates"]["configured_list_count"], 1);
        assert_eq!(value["automatic_upload"], false);
    }
}
