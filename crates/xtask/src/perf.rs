use browser_core::{
    CommandRegistry, CompletionCandidate, CompletionCategory, ParseInput, complete, parse_chain,
    switcher_rank, tokenize_switcher_query,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    hint::black_box,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const DEFAULT_SAMPLES: usize = 30;
const MAX_SAMPLES: usize = 1_000;
const HISTORY_CANDIDATES: usize = 100_000;
const UI_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const UI_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const UI_IDLE_WINDOW: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PerfSuite {
    Core,
    Ui,
}

#[derive(Debug, Eq, PartialEq)]
struct PerfOptions {
    suite: PerfSuite,
    samples: usize,
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
        PerfSuite::Ui => collect_ui_report(options.samples)?,
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

fn parse_arguments(arguments: &[String]) -> Result<PerfOptions, String> {
    let mut suite = PerfSuite::Core;
    let mut suite_seen = false;
    let mut samples = DEFAULT_SAMPLES;
    let mut samples_seen = false;
    let mut output = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--suite" => {
                if suite_seen || index + 1 >= arguments.len() {
                    return Err("perf accepts --suite core|ui at most once".into());
                }
                suite_seen = true;
                suite = match arguments[index + 1].as_str() {
                    "core" => PerfSuite::Core,
                    "ui" => PerfSuite::Ui,
                    _ => return Err("perf --suite must be core or ui".into()),
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
    Ok(PerfOptions {
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
        "schema_version": 1,
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

fn collect_ui_report(samples: usize) -> Result<Value, String> {
    if !cfg!(target_os = "linux") {
        return Err("perf --suite ui currently requires Linux /proc metrics".into());
    }
    let harness = std::env::current_exe()
        .map_err(|error| format!("could not resolve the performance harness: {error}"))?;
    let browser = harness
        .parent()
        .ok_or_else(|| "performance harness has no executable directory".to_owned())?
        .join("rustbrowser");
    if !browser.is_file() {
        return Err(format!(
            "UI performance suite requires {}; build it with `cargo build --release -p rustbrowser -p xtask --locked`",
            browser.display()
        ));
    }

    let (mut run, startup_ms) = start_ui_run(&browser)?;
    let browser_pid = run.child.id();
    let ipc_samples_us = collect_ipc_samples(&browser, &run.basedir, samples)?;
    let (idle_elapsed, idle_cpu_percent) = measure_idle_cpu(browser_pid)?;
    let process_metrics = process_tree_metrics(browser_pid)?;
    let diagnostics = browser_json(&browser, &run.basedir, &["diagnostics", "--format", "json"])?;
    let runtime = diagnostics.get("runtime").cloned().unwrap_or(Value::Null);
    shutdown_ui_run(&browser, &mut run)?;
    let measurements = UiMeasurements {
        startup_ms,
        ipc_samples_us,
        idle_elapsed,
        idle_cpu_percent,
        process_metrics,
        runtime,
    };
    Ok(ui_report(&browser, samples, &measurements))
}

fn start_ui_run(browser: &Path) -> Result<(UiRun, f64), String> {
    let basedir = create_ui_basedir()?;
    let child = Command::new(browser)
        .arg("--basedir")
        .arg(&basedir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("could not start {}: {error}", browser.display()))?;
    let mut run = UiRun { child, basedir };
    let started = Instant::now();
    loop {
        if let Some(status) = run
            .child
            .try_wait()
            .map_err(|error| format!("could not inspect RustBrowser startup: {error}"))?
        {
            return Err(format!(
                "RustBrowser exited before UI measurement with {status}"
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
                "RustBrowser UI startup exceeded {} seconds: {error}",
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

fn measure_idle_cpu(pid: u32) -> Result<(Duration, f64), String> {
    thread::sleep(Duration::from_millis(250));
    let ticks_before = process_cpu_ticks(pid)?;
    let started = Instant::now();
    thread::sleep(UI_IDLE_WINDOW);
    let elapsed = started.elapsed();
    let tick_delta = u32::try_from(process_cpu_ticks(pid)?.saturating_sub(ticks_before))
        .map_err(|_| "browser CPU tick delta exceeded the measurement range".to_owned())?;
    let tick_rate = u32::try_from(clock_ticks_per_second()?)
        .map_err(|_| "CLK_TCK exceeded the measurement range".to_owned())?;
    let percent = 100.0 * (f64::from(tick_delta) / f64::from(tick_rate)) / elapsed.as_secs_f64();
    Ok((elapsed, percent))
}

fn shutdown_ui_run(browser: &Path, run: &mut UiRun) -> Result<(), String> {
    let quit = Command::new(browser)
        .arg("--basedir")
        .arg(&run.basedir)
        .args(["command", "--", "quit"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("could not request RustBrowser shutdown: {error}"))?;
    if !quit.success() {
        return Err(format!(
            "RustBrowser rejected the UI-suite shutdown request with {quit}"
        ));
    }
    let started = Instant::now();
    loop {
        if run
            .child
            .try_wait()
            .map_err(|error| format!("could not inspect RustBrowser shutdown: {error}"))?
            .is_some()
        {
            return Ok(());
        }
        if started.elapsed() >= UI_SHUTDOWN_TIMEOUT {
            return Err(format!(
                "RustBrowser did not exit within {} seconds",
                UI_SHUTDOWN_TIMEOUT.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

struct UiMeasurements {
    startup_ms: f64,
    ipc_samples_us: Vec<f64>,
    idle_elapsed: Duration,
    idle_cpu_percent: f64,
    process_metrics: ProcessTreeMetrics,
    runtime: Value,
}

fn ui_report(browser: &Path, samples: usize, measurements: &UiMeasurements) -> Value {
    json!({
        "schema_version": 1,
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
            "browser": browser.file_name().and_then(|name| name.to_str()).unwrap_or("rustbrowser"),
        },
        "reference_environment": reference_environment(),
        "method": {
            "startup_ready_boundary": "successful owner-IPC operations query",
            "idle_window_seconds": measurements.idle_elapsed.as_secs_f64(),
            "ipc_sample_boundary": "fresh forwarding CLI process through decoded owner response",
            "memory": "sum of Linux smaps_rollup values across the browser process tree",
            "notes": "The suite uses a disposable basedir, automatic Qt graphics selection, one blank tab, and closed DevTools."
        },
        "measurements": [
            {
                "name": "startup.ipc_ready",
                "value_ms": measurements.startup_ms,
            },
            summarize_samples("ipc.forward", "microseconds", &measurements.ipc_samples_us),
            {
                "name": "idle.main_cpu",
                "value_percent": measurements.idle_cpu_percent,
            },
            {
                "name": "process_tree",
                "processes": measurements.process_metrics.processes,
                "renderers": measurements.process_metrics.renderers,
                "pss_kib": measurements.process_metrics.pss_kib,
                "rss_kib": measurements.process_metrics.rss_kib,
            }
        ],
        "runtime": {
            "graphics_backend": measurements.runtime.get("graphics_backend").cloned().unwrap_or(Value::Null),
            "window_system": measurements.runtime.get("window_system").cloned().unwrap_or(Value::Null),
            "gpu_driver": measurements.runtime.get("gpu_driver").cloned().unwrap_or(Value::Null),
            "software_rendering": measurements.runtime.get("software_rendering").cloned().unwrap_or(Value::Null),
        },
        "targets": {
            "scope": "single blank tab on the current native desktop session; no fixed release thresholds are claimed"
        }
    })
}

fn create_ui_basedir() -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    for attempt in 0..32_u8 {
        let path = std::env::temp_dir().join(format!(
            "rustbrowser-ui-perf-{}-{nonce}-{attempt}",
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
    let output = Command::new(browser)
        .arg("--basedir")
        .arg(basedir)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("could not invoke {}: {error}", browser.display()))?;
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
        .map_err(|error| format!("RustBrowser returned invalid JSON: {error}"))
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
}

fn process_tree_metrics(root: u32) -> Result<ProcessTreeMetrics, String> {
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

    let mut metrics = ProcessTreeMetrics {
        processes: 0,
        renderers: 0,
        pss_kib: 0,
        rss_kib: 0,
    };
    for pid in tree {
        let Ok(rollup) = fs::read_to_string(format!("/proc/{pid}/smaps_rollup")) else {
            continue;
        };
        metrics.processes += 1;
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
    if metrics.processes == 0 {
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
        "p99": percentile(&sorted, 99, 100),
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
        "p99_us": percentile(&sorted, 99, 100),
        "target_p95_us": target_p95_us,
        "within_target": target_p95_us.is_none_or(|target| p95_us <= target),
        "raw_samples_us": raw,
    })
}

fn percentile(sorted: &[f64], numerator: usize, denominator: usize) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) * numerator).div_ceil(denominator);
    sorted[index.min(sorted.len() - 1)]
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
        assert!((percentile(&samples, 1, 2) - 3.0).abs() < f64::EPSILON);
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
