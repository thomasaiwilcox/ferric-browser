"""Import a separately authorised and qualified recovery without rewriting attempts."""
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess

from common import PACK, digest, git, load, safe_relative

# Filled only after the real qualification and independent evidence review.
CONTRACT_SHA256 = "b9f3d779ce9c8e3c2a90eaec12b0063d16b5056560957a1b678eba974a814833"
STAGES = ["sdk", "engine", "configure", "harness", "link", "discovery", "native"]
HELPER_CASES = {"timeout_before_completion", "late_completion_while_fresh_waiter", "queued_current_completion_42", "synchronous_completion_7"}


def independent(reviewer):
    return isinstance(reviewer, str) and bool(reviewer.strip()) and reviewer != "/root"


def concrete_mapping(mapping, cases=None):
    return (isinstance(mapping, dict) and bool(mapping)
            and (cases is None or set(mapping) == cases)
            and all(isinstance(value, str) and value.strip() for value in mapping.values()))


def review_chain(root, report, field):
    def check(entry):
        if isinstance(entry, dict):
            if "path" in entry or "sha256" in entry:
                path = Path(entry["path"])
                if path.is_absolute():
                    path = path.relative_to(root)
                reference(root, {"path": str(path), "sha256": entry["sha256"]})
            else:
                for value in entry.values():
                    check(value)
        elif isinstance(entry, list):
            for value in entry:
                check(value)
    check(report.get(field, []))


def recovery_open(state, target):
    owners = {"F02e2s", "F02e2s-direct-recovery"}
    artifact = Path(target).name
    lease = state.get("native_lease", {})
    return (bool(owners.intersection(state.get("active", [])))
            or artifact in state.get("active_builds", [])
            or any(record.get("slice") in owners or record.get("artifact") == artifact
                   or record.get("target") == target for record in state.get("active_locks", {}).values())
            or bool(lease.get("active") and (lease.get("reserved_for") in owners
                    or lease.get("slice") in owners or lease.get("artifact") == artifact or lease.get("target") == target)))


