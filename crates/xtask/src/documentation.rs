use super::{
    ActionRegistry, BindingRegistry, CommandRegistry, Config, Mode, Path, SettingRegistry, fs,
};
use std::fmt::Write as _;

pub(super) fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

pub(super) fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "normal",
        Mode::Insert => "insert",
        Mode::InsertLocked => "insert-locked",
        Mode::Command => "command",
        Mode::Search => "search",
        Mode::Hint => "hint",
        Mode::Grid => "grid",
        Mode::Caret => "caret",
        Mode::PassThrough => "pass-through",
        Mode::PassThroughOnce => "pass-through-once",
    }
}

pub(super) fn render_binding_docs() -> Result<String, String> {
    let bindings = BindingRegistry::default_v1(CommandRegistry::default_v1())
        .map_err(|error| format!("could not build default bindings: {error}"))?;
    let mut document = String::from(concat!(
        "# Keyboard bindings\n\n",
        "This table is generated from the built-in binding registry. Configuration overrides ",
        "are applied on top of these defaults at runtime.\n\n",
        "Press `i` for Transient Insert (`INSERT`): ordinary Enter reaches the page, then returns to Normal.\n",
        "Use Shift+Enter to continue composing; other modified Enter combinations also stay in Insert.\n",
        "Press `I` (Shift+i) for Locked Insert (`INSERT LOCKED`), where Enter keeps editing.\n",
        "Escape exits either variant. For example, use `i`, type a search, Enter, then `j` to browse;\n",
        "use `I` for document editing with repeated paragraphs. Neither command focuses an arbitrary input.\n",
        "Both variants reset when changing tabs or navigating; Transient Insert also ends when its editable context loses focus.\n\n",
        "Ctrl+C/Ctrl+X copy/cut, Ctrl+V pastes, and Ctrl+Shift+V pastes plain text. For persistent webpage shortcuts, press `; V` for `PASS THROUGH`.\n",
        "Press `; v` for `PASS NEXT KEY`: the next key, its repeats, and release reach the page, then Normal resumes.\n",
        "Modifier keys alone do not finish one-key forwarding. `Ctrl+Escape` returns either variant to Normal; ordinary Escape reaches the page.\n",
        "Both reset when changing tabs or navigating; one-key forwarding also cancels when the window loses activation.\n",
        "The entry shortcuts are configurable Normal-mode bindings; `Ctrl+Escape` is a reserved recovery shortcut.\n",
        "See [Webpage shortcuts](webpage-shortcuts.md) for examples and the existing emergency/media-key exceptions.\n\n",
        "| Mode | Key chain | Command |\n",
        "| --- | --- | --- |\n",
    ));
    for binding in bindings.definitions() {
        writeln!(
            document,
            "| `{}` | `{}` | `{}` |",
            mode_name(binding.mode),
            markdown_cell(&binding.keys.join(" ")),
            markdown_cell(&binding.command),
        )
        .map_err(|_| "could not format binding documentation".to_owned())?;
    }
    Ok(document)
}

