use std::{
    collections::BTreeSet,
    fs,
    io::BufReader,
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ferric_browser_config::{Config, SettingRegistry};
use ferric_browser_core::{ActionRegistry, BindingRegistry, CommandRegistry, Mode};

mod documentation;
use documentation::{check_generated_docs, generate_user_docs};
mod native_tests;
use native_tests::{
    prepare_application_tests, run_application_tests, run_download_source_tests,
    run_native_insert_tests, run_profile_lifetime_tests, run_qml_lint, run_qml_ui_tests,
    run_wayland_application_tests,
};
mod architecture_checks;
use architecture_checks::{
    check_accessibility_contract, check_architecture_boundaries, check_capability_matrix,
    check_compatibility_diagnosis, check_compatibility_report, check_release_freshness_policy,
    check_renderer_failure_contract, check_ui_automation_boundary, check_ui_thread_contract,
    visit_files,
};
mod desktop_tests;
use desktop_tests::{
    check_nested_wayland_requirements, cleanup_nested_runtime_dir, command_available, run_test,
    start_nested_compositor, wait_for_wayland_socket,
};
mod packaging;
use packaging::{run_package, validate_license_policy, validate_release_artifacts};

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
            println!("cargo xtask test application");
            println!("  Run production BrowserUi/QML/WebEngine integration tests offscreen.");
            println!("cargo xtask test clipboard-wayland");
            println!(
                "  Qualify isolated Wayland clipboard exchange with verified compositor input."
            );
            println!("cargo xtask test application-clipboard");
            println!("  Run the focused production clipboard regression suite offscreen.");
            println!("cargo xtask test application-wayland");
            println!(
                "  Run production integration tests with Qt-generated events on nested Wayland (not physical-input qualification)."
            );
            println!("cargo xtask test wayland-input");
            println!(
                "  Require compositor-delivered input; unavailable input fails qualification."
            );
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
    run_qml_lint()?;
    run_qml_ui_tests()?;
    run_profile_lifetime_tests()?;
    run_download_source_tests()?;
    check_generated_docs()?;
    check_architecture_boundaries()?;
    validate_release_artifacts()?;
    validate_license_policy()?;
    run("cargo", &["fmt", "--all", "--", "--check"])?;
    run("cargo", &["test", "--workspace", "--locked"])
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
    use super::native_tests::qml_lint_critical_diagnostics;

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

        assert_eq!(
            qml_lint_critical_diagnostics(report).expect("valid qmllint report"),
            Vec::<String>::new()
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

#[cfg(test)]
mod adapter_boundary_tests {
    use super::architecture_checks::qt_adapter_core_mutation_marker;

    #[test]
    fn opaque_application_metadata_services_are_allowed() {
        let source = "if let Some(application) = this.state.as_mut() { application.retry_download_history(); }";
        assert_eq!(qt_adapter_core_mutation_marker(source), None);
    }

    #[test]
    fn direct_model_writes_and_reducer_bypasses_remain_rejected() {
        for source in [
            "model.tabs.get_mut(&tab)",
            "model.windows.get_mut(&window)",
            "model.profiles.get_mut(&profile)",
            "model.journey.clear_matching(filter)",
            "model.active_window = selected",
            "model.last_focused_window = selected",
            "this.state.handle(event)",
        ] {
            assert!(
                qt_adapter_core_mutation_marker(source).is_some(),
                "{source}"
            );
        }
    }
}

#[cfg(test)]
mod refactoring_contracts {
    #[test]
    fn application_boundaries_are_enforced() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::env::set_current_dir(root).unwrap();
        super::check_architecture_boundaries().unwrap();
    }
}
