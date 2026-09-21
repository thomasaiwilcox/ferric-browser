use std::{
    collections::BTreeSet,
    fmt::Write as _,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::Path,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use browser_config::{Config, setting_metadata_all};
use browser_core::{ActionRegistry, BindingTrie, CommandRegistry, Mode};

mod fixture_server;
mod perf;

fn main() {
    let mut arguments = std::env::args();
    let _program = arguments.next();
    let command = arguments.next().unwrap_or_else(|| "help".into());
    let remaining = arguments.collect::<Vec<_>>();
    let result = match command.as_str() {
        "check" => run_check(),
        "test" => run_test(&remaining),
        "package" => run_package(&remaining),
        "docs" => generate_user_docs(),
        "fixture-server" => fixture_server::run(&remaining),
        "perf" => perf::run(&remaining),
        "help" => {
            println!("cargo xtask check");
            println!("  Run deterministic project checks.");
            println!("cargo xtask test engine");
            println!("  Build and run the QtWebEngine adapter tests.");
            println!("cargo xtask test adapter");
            println!("  Run the bounded offscreen Qt/QML adapter integration smoke.");
            println!("cargo xtask test core");
            println!("  Run the Qt-independent browser-core contract tests.");
            println!("cargo xtask test fuzz");
            println!(
                "  Compile and run every standalone libFuzzer target against bounded corpus seeds."
            );
            println!("cargo xtask test storage");
            println!("  Run SQLite and session interruption/recovery tests.");
            println!("cargo xtask test fixtures");
            println!("  Run deterministic local web fixture route tests.");
            println!("cargo xtask test repeat");
            println!("  Repeat the Qt-independent core/storage contract suite three times.");
            println!("cargo xtask test accessibility");
            println!("  Validate the browser-owned QML accessibility contract markers.");
            println!("cargo xtask test wayland");
            println!("  Run the bounded disposable nested-Wayland startup smoke test.");
            println!("cargo xtask test blocking");
            println!("  Run the real Qt/WebEngine cached-list block/exception smoke.");
            println!("cargo xtask test config");
            println!("  Run the disposable Wayland configuration reload smoke.");
            println!("cargo xtask test mpris");
            println!("  Probe the live session-D-Bus MPRIS bridge and PlayPause path.");
            println!("cargo xtask test compatibility");
            println!("  Validate the versioned Google compatibility qualification report.");
            println!("cargo xtask package arch");
            println!("  Validate Arch package metadata and desktop integration files.");
            println!("cargo xtask package artifacts");
            println!("  Validate the checked-in stable-release artifact manifest.");
            println!("cargo xtask docs");
            println!("  Generate user command and configuration documentation from registries.");
            println!(
                "cargo run --release -p xtask -- perf [--suite core|ui] [--samples N] [--output PATH]"
            );
            println!(
                "  Measure optimized core latency or disposable live UI/process-tree performance."
            );
            println!("cargo xtask fixture-server [--bind ADDRESS] [--once] [--https]");
            println!("  Run the bounded loopback HTTP fixture server (or its dev HTTPS wrapper).");
            Ok(())
        }
        other => Err(format!("unknown xtask command: {other}")),
    };
    if let Err(error) = result {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "normal",
        Mode::Insert => "insert",
        Mode::Command => "command",
        Mode::Search => "search",
        Mode::Hint => "hint",
        Mode::Caret => "caret",
        Mode::PassThrough => "pass-through",
    }
}

fn render_binding_docs() -> Result<String, String> {
    let bindings = BindingTrie::default_v1(CommandRegistry::default_v1())
        .map_err(|error| format!("could not build default bindings: {error}"))?;
    let mut document = String::from(concat!(
        "# Keyboard bindings\n\n",
        "This table is generated from the built-in binding registry. Configuration overrides ",
        "are applied on top of these defaults at runtime.\n\n",
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

fn render_action_docs() -> Result<String, String> {
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

fn render_user_docs() -> Result<(String, String, String, String), String> {
    let registry = CommandRegistry::default_v1();
    let mut commands = String::from(concat!(
        "# RustBrowser commands\n\n",
        "Generated by `cargo xtask docs` from `browser-core::CommandRegistry::default_v1`. ",
        "Edit the registry, not this file.\n\n",
        "| Command | Aliases | Modes | Count | Scope | Effect | Arguments | Description |\n",
        "| --- | --- | --- | --- | --- | --- | --- | --- |\n",
    ));
    for definition in registry.definitions() {
        let aliases = definition.aliases.join(", ");
        let modes = definition
            .modes
            .iter()
            .map(|mode| format!("{mode:?}").to_ascii_lowercase())
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
    let mut configuration = String::from(concat!(
        "# RustBrowser configuration\n\n",
        "Generated by `cargo xtask docs` from the typed configuration defaults and setting registry. ",
        "Edit the schema and defaults in `browser-config`, not this file.\n\n",
        "## Setting registry\n\n",
        "| Key | Type | Default | Scopes | Apply time | Prerequisite | Sensitivity |\n",
        "| --- | --- | --- | --- | --- | --- | --- |\n",
    ));
    for metadata in setting_metadata_all() {
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

fn generate_user_docs() -> Result<(), String> {
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

fn check_generated_docs() -> Result<(), String> {
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

fn run_check() -> Result<(), String> {
    let command_registry = CommandRegistry::default_v1();
    ActionRegistry::default_v1()
        .validate_against_commands(&command_registry)
        .map_err(|error| format!("action registry validation failed: {error}"))?;
    check_requirements(
        Path::new("docs/DEVELOPMENT_SPEC.md"),
        Path::new("docs/requirements.csv"),
    )?;
    check_ui_thread_contract()?;
    check_ui_automation_boundary()?;
    check_accessibility_contract()?;
    check_renderer_failure_contract()?;
    check_compatibility_report()?;
    check_compatibility_diagnosis()?;
    check_release_freshness_policy()?;
    check_capability_matrix(Path::new("docs/architecture/capabilities.md"))?;
    check_generated_docs()?;
    validate_release_artifacts()?;
    validate_license_policy()?;
    run("cargo", &["fmt", "--all", "--", "--check"])?;
    run("cargo", &["test", "--workspace", "--locked"])
}

fn check_release_freshness_policy() -> Result<(), String> {
    let policy_path = Path::new("docs/RELEASE_POLICY.md");
    let policy = fs::read_to_string(policy_path)
        .map_err(|error| format!("could not read {}: {error}", policy_path.display()))?;
    for marker in [
        "Qt and QtWebEngine release/security advisories",
        "Chromium security notes and the QtWebEngine backport/patch model",
        "Rust dependency updates from the locked graph and their advisories",
        "supported distribution package updates and known-bad ranges",
        "applicable critical vulnerabilities",
        "mitigation or explicit release block",
        "A Chromium base version alone",
        "does not establish a security-patch level",
        "Offline repository tests and Rust dependency scanners do not",
        "constitute an engine-security review",
        "The browser never",
        "updates itself, runs a package manager",
    ] {
        if !policy.contains(marker) {
            return Err(format!(
                "release freshness policy is missing required rule: {marker}"
            ));
        }
    }

    let evidence_path = Path::new("docs/testing/m0-167-release-freshness.md");
    let evidence = fs::read_to_string(evidence_path)
        .map_err(|error| format!("could not read {}: {error}", evidence_path.display()))?;
    for marker in [
        "Status: in progress",
        "mandatory release-candidate review",
        "A real advisory review",
        "remain release responsibilities",
    ] {
        if !evidence.contains(marker) {
            return Err(format!(
                "release freshness evidence is missing required marker: {marker}"
            ));
        }
    }
    Ok(())
}

fn check_ui_thread_contract() -> Result<(), String> {
    let paths = [
        Path::new("crates/browser-engine-qt/src"),
        Path::new("crates/browser-engine-qt/qml"),
    ];
    let forbidden = ["block_on(", "futures::executor", "tokio::runtime"];
    for root in paths {
        let mut files = vec![root.to_owned()];
        while let Some(path) = files.pop() {
            let metadata = fs::metadata(&path)
                .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
            if metadata.is_dir() {
                for entry in fs::read_dir(&path)
                    .map_err(|error| format!("could not read {}: {error}", path.display()))?
                {
                    files.push(
                        entry
                            .map_err(|error| format!("could not enumerate source: {error}"))?
                            .path(),
                    );
                }
                continue;
            }
            if !matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("rs" | "qml")
            ) {
                continue;
            }
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            if let Some(token) = forbidden.iter().find(|token| source.contains(**token)) {
                return Err(format!(
                    "UI-thread contract violation: {} contains forbidden blocking executor call {token}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn check_accessibility_contract() -> Result<(), String> {
    let path = Path::new("crates/browser-engine-qt/qml/Main.qml");
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let required = [
        "Accessible.name:",
        "Accessible.description:",
        "Accessible.role: Accessible.Dialog",
        "Accessible.role: Accessible.EditableText",
        "Accessible.role: Accessible.List",
        "Accessible.role: Accessible.MenuItem",
        "Accessible.role: Accessible.PageTabList",
        "Accessible.role: Accessible.PageTab",
        "Accessible.role: Accessible.StatusBar",
        "Accessible.description: \"Current universal switcher result count\"",
        "Accessible.selected:",
        "focus: visible",
        "event.key === Qt.Key_Escape",
    ];
    for marker in required {
        if !source.contains(marker) {
            return Err(format!(
                "accessibility contract violation: {} is missing from {}",
                marker,
                path.display()
            ));
        }
    }
    if source.contains("Accessible.name: \"\"") {
        return Err(format!(
            "accessibility contract violation: an empty Accessible.name exists in {}",
            path.display()
        ));
    }
    Ok(())
}

fn check_renderer_failure_contract() -> Result<(), String> {
    let path = Path::new("crates/browser-engine-qt/qml/Main.qml");
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    for marker in [
        "function rendererTerminationName(status)",
        "WebEngineView.CrashedTerminationStatus",
        "WebEngineView.KilledTerminationStatus",
        "WebEngineView.AbnormalTerminationStatus",
        "function handleRendererProcessTerminated(",
        "ui.note_renderer_process_terminated(tabIndex)",
        "view.rendererFailureAt > 0",
        "now - view.rendererFailureAt <= 60000",
        "function reloadRendererFailure()",
        "ui.prepare_renderer_recovery(window.rendererFailureTabIndex)",
        "repeated crashes will not auto-reload",
        "onRenderProcessTerminated",
    ] {
        if !source.contains(marker) {
            return Err(format!(
                "renderer failure contract violation: {marker} is missing from {}",
                path.display()
            ));
        }
    }
    let handler = source
        .split_once("function handleRendererProcessTerminated(")
        .and_then(|(_, remainder)| remainder.split_once("function showRendererFailureForTab("))
        .map(|(handler, _)| handler)
        .ok_or_else(|| "renderer failure handler boundaries are malformed".to_owned())?;
    if handler.contains("view.reload()") {
        return Err(
            "renderer failure contract violation: termination callback must not auto-reload".into(),
        );
    }
    Ok(())
}

fn check_compatibility_report() -> Result<(), String> {
    let path = Path::new("docs/testing/compatibility-qualification.md");
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    for marker in [
        "Application commit/package:",
        "Qt build and Chromium security-patch level:",
        "Distribution/compositor/GPU baseline:",
        "Qualification date:",
        "Credential and external-effects policy:",
    ] {
        if !source.contains(marker) {
            return Err(format!(
                "compatibility qualification report is missing `{marker}`"
            ));
        }
    }
    let scenarios = [
        "G-01", "G-02", "G-03", "G-04", "G-05", "G-06", "G-07", "G-08", "G-09", "G-10", "G-11",
        "G-12", "G-13", "G-14", "G-15", "G-16", "G-17",
    ];
    let mut seen = BTreeSet::new();
    for scenario in scenarios {
        let row = source
            .lines()
            .find(|line| line.trim_start().starts_with(&format!("| {scenario} |")))
            .ok_or_else(|| format!("compatibility qualification report omits {scenario}"))?;
        let fields = row.split('|').map(str::trim).collect::<Vec<_>>();
        if fields.len() < 4 {
            return Err(format!(
                "compatibility report row for {scenario} is malformed"
            ));
        }
        let status = fields[2];
        if !matches!(status, "pass" | "fail" | "blocked" | "not-run") {
            return Err(format!(
                "compatibility report row for {scenario} has invalid status `{status}`"
            ));
        }
        if !seen.insert(scenario) {
            return Err(format!("compatibility report duplicates {scenario}"));
        }
    }
    Ok(())
}

fn check_compatibility_diagnosis() -> Result<(), String> {
    let path = Path::new("docs/testing/compatibility-diagnosis.md");
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    for heading in [
        "## Application and engine identity",
        "## Observed site and symptom",
        "## Sanitized reproduction",
        "## Minimal Qt reference comparison",
        "## Upstream issue and authorization",
        "## Scoped workaround",
        "## Removal condition",
    ] {
        if !source.contains(heading) {
            return Err(format!("compatibility diagnosis is missing {heading}"));
        }
    }
    for safety_marker in [
        "Do not record credentials",
        "Do not record cookies",
        "Do not copy a qutebrowser workaround",
    ] {
        if !source.contains(safety_marker) {
            return Err(format!(
                "compatibility diagnosis is missing safety rule {safety_marker}"
            ));
        }
    }
    Ok(())
}

fn check_capability_matrix(path: &Path) -> Result<(), String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let header = "| Feature | Public API/module | Minimum | Compiled support | Runtime probe | Tested package | Status | Limitation/test |";
    if !source.lines().any(|line| line.trim() == header) {
        return Err(format!(
            "capability matrix {} has an unexpected table header",
            path.display()
        ));
    }
    let allowed_statuses = [
        "available",
        "in-progress",
        "not-tested",
        "unknown",
        "unavailable",
        "blocked",
    ];
    let mut features = BTreeSet::new();
    for line in source.lines().filter(|line| line.starts_with('|')) {
        let fields = line
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if fields.iter().all(|field| field.chars().all(|c| c == '-')) {
            continue;
        }
        if fields.first() == Some(&"Feature") {
            continue;
        }
        if fields.len() != 8 {
            return Err(format!(
                "capability matrix row `{}` has {} fields; expected 8",
                fields.first().copied().unwrap_or("<empty>"),
                fields.len()
            ));
        }
        if fields.iter().any(|field| field.is_empty()) {
            return Err(format!(
                "capability matrix row `{}` contains an empty field",
                fields[0]
            ));
        }
        if !allowed_statuses.contains(&fields[6]) {
            return Err(format!(
                "capability matrix feature `{}` has invalid status `{}`",
                fields[0], fields[6]
            ));
        }
        if !features.insert(fields[0]) {
            return Err(format!(
                "capability matrix duplicates feature `{}`",
                fields[0]
            ));
        }
    }
    for required in [
        "WebAuthn transports",
        "Screen/window capture",
        "System audio sharing",
        "Codecs/DRM",
        "Hardware decode",
        "PDF and printing",
        "Web notifications",
        "Push service",
        "Spellcheck",
        "Primary selection",
        "Per-site settings",
    ] {
        if !features.contains(required) {
            return Err(format!(
                "capability matrix omits required feature `{required}`"
            ));
        }
    }
    Ok(())
}

fn check_ui_automation_boundary() -> Result<(), String> {
    let roots = [
        Path::new("crates/browser-engine-qt/src"),
        Path::new("crates/browser-engine-qt/qml"),
        Path::new("crates/rustbrowser/src"),
        Path::new("packaging"),
    ];
    // Keep these as split literals so this guard does not match its own source.
    let forbidden = [
        ["QTWEBENGINE_", "REMOTE_DEBUGGING"].concat(),
        ["remote", "-debugging-port"].concat(),
        ["remote", "-debugging-address"].concat(),
        ["automation", "-socket"].concat(),
    ];
    for root in roots {
        let mut files = vec![root.to_owned()];
        while let Some(path) = files.pop() {
            let metadata = fs::metadata(&path)
                .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
            if metadata.is_dir() {
                for entry in fs::read_dir(&path)
                    .map_err(|error| format!("could not read {}: {error}", path.display()))?
                {
                    files.push(
                        entry
                            .map_err(|error| format!("could not enumerate source: {error}"))?
                            .path(),
                    );
                }
                continue;
            }
            if !matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("rs" | "cpp" | "h" | "qml" | "toml" | "desktop")
            ) {
                continue;
            }
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            let lower = source.to_ascii_lowercase();
            if let Some(token) = forbidden
                .iter()
                .find(|token| lower.contains(&token.to_lowercase()))
            {
                return Err(format!(
                    "UI automation boundary violation: {} contains production automation marker {token}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn run_test(arguments: &[String]) -> Result<(), String> {
    let task = arguments.first().ok_or_else(|| {
        "test requires core, engine, adapter, fuzz, storage, fixtures, repeat, accessibility, compatibility, wayland, blocking, config, or mpris"
            .to_owned()
    })?;
    if arguments.len() > 1 {
        return Err(format!("test {task} does not accept additional arguments"));
    }
    match task.as_str() {
        "core" => run(
            "cargo",
            &["test", "-p", "browser-core", "--locked", "--offline"],
        ),
        "engine" => run(
            "cargo",
            &["test", "-p", "browser-engine-qt", "--locked", "--offline"],
        ),
        "adapter" => run_adapter_smoke(),
        "fuzz" => run_fuzz_smoke(),
        "storage" => run(
            "cargo",
            &["test", "-p", "browser-storage", "--locked", "--offline"],
        ),
        "fixtures" => run("cargo", &["test", "-p", "xtask", "--locked", "--offline"]),
        "repeat" => run_repeated_contract_smoke(),
        "accessibility" => {
            check_accessibility_contract()?;
            println!("browser-owned QML accessibility contract passed");
            Ok(())
        }
        "compatibility" => {
            check_compatibility_report()?;
            println!(
                "compatibility qualification report is structurally complete; live evidence remains report-owned"
            );
            Ok(())
        }
        "wayland" => run_wayland_smoke(),
        "blocking" => run_blocking_smoke(),
        "config" => run_config_smoke(),
        "mpris" => run_mpris_smoke(),
        other => Err(format!("unknown test task: {other}")),
    }
}

fn run_mpris_smoke() -> Result<(), String> {
    if !command_available("gdbus") {
        println!("MPRIS smoke not-run: gdbus is unavailable");
        return Ok(());
    }
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        println!("MPRIS smoke not-run: no session D-Bus address is available");
        return Ok(());
    }
    let executable = Path::new("target/debug/rustbrowser");
    if !executable.is_file() {
        return Err(format!(
            "MPRIS smoke requires {}; build the browser first",
            executable.display()
        ));
    }
    let mut fixture = ChildGuard::spawn(
        Command::new(
            std::env::current_exe()
                .map_err(|error| format!("could not locate xtask for MPRIS fixture: {error}"))?,
        )
        .args(["fixture-server", "--bind", "127.0.0.1:0", "--once"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()),
        "MPRIS fixture server",
    )?;
    let fixture_url = read_fixture_url_named(&mut fixture, "MPRIS fixture server", "")?;
    let fixture_url =
        format!("{fixture_url}/?token=secret&access_token=hidden&keep=value#fragment");
    let basedir = DisposableDirectory::new("rustbrowser-mpris")?;
    let mut app = ChildGuard::spawn(
        Command::new(executable)
            .arg("--basedir")
            .arg(basedir.path())
            .arg("open")
            .arg(&fixture_url),
        "RustBrowser MPRIS smoke",
    )?;
    let pid = app.child_mut("RustBrowser MPRIS smoke")?.id().to_string();
    let service = format!("org.mpris.MediaPlayer2.rustbrowser.instance{pid}");
    let introspection = wait_for_mpris_introspection(&service, &mut app)?;
    assert_mpris_introspection(&introspection)?;
    call_mpris_player_method(&service, "PlayPause")?;
    wait_for_redacted_mpris_metadata(&service)?;
    thread::sleep(Duration::from_millis(250));
    if app
        .child_mut("RustBrowser MPRIS smoke")?
        .try_wait()
        .map_err(|error| format!("could not verify MPRIS smoke process: {error}"))?
        .is_some()
    {
        return Err("browser exited after MPRIS PlayPause".into());
    }
    println!("MPRIS session-bus introspection and PlayPause smoke passed");
    Ok(())
}

fn wait_for_mpris_introspection(service: &str, app: &mut ChildGuard) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let output = Command::new("gdbus")
            .args([
                "introspect",
                "--session",
                "--dest",
                service,
                "--object-path",
                "/org/mpris/MediaPlayer2",
            ])
            .output()
            .map_err(|error| format!("could not run gdbus introspection: {error}"))?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        if app
            .child_mut("RustBrowser MPRIS smoke")?
            .try_wait()
            .map_err(|error| format!("could not poll MPRIS smoke process: {error}"))?
            .is_some()
        {
            return Err("browser exited before registering its MPRIS service".into());
        }
        if Instant::now() >= deadline {
            return Err("MPRIS service was not discoverable within 10s".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn assert_mpris_introspection(introspection: &str) -> Result<(), String> {
    for marker in [
        "interface org.mpris.MediaPlayer2.Player",
        "PlayPause();",
        "CanGoNext = false",
        "CanGoPrevious = false",
    ] {
        if !introspection.contains(marker) {
            return Err(format!("MPRIS introspection omitted `{marker}`"));
        }
    }
    Ok(())
}

fn call_mpris_player_method(service: &str, method: &str) -> Result<(), String> {
    let method = format!("org.mpris.MediaPlayer2.Player.{method}");
    let call = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            service,
            "--object-path",
            "/org/mpris/MediaPlayer2",
            "--method",
            &method,
        ])
        .output()
        .map_err(|error| format!("could not call MPRIS player method: {error}"))?;
    if call.status.success() {
        Ok(())
    } else {
        Err(format!(
            "MPRIS {method} failed: {}",
            String::from_utf8_lossy(&call.stderr).trim()
        ))
    }
}

fn read_mpris_property(service: &str, property: &str) -> Result<String, String> {
    let output = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            service,
            "--object-path",
            "/org/mpris/MediaPlayer2",
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.mpris.MediaPlayer2.Player",
            property,
        ])
        .output()
        .map_err(|error| format!("could not read MPRIS {property}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "MPRIS property {property} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn wait_for_redacted_mpris_metadata(service: &str) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let metadata = read_mpris_property(service, "Metadata")?;
        let can_play = read_mpris_property(service, "CanPlay")?;
        if metadata.contains("keep=value")
            && !metadata.contains("token=secret")
            && !metadata.contains("access_token=hidden")
            && !metadata.contains("#fragment")
            && can_play.contains("false")
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "MPRIS metadata did not reach the redacted fixture URL: {metadata}"
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn run_repeated_contract_smoke() -> Result<(), String> {
    const RUNS: usize = 3;
    for run_number in 1..=RUNS {
        println!("repeated contract run {run_number}/{RUNS}");
        run(
            "cargo",
            &[
                "test",
                "-p",
                "browser-core",
                "-p",
                "browser-storage",
                "--locked",
            ],
        )?;
    }
    println!("repeated core/storage contract smoke passed ({RUNS} runs)");
    Ok(())
}

fn run_adapter_smoke() -> Result<(), String> {
    run("cargo", &["build", "-p", "rustbrowser", "--locked"])?;
    let executable = Path::new("target/debug/rustbrowser");
    if !executable.is_file() {
        return Err(format!(
            "adapter smoke build did not produce {}",
            executable.display()
        ));
    }

    let mut application = ChildGuard::spawn(
        Command::new(executable)
            .arg("--temp-basedir")
            .env("QT_QPA_PLATFORM", "offscreen")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
        "RustBrowser offscreen adapter smoke",
    )?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = application
            .child_mut("RustBrowser offscreen adapter smoke")?
            .try_wait()
            .map_err(|error| format!("could not poll offscreen adapter smoke: {error}"))?
        {
            let output = application
                .take("RustBrowser offscreen adapter smoke")?
                .wait_with_output()
                .map_err(|error| format!("could not collect offscreen adapter smoke: {error}"))?;
            return Err(format!(
                "offscreen Qt/QML adapter smoke exited with {status}; stdout={:?}; stderr={:?}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if Instant::now() >= deadline {
            let mut child = application.take("RustBrowser offscreen adapter smoke")?;
            child
                .kill()
                .map_err(|error| format!("could not stop offscreen adapter smoke: {error}"))?;
            let output = child
                .wait_with_output()
                .map_err(|error| format!("could not collect offscreen adapter smoke: {error}"))?;
            let combined = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if combined.contains("QQmlApplicationEngine failed")
                || (combined.contains("module \"") && combined.contains("is not installed"))
            {
                return Err(format!(
                    "offscreen adapter smoke reached its bound with QML errors: {combined:?}"
                ));
            }
            println!("offscreen Qt/QML adapter smoke passed (bounded 10s lifetime)");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn run_wayland_smoke() -> Result<(), String> {
    check_nested_wayland_requirements()?;
    run("cargo", &["build", "-p", "rustbrowser", "--locked"])?;
    let executable = Path::new("target/debug/rustbrowser");
    if !executable.is_file() {
        return Err(format!(
            "Wayland smoke build did not produce {}",
            executable.display()
        ));
    }

    run_nested_wayland_smoke(executable)
}

#[allow(clippy::too_many_lines)]
fn run_config_smoke() -> Result<(), String> {
    check_nested_wayland_requirements()?;
    run("cargo", &["build", "-p", "rustbrowser", "--locked"])?;
    let executable = Path::new("target/debug/rustbrowser");
    if !executable.is_file() {
        return Err(format!(
            "config smoke build did not produce {}",
            executable.display()
        ));
    }

    let base = DisposableDirectory::new("rustbrowser-config")?;
    let config = base.path().join("config/config.toml");
    fs::create_dir_all(config.parent().expect("config parent"))
        .map_err(|error| format!("could not create config smoke directory: {error}"))?;
    fs::write(&config, "[ui]\nfont_size_pt = 11.0\n")
        .map_err(|error| format!("could not write config smoke config: {error}"))?;
    let query_config = base.path().join("config/query.toml");
    fs::write(&query_config, "[ui]\nfont_size_pt = 10.0\n")
        .map_err(|error| format!("could not write config smoke query config: {error}"))?;

    let mut fixture = start_nested_fixture_server()?;
    let fixture_url = read_fixture_url(&mut fixture)?;
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("could not create config smoke run ID: {error}"))?
        .as_nanos();
    let runtime_dir = std::env::temp_dir().join(format!("rustbrowser-config-wayland-{run_id}"));
    fs::create_dir(&runtime_dir)
        .map_err(|error| format!("could not create config Wayland runtime directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700)).map_err(|error| {
            format!("could not protect config Wayland runtime directory: {error}")
        })?;
    }
    let display = format!("wayland-rustbrowser-config-{run_id}");
    let result = (|| {
        let mut compositor = start_nested_compositor(&runtime_dir, &display)?;
        let socket = runtime_dir.join(&display);
        wait_for_wayland_socket(&mut compositor, &socket)?;
        let mut app = ChildGuard::spawn(
            Command::new("setsid")
                .arg("--")
                .arg("dbus-run-session")
                .arg("--")
                .arg(executable)
                .arg("--basedir")
                .arg(base.path())
                .arg("--instance")
                .arg("config-smoke")
                .arg("--software-rendering")
                .arg("open")
                .arg(&fixture_url)
                .env("QT_QPA_PLATFORM", "wayland")
                .env("XDG_RUNTIME_DIR", &runtime_dir)
                .env("WAYLAND_DISPLAY", &display)
                .env("GIO_USE_VFS", "local")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
            "RustBrowser configuration smoke",
        )?;

        thread::sleep(Duration::from_secs(2));
        let replacement = config.with_extension("toml.next");
        fs::write(&replacement, "[ui]\nfont_size_pt = 13.0\n")
            .map_err(|error| format!("could not stage config replacement: {error}"))?;
        fs::rename(&replacement, &config)
            .map_err(|error| format!("could not atomically replace config: {error}"))?;

        let deadline = Instant::now() + Duration::from_secs(6);
        let mut observed = false;
        let mut last_query = String::new();
        while Instant::now() < deadline {
            let output = Command::new(executable)
                .arg("--basedir")
                .arg(base.path())
                .arg("--instance")
                .arg("config-smoke")
                .arg("--config")
                .arg(&query_config)
                .arg("query")
                .arg("config")
                .arg("ui.font_size_pt")
                .arg("--format")
                .arg("json")
                .env("XDG_RUNTIME_DIR", &runtime_dir)
                .env("WAYLAND_DISPLAY", &display)
                .output()
                .map_err(|error| format!("could not query reloaded config: {error}"))?;
            last_query = format!(
                "status={}; stdout={:?}; stderr={:?}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if output.status.success()
                && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout)
                && value
                    .get("value")
                    .and_then(serde_json::Value::as_f64)
                    .is_some_and(|value| (value - 13.0).abs() < f64::EPSILON)
            {
                observed = true;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let invalid = config.with_extension("toml.invalid");
        fs::write(&invalid, "[ui\nfont_size_pt = 999.0\n")
            .map_err(|error| format!("could not stage invalid config replacement: {error}"))?;
        fs::rename(&invalid, &config).map_err(|error| {
            format!("could not atomically replace config with invalid candidate: {error}")
        })?;
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut retained = false;
        let mut last_retained_query = String::new();
        while Instant::now() < deadline {
            let output = Command::new(executable)
                .arg("--basedir")
                .arg(base.path())
                .arg("--instance")
                .arg("config-smoke")
                .arg("--config")
                .arg(&query_config)
                .arg("query")
                .arg("config")
                .arg("ui.font_size_pt")
                .arg("--format")
                .arg("json")
                .env("XDG_RUNTIME_DIR", &runtime_dir)
                .env("WAYLAND_DISPLAY", &display)
                .output()
                .map_err(|error| format!("could not query retained config: {error}"))?;
            last_retained_query = format!(
                "status={}; stdout={:?}; stderr={:?}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if output.status.success()
                && serde_json::from_slice::<serde_json::Value>(&output.stdout)
                    .ok()
                    .and_then(|value| value.get("value").and_then(serde_json::Value::as_f64))
                    .is_some_and(|value| (value - 13.0).abs() < f64::EPSILON)
            {
                retained = true;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let app_output = stop_application_with_output(&mut app, "RustBrowser configuration smoke")?;
        drop(compositor);
        if !retained {
            return Err(format!(
                "configuration reload did not retain the last-known-good value after an invalid atomic replacement; observed_valid={observed}; last_query={last_query:?}; last_retained_query={last_retained_query:?}; application_output={app_output:?}"
            ));
        }
        Ok(())
    })();
    let _ = stop_fixture_server(&mut fixture, "nested loopback fixture server");
    let cleanup = cleanup_nested_runtime_dir(&runtime_dir);
    match (result, cleanup) {
        (Ok(()), Ok(())) => {
            println!("nested Wayland configuration reload smoke passed");
            Ok(())
        }
        (Ok(()), Err(error)) => Err(format!(
            "configuration reload smoke passed but runtime cleanup failed: {error}"
        )),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(format!(
            "{error}; configuration runtime cleanup also failed: {cleanup_error}"
        )),
    }
}

#[allow(clippy::too_many_lines)]
fn run_blocking_smoke() -> Result<(), String> {
    check_nested_wayland_requirements()?;
    run("cargo", &["build", "-p", "rustbrowser", "--locked"])?;
    let executable = Path::new("target/debug/rustbrowser");
    if !executable.is_file() {
        return Err(format!(
            "blocking smoke build did not produce {}",
            executable.display()
        ));
    }

    let base = DisposableDirectory::new("rustbrowser-blocking")?;
    let config = base.path().join("config/config.toml");
    fs::create_dir_all(config.parent().expect("config parent"))
        .map_err(|error| format!("could not create blocking smoke config: {error}"))?;
    fs::write(
        &config,
        "[blocking]\nenabled = true\nnetwork_filtering = true\ncosmetic_filtering = false\nlists = [\"easylist\"]\n",
    )
    .map_err(|error| format!("could not write blocking smoke config: {error}"))?;
    let list = base.path().join("cache/blocklists/easylist.txt");
    fs::create_dir_all(list.parent().expect("blocklist parent"))
        .map_err(|error| format!("could not create cached blocklist directory: {error}"))?;
    fs::write(
        &list,
        "! RustBrowser blocking smoke\n||127.0.0.1^$image\n@@||127.0.0.1^$script\n",
    )
    .map_err(|error| format!("could not seed cached blocklist: {error}"))?;

    let mut page_fixture = start_blocking_fixture_server("127.0.0.1:0", Some("/blocking"))?;
    let page_url = read_fixture_url_named(
        &mut page_fixture,
        "blocking page fixture server",
        "/blocking",
    )?;
    let mut probe_fixture =
        start_blocking_fixture_server("127.0.0.1:18774", Some("/__rustbrowser_excepted__"))?;
    wait_for_fixture_listener("127.0.0.1:18774")?;

    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("could not create blocking smoke run ID: {error}"))?
        .as_nanos();
    let runtime_dir = std::env::temp_dir().join(format!("rustbrowser-blocking-wayland-{run_id}"));
    fs::create_dir(&runtime_dir)
        .map_err(|error| format!("could not create blocking Wayland runtime directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700)).map_err(|error| {
            format!("could not protect blocking Wayland runtime directory: {error}")
        })?;
    }
    let display = format!("wayland-rustbrowser-blocking-{run_id}");
    let mut application_output = String::new();
    let result = (|| {
        let mut compositor = start_nested_compositor(&runtime_dir, &display)?;
        let socket = runtime_dir.join(&display);
        wait_for_wayland_socket(&mut compositor, &socket)?;
        let mut app = ChildGuard::spawn(
            Command::new("setsid")
                .arg("--")
                .arg("dbus-run-session")
                .arg("--")
                .arg(executable)
                .arg("--basedir")
                .arg(base.path())
                .arg("--instance")
                .arg("blocking-smoke")
                .arg("--software-rendering")
                .arg("open")
                .arg("about:blank")
                .env("QT_QPA_PLATFORM", "wayland")
                .env("XDG_RUNTIME_DIR", &runtime_dir)
                .env("WAYLAND_DISPLAY", &display)
                .env("GIO_USE_VFS", "local")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
            "RustBrowser blocking smoke",
        )?;
        thread::sleep(Duration::from_secs(2));
        let open = Command::new(executable)
            .arg("--basedir")
            .arg(base.path())
            .arg("--instance")
            .arg("blocking-smoke")
            .arg("open")
            .arg(&page_url)
            .env("XDG_RUNTIME_DIR", &runtime_dir)
            .env("WAYLAND_DISPLAY", &display)
            .output()
            .map_err(|error| format!("could not open blocking fixture through IPC: {error}"))?;
        if !open.status.success() {
            return Err(format!(
                "blocking fixture IPC open failed with {}; stdout={:?}; stderr={:?}",
                open.status,
                String::from_utf8_lossy(&open.stdout),
                String::from_utf8_lossy(&open.stderr)
            ));
        }
        // The probe fixture exits after its one-shot exception request is received.
        // Wait for that authoritative event instead of relying on a fixed
        // startup delay, while retaining a hard bound for a broken launch.
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Some(status) = app
                .child_mut("RustBrowser blocking smoke")?
                .try_wait()
                .map_err(|error| format!("could not poll RustBrowser blocking smoke: {error}"))?
            {
                let output = app
                    .take("RustBrowser blocking smoke")?
                    .wait_with_output()
                    .map_err(|error| {
                        format!("could not collect RustBrowser blocking smoke: {error}")
                    })?;
                return Err(format!(
                    "RustBrowser blocking smoke exited with {status}; stdout={:?}; stderr={:?}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            let probe_finished = probe_fixture
                .child_mut("blocking fixture server")?
                .try_wait()
                .map_err(|error| format!("could not poll blocking probe fixture server: {error}"))?
                .is_some();
            let page_finished = page_fixture
                .child_mut("blocking fixture server")?
                .try_wait()
                .map_err(|error| format!("could not poll blocking page fixture server: {error}"))?
                .is_some();
            if probe_finished && page_finished {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        application_output = stop_application_with_output(&mut app, "RustBrowser blocking smoke")?;
        drop(compositor);
        Ok(())
    })();

    let probe_output = stop_fixture_server(&mut probe_fixture, "blocking probe fixture server")?;
    let page_output = stop_fixture_server(&mut page_fixture, "blocking page fixture server")?;
    let blocked_request =
        probe_output.contains("RUSTBROWSER_FIXTURE_EVENT path=/__rustbrowser_blocked__");
    let exception_request =
        probe_output.contains("RUSTBROWSER_FIXTURE_EVENT path=/__rustbrowser_excepted__");
    let cleanup = cleanup_nested_runtime_dir(&runtime_dir);
    match (result, blocked_request, exception_request, cleanup) {
        (Ok(()), false, true, Ok(())) => {
            println!(
                "cached-list blocking smoke passed: blocked request suppressed and exception request allowed"
            );
            Ok(())
        }
        (Ok(()), blocked, exception, Ok(())) => Err(format!(
            "blocking smoke policy result was wrong: blocked_request={blocked}; exception_request={exception}; probe_fixture={probe_output:?}; page_fixture={page_output:?}; application_output={application_output:?}"
        )),
        (Err(error), _, _, Ok(())) => Err(error),
        (Ok(()), _, _, Err(cleanup_error)) => Err(format!(
            "blocking smoke failed during runtime cleanup: {cleanup_error}"
        )),
        (Err(error), _, _, Err(cleanup_error)) => Err(format!(
            "{error}; blocking runtime cleanup also failed: {cleanup_error}"
        )),
    }
}

fn start_blocking_fixture_server(
    bind: &str,
    exit_on_path: Option<&str>,
) -> Result<ChildGuard, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate xtask for blocking fixture: {error}"))?;
    let mut command = Command::new(executable);
    command.arg("fixture-server").arg("--bind").arg(bind);
    if let Some(path) = exit_on_path {
        command.arg("--exit-on-path").arg(path);
    }
    ChildGuard::spawn(
        command.stdout(Stdio::piped()).stderr(Stdio::piped()),
        "blocking fixture server",
    )
}

fn wait_for_fixture_listener(bind: &str) -> Result<(), String> {
    let address = bind
        .to_socket_addrs()
        .map_err(|error| format!("could not resolve blocking fixture {bind}: {error}"))?
        .next()
        .ok_or_else(|| format!("blocking fixture {bind} resolved to no address"))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(100)) {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
            let request = format!(
                "GET /__rustbrowser_fixture_ready__ HTTP/1.1\r\nHost: {bind}\r\nConnection: close\r\n\r\n"
            );
            if stream.write_all(request.as_bytes()).is_ok() {
                let mut response = [0_u8; 1024];
                while stream.read(&mut response).is_ok_and(|read| read > 0) {}
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(format!(
        "blocking fixture listener {bind} did not become ready within 5s"
    ))
}

fn stop_fixture_server(server: &mut ChildGuard, label: &str) -> Result<String, String> {
    let mut child = server.take(label)?;
    let _ = child.kill();
    let output = child
        .wait_with_output()
        .map_err(|error| format!("could not collect {label}: {error}"))?;
    Ok(format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn stop_application_with_output(child: &mut ChildGuard, label: &str) -> Result<String, String> {
    let mut child = child.take(label)?;
    #[cfg(unix)]
    {
        let process_group = format!("-{}", child.id());
        let _ = Command::new("kill")
            .args(["-KILL", process_group.as_str()])
            .status();
    }
    let _ = child.kill();
    let output = child
        .wait_with_output()
        .map_err(|error| format!("could not collect {label}: {error}"))?;
    Ok(format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

struct ChildGuard {
    child: Option<Child>,
}

struct DisposableDirectory {
    path: std::path::PathBuf,
}

impl DisposableDirectory {
    fn new(prefix: &str) -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("could not make disposable directory nonce: {error}"))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{nonce}-{}", std::process::id()));
        fs::create_dir(&path)
            .map_err(|error| format!("could not create disposable directory: {error}"))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DisposableDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl ChildGuard {
    fn spawn(command: &mut Command, label: &str) -> Result<Self, String> {
        let child = command
            .spawn()
            .map_err(|error| format!("failed to start {label}: {error}"))?;
        Ok(Self { child: Some(child) })
    }

    fn child_mut(&mut self, label: &str) -> Result<&mut Child, String> {
        self.child
            .as_mut()
            .ok_or_else(|| format!("{label} process handle is unavailable"))
    }

    fn take(&mut self, label: &str) -> Result<Child, String> {
        self.child
            .take()
            .ok_or_else(|| format!("{label} process handle is unavailable"))
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn command_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

#[cfg(unix)]
fn is_socket(path: &Path) -> bool {
    use std::os::unix::fs::FileTypeExt;

    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_socket())
}

#[cfg(not(unix))]
fn is_socket(_path: &Path) -> bool {
    false
}

fn start_nested_compositor(runtime_dir: &Path, display: &str) -> Result<ChildGuard, String> {
    ChildGuard::spawn(
        Command::new("weston")
            .arg("--backend=headless-backend.so")
            .arg(format!("--socket={display}"))
            .arg("--idle-time=0")
            .arg("--no-config")
            .env("XDG_RUNTIME_DIR", runtime_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
        "nested Weston compositor",
    )
}

fn wait_for_wayland_socket(compositor: &mut ChildGuard, socket: &Path) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if is_socket(socket) {
            return Ok(());
        }
        if compositor
            .child_mut("nested Weston compositor")?
            .try_wait()
            .map_err(|error| format!("could not poll nested Weston compositor: {error}"))?
            .is_some()
        {
            return Err("nested Weston compositor exited before creating its socket".to_owned());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!(
        "nested Weston compositor did not create {} within 5s",
        socket.display()
    ))
}

fn run_nested_application(
    executable: &Path,
    runtime_dir: &Path,
    display: &str,
    fixture_url: &str,
) -> Result<bool, String> {
    let mut app = ChildGuard::spawn(
        Command::new("setsid")
            .arg("--")
            .arg("dbus-run-session")
            .arg("--")
            .arg(executable)
            .arg("--temp-basedir")
            .arg("open")
            .arg(fixture_url)
            .env("QT_QPA_PLATFORM", "wayland")
            .env("XDG_RUNTIME_DIR", runtime_dir)
            .env("WAYLAND_DISPLAY", display)
            .env("GIO_USE_VFS", "local")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
        "RustBrowser in nested Wayland session",
    )?;
    thread::sleep(Duration::from_secs(2));
    let input_qualified = if command_available("wtype") {
        let input_status = Command::new("wtype")
            .arg("-M")
            .arg("ctrl")
            .arg("-k")
            .arg("a")
            .arg("-m")
            .arg("ctrl")
            .arg("--")
            .arg("native-input")
            .env("XDG_RUNTIME_DIR", runtime_dir)
            .env("WAYLAND_DISPLAY", display)
            .status()
            .map_err(|error| format!("could not send native input through wtype: {error}"))?;
        if input_status.success() {
            true
        } else {
            eprintln!(
                "native input qualification not-run: wtype could not access the compositor virtual-keyboard protocol ({input_status})"
            );
            false
        }
    } else {
        eprintln!("native input qualification not-run: optional wtype helper is unavailable");
        false
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = app
            .child_mut("RustBrowser")?
            .try_wait()
            .map_err(|error| format!("could not poll RustBrowser Wayland smoke: {error}"))?
        {
            let output = app
                .take("RustBrowser")?
                .wait_with_output()
                .map_err(|error| format!("could not collect RustBrowser smoke output: {error}"))?;
            return Err(format_smoke_failure(status, &output));
        }
        if Instant::now() >= deadline {
            let output =
                stop_application_with_output(&mut app, "RustBrowser in nested Wayland session")?;
            if output.contains("QQmlApplicationEngine failed")
                || (output.contains("module \"") && output.contains("is not installed"))
            {
                return Err(format!(
                    "nested Wayland smoke reached its bound with QML errors: {output:?}"
                ));
            }
            println!("nested native Wayland startup smoke passed (bounded 10s lifetime)");
            return Ok(input_qualified);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn run_nested_wayland_smoke(executable: &Path) -> Result<(), String> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("could not create nested Wayland run ID: {error}"))?
        .as_nanos();
    let mut fixture = start_nested_fixture_server()?;
    let fixture_url = read_fixture_url(&mut fixture)?;
    let runtime_dir = std::env::temp_dir().join(format!("rustbrowser-wayland-{run_id}"));
    fs::create_dir(&runtime_dir)
        .map_err(|error| format!("could not create private Wayland runtime directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not protect Wayland runtime directory: {error}"))?;
    }

    let display = format!("wayland-rustbrowser-{run_id}");
    let result = (|| {
        let mut compositor = start_nested_compositor(&runtime_dir, &display)?;
        let socket = runtime_dir.join(&display);
        wait_for_wayland_socket(&mut compositor, &socket)?;
        let result = run_nested_application(executable, &runtime_dir, &display, &fixture_url);
        drop(compositor);
        result
    })();
    let fixture_result = fixture
        .child_mut("nested loopback fixture server")?
        .try_wait()
        .map_err(|error| format!("could not poll nested fixture server: {error}"))?;
    let result = match (result, fixture_result) {
        (Ok(true), Some(status)) if status.success() => {
            println!("nested Wayland editable-page input smoke passed");
            Ok(())
        }
        (Ok(false), _) => {
            println!(
                "nested native Wayland startup smoke passed; editable-page input qualification not-run (compositor lacks virtual-keyboard protocol)"
            );
            Ok(())
        }
        (Ok(true), Some(status)) => Err(format!(
            "nested fixture server exited unsuccessfully after the browser request: {status}"
        )),
        (Ok(true), None) => {
            Err("nested Wayland smoke stayed alive but did not request the loopback fixture".into())
        }
        (Err(error), _) => Err(error),
    };
    drop(fixture);

    let cleanup = cleanup_nested_runtime_dir(&runtime_dir);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(error)) => Err(format!(
            "nested Wayland smoke passed but private runtime cleanup failed: {error}"
        )),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(format!(
            "{error}; private runtime cleanup also failed: {cleanup_error}"
        )),
    }
}

fn start_nested_fixture_server() -> Result<ChildGuard, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate xtask for fixture server: {error}"))?;
    ChildGuard::spawn(
        Command::new(executable)
            .arg("fixture-server")
            .arg("--bind")
            .arg("127.0.0.1:0")
            .arg("--once")
            .arg("--exit-on-path")
            .arg("/input-result")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
        "nested loopback fixture server",
    )
}

fn read_fixture_url(fixture: &mut ChildGuard) -> Result<String, String> {
    read_fixture_url_named(fixture, "nested loopback fixture server", "/editable")
}

fn read_fixture_url_named(
    fixture: &mut ChildGuard,
    label: &str,
    path: &str,
) -> Result<String, String> {
    let stdout = fixture
        .child_mut(label)?
        .stdout
        .as_mut()
        .ok_or_else(|| format!("{label} stdout is unavailable"))?;
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .map_err(|error| format!("could not read fixture readiness: {error}"))?;
    let url = line
        .trim()
        .strip_prefix("RUSTBROWSER_FIXTURE_READY http://")
        .map(|address| format!("http://{address}{path}"))
        .ok_or_else(|| format!("fixture server returned invalid readiness: {line:?}"))?;
    if !url.starts_with("http://127.0.0.1:") {
        return Err(format!("fixture server escaped loopback: {url}"));
    }
    Ok(url)
}

fn cleanup_nested_runtime_dir(runtime_dir: &Path) -> std::io::Result<()> {
    // A D-Bus session can start gvfsd-fuse inside the private runtime
    // directory. It outlives the short-lived session wrapper, and its
    // unmount can race with process teardown, so retry this exact path.
    let cleanup_dir = runtime_dir.with_extension("cleanup");
    let mut cleanup_root = runtime_dir.to_path_buf();
    let mut last_error = None;
    for _ in 0..300 {
        if cleanup_root == runtime_dir {
            match fs::rename(runtime_dir, &cleanup_dir) {
                Ok(()) => cleanup_root.clone_from(&cleanup_dir),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => {
                    last_error = Some(error);
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
            }
        }
        for mount in [cleanup_root.join("gvfs"), cleanup_root.join("doc")] {
            if mount.exists() {
                let _ = Command::new("fusermount3")
                    .args(["-u", "-z"])
                    .arg(mount)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        }
        match remove_nested_runtime_entries(&cleanup_root) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
    Err(last_error.expect("nested runtime cleanup made no attempt"))
}

fn remove_nested_runtime_entries(runtime_dir: &Path) -> std::io::Result<()> {
    let entries = match fs::read_dir(runtime_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    for entry in entries {
        let path = entry?.path();
        let is_directory = fs::symlink_metadata(&path)
            .map_or_else(|_| path.is_dir(), |metadata| metadata.file_type().is_dir());
        if is_directory
            || path
                .file_name()
                .is_some_and(|name| name == "gvfs" || name == "doc")
        {
            match fs::remove_dir_all(&path) {
                Ok(()) => {}
                Err(error)
                    if error.raw_os_error() == Some(107)
                        && path
                            .file_name()
                            .is_some_and(|name| name == "gvfs" || name == "doc") =>
                {
                    // A disconnected portal FUSE mount can remain as an
                    // empty mount point after its owning D-Bus session has
                    // exited. Removing the mount point is safe here because
                    // this function owns the private disposable runtime.
                    fs::remove_dir(&path).map_err(|remove_error| {
                        std::io::Error::new(
                            remove_error.kind(),
                            format!("remove directory {}: {remove_error}", path.display()),
                        )
                    })?;
                }
                Err(error) => {
                    return Err(std::io::Error::new(
                        error.kind(),
                        format!("remove directory {}: {error}", path.display()),
                    ));
                }
            }
        } else {
            fs::remove_file(&path).map_err(|error| {
                std::io::Error::new(
                    error.kind(),
                    format!("remove file {}: {error}", path.display()),
                )
            })?;
        }
    }
    fs::remove_dir(runtime_dir).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!(
                "remove runtime directory {}: {error}",
                runtime_dir.display()
            ),
        )
    })
}

fn check_nested_wayland_requirements() -> Result<(), String> {
    if !command_available("weston") {
        return Err(
            "native Wayland qualification unavailable: weston is required for the disposable nested compositor"
                .to_owned(),
        );
    }
    if !command_available("dbus-run-session") {
        return Err(
            "native Wayland qualification unavailable: dbus-run-session is required for the disposable D-Bus session"
                .to_owned(),
        );
    }
    Ok(())
}

fn format_smoke_failure(status: std::process::ExitStatus, output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "nested native Wayland startup smoke exited with {status}; stdout={stdout:?}; stderr={stderr:?}"
    )
}

fn run_package(arguments: &[String]) -> Result<(), String> {
    let task = arguments
        .first()
        .ok_or_else(|| "package requires arch or artifacts".to_owned())?;
    if arguments.len() > 1 {
        return Err(format!(
            "package {task} does not accept additional arguments"
        ));
    }
    match task.as_str() {
        "arch" => validate_arch_package(),
        "artifacts" => validate_release_artifacts().and_then(|()| validate_license_policy()),
        other => Err(format!("unknown package task: {other}")),
    }
}

fn validate_release_artifacts() -> Result<(), String> {
    let manifest_path = Path::new("packaging/release-artifacts.toml");
    let manifest = fs::read_to_string(manifest_path)
        .map_err(|error| format!("could not read {}: {error}", manifest_path.display()))?;
    let parsed = manifest
        .parse::<toml::Table>()
        .map_err(|error| format!("could not parse {}: {error}", manifest_path.display()))?;
    let entries = parsed
        .get("artifact")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "release artifact manifest must contain an [[artifact]] array".to_owned())?;
    if entries.is_empty() || entries.len() > 64 {
        return Err("release artifact manifest must contain 1..=64 entries".to_owned());
    }
    let mut paths = BTreeSet::new();
    for entry in entries {
        let table = entry
            .as_table()
            .ok_or_else(|| "release artifact entries must be tables".to_owned())?;
        let name = table
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| "release artifact entry is missing a name".to_owned())?;
        let path = table
            .get("path")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("release artifact {name} is missing a path"))?;
        if name.is_empty() || name.len() > 96 || path.is_empty() || path.len() > 256 {
            return Err(format!(
                "release artifact {name:?} has an invalid bounded name/path"
            ));
        }
        let relative = Path::new(path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| component == std::path::Component::ParentDir)
        {
            return Err(format!(
                "release artifact {name} has an unsafe path {path:?}"
            ));
        }
        if !paths.insert(path.to_owned()) {
            return Err(format!("release artifact manifest duplicates {path}"));
        }
        if !relative.is_file() {
            return Err(format!("release artifact {name} is missing: {path}"));
        }
    }
    println!(
        "release artifact manifest passed ({} checked-in inputs; source archives, signatures, and generated checksums remain release outputs)",
        entries.len()
    );
    Ok(())
}

fn validate_license_policy() -> Result<(), String> {
    let policy_path = Path::new("packaging/license-policy.toml");
    let policy = fs::read_to_string(policy_path)
        .map_err(|error| format!("could not read {}: {error}", policy_path.display()))?;
    let parsed = policy
        .parse::<toml::Table>()
        .map_err(|error| format!("could not parse {}: {error}", policy_path.display()))?;
    let entries = parsed
        .get("dependency")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "license policy must contain a [[dependency]] array".to_owned())?;
    if entries.is_empty() || entries.len() > 32 {
        return Err("license policy must contain 1..=32 dependencies".to_owned());
    }
    let lockfile = fs::read_to_string("Cargo.lock")
        .map_err(|error| format!("could not read Cargo.lock: {error}"))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let table = entry
            .as_table()
            .ok_or_else(|| "license policy entries must be tables".to_owned())?;
        let name = table
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| "license policy dependency is missing a name".to_owned())?;
        let license = table
            .get("license")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("license policy dependency {name} is missing a license"))?;
        let provenance = table
            .get("provenance")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("license policy dependency {name} is missing provenance"))?;
        if name.is_empty()
            || name.len() > 96
            || license.is_empty()
            || license.len() > 128
            || provenance.is_empty()
            || provenance.len() > 512
            || name.chars().any(char::is_control)
            || license.chars().any(char::is_control)
            || provenance.chars().any(char::is_control)
        {
            return Err(format!(
                "license policy dependency {name:?} has invalid bounded metadata"
            ));
        }
        if !names.insert(name.to_owned()) {
            return Err(format!("license policy duplicates dependency {name}"));
        }
        let lock_name = format!(r#"name = "{name}""#);
        if !lockfile.lines().any(|line| line == lock_name) {
            return Err(format!(
                "license policy dependency {name} is absent from Cargo.lock"
            ));
        }
    }
    println!(
        "license policy passed ({} locked dependencies with bounded provenance)",
        entries.len()
    );
    Ok(())
}

fn validate_arch_package() -> Result<(), String> {
    let required_files = [
        "packaging/PKGBUILD",
        "packaging/omarchy/PKGBUILD",
        "packaging/io.github.rustbrowser.RustBrowser.desktop",
        "packaging/io.github.rustbrowser.RustBrowser.svg",
        "packaging/dependencies.toml",
    ];
    for file in required_files {
        if !Path::new(file).is_file() {
            return Err(format!("Arch package input is missing: {file}"));
        }
    }
    validate_arch_install_layout()?;

    run_from_directory(
        "desktop-file-validate",
        &["io.github.rustbrowser.RustBrowser.desktop"],
        Path::new("packaging"),
    )?;
    run_from_directory(
        "makepkg",
        &["--printsrcinfo", "-p", "PKGBUILD"],
        Path::new("packaging"),
    )?;
    run_from_directory(
        "makepkg",
        &["--printsrcinfo", "-p", "PKGBUILD"],
        Path::new("packaging/omarchy"),
    )?;
    println!("Arch package metadata and desktop integration validation passed");
    Ok(())
}

fn validate_arch_install_layout() -> Result<(), String> {
    let recipe = fs::read_to_string("packaging/PKGBUILD")
        .map_err(|error| format!("could not read packaging/PKGBUILD: {error}"))?;
    let required_recipe_fragments = [
        "cargo build --release --locked",
        "target/release/rustbrowser",
        "/usr/bin/rustbrowser",
        "/usr/share/applications/io.github.rustbrowser.RustBrowser.desktop",
        "/usr/share/icons/hicolor/scalable/apps/io.github.rustbrowser.RustBrowser.svg",
        "/usr/share/doc/${pkgname}/README.md",
        "/usr/share/doc/${pkgname}/DEVELOPMENT_SPEC.md",
        "/usr/share/doc/${pkgname}/dependencies.toml",
    ];
    for fragment in required_recipe_fragments {
        if !recipe.contains(fragment) {
            return Err(format!(
                "Arch package recipe does not declare the required installed layout fragment: {fragment}"
            ));
        }
    }
    validate_installed_file_manifest(
        Path::new("packaging/installed-files.txt"),
        &recipe,
        "rustbrowser",
    )?;
    let installed_manifest = fs::read_to_string("packaging/installed-files.txt")
        .map_err(|error| format!("could not read packaging/installed-files.txt: {error}"))?;
    for required in [
        "/usr/share/doc/rustbrowser/LICENSES.md",
        "/usr/share/doc/rustbrowser/license-policy.toml",
        "/usr/share/doc/rustbrowser/SUPPORT.md",
        "/usr/share/doc/rustbrowser/SECURITY.md",
        "/usr/share/doc/rustbrowser/RELEASE_POLICY.md",
        "/usr/share/doc/rustbrowser/PROVENANCE.md",
        "/usr/share/doc/rustbrowser/MAINTENANCE.md",
        "/usr/share/doc/rustbrowser/CONTRIBUTING.md",
        "/usr/share/doc/rustbrowser/CODE_OF_CONDUCT.md",
    ] {
        if !installed_manifest
            .lines()
            .any(|line| line.trim() == required)
        {
            return Err(format!(
                "primary installed-file manifest omits required release documentation: {required}"
            ));
        }
    }
    let omarchy_recipe = fs::read_to_string("packaging/omarchy/PKGBUILD")
        .map_err(|error| format!("could not read packaging/omarchy/PKGBUILD: {error}"))?;
    validate_installed_file_manifest(
        Path::new("packaging/omarchy/installed-files.txt"),
        &omarchy_recipe,
        "rustbrowser-omarchy",
    )?;
    let dependencies = fs::read_to_string("packaging/dependencies.toml")
        .map_err(|error| format!("could not read packaging/dependencies.toml: {error}"))?;
    for fragment in [
        "model = \"system-dynamic\"",
        "[required]",
        "xdg-utils",
        "[build]",
        "desktop-file-utils",
        "check_packages = [\"dbus\", \"weston\"]",
        "[optional.portals]",
    ] {
        if !dependencies.contains(fragment) {
            return Err(format!(
                "dependency manifest does not declare the distribution boundary: {fragment}"
            ));
        }
    }
    Ok(())
}

fn validate_installed_file_manifest(
    manifest_path: &Path,
    recipe: &str,
    package_name: &str,
) -> Result<(), String> {
    let manifest = fs::read_to_string(manifest_path)
        .map_err(|error| format!("could not read {}: {error}", manifest_path.display()))?;
    let expanded_recipe = recipe.replace("${pkgname}", package_name);
    let mut paths = BTreeSet::new();
    for (line_number, line) in manifest.lines().enumerate() {
        let path = line.trim();
        if path.is_empty() || path.starts_with('#') {
            continue;
        }
        let relative = Path::new(path);
        if !path.starts_with('/')
            || path.len() > 256
            || relative
                .components()
                .any(|component| component == std::path::Component::ParentDir)
        {
            return Err(format!(
                "installed-file manifest {} line {} has an unsafe path: {path:?}",
                manifest_path.display(),
                line_number + 1
            ));
        }
        if !paths.insert(path.to_owned()) {
            return Err(format!(
                "installed-file manifest {} duplicates {path}",
                manifest_path.display()
            ));
        }
        if !expanded_recipe.contains(path) {
            return Err(format!(
                "package recipe for {package_name} does not install manifest path {path}"
            ));
        }
    }
    if paths.is_empty() {
        return Err(format!(
            "installed-file manifest {} is empty",
            manifest_path.display()
        ));
    }
    Ok(())
}

fn check_requirements(spec_path: &Path, tracker_path: &Path) -> Result<(), String> {
    let spec = fs::read_to_string(spec_path)
        .map_err(|error| format!("cannot read {}: {error}", spec_path.display()))?;
    let tracker = fs::read_to_string(tracker_path)
        .map_err(|error| format!("cannot read {}: {error}", tracker_path.display()))?;

    let expected: BTreeSet<_> = spec
        .lines()
        .filter_map(|line| line.strip_prefix("**"))
        .filter_map(|line| line.split_once(" — "))
        .map(|(id, _)| id.to_owned())
        .collect();
    let header = parse_csv_record(
        tracker
            .lines()
            .next()
            .ok_or_else(|| "requirements tracker is empty".to_owned())?,
    )?;
    let expected_header = [
        "id",
        "title",
        "milestone",
        "status",
        "implementation",
        "automated_tests",
        "manual_evidence",
        "issue_notes",
    ];
    if header.iter().map(String::as_str).collect::<Vec<_>>() != expected_header {
        return Err(format!(
            "requirements tracker header mismatch: expected {expected_header:?}, got {header:?}"
        ));
    }
    let mut actual = BTreeSet::new();
    for (line_number, line) in tracker.lines().enumerate().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_record(line)
            .map_err(|error| format!("requirements tracker row {}: {error}", line_number + 1))?;
        if fields.len() != expected_header.len() {
            return Err(format!(
                "requirements tracker row {} has {} fields; expected {}",
                line_number + 1,
                fields.len(),
                expected_header.len()
            ));
        }
        validate_requirement_path_references(&fields, line_number + 1)?;
        let id = fields[0].as_str();
        if id.is_empty() {
            return Err(format!(
                "requirements tracker row {} has no ID",
                line_number + 1
            ));
        }
        let status = fields[3].as_str();
        if !matches!(
            status,
            "not-started" | "in-progress" | "verified" | "blocked"
        ) {
            return Err(format!(
                "requirements tracker row {} has invalid status {status:?}",
                line_number + 1
            ));
        }
        if status != "not-started" && fields[4..7].iter().any(String::is_empty) {
            return Err(format!(
                "requirements tracker row {} with status {status:?} must include implementation, automated tests, and manual evidence",
                line_number + 1
            ));
        }
        if status == "blocked" && fields[7].is_empty() {
            return Err(format!(
                "requirements tracker row {} marked blocked must describe its issue",
                line_number + 1
            ));
        }
        if !actual.insert(id.to_owned()) {
            return Err(format!("requirements tracker duplicates {id}"));
        }
    }
    if expected != actual {
        let missing: Vec<_> = expected.difference(&actual).cloned().collect();
        let extra: Vec<_> = actual.difference(&expected).cloned().collect();
        return Err(format!(
            "requirements tracker mismatch; missing={missing:?}, extra={extra:?}"
        ));
    }
    Ok(())
}

fn validate_requirement_path_references(
    fields: &[String],
    line_number: usize,
) -> Result<(), String> {
    for field in fields {
        for prefix in ["docs/", "crates/", "fuzz/", "packaging/"] {
            let mut remaining = field.as_str();
            while let Some(start) = remaining.find(prefix) {
                remaining = &remaining[start..];
                let end = remaining
                    .find(|character: char| {
                        character.is_whitespace() || ",;)]}".contains(character)
                    })
                    .unwrap_or(remaining.len());
                let reference = &remaining[..end];
                if !Path::new(reference).exists() {
                    return Err(format!(
                        "requirements tracker row {line_number} references missing path {reference}"
                    ));
                }
                remaining = &remaining[end..];
            }
        }
    }
    Ok(())
}

fn parse_csv_record(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut just_closed_quote = false;
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        match (quoted, character) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (true, '"') => {
                quoted = false;
                just_closed_quote = true;
            }
            (false, '"') if field.is_empty() => {
                quoted = true;
                just_closed_quote = false;
            }
            (false, ',') => {
                fields.push(std::mem::take(&mut field));
                just_closed_quote = false;
            }
            (false, _) if just_closed_quote => {
                return Err("unexpected characters after a quoted field".into());
            }
            (false | true, character) => field.push(character),
        }
    }
    if quoted {
        return Err("unterminated quoted field".into());
    }
    fields.push(field);
    Ok(fields)
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|error| format!("failed to run {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {args:?} exited with {status}"))
    }
}

fn run_from_directory(program: &str, args: &[&str], directory: &Path) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .current_dir(directory)
        .status()
        .map_err(|error| {
            format!(
                "failed to run {program} in {}: {error}",
                directory.display()
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} {args:?} in {} exited with {status}",
            directory.display()
        ))
    }
}

fn run_fuzz_smoke() -> Result<(), String> {
    run(
        "cargo",
        &[
            "build",
            "--manifest-path",
            "fuzz/Cargo.toml",
            "--locked",
            "--offline",
        ],
    )?;

    let targets = [
        "action_target",
        "bindings",
        "command_parser",
        "contexts_routes",
        "hints",
        "ipc_frames",
        "session_snapshot",
        "site_rule",
        "state_sequences",
        "switcher_matching",
        "url_and_clean_link",
        "userscript_results",
    ];
    for target in targets {
        let corpus = format!("fuzz/corpus/{target}");
        let binary = format!("fuzz/target/debug/{target}");
        let status = Command::new(&binary)
            .args(["-runs=32", "-close_fd_mask=3", &corpus])
            .status()
            .map_err(|error| format!("failed to run fuzz smoke target {target}: {error}"))?;
        if !status.success() {
            return Err(format!("fuzz smoke target {target} exited with {status}"));
        }
    }
    println!(
        "bounded fuzz corpus smoke passed ({} targets; 32 runs each)",
        targets.len()
    );
    Ok(())
}

#[cfg(test)]
mod requirement_tests {
    use super::parse_csv_record;

    #[test]
    fn csv_record_parser_handles_escaped_quotes_and_commas() {
        assert_eq!(
            parse_csv_record("\"id\",\"a, title\",\"a \"\"quoted\"\" title\"").expect("record"),
            vec!["id", "a, title", "a \"quoted\" title"]
        );
    }

    #[test]
    fn csv_record_parser_rejects_malformed_quotes() {
        assert!(parse_csv_record(r#""id,"title""#).is_err());
        assert!(parse_csv_record(r#""id"tail,"title""#).is_err());
    }
}
