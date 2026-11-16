//! IPC projection of Core's typed Site Policies grammar.
use ferric_browser_core::{
    ParsedCommand, SitePolicyCommand, SitePolicyEdit, SitePolicyExplanation,
    SitePolicyExportSubject, SitePolicyRuleRef, parse_site_policy_command,
};
use serde_json::{Value, json};

fn rule_fields(value: &mut Value, rule: &SitePolicyRuleRef) {
    value["source"] = json!(rule.source.token());
    value["id"] = json!(rule.id);
}

pub(super) fn encode(command: &ParsedCommand) -> Result<Option<super::EncodedCommand>, String> {
    if !is_site_policy_command(&command.name) {
        return Ok(None);
    }
    let arguments = match parse_site_policy_command(command)? {
        SitePolicyCommand::Open { url } => json!({"url":url}),
        SitePolicyCommand::List { url, all } => json!({"url":url,"all":all}),
        SitePolicyCommand::Explain { url, rule, subject } => {
            let mut value = json!({"url":url});
            if let Some(rule) = rule {
                rule_fields(&mut value, &rule);
            }
            match subject {
                SitePolicyExplanation::Effective => {}
                SitePolicyExplanation::Key(key) => value["key"] = json!(key.as_str()),
                SitePolicyExplanation::Code(code) => value["code"] = json!(code),
            }
            value
        }
        SitePolicyCommand::Edit(SitePolicyEdit::Rebase { draft, revision }) => {
            json!({"draft":draft,"revision":revision,"rebase":true})
        }
        SitePolicyCommand::Edit(SitePolicyEdit::Rule {
            target,
            new,
            pattern,
            priority,
            remove_interaction,
            enabled,
        }) => {
            let mut value = json!({"new":new,"pattern":pattern,"priority":priority,"remove_interaction":remove_interaction,"enabled":enabled});
            rule_fields(&mut value, &target);
            value
        }
        SitePolicyCommand::Import {
            path,
            temporary,
            replace_existing,
        } => json!({"path":path,"temporary":temporary,"replace_existing":replace_existing}),
        SitePolicyCommand::Preview { draft, tab } => json!({"draft":draft,"tab":tab}),
        SitePolicyCommand::Apply {
            draft,
            revision,
            activate,
        } => json!({"draft":draft,"revision":revision,"activate":activate}),
        SitePolicyCommand::Export {
            path,
            subject,
            overwrite,
        } => {
            let mut value = json!({"path":path,"overwrite":overwrite});
            match subject {
                SitePolicyExportSubject::Rule(rule) => rule_fields(&mut value, &rule),
                SitePolicyExportSubject::Draft(draft) => value["draft"] = json!(draft),
            }
            value
        }
        SitePolicyCommand::Discard { draft } => json!({"draft":draft}),
    };
    Ok(Some(super::EncodedCommand::new(arguments)))
}

pub(crate) fn argument_names(command: &str) -> Option<&'static [&'static str]> {
    Some(match command {
        "site-policy" => &["url"],
        "site-policy-list" => &["url", "all"],
        "site-policy-explain" => &["url", "source", "id", "key", "code"],
        "site-policy-edit" => &[
            "source",
            "id",
            "new",
            "pattern",
            "priority",
            "remove_interaction",
            "enabled",
            "draft",
            "revision",
            "rebase",
        ],
        "site-policy-import" => &["path", "temporary", "replace_existing"],
        "site-policy-preview" => &["draft", "tab"],
        "site-policy-apply" => &["draft", "revision", "activate"],
        "site-policy-export" => &["path", "draft", "source", "id", "overwrite"],
        "site-policy-discard" => &["draft"],
        _ => return None,
    })
}

pub(crate) fn is_site_policy_command(command: &str) -> bool {
    argument_names(command).is_some()
}

