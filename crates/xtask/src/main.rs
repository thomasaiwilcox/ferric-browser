use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ferric_browser_config::{Config, SettingRegistry};
use ferric_browser_core::versioning::{
    PageStateSidecar, SidecarSummary, route_fingerprint_for_url,
};
use ferric_browser_core::{ActionRegistry, BindingRegistry, CommandRegistry, Mode};

mod fixture_server;
mod perf;
mod versioning_capabilities;

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
            println!("cargo xtask test archive");
            println!("  Run the disposable native MHTML routing/isolation probe.");
            println!("cargo xtask test versioning-geometry");
            println!("  Validate a synthetic native geometry result through the Rust sidecar DTO.");
            println!("cargo xtask test versioning-profile-download");
            println!(
                "  Check exact view ownership for two disposable shared-profile Qt downloads."
            );
            println!("cargo xtask test versioning-clean-branch-profile");
            println!(
                "  Check distinct off-record Quick profiles and cookie stores for clean branches."
            );
            println!("cargo xtask test versioning-viewless-download");
            println!(
                "  Probe a public MV3 extension-originated download on a disposable Quick profile."
            );
            println!("cargo xtask test cookie");
            println!("  Run the disposable native cookie API gap probe.");
            println!("cargo xtask test versioning-cookie-adapter");
            println!("  Test bounded public Quick cookie observations and profile teardown.");
            println!("cargo xtask test versioning-cookie-boundary");
            println!(
                "  Probe persistent and concurrent cookies without claiming a snapshot boundary."
            );
            println!("cargo xtask test versioning-fixtures");
            println!("  Run the disposable pinned-HTTPS native fixture probe.");
            println!("cargo xtask test adapter");
            println!("  Run the bounded offscreen Qt/QML adapter integration smoke.");
            println!("cargo xtask test core");
            println!("  Run the Qt-independent ferric-browser-core contract tests.");
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
            println!(
                "cargo xtask fixture-server [--bind ADDRESS] [--once] [--https] [--exit-on-path PATH]"
            );
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
        Mode::Grid => "grid",
        Mode::Caret => "caret",
        Mode::PassThrough => "pass-through",
    }
}

fn render_binding_docs() -> Result<String, String> {
    let bindings = BindingRegistry::default_v1(CommandRegistry::default_v1())
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
    versioning_capabilities::check_file(Path::new(
        "docs/testing/versioned-browsing/capabilities.json",
    ))?;
    run_qml_lint()?;
    run_qml_ui_tests()?;
    run_geometry_sidecar_probe()?;
    run_native_profile_download_probe()?;
    run_native_clean_branch_profile_probe()?;
    run_native_archive_probe()?;
    run_native_cookie_probe()?;
    run_native_cookie_adapter_probe()?;
    run_native_cookie_boundary_probe()?;
    run_native_fixture_tls_probe()?;
    check_generated_docs()?;
    check_architecture_boundaries()?;
    validate_release_artifacts()?;
    validate_license_policy()?;
    run("cargo", &["fmt", "--all", "--", "--check"])?;
    run("cargo", &["test", "--workspace", "--locked"])
}

fn run_qml_lint() -> Result<(), String> {
    let candidates = ["qmllint6", "qmllint", "/usr/lib/qt6/bin/qmllint"];
    let executable = candidates
        .into_iter()
        .find(|candidate| command_available(candidate))
        .ok_or_else(|| "qmllint is required for `cargo xtask check`".to_owned())?;
    let mut qml_files = Vec::new();
    visit_files(
        Path::new("crates/ferric-browser-engine-qt/qml"),
        &mut |path, _| {
            if path.extension().is_some_and(|extension| extension == "qml") {
                qml_files.push(path.to_owned());
            }
            Ok(())
        },
    )?;
    qml_files.sort();
    let mut import_paths = vec![qt_qml_import_path()?];
    let generated_module_path = PathBuf::from("target/cxxqt/qml_modules");
    if generated_module_path.is_dir() {
        import_paths.push(generated_module_path);
    }
    let mut arguments = vec!["--json".to_owned(), "-".to_owned()];
    for import_path in import_paths {
        arguments.push("-I".to_owned());
        arguments.push(import_path.display().to_string());
    }
    arguments.extend(qml_files.into_iter().map(|path| path.display().to_string()));
    let output = Command::new(executable)
        .args(&arguments)
        .output()
        .map_err(|error| format!("failed to run {executable}: {error}"))?;
    let critical_diagnostics = qml_lint_critical_diagnostics(&output.stdout)?;
    if !critical_diagnostics.is_empty() {
        for diagnostic in &critical_diagnostics {
            eprintln!("{diagnostic}");
        }
        return Err(format!(
            "{executable} reported {} critical QML lint diagnostic(s)",
            critical_diagnostics.len()
        ));
    }
    if !output.status.success() {
        if output.status.code() != Some(255) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "{executable} exited unexpectedly with {}: {}",
                output.status,
                stderr.trim()
            ));
        }
        eprintln!(
            "{executable} exited with {} after advisory diagnostics; no critical QML issues were reported",
            output.status
        );
    }
    Ok(())
}

fn qml_lint_critical_diagnostics(report: &[u8]) -> Result<Vec<String>, String> {
    let report = serde_json::from_slice::<serde_json::Value>(report)
        .map_err(|error| format!("could not parse qmllint JSON output: {error}"))?;
    let files = report
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "qmllint JSON output did not contain a files array".to_owned())?;
    let mut diagnostics = Vec::new();
    for file in files {
        let filename = file
            .get("filename")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown QML file>");
        let Some(warnings) = file.get("warnings").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for warning in warnings {
            let severity = warning
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if !matches!(severity, "critical" | "error" | "fatal") {
                continue;
            }
            let line = warning
                .get("line")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let column = warning
                .get("column")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let message = warning
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown qmllint error");
            diagnostics.push(format!("{filename}:{line}:{column}: {message}"));
        }
    }
    Ok(diagnostics)
}

fn qt_qml_import_path() -> Result<PathBuf, String> {
    for executable in [
        "qtpaths6",
        "qtpaths",
        "/usr/lib/qt6/bin/qtpaths6",
        "/usr/lib/qt6/bin/qtpaths",
    ] {
        let Ok(output) = Command::new(executable)
            .args(["--query", "QT_INSTALL_QML"])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let Ok(stdout) = String::from_utf8(output.stdout) else {
            continue;
        };
        let path = PathBuf::from(stdout.trim());
        if path.is_dir() {
            return Ok(path);
        }
    }
    Err("could not query Qt's installed QML import directory with qtpaths".to_owned())
}

fn run_qml_ui_tests() -> Result<(), String> {
    let candidates = [
        "qmltestrunner6",
        "qmltestrunner",
        "/usr/lib/qt6/bin/qmltestrunner",
    ];
    let executable = candidates
        .into_iter()
        .find(|candidate| command_available(candidate))
        .ok_or_else(|| {
            "qmltestrunner is required for browser chrome interaction tests".to_owned()
        })?;
    let status = Command::new(executable)
        .args(["-input", "crates/ferric-browser-engine-qt/tests/qml"])
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QPA_PLATFORMTHEME", "")
        .env("QT_QUICK_BACKEND", "software")
        .status()
        .map_err(|error| format!("failed to run {executable}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{executable} chrome interaction tests exited with {status}"
        ))
    }
}

