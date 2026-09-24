//! Typed decoding of compact QML action values.
//!
//! The presentation layer emits bounded strings; this module owns the sole
//! conversion into the argument objects consumed by the shared action path.

use serde_json::Value;

pub(super) fn decode_ui_action_arguments(action_id: &str, value: &str) -> Result<Value, String> {
    match action_id {
        "browser.url.open" | "browser.tab.open" => Ok(serde_json::json!({"input": value})),
        "browser.url.copy" | "browser.url.clean-copy" => Ok(serde_json::json!({})),
        "browser.url.clean" | "browser.url.explain" => Ok(optional_value("url", value)),
        "browser.link.open"
        | "browser.link.copy"
        | "browser.link.clean-copy"
        | "browser.link.download" => Ok(serde_json::json!({"url": value})),
        "browser.url.send" | "browser.link.send" => send_arguments(value),
        "browser.selection.send" | "browser.tab.send" => Ok(serde_json::json!({"target": value})),
        "browser.selection.copy" => Ok(serde_json::json!({})),
        "browser.selection.search" => Ok(optional_value("engine", value)),
        "browser.context.enter" => Ok(serde_json::json!({"name": value})),
        "browser.context.save" => Ok(optional_value("name", value)),
        "browser.window.focus" => Ok(serde_json::json!({"id": value})),
        "browser.window.move" => {
            let values = fields(value, 2, "Window move")?;
            Ok(serde_json::json!({"id": values[0], "workspace": values[1]}))
        }
        "browser.window.new" => window_new_arguments(value),
        "browser.window.close"
        | "browser.tab.stop"
        | "browser.tab.clone"
        | "browser.tab.undo"
        | "browser.tab.reopen-window"
        | "browser.tab.detach"
            if value.is_empty() =>
        {
            Ok(serde_json::json!({}))
        }
        "browser.tab.next" | "browser.tab.previous" => {
            count_arguments(value, 9_999, "Tab traversal")
        }
        "browser.tab.back" | "browser.tab.forward" => {
            count_arguments(value, 100, "History traversal")
        }
        "browser.tab.reload" if value.is_empty() => Ok(serde_json::json!({})),
        "browser.tab.reload" if value == "bypass-cache" => {
            Ok(serde_json::json!({"bypass_cache": true}))
        }
        "browser.window.fullscreen" => Ok(optional_value("state", value)),
        "browser.tab.zoom" => Ok(serde_json::json!({"factor": value})),
        "browser.tab.scroll" | "browser.tab.scroll-page" => {
            Ok(serde_json::json!({"direction": value}))
        }
        "browser.tab.scroll-to" => Ok(serde_json::json!({"edge": value})),
        "browser.tab.search-next" => Ok(optional_value("direction", value)),
        "browser.tab.select" => Ok(serde_json::json!({"selector": value})),
        "browser.tab.focus"
        | "browser.tab.close"
        | "browser.tab.suspend"
        | "browser.tab.discard"
        | "browser.tab.resume" => Ok(serde_json::json!({"id": value})),
        "browser.tab.give" => Ok(serde_json::json!({"window_id": value})),
        "browser.tab.move" => {
            let values = fields(value, 2, "Tab move")?;
            Ok(serde_json::json!({"id": values[0], "direction": values[1]}))
        }
        "browser.tab.pin" | "browser.tab.mute" => tab_state_arguments(value),
        "browser.history-entry.open" | "browser.bookmark.open" | "browser.bookmark.delete" => {
            Ok(serde_json::json!({"id": value}))
        }
        "browser.history-entry.clear" => {
            if value.is_empty() {
                Ok(serde_json::json!({}))
            } else {
                serde_json::from_str(value)
                    .map_err(|_| "History clear UI action requires a JSON object".to_owned())
            }
        }
        "browser.bookmark.add" => Ok(optional_value("title", value)),
        "browser.bookmark.edit" => {
            let values = fields(value, 2, "Bookmark edit")?;
            Ok(serde_json::json!({"id": values[0], "title": values[1]}))
        }
        "browser.bookmark.list" | "browser.quickmark.list" | "browser.session.list"
            if value.is_empty() =>
        {
            Ok(serde_json::json!({}))
        }
        "browser.quickmark.open" | "browser.quickmark.delete" => {
            Ok(serde_json::json!({"name": value}))
        }
        "browser.quickmark.add" => quickmark_add_arguments(value),
        "browser.quickmark.edit" => {
            let values = fields(value, 2, "Quickmark edit")?;
            Ok(serde_json::json!({"name": values[0], "url": values[1]}))
        }
        "browser.session.save" | "browser.session.delete" => Ok(serde_json::json!({"name": value})),
        "browser.session.load" => session_load_arguments(value),
        "browser.download.open"
        | "browser.download.show"
        | "browser.download.pause"
        | "browser.download.resume"
        | "browser.download.cancel"
        | "browser.download.retry" => Ok(serde_json::json!({"id": value})),
        "browser.command.help" | "browser.command.execute" => Ok(serde_json::json!({"id": value})),
        _ => Err("UI action is not available here".into()),
    }
}

