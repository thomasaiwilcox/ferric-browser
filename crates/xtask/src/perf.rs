use ferric_browser_core::{
    CommandRegistry, CompletionCandidate, CompletionCategory, ParseInput, complete, parse_chain,
    switcher_rank, tokenize_switcher_query,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    hint::black_box,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const DEFAULT_SAMPLES: usize = 30;
const MAX_SAMPLES: usize = 1_000;
const HISTORY_CANDIDATES: usize = 100_000;
const UI_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const UI_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PerfSuite {
    Core,
    Hints,
    Ui,
}

#[derive(Debug, Eq, PartialEq)]
struct PerfOptions {
    suite: PerfSuite,
    samples: usize,
    repetitions: usize,
    idle_seconds: u64,
    pages: usize,
    presentation: String,
    page_url: String,
    quit_plugin_dir: Option<PathBuf>,
    output: Option<PathBuf>,
}

pub fn run(arguments: &[String]) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err(
            "performance measurements require an optimized xtask; run `cargo run --release -p xtask -- perf ...` or `target/release/xtask perf ...`"
                .into(),
        );
    }
    let options = parse_arguments(arguments)?;
    let report = match options.suite {
        PerfSuite::Core => collect_core_report(options.samples),
        PerfSuite::Hints => collect_hint_report(options.samples),
        PerfSuite::Ui => collect_ui_report(&options)?,
    };
    let serialized = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("could not serialize performance report: {error}"))?;
    if let Some(path) = options.output {
        if fs::symlink_metadata(&path).is_ok() {
            return Err(format!(
                "performance report refuses to overwrite {}",
                path.display()
            ));
        }
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;

            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .map_err(|error| format!("could not create {}: {error}", path.display()))?;
        file.write_all(format!("{serialized}\n").as_bytes())
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        println!("performance report written to {}", path.display());
    } else {
        println!("{serialized}");
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn parse_arguments(arguments: &[String]) -> Result<PerfOptions, String> {
    let mut suite = PerfSuite::Core;
    let mut suite_seen = false;
    let mut samples = DEFAULT_SAMPLES;
    let mut samples_seen = false;
    let mut repetitions = None;
    let mut idle_seconds = None;
    let mut pages = None;
    let mut presentation = None;
    let mut page_url = None;
    let mut quit_plugin_dir = None;
    let mut output = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--suite" => {
                if suite_seen || index + 1 >= arguments.len() {
                    return Err("perf accepts --suite core|hints|ui at most once".into());
                }
                suite_seen = true;
                suite = match arguments[index + 1].as_str() {
                    "core" => PerfSuite::Core,
                    "hints" => PerfSuite::Hints,
                    "ui" => PerfSuite::Ui,
                    _ => return Err("perf --suite must be core, hints or ui".into()),
                };
                index += 2;
            }
            "--samples" => {
                if samples_seen || index + 1 >= arguments.len() {
                    return Err("perf accepts --samples N at most once".into());
                }
                samples_seen = true;
                samples = arguments[index + 1]
                    .parse::<usize>()
                    .map_err(|_| "perf --samples must be a positive integer".to_owned())?;
                if !(DEFAULT_SAMPLES..=MAX_SAMPLES).contains(&samples) {
                    return Err(format!(
                        "perf --samples must be between {DEFAULT_SAMPLES} and {MAX_SAMPLES}"
                    ));
                }
                index += 2;
            }
            "--repetitions" | "--idle-seconds" => {
                let flag = arguments[index].as_str();
                let slot = if flag == "--repetitions" {
                    &mut repetitions
                } else {
                    &mut idle_seconds
                };
                if slot.is_some() || index + 1 >= arguments.len() {
                    return Err(format!("perf accepts {flag} N at most once"));
                }
                let value = arguments[index + 1]
                    .parse::<u64>()
                    .map_err(|_| format!("{flag} requires a positive integer"))?;
                let maximum = if flag == "--repetitions" { 100 } else { 3600 };
                if !(1..=maximum).contains(&value) {
                    return Err(format!("{flag} must be between 1 and {maximum}"));
                }
                *slot = Some(value);
                index += 2;
            }
            "--pages" => {
                if pages.is_some() || index + 1 >= arguments.len() {
                    return Err("perf accepts --pages N at most once".into());
                }
                let value = arguments[index + 1]
                    .parse::<usize>()
                    .map_err(|_| "--pages requires an integer")?;
                if !(1..=20).contains(&value) {
                    return Err("--pages must be between 1 and 20".into());
                }
                pages = Some(value);
                index += 2;
            }
            "--presentation" | "--page-url" => {
                let flag = arguments[index].as_str();
                let slot = if flag == "--presentation" {
                    &mut presentation
                } else {
                    &mut page_url
                };
                if slot.is_some() || index + 1 >= arguments.len() {
                    return Err(format!("perf accepts {flag} VALUE at most once"));
                }
                let value = &arguments[index + 1];
                if value.is_empty()
                    || value.len() > 4096
                    || value.chars().any(char::is_control)
                    || flag == "--presentation"
                        && !["internal", "windows"].contains(&value.as_str())
                {
                    return Err(format!("invalid {flag} value"));
                }
                *slot = Some(value.clone());
                index += 2;
            }
            "--quit-plugin-dir" => {
                if quit_plugin_dir.is_some() || index + 1 >= arguments.len() {
                    return Err("perf accepts --quit-plugin-dir PATH at most once".into());
                }
                let value = &arguments[index + 1];
                if value.is_empty() || value.chars().any(char::is_control) {
                    return Err("invalid quit plugin directory".into());
                }
                quit_plugin_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--output" => {
                if output.is_some() || index + 1 >= arguments.len() {
                    return Err("perf accepts --output PATH at most once".into());
                }
                let path = arguments[index + 1].as_str();
                if path.is_empty() || path.as_bytes().contains(&0) {
                    return Err("perf --output requires a valid path".into());
                }
                output = Some(PathBuf::from(path));
                index += 2;
            }
            other => return Err(format!("unknown perf option {other}")),
        }
    }
    if suite != PerfSuite::Ui
        && (repetitions.is_some()
            || idle_seconds.is_some()
            || pages.is_some()
            || presentation.is_some()
            || page_url.is_some()
            || quit_plugin_dir.is_some())
    {
        return Err("UI workload options require --suite ui".into());
    }
    Ok(PerfOptions {
        repetitions: usize::try_from(repetitions.unwrap_or(1)).expect("bounded repetitions"),
        idle_seconds: idle_seconds.unwrap_or(5),
        pages: pages.unwrap_or(1),
        presentation: presentation.unwrap_or_else(|| "internal".into()),
        page_url: page_url.unwrap_or_else(|| "about:blank".into()),
        quit_plugin_dir,
        suite,
        samples,
        output,
    })
}