fn run_geometry_sidecar_probe() -> Result<(), String> {
    const MARKER: &str = "FERRIC_GEOMETRY_RESULT:";
    const EDITABLE_MARKER: &str = "FERRIC_EDITABLE_RESULT:";
    const FRAME_MARKER: &str = "FERRIC_FRAME_RESULT:";
    const FRAME_TREE_MARKER: &str = "FERRIC_FRAME_TREE_RESULT:";
    const FRAME_DEPTH_MARKER: &str = "FERRIC_FRAME_DEPTH_RESULT:";
    const FRAME_BRANCH_MARKER: &str = "FERRIC_FRAME_BRANCH_RESULT:";
    const POST_RESULT_MARKER: &str = "FERRIC_POST_RESULT_SIDECAR:";
    let candidates = [
        "qmltestrunner6",
        "qmltestrunner",
        "/usr/lib/qt6/bin/qmltestrunner",
    ];
    let executable = candidates
        .into_iter()
        .find(|candidate| command_available(candidate))
        .ok_or_else(|| "qmltestrunner is required for native geometry validation".to_owned())?;
    let mut fixture = start_blocking_fixture_server("127.0.0.1:0", None)?;
    let fixture_url = read_fixture_url_named(
        &mut fixture,
        "native page-state frame fixture server",
        "/versioning/frames",
    )?;
    let fixture_qml_dir = prepare_versioned_page_state_qml_fixture(&fixture_url)?;
    let output = Command::new("timeout")
        .arg("45s")
        .arg(executable)
        .arg("-input")
        .arg(fixture_qml_dir.path())
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QPA_PLATFORMTHEME", "")
        .env("QT_QUICK_BACKEND", "software")
        .output()
        .map_err(|error| format!("failed to run geometry fixture: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "native page-state fixture failed with {}",
            output.status
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "native geometry fixture output was not UTF-8".to_owned())?;
    let summary = validate_geometry_probe(&stdout, MARKER)?;
    let editable_summary = validate_editable_probe(&stdout, EDITABLE_MARKER)?;
    let frame_summary = validate_frame_probe(&stdout, FRAME_MARKER)?;
    let frame_tree_summary = validate_frame_tree_probe(&stdout, FRAME_TREE_MARKER, &fixture_url)?;
    let frame_depth_summary =
        validate_frame_depth_probe(&stdout, FRAME_DEPTH_MARKER, &fixture_url)?;
    let frame_branch_summary =
        validate_frame_branch_probe(&stdout, FRAME_BRANCH_MARKER, &fixture_url)?;
    let post_result_summary =
        validate_post_result_probe(&stdout, POST_RESULT_MARKER, &fixture_url)?;
    println!(
        "native page-state sidecars passed Rust admission (geometry: {} candidates, {} positions; editable: {} values, {} exclusions; cross-origin frame: {} captured, {} exclusions; same-origin frame tree: {} captured, {} exclusions; depth-limited frame tree: {} captured, {} exclusions; branched frame tree: {} captured, {} exclusions; POST result: {} frame)",
        summary.candidate_count,
        summary.position_count,
        editable_summary.editable_count,
        editable_summary.exclusion_count,
        frame_summary.frame_count,
        frame_summary.exclusion_count,
        frame_tree_summary.frame_count,
        frame_tree_summary.exclusion_count,
        frame_depth_summary.frame_count,
        frame_depth_summary.exclusion_count,
        frame_branch_summary.frame_count,
        frame_branch_summary.exclusion_count,
        post_result_summary.frame_count
    );
    Ok(())
}

fn run_native_profile_download_probe() -> Result<(), String> {
    const SOURCE: &str =
        "crates/ferric-browser-engine-qt/tests/qml/tst_profile_download_native.qml";
    const DIRECTORY_MARKER: &str = "\"__FERRIC_PROFILE_DOWNLOAD_DIR__\"";
    const URL_MARKER: &str = "\"__FERRIC_PROFILE_DOWNLOAD_URL__\"";
    const COMPONENT_IMPORT: &str = "import \"../../qml/components\" as FerricComponents";
    const ROUTING_IMPORT: &str =
        "import \"../../qml/scripts/ProfileDownloadRouting.js\" as ProfileDownloadRouting";
    const RESULT_MARKER: &str = "FERRIC_PROFILE_DOWNLOAD_RESULT:3/3";
    let executable = [
        "qmltestrunner6",
        "qmltestrunner",
        "/usr/lib/qt6/bin/qmltestrunner",
    ]
    .into_iter()
    .find(|candidate| command_available(candidate))
    .ok_or_else(|| "qmltestrunner is required for native profile download validation".to_owned())?;
    let temporary = DisposableDirectory::new("ferric-profile-download-qml")?;
    let source = fs::read_to_string(SOURCE)
        .map_err(|error| format!("could not read native profile download fixture: {error}"))?;
    if source.matches(DIRECTORY_MARKER).count() != 1
        || source.matches(URL_MARKER).count() != 1
        || source.matches(COMPONENT_IMPORT).count() != 1
        || source.matches(ROUTING_IMPORT).count() != 1
    {
        return Err("native profile download fixture marker is invalid".to_owned());
    }
    let output_directory = temporary
        .path()
        .to_str()
        .ok_or_else(|| "disposable profile download path is not UTF-8".to_owned())?;
    let quoted_directory = serde_json::to_string(output_directory)
        .map_err(|error| format!("could not quote disposable download path: {error}"))?;
    let executable_path = std::env::current_exe()
        .map_err(|error| format!("could not locate xtask for download fixture: {error}"))?;
    let mut fixture = ChildGuard::spawn(
        Command::new(executable_path)
            .args(["fixture-server", "--bind", "127.0.0.1:0", "--once"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
        "native profile download fixture server",
    )?;
    let slow_download_url = read_fixture_url_named(
        &mut fixture,
        "native profile download fixture server",
        "/versioning/slow-download",
    )?;
    let quoted_url = serde_json::to_string(&slow_download_url)
        .map_err(|error| format!("could not quote disposable download URL: {error}"))?;
    let staged = source
        .replace(DIRECTORY_MARKER, &quoted_directory)
        .replace(URL_MARKER, &quoted_url)
        .replace(COMPONENT_IMPORT, "import \".\" as FerricComponents")
        .replace(
            ROUTING_IMPORT,
            "import \"ProfileDownloadRouting.js\" as ProfileDownloadRouting",
        );
    fs::write(
        temporary.path().join("tst_profile_download_native.qml"),
        staged,
    )
    .map_err(|error| format!("could not stage native profile download fixture: {error}"))?;
    for source in [
        "crates/ferric-browser-engine-qt/qml/components/FerricVersioningWindowOwner.qml",
        "crates/ferric-browser-engine-qt/qml/scripts/ProfileDownloadRouting.js",
        "crates/ferric-browser-engine-qt/qml/scripts/PopupWindowOwnership.js",
    ] {
        let name = Path::new(source)
            .file_name()
            .ok_or_else(|| "native download fixture asset has no file name".to_owned())?;
        fs::copy(source, temporary.path().join(name))
            .map_err(|error| format!("could not stage native download fixture asset: {error}"))?;
    }
    let owner_component = temporary.path().join("FerricVersioningWindowOwner.qml");
    let owner_source = fs::read_to_string(&owner_component).map_err(|error| {
        format!("could not read staged native download owner component: {error}")
    })?;
    let owner_source = owner_source.replace(
        "../scripts/PopupWindowOwnership.js",
        "PopupWindowOwnership.js",
    );
    fs::write(owner_component, owner_source).map_err(|error| {
        format!("could not update staged native download owner import: {error}")
    })?;
    let output = Command::new("timeout")
        .arg("90s")
        .arg(executable)
        .arg("-input")
        .arg(temporary.path())
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QPA_PLATFORMTHEME", "")
        .env("QT_QUICK_BACKEND", "software")
        .output()
        .map_err(|error| format!("failed to run native profile download fixture: {error}"))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "native profile download fixture exited with {}\nstdout:\n{}\nstderr:\n{}",
            output.status, stdout, stderr
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "native profile download fixture output was not UTF-8".to_owned())?;
    if !stdout.contains(RESULT_MARKER) {
        return Err("native profile download fixture did not prove all callbacks".to_owned());
    }
    println!(
        "native profile download request/finish ownership passed for two views, streamed cancellation and Ferric routing"
    );
    Ok(())
}

fn run_native_viewless_download_probe() -> Result<bool, String> {
    const NAME: &str = "versioning_viewless_download_probe";
    if std::env::var("FERRIC_TEST_NATIVE_VIEWLESS_EXTENSION").as_deref() != Ok("yes") {
        println!(
            "native MV3 extension probe skipped: set FERRIC_TEST_NATIVE_VIEWLESS_EXTENSION=yes only on a disposable, explicitly selected Qt runtime"
        );
        return Ok(false);
    }
    let working = DisposableDirectory::new("ferric-viewless-download-probe")?;
    let binary = working.path().join(NAME);
    let qml = format!("crates/ferric-browser-engine-qt/tests/native/{NAME}.qml");
    compile_native_probe(
        NAME,
        &[
            "Qt6Quick",
            "Qt6Qml",
            "Qt6WebEngineQuick",
            "Qt6WebEngineCore",
            "Qt6Network",
        ],
        &binary,
        &[],
    )?;
    let output = Command::new("timeout")
        .arg("60s")
        .arg(&binary)
        .arg(&qml)
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QPA_PLATFORMTHEME", "")
        .env("QT_QUICK_BACKEND", "software")
        .output()
        .map_err(|error| format!("could not run native {NAME}: {error}"))?;
    if output.status.code() == Some(77) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let skip_reason = stdout
            .lines()
            .find_map(|line| line.strip_prefix("SKIP: "))
            .unwrap_or("Qt extension support is unavailable");
        println!("native MV3 extension probe skipped: {skip_reason}");
        return Ok(false);
    }
    if !output.status.success() {
        let fixed_failures = String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter(|line| line.starts_with("FAIL: "))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(if fixed_failures.is_empty() {
            format!("native {NAME} exited with {}", output.status)
        } else {
            format!(
                "native {NAME} exited with {}: {fixed_failures}",
                output.status
            )
        });
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| format!("native {NAME} output was not UTF-8"))?;
    if !stdout.contains("FERRIC_VIEWLESS_DOWNLOAD_RESULT:1/1") {
        return Err(format!(
            "native {NAME} did not prove the expected profile route"
        ));
    }
    println!("FERRIC_VIEWLESS_DOWNLOAD_RESULT:1/1");
    Ok(true)
}

fn prepare_versioned_page_state_qml_fixture(
    fixture_url: &str,
) -> Result<DisposableDirectory, String> {
    const URL_MARKER: &str = "__FERRIC_FRAME_FIXTURE_URL__";
    const QML_SOURCE: &str =
        "crates/ferric-browser-engine-qt/tests/qml/tst_versioned_page_state.qml";
    const BROWSER_IMPORT: &str = "import \"../../qml/scripts/BrowserScripts.js\" as BrowserScripts";
    const STATE_IMPORT: &str =
        "import \"../../qml/scripts/VersionedPageState.js\" as VersionedPageState";
    let temporary = DisposableDirectory::new("ferric-page-state-qml")?;
    let source = fs::read_to_string(QML_SOURCE)
        .map_err(|error| format!("could not read native page-state QML fixture: {error}"))?;
    if source.matches(URL_MARKER).count() != 1
        || source.matches(BROWSER_IMPORT).count() != 1
        || source.matches(STATE_IMPORT).count() != 1
        || !fixture_url.starts_with("http://127.0.0.1:")
        || fixture_url.len() > 256
    {
        return Err("native page-state frame fixture URL or template is invalid".to_owned());
    }
    let rewritten = source
        .replace(
            BROWSER_IMPORT,
            "import \"BrowserScripts.js\" as BrowserScripts",
        )
        .replace(
            STATE_IMPORT,
            "import \"VersionedPageState.js\" as VersionedPageState",
        )
        .replace(URL_MARKER, fixture_url);
    fs::write(
        temporary.path().join("tst_versioned_page_state.qml"),
        rewritten,
    )
    .map_err(|error| format!("could not stage native page-state QML fixture: {error}"))?;
    for name in ["BrowserScripts.js", "VersionedPageState.js"] {
        let source_path = Path::new("crates/ferric-browser-engine-qt/qml/scripts").join(name);
        fs::copy(&source_path, temporary.path().join(name)).map_err(|error| {
            format!("could not stage native page-state dependency {name}: {error}")
        })?;
    }
    Ok(temporary)
}

fn parse_probe_sidecar(
    stdout: &str,
    marker: &str,
    label: &str,
) -> Result<(PageStateSidecar, SidecarSummary), String> {
    let result = stdout
        .lines()
        .find_map(|line| line.split_once(marker).map(|(_, json)| json))
        .ok_or_else(|| format!("native {label} fixture emitted no sidecar result"))?;
    let sidecar = PageStateSidecar::parse_json(result.as_bytes())
        .map_err(|error| format!("native {label} result failed Rust admission: {error}"))?;
    let summary = sidecar.summary();
    Ok((sidecar, summary))
}

fn validate_geometry_probe(stdout: &str, marker: &str) -> Result<SidecarSummary, String> {
    let (sidecar, summary) = parse_probe_sidecar(stdout, marker, "geometry")?;
    if !sidecar.qualifies_for(
        "https://example.test",
        &"b".repeat(64),
        &"a".repeat(64),
        &"1".repeat(32),
    ) || summary.frame_count != 1
        || summary.candidate_count < 1
        || summary.position_count < 3
        || summary.editable_count != 0
    {
        return Err("native geometry result did not match the admitted fixture".to_owned());
    }
    Ok(summary)
}

fn validate_editable_probe(stdout: &str, marker: &str) -> Result<SidecarSummary, String> {
    let (sidecar, summary) = parse_probe_sidecar(stdout, marker, "editable")?;
    if !sidecar.qualifies_for(
        "https://example.test",
        &"b".repeat(64),
        &"a".repeat(64),
        &"1".repeat(32),
    ) || summary.editable_count < 3
        || summary.exclusion_count == 0
        || summary.position_count < 4
    {
        return Err("native editable result did not match the admitted fixture".to_owned());
    }
    Ok(summary)
}

fn validate_frame_probe(stdout: &str, marker: &str) -> Result<SidecarSummary, String> {
    let (sidecar, summary) = parse_probe_sidecar(stdout, marker, "frame")?;
    if !sidecar.qualifies_for(
        "https://example.test",
        &"b".repeat(64),
        &"a".repeat(64),
        &"1".repeat(32),
    ) || summary.frame_count != 1
        || summary
            .exclusions_by_reason
            .get(&ferric_browser_core::versioning::ExclusionReason::CrossOrigin)
            != Some(&1)
        || summary
            .coverage
            .iter()
            .any(|coverage| !coverage.truncated())
    {
        return Err(
            "native frame result did not preserve bounded cross-origin exclusion".to_owned(),
        );
    }
    Ok(summary)
}

fn validate_frame_tree_probe(
    stdout: &str,
    marker: &str,
    fixture_url: &str,
) -> Result<SidecarSummary, String> {
    let (sidecar, summary) = parse_probe_sidecar(stdout, marker, "same-origin frame tree")?;
    let expected_origin = fixture_url
        .strip_suffix("/versioning/frames")
        .ok_or_else(|| "native frame fixture URL had an unexpected route".to_owned())?;
    if !sidecar.qualifies_for(
        expected_origin,
        &"b".repeat(64),
        &"a".repeat(64),
        &"1".repeat(32),
    ) || summary.frame_count != 2
        || summary
            .exclusions_by_reason
            .get(&ferric_browser_core::versioning::ExclusionReason::CrossOrigin)
            != Some(&1)
        || summary
            .coverage
            .iter()
            .any(|coverage| !coverage.truncated())
    {
        return Err(
            "native frame-tree result did not admit the same-origin child and bound cross-origin sibling"
                .to_owned(),
        );
    }
    Ok(summary)
}

fn validate_frame_depth_probe(
    stdout: &str,
    marker: &str,
    fixture_url: &str,
) -> Result<SidecarSummary, String> {
    let (sidecar, summary) = parse_probe_sidecar(stdout, marker, "depth-limited frame tree")?;
    let expected_origin = fixture_url
        .strip_suffix("/versioning/frames")
        .ok_or_else(|| "native frame fixture URL had an unexpected route".to_owned())?;
    if !sidecar.qualifies_for(
        expected_origin,
        &"b".repeat(64),
        &"a".repeat(64),
        &"1".repeat(32),
    ) || summary.frame_count != 9
        || summary
            .exclusions_by_reason
            .get(&ferric_browser_core::versioning::ExclusionReason::Limit)
            != Some(&1)
        || summary
            .coverage
            .iter()
            .any(|coverage| !coverage.truncated())
    {
        return Err("native depth-limited frame result was incomplete or overclaimed".to_owned());
    }
    Ok(summary)
}

fn validate_frame_branch_probe(
    stdout: &str,
    marker: &str,
    fixture_url: &str,
) -> Result<SidecarSummary, String> {
    let (sidecar, summary) = parse_probe_sidecar(stdout, marker, "branched frame tree")?;
    let origin = fixture_url
        .strip_suffix("/versioning/frames")
        .ok_or_else(|| "native frame fixture URL had an unexpected route".to_owned())?;
    if !sidecar.qualifies_for(origin, &"b".repeat(64), &"a".repeat(64), &"1".repeat(32))
        || summary.frame_count != 6
        || summary
            .exclusions_by_reason
            .get(&ferric_browser_core::versioning::ExclusionReason::CrossOrigin)
            != Some(&1)
    {
        return Err("native branched frame result failed bounded Rust admission".to_owned());
    }
    let encoded = stdout
        .lines()
        .find_map(|line| line.split_once(marker).map(|(_, json)| json))
        .ok_or_else(|| "native branched frame result is missing".to_owned())?;
    if encoded.contains("/versioning/frame-branch") || encoded.contains("Frame branch") {
        return Err("native branched frame result disclosed a frame URL or title".to_owned());
    }
    let document: serde_json::Value = serde_json::from_str(encoded)
        .map_err(|error| format!("native branched frame JSON is invalid: {error}"))?;
    let frames = document["frames"]
        .as_array()
        .ok_or_else(|| "native branched frame records are missing".to_owned())?;
    let mut by_route = BTreeMap::new();
    for frame in frames {
        let route = frame["route_fingerprint"]
            .as_str()
            .ok_or_else(|| "native branched frame fingerprint is missing".to_owned())?;
        let id = frame["id"]
            .as_u64()
            .ok_or_else(|| "native branched frame ID is missing".to_owned())?;
        let parent = if frame["parent"].is_null() {
            None
        } else {
            Some(
                frame["parent"]
                    .as_u64()
                    .ok_or_else(|| "native branched frame parent is invalid".to_owned())?,
            )
        };
        if by_route.insert(route.to_owned(), (id, parent)).is_some() {
            return Err("native branched frame route was duplicated".to_owned());
        }
    }
    if by_route.get(&"b".repeat(64)) != Some(&(0, None)) || by_route.len() != 6 {
        return Err("native branched frame root or count was invalid".to_owned());
    }
    let find = |route: &str| -> Result<(u64, Option<u64>), String> {
        let url = format!("{origin}{route}");
        let fingerprint = route_fingerprint_for_url(&url)
            .ok_or_else(|| "native branched fixture URL was invalid".to_owned())?;
        by_route
            .get(&fingerprint)
            .copied()
            .ok_or_else(|| format!("native branched frame is missing: {route}"))
    };
    let (left, left_parent) = find("/versioning/frame-branch-left")?;
    let (right, right_parent) = find("/versioning/frame-branch-right")?;
    if left_parent != Some(0)
        || right_parent != Some(0)
        || find("/versioning/frame-branch-leaf?branch=left-a")?.1 != Some(left)
        || find("/versioning/frame-branch-leaf?branch=left-b")?.1 != Some(left)
        || find("/versioning/frame-branch-leaf?branch=right-a")?.1 != Some(right)
    {
        return Err("native branched frame parents did not match their routes".to_owned());
    }
    Ok(summary)
}

fn validate_post_result_probe(
    stdout: &str,
    marker: &str,
    fixture_url: &str,
) -> Result<SidecarSummary, String> {
    let (sidecar, summary) = parse_probe_sidecar(stdout, marker, "POST-result page")?;
    let expected_origin = fixture_url
        .strip_suffix("/versioning/frames")
        .ok_or_else(|| "native POST-result fixture URL had an unexpected route".to_owned())?;
    if !sidecar.qualifies_for(
        expected_origin,
        &"b".repeat(64),
        &"a".repeat(64),
        &"1".repeat(32),
    ) || summary.frame_count != 1
        || summary.editable_count != 0
    {
        return Err("native POST-result sidecar did not match its capture challenge".to_owned());
    }
    Ok(summary)
}

fn run_native_archive_probe() -> Result<(), String> {
    const NAME: &str = "versioning_archive_probe";
    const PACKAGES: &[&str] = &[
        "Qt6Quick",
        "Qt6Qml",
        "Qt6WebEngineQuick",
        "Qt6WebEngineCore",
        "Qt6Network",
        "Qt6Test",
    ];
    let viewless_download_supported = run_native_viewless_download_probe()?;
    // The probe links the same C++ request interceptor used by the production
    // QML component, including CXX-Qt's generated meta-object implementation.
    let meta_object =
        generated_qt_moc_source("moc_versioning_archive_interceptor.h.cpp")?.filter(|path| {
            let header =
                Path::new("crates/ferric-browser-engine-qt/src/versioning_archive_interceptor.h");
            let generated_time = fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            let header_time = fs::metadata(header)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::now());
            generated_time >= header_time
        });
    let meta_object = if let Some(meta_object) = meta_object {
        meta_object
    } else {
        run(
            "cargo",
            &[
                "check",
                "-p",
                "ferric-browser-engine-qt",
                "--locked",
                "--offline",
            ],
        )?;
        generated_qt_moc_source("moc_versioning_archive_interceptor.h.cpp")?
            .ok_or_else(|| "CXX-Qt archive interceptor meta-object is unavailable".to_owned())?
    };
    let meta_object = meta_object.to_string_lossy().into_owned();
    let extra_sources = [
        "crates/ferric-browser-engine-qt/src/versioning_archive_interceptor.cpp",
        meta_object.as_str(),
    ];
    run_native_input_qml_probe_with_sources(
        NAME,
        PACKAGES,
        &extra_sources,
        viewless_download_supported,
    )
}