def object_sha(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def contract_A57(pack=PACK):
    path = pack / "control/direct-recovery.A57.json"
    if digest(path) != CONTRACT_SHA256:
        raise ValueError("A57 qualified import contract changed or is not final")
    return load(path)


def historical_source_map_A57(mapping, pack=PACK):
    contract = contract_A57(pack)
    result = copy.deepcopy(mapping)
    entries = result["execution_amendments"]
    if [entry for entry in entries if entry.get("id") == "A57"] != [contract["source_map"]]:
        raise ValueError("A57 source map missing, duplicated or changed")
    result["execution_amendments"] = [entry for entry in entries if entry.get("id") != "A57"]
    return result


def validate_plan_A57(plan, pack=PACK):
    if "direct_recovery_A57" not in plan and not (pack / "control/direct-recovery.A57.json").exists():
        return []
    try:
        c = contract_A57(pack)
        previous = copy.deepcopy(plan)
        if previous.pop("direct_recovery_A57") != c["metadata"] or object_sha(previous) != c["prior_plan_sha256"]:
            raise ValueError("A57 may add only the exact import; existing tasks and limits stay unchanged")
        prior_map = historical_source_map_A57(load(pack / "control/source-map.execution.json"), pack)
        if object_sha(prior_map) != c["prior_source_map_sha256"]:
            raise ValueError("A57 changed an earlier source-map record")
        if c["metadata"]["kind"] != "qualified_direct_recovery" or c["metadata"]["slice"] != "F02e2s":
            raise ValueError("A57 is only the qualified F02e2s import")
    except (OSError, ValueError, KeyError, TypeError) as error:
        return [f"A57 plan: {error}"]
    return []


def reference(root, record):
    path = safe_relative(root, record["path"])
    if digest(path) != record["sha256"]:
        raise ValueError("A57 evidence changed: " + record["path"])
    return path


def snapshot_matches(snapshot, source):
    return (snapshot.get("sha") == source["commit"] and snapshot.get("tree") == source["tree"]
            and snapshot.get("dirty") is False)


def product_inventory(repository, commit):
    lines = git(repository, "ls-tree", "-r", commit).splitlines()
    return [line for line in lines if not line.split("\t", 1)[1].startswith("docs/implementation/everyday-v1/")]


def validate_integration_A57(root, state, c):
    authority = state["reconciliation_A57_authority"]
    report_path = reference(root, authority["review"])
    if authority["review"]["path"] != c["metadata"]["pack_review"]:
        raise ValueError("A57 pack review is on an alternate path")
    report = load(report_path)
    candidate = report.get("candidate", {})
    if (report.get("decision") != "accept" or not independent(report.get("reviewer"))
            or candidate.get("commit") != authority["commit"] or candidate.get("tree") != authority["tree"]):
        raise ValueError("A57 pack was not independently accepted at its exact commit")
    repository = safe_relative(root, authority["worktree"])
    if (git(repository, "rev-parse", "HEAD") != authority["commit"]
            or git(repository, "rev-parse", "HEAD^{tree}") != authority["tree"]
            or git(repository, "status", "--porcelain")):
        raise ValueError("A57 import pack changed after review")
    if product_inventory(repository, authority["commit"]) != product_inventory(repository, c["source"]["commit"]):
        raise ValueError("A57 integrated product paths/modes/blobs differ from the tested candidate")
    if git(repository, "merge-base", "--is-ancestor", authority["commit"], state["integration_sha"], check=False).returncode:
        raise ValueError("Current integration does not descend from the qualified A57 import")


def validate_history_A57(root, c):
    history_path = reference(root, c["metadata"]["historical_state"])
    original_path = reference(root, c["metadata"]["original_historical_state"])
    if history_path.parent != root / "control" or history_path.read_bytes() != original_path.read_bytes():
        raise ValueError("A57 historical validation requires the complete snapshot under the real workspace control root")
    verifier = c["historical_validator"]
    repository = safe_relative(root, verifier["worktree"])
    if (git(repository, "rev-parse", "HEAD") != verifier["commit"]
            or git(repository, "rev-parse", "HEAD^{tree}") != verifier["tree"]
            or git(repository, "status", "--porcelain")):
        raise ValueError("A57 retained historical validator changed")
    result = subprocess.run(["python3", str(repository / verifier["entry_point"]), "--state", str(history_path)],
                            capture_output=True, text=True, timeout=60)
    if result.returncode or "VALID: 97 slices" not in result.stdout:
        raise ValueError("A57 complete historical A56 validation failed: " + result.stderr[:1000])
    return load(history_path)


def validate_qualification_A57(root, c):
    source = c["source"]
    q = load(reference(root, c["metadata"]["qualification"]))
    if q.get("result") != "passed" or q.get("source_sha") != source["commit"] or q.get("source_tree") != source["tree"]:
        raise ValueError("A57 import is not qualified at the exact source")
    configuration = load(reference(root, q["configuration"]))
    target = str(safe_relative(root, c["target"]))
    if configuration["target"] != target or configuration["cargo_target"] != target + "/cargo-target" or configuration["cmake_dir"] != target + "/cmake":
        raise ValueError("A57 qualification target differs")
    if configuration["environment"]["CARGO_TARGET_DIR"] != configuration["cargo_target"]:
        raise ValueError("A57 build environment target differs")
    reference(root, q["source_review"])
    sr = load(root / q["source_review"]["path"])
    if (sr.get("decision") not in {"accept", "approve"} or sr.get("candidate_commit") != source["commit"]
            or sr.get("candidate_tree") != source["tree"] or not independent(sr.get("reviewer"))
            or sr.get("findings") or not sr.get("complete_current_product_delta_covered")):
        raise ValueError("A57 final complete source review does not approve this candidate")
    review_chain(root, sr, "prior_reviews_preserved")
    runner_review = load(reference(root, q["runner_review"]))
    if runner_review.get("decision") != "approve" or not independent(runner_review.get("reviewer")) or runner_review.get("findings"):
        raise ValueError("A57 runner lacks independent approval")
    review_chain(root, runner_review, "preserved_reviews")
    review_chain(root, runner_review, "preserved_chain")
    supports = q["execution_support"]
    required_supports = {"run.py", "recovery_gate.py", "run_gate.py", "common.py", "qt_include_guard.py", "watch_qt_includes.py", "verify_link.py", "helper_sanitizer.py", "native_session.py", "hyprland-test.lua"}
    if set(supports) != required_supports:
        raise ValueError("A57 execution support inventory is incomplete")
    for name, record in supports.items():
        reference(root, record)
        if name in runner_review["exact_hashes"] and record["sha256"] != runner_review["exact_hashes"][name]:
            raise ValueError("A57 execution support differs from reviewed bytes: " + name)
    if runner_review["exact_hashes"]["CONFIG.json"] != q["configuration"]["sha256"]:
        raise ValueError("A57 runner reviewed a different configuration")
    gates = q["gates"]
    if [gate["stage"] for gate in gates] != STAGES:
        raise ValueError("A57 requires all seven ordered native gates")
    previous_finish = None
    records = {}
    for gate in gates:
        invocation = load(reference(root, gate["invocation"]))
        receipt_path = reference(root, gate["receipt"])
        receipt = load(receipt_path)
        closure = load(reference(root, gate["closure"]))
        monitor = load(reference(root, gate["monitor"]))
        reference(root, gate["runner"])
        if (invocation["stage"] != gate["stage"] or invocation["target"] != target
                or invocation["config_sha256"] != q["configuration"]["sha256"]
                or invocation["runner_sha256"] != gate["runner"]["sha256"]
                or not snapshot_matches(invocation["source"], source)):
            raise ValueError("A57 gate invocation/source/config/runner mismatch")
        if (receipt.get("result") != "passed" or receipt.get("exit_code") != 0
                or receipt.get("timed_out") or receipt.get("output_limit_exceeded") or receipt.get("launch_error")
                or receipt.get("observed_failed", 0) or receipt.get("observed_skips", 0)
                or receipt.get("qt_include_guard") != "clear"
                or receipt.get("qt_include_prefix") != configuration["qt_prefix"]
                or any(not snapshot_matches(receipt[key], source) for key in ("source_before", "source_after"))):
            raise ValueError("A57 gate did not pass on clean exact source")
        if (closure.get("result") != "passed" or closure.get("lease_released") is not True
                or not snapshot_matches(closure["source"], source) or closure.get("monitor") != monitor
                or monitor.get("result") != "clear" or monitor.get("pinned_qt_prefix") != configuration["qt_prefix"]
                or monitor.get("artifact") != Path(target).name):
            raise ValueError("A57 gate ownership/monitor did not close cleanly")
        if previous_finish is not None and receipt["started"] < previous_finish:
            raise ValueError("A57 selected gates overlap or are out of order")
        previous_finish = receipt["finished"]
        log = safe_relative(receipt_path.parent, receipt["log"])
        if digest(log) != receipt["log_sha256"]:
            raise ValueError("A57 raw command log changed")
        records[gate["stage"]] = (receipt, log.read_text(errors="replace"))
    qt = configuration["qt_prefix"]
    if records["sdk"][0]["command"] != [qt + "/bin/qmake", "-query", "QT_VERSION"] or records["sdk"][1].strip() != "6.11.2":
        raise ValueError("A57 matching Qt SDK preflight missing")
    abi = reference(root, q["sdk_kf_abi_log"]).read_text()
    if "Qt_6.11" not in abi or "Qt_6.12" in abi or digest(Path(configuration["kf_library"])) != q["kf_library_sha256"]:
        raise ValueError("A57 matching KF library/ABI changed")
    engine = ["cargo", "+stable", "build", "--package", "ferric-browser-engine-qt", "--release", "--locked", "--offline", "--jobs", "2", "--verbose", "--target-dir", configuration["cargo_target"]]
    if records["engine"][0]["command"] != engine:
        raise ValueError("A57 engine must use the exact release/offline/two-job command")
    configure = records["configure"][0]["command"]
    if (configure[:4] != ["cmake", "-S", "crates/ferric-browser-engine-qt/tests/public-page-host", "-B"]
            or configure[4] != configuration["cmake_dir"]
            or "-DFERRIC_ENGINE_LIBRARY=" + configuration["cargo_target"] + "/release/libferric_browser_engine_qt.a" not in configure):
        raise ValueError("A57 harness does not use the built engine archive")
    if records["harness"][0]["command"] != ["cmake", "--build", configuration["cmake_dir"], "--verbose", "--target", "public_page_host_integration_tests", "--parallel", "2"]:
        raise ValueError("A57 integration build command changed")
    link = load(reference(root, q["link_report"]))
    if (link.get("result") != "passed" or link.get("source_sha") != source["commit"] or link.get("source_tree") != source["tree"]
            or link.get("target") != target or link.get("engine_archive_before_kf6windowsystem") is not True
            or link.get("matching_qt_and_kf_runtime") is not True):
        raise ValueError("A57 actual executable/link/runtime report missing")
    link_command = records["link"][0]["command"]
    if len(link_command) != 4 or link_command[0] != "python3" or link_command[2:] != ["--out", str(root / q["link_report"]["path"])]:
        raise ValueError("A57 link report is not the executed link gate output")
    link_script = reference(root, q["link_script"])
    if Path(link_command[1]) != link_script or q["link_script"] != supports["verify_link.py"]:
        raise ValueError("A57 link gate script differs")
    cmake = Path(configuration["cmake_dir"])
    executable = cmake / "public_page_host_integration_tests"
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise ValueError("A57 retained native test is not executable")
    archive = Path(configuration["cargo_target"]) / "release/libferric_browser_engine_qt.a"
    link_file = cmake / "CMakeFiles/public_page_host_integration_tests.dir/link.txt"
    ldd_log = (root / q["link_report"]["path"]).parent / "link-runtime-ldd.log"
    for path, key in ((executable, "executable_sha256"), (archive, "engine_archive_sha256"), (link_file, "link_command_sha256"), (ldd_log, "ldd_log_sha256")):
        if digest(path) != link.get(key):
            raise ValueError("A57 retained native artifact changed: " + str(path))
    tokens = shlex.split(link_file.read_text())
    engine_positions = [i for i, token in enumerate(tokens) if token == str(archive)]
    kf_positions = [i for i, token in enumerate(tokens) if token == configuration["kf_library"]]
    if len(engine_positions) != 1 or len(kf_positions) != 1 or engine_positions[0] >= kf_positions[0]:
        raise ValueError("A57 actual engine/KF link order differs")
    runtime = ldd_log.read_text()
    kf = re.search(r"libKF6WindowSystem\S*\s+=>\s+(\S+)", runtime)
    qt_libraries = re.findall(r"libQt6\S*\s+=>\s+(\S+)", runtime)
    if ("not found" in runtime or not kf or Path(kf.group(1)).resolve() != Path(configuration["kf_library"]).resolve()
            or not qt_libraries or any(not Path(path).resolve().is_relative_to((Path(qt) / "lib").resolve()) for path in qt_libraries)):
        raise ValueError("A57 actual Qt/KF runtime selection differs")
    if records["discovery"][0]["command"] != ["ctest", "--test-dir", configuration["cmake_dir"], "--show-only=json-v1"]:
        raise ValueError("A57 must retain the actual JSON discovery command")
    inventory = json.loads(records["discovery"][1])
    selected = [test for test in inventory["tests"] if test["name"] == "T014e2s"]
    if len(selected) != 1:
        raise ValueError("A57 real production-bootstrap test must be discovered exactly once")
    test = selected[0]
    if test["command"] != [str(executable), "rustBrowserUiRequestReparentsSamePageAcrossHostsWithProfileBootstrap"]:
        raise ValueError("A57 real production-bootstrap test was not discovered")
    properties = {p["name"]: p["value"] for p in test["properties"]}
    if properties.get("TIMEOUT") != 180.0:
        raise ValueError("A57 native test timeout changed")
    helper = reference(root, q["private_bus_helper"])
    if q["private_bus_helper"] != c["private_bus_helper"]:
        raise ValueError("A57 qualified private bus helper changed")
    cleanup_path = reference(root, q["private_bus_cleanup"])
    session_path = reference(root, q["native_session"])
    test_command = ["ctest", "--test-dir", configuration["cmake_dir"], "--verbose", "--output-on-failure", "--no-tests=error", "--parallel", "1", "--tests-regex", "^T014e2s$"]
    expected_native = ["python3", str(helper), "--receipt", str(cleanup_path), "--", "python3", str(root / supports["native_session.py"]["path"]), "--receipt", str(session_path), "--", *test_command]
    native, output = records["native"]
    if (native["command"] != expected_native or native.get("require_tests") is not True
            or native.get("observed_test_summary_count", 0) <= 0
            or not re.search(r"^1/1\s+Test\s+#\d+:\s*T014e2s\s+(?:\.{2,}\s+)?Passed\b", output, re.M)
            or not re.search(r"100% tests passed(?:, 0 tests failed)? out of 1\b", output)):
        raise ValueError("A57 single serial real native test did not pass")
    session = load(session_path)
    session_cleanup = load(reference(root, q["native_session_supervisor"]))
    owner_token = session.get("owner_token")
    if not isinstance(owner_token, str) or not re.fullmatch(r"[0-9a-f]{32}", owner_token):
        raise ValueError("A57 native session owner token is invalid")
    expected_runtime = str(Path("/run/user") / str(os.getuid()) / ("fn-" + owner_token[:6]))
    if session.get("runtime_directory") != expected_runtime:
        raise ValueError("A57 native runtime directory differs from its owned nonce")
    compositor = Path("/usr/bin/Hyprland")
    compositor_config = reference(root, supports["hyprland-test.lua"])
    compositor_command = session.get("compositor_command", [])
    if (len(compositor_command) != 7 or compositor_command[:6] != [str(compositor), "--config", str(compositor_config), "--socket", "ferric-test", "--wayland-fd"]
            or not re.fullmatch(r"[0-9]+", compositor_command[6]) or int(compositor_command[6]) < 3):
        raise ValueError("A57 private compositor socket handover differs")
    monitors = session.get("monitors", [])
    if (len(monitors) != 1 or monitors[0].get("name") != "ferric-virtual"
            or monitors[0].get("width") != 1920 or monitors[0].get("height") != 1080
            or monitors[0].get("scale") != 1 or monitors[0].get("dpmsStatus") is not True
            or monitors[0].get("disabled") is not False):
        raise ValueError("A57 test must use only the private active headless output")
    session_log = safe_relative(session_path.parent, session["compositor_log"])
    if (session.get("result") != "passed" or session.get("command_exit_code") != 0 or session.get("error")
            or session.get("cleanup_passed") is not True or session.get("status") != "closed"
            or session.get("runtime_removed") is not False
            or session.get("bus_command") != ["dbus-run-session", "--", *test_command]
            or session_cleanup.get("passed") is not True or session_cleanup.get("runtime_removed") is not True
            or session_cleanup.get("receipt_sha256") != q["native_session"]["sha256"]
            or session_cleanup.get("owner_token") != session.get("owner_token")
            or session_cleanup.get("runtime_directory") != session.get("runtime_directory")
            or session.get("physical_qualification") is not False or session.get("command") != test_command
            or session.get("config_errors") != []
            or session.get("compositor_config") != str(compositor_config)
            or session.get("compositor_config_sha256") != supports["hyprland-test.lua"]["sha256"]
            or session.get("compositor_isolation") != {"HYPRLAND_NO_SD_VARS":"1", "HYPRLAND_NO_SD_NOTIFY":"1", "HYPRLAND_NO_RT":"1", "LIBSEAT_BACKEND":"ferric-disabled", "AQ_DRM_DEVICES":""}
            or digest(Path("/usr/bin/hyprctl")) != session.get("control_binary_sha256")
            or digest(compositor) != session.get("compositor_sha256")
            or digest(session_log) != session.get("compositor_log_sha256")):
        raise ValueError("A57 isolated native Wayland session did not qualify and close")
    cleanup = load(cleanup_path)
    supervisor = load(reference(root, q["private_bus_supervisor"]))
    if (cleanup.get("status") != "closed" or cleanup.get("command_exit_code") != 0 or cleanup.get("wrapper_exit_code") != 0
            or cleanup.get("cancelled_signal") or cleanup.get("launch_error") or cleanup.get("output_error")
            or cleanup.get("cleanup", {}).get("passed") is not True or supervisor.get("passed") is not True
            or cleanup.get("owner_token") != session.get("owner_token")
            or supervisor.get("remaining") or supervisor.get("owner_token") != cleanup.get("owner_token")
            or supervisor.get("receipt_sha256") != q["private_bus_cleanup"]["sha256"]):
        raise ValueError("A57 private session ownership did not close cleanly")
    supplemental = q["supplemental_helper"]
    helper_gate = supplemental["gate"]
    helper_receipt_path = reference(root, helper_gate["receipt"])
    helper_receipt = load(helper_receipt_path)
    helper_invocation = load(reference(root, helper_gate["invocation"]))
    helper_closure = load(reference(root, helper_gate["closure"]))
    helper_monitor = load(reference(root, helper_gate["monitor"]))
    reference(root, helper_gate["runner"])
    provenance_path = reference(root, supplemental["provenance"])
    provenance = load(provenance_path)
    helper_log = safe_relative(helper_receipt_path.parent, helper_receipt["log"])
    test_path = "crates/ferric-browser-engine-qt/tests/public-page-host/tst_public_page_host_integration.cpp"
    if (helper_receipt.get("result") != "passed" or helper_receipt.get("exit_code") != 0
            or helper_receipt.get("timed_out") or helper_receipt.get("output_limit_exceeded")
            or helper_receipt.get("observed_failed") or helper_receipt.get("observed_skips")
            or helper_receipt.get("require_tests") is not True or helper_receipt.get("observed_test_summary_count", 0) <= 0
            or helper_receipt.get("qt_include_prefix") != qt or helper_receipt.get("qt_include_guard") != "clear"
            or any(not snapshot_matches(helper_receipt[key], source) for key in ("source_before", "source_after"))
            or helper_receipt["command"] != ["python3", str(root / supports["helper_sanitizer.py"]["path"]), "--out", str(provenance_path.parent)]
            or helper_invocation.get("stage") != "helper" or not snapshot_matches(helper_invocation["source"], source)
            or helper_invocation.get("runner_sha256") != helper_gate["runner"]["sha256"]
            or helper_invocation.get("config_sha256") != q["configuration"]["sha256"]
            or helper_invocation.get("target") != target
            or helper_closure.get("result") != "passed" or helper_closure.get("lease_released") is not True
            or helper_closure.get("monitor") != helper_monitor or helper_monitor.get("result") != "clear"
            or helper_monitor.get("pinned_qt_prefix") != qt or helper_monitor.get("artifact") != Path(target).name
            or digest(helper_log) != helper_receipt["log_sha256"]
            or "Totals: 5 passed, 0 failed, 0 skipped" not in helper_log.read_text()
            or provenance.get("candidate_file_sha256") != source["product_blobs"][test_path]
            or digest(provenance_path.parent / "waiter.cpp") != provenance.get("generated_test_sha256")
            or "-fsanitize=address,undefined" not in provenance["compiler_command"]
            or "-fno-sanitize-recover=all" not in provenance["compiler_command"]):
        raise ValueError("A57 exact-candidate sanitizer diagnostic evidence differs")
    review = load(reference(root, c["metadata"]["evidence_review"]))
    if (review.get("decision") != "accept" or not independent(review.get("reviewer"))
            or review.get("source_sha") != source["commit"] or review.get("source_tree") != source["tree"]
            or review.get("qualification") != c["metadata"]["qualification"]
            or review.get("original_native_assertions_preserved_and_passed") is not True
            or review.get("helper_regression_assertions_passed") is not True
            or not concrete_mapping(review.get("original_assertion_mapping"))
            or not concrete_mapping(review.get("helper_regression_mapping"), HELPER_CASES)
            or review.get("reviewed_execution_support") != supports
            or review.get("reviewed_gate_runners") != {gate["stage"]: gate["runner"] for gate in gates}
            or review.get("reviewed_supplemental_helper_runner") != helper_gate["runner"]
            or review.get("effective_gate_timeouts") != {stage: 5400 if stage == "engine" else 900 if stage in {"harness", "native"} else 120 for stage in STAGES}):
        raise ValueError("A57 independent actual-assertion evidence review missing")


def validate_state_A57(plan, state, root, initial=False, pack=PACK):
    if "direct_recovery_A57" not in plan:
        return []
    try:
        c = contract_A57(pack)
        if initial:
            if state != load(pack / "control/state.initial.json"):
                raise ValueError("A57 does not change the original initial template")
            return []
        historical = validate_history_A57(root, c)
        reference(root, c["metadata"]["authorization"])
        parent = state["slices"]["F02e2s"]
        old_parent = historical["slices"]["F02e2s"]
        if (object_sha(parent) != c["accepted_parent_sha256"] or parent.get("status") != "accepted"
                or parent.get("attempts") != 5 or parent.get("attempt_history") != old_parent["attempt_history"]):
            raise ValueError("A57 accepted import or unchanged five-attempt history differs")
        historical_accepted = {sid: object_sha(record) for sid, record in historical["slices"].items() if record.get("status") == "accepted"}
        if c["accepted_states_sha256"] != historical_accepted:
            raise ValueError("A57 must preserve every previously accepted slice")
        for sid, expected in c["accepted_states_sha256"].items():
            if object_sha(state["slices"].get(sid)) != expected:
                raise ValueError("A57 changed a previously accepted actual slice: " + sid)
        owners = {"F02e2s", "F02e2s-direct-recovery"}
        if recovery_open(state, str(root / c["target"])):
            raise ValueError("A57 imported recovery must remain closed")
        registry = load(root / "control/builds.json")
        if any(record.get("active") and (record.get("slice") in owners or record.get("path") == str(root / c["target"])) for record in registry["artifacts"]):
            raise ValueError("A57 recovery build remains active")
        source = c["source"]
        evidence_path = reference(root, c["metadata"]["acceptance_evidence"])
        evidence = load(evidence_path)
        review = load(reference(root, c["metadata"]["evidence_review"]))
        if (parent.get("source_sha") != source["commit"] or parent.get("source_tree") != source["tree"]
                or parent.get("evidence") != c["metadata"]["acceptance_evidence"]["path"]
                or evidence.get("source_sha") != source["commit"] or evidence.get("source_tree") != source["tree"]
                or evidence.get("qualification") != c["metadata"]["qualification"]
                or evidence.get("review_path") != c["metadata"]["evidence_review"]["path"]
                or evidence.get("review_sha256") != c["metadata"]["evidence_review"]["sha256"]
                or parent.get("reviewer") != review.get("reviewer") or evidence.get("reviewer") != review.get("reviewer")):
            raise ValueError("A57 actual parent/evidence/source/reviewer binding differs")
        repository = safe_relative(root, source["worktree"])
        if git(repository, "rev-parse", source["commit"] + "^{tree}") != source["tree"]:
            raise ValueError("A57 tested source tree differs")
        if git(repository, "merge-base", "--is-ancestor", source["base_commit"], source["commit"], check=False).returncode:
            raise ValueError("A57 source is not based on the accepted identity prerequisite")
        changed = set(git(repository, "diff", "--name-only", source["base_commit"], source["commit"]).splitlines())
        changed = {path for path in changed if not path.startswith("docs/implementation/everyday-v1/")}
        if changed != set(source["product_blobs"]):
            raise ValueError("A57 reviewed product change inventory is incomplete")
        for path, expected in source["product_blobs"].items():
            raw = subprocess.check_output(["git", "-C", str(repository), "show", source["commit"] + ":" + path])
            if hashlib.sha256(raw).hexdigest() != expected:
                raise ValueError("A57 reviewed product blob differs: " + path)
        validate_integration_A57(root, state, c)
        validate_qualification_A57(root, c)
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, subprocess.SubprocessError) as error:
        return [f"A57 state: {error}"]
    return []