fn collect_core_report(samples: usize) -> Value {
    let registry = CommandRegistry::default_v1();
    let history_catalog = synthetic_history_catalog();
    let query = tokenize_switcher_query("example document 99999");
    let fields = vec!["https://example.test/document/99999".to_owned()];
    let measurements = vec![
        measure("command.parse", samples, Some(2_000.0), || {
            let _ = black_box(parse_chain(
                ":open 'https://example.test/a;;b' ;; zoom 1.25 ;; tab-next --count 3",
                ParseInput::Cli,
            ));
        }),
        measure("completion.100k", samples, Some(50_000.0), || {
            let _ = black_box(complete(
                "open example",
                12,
                &registry,
                &history_catalog,
                100,
            ));
        }),
        measure("switcher.rank", samples, Some(50_000.0), || {
            let _ = black_box(switcher_rank(
                &query,
                "history-99999",
                "Example document 99999",
                &fields,
            ));
        }),
    ];
    json!({
        "schema_version": 2,
        "suite": "core",
        "application": env!("CARGO_PKG_VERSION"),
        "timestamp_unix_seconds": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs()),
        "sample_count": samples,
        "build": {
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "target": std::env::consts::ARCH,
            "os": std::env::consts::OS,
            "parallelism": thread::available_parallelism()
                .map_or(1, std::num::NonZeroUsize::get),
        },
        "provenance": provenance(),
        "reference_environment": reference_environment(),
        "targets": {
            "command.parse_p95_us": 2_000.0,
            "completion.100k_p95_us": 50_000.0,
            "switcher.rank_p95_us": 50_000.0,
            "scope": "pure-core only; Qt, engine, network, storage, and process-tree budgets are separate"
        },
        "method": {
            "clock": "std::time::Instant",
            "statistics": ["median", "p95", "p99"],
            "raw_samples_unit": "microseconds",
            "completion_catalog_entries": HISTORY_CANDIDATES,
            "notes": "Pure Rust timings exclude Qt, engine, network, and storage I/O."
        },
        "measurements": measurements,
    })
}

fn collect_hint_report(samples: usize) -> Value {
    use ferric_browser_core::{
        HintAutoFollow, HintCandidate, HintGeometry, HintInteraction, HintInteractionInput,
        HintKind, assign_labels,
    };
    let mut measurements = Vec::new();
    for count in [100_u32, 500, 1000, 5000] {
        let candidates = (0..count)
            .map(|index| HintCandidate {
                element_id: index + 1,
                related_element_id: None,
                kind: HintKind::Button,
                frame_path: "0".into(),
                frame_owner: String::new(),
                text: format!("Synthetic target {index} CAFÉ 東京"),
                href: None,
                geometry: HintGeometry {
                    x: f64::from(index % 25) * 20.0,
                    y: f64::from(index / 25) * 20.0,
                    width: 16.0,
                    height: 16.0,
                },
                presentation: None,
            })
            .collect();
        let hints = assign_labels(candidates).expect("bounded synthetic hints");
        let mut interaction =
            HintInteraction::new(hints, "asdfghjkl".into(), HintAutoFollow::Never);
        for query in ["synthetic target 42", "st42", "CAFÉ 東京", "no-match-xyz"] {
            for _ in 0..10 {
                black_box(
                    black_box(&mut interaction).handle(HintInteractionInput::SetText(query.into())),
                );
            }
            measurements.push(measure(
                &format!("hint.rank.{count}.{query}"),
                samples,
                None,
                || {
                    black_box(
                        black_box(&mut interaction)
                            .handle(HintInteractionInput::SetText(query.into())),
                    );
                },
            ));
        }
    }
    json!({ "schema_version": 2, "suite": "hints", "sample_count": samples,
        "provenance": provenance(), "reference_environment": reference_environment(),
        "method": {"warmups": 10, "boundary": "HintInteraction SetText; excludes setup, decoding, Qt and DOM"},
        "measurements": measurements })
}

struct UiRun {
    child: Child,
    basedir: PathBuf,
}

impl Drop for UiRun {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.basedir);
    }
}