fn run_native_cookie_probe() -> Result<(), String> {
    run_native_qml_probe(
        "versioning_cookie_probe",
        &[
            "Qt6Quick",
            "Qt6Qml",
            "Qt6WebEngineQuick",
            "Qt6WebEngineCore",
            "Qt6Network",
            "Qt6Gui",
        ],
    )
}

fn run_native_cookie_adapter_probe() -> Result<(), String> {
    const NAME: &str = "versioning_cookie_adapter_probe";
    const PACKAGES: &[&str] = &[
        "Qt6Quick",
        "Qt6Qml",
        "Qt6WebEngineQuick",
        "Qt6WebEngineCore",
        "Qt6Network",
        "Qt6Gui",
    ];
    let working = DisposableDirectory::new("ferric-cookie-adapter-probe")?;
    let binary = working.path().join(NAME);
    compile_native_probe(
        NAME,
        PACKAGES,
        &binary,
        &["crates/ferric-browser-engine-qt/src/versioning_cookie_adapter.cpp"],
    )?;
    let output = Command::new("timeout")
        .arg("45s")
        .arg(&binary)
        .arg("crates/ferric-browser-engine-qt/tests/native/versioning_cookie_probe.qml")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QPA_PLATFORMTHEME", "")
        .env("QT_QUICK_BACKEND", "software")
        .output()
        .map_err(|error| format!("could not run {NAME}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{NAME} exited with {}", output.status));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.contains("PASS: bounded Quick cookie observation")
        || stdout.contains("synthetic-secret")
        || stderr.contains("synthetic-secret")
        || stdout.contains("synthetic-name")
        || stderr.contains("synthetic-name")
    {
        return Err(
            "native cookie adapter output was missing or contained secret fixture bytes".to_owned(),
        );
    }
    println!("native Quick cookie adapter passed bounded observation, overflow and teardown");
    Ok(())
}