pub(crate) fn decode(envelope: &crate::CommandEnvelope) -> Result<ParsedCommand, String> {
    let name = envelope.command.as_str();
    crate::validate_command_argument_fields(name, &envelope.arguments)?;
    let mut arguments = Vec::new();
    for field in crate::command_argument_names(name) {
        let Some(value) = envelope
            .arguments
            .get(*field)
            .filter(|value| !value.is_null())
        else {
            continue;
        };
        let positional = *field == "path"
            || (*field == "draft"
                && matches!(
                    name,
                    "site-policy-preview" | "site-policy-apply" | "site-policy-discard"
                ));
        let flag = match *field {
            "temporary" => "--temp".into(),
            field => format!("--{}", field.replace('_', "-")),
        };
        match *field {
            "all" | "new" | "remove_interaction" | "temporary" | "replace_existing"
            | "activate" | "overwrite" | "rebase" => {
                let enabled = value
                    .as_bool()
                    .ok_or_else(|| format!("{field} must be a boolean"))?;
                if enabled {
                    arguments.push(flag);
                }
            }
            "enabled" => {
                let enabled = value
                    .as_bool()
                    .ok_or_else(|| "enabled must be a boolean".to_owned())?;
                arguments.extend([flag, enabled.to_string()]);
            }
            "revision" => {
                let revision = value
                    .as_u64()
                    .filter(|revision| *revision > 0)
                    .ok_or_else(|| "revision must be a positive integer".to_owned())?;
                arguments.extend([flag, revision.to_string()]);
            }
            "priority" => {
                let priority = value
                    .as_i64()
                    .and_then(|priority| i32::try_from(priority).ok())
                    .ok_or_else(|| "priority must be a signed 32-bit integer".to_owned())?;
                arguments.extend([flag, priority.to_string()]);
            }
            _ => {
                let text = value
                    .as_str()
                    .ok_or_else(|| format!("{field} must be text"))?;
                if !positional {
                    arguments.push(flag);
                }
                arguments.push(text.into());
            }
        }
    }
    let command = ParsedCommand {
        name: name.into(),
        arguments,
    };
    parse_site_policy_command(&command)?;
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_command_invocation, encode_command};

    #[test]
    fn all_management_forms_round_trip_through_the_shared_grammar() {
        for text in [
            "site-policy",
            "site-policy-list --all",
            "site-policy-explain --key Return",
            "site-policy-edit --source generated --id player --new --pattern https://example.test/* --enabled false",
            "site-policy-edit --draft draft-1 --revision 3 --rebase",
            "site-policy-import 'local policy.json' --temp --replace-existing",
            "site-policy-preview draft-1 --tab tab-1",
            "site-policy-apply draft-1 --revision 3 --activate",
            "site-policy-export out.json --source authored:2 --id player --overwrite",
            "site-policy-export out.json --draft draft-1",
            "site-policy-discard draft-1",
        ] {
            let command =
                ferric_browser_core::parse_chain(text, ferric_browser_core::ParseInput::Cli)
                    .unwrap()
                    .remove(0);
            let expected = parse_site_policy_command(&command).unwrap();
            let envelope = encode_command(&command).unwrap();
            let decoded = decode_command_invocation(&envelope).unwrap();
            assert_eq!(
                parse_site_policy_command(&decoded.command).unwrap(),
                expected,
                "{text}"
            );
        }
    }

    #[test]
    fn ipc_rejects_wrong_scalar_types_unknown_fields_and_conflicting_authority() {
        for request in [
            json!({"command":"site-policy-apply","arguments":{"draft":"d","revision":"1"}}),
            json!({"command":"site-policy-apply","arguments":{"draft":"d","revision":0}}),
            json!({"command":"site-policy-import","arguments":{"path":"x","temporary":"true"}}),
            json!({"command":"site-policy-edit","arguments":{"source":"generated","id":"x","enabled":"false"}}),
            json!({"command":"site-policy-export","arguments":{"path":"x","draft":"d","source":"generated","id":"x"}}),
            json!({"command":"site-policy-explain","arguments":{"source":"generated"}}),
            json!({"command":"site-policy-edit","arguments":{"draft":"d","revision":1,"rebase":true,"pattern":"https://example.test/*"}}),
            json!({"command":"site-policy-import","arguments":{"path":"x","set":{"permission":"allow"}}}),
        ] {
            assert!(decode_command_invocation(&request).is_err(), "{request}");
        }
    }
}