fn fields<'a>(value: &'a str, count: usize, label: &str) -> Result<Vec<&'a str>, String> {
    let fields = value.split('\t').collect::<Vec<_>>();
    if fields.len() != count || fields.iter().any(|field| field.is_empty()) {
        return Err(format!("{label} UI action value has invalid fields"));
    }
    Ok(fields)
}

fn optional_fields<'a>(value: &'a str, max: usize, label: &str) -> Result<Vec<&'a str>, String> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let fields = value.split('\t').collect::<Vec<_>>();
    if fields.len() > max || fields.iter().any(|field| field.is_empty()) {
        return Err(format!("{label} UI action value has invalid fields"));
    }
    Ok(fields)
}

fn optional_value(name: &str, value: &str) -> Value {
    if value.is_empty() {
        return serde_json::json!({});
    }
    let mut object = serde_json::Map::new();
    object.insert(name.to_owned(), Value::String(value.to_owned()));
    Value::Object(object)
}

fn send_arguments(value: &str) -> Result<Value, String> {
    match optional_fields(value, 2, "Send")?.as_slice() {
        [target] => Ok(serde_json::json!({"target": target})),
        [target, url] => Ok(serde_json::json!({"target": target, "url": url})),
        _ => Err("Send UI action requires a target".into()),
    }
}

fn window_new_arguments(value: &str) -> Result<Value, String> {
    match optional_fields(value, 2, "Window new")?.as_slice() {
        [] => Ok(serde_json::json!({})),
        [private] if *private == "private" => Ok(serde_json::json!({"private": true})),
        [profile] => Ok(serde_json::json!({"profile": profile})),
        [profile, private] if *private == "private" => {
            Ok(serde_json::json!({"profile": profile, "private": true}))
        }
        _ => unreachable!("optional fields are bounded"),
    }
}

fn count_arguments(value: &str, maximum: u64, label: &str) -> Result<Value, String> {
    if value.is_empty() {
        return Ok(serde_json::json!({}));
    }
    value
        .parse::<u64>()
        .ok()
        .filter(|count| (1..=maximum).contains(count))
        .map(|count| serde_json::json!({"count": count}))
        .ok_or_else(|| format!("{label} count must be 1..{maximum}"))
}

fn tab_state_arguments(value: &str) -> Result<Value, String> {
    match optional_fields(value, 2, "Tab state")?.as_slice() {
        [id] => Ok(serde_json::json!({"id": id})),
        [id, state] => Ok(serde_json::json!({"id": id, "state": state})),
        _ => Err("Tab state UI action requires a tab ID".into()),
    }
}

fn quickmark_add_arguments(value: &str) -> Result<Value, String> {
    match optional_fields(value, 2, "Quickmark add")?.as_slice() {
        [name] => Ok(serde_json::json!({"name": name})),
        [name, url] => Ok(serde_json::json!({"name": name, "url": url})),
        _ => Err("Quickmark add UI action requires a name".into()),
    }
}

fn session_load_arguments(value: &str) -> Result<Value, String> {
    match optional_fields(value, 2, "Session load")?.as_slice() {
        [name] => Ok(serde_json::json!({"name": name})),
        [name, append] if *append == "append" => {
            Ok(serde_json::json!({"name": name, "append": true}))
        }
        _ => Err("Session load UI action requires a session name".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_compact_action_values_without_opening_the_argument_boundary() {
        assert_eq!(
            decode_ui_action_arguments("browser.link.send", "mpv\thttps://example.test")
                .expect("valid send"),
            serde_json::json!({"target": "mpv", "url": "https://example.test"})
        );
        assert_eq!(
            decode_ui_action_arguments("browser.tab.next", "3").expect("valid count"),
            serde_json::json!({"count": 3})
        );
        assert!(decode_ui_action_arguments("browser.tab.move", "tab-1").is_err());
        assert!(decode_ui_action_arguments("browser.tab.reload", "unexpected").is_err());
    }
}