fn run_native_cookie_boundary_probe() -> Result<(), String> {
    const NAME: &str = "versioning_cookie_boundary_probe";
    const PACKAGES: &[&str] = &[
        "Qt6Quick",
        "Qt6Qml",
        "Qt6WebEngineQuick",
        "Qt6WebEngineCore",
        "Qt6Network",
        "Qt6Gui",
    ];
    let working = DisposableDirectory::new("ferric-cookie-boundary-probe")?;
    let binary = working.path().join(NAME);
    compile_native_probe(
        NAME,
        PACKAGES,
        &binary,
        &["crates/ferric-browser-engine-qt/src/versioning_cookie_adapter.cpp"],
    )?;
    let qml = "crates/ferric-browser-engine-qt/tests/native/versioning_cookie_boundary_probe.qml";
    for (stage, marker) in [
        ("seed", "PASS: disposable persistent fixture seeded"),
        (
            "observe",
            "PASS: persistent source, concurrent write, JS mutation and expiration observed",
        ),
    ] {
        let output = Command::new("timeout")
            .arg("45s")
            .arg(&binary)
            .args([qml, stage])
            .env("QT_QPA_PLATFORM", "offscreen")
            .env("QT_QPA_PLATFORMTHEME", "")
            .env("QT_QUICK_BACKEND", "software")
            .env("XDG_CONFIG_HOME", working.path().join("config"))
            .env("XDG_CACHE_HOME", working.path().join("cache"))
            .env("XDG_DATA_HOME", working.path().join("data"))
            .output()
            .map_err(|error| format!("could not run {NAME} {stage}: {error}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            let reason = stderr
                .lines()
                .find_map(|line| match line {
                    "FAIL: load disposable persistent Quick fixture" => Some("QML fixture load"),
                    "FAIL: concurrent cookie absent from observer" => {
                        Some("concurrent cookie observation")
                    }
                    "FAIL: JavaScript mutation reached native observer" => {
                        Some("JavaScript observation")
                    }
                    "FAIL: server truth confirms persistent seed was in Chromium jar" => {
                        Some("persisted-cookie request truth")
                    }
                    "FAIL: short-lived cookie reached observer" => Some("short-lived observation"),
                    "FAIL: post-expiration browser request reached local server" => {
                        Some("post-expiration request")
                    }
                    "FAIL: server truth shows retained, JS and expired-cookie states" => {
                        Some("expiry request truth")
                    }
                    _ => None,
                })
                .unwrap_or("native fixture failure");
            return Err(format!(
                "{NAME} {stage} exited with {} ({reason})",
                output.status
            ));
        }
        if !stdout.contains(marker)
            || stdout.contains("synthetic-")
            || stderr.contains("synthetic-")
            || stdout.contains("vb_boundary_")
            || stderr.contains("vb_boundary_")
        {
            return Err(format!(
                "{NAME} {stage} output was missing or contained cookie fixture bytes"
            ));
        }
        if stage == "observe" {
            let callback = if stdout.contains("initial seed callback=yes") {
                "yes"
            } else if stdout.contains("initial seed callback=no") {
                "no"
            } else {
                return Err("native cookie boundary callback status was missing".into());
            };
            println!(
                "persistent cookie request truth passed; initial observer callback: {callback}"
            );
        }
    }
    let working_path = working.path().to_path_buf();
    drop(working);
    if working_path.exists() {
        return Err("disposable cookie boundary profile root remains after teardown".into());
    }
    println!("native cookie boundary stress passed; no exact snapshot inferred");
    Ok(())
}

fn run_native_clean_branch_profile_probe() -> Result<(), String> {
    const NAME: &str = "versioning_clean_branch_profile_probe";
    const PACKAGES: &[&str] = &[
        "Qt6Quick",
        "Qt6Qml",
        "Qt6WebEngineQuick",
        "Qt6WebEngineCore",
        "Qt6Network",
        "Qt6Gui",
    ];
    let working = DisposableDirectory::new("ferric-clean-branch-profile")?;
    let binary = working.path().join(NAME);
    compile_native_probe(NAME, PACKAGES, &binary, &[])?;
    let output = Command::new("timeout")
        .arg("30s")
        .arg(&binary)
        .arg("crates/ferric-browser-engine-qt/tests/native/versioning_clean_branch_profile_probe.qml")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QPA_PLATFORMTHEME", "")
        .env("QT_QUICK_BACKEND", "software")
        .env("XDG_CONFIG_HOME", working.path().join("config"))
        .env("XDG_CACHE_HOME", working.path().join("cache"))
        .env("XDG_DATA_HOME", working.path().join("data"))
        .output()
        .map_err(|error| format!("could not run {NAME}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{NAME} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if !String::from_utf8_lossy(&output.stdout)
        .contains("PASS: distinct memory-only Quick branch profiles and cookie stores")
    {
        return Err(format!(
            "{NAME} did not report the qualified branch profile result"
        ));
    }
    println!("native clean-branch profile and cookie separation passed");
    Ok(())
}

fn run_native_fixture_tls_probe() -> Result<(), String> {
    const PACKAGES: &[&str] = &["Qt6Gui", "Qt6WebEngineCore", "Qt6Network"];
    if !command_available("openssl") || !command_available("socat") || !command_available("curl") {
        println!(
            "native HTTPS fixture qualification skipped/unqualified: openssl, socat and curl are required"
        );
        return Ok(());
    }
    if !Command::new("pkg-config")
        .args(["--exists"])
        .args(PACKAGES)
        .status()
        .is_ok_and(|status| status.success())
    {
        println!(
            "native HTTPS fixture qualification skipped/unqualified: Qt WebEngine Core development packages are unavailable"
        );
        return Ok(());
    }

    let working = DisposableDirectory::new("ferric-native-fixture-tls")?;
    let binary = working.path().join("versioning_fixture_tls_probe");
    compile_native_probe("versioning_fixture_tls_probe", PACKAGES, &binary, &[])?;
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate xtask for HTTPS fixture: {error}"))?;
    let mut fixture = ChildGuard::spawn(
        Command::new(executable)
            .args([
                "fixture-server",
                "--https",
                "--exit-on-path",
                "/versioning/fault/close",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
        "native HTTPS fixture server",
    )?;
    let (base, certificate) = read_https_fixture_details(&mut fixture)?;
    let target = format!("{base}/versioning/cookies/secure");
    let probe_status = Command::new("timeout")
        .arg("45s")
        .arg(binary)
        .arg(&target)
        .arg(&certificate)
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QPA_PLATFORMTHEME", "")
        .env("XDG_CONFIG_HOME", working.path().join("config"))
        .env("XDG_CACHE_HOME", working.path().join("cache"))
        .env("XDG_DATA_HOME", working.path().join("data"))
        .status()
        .map_err(|error| format!("could not run native HTTPS fixture probe: {error}"))?;
    // The loopback-only close route intentionally sends no HTTP response.
    // Its request is the server's graceful teardown signal, even when the
    // browser navigation failed before loading the fixture page.
    let _ = Command::new("curl")
        .args(["--cacert", certificate.to_str().unwrap_or_default()])
        .args(["--max-time", "5", "--silent", "--output", "/dev/null"])
        .arg(format!("{base}/versioning/fault/close"))
        .stderr(Stdio::null())
        .status();
    let deadline = Instant::now() + Duration::from_secs(5);
    let fixture_status = loop {
        if let Some(status) = fixture
            .child_mut("native HTTPS fixture server")?
            .try_wait()
            .map_err(|error| format!("could not poll native HTTPS fixture server: {error}"))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            return Err("native HTTPS fixture server did not tear down within 5s".into());
        }
        thread::sleep(Duration::from_millis(20));
    };
    if !fixture_status.success() {
        return Err(format!(
            "native HTTPS fixture server exited with {fixture_status}"
        ));
    }
    if certificate.exists() {
        return Err("disposable HTTPS fixture certificate remains after teardown".into());
    }
    if !probe_status.success() {
        return Err(format!(
            "native HTTPS fixture probe exited with {probe_status}"
        ));
    }
    let working_path = working.path().to_path_buf();
    drop(working);
    if working_path.exists() {
        return Err("disposable native fixture profile root remains after teardown".into());
    }
    println!("native HTTPS fixture profile and certificate teardown passed");
    Ok(())
}

fn read_https_fixture_details(fixture: &mut ChildGuard) -> Result<(String, PathBuf), String> {
    let stdout = fixture
        .child_mut("native HTTPS fixture server")?
        .stdout
        .as_mut()
        .ok_or_else(|| "native HTTPS fixture stdout is unavailable".to_owned())?;
    let mut reader = BufReader::new(stdout);
    let mut ready = String::new();
    let mut cert = String::new();
    reader
        .read_line(&mut ready)
        .map_err(|error| format!("could not read HTTPS fixture readiness: {error}"))?;
    reader
        .read_line(&mut cert)
        .map_err(|error| format!("could not read HTTPS fixture certificate: {error}"))?;
    let base = ready
        .trim()
        .strip_prefix("FERRIC_BROWSER_FIXTURE_READY ")
        .filter(|base| base.starts_with("https://127.0.0.1:"))
        .ok_or_else(|| format!("invalid HTTPS fixture readiness: {ready:?}"))?
        .to_owned();
    let certificate = cert
        .trim()
        .strip_prefix("FERRIC_BROWSER_FIXTURE_CERT ")
        .map(PathBuf::from)
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .is_some_and(|name| name == "fixture-cert.pem")
        })
        .ok_or_else(|| format!("invalid HTTPS fixture certificate: {cert:?}"))?;
    Ok((base, certificate))
}

fn run_native_qml_probe(name: &str, packages: &[&str]) -> Result<(), String> {
    run_native_qml_probe_with_sources(name, packages, &[])
}

fn run_native_qml_probe_with_sources(
    name: &str,
    packages: &[&str],
    extra_sources: &[&str],
) -> Result<(), String> {
    let working = DisposableDirectory::new("ferric-native-versioning-probe")?;
    let binary = working.path().join(name);
    let qml = format!("crates/ferric-browser-engine-qt/tests/native/{name}.qml");
    compile_native_probe(name, packages, &binary, extra_sources)?;
    let status = Command::new("timeout")
        .arg("45s")
        .arg(&binary)
        .arg(&qml)
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QPA_PLATFORMTHEME", "")
        .env("QT_QUICK_BACKEND", "software")
        .status()
        .map_err(|error| format!("could not run {name}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{name} exited with {status}"))
    }
}

fn run_native_input_qml_probe_with_sources(
    name: &str,
    packages: &[&str],
    extra_sources: &[&str],
    viewless_download_supported: bool,
) -> Result<(), String> {
    let working = DisposableDirectory::new("ferric-native-input-versioning-probe")?;
    let binary = working.path().join(name);
    let archive_output = working.path().join("verified-capture.mhtml");
    let qml = format!("crates/ferric-browser-engine-qt/tests/native/{name}.qml");
    compile_native_probe(name, packages, &binary, extra_sources)?;

    if run_disposable_weston_input_probe(name, &binary, &qml, &archive_output)? {
        run_native_archive_adapter_fixture(&archive_output, viewless_download_supported)?;
        return Ok(());
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_none()
        || std::env::var_os("XDG_RUNTIME_DIR").is_none()
    {
        println!(
            "native Wayland input qualification not run for {name}: no active Wayland session; archive release gate remains disabled"
        );
        return Ok(());
    }

    let output = Command::new("timeout")
        .arg("45s")
        .arg(&binary)
        .arg(&qml)
        .arg(&archive_output)
        .env("QT_QPA_PLATFORM", "wayland")
        .env("QT_QPA_PLATFORMTHEME", "")
        .env("QT_QUICK_BACKEND", "software")
        .env("GIO_USE_VFS", "local")
        .output()
        .map_err(|error| format!("could not run native-input {name}: {error}"))?;
    if output.status.code() == Some(77) {
        println!(
            "native Wayland input qualification not completed for {name}; archive release gate remains disabled; stdout={:?}; stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }
    if !output.status.success() {
        return Err(format!(
            "native-input {name} exited with {}; stdout={:?}; stderr={:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    println!("native Wayland input probe passed: {name}");
    run_native_archive_adapter_fixture(&archive_output, viewless_download_supported)?;
    Ok(())
}

const NATIVE_ARCHIVE_JOURNEYS: [&str; 42] = [
    "native_qt_mhtml_file_commits_through_durable_adapter",
    "native_qt_page_collects_saves_and_publishes_through_production_window",
    "native_qt_two_tabs_publish_one_strict_window_capture",
    "native_qt_strict_window_publishes_nothing_after_first_tab_closes",
    "native_qt_partial_window_retains_one_page_after_one_cancelled_save",
    "native_qt_partial_window_retains_first_page_when_second_save_times_out",
    "native_qt_partial_window_retains_one_page_after_first_page_navigation",
    "native_qt_partial_window_retains_one_page_after_first_save_deadline",
    "native_qt_partial_window_retains_one_page_after_first_tab_closes",
    "native_qt_partial_window_retains_first_page_after_second_tab_closes_during_save",
    "native_qt_partial_window_retains_saved_page_after_its_tab_closes",
    "native_qt_partial_window_defers_close_until_archive_install_finishes",
    "native_qt_partial_window_retains_one_page_after_archive_copy_storage_full",
    "native_qt_partial_window_retains_first_page_after_second_archive_copy_storage_full",
    "native_qt_committed_geometry_applies_in_pristine_restore_window",
    "native_qt_committed_multi_page_geometry_keeps_inactive_restore_lazy",
    "native_qt_committed_draft_requires_review_before_live_refill",
    "native_qt_mutating_page_publishes_partial_fidelity",
    "native_qt_post_result_publishes_nonreplayable_archive",
    "native_qt_post_redirect_303_publishes_result_without_replay",
    "native_qt_post_redirect_307_preserves_body_and_publishes",
    "native_qt_secondary_post_result_does_not_replay_as_get",
    "native_qt_frame_tree_excludes_cross_origin_and_publishes",
    "native_qt_editor_opt_in_publishes_bounded_text_draft",
    "native_qt_selection_opt_in_publishes_offsets_without_selected_text",
    "native_qt_media_metadata_publishes_without_playback",
    "native_qt_committed_paused_media_position_restores_without_playback",
    "native_qt_navigation_during_capture_does_not_publish_mixed_document",
    "native_qt_removed_child_frame_during_save_does_not_publish_mixed_document",
    "native_qt_same_url_child_reload_during_save_does_not_publish_mixed_document",
    "native_qt_cancelled_archive_after_collector_does_not_publish",
    "native_qt_declined_capture_requires_new_one_use_disclosure",
    "native_qt_collector_timeout_keeps_successful_archive_only",
    "native_qt_ui_responds_while_archive_worker_copy_is_paused",
    "native_qt_cancel_quit_waits_for_archive_worker_drain",
    "native_qt_archive_install_storage_full_publishes_no_commit",
    "native_qt_sidecar_worker_refusal_keeps_successful_archive_only",
    "native_qt_sidecar_filesystem_denial_keeps_successful_archive_only",
    "native_qt_archive_closes_through_real_browser_window_and_storage",
    "native_qt_archive_unsafe_cleanup_retains_file_through_shutdown",
    "native_qt_archive_close_then_switches_to_fresh_profile",
    "native_qt_archive_cleanup_retries_after_one_worker_refusal",
];

fn run_native_archive_adapter_fixture(
    archive_output: &Path,
    viewless_download_supported: bool,
) -> Result<(), String> {
    if !archive_output.is_file() {
        return Err("native archive probe did not export its verified MHTML".to_owned());
    }
    // Build the lib-test executable once. Cargo may rebuild CXX-Qt between
    // separate `cargo test` invocations, and Qt cannot create two GUI
    // applications in the same process. Execute the ignored legs as
    // separate processes of this exact artifact.
    let build = Command::new("timeout")
        .args([
            "420s",
            "cargo",
            "test",
            "-p",
            "ferric-browser-engine-qt",
            "--lib",
            "--locked",
            "--offline",
            "--features",
            "native-archive-fault-test",
            "--no-run",
            "--message-format=json",
        ])
        .output()
        .map_err(|error| format!("could not build native archive adapter journeys: {error}"))?;
    if !build.status.success() {
        return Err(format!(
            "native archive adapter test build exited with {}; stderr={:?}",
            build.status,
            String::from_utf8_lossy(&build.stderr)
        ));
    }
    let executable = String::from_utf8_lossy(&build.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|message| {
            message["reason"] == "compiler-artifact"
                && message["target"]["name"] == "ferric_browser_engine_qt"
                && message["target"]["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "lib"))
                && message["executable"].is_string()
        })
        .and_then(|message| message["executable"].as_str().map(str::to_owned))
        .ok_or("cargo did not report the Qt lib-test executable")?;
    let mut capture_fixture = start_blocking_fixture_server("127.0.0.1:0", None)?;
    let capture_origin = read_fixture_url_named(
        &mut capture_fixture,
        "native page-capture fixture server",
        "",
    )?;
    let selected_journey = std::env::var("FERRIC_NATIVE_ARCHIVE_JOURNEY").ok();
    if selected_journey
        .as_deref()
        .is_some_and(|selected| !NATIVE_ARCHIVE_JOURNEYS.contains(&selected))
        && let Some(selected) = selected_journey.as_deref()
    {
        return Err(format!("unknown native archive journey filter: {selected}"));
    }
    for test in NATIVE_ARCHIVE_JOURNEYS {
        if selected_journey
            .as_deref()
            .is_some_and(|selected| selected != test)
        {
            continue;
        }
        let capture_path = native_archive_capture_path(test);
        // The native journey has distinct bounded profile, load, capture,
        // shutdown and reopen phases. Leave enough wall time for their own
        // assertions to identify the failing phase on a slow Qt startup.
        let status = Command::new("timeout")
            .args(["180s", &executable, test, "--ignored", "--nocapture"])
            .env("FERRIC_NATIVE_ARCHIVE_FIXTURE", archive_output)
            .env(
                "FERRIC_NATIVE_CAPTURE_URL",
                format!("{capture_origin}{capture_path}"),
            )
            .env("QT_QPA_PLATFORM", "offscreen")
            .env("QT_QPA_PLATFORMTHEME", "")
            .env("QT_QUICK_BACKEND", "software")
            .status()
            .map_err(|error| format!("could not run native archive journey {test}: {error}"))?;
        if !status.success() {
            let fixture_output =
                stop_fixture_server(&mut capture_fixture, "native page-capture fixture server")?;
            return Err(format!(
                "native archive journey {test} exited with {status}; fixture output={fixture_output:?}"
            ));
        }
    }
    if viewless_download_supported
        && selected_journey.as_deref().is_none_or(|selected| {
            selected == "native_qt_viewless_extension_download_completes_in_profile_ledger"
        })
    {
        let status = Command::new("timeout")
            .args([
                "180s",
                &executable,
                "native_qt_viewless_extension_download_completes_in_profile_ledger",
                "--ignored",
                "--nocapture",
            ])
            .env(
                "FERRIC_NATIVE_VIEWLESS_DOWNLOAD_URL",
                format!("{capture_origin}/versioning/archive-download"),
            )
            .env("QT_QPA_PLATFORM", "offscreen")
            .env("QT_QPA_PLATFORMTHEME", "")
            .env("QT_QUICK_BACKEND", "software")
            .status()
            .map_err(|error| format!("could not run native viewless ledger journey: {error}"))?;
        if !status.success() {
            let fixture_output =
                stop_fixture_server(&mut capture_fixture, "native page-capture fixture server")?;
            return Err(format!(
                "native viewless download ledger journey exited with {status}; fixture output={fixture_output:?}"
            ));
        }
        println!("native viewless extension download completed through the durable profile ledger");
    } else {
        println!(
            "native durable viewless-download journey skipped because public extension activation is not qualified on this Qt build"
        );
    }
    let fixture_output =
        stop_fixture_server(&mut capture_fixture, "native page-capture fixture server")?;
    if selected_journey.is_none() {
        validate_native_post_fixture_output(&fixture_output)?;
        validate_native_redirect_fixture_output(&fixture_output)?;
    } else {
        println!("filtered native archive journey: suite-wide POST fixture assertions skipped");
    }
    println!(
        "native Qt archive passed durable adapter, strict two-tab and cancelled-page, navigated-page, timed-out-page, second-page view-loss, first-page and second-page storage-full partial two-tab windows, stable/mutating/primary-and-secondary-POST/303-and-307-redirect/frame/editor/selection/media/navigation/removed-and-same-URL-reloaded-child-frame/cancelled-save/declined-and-fresh-consent/collector-timeout/UI-responsive-paused-archive-copy/cancel-quit-worker-drain/archive-install-ENOSPC/sidecar-worker-refusal/sidecar-filesystem-denial live captures, committed geometry, reviewed-draft and paused-media restore, and four real-window lifecycle journeys"
    );
    Ok(())
}

fn native_archive_capture_path(test: &str) -> &'static str {
    match test {
        "native_qt_mutating_page_publishes_partial_fidelity" => "/versioning/mutation",
        "native_qt_post_result_publishes_nonreplayable_archive"
        | "native_qt_secondary_post_result_does_not_replay_as_get" => "/versioning/form",
        "native_qt_post_redirect_303_publishes_result_without_replay" => "/versioning/redirect-303",
        "native_qt_post_redirect_307_preserves_body_and_publishes" => "/versioning/redirect-307",
        "native_qt_frame_tree_excludes_cross_origin_and_publishes"
        | "native_qt_removed_child_frame_during_save_does_not_publish_mixed_document"
        | "native_qt_same_url_child_reload_during_save_does_not_publish_mixed_document" => {
            "/versioning/frames"
        }
        "native_qt_editor_opt_in_publishes_bounded_text_draft"
        | "native_qt_selection_opt_in_publishes_offsets_without_selected_text" => {
            "/versioning/editor"
        }
        "native_qt_committed_draft_requires_review_before_live_refill" => {
            "/versioning/editor-refill"
        }
        "native_qt_media_metadata_publishes_without_playback"
        | "native_qt_committed_paused_media_position_restores_without_playback" => {
            "/versioning/media"
        }
        _ => "/versioning/document",
    }
}

fn validate_native_post_fixture_output(output: &str) -> Result<(), String> {
    let form_requests = output
        .lines()
        .filter(|line| line.starts_with("fixture-server: form result method="))
        .collect::<Vec<_>>();
    if form_requests.len() == 2
        && form_requests.iter().all(|request| {
            request
                .strip_prefix("fixture-server: form result method=POST body_bytes=")
                .and_then(|bytes| bytes.parse::<usize>().ok())
                .is_some_and(|bytes| bytes > 0)
        })
    {
        Ok(())
    } else {
        Err(format!(
            "native POST journeys did not issue exactly two body-bearing POSTs: {form_requests:?}"
        ))
    }
}

fn validate_native_redirect_fixture_output(output: &str) -> Result<(), String> {
    let observed = output
        .lines()
        .filter(|line| line.starts_with("fixture-server: redirect path="))
        .collect::<Vec<_>>();
    let expected = [
        "fixture-server: redirect path=/versioning/redirect-303/submit method=POST body_bytes=18",
        "fixture-server: redirect path=/versioning/redirect-303/result method=GET body_bytes=0",
        "fixture-server: redirect path=/versioning/redirect-307/submit method=POST body_bytes=18",
        "fixture-server: redirect path=/versioning/redirect-307/result method=POST body_bytes=18",
    ];
    if observed.len() == expected.len() && expected.iter().all(|line| observed.contains(line)) {
        Ok(())
    } else {
        Err(format!(
            "native POST redirect journeys changed method or replayed a request: {observed:?}"
        ))
    }
}

fn run_disposable_weston_input_probe(
    name: &str,
    binary: &Path,
    qml: &str,
    archive_output: &Path,
) -> Result<bool, String> {
    let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return Ok(false);
    };
    if !command_available("weston") {
        return Ok(false);
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("could not make Wayland probe nonce: {error}"))?
        .as_nanos();
    let socket = format!("ferric-probe-{}-{nonce}", std::process::id());
    let socket_path = PathBuf::from(runtime_dir).join(&socket);
    let mut command = Command::new("weston");
    command
        .args([
            "--backend=headless",
            "--renderer=pixman",
            "--fake-seat",
            "--shell=kiosk",
            "--idle-time=0",
        ])
        .arg(format!("--socket={socket}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let Ok(child) = command.spawn() else {
        return Ok(false);
    };
    let mut compositor = DisposableWaylandCompositor {
        child: ChildGuard { child: Some(child) },
        socket_path: socket_path.clone(),
        lock_path: socket_path.with_file_name(format!("{socket}.lock")),
    };
    for _ in 0..50 {
        if socket_path.exists() {
            break;
        }
        if compositor
            .child
            .child_mut("disposable Weston")?
            .try_wait()
            .map_err(|error| format!("could not inspect disposable Weston: {error}"))?
            .is_some()
        {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(100));
    }
    if !socket_path.exists() {
        return Ok(false);
    }
    let output = Command::new("timeout")
        .arg("45s")
        .arg(binary)
        .arg(qml)
        .arg(archive_output)
        .env("WAYLAND_DISPLAY", &socket)
        .env("QT_QPA_PLATFORM", "wayland")
        .env("QT_QPA_PLATFORMTHEME", "")
        .env("QT_QUICK_BACKEND", "software")
        .env("GIO_USE_VFS", "local")
        .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
        .output()
        .map_err(|error| format!("could not run {name} in disposable Weston: {error}"))?;
    if output.status.code() == Some(77) {
        println!(
            "disposable Weston input unavailable for {name}; trying the active Wayland session; stderr={:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(false);
    }
    if !output.status.success() {
        return Err(format!(
            "{name} in disposable Weston exited with {}; stdout={:?}; stderr={:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if name == "versioning_archive_probe" {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.starts_with("FERRIC_ARCHIVE_") && line.len() <= 256 {
                println!("{line}");
            }
        }
    }
    println!("native Wayland input probe passed in disposable Weston: {name}");
    Ok(true)
}

fn generated_qt_moc_source(file_name: &str) -> Result<Option<PathBuf>, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate the xtask executable: {error}"))?;
    let mut profile_dir = executable
        .parent()
        .ok_or_else(|| "xtask executable has no containing directory".to_owned())?;
    if profile_dir.file_name().is_some_and(|name| name == "deps") {
        profile_dir = profile_dir
            .parent()
            .ok_or_else(|| "xtask deps directory has no profile parent".to_owned())?;
    }
    let build_root = profile_dir.join("build");
    let entries = fs::read_dir(&build_root).map_err(|error| {
        format!(
            "could not read Qt build outputs {}: {error}",
            build_root.display()
        )
    })?;
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("could not inspect Qt build output: {error}"))?;
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with("ferric-browser-engine-qt-")
        {
            continue;
        }
        let path = entry.path().join("out/moc").join(file_name);
        if path.is_file() {
            let modified = fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            candidates.push((modified, path));
        }
    }
    candidates.sort_by_key(|(modified, _)| *modified);
    Ok(candidates.pop().map(|(_, path)| path))
}

fn compile_native_probe(
    name: &str,
    packages: &[&str],
    binary: &Path,
    extra_sources: &[&str],
) -> Result<(), String> {
    let source = format!("crates/ferric-browser-engine-qt/tests/native/{name}.cpp");
    let flags = Command::new("pkg-config")
        .args(["--cflags", "--libs"])
        .args(packages)
        .output()
        .map_err(|error| format!("could not query {name} Qt flags: {error}"))?;
    if !flags.status.success() {
        return Err(format!("{name} Qt pkg-config packages are unavailable"));
    }
    let flags = String::from_utf8(flags.stdout)
        .map_err(|error| format!("{name} Qt flags were not UTF-8: {error}"))?;
    let status = Command::new("c++")
        .args(["-std=c++17", "-fPIC"])
        .arg(&source)
        .args(extra_sources)
        .arg("-o")
        .arg(binary)
        .args(flags.split_ascii_whitespace())
        .status()
        .map_err(|error| format!("could not compile {name}: {error}"))?;
    if !status.success() {
        return Err(format!("{name} compile exited with {status}"));
    }
    Ok(())
}

fn check_architecture_boundaries() -> Result<(), String> {
    check_identity_and_layer_boundaries()?;
    check_qt_adapter_boundaries()?;
    check_core_and_error_boundaries()
}

fn check_identity_and_layer_boundaries() -> Result<(), String> {
    let retired_markers = [
        ["Rust", "Browser"].concat(),
        ["rust", "browser"].concat(),
        ["io.github.", "rust", "browser"].concat(),
    ];
    for root in [Path::new("crates"), Path::new("packaging")] {
        visit_files(root, &mut |path, source| {
            if let Some(marker) = retired_markers
                .iter()
                .find(|marker| source.contains(marker.as_str()))
            {
                return Err(format!(
                    "retired identity {marker:?} is not allowed in production file {}",
                    path.display()
                ));
            }
            Ok(())
        })?;
    }

    for root in [
        Path::new("crates/ferric-browser-core/src"),
        Path::new("crates/ferric-browser-runtime/src"),
        Path::new("crates/ferric-browser-application/src"),
    ] {
        visit_files(root, &mut |path, source| {
            for marker in ["cxx_qt", "QString", "QObject", "QAbstractListModel"] {
                if source.contains(marker) {
                    return Err(format!(
                        "Qt-independent crate boundary violation: {} contains {marker}",
                        path.display()
                    ));
                }
            }
            Ok(())
        })?;
    }

    let application = fs::read_to_string("crates/ferric-browser-application/src/lib.rs")
        .map_err(|error| format!("could not inspect the application boundary: {error}"))?;
    for marker in [
        "pub enum ApplicationInput",
        "pub enum ApplicationEffect",
        "pub fn handle(",
    ] {
        if application.contains(marker) {
            return Err(format!(
                "application concerns must use domain-specific methods, not a catch-all boundary: {marker}"
            ));
        }
    }

    Ok(())
}

fn check_qt_adapter_boundaries() -> Result<(), String> {
    // A mutable borrow is needed to call BrowserApplication's domain APIs;
    // prohibit writes to reducer-owned internals and catch-all dispatch instead.
    let reducer_mutation_markers = [
        ".tabs.get_mut(",
        ".windows.get_mut(",
        ".profiles.get_mut(",
        ".journey.clear_matching(",
        ".active_window =",
        ".last_focused_window =",
        ".state.handle(",
    ];
    let reducer_ownership_markers = ["ApplicationState", "RuntimeOwner", "StateView"];
    let prose_error_markers = ["error.contains(", "error.starts_with(", "error.ends_with("];
    visit_files(
        Path::new("crates/ferric-browser-engine-qt/src"),
        &mut |path, source| {
            if path.extension().is_none_or(|extension| extension != "rs") {
                return Ok(());
            }
            if path.starts_with(Path::new("crates/ferric-browser-engine-qt/src/tests"))
                || path.file_name().is_some_and(|name| name == "tests.rs")
            {
                return Ok(());
            }
            let production_source = source.split("#[cfg(test)]").next().unwrap_or(source);
            if let Some(marker) = reducer_mutation_markers
                .iter()
                .find(|marker| production_source.contains(**marker))
            {
                return Err(format!(
                    "Qt adapter writes reducer internals outside the reducer: {} contains {marker}",
                    path.display()
                ));
            }
            if let Some(marker) = reducer_ownership_markers
                .iter()
                .find(|marker| production_source.contains(**marker))
            {
                return Err(format!(
                    "Qt adapter must use BrowserApplication read projections: {} contains {marker}",
                    path.display()
                ));
            }
            if let Some(marker) = prose_error_markers
                .iter()
                .find(|marker| production_source.contains(**marker))
            {
                return Err(format!(
                    "Qt adapter must not classify public errors from prose: {} contains {marker}",
                    path.display()
                ));
            }
            Ok(())
        },
    )?;
    check_qt_adapter_root()
}

fn check_qt_adapter_root() -> Result<(), String> {
    // Root-specific checks remain below because the generated QObject bridge is
    // intentionally declared in lib.rs, while reducer-mutation checks above
    // cover every extracted adapter module.
    let adapter = fs::read_to_string("crates/ferric-browser-engine-qt/src/lib.rs")
        .map_err(|error| format!("could not inspect Qt adapter root: {error}"))?;
    if adapter.lines().count() > 3_500 {
        return Err("Qt adapter root must remain a narrow bridge below 3,500 lines".to_owned());
    }
    let production_adapter = adapter
        .split_once("\n#[cfg(test)]\nmod tests;")
        .map(|(production, _)| production)
        .ok_or_else(|| "could not locate Qt adapter test-module boundary".to_owned())?;
    for marker in [
        "dispatch_command(",
        "dispatch_command_for(",
        "dispatch_for_legacy(",
        "reduce(",
    ] {
        if production_adapter.contains(marker) {
            return Err(format!(
                "Qt adapter bypasses the runtime command boundary: {marker}"
            ));
        }
    }
    for marker in ["state: Option<ApplicationState>", ".into_state()"] {
        if production_adapter.contains(marker) {
            return Err(format!(
                "Qt adapter must retain BrowserRuntime ownership instead of core state: {marker}"
            ));
        }
    }
    for marker in [
        "storage_worker: Option<ProfileStoreWorker>",
        "ProfileStoreWorker::spawn(",
        "profile_setup_worker:",
        "ProfileSetupWorker",
        "ProfileActivation",
        "open_profile_store(",
        "load_profile_runtime_overrides(",
        "load_runtime_overrides(",
        "ProfileRegistry::open(",
        "ContextRegistry::open(",
        "StorageCompletions",
        "poll_completions(",
        "pending_history_writes",
        "pending_permission_writes",
        "pending_mark_writes",
        "pending_download_writes",
        "pending_journey_writes",
    ] {
        if production_adapter.contains(marker) {
            return Err(format!(
                "Qt adapter must not own a profile metadata worker: {marker}"
            ));
        }
    }

    let browser_ui_state = production_adapter
        .split("pub struct BrowserUiRust")
        .nth(1)
        .and_then(|source| source.split("struct ContextSnapshot").next())
        .ok_or_else(|| "could not locate Qt BrowserUi state boundary".to_owned())?;
    for marker in [
        "Option<ProfileStore>",
        "Option<ProfileLock>",
        "Option<ContextRegistry>",
        "Option<ProfileStoreWorker>",
    ] {
        if browser_ui_state.contains(marker) {
            return Err(format!(
                "Qt BrowserUi state must project application-owned profile resources, not own {marker}"
            ));
        }
    }
    if !production_adapter.contains("state: Option<BrowserApplication>") {
        return Err("Qt adapter must keep browser ownership behind BrowserApplication".to_owned());
    }
    check_shared_ui_contracts(production_adapter)
}

// This is a deliberately linear audit checklist; keeping related boundary
// assertions together makes the architectural contract reviewable.
#[allow(clippy::too_many_lines)]
fn check_shared_ui_contracts(production_adapter: &str) -> Result<(), String> {
    let config_projection =
        fs::read_to_string("crates/ferric-browser-engine-qt/src/config_projection.rs")
            .map_err(|error| format!("could not inspect Qt config projection: {error}"))?;
    for marker in [
        "config_value_with_layers",
        "apply_runtime_overrides(",
        "resolve_runtime_override_layers",
        "presentation_config_with_layers",
    ] {
        if production_adapter.contains(marker) || config_projection.contains(marker) {
            return Err(format!(
                "Qt adapter must not define configuration-layer policy: {marker}"
            ));
        }
    }
    let ipc_decoder =
        fs::read_to_string("crates/ferric-browser-engine-qt/src/ipc_command_decoder.rs")
            .map_err(|error| format!("could not inspect the Qt IPC command decoder: {error}"))?;
    if !ipc_decoder.contains("ferric_browser_ipc::decode_command_invocation") {
        return Err("Qt command decoding must use the shared typed IPC invocation".to_owned());
    }
    let cli = fs::read_to_string("crates/ferric-browser/src/main.rs")
        .map_err(|error| format!("could not inspect the CLI command encoder: {error}"))?;
    if !cli.contains("encode_command as command_params") {
        return Err("CLI command encoding must use the shared IPC command codec".to_owned());
    }
    let ipc_codec = fs::read_to_string("crates/ferric-browser-ipc/src/command_codec.rs")
        .map_err(|error| format!("could not inspect the shared IPC command codec: {error}"))?;
    if !ipc_codec.contains("crate::CommandEnvelope::new") {
        return Err("IPC command encoding must produce the shared typed envelope".to_owned());
    }
    for module in [
        "command_codec_navigation.rs",
        "command_codec_tabs_windows.rs",
        "command_codec_configuration.rs",
        "command_codec_library.rs",
        "command_codec_content.rs",
        "command_codec_contexts.rs",
        "command_codec_automation.rs",
        "command_argument_decoder.rs",
        "command_argument_validation.rs",
    ] {
        let path = Path::new("crates/ferric-browser-ipc/src").join(module);
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("could not inspect IPC family module {module}: {error}"))?;
        if source.lines().count() > 1_000 {
            return Err(format!(
                "IPC family module {module} exceeds the 1,000-line implementation budget"
            ));
        }
    }
    if production_adapter.contains("settings_row_") {
        return Err("Qt settings state must use structured rows, not parallel arrays".to_owned());
    }
    let background_workers = fs::read_to_string(
        "crates/ferric-browser-engine-qt/src/background_workers.rs",
    )
    .map_err(|error| format!("could not inspect extracted Qt background workers: {error}"))?;
    if background_workers.lines().count() > 1_000
        || !background_workers.contains("pub(super) struct PrintWorker")
        || !background_workers.contains("pub(super) struct UserscriptManagerWorker")
        || production_adapter.contains("struct PrintWorker")
        || production_adapter.contains("struct UserscriptManagerWorker")
    {
        return Err(
            "Qt background workers must remain extracted from the QObject bridge root".to_owned(),
        );
    }
    let browser_ui_state =
        fs::read_to_string("crates/ferric-browser-engine-qt/src/browser_ui_state.rs")
            .map_err(|error| format!("could not inspect extracted Qt browser state: {error}"))?;
    if browser_ui_state.lines().count() > 1_000
        || !browser_ui_state.contains("pub(super) enum ProfilePersistence")
        || !browser_ui_state.contains("pub(super) struct SwitcherQueryCache")
        || production_adapter.contains("enum ProfilePersistence")
        || production_adapter.contains("struct SwitcherQueryCache")
    {
        return Err(
            "Qt browser presentation state must remain extracted from the QObject bridge root"
                .to_owned(),
        );
    }
    for module in [
        "userscript.rs",
        "userscript_discovery.rs",
        "userscript_install.rs",
        "userscript_manifest.rs",
        "userscript_protocol.rs",
        "userscript_storage.rs",
    ] {
        let path = Path::new("crates/ferric-browser-engine-qt/src").join(module);
        let source = fs::read_to_string(&path).map_err(|error| {
            format!("could not inspect extracted userscript module {module}: {error}")
        })?;
        let implementation = source
            .split("#[cfg(test)]\nmod tests {")
            .next()
            .unwrap_or(&source);
        if implementation.lines().count() > 1_000 {
            return Err(format!(
                "userscript module {module} exceeds the 1,000-line implementation budget"
            ));
        }
    }
    for entry in fs::read_dir("crates/ferric-browser-engine-qt/src")
        .map_err(|error| format!("could not enumerate Qt implementation modules: {error}"))?
    {
        let path = entry
            .map_err(|error| format!("could not inspect Qt implementation module: {error}"))?
            .path();
        if path.file_name().is_some_and(|name| name == "lib.rs")
            || path.extension().is_none_or(|extension| extension != "rs")
        {
            continue;
        }
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
        let implementation = source
            .split_once("\n#[cfg(test)]\nmod tests")
            .map_or(source.as_str(), |(implementation, _)| implementation);
        if implementation.lines().count() > 1_000 {
            return Err(format!(
                "Qt implementation module {} exceeds the 1,000-line budget",
                path.display()
            ));
        }
    }
    if !production_adapter.contains("Architecture budget exception:")
        || !production_adapter.contains("#[cxx_qt::bridge]")
    {
        return Err(
            "the over-budget Qt root exception must remain documented and bridge-only".to_owned(),
        );
    }
    let qml_root = fs::read_to_string("crates/ferric-browser-engine-qt/qml/Main.qml")
        .map_err(|error| format!("could not inspect the QML root: {error}"))?;
    let primary_window = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricPrimaryBrowserWindow.qml",
    )
    .map_err(|error| format!("could not inspect the primary browser window: {error}"))?;
    let profile_session_surfaces = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricProfileSessionSurfaces.qml",
    )
    .map_err(|error| format!("could not inspect profile/session surfaces: {error}"))?;
    let focus_overlay_controller = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricFocusOverlayController.qml",
    )
    .map_err(|error| format!("could not inspect focus overlay controller: {error}"))?;
    let webengine_surface_recovery = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricWebEngineSurfaceRecovery.qml",
    )
    .map_err(|error| format!("could not inspect WebEngine surface recovery: {error}"))?;
    let chrome_presentation_controller = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricChromePresentationController.qml",
    )
    .map_err(|error| format!("could not inspect chrome presentation controller: {error}"))?;
    let window_registry_controller = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricWindowRegistryController.qml",
    )
    .map_err(|error| format!("could not inspect window registry controller: {error}"))?;
    let request_presentation_controller = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricRequestPresentationController.qml",
    )
    .map_err(|error| format!("could not inspect request presentation controller: {error}"))?;
    for (module, root_type) in [
        ("FerricBrowserRuntimeBase.qml", "ApplicationWindow"),
        (
            "FerricBrowserRuntimeServices.qml",
            "FerricBrowserRuntimeBase",
        ),
        (
            "FerricBrowserRuntimePresentation.qml",
            "FerricBrowserRuntimeServices",
        ),
        (
            "FerricBrowserRuntimeRequests.qml",
            "FerricBrowserRuntimePresentation",
        ),
        (
            "FerricBrowserRuntimeChrome.qml",
            "FerricBrowserRuntimeRequests",
        ),
        (
            "FerricBrowserRuntimeSurface.qml",
            "FerricBrowserRuntimeChrome",
        ),
    ] {
        let path = Path::new("crates/ferric-browser-engine-qt/qml/components").join(module);
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("could not inspect QML runtime layer {module}: {error}"))?;
        if source.lines().count() > 2_500 || !source.contains(root_type) {
            return Err(format!(
                "QML runtime layer {module} must inherit {root_type} and remain below 2,500 lines"
            ));
        }
    }
    if primary_window.lines().count() > 2_500
        || !qml_root.contains("FerricPrimaryBrowserWindow")
        || !primary_window.contains("FerricBrowserRuntimeSurface")
        || !profile_session_surfaces.contains("FerricProfileManager")
        || !profile_session_surfaces.contains("FerricSessionManager")
        || !profile_session_surfaces.contains("signal createProfileRequested")
        || !profile_session_surfaces.contains("signal saveSessionRequested")
        || !focus_overlay_controller.contains("required property var browserWindow")
        || !focus_overlay_controller.contains("function captureOverlayFocus")
        || !focus_overlay_controller.contains("function restoreOverlayFocus")
        || !webengine_surface_recovery.contains("required property var hostWindow")
        || !webengine_surface_recovery.contains("function onFrameSwapped()")
        || webengine_surface_recovery.contains("Qt.callLater")
        || webengine_surface_recovery.contains("Timer {")
        || webengine_surface_recovery.contains("runJavaScript")
        || webengine_surface_recovery.contains(".reload(")
        || !chrome_presentation_controller.contains("required property var browserWindow")
        || !chrome_presentation_controller.contains("required property var browserUi")
        || !chrome_presentation_controller.contains("function refreshChromeAppearance")
        || !window_registry_controller.contains("required property var browserWindow")
        || !window_registry_controller.contains("function retainEphemeralProfileOwner")
        || !window_registry_controller.contains("function releaseEphemeralProfileOwner")
        || !request_presentation_controller.contains("required property var browserWindow")
        || !request_presentation_controller.contains("function boundedPageDialogText")
        || !request_presentation_controller.contains("function pageDialogType")
        || request_presentation_controller.contains("resolveQtRequest")
    {
        return Err(
            "the primary QML coordinator must remain below 2,500 lines and delegate presentation intents"
                .to_owned(),
        );
    }

    for module in [
        "browser_ui_query_profiles.rs",
        "browser_ui_query_bindings.rs",
        "browser_ui_query_configuration.rs",
        "browser_ui_query_permissions.rs",
        "browser_ui_query_switcher.rs",
        "browser_ui_query_site.rs",
        "browser_ui_query_diagnostics.rs",
    ] {
        let path = Path::new("crates/ferric-browser-engine-qt/src").join(module);
        let source = fs::read_to_string(&path).map_err(|error| {
            format!("could not inspect extracted Qt query module {module}: {error}")
        })?;
        if source.lines().count() > 1_000 {
            return Err(format!(
                "Qt query module {module} exceeds the 1,000-line implementation budget"
            ));
        }
    }

    Ok(())
}