pub(super) fn render_action_docs() -> Result<String, String> {
    let mut document = String::from(concat!(
        "# Typed actions\n\n",
        "This table is generated from the shared action registry. UI, hints, IPC, the ",
        "universal switcher, and userscripts use these stable action identities.\n\n",
        "| ID | Subject | Verb | Command | Arguments | Examples | Sources | Required capabilities | Completion provider | Availability predicate | Effect | Confirmation |\n",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    ));
    for action in ActionRegistry::default_v1().definitions() {
        let arguments = action
            .arguments
            .iter()
            .map(|argument| {
                format!(
                    "{}:{}{}",
                    argument.name,
                    format!("{:?}", argument.kind).to_ascii_lowercase(),
                    if argument.required { " (required)" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let examples = action
            .examples
            .iter()
            .map(|example| format!("`{example}`"))
            .collect::<Vec<_>>()
            .join("<br>");
        let sources = action
            .sources
            .iter()
            .map(|source| source.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            document,
            "| `{}` | `{}` | `{}` | `{}` | {} | {} | {} | {} | `{}` | `{}` | `{}` | `{}` |",
            markdown_cell(&action.id),
            action.subject.as_str(),
            markdown_cell(&action.verb),
            markdown_cell(&action.command),
            markdown_cell(&arguments),
            markdown_cell(&examples),
            markdown_cell(&sources),
            markdown_cell(&action.required_capabilities().join(", ")),
            action.completion_provider(),
            action.availability_predicate(),
            format!("{:?}", action.effect).to_ascii_lowercase(),
            action.confirmation.as_str(),
        )
        .map_err(|_| "could not format action documentation".to_owned())?;
    }
    Ok(document)
}

pub(super) fn render_user_docs() -> Result<(String, String, String, String), String> {
    let registry = CommandRegistry::default_v1();
    let mut commands = String::from(concat!(
        "# Ferric Browser commands\n\n",
        "Generated by `cargo xtask docs` from `ferric-browser-core::CommandRegistry::default_v1`. ",
        "Edit the registry, not this file.\n\n",
        "| Command | Aliases | Modes | Count | Scope | Effect | Arguments | Description |\n",
        "| --- | --- | --- | --- | --- | --- | --- | --- |\n",
    ));
    for definition in registry.definitions() {
        let aliases = definition.aliases.join(", ");
        let modes = definition
            .modes
            .iter()
            .map(|mode| mode_name(*mode))
            .collect::<Vec<_>>()
            .join(", ");
        let count = format!("{:?}", definition.count);
        let arguments = definition
            .arguments
            .iter()
            .map(|argument| {
                format!(
                    "{}:{}{}",
                    argument.name,
                    format!("{:?}", argument.kind).to_ascii_lowercase(),
                    if argument.required { " (required)" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            commands,
            "| `{}` | {} | {} | {} | {:?} | {:?} | {} | {} |",
            definition.name,
            markdown_cell(&aliases),
            markdown_cell(&modes),
            markdown_cell(&count),
            definition.scope,
            definition.effect,
            markdown_cell(&arguments),
            markdown_cell(&definition.description),
        )
        .map_err(|_| "could not format command documentation".to_owned())?;
    }
    commands.push_str("\n## Feature manual\n\nThe native Guide displays effective configured bindings. These explanations share its authored feature metadata.\n\n");
    for definition in registry.definitions() {
        let article = ferric_browser_core::guide_article(&definition.name)
            .ok_or_else(|| format!("missing manual for {}", definition.name))?;
        writeln!(
            commands,
            "### `{}`\n\n{}\n\n{}\n\nExit and recovery: {}\n",
            definition.name, definition.description, article.explanation, article.recovery
        )
        .map_err(|_| "could not format feature manual".to_owned())?;
    }
    commands.truncate(commands.trim_end().len());
    commands.push('\n');
    let mut configuration = String::from(concat!(
        "# Ferric Browser configuration\n\n",
        "Generated by `cargo xtask docs` from the typed configuration defaults and setting registry. ",
        "Edit the schema and defaults in `ferric-browser-config`, not this file.\n\n",
        "## Setting registry\n\n",
        "| Key | Type | Default | Scopes | Apply time | Prerequisite | Sensitivity |\n",
        "| --- | --- | --- | --- | --- | --- | --- |\n",
    ));
    for metadata in SettingRegistry::default_v1().definitions() {
        writeln!(
            configuration,
            "| `{}` | {} | `{}` | {} | {} | {} | {} |",
            metadata.key,
            metadata.value_type,
            markdown_cell(metadata.default_value),
            metadata.supported_scopes.join(", "),
            metadata.apply_time,
            metadata.prerequisite,
            metadata.sensitivity,
        )
        .map_err(|_| "could not format configuration metadata".to_owned())?;
    }
    configuration.push_str(concat!(
        "\n## Runtime persistence\n\n",
        "Runtime settings and bindings are prepared and validated before their generated override file is replaced. A rejected validation or write before replacement leaves the old effective configuration and interaction sessions intact. Accepted command results and `config.changed` events include `persistence`: `memory-only`, `durable`, or `durability-uncertain`. The last outcome means replacement committed and memory matches the visible file, but the final directory sync could not confirm crash durability. It is not a rejected change and does not trigger automatic rollback or retry. Selector commands can first return `pending-validation`; their eventual publication is reported by `config.changed`, while rejection is shown in browser status.\n",
    ));
    append_download_folder_docs(&mut configuration);
    append_hint_selector_docs(&mut configuration);
    configuration.push_str(concat!(
        "\n## Validated hint proxies\n\n",
        "`hints.proxy_targets` accepts `off` (the default) or `validated` and can be set globally, in a profile, or with a site rule. `validated` enables only the browser's checked editor-container and native label/control pairings; it does not let selectors nominate hidden receivers or grant a general visibility exception. Editor proxies must remain connected, enabled, editable, non-inert, and visible through their editor container. The feature does not read editor contents or change page focusability styles. Structural fixtures do not establish compatibility with a particular editor framework build.\n",
    ));
    configuration.push_str("\n## Starter configuration\n\n```toml\n");
    let starter = toml::to_string_pretty(&Config::default())
        .map_err(|error| format!("could not serialize default configuration: {error}"))?;
    configuration.push_str(&starter);
    configuration.push_str("```\n");
    Ok((
        commands,
        configuration,
        render_binding_docs()?,
        render_action_docs()?,
    ))
}

pub(super) fn append_download_folder_docs(output: &mut String) {
    output.push_str(r#"
## Download folder

`downloads.directory` accepts `xdg-downloads` or an absolute folder path, including spaces and Unicode:

```toml
[downloads]
directory = "/home/user/Downloads/Reports"
```

For a temporary change in the current window:

```text
:set --temp downloads.directory="/home/user/Downloads/Reports"
```

Use `:set --temp downloads.directory=xdg-downloads` for the desktop Downloads folder. The existing tagged form `{ path = "/absolute/folder" }` remains supported. Relative paths and control characters are rejected; a rejected change leaves the current folder unchanged.
"#);
}

pub(super) fn append_hint_selector_docs(configuration: &mut String) {
    configuration.push_str(concat!(
        "\n## Hint selector policies\n\n",
        "Configure the seven existing Hint families (`all`, `links`, `inputs`, `buttons`, `images`, `media`, and `scrollables`) with `mode = \"extend\"` or `mode = \"replace\"`. With no entry, Ferric uses that family's built-in selectors. `extend` adds unique selectors in stable order; `replace` uses only the listed selectors. An empty `replace` list disables that family. For example:\n\n",
        "```toml\n[hints.selectors.buttons]\nmode = \"extend\"\nselectors = ['button[data-action]']\n```\n\n",
        "The effective global/profile/runtime settings form the base policy. Matching site rules then apply in the repository's existing priority and source order; each family layers independently, and `all` is not an implicit union of the other families. `scrollables` must still match an actually scrollable element. Selectors control discovery only: semantic type checks, visibility, disabled/inert checks, and the existing action or proxy guards still decide what a target can do.\n\n",
        "Each authored family entry allows at most 16 selectors, each up to 1,024 UTF-8 bytes, and at most 16 KiB of selector strings. A site rule has a shared 16 KiB selector-string budget across its families. After layering, each family allows at most 32 selectors and 32 KiB; the resolved policy across all families is also capped at 32 KiB. Empty strings, control characters, and duplicate selectors in one entry are rejected. The WebEngine CSS parser checks syntax before a selector setting is committed.\n\n",
        "A Hint snapshot inspects at most 50,000 DOM nodes and returns at most 5,000 candidates. Truncation is reported in the Hint status. These limits bound traversal and output; one CSS selector match is synchronous and cannot be interrupted by a timeout.\n",
    ));
}

pub(super) fn generate_user_docs() -> Result<(), String> {
    let output_dir = Path::new("docs/user");
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("could not create {}: {error}", output_dir.display()))?;
    let (commands, configuration, bindings, actions) = render_user_docs()?;
    fs::write(output_dir.join("commands.md"), commands)
        .map_err(|error| format!("could not write generated commands: {error}"))?;
    fs::write(output_dir.join("configuration.md"), configuration)
        .map_err(|error| format!("could not write generated configuration: {error}"))?;
    fs::write(output_dir.join("bindings.md"), bindings)
        .map_err(|error| format!("could not write generated bindings: {error}"))?;
    fs::write(output_dir.join("actions.md"), actions)
        .map_err(|error| format!("could not write generated actions: {error}"))?;
    println!("generated docs/user/commands.md, configuration.md, bindings.md, and actions.md");
    Ok(())
}

pub(super) fn check_generated_docs() -> Result<(), String> {
    let (commands, configuration, bindings, actions) = render_user_docs()?;
    for (name, expected) in [
        ("commands.md", commands),
        ("configuration.md", configuration),
        ("bindings.md", bindings),
        ("actions.md", actions),
    ] {
        let path = Path::new("docs/user").join(name);
        let actual = fs::read_to_string(&path)
            .map_err(|error| format!("could not read generated {}: {error}", path.display()))?;
        if actual != expected {
            return Err(format!(
                "generated documentation is stale: run `cargo xtask docs` for {}",
                path.display()
            ));
        }
    }
    Ok(())
}
