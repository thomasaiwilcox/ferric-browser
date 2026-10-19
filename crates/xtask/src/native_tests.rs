use super::{
    Command, Duration, Instant, Path, PathBuf, SystemTime, UNIX_EPOCH,
    check_nested_wayland_requirements, cleanup_nested_runtime_dir, command_available, fs, run,
    start_nested_compositor, thread, visit_files, wait_for_wayland_socket,
};
use crate::desktop_tests::ChildGuard;

pub(super) fn run_qml_lint() -> Result<(), String> {
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
    let generated_module_path = std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| PathBuf::from("target"), PathBuf::from)
        .join("cxxqt/qml_modules");
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

pub(super) fn qml_lint_critical_diagnostics(report: &[u8]) -> Result<Vec<String>, String> {
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

pub(super) fn qt_qml_import_path() -> Result<PathBuf, String> {
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

pub(super) fn run_qml_ui_tests() -> Result<(), String> {
    run(
        "cmake",
        &[
            "-S",
            "crates/ferric-browser-engine-qt/tests/qml-runner",
            "-B",
            "target/qml-tests",
        ],
    )?;
    run("cmake", &["--build", "target/qml-tests", "--parallel", "2"])?;
    let executable = "target/qml-tests/qml_tests";
    let status = Command::new(executable)
        .args(["-input", "crates/ferric-browser-engine-qt/tests/qml"])
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QPA_PLATFORMTHEME", "")
        .env("QT_QUICK_BACKEND", "software")
        .env(
            "QTWEBENGINE_CHROMIUM_FLAGS",
            "--disable-gpu --js-flags=--expose-gc",
        )
        .status()
        .map_err(|error| format!("failed to run {executable}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{executable} QML interaction tests exited with {status}"
        ))
    }
}

pub(super) fn run_profile_lifetime_tests() -> Result<(), String> {
    run(
        "cmake",
        &[
            "-S",
            "crates/ferric-browser-engine-qt/tests/profile-lifetime",
            "-B",
            "target/profile-lifetime-tests",
        ],
    )?;
    run(
        "cmake",
        &[
            "--build",
            "target/profile-lifetime-tests",
            "--parallel",
            "2",
        ],
    )?;
    run(
        "ctest",
        &[
            "--test-dir",
            "target/profile-lifetime-tests",
            "--output-on-failure",
        ],
    )
}

pub(super) fn run_download_source_tests() -> Result<(), String> {
    run(
        "cmake",
        &[
            "-S",
            "crates/ferric-browser-engine-qt/tests/download-source",
            "-B",
            "target/download-source-tests",
        ],
    )?;
    run(
        "cmake",
        &["--build", "target/download-source-tests", "--parallel", "2"],
    )?;
    run(
        "ctest",
        &[
            "--test-dir",
            "target/download-source-tests",
            "--output-on-failure",
        ],
    )
}

pub(super) fn run_native_insert_tests() -> Result<(), String> {
    run(
        "cmake",
        &[
            "-S",
            "crates/ferric-browser-engine-qt/tests/native-insert",
            "-B",
            "target/native-insert",
        ],
    )?;
    run(
        "cmake",
        &["--build", "target/native-insert", "--parallel", "2"],
    )?;
    run(
        "ctest",
        &["--test-dir", "target/native-insert", "--output-on-failure"],
    )
}

pub(super) fn prepare_application_tests() -> Result<(), String> {
    run(
        "cargo",
        &[
            "build",
            "--release",
            "-p",
            "ferric-browser-engine-qt",
            "-p",
            "ferric-browser",
            "--locked",
            "--offline",
        ],
    )?;
    run(
        "cmake",
        &[
            "-S",
            "crates/ferric-browser-engine-qt/tests/application",
            "-B",
            "target/application-tests",
        ],
    )?;
    run(
        "cmake",
        &["--build", "target/application-tests", "--parallel", "2"],
    )?;
    Ok(())
}

pub(super) fn run_application_tests() -> Result<(), String> {
    prepare_application_tests()?;
    run(
        "ctest",
        &[
            "--test-dir",
            "target/application-tests",
            "--output-on-failure",
        ],
    )?;
    run(
        "python3",
        &["crates/ferric-browser-engine-qt/tests/application/run_ipc_probe.py"],
    )?;
    run(
        "python3",
        &["crates/ferric-browser-engine-qt/tests/application/run_window_ipc_probe.py"],
    )
}

pub(super) fn run_wayland_application_tests() -> Result<(), String> {
    check_nested_wayland_requirements()?;
    prepare_application_tests()?;
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let runtime_dir = std::env::temp_dir().join(format!("ferric-application-wayland-{run_id}"));
    fs::create_dir(&runtime_dir).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    let display = format!("wayland-application-{run_id}");
    let result = (|| {
        let mut compositor = start_nested_compositor(&runtime_dir, &display)?;
        wait_for_wayland_socket(&mut compositor, &runtime_dir.join(&display))?;
        let mut application = ChildGuard::spawn(
            Command::new("target/application-tests/application_tests")
                .env("QT_QPA_PLATFORM", "wayland")
                .env("QT_QPA_PLATFORMTHEME", "")
                .env("QT_QUICK_BACKEND", "software")
                .env("QTWEBENGINE_CHROMIUM_FLAGS", "--disable-gpu")
                .env("WAYLAND_DISPLAY", &display)
                .env("XDG_RUNTIME_DIR", &runtime_dir),
            "production Wayland integration tests",
        )?;
        let deadline = Instant::now() + Duration::from_secs(240);
        loop {
            if let Some(status) = application
                .child_mut("production Wayland integration tests")?
                .try_wait()
                .map_err(|error| error.to_string())?
            {
                if status.success() {
                    println!(
                        "production Wayland Qt-generated event tests passed; physical/compositor-delivered input remains a separate gate"
                    );
                    return Ok(());
                }
                return Err(format!(
                    "production Wayland integration tests failed: {status}"
                ));
            }
            if Instant::now() >= deadline {
                return Err("production Wayland integration tests exceeded 240 seconds".into());
            }
            thread::sleep(Duration::from_millis(50));
        }
    })();
    let cleanup = cleanup_nested_runtime_dir(&runtime_dir).map_err(|error| error.to_string());
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(format!(
            "{error}; private runtime cleanup failed: {cleanup}"
        )),
        (Err(error), _) | (_, Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}