fn check_core_and_error_boundaries() -> Result<(), String> {
    let model = fs::read_to_string("crates/ferric-browser-core/src/model.rs")
        .map_err(|error| format!("could not inspect core state model: {error}"))?;
    for marker in [
        "pub(crate) profiles:",
        "pub(crate) windows:",
        "pub(crate) tabs:",
        "pub(crate) journey:",
        "pub(crate) active_window:",
        "pub(crate) revision:",
    ] {
        if model.contains(marker) {
            return Err(format!(
                "application state must remain reducer-private: {marker}"
            ));
        }
    }
    for (path, marker) in [
        ("crates/ferric-browser/src/main.rs", "fn classify_cli_error"),
        (
            "crates/ferric-browser-engine-qt/src/lib.rs",
            "fn ipc_error_code",
        ),
    ] {
        let source = fs::read_to_string(path)
            .map_err(|error| format!("could not inspect public error boundary {path}: {error}"))?;
        if source.contains(marker) {
            return Err(format!(
                "public error codes must be carried by typed errors, not inferred from prose: {path} contains {marker}"
            ));
        }
    }
    let ipc_dispatch_source =
        fs::read_to_string("crates/ferric-browser-engine-qt/src/browser_ui_ipc_dispatch.rs")
            .map_err(|error| format!("could not inspect Qt IPC dispatch: {error}"))?;
    let ipc_dispatch = ipc_dispatch_source
        .split("fn handle_ipc_request")
        .nth(1)
        .ok_or_else(|| "could not locate Qt IPC dispatch boundary".to_owned())?;
    for marker in ["error.contains(", "error.starts_with("] {
        if ipc_dispatch.contains(marker) {
            return Err(format!(
                "IPC dispatch must select public error codes from typed errors, not {marker}"
            ));
        }
    }
    Ok(())
}