fn collect_ui_report(options: &PerfOptions) -> Result<Value, String> {
    let plugin_dir = options.quit_plugin_dir.as_ref().ok_or("UI performance requires --quit-plugin-dir PATH; build tests/performance-helpers with the same Qt SDK first")?;
    if !plugin_dir
        .join("generic/libferric_perf_quit_plugin.so")
        .is_file()
    {
        return Err("quit plugin directory does not contain the owned performance helper".into());
    }
    let mut runs = Vec::with_capacity(options.repetitions);
    for _ in 0..options.repetitions {
        runs.push(collect_ui_run(options, plugin_dir)?);
    }
    let mut report = runs[0].clone();
    report["aggregation"] = json!(
        "top-level measurements describe the first repetition; runs contains every independent repetition"
    );
    report["repetition_count"] = json!(options.repetitions);
    report["runs"] = json!(runs);
    report["provenance"] = provenance();
    report["workload"] = json!({"pages": options.pages, "presentation": options.presentation, "url": options.page_url});
    Ok(report)
}

fn collect_ui_run(options: &PerfOptions, plugin_dir: &Path) -> Result<Value, String> {
    let samples = options.samples;
    let idle_window = Duration::from_secs(options.idle_seconds);
    if !cfg!(target_os = "linux") {
        return Err("perf --suite ui currently requires Linux /proc metrics".into());
    }
    let harness = std::env::current_exe()
        .map_err(|error| format!("could not resolve the performance harness: {error}"))?;
    let browser = harness
        .parent()
        .ok_or_else(|| "performance harness has no executable directory".to_owned())?
        .join("ferric-browser");
    if !browser.is_file() {
        return Err(format!(
            "UI performance suite requires {}; build it with `cargo build --release -p ferric-browser -p xtask --locked`",
            browser.display()
        ));
    }

    let (mut run, startup_ms) = start_ui_run(&browser, options, plugin_dir)?;
    let ready_started = Instant::now();
    loop {
        let tabs = browser_json_response(
            &browser,
            &run.basedir,
            &["query", "tabs", "--format", "json"],
            true,
        )?;
        if tabs
            .as_ref()
            .map(|tabs| page_workload_ready(tabs, options.pages))
            .transpose()?
            .unwrap_or(false)
        {
            break;
        }
        if ready_started.elapsed() > Duration::from_secs(60) {
            return Err(format!(
                "workload did not settle: requested {}, observed {}; stderr: {}",
                options.pages,
                tabs.map_or_else(|| "E_BUSY".into(), |value| value.to_string()),
                fixture_stderr(&run.basedir)
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
    let workload_ready_ms = startup_ms + ready_started.elapsed().as_secs_f64() * 1000.0;
    let browser_pid = run.child.id();
    let ipc_samples_us = collect_ipc_samples(&browser, &run.basedir, samples)?;
    let idle = measure_idle_cpu(browser_pid, idle_window)?;
    let process_metrics = process_tree_metrics(browser_pid)?;
    let diagnostics = browser_json(&browser, &run.basedir, &["diagnostics", "--format", "json"])?;
    let runtime = diagnostics.get("runtime").cloned().unwrap_or(Value::Null);
    let shutdown = shutdown_ui_run(&browser, &mut run);
    let shutdown_ms = shutdown.as_ref().ok().copied();
    let shutdown_error = shutdown.err();
    let measurements = UiMeasurements {
        startup_ms,
        workload_ready_ms,
        quit_confirmed: run.basedir.join("quit-confirmed").is_file(),
        ipc_samples_us,
        idle,
        shutdown_ms,
        shutdown_error,
        process_metrics,
        runtime,
    };
    Ok(ui_report(&browser, samples, &measurements))
}

fn page_workload_ready(response: &Value, expected: usize) -> Result<bool, String> {
    let rows = response
        .get("tabs")
        .and_then(Value::as_array)
        .ok_or("workload readiness response omitted tabs")?;
    if rows.len() > expected {
        return Err("workload opened more pages than requested".into());
    }
    let mut settled = rows.len() == expected;
    for row in rows {
        match row.get("loading").and_then(Value::as_str) {
            Some("idle" | "complete") => {}
            Some("provisional" | "committed") => settled = false,
            Some("failed" | "cancelled") => {
                return Err("workload page failed or was cancelled".into());
            }
            _ => return Err("workload returned an unknown loading state".into()),
        }
    }
    Ok(settled)
}

fn start_ui_run(
    browser: &Path,
    options: &PerfOptions,
    plugin_dir: &Path,
) -> Result<(UiRun, f64), String> {
    let basedir = create_ui_basedir()?;
    fs::write(
        basedir.join(".ferric-performance-fixture"),
        "Ferric performance fixture only.\n",
    )
    .map_err(|error| format!("could not mark the owned performance profile: {error}"))?;
    let mut plugin_paths = vec![plugin_dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("QT_PLUGIN_PATH") {
        plugin_paths.extend(std::env::split_paths(&existing));
    }
    let plugins = std::env::join_paths(plugin_paths)
        .map_err(|error| format!("invalid plugin paths: {error}"))?;
    let stderr = fs::File::create(basedir.join("browser-stderr.log"))
        .map_err(|error| format!("could not capture fixture stderr: {error}"))?;
    let child = Command::new(browser)
        .arg("--basedir")
        .arg(&basedir)
        .arg("--set")
        .arg(format!("tabs.presentation={:?}", options.presentation))
        .env("QT_PLUGIN_PATH", plugins)
        .env("QT_QPA_GENERIC_PLUGINS", "ferricperfquitfixture")
        .env("FERRIC_PERF_QUIT_ROOT", &basedir)
        .arg("open")
        .args(std::iter::repeat_n(&options.page_url, options.pages))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| format!("could not start {}: {error}", browser.display()))?;
    let mut run = UiRun { child, basedir };
    let started = Instant::now();
    loop {
        if let Some(status) = run
            .child
            .try_wait()
            .map_err(|error| format!("could not inspect Ferric Browser startup: {error}"))?
        {
            return Err(format!(
                "Ferric Browser exited before UI measurement with {status}: {}",
                fixture_stderr(&run.basedir)
            ));
        }
        let error = match browser_json(
            browser,
            &run.basedir,
            &["query", "operations", "--format", "json"],
        ) {
            Ok(response) if response.get("operations").is_some_and(Value::is_array) => {
                return Ok((run, started.elapsed().as_secs_f64() * 1_000.0));
            }
            Ok(_) => "owner IPC returned an unexpected response".to_owned(),
            Err(error) => error,
        };
        if started.elapsed() >= UI_STARTUP_TIMEOUT {
            return Err(format!(
                "Ferric Browser UI startup exceeded {} seconds: {error}",
                UI_STARTUP_TIMEOUT.as_secs(),
            ));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn collect_ipc_samples(browser: &Path, basedir: &Path, samples: usize) -> Result<Vec<f64>, String> {
    let mut measurements = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        let response = browser_json(
            browser,
            basedir,
            &["query", "operations", "--format", "json"],
        )?;
        if !response.get("operations").is_some_and(Value::is_array) {
            return Err("owner IPC returned an unexpected operations response".into());
        }
        measurements.push(started.elapsed().as_secs_f64() * 1_000_000.0);
    }
    Ok(measurements)
}

struct IdleMeasurement {
    elapsed: Duration,
    main_percent: f64,
    tree_percent: Option<f64>,
    appeared: usize,
    disappeared: usize,
}

fn measure_idle_cpu(pid: u32, window: Duration) -> Result<IdleMeasurement, String> {
    thread::sleep(Duration::from_millis(250));
    let before = cpu_snapshot(pid)?;
    let root_before = process_cpu_ticks(pid)?;
    let started = Instant::now();
    thread::sleep(window);
    let after = cpu_snapshot(pid)?;
    let root_after = process_cpu_ticks(pid)?;
    let elapsed = started.elapsed();
    let tick_rate = u32::try_from(clock_ticks_per_second()?)
        .map_err(|_| "CLK_TCK exceeded the measurement range".to_owned())?;
    let percent = |ticks: u64| -> Result<f64, String> {
        let ticks =
            u32::try_from(ticks).map_err(|_| "CPU tick delta exceeded measurement range")?;
        Ok(100.0 * f64::from(ticks) / f64::from(tick_rate) / elapsed.as_secs_f64())
    };
    let (ticks, appeared, disappeared) = cpu_delta(&before, &after);
    Ok(IdleMeasurement {
        elapsed,
        main_percent: percent(root_after.saturating_sub(root_before))?,
        // Endpoint snapshots cannot account for CPU used by exited children.
        // Never report a partial sum as a complete tree measurement.
        tree_percent: if disappeared == 0 {
            Some(percent(ticks)?)
        } else {
            None
        },
        appeared,
        disappeared,
    })
}

fn cpu_delta(
    before: &BTreeMap<(u32, u64), u64>,
    after: &BTreeMap<(u32, u64), u64>,
) -> (u64, usize, usize) {
    let ticks = after
        .iter()
        .map(|(identity, ticks)| ticks.saturating_sub(*before.get(identity).unwrap_or(&0)))
        .sum();
    let appeared = after.keys().filter(|key| !before.contains_key(key)).count();
    let disappeared = before.keys().filter(|key| !after.contains_key(key)).count();
    (ticks, appeared, disappeared)
}

fn cpu_snapshot(root: u32) -> Result<BTreeMap<(u32, u64), u64>, String> {
    let mut snapshot = BTreeMap::new();
    for pid in process_tree_pids(root)? {
        let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return Err(format!(
                "process {pid} disappeared during CPU snapshot; repeat the run"
            ));
        };
        let (birth_ticks, ticks) = parse_cpu_stat(&stat)?;
        snapshot.insert((pid, birth_ticks), ticks);
    }
    Ok(snapshot)
}

fn parse_cpu_stat(stat: &str) -> Result<(u64, u64), String> {
    let fields = stat
        .rsplit_once(')')
        .ok_or("invalid /proc stat shape")?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    let field = |index: usize| -> Result<u64, String> {
        fields
            .get(index)
            .ok_or_else(|| "missing /proc stat field".to_owned())?
            .parse()
            .map_err(|_| "invalid /proc stat counter".into())
    };
    Ok((field(19)?, field(11)?.saturating_add(field(12)?)))
}

fn shutdown_ui_run(browser: &Path, run: &mut UiRun) -> Result<f64, String> {
    let shutdown_started = Instant::now();
    let authorization = run.basedir.join("confirm-quit.tmp");
    fs::write(&authorization, "Confirm this performance fixture's quit.\n")
        .and_then(|()| fs::rename(&authorization, run.basedir.join("confirm-quit")))
        .map_err(|error| format!("could not authorize fixture shutdown: {error}"))?;
    let quit = Command::new(browser)
        .arg("--basedir")
        .arg(&run.basedir)
        .args(["command", "--", "quit"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("could not request Ferric Browser shutdown: {error}"))?;
    if !quit.success() {
        return Err(format!(
            "Ferric Browser rejected the UI-suite shutdown request with {quit}"
        ));
    }
    let started = Instant::now();
    loop {
        if run
            .child
            .try_wait()
            .map_err(|error| format!("could not inspect Ferric Browser shutdown: {error}"))?
            .is_some()
        {
            return Ok(shutdown_started.elapsed().as_secs_f64() * 1000.0);
        }
        if started.elapsed() >= UI_SHUTDOWN_TIMEOUT {
            return Err(format!(
                "Ferric Browser did not exit within {} seconds: {}",
                UI_SHUTDOWN_TIMEOUT.as_secs(),
                fixture_stderr(&run.basedir)
            ));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

struct UiMeasurements {
    startup_ms: f64,
    workload_ready_ms: f64,
    quit_confirmed: bool,
    ipc_samples_us: Vec<f64>,
    idle: IdleMeasurement,
    shutdown_ms: Option<f64>,
    shutdown_error: Option<String>,
    process_metrics: ProcessTreeMetrics,
    runtime: Value,
}

fn ui_report(browser: &Path, samples: usize, measurements: &UiMeasurements) -> Value {
    json!({
        "schema_version": 2,
        "suite": "ui",
        "application": env!("CARGO_PKG_VERSION"),
        "timestamp_unix_seconds": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs()),
        "sample_count": samples,
        "build": {
            "profile": "release",
            "target": std::env::consts::ARCH,
            "os": std::env::consts::OS,
            "parallelism": thread::available_parallelism()
                .map_or(1, std::num::NonZeroUsize::get),
            "browser": browser.file_name().and_then(|name| name.to_str()).unwrap_or("ferric-browser"),
        },
        "provenance": provenance(),
        "reference_environment": reference_environment(),
        "method": {
            "startup_ready_boundary": "successful owner-IPC operations query",
            "idle_window_seconds": measurements.idle.elapsed.as_secs_f64(),
            "ipc_sample_boundary": "fresh forwarding CLI process through decoded owner response",
            "memory": "sum of Linux smaps_rollup values across the browser process tree",
            "notes": "Disposable basedir, automatic Qt graphics selection, closed DevTools; see workload for page configuration."
        },
        "measurements": [
            {
                "name": "startup.ipc_ready",
                "value_ms": measurements.startup_ms,
            },
            { "name": "workload.ready", "value_ms": measurements.workload_ready_ms },
            { "name": "shutdown", "value_ms": measurements.shutdown_ms,
              "owned_quit_confirmed": measurements.quit_confirmed,
              "error": measurements.shutdown_error,
              "cleanup": "owned fixture is terminated after a failed shutdown; failure is not a successful latency sample",
              "confirmation": "test-only Qt button click after measurement; helper dormant until authorization" },
            { "name": "idle.process_tree_cpu",
              "value_percent": measurements.idle.tree_percent,
              "appeared": measurements.idle.appeared,
              "disappeared": measurements.idle.disappeared,
              "normalization": "one logical CPU = 100%; null if a process exited" },
            summarize_samples("ipc.forward", "microseconds", &measurements.ipc_samples_us),
            {
                "name": "idle.main_cpu",
                "value_percent": measurements.idle.main_percent,
            },
            {
                "name": "process_tree",
                "processes": measurements.process_metrics.processes,
                "renderers": measurements.process_metrics.renderers,
                "pss_kib": measurements.process_metrics.pss_kib,
                "rss_kib": measurements.process_metrics.rss_kib,
                "threads": (measurements.process_metrics.unavailable_thread_processes == 0).then_some(measurements.process_metrics.threads),
                "unavailable_thread_processes": measurements.process_metrics.unavailable_thread_processes,
                "unavailable_memory_processes": measurements.process_metrics.unavailable_memory_processes,
            }
        ],
        "runtime": {
            "graphics_backend": measurements.runtime.get("graphics_backend").cloned().unwrap_or(Value::Null),
            "window_system": measurements.runtime.get("window_system").cloned().unwrap_or(Value::Null),
            "gpu_driver": measurements.runtime.get("gpu_driver").cloned().unwrap_or(Value::Null),
            "software_rendering": measurements.runtime.get("software_rendering").cloned().unwrap_or(Value::Null),
        },
        "targets": {
            "scope": "requested page workload on the current native desktop session; no fixed release thresholds are claimed"
        }
    })
}

fn create_ui_basedir() -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    for attempt in 0..32_u8 {
        let path = std::env::temp_dir().join(format!(
            "ferric-browser-ui-perf-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                        .map_err(|error| format!("could not secure {}: {error}", path.display()))?;
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "could not create disposable UI performance root: {error}"
                ));
            }
        }
    }
    Err("could not allocate a unique disposable UI performance root".into())
}

fn browser_json(browser: &Path, basedir: &Path, arguments: &[&str]) -> Result<Value, String> {
    browser_json_response(browser, basedir, arguments, false)?
        .ok_or_else(|| "unexpected busy response".to_owned())
}

fn browser_json_response(
    browser: &Path,
    basedir: &Path,
    arguments: &[&str],
    allow_busy: bool,
) -> Result<Option<Value>, String> {
    let output = Command::new(browser)
        .arg("--basedir")
        .arg(basedir)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("could not invoke {}: {error}", browser.display()))?;
    if allow_busy && busy_response(output.status.code(), &output.stderr) {
        return Ok(None);
    }
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{} returned {}: {}",
            browser.display(),
            output.status,
            stderr.trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map(Some)
        .map_err(|error| format!("Ferric Browser returned invalid JSON: {error}"))
}

fn busy_response(status: Option<i32>, stderr: &[u8]) -> bool {
    status == Some(6) && serde_json::from_slice::<Value>(stderr).is_ok_and(|value| {
        value.get("error").and_then(|error| error.get("code")).and_then(Value::as_str) == Some("E_BUSY")
    })
}

fn fixture_stderr(basedir: &Path) -> String {
    let mut bytes = Vec::new();
    if let Ok(mut file) = fs::File::open(basedir.join("browser-stderr.log")) {
        let size = file.metadata().map_or(0, |metadata| metadata.len());
        let _ = file.seek(SeekFrom::Start(size.saturating_sub(4096)));
        let _ = file.take(4096).read_to_end(&mut bytes);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn process_cpu_ticks(pid: u32) -> Result<u64, String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|error| format!("could not read browser CPU counters: {error}"))?;
    let fields = stat
        .rsplit_once(')')
        .ok_or_else(|| "browser CPU counters had an invalid shape".to_owned())?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    let user = fields
        .get(11)
        .ok_or_else(|| "browser CPU counters omitted user ticks".to_owned())?
        .parse::<u64>()
        .map_err(|_| "browser user CPU ticks were invalid".to_owned())?;
    let system = fields
        .get(12)
        .ok_or_else(|| "browser CPU counters omitted system ticks".to_owned())?
        .parse::<u64>()
        .map_err(|_| "browser system CPU ticks were invalid".to_owned())?;
    Ok(user.saturating_add(system))
}

fn clock_ticks_per_second() -> Result<u64, String> {
    let output = Command::new("getconf")
        .arg("CLK_TCK")
        .output()
        .map_err(|error| format!("could not query CLK_TCK: {error}"))?;
    if !output.status.success() {
        return Err("getconf CLK_TCK failed".into());
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .map_err(|_| "getconf CLK_TCK returned an invalid value".into())
}

#[derive(Debug)]
struct ProcessTreeMetrics {
    processes: usize,
    renderers: usize,
    pss_kib: u64,
    rss_kib: u64,
    threads: u64,
    unavailable_memory_processes: usize,
    unavailable_thread_processes: usize,
}

fn process_tree_pids(root: u32) -> Result<BTreeSet<u32>, String> {
    let mut parents = BTreeMap::new();
    for entry in fs::read_dir("/proc").map_err(|error| format!("could not scan /proc: {error}"))? {
        let entry = entry.map_err(|error| format!("could not enumerate /proc: {error}"))?;
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some((_, fields)) = stat.rsplit_once(')') else {
            continue;
        };
        let Some(parent) = fields
            .split_whitespace()
            .nth(1)
            .and_then(|field| field.parse::<u32>().ok())
        else {
            continue;
        };
        parents.insert(pid, parent);
    }

    let mut tree = BTreeSet::from([root]);
    loop {
        let previous_len = tree.len();
        for (&pid, &parent) in &parents {
            if tree.contains(&parent) {
                tree.insert(pid);
            }
        }
        if tree.len() == previous_len {
            break;
        }
    }

    Ok(tree)
}

fn process_tree_metrics(root: u32) -> Result<ProcessTreeMetrics, String> {
    let tree = process_tree_pids(root)?;
    let mut metrics = ProcessTreeMetrics {
        processes: 0,
        renderers: 0,
        pss_kib: 0,
        rss_kib: 0,
        threads: 0,
        unavailable_memory_processes: 0,
        unavailable_thread_processes: 0,
    };
    for pid in tree {
        metrics.processes += 1;
        let thread_count = fs::read_to_string(format!("/proc/{pid}/status"))
            .ok()
            .and_then(|status| {
                status.lines().find_map(|line| {
                    line.strip_prefix("Threads:")
                        .and_then(|value| value.trim().parse::<u64>().ok())
                })
            });
        if let Some(count) = thread_count {
            metrics.threads += count;
        } else {
            metrics.unavailable_thread_processes += 1;
        }
        let Ok(rollup) = fs::read_to_string(format!("/proc/{pid}/smaps_rollup")) else {
            metrics.unavailable_memory_processes += 1;
            continue;
        };
        for line in rollup.lines() {
            let mut fields = line.split_whitespace();
            match (fields.next(), fields.next()) {
                (Some("Pss:"), Some(value)) => {
                    metrics.pss_kib = metrics
                        .pss_kib
                        .saturating_add(value.parse::<u64>().unwrap_or(0));
                }
                (Some("Rss:"), Some(value)) => {
                    metrics.rss_kib = metrics
                        .rss_kib
                        .saturating_add(value.parse::<u64>().unwrap_or(0));
                }
                _ => {}
            }
        }
        if fs::read(format!("/proc/{pid}/cmdline"))
            .is_ok_and(|command| contains_bytes(&command, b"--type=renderer"))
        {
            metrics.renderers += 1;
        }
    }
    if metrics.processes == metrics.unavailable_memory_processes {
        return Err("browser process tree disappeared before memory measurement".into());
    }
    Ok(metrics)
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn summarize_samples(name: &str, unit: &str, raw: &[f64]) -> Value {
    let mut sorted = raw.to_owned();
    sorted.sort_by(f64::total_cmp);
    json!({
        "name": name,
        "unit": unit,
        "median": percentile(&sorted, 1, 2),
        "p95": percentile(&sorted, 19, 20),
        "p99": (raw.len() >= 1000).then(|| percentile(&sorted, 99, 100)),
        "raw_samples": raw,
    })
}

fn synthetic_history_catalog() -> Vec<CompletionCandidate> {
    (0..HISTORY_CANDIDATES)
        .map(|index| CompletionCandidate {
            insert_text: format!("https://example.test/document/{index}"),
            label: format!("Example document {index}"),
            detail: "synthetic performance fixture".into(),
            category: CompletionCategory::History,
            recency: (HISTORY_CANDIDATES - index) as u64,
            frequency: u32::try_from(index % 17).expect("synthetic frequency is bounded"),
        })
        .collect()
}

fn measure<F>(name: &str, samples: usize, target_p95_us: Option<f64>, mut operation: F) -> Value
where
    F: FnMut(),
{
    let mut raw = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        operation();
        raw.push(started.elapsed().as_secs_f64() * 1_000_000.0);
    }
    let mut sorted = raw.clone();
    sorted.sort_by(f64::total_cmp);
    let p95_us = percentile(&sorted, 19, 20);
    json!({
        "name": name,
        "median_us": percentile(&sorted, 1, 2),
        "p95_us": p95_us,
        "p99_us": (raw.len() >= 1000).then(|| percentile(&sorted, 99, 100)),
        "target_p95_us": target_p95_us,
        "within_target": target_p95_us.is_none_or(|target| p95_us <= target),
        "raw_samples_us": raw,
    })
}

fn percentile(sorted: &[f64], numerator: usize, denominator: usize) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if numerator == 1 && denominator == 2 && sorted.len().is_multiple_of(2) {
        return sorted[sorted.len() / 2 - 1].midpoint(sorted[sorted.len() / 2]);
    }
    let index = (sorted.len() * numerator)
        .div_ceil(denominator)
        .saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}

fn provenance() -> Value {
    fn command(program: &str, args: &[&str]) -> Option<String> {
        let output = Command::new(program).args(args).output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
    json!({
        "source_commit": command("git", &["rev-parse", "HEAD"]),
        "working_tree_status": command("git", &["status", "--porcelain"]),
        "rustc": command("rustc", &["--version"]),
        "kernel": command("uname", &["-srvm"]),
        "distribution": fs::read_to_string("/etc/os-release").ok(),
        "qt": command("pkg-config", &["--modversion", "Qt6Core"]),
        "qt_webengine": command("pkg-config", &["--modversion", "Qt6WebEngineCore"]),
        "rustflags": std::env::var("RUSTFLAGS").ok(),
        "qpa_platform": std::env::var("QT_QPA_PLATFORM").ok(),
        "quick_backend": std::env::var("QT_QUICK_BACKEND").ok(),
        "scale_factor": std::env::var("QT_SCALE_FACTOR").ok(),
        "chromium_flags": std::env::var("QTWEBENGINE_CHROMIUM_FLAGS").ok(),
        "invocation": std::env::args().collect::<Vec<_>>(),
        "source_note": "git describes the invoking checkout; build the binaries from this checkout before measuring"
    })
}

fn reference_environment() -> Value {
    let mut environment = BTreeMap::new();
    for (key, path) in [
        ("cpu_model", "/proc/cpuinfo"),
        ("memory", "/proc/meminfo"),
        (
            "cpu_governor",
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor",
        ),
    ] {
        if let Ok(contents) = fs::read_to_string(path) {
            let value = if key == "cpu_model" {
                contents
                    .lines()
                    .find_map(|line| line.strip_prefix("model name\t: "))
                    .unwrap_or("unknown")
                    .to_owned()
            } else if key == "memory" {
                contents
                    .lines()
                    .find(|line| line.starts_with("MemTotal:"))
                    .unwrap_or("unknown")
                    .to_owned()
            } else {
                contents.trim().to_owned()
            };
            environment.insert(key.to_owned(), Value::String(value));
        }
    }
    environment.insert(
        "wayland_display".into(),
        std::env::var("WAYLAND_DISPLAY").map_or(Value::Null, Value::String),
    );
    Value::Object(environment.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_SAMPLES, MAX_SAMPLES, PerfSuite, contains_bytes, measure, parse_arguments,
        percentile,
    };

    #[test]
    fn perf_arguments_enforce_bounded_samples() {
        let defaults = parse_arguments(&[]).expect("defaults");
        assert_eq!(defaults.samples, DEFAULT_SAMPLES);
        assert_eq!(defaults.suite, PerfSuite::Core);
        assert!(parse_arguments(&["--samples".into(), "29".into()]).is_err());
        assert!(parse_arguments(&["--samples".into(), (MAX_SAMPLES + 1).to_string()]).is_err());
        assert!(
            parse_arguments(&[
                "--samples".into(),
                "30".into(),
                "--samples".into(),
                "31".into()
            ])
            .is_err()
        );
    }

    #[test]
    fn readiness_retries_only_explicit_temporary_busy_responses() {
        let busy = br#"{"error":{"code":"E_BUSY"}}"#;
        assert!(super::busy_response(Some(6), busy));
        assert!(!super::busy_response(Some(1), busy));
        assert!(!super::busy_response(Some(6), br#"{"error":{"code":"E_DENIED"}}"#));
        assert!(!super::busy_response(Some(6), b"malformed E_BUSY"));
        assert!(!super::busy_response(None, busy));
    }

    #[test]
    fn perf_arguments_select_one_known_suite() {
        let ui = parse_arguments(&["--suite".into(), "ui".into()]).expect("UI suite");
        assert_eq!(ui.suite, PerfSuite::Ui);
        assert!(parse_arguments(&["--suite".into(), "unknown".into()]).is_err());
        assert!(
            parse_arguments(&[
                "--suite".into(),
                "core".into(),
                "--suite".into(),
                "ui".into()
            ])
            .is_err()
        );
    }

    #[test]
    fn ui_sampling_options_are_bounded_and_not_silently_ignored() {
        let parse =
            |args: &[&str]| parse_arguments(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>());
        let options = parse(&[
            "--suite",
            "ui",
            "--repetitions",
            "5",
            "--idle-seconds",
            "120",
        ])
        .unwrap();
        assert_eq!((options.repetitions, options.idle_seconds), (5, 120));
        for args in [
            vec!["--repetitions", "5"],
            vec!["--suite", "ui", "--idle-seconds", "0"],
            vec!["--suite", "ui", "--repetitions", "101"],
            vec!["--suite", "ui", "--idle-seconds", "3601"],
            vec!["--suite", "ui", "--repetitions", "2", "--repetitions", "3"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn page_readiness_uses_real_loading_states_and_rejects_failures() {
        let response = |state: &str| serde_json::json!({"tabs": [{"loading": state}]});
        for state in ["idle", "complete"] {
            assert!(super::page_workload_ready(&response(state), 1).unwrap());
        }
        for state in ["provisional", "committed"] {
            assert!(!super::page_workload_ready(&response(state), 1).unwrap());
        }
        for state in ["failed", "cancelled", "loading"] {
            assert!(super::page_workload_ready(&response(state), 1).is_err());
        }
        assert!(!super::page_workload_ready(&response("complete"), 2).unwrap());
        assert!(super::page_workload_ready(&serde_json::json!({}), 1).is_err());
    }

    #[test]
    fn ui_workloads_require_valid_explicit_options() {
        let parse =
            |args: &[&str]| parse_arguments(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>());
        let options = parse(&[
            "--suite",
            "ui",
            "--pages",
            "20",
            "--presentation",
            "windows",
        ])
        .unwrap();
        assert_eq!(options.pages, 20);
        assert_eq!(options.presentation, "windows");
        assert_eq!(options.page_url, "about:blank");
        for args in [
            vec!["--pages", "5"],
            vec!["--suite", "ui", "--pages", "0"],
            vec!["--suite", "ui", "--pages", "21"],
            vec!["--suite", "ui", "--presentation", "invalid"],
            vec!["--suite", "ui", "--page-url", "bad\nurl"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn cpu_accounting_distinguishes_pid_reuse_and_new_children() {
        use std::collections::BTreeMap;
        let before = BTreeMap::from([((10, 100), 20), ((11, 101), 50)]);
        let after = BTreeMap::from([((10, 100), 27), ((11, 200), 2), ((12, 201), 3)]);
        assert_eq!(super::cpu_delta(&before, &after), (12, 2, 1));
        let stat = "10 (name with ) spaces) S 1 0 0 0 0 0 0 0 0 0 20 3 0 0 0 0 0 0 12345";
        assert_eq!(super::parse_cpu_stat(stat).unwrap(), (12345, 23));
        assert!(super::parse_cpu_stat("bad stat").is_err());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn reads_live_linux_process_counters() {
        let pid = std::process::id();
        let snapshot = super::cpu_snapshot(pid).unwrap();
        assert!(
            snapshot
                .keys()
                .any(|(member, birth)| *member == pid && *birth > 0)
        );
        let metrics = super::process_tree_metrics(pid).unwrap();
        assert!(metrics.processes >= 1);
        assert!(metrics.threads >= 1);
        assert!(metrics.pss_kib > 0);
        assert_eq!(metrics.unavailable_thread_processes, 0);
    }

    #[test]
    fn short_runs_do_not_claim_p99() {
        assert!(super::summarize_samples("short", "us", &[1.0; 30])["p99"].is_null());
        assert_eq!(
            super::summarize_samples("long", "us", &[1.0; 1000])["p99"],
            1.0
        );
    }

    #[test]
    fn renderer_marker_search_is_byte_safe() {
        assert!(contains_bytes(
            b"QtWebEngineProcess\0--type=renderer\0",
            b"--type=renderer"
        ));
        assert!(!contains_bytes(b"--type=zygote", b"--type=renderer"));
    }

    #[test]
    fn percentile_uses_nearest_rank_on_sorted_samples() {
        let samples = [1.0, 2.0, 3.0, 4.0];
        assert!((percentile(&samples, 1, 2) - 2.5).abs() < f64::EPSILON);
        assert!((percentile(&samples, 19, 20) - 4.0).abs() < f64::EPSILON);
        assert!(percentile(&[], 99, 100).abs() < f64::EPSILON);
    }

    #[test]
    fn measurements_report_target_status() {
        let report = measure("test", 30, Some(1_000.0), || {});
        assert_eq!(report["name"], "test");
        assert_eq!(report["target_p95_us"], 1_000.0);
        assert_eq!(report["within_target"], true);
    }
}
