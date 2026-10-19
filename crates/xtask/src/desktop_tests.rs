use super::{
    BufReader, Child, Command, Duration, Instant, Output, Path, Stdio, SystemTime, TcpStream,
    UNIX_EPOCH, check_accessibility_contract, check_compatibility_report, fs,
    prepare_application_tests, run, run_application_tests, run_fuzz_smoke, run_native_insert_tests,
    run_qml_ui_tests, run_wayland_application_tests, thread,
};
use std::{
    io::{BufRead as _, Read as _, Write as _},
    net::ToSocketAddrs as _,
};

pub(super) fn run_test(arguments: &[String]) -> Result<(), String> {
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
            run_qml_ui_tests()?;
            run_native_insert_tests()
        }
        "application" => run_application_tests(),
        "application-clipboard" => {
            prepare_application_tests()?;
            run(
                "ctest",
                &[
                    "--test-dir",
                    "target/application-tests",
                    "--output-on-failure",
                    "-R",
                    "^application_clipboard$",
                ],
            )
        }
        "application-wayland" => run_wayland_application_tests(),
        "clipboard-wayland" => {
            prepare_application_tests()?;
            run(
                "python3",
                &["crates/ferric-browser-engine-qt/tests/application/run_clipboard_wayland.py"],
            )
        }
        "wayland-input" => run_wayland_input_qualification(),
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

pub(super) fn run_mpris_smoke() -> Result<(), String> {
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

pub(super) fn wait_for_mpris_introspection(
    service: &str,
    app: &mut ChildGuard,
) -> Result<String, String> {
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

pub(super) fn assert_mpris_introspection(introspection: &str) -> Result<(), String> {
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

pub(super) fn call_mpris_player_method(service: &str, method: &str) -> Result<(), String> {
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

pub(super) fn read_mpris_property(service: &str, property: &str) -> Result<String, String> {
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

pub(super) fn wait_for_redacted_mpris_metadata(service: &str) -> Result<(), String> {
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

pub(super) fn run_repeated_contract_smoke() -> Result<(), String> {
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

pub(super) fn run_adapter_smoke() -> Result<(), String> {
    run("cargo", &["build", "-p", "ferric-browser", "--locked"])?;
    let executable = Path::new("target/debug/ferric-browser");
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
        "Ferric Browser offscreen adapter smoke",
    )?;
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
                "offscreen Qt/QML adapter smoke exited with {status}; stdout={:?}; stderr={:?}",
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
                    "offscreen adapter smoke reached its bound with QML errors: {combined:?}"
                ));
            }
            println!("offscreen Qt/QML adapter smoke passed (bounded 10s lifetime)");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub(super) fn run_wayland_smoke() -> Result<(), String> {
    check_nested_wayland_requirements()?;
    run("cargo", &["build", "-p", "ferric-browser", "--locked"])?;
    let executable = Path::new("target/debug/ferric-browser");
    if !executable.is_file() {
        return Err(format!(
            "Wayland smoke build did not produce {}",
            executable.display()
        ));
    }

    run_nested_wayland_smoke(executable, false)
}

pub(super) fn run_wayland_input_qualification() -> Result<(), String> {
    check_nested_wayland_requirements()?;
    run(
        "cargo",
        &[
            "build",
            "--release",
            "-p",
            "ferric-browser",
            "--locked",
            "--offline",
        ],
    )?;
    run_nested_wayland_smoke(Path::new("target/release/ferric-browser"), true)
}

#[allow(clippy::too_many_lines)]
pub(super) fn run_config_smoke() -> Result<(), String> {
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
pub(super) fn run_blocking_smoke() -> Result<(), String> {
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

pub(super) fn start_blocking_fixture_server(
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

pub(super) fn wait_for_fixture_listener(bind: &str) -> Result<(), String> {
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

pub(super) fn stop_fixture_server(server: &mut ChildGuard, label: &str) -> Result<String, String> {
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

pub(super) fn stop_application_with_output(
    child: &mut ChildGuard,
    label: &str,
) -> Result<String, String> {
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

pub(super) struct ChildGuard {
    child: Option<Child>,
}

pub(super) struct DisposableDirectory {
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
    pub(super) fn spawn(command: &mut Command, label: &str) -> Result<Self, String> {
        let child = command
            .spawn()
            .map_err(|error| format!("failed to start {label}: {error}"))?;
        Ok(Self { child: Some(child) })
    }

    pub(super) fn child_mut(&mut self, label: &str) -> Result<&mut Child, String> {
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

pub(super) fn command_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

#[cfg(unix)]
pub(super) fn is_socket(path: &Path) -> bool {
    use std::os::unix::fs::FileTypeExt;

    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_socket())
}

#[cfg(not(unix))]
pub(super) fn is_socket(_path: &Path) -> bool {
    false
}

pub(super) fn start_nested_compositor(
    runtime_dir: &Path,
    display: &str,
) -> Result<ChildGuard, String> {
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

pub(super) fn wait_for_wayland_socket(
    compositor: &mut ChildGuard,
    socket: &Path,
) -> Result<(), String> {
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

pub(super) fn run_nested_application(
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

pub(super) fn run_nested_wayland_smoke(
    executable: &Path,
    require_input: bool,
) -> Result<(), String> {
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
        (Ok(false), _) if require_input => Err(
            "native Wayland input qualification unavailable: compositor lacks virtual-keyboard protocol; startup success is not input evidence".into()
        ),
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

pub(super) fn start_nested_fixture_server() -> Result<ChildGuard, String> {
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

pub(super) fn read_fixture_url(fixture: &mut ChildGuard) -> Result<String, String> {
    read_fixture_url_named(fixture, "nested loopback fixture server", "/editable")
}

pub(super) fn read_fixture_url_named(
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

pub(super) fn cleanup_nested_runtime_dir(runtime_dir: &Path) -> std::io::Result<()> {
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

pub(super) fn remove_nested_runtime_entries(runtime_dir: &Path) -> std::io::Result<()> {
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

pub(super) fn check_nested_wayland_requirements() -> Result<(), String> {
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

pub(super) fn format_smoke_failure(status: std::process::ExitStatus, output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "nested native Wayland startup smoke exited with {status}; stdout={stdout:?}; stderr={stderr:?}"
    )
}