fn visit_files(
    root: &Path,
    visitor: &mut dyn FnMut(&Path, &str) -> Result<(), String>,
) -> Result<(), String> {
    let mut pending = vec![root.to_owned()];
    while let Some(path) = pending.pop() {
        let metadata = fs::metadata(&path)
            .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
        if metadata.is_dir() {
            for entry in fs::read_dir(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?
            {
                pending.push(
                    entry
                        .map_err(|error| format!("could not enumerate source: {error}"))?
                        .path(),
                );
            }
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "cpp" | "h" | "qml" | "toml" | "desktop" | "sh")
        ) {
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            visitor(&path, &source)?;
        }
    }
    Ok(())
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
        Path::new("crates/ferric-browser-engine-qt/src"),
        Path::new("crates/ferric-browser-engine-qt/qml"),
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
    let root = Path::new("crates/ferric-browser-engine-qt/qml");
    let mut source = String::new();
    visit_files(root, &mut |_, contents| {
        source.push_str(contents);
        source.push('\n');
        Ok(())
    })?;
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
                root.display()
            ));
        }
    }
    if source.contains("Accessible.name: \"\"") {
        return Err(format!(
            "accessibility contract violation: an empty Accessible.name exists in {}",
            root.display()
        ));
    }
    Ok(())
}

