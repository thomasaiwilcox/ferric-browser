//! The shared, side-effect-free grammar for Site Policies management.
use std::collections::BTreeMap;

use crate::{LogicalKeyToken, ParsedCommand, canonical_origin};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SitePolicySource {
    Generated,
    Temporary,
    Authored(u32),
}

impl SitePolicySource {
    /// Parses a catalogue identity, never a filesystem path.
    ///
    /// # Errors
    /// Rejects unknown source kinds and malformed authored document numbers.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "generated" => Ok(Self::Generated),
            "temporary" => Ok(Self::Temporary),
            _ => value
                .strip_prefix("authored:")
                .filter(|number| {
                    !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
                })
                .and_then(|number| number.parse().ok())
                .map(Self::Authored)
                .ok_or_else(|| {
                    "source must be generated, temporary, or authored:DOCUMENT_ID".into()
                }),
        }
    }

    #[must_use]
    pub fn token(&self) -> String {
        match self {
            Self::Generated => "generated".into(),
            Self::Temporary => "temporary".into(),
            Self::Authored(document) => format!("authored:{document}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SitePolicyRuleRef {
    pub source: SitePolicySource,
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SitePolicyExplanation {
    Effective,
    Key(LogicalKeyToken),
    Code(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SitePolicyEdit {
    Rule {
        target: SitePolicyRuleRef,
        new: bool,
        pattern: Option<String>,
        priority: Option<i32>,
        remove_interaction: bool,
        enabled: Option<bool>,
    },
    Rebase {
        draft: String,
        revision: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SitePolicyExportSubject {
    Rule(SitePolicyRuleRef),
    Draft(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SitePolicyCommand {
    Open {
        url: Option<String>,
    },
    List {
        url: Option<String>,
        all: bool,
    },
    Explain {
        url: Option<String>,
        rule: Option<SitePolicyRuleRef>,
        subject: SitePolicyExplanation,
    },
    Edit(SitePolicyEdit),
    Import {
        path: String,
        temporary: bool,
        replace_existing: bool,
    },
    Preview {
        draft: String,
        tab: Option<String>,
    },
    Apply {
        draft: String,
        revision: u64,
        activate: bool,
    },
    Export {
        path: String,
        subject: SitePolicyExportSubject,
        overwrite: bool,
    },
    Discard {
        draft: String,
    },
}

struct Options<'a> {
    values: BTreeMap<&'a str, &'a str>,
    positional: Option<&'a str>,
}

impl<'a> Options<'a> {
    fn parse(
        arguments: &'a [String],
        values: &[&str],
        flags: &[&str],
        positional: bool,
    ) -> Result<Self, String> {
        let mut result = Self {
            values: BTreeMap::new(),
            positional: None,
        };
        let mut input = arguments.iter();
        for value in arguments {
            if value.is_empty() || value.len() > 64 * 1024 || value.chars().any(char::is_control) {
                return Err("site-policy arguments must be bounded nonempty text without control characters".into());
            }
        }
        while let Some(argument) = input.next() {
            if result.values.contains_key(argument.as_str()) {
                return Err(format!("duplicate option {argument}"));
            }
            if values.contains(&argument.as_str()) {
                let value = input
                    .next()
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| format!("{argument} requires a value"))?;
                result.values.insert(argument, value);
            } else if flags.contains(&argument.as_str()) {
                result.values.insert(argument, "true");
            } else if positional && !argument.starts_with('-') && result.positional.is_none() {
                result.positional = Some(argument);
            } else {
                return Err(format!("unknown option or extra argument: {argument}"));
            }
        }
        Ok(result)
    }

    fn get(&self, key: &str) -> Option<&'a str> {
        self.values.get(key).copied()
    }
    fn has(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }
    fn required(&self, key: &str) -> Result<&'a str, String> {
        self.get(key).ok_or_else(|| format!("{key} is required"))
    }
    fn positional(&self, label: &str) -> Result<String, String> {
        self.positional
            .map(str::to_owned)
            .ok_or_else(|| format!("{label} is required"))
    }
    fn url(&self) -> Result<Option<String>, String> {
        self.get("--url")
            .map(|url| {
                canonical_origin(url).ok_or_else(|| "--url requires an HTTP(S) URL".to_owned())?;
                Ok(url.to_owned())
            })
            .transpose()
    }
    fn rule(&self) -> Result<Option<SitePolicyRuleRef>, String> {
        match (self.get("--source"), self.get("--id")) {
            (None, None) => Ok(None),
            (Some(source), Some(id)) => Ok(Some(SitePolicyRuleRef {
                source: SitePolicySource::parse(source)?,
                id: id.into(),
            })),
            _ => Err("--source and --id must be provided together".into()),
        }
    }
    fn revision(&self) -> Result<u64, String> {
        self.required("--revision")?
            .parse::<u64>()
            .ok()
            .filter(|revision| *revision > 0)
            .ok_or_else(|| "--revision requires a positive integer".into())
    }
}

/// Parses one of the nine management commands. This never reads files, creates
/// drafts, validates live DOM, writes configuration or executes suggested text.
///
/// # Errors
/// Rejects unknown/duplicate options, invalid combinations and incomplete identities.
pub fn parse_site_policy_command(command: &ParsedCommand) -> Result<SitePolicyCommand, String> {
    if command.arguments.len() > 256
        || command.arguments.iter().map(String::len).sum::<usize>() > 64 * 1024
    {
        return Err("site-policy command exceeds the command input bounds".into());
    }
    let args = &command.arguments;
    match command.name.as_str() {
        "site-policy" => {
            let options = Options::parse(args, &["--url"], &[], false)?;
            Ok(SitePolicyCommand::Open {
                url: options.url()?,
            })
        }
        "site-policy-list" => {
            let options = Options::parse(args, &["--url"], &["--all"], false)?;
            Ok(SitePolicyCommand::List {
                url: options.url()?,
                all: options.has("--all"),
            })
        }
        "site-policy-explain" => parse_explain(args),
        "site-policy-edit" => parse_edit(args).map(SitePolicyCommand::Edit),
        "site-policy-import" => {
            let options = Options::parse(args, &[], &["--temp", "--replace-existing"], true)?;
            Ok(SitePolicyCommand::Import {
                path: options.positional("PATH")?,
                temporary: options.has("--temp"),
                replace_existing: options.has("--replace-existing"),
            })
        }
        "site-policy-preview" => {
            let options = Options::parse(args, &["--tab"], &[], true)?;
            Ok(SitePolicyCommand::Preview {
                draft: options.positional("DRAFT_ID")?,
                tab: options.get("--tab").map(str::to_owned),
            })
        }
        "site-policy-apply" => {
            let options = Options::parse(args, &["--revision"], &["--activate"], true)?;
            Ok(SitePolicyCommand::Apply {
                draft: options.positional("DRAFT_ID")?,
                revision: options.revision()?,
                activate: options.has("--activate"),
            })
        }
        "site-policy-export" => parse_export(args),
        "site-policy-discard" => {
            let options = Options::parse(args, &[], &[], true)?;
            Ok(SitePolicyCommand::Discard {
                draft: options.positional("DRAFT_ID")?,
            })
        }
        _ => Err("unknown site-policy command".into()),
    }
}

fn parse_explain(args: &[String]) -> Result<SitePolicyCommand, String> {
    let options = Options::parse(
        args,
        &["--url", "--source", "--id", "--key", "--code"],
        &[],
        false,
    )?;
    let subject = match (options.get("--key"), options.get("--code")) {
        (Some(key), None) => SitePolicyExplanation::Key(
            LogicalKeyToken::parse(key).map_err(|error| error.to_string())?,
        ),
        (None, Some(code))
            if code.len() <= crate::MAX_HINT_CODE_BYTES
                && code.bytes().all(|byte| byte.is_ascii_graphic()) =>
        {
            SitePolicyExplanation::Code(code.into())
        }
        (None, Some(_)) => return Err("--code requires 1–8 printable ASCII characters".into()),
        (None, None) => SitePolicyExplanation::Effective,
        _ => return Err("--key and --code are mutually exclusive".into()),
    };
    Ok(SitePolicyCommand::Explain {
        url: options.url()?,
        rule: options.rule()?,
        subject,
    })
}

fn parse_edit(args: &[String]) -> Result<SitePolicyEdit, String> {
    let options = Options::parse(
        args,
        &[
            "--source",
            "--id",
            "--pattern",
            "--priority",
            "--enabled",
            "--draft",
            "--revision",
        ],
        &["--new", "--remove-interaction", "--rebase"],
        false,
    )?;
    if options.has("--draft") || options.has("--revision") || options.has("--rebase") {
        if options.values.len() != 3 || !options.has("--rebase") {
            return Err(
                "rebase requires exactly --draft DRAFT_ID --revision REVISION --rebase".into(),
            );
        }
        return Ok(SitePolicyEdit::Rebase {
            draft: options.required("--draft")?.into(),
            revision: options.revision()?,
        });
    }
    let target = options
        .rule()?
        .ok_or_else(|| "--source and --id are required".to_owned())?;
    let new = options.has("--new");
    let remove_interaction = options.has("--remove-interaction");
    if new && !options.has("--pattern") {
        return Err("--new requires --pattern".into());
    }
    if remove_interaction
        && ["--new", "--pattern", "--priority", "--enabled"]
            .iter()
            .any(|key| options.has(key))
    {
        return Err(
            "--remove-interaction cannot be combined with new, retargeting, or enabled options"
                .into(),
        );
    }
    let priority = options
        .get("--priority")
        .map(|value| {
            value
                .parse::<i32>()
                .map_err(|_| "--priority requires a signed 32-bit integer".to_owned())
        })
        .transpose()?;
    let enabled = options
        .get("--enabled")
        .map(|value| match value {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err("--enabled requires true or false".to_owned()),
        })
        .transpose()?;
    Ok(SitePolicyEdit::Rule {
        target,
        new,
        pattern: options.get("--pattern").map(str::to_owned),
        priority,
        remove_interaction,
        enabled,
    })
}

fn parse_export(args: &[String]) -> Result<SitePolicyCommand, String> {
    let options = Options::parse(
        args,
        &["--draft", "--source", "--id"],
        &["--overwrite"],
        true,
    )?;
    let subject = match (options.get("--draft"), options.rule()?) {
        (Some(draft), None) => SitePolicyExportSubject::Draft(draft.into()),
        (None, Some(rule)) => SitePolicyExportSubject::Rule(rule),
        _ => return Err("export requires exactly --draft or the --source/--id pair".into()),
    };
    Ok(SitePolicyCommand::Export {
        path: options.positional("PATH")?,
        subject,
        overwrite: options.has("--overwrite"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(text: &str) -> Result<SitePolicyCommand, String> {
        let commands =
            crate::parse_chain(text, crate::ParseInput::Cli).map_err(|error| error.to_string())?;
        parse_site_policy_command(&commands[0])
    }

    #[test]
    fn required_command_forms_and_tagged_rebase_are_typed() {
        for command in [
            "site-policy",
            "site-policy-list --all --url https://example.test/page",
            "site-policy-explain --source authored:0 --id navigation --key Return",
            "site-policy-edit --source generated --id navigation --new --pattern https://example.test/* --priority -4 --enabled true",
            "site-policy-import 'local policy.json' --temp --replace-existing",
            "site-policy-preview draft-1 --tab tab-1",
            "site-policy-apply draft-1 --revision 2 --activate",
            "site-policy-export copy.json --draft draft-1 --overwrite",
            "site-policy-discard draft-1",
            "site-policy-edit --draft draft-1 --revision 2 --rebase",
        ] {
            assert!(parse(command).is_ok(), "{command}");
        }
        assert!(matches!(
            parse("site-policy-edit --draft draft-1 --revision 2 --rebase"),
            Ok(SitePolicyCommand::Edit(SitePolicyEdit::Rebase {
                revision: 2,
                ..
            }))
        ));
        assert!(
            matches!(parse("site-policy-explain --key Return"), Ok(SitePolicyCommand::Explain { subject: SitePolicyExplanation::Key(key), .. }) if key.as_str()=="Enter")
        );
    }

    #[test]
    fn malformed_authority_and_ambiguous_operations_are_rejected() {
        for command in [
            "site-policy --url file:///tmp/config",
            "site-policy --url https://a.test --url https://b.test",
            "site-policy-list --all --all",
            "site-policy-explain --source generated",
            "site-policy-explain --source /tmp/config --id x",
            "site-policy-explain --key k --code a",
            "site-policy-edit --source generated --id x --new",
            "site-policy-edit --source generated --id x --enabled yes",
            "site-policy-edit --source generated --id x --remove-interaction --priority 1",
            "site-policy-edit --source generated --id x --draft d --revision 1 --rebase",
            "site-policy-edit --draft d --revision 1",
            "site-policy-edit --draft d --revision 0 --rebase",
            "site-policy-apply d --revision 1 --revision 2",
            "site-policy-apply d --activate",
            "site-policy-export x --draft d --source generated --id x",
            "site-policy-export x --overwrite",
            "site-policy-import x --include y",
            "site-policy-discard d extra",
        ] {
            assert!(parse(command).is_err(), "{command}");
        }
    }
}