fn check_renderer_failure_contract() -> Result<(), String> {
    let root = Path::new("crates/ferric-browser-engine-qt/qml/components");
    let mut source = String::new();
    for module in [
        "FerricBrowserRuntimeBase.qml",
        "FerricBrowserRuntimeServices.qml",
        "FerricBrowserRuntimePresentation.qml",
        "FerricBrowserRuntimeRequests.qml",
        "FerricBrowserRuntimeChrome.qml",
        "FerricBrowserRuntimeSurface.qml",
    ] {
        let path = root.join(module);
        source.push_str(&fs::read_to_string(&path).map_err(|error| {
            format!(
                "could not read renderer contract module {}: {error}",
                path.display()
            )
        })?);
        source.push('\n');
    }
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
                root.display()
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
        Path::new("crates/ferric-browser-engine-qt/src"),
        Path::new("crates/ferric-browser-engine-qt/qml"),
        Path::new("crates/ferric-browser/src"),
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
        "test requires core, engine, archive, cookie, adapter, fuzz, storage, fixtures, repeat, accessibility, compatibility, wayland, blocking, config, or mpris"
            .to_owned()
    })?;
    if arguments.len() > 1 {
        return Err(format!("test {task} does not accept additional arguments"));
    }
    match task.as_str() {
        "core" => run(
            "cargo",
            &["test", "-p", "ferric-browser-core", "--locked", "--offline"],
        ),
        "engine" => {
            run(
                "cargo",
                &[
                    "test",
                    "-p",
                    "ferric-browser-engine-qt",
                    "--locked",
                    "--offline",
                ],
            )?;
            run_qml_ui_tests()
        }
        "archive" => run_native_archive_probe(),
        "cookie" => run_native_cookie_probe(),
        "versioning-cookie-adapter" => run_native_cookie_adapter_probe(),
        "versioning-cookie-boundary" => run_native_cookie_boundary_probe(),
        "versioning-fixtures" => run_native_fixture_tls_probe(),
        "versioning-geometry" => run_geometry_sidecar_probe(),
        "versioning-profile-download" => run_native_profile_download_probe(),
        "versioning-clean-branch-profile" => run_native_clean_branch_profile_probe(),
        "versioning-viewless-download" => run_native_viewless_download_probe().map(|_| ()),
        "adapter" => run_adapter_smoke(),
        "fuzz" => run_fuzz_smoke(),
        "storage" => run(
            "cargo",
            &[
                "test",
                "-p",
                "ferric-browser-storage",
                "--locked",
                "--offline",
            ],
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
    let executable = Path::new("target/debug/ferric-browser");
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
    let basedir = DisposableDirectory::new("ferric-browser-mpris")?;
    let mut app = ChildGuard::spawn(
        Command::new(executable)
            .arg("--basedir")
            .arg(basedir.path())
            .arg("open")
            .arg(&fixture_url),
        "Ferric Browser MPRIS smoke",
    )?;
    let pid = app
        .child_mut("Ferric Browser MPRIS smoke")?
        .id()
        .to_string();
    let service = format!("org.mpris.MediaPlayer2.ferric-browser.instance{pid}");
    let introspection = wait_for_mpris_introspection(&service, &mut app)?;
    assert_mpris_introspection(&introspection)?;
    call_mpris_player_method(&service, "PlayPause")?;
    wait_for_redacted_mpris_metadata(&service)?;
    thread::sleep(Duration::from_millis(250));
    if app
        .child_mut("Ferric Browser MPRIS smoke")?
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
            .child_mut("Ferric Browser MPRIS smoke")?
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
                "ferric-browser-core",
                "-p",
                "ferric-browser-storage",
                "--locked",
            ],
        )?;
    }
    println!("repeated core/storage contract smoke passed ({RUNS} runs)");
    Ok(())
}

fn run_adapter_smoke() -> Result<(), String> {
    run("cargo", &["build", "-p", "ferric-browser", "--locked"])?;
    let executable = Path::new("target/debug/ferric-browser");
    if !executable.is_file() {
        return Err(format!(
            "adapter smoke build did not produce {}",
            executable.display()
        ));
    }

    run_adapter_smoke_variant(executable, false)?;
    run_adapter_smoke_variant(executable, true)
}

fn run_adapter_smoke_variant(executable: &Path, uncached: bool) -> Result<(), String> {
    let variant = if uncached {
        "uncached"
    } else {
        "default cache"
    };
    let mut command = Command::new(executable);
    command
        .arg("--temp-basedir")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("FERRIC_BROWSER_QML_DIAGNOSTICS", "1")
        // Diagnostic GC/JIT overrides must not mask a startup regression.
        .env_remove("QV4_GC_TIMELIMIT")
        .env_remove("QV4_MM_AGGRESSIVE_GC")
        .env_remove("QV4_MM_CROSS_VALIDATE_INCREMENTAL_GC")
        .env_remove("QV4_FORCE_INTERPRETER")
        .env_remove("QML_DISABLE_DISK_CACHE")
        .env_remove("QML_FORCE_DISK_CACHE")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if uncached {
        command.env("QML_DISABLE_DISK_CACHE", "1");
    }

    let mut application =
        ChildGuard::spawn(&mut command, "Ferric Browser offscreen adapter smoke")?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = application
            .child_mut("Ferric Browser offscreen adapter smoke")?
            .try_wait()
            .map_err(|error| format!("could not poll offscreen adapter smoke: {error}"))?
        {
            let output = application
                .take("Ferric Browser offscreen adapter smoke")?
                .wait_with_output()
                .map_err(|error| format!("could not collect offscreen adapter smoke: {error}"))?;
            return Err(format!(
                "offscreen Qt/QML adapter smoke ({variant}) exited with {status}; stdout={:?}; stderr={:?}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if Instant::now() >= deadline {
            let mut child = application.take("Ferric Browser offscreen adapter smoke")?;
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
                    "offscreen adapter smoke ({variant}) reached its bound with QML errors: {combined:?}"
                ));
            }
            println!("offscreen Qt/QML adapter smoke passed ({variant}, bounded 10s lifetime)");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn run_wayland_smoke() -> Result<(), String> {
    check_nested_wayland_requirements()?;
    run("cargo", &["build", "-p", "ferric-browser", "--locked"])?;
    let executable = Path::new("target/debug/ferric-browser");
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
    run("cargo", &["build", "-p", "ferric-browser", "--locked"])?;
    let executable = Path::new("target/debug/ferric-browser");
    if !executable.is_file() {
        return Err(format!(
            "config smoke build did not produce {}",
            executable.display()
        ));
    }

    let base = DisposableDirectory::new("ferric-browser-config")?;
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
    let runtime_dir = std::env::temp_dir().join(format!("ferric-browser-config-wayland-{run_id}"));
    fs::create_dir(&runtime_dir)
        .map_err(|error| format!("could not create config Wayland runtime directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700)).map_err(|error| {
            format!("could not protect config Wayland runtime directory: {error}")
        })?;
    }
    let display = format!("wayland-ferric-browser-config-{run_id}");
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
            "Ferric Browser configuration smoke",
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
        let app_output =
            stop_application_with_output(&mut app, "Ferric Browser configuration smoke")?;
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
    run("cargo", &["build", "-p", "ferric-browser", "--locked"])?;
    let executable = Path::new("target/debug/ferric-browser");
    if !executable.is_file() {
        return Err(format!(
            "blocking smoke build did not produce {}",
            executable.display()
        ));
    }

    let base = DisposableDirectory::new("ferric-browser-blocking")?;
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
        "! Ferric Browser blocking smoke\n||127.0.0.1^$image\n@@||127.0.0.1^$script\n",
    )
    .map_err(|error| format!("could not seed cached blocklist: {error}"))?;

    let mut page_fixture = start_blocking_fixture_server("127.0.0.1:0", Some("/blocking"))?;
    let page_url = read_fixture_url_named(
        &mut page_fixture,
        "blocking page fixture server",
        "/blocking",
    )?;
    let mut probe_fixture =
        start_blocking_fixture_server("127.0.0.1:18774", Some("/__ferric_browser_excepted__"))?;
    wait_for_fixture_listener("127.0.0.1:18774")?;

    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("could not create blocking smoke run ID: {error}"))?
        .as_nanos();
    let runtime_dir =
        std::env::temp_dir().join(format!("ferric-browser-blocking-wayland-{run_id}"));
    fs::create_dir(&runtime_dir)
        .map_err(|error| format!("could not create blocking Wayland runtime directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700)).map_err(|error| {
            format!("could not protect blocking Wayland runtime directory: {error}")
        })?;
    }
    let display = format!("wayland-ferric-browser-blocking-{run_id}");
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
            "Ferric Browser blocking smoke",
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
                .child_mut("Ferric Browser blocking smoke")?
                .try_wait()
                .map_err(|error| format!("could not poll Ferric Browser blocking smoke: {error}"))?
            {
                let output = app
                    .take("Ferric Browser blocking smoke")?
                    .wait_with_output()
                    .map_err(|error| {
                        format!("could not collect Ferric Browser blocking smoke: {error}")
                    })?;
                return Err(format!(
                    "Ferric Browser blocking smoke exited with {status}; stdout={:?}; stderr={:?}",
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
        application_output =
            stop_application_with_output(&mut app, "Ferric Browser blocking smoke")?;
        drop(compositor);
        Ok(())
    })();

    let probe_output = stop_fixture_server(&mut probe_fixture, "blocking probe fixture server")?;
    let page_output = stop_fixture_server(&mut page_fixture, "blocking page fixture server")?;
    let blocked_request =
        probe_output.contains("FERRIC_BROWSER_FIXTURE_EVENT path=/__ferric_browser_blocked__");
    let exception_request =
        probe_output.contains("FERRIC_BROWSER_FIXTURE_EVENT path=/__ferric_browser_excepted__");
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
                "GET /__ferric_browser_fixture_ready__ HTTP/1.1\r\nHost: {bind}\r\nConnection: close\r\n\r\n"
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

struct DisposableWaylandCompositor {
    child: ChildGuard,
    socket_path: PathBuf,
    lock_path: PathBuf,
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

impl Drop for DisposableWaylandCompositor {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_file(&self.socket_path);
        let _ = fs::remove_file(&self.lock_path);
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
        if let Some(status) = compositor
            .child_mut("nested Weston compositor")?
            .try_wait()
            .map_err(|error| format!("could not poll nested Weston compositor: {error}"))?
        {
            let output = compositor
                .take("nested Weston compositor")?
                .wait_with_output()
                .map_err(|error| format!("could not read nested Weston output: {error}"))?;
            return Err(format!(
                "nested Weston compositor exited with {status} before creating its socket; stdout={:?}; stderr={:?}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
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
        "Ferric Browser in nested Wayland session",
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
            .child_mut("Ferric Browser")?
            .try_wait()
            .map_err(|error| format!("could not poll Ferric Browser Wayland smoke: {error}"))?
        {
            let output = app
                .take("Ferric Browser")?
                .wait_with_output()
                .map_err(|error| {
                    format!("could not collect Ferric Browser smoke output: {error}")
                })?;
            return Err(format_smoke_failure(status, &output));
        }
        if Instant::now() >= deadline {
            let output =
                stop_application_with_output(&mut app, "Ferric Browser in nested Wayland session")?;
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
    let runtime_dir = std::env::temp_dir().join(format!("ferric-browser-wayland-{run_id}"));
    fs::create_dir(&runtime_dir)
        .map_err(|error| format!("could not create private Wayland runtime directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not protect Wayland runtime directory: {error}"))?;
    }

    let display = format!("wayland-ferric-browser-{run_id}");
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
        .strip_prefix("FERRIC_BROWSER_FIXTURE_READY http://")
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
        "packaging/io.github.ferricbrowser.FerricBrowser.desktop",
        "packaging/io.github.ferricbrowser.FerricBrowser.svg",
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
        &["io.github.ferricbrowser.FerricBrowser.desktop"],
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
        "target/release/ferric-browser",
        "/usr/bin/ferric-browser",
        "/usr/share/applications/io.github.ferricbrowser.FerricBrowser.desktop",
        "/usr/share/icons/hicolor/scalable/apps/io.github.ferricbrowser.FerricBrowser.svg",
        "/usr/share/doc/${pkgname}/README.md",
        "/usr/share/doc/${pkgname}/DEVELOPMENT_SPEC.md",
        "/usr/share/doc/${pkgname}/grid-navigation.md",
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
        "ferric-browser",
    )?;
    let installed_manifest = fs::read_to_string("packaging/installed-files.txt")
        .map_err(|error| format!("could not read packaging/installed-files.txt: {error}"))?;
    for required in [
        "/usr/share/doc/ferric-browser/grid-navigation.md",
        "/usr/share/doc/ferric-browser/LICENSES.md",
        "/usr/share/doc/ferric-browser/license-policy.toml",
        "/usr/share/doc/ferric-browser/SUPPORT.md",
        "/usr/share/doc/ferric-browser/SECURITY.md",
        "/usr/share/doc/ferric-browser/RELEASE_POLICY.md",
        "/usr/share/doc/ferric-browser/PROVENANCE.md",
        "/usr/share/doc/ferric-browser/MAINTENANCE.md",
        "/usr/share/doc/ferric-browser/CONTRIBUTING.md",
        "/usr/share/doc/ferric-browser/CODE_OF_CONDUCT.md",
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
        "ferric-browser-omarchy",
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
mod qml_lint_tests {
    use super::qml_lint_critical_diagnostics;

    #[test]
    fn advisory_qml_diagnostics_do_not_fail_older_qt_versions() {
        let report = br#"{
            "files": [{
                "filename": "Overlay.qml",
                "success": false,
                "warnings": [{
                    "line": 12,
                    "column": 8,
                    "message": "Unqualified access",
                    "type": "warning"
                }]
            }]
        }"#;

        assert!(
            qml_lint_critical_diagnostics(report)
                .expect("valid qmllint report")
                .is_empty()
        );
    }

    #[test]
    fn critical_qml_diagnostics_remain_blocking() {
        let report = br#"{
            "files": [{
                "filename": "Overlay.qml",
                "success": false,
                "warnings": [{
                    "line": 17,
                    "column": 5,
                    "message": "Expected token `}'",
                    "type": "critical"
                }]
            }]
        }"#;

        assert_eq!(
            qml_lint_critical_diagnostics(report).expect("valid qmllint report"),
            ["Overlay.qml:17:5: Expected token `}'"]
        );
    }
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
