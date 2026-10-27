"""Exact A55 parent repair overlay. No execution or implicit state mutation."""
import hashlib
import json
import subprocess
from pathlib import Path
from common import (PACK, load, digest, safe_relative, git, packet,
                    ATTEMPT_EXCEPTION_AMENDMENTS, ATTEMPT_EXCEPTION_LIMITS, GLOBAL_MAX_ATTEMPTS,
                    parent_attempt_lifecycle_A55, parent_attempt_phases_A55,
                    parent_attempt_failure_artifacts_A55, parent_source_artifacts_A55, parent_worker_inputs_A55, identity_review_decision_A54)

CONTRACT_SHA256 = "1dc6bdf60fc9ad9a9994ff23dc87a3234256e9d80e14d5252b56b02d4ceaa585"


NATIVE_POLICY_KEYS = ["slice", "amendment", "attempt", "attempt_limit", "source_sha", "source_tree", "worktree", "branch",
                      "pack_worktree", "pack_commit", "pack_tree", "target", "artifact", "cargo_target_dir", "cmake_dir",
                      "qt_prefix", "kf_prefix", "kf_library", "environment", "gates", "private_bus_helper",
                      "private_bus_helper_sha256", "private_bus_receipt"]
HELPER_CASES = {"timeout_before_completion", "late_completion_while_fresh_waiter", "queued_current_completion_42", "synchronous_completion_7"}


def object_sha(data):
    return hashlib.sha256(json.dumps(data, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def contract_A55(pack=PACK):
    path = pack / "control/test-helper-repair.A55.json"
    if digest(path) != CONTRACT_SHA256:
        raise ValueError("A55 independently reviewed repair contract changed")
    return load(path)


def validate_plan_A55(plan, pack=PACK):
    if plan.get("test_helper_repair_A55") is None and not (pack / "control/test-helper-repair.A55.json").exists():
        return []
    try:
        c = contract_A55(pack)
        m = c["metadata"]
        if plan.get("test_helper_repair_A55") != m:
            raise ValueError("exact A55 policy/review/patch metadata differs")
        if plan.get("limits") != c["prior_limits"] or any(type(plan["limits"][key]) is not type(value) for key, value in c["prior_limits"].items()):
            raise ValueError("global/prior resource and packet limits changed")
        tasks = {t["id"]: t for t in plan["slices"]}
        if tasks.get("F02e2s") != c["task"]:
            raise ValueError("parent scope/continuation/cap differs from A55")
        if object_sha([t for t in plan["slices"] if t["id"] != "F02e2s"]) != c["prior_other_tasks_sha256"]:
            raise ValueError("unrelated task changed")
        expected = dict(c["prior_attempt_exceptions"], F02e2s={
            "amendment": "A55", "default_limit": 3, "authorized_limit": 4,
            "reason": "One exact test-only lifetime repair after preserved failed3; no reset/retry/source5, unchanged native qualification."})
        if plan.get("attempt_exceptions") != expected:
            raise ValueError("only the named A55 exception may be added")
        if (GLOBAL_MAX_ATTEMPTS != 3 or ATTEMPT_EXCEPTION_AMENDMENTS != dict(c["prior_policy_amendments"], F02e2s="A55")
                or ATTEMPT_EXCEPTION_LIMITS != dict(c["prior_policy_limits"], F02e2s=4)):
            raise ValueError("global/prior named attempt limits changed")
        if digest(pack / "control/test_matrix.json") != c["prior_test_matrix_sha256"]:
            raise ValueError("test inventory/cases/thresholds changed")
        initial = load(pack / "control/state.initial.json")
        if initial["slices"].get("F02e2s") != c["initial_parent"]:
            raise ValueError("initial parent must retain exact failed3")
        remainder = dict(initial, slices={k: v for k, v in initial["slices"].items() if k != "F02e2s"})
        if object_sha(remainder) != c["prior_initial_remainder_sha256"]:
            raise ValueError("unrelated initial state changed")
        for name, expected_hash in c["prior_contract_sha256"].items():
            if digest(pack / "control" / name) != expected_hash:
                raise ValueError("A53/A54 canonical contract changed")
        amendments = (pack / "control/AMENDMENTS.md").read_text()
        for tag, expected_hash in c["prior_amendment_sections_sha256"].items():
            section = amendments.split("## " + tag, 1)[1].split("\n## ", 1)[0]
            if hashlib.sha256(section.encode()).hexdigest() != expected_hash:
                raise ValueError("A53/A54 amendment section changed")
        source = load(pack / "control/source-map.execution.json")
        if [x for x in source["execution_amendments"] if x.get("id") == "A55"] != [c["source_map"]]:
            raise ValueError("exact A55 source map differs")
        source["execution_amendments"] = [x for x in source["execution_amendments"] if x.get("id") != "A55"]
        if object_sha(source) != c["prior_source_map_sha256"]:
            raise ValueError("prior source-map sections changed")
        section = amendments.split("## A55", 1)[1].split("\n## ", 1)[0]
        if any(m[k]["sha256"] not in section for k in ("review", "review_budget", "proposal", "repair_patch")):
            raise ValueError("A55 amendment lost pinned independent approval")
        if any(len(packet(t, pack).encode()) > 24000 for t in plan["slices"]):
            raise ValueError("execution packet exceeds24000 bytes")
    except (OSError, ValueError, TypeError, KeyError, IndexError) as error:
        return [f"A55: {error}"]
    return []


def validate_review_A55(root, path, expected_sha, source, tree, author):
    report_path = safe_relative(root, path)
    if digest(report_path) != expected_sha:
        raise ValueError("fixed A55 review digest changed")
    review = load(report_path)
    candidate = review.get("candidate", {})
    actual = review.get("source_commit", review.get("source_sha", candidate.get("commit")))
    actual_tree = review.get("source_tree", candidate.get("tree"))
    if (identity_review_decision_A54(review) != "accept" or not review.get("reviewer")
            or review["reviewer"] in {author, "/root"} or actual != source or actual_tree != tree
            or candidate.get("slice") != "F02e2s" or candidate.get("amendment") != "A55"
            or type(candidate.get("attempt")) is not int or candidate["attempt"] != 4):
        raise ValueError("A55 review does not independently accept the exact source4")
    return review


def validate_product_A55(repository, source, c, integrated=False, authority=None):
    m = c["metadata"]
    expected = dict(m["unchanged_product_sha256"], **{m["test_path"]: m["test_after_sha256"]})
    if not isinstance(source, str) or len(source) != 40 or any(x not in "0123456789abcdef" for x in source):
        raise ValueError("A55 actual source SHA missing")
    for path, wanted in expected.items():
        raw = subprocess.run(["git", "-C", str(repository), "show", f"{source}:{path}"], capture_output=True)
        if raw.returncode or hashlib.sha256(raw.stdout).hexdigest() != wanted:
            raise ValueError(f"A55 pinned product content changed: {path}")
    changed = git(repository, "diff", "--name-only", m["source_base_sha"], source).splitlines()
    product = [p for p in changed if not p.startswith("docs/implementation/everyday-v1/")]
    if set(product) != set(c["task"]["allowed_paths"]):
        raise ValueError("A55 source scope differs from exact ten-file reapply")
    parent = git(repository, "rev-parse", f"{source}^")
    if integrated:
        if not authority or parent != authority["pack_commit"]:
            raise ValueError("A55 integration must cherry-pick onto exact accepted pack")
        namespace = "docs/implementation/everyday-v1"
        if git(repository, "rev-parse", f"{source}:{namespace}") != git(repository, "rev-parse", f'{authority["pack_commit"]}:{namespace}'):
            raise ValueError("integrated pack content changed")
    elif parent != m["source_base_sha"] or set(changed) != set(c["task"]["allowed_paths"]):
        raise ValueError("A55 worker must use accepted child as sole parent, no failed candidate base or unrelated edits")
    if git(repository, "rev-parse", "HEAD") != source or git(repository, "status", "--porcelain"):
        raise ValueError("A55 source worktree changed after freeze")


def validate_pack_authority_A55(root, parent, c):
    m = c["metadata"]
    authority = parent.get("source_repair_A55_authority", {})
    if authority.get("review") != m["pack_implementation_review"]:
        raise ValueError("A55 readiness requires its fixed final independent pack review")
    path = safe_relative(root, authority["review"])
    report = load(path)
    candidate = report.get("candidate", {})
    if (digest(path) != authority.get("review_sha256") or report.get("decision") != "accept"
            or not report.get("reviewer") or report["reviewer"] == "/root"
            or candidate.get("commit") != authority.get("pack_commit") or candidate.get("tree") != authority.get("pack_tree")):
        raise ValueError("A55 exact accepted pack authority mismatch")
    repository = Path(authority["pack_worktree"])
    if (not repository.resolve().is_relative_to((root / "worktrees").resolve())
            or git(repository, "rev-parse", "HEAD") != authority["pack_commit"]
            or git(repository, "rev-parse", "HEAD^{tree}") != authority["pack_tree"]
            or git(repository, "status", "--porcelain")):
        raise ValueError("A55 accepted pack is not the actual clean retained source")
    raw = subprocess.run(["git", "-C", str(repository), "show", authority["pack_commit"] + ":docs/implementation/everyday-v1/control/test-helper-repair.A55.json"], capture_output=True)
    if raw.returncode or hashlib.sha256(raw.stdout).hexdigest() != CONTRACT_SHA256:
        raise ValueError("A55 pack authority does not contain the exact reviewed repair contract")
    if git(repository, "merge-base", "--is-ancestor", m["source_base_sha"], authority["pack_commit"], check=False).returncode:
        raise ValueError("A55 accepted pack must descend from the qualified child base")
    checks = authority.get("pack_checks", {})
    if set(checks) != {"tool_tests", "pack_validation", "checksums"} or report.get("pack_checks") != checks:
        raise ValueError("A55 pack readiness lacks independently bound actual checks")
    for name, reference in checks.items():
        receipt_path = safe_relative(root, reference["path"])
        if not receipt_path.is_relative_to((root / "evidence/A55").resolve()) or digest(receipt_path) != reference["sha256"]:
            raise ValueError("A55 pack-check receipt missing or replaced")
        receipt = load(receipt_path)
        tools = repository / "docs/implementation/everyday-v1/tools"
        commands = {"tool_tests": ["python3", str(tools / "test_tools.py")],
                    "pack_validation": ["python3", str(tools / "validate_pack.py"), "--state", str(root / "control/state-A55-preview.json")],
                    "checksums": ["sha256sum", "--check", str(repository / "docs/implementation/everyday-v1/CHECKSUMS.sha256")]}
        if receipt.get("command") != commands[name]:
            raise ValueError("A55 pack check did not run the complete required command")
        if (receipt.get("result") != "passed" or receipt.get("exit_code") != 0 or receipt.get("timed_out")
                or receipt.get("output_limit_exceeded") or receipt.get("observed_failed", 0) != 0 or receipt.get("observed_skips", 0) != 0):
            raise ValueError("A55 pack checks did not pass")
        for key in ("source_before", "source_after"):
            snapshot = receipt.get(key, {})
            if snapshot.get("sha") != authority["pack_commit"] or snapshot.get("tree") != authority["pack_tree"] or snapshot.get("dirty") is not False:
                raise ValueError("A55 check did not run on exact clean pack")
        if digest(safe_relative(receipt_path.parent, receipt["log"])) != receipt["log_sha256"]:
            raise ValueError("A55 check log changed")
        if name == "tool_tests" and (receipt.get("require_tests") is not True or receipt.get("observed_test_summary_count", 0) < 96):
            raise ValueError("A55 tooling tests did not execute")
    return authority


def validate_worker_evidence_A55(root, parent, c):
    expected = parent_worker_inputs_A55(root, parent, c)
    frozen = load(safe_relative(root, c["metadata"]["source_freeze"])).get("worker_evidence", {})
    if frozen != expected:
        raise ValueError("A55 worker evidence changed after immutable source freeze")


def validate_native_binding_A55(root, parent, c, events, authority):
    m = c["metadata"]
    binding = load(safe_relative(root, m["manifest_binding"]))
    manifest_path = safe_relative(root, m["native_command_manifest"])
    reference = {"path": m["native_command_manifest"], "sha256": digest(manifest_path)}
    if binding.get("manifest") != reference or parent.get("native_command_manifest") != reference["path"] or parent.get("native_command_manifest_sha256") != reference["sha256"]:
        raise ValueError("A55 final native manifest changed after its one-time freeze")
    manifest = load(manifest_path)
    target = str(root / m["target_relative"])
    expected = {"slice": "F02e2s", "amendment": "A55", "attempt": 4, "attempt_limit": 4,
                "source_sha": parent["source_sha"], "source_tree": parent["source_tree"], "worktree": parent["worktree"],
                "base_sha": m["source_base_sha"], "base_tree": m["source_base_tree"],
                "pack_commit": authority["pack_commit"], "pack_tree": authority["pack_tree"], "pack_worktree": authority["pack_worktree"],
                "target": target, "artifact": Path(target).name, "cargo_target_dir": target + "/cargo-target", "cmake_dir": target + "/cmake",
                "private_bus_helper": m["private_bus_helper"]["path"], "private_bus_helper_sha256": m["private_bus_helper"]["sha256"],
                "private_bus_receipt": m["private_bus_cleanup"]}
    if any(manifest.get(k) != v for k, v in expected.items()):
        raise ValueError("A55 native manifest owner/source/base/target/helper mismatch")
    if manifest.get("environment", {}).get("CARGO_TARGET_DIR") != expected["cargo_target_dir"]:
        raise ValueError("A55 native environment target mismatch")
    gates = manifest.get("gates", [])
    if [g.get("name") for g in gates] != m["native_gate_sequence"]:
        raise ValueError("A55 native gate order changed")
    engine = ["cargo", "+stable", "build", "--package", "ferric-browser-engine-qt", "--release", "--locked", "--offline",
              "--jobs", "2", "--verbose", "--target-dir", expected["cargo_target_dir"]]
    serial = ["python3", str(root / m["private_bus_helper"]["path"]), "--receipt", str(root / m["private_bus_cleanup"]), "--",
              "dbus-run-session", "--", "ctest", "--test-dir", expected["cmake_dir"], "--verbose", "--output-on-failure",
              "--no-tests=error", "--parallel", "1", "--tests-regex", "^T014e2s$"]
    if gates[1].get("command") != engine or gates[1].get("timeout_seconds") != 5400 or gates[6].get("command") != serial:
        raise ValueError("A55 exact release/offline/two-job build or isolated verbose single test changed")
    policy = object_sha({k: manifest[k] for k in NATIVE_POLICY_KEYS})
    frozen = load(safe_relative(root, m["native_policy_freeze"]))
    review = load(safe_relative(root, m["native_runner_review"]))
    if policy != manifest.get("command_policy_sha256") or policy != frozen.get("command_policy_sha256") or policy != review.get("command_policy_sha256"):
        raise ValueError("A55 exact independently reviewed native command policy changed")
    for key, file_key in (("coordinator_sha256", "native_coordinator"), ("discovery_script_sha256", "discovery_script")):
        actual = digest(safe_relative(root, m[file_key]))
        if actual != manifest.get(key) or actual != frozen.get(key) or actual != review.get(key):
            raise ValueError("A55 exact reviewed runner/discovery source changed")
    if review.get("private_bus_helper_sha256") != m["private_bus_helper"]["sha256"] or digest(safe_relative(root, m["private_bus_helper"]["path"])) != m["private_bus_helper"]["sha256"]:
        raise ValueError("A55 accepted private bus helper changed")
    tools = Path(authority["pack_worktree"]) / "docs/implementation/everyday-v1/tools"
    if digest(tools / "run_gate.py") != manifest.get("run_gate_sha256") or digest(tools / "qt_include_guard.py") != manifest.get("include_guard_sha256"):
        raise ValueError("A55 owned gate/include guard changed")
    for event in events:
        if event["phase"] in {f"gate_{i}_passed" for i in (1, 2, 3, 5, 6)}:
            index = int(event["phase"][5])
            receipt = load(safe_relative(root, m["native_gate_receipts"][index]))
            if receipt.get("command") != gates[index].get("command"):
                raise ValueError("A55 executed command differs from reviewed one-time manifest")


def validate_state_A55(plan, state, root, initial=False, pack=PACK):
    if plan.get("test_helper_repair_A55") is None:
        return []
    try:
        c = contract_A55(pack)
        m = c["metadata"]
        parent = state["slices"]["F02e2s"]
        if initial:
            if parent != c["initial_parent"]:
                raise ValueError("initial A55 snapshot cannot reset failed3 or assign4")
            return []
        if object_sha(state["slices"]["F02e2i"]) != m["accepted_child"]["state_record_sha256"]:
            raise ValueError("qualified accepted child state changed")
        for slice_id, expected_hash in c["accepted_states_sha256"].items():
            if object_sha(state["slices"].get(slice_id)) != expected_hash:
                raise ValueError("A55 previously accepted slice changed")
        for number, record in c["history"].items():
            if object_sha(parent.get("attempt_history", {}).get(number)) != record["state_record_sha256"]:
                raise ValueError(f"A55 failed source/native history{number} changed")
            for reference in record["files"].values():
                if digest(safe_relative(root, reference["path"])) != reference["sha256"]:
                    raise ValueError("A55 prior failed artifact changed")
        for reference in list(c["retained_external_artifacts"].values()) + [m[k] for k in ("review", "review_budget", "review_clarification", "proposal", "repair_patch")]:
            if digest(safe_relative(root, reference["path"])) != reference["sha256"]:
                raise ValueError("A55 approval/failure/budget/patch artifact changed")
        attempts = parent.get("attempts")
        if type(attempts) is not int or attempts not in (3, 4):
            raise ValueError("A55 retains failed3 then records exactly4; no reset or5")
        assignment_path = safe_relative(root, m["source4_assignment"])
        terminal_path = safe_relative(root, m["terminal_marker"])
        failures = parent_attempt_failure_artifacts_A55(root, c)
        if failures and (attempts != 4 or parent.get("status") != "failed" or not terminal_path.exists()):
            raise ValueError("A55 any failed/incomplete fixed source/review/gate is permanently terminal4")
        if attempts == 3:
            if type(parent.get("verified_source_read_bytes")) is not int or parent["verified_source_read_bytes"] != m["worker_source3_budget"]:
                raise ValueError("A55 failed3 source budget cannot reset")
            if (parent.get("status") not in {"failed", "ready"} or assignment_path.exists() or terminal_path.exists()
                    or safe_relative(root, m["lifecycle"]).exists() or safe_relative(root, m["audit_repository"]).exists()
                    or parent.get("source_changes_authorized") is True or parent.get("compile_test_authorized") is True
                    or parent.get("native_work_authorized") is True):
                raise ValueError("A55 failed3 cannot restart, authorize work or hide assigned4")
            if parent["status"] == "ready":
                validate_pack_authority_A55(root, parent, c)
            return []
        if parent.get("status") in {"ready", "not_started", "blocked_specification"}:
            raise ValueError("A55 assigned4 is not a dispatchable retry")
        authority = validate_pack_authority_A55(root, parent, c)
        if parent.get("assignment") != m["source4_assignment"] or digest(assignment_path) != parent.get("assignment_sha256"):
            raise ValueError("A55 sole assignment missing/replaced/alternate")
        assignment = load(assignment_path)
        if any(type(assignment.get(key)) is not int for key in ("attempt", "attempt_limit", "source_soft_limit", "source_hard_limit")):
            raise ValueError("A55 assignment counters/limits must be exact integers")
        required = {"slice": "F02e2s", "amendment": "A55", "attempt": 4, "attempt_limit": 4,
                    "base_sha": m["source_base_sha"], "base_tree": m["source_base_tree"],
                    "reapply_source_sha": m["reapply_source_sha"], "reapply_source_tree": m["reapply_source_tree"],
                    "repair_patch_sha256": m["repair_patch"]["sha256"], "test_after_sha256": m["test_after_sha256"],
                    "pack_commit": authority["pack_commit"], "pack_tree": authority["pack_tree"],
                    "worker": parent.get("author"), "worktree": str(root / m["source4_worktree_relative"]),
                    "target": str(root / m["target_relative"]), "source_soft_limit": 48000, "source_hard_limit": 96000}
        if any(assignment.get(k) != v for k, v in required.items()) or assignment.get("allowed_paths") != c["task"]["allowed_paths"]:
            raise ValueError("A55 assignment changes owner/base/reapply/pins/authority/target/budget/scope")
        if not assignment.get("worker") or assignment["worker"] == "/root":
            raise ValueError("A55 source actor must be the recorded bounded worker")
        packet_path = safe_relative(root, assignment["packet"])
        if digest(packet_path) != assignment.get("packet_sha256") or len(packet_path.read_bytes()) > 24000:
            raise ValueError("A55 assigned packet changed or exceeds24000")
        events = parent_attempt_lifecycle_A55(root, parent, c)
        phase = events[-1]["phase"]
        if terminal_path.exists():
            terminal = load(terminal_path)
            if (terminal.get("slice") != "F02e2s" or terminal.get("attempt") != 4 or terminal.get("result") != "failed"
                    or terminal.get("no_retry") is not True or parent.get("status") != "failed" or phase != "terminal_failed"):
                raise ValueError("A55 terminal outcome cannot restart/relabel/accept")
            retained = terminal.get("failed_artifacts", {})
            if any(retained.get(k) != v for k, v in failures.items()):
                raise ValueError("A55 failed artifacts must remain archived")
            for reference in retained.values():
                if digest(safe_relative(root, reference["path"])) != reference["sha256"]:
                    raise ValueError("A55 terminal artifact deleted or changed")
            source_artifacts = terminal.get("source_artifacts", {})
            if any(source_artifacts.get(key) != value for key, value in parent_source_artifacts_A55(root, c).items()):
                raise ValueError("A55 terminal source inventory changed")
            for reference in source_artifacts.values():
                if digest(safe_relative(root, reference["path"])) != reference["sha256"]:
                    raise ValueError("A55 terminal source input/payload was deleted or changed")
        elif phase == "terminal_failed" or parent.get("status") == "failed":
            raise ValueError("A55 failed4 requires permanent terminal marker/history")
        if (parent.get("status") == "accepted") != (phase == "accepted"):
            raise ValueError("A55 acceptance requires full ordered lifecycle")
        if parent.get("source_changes_authorized") is True and phase != "assigned":
            raise ValueError("A55 source freezes at review start")
        for name in ("source_review", "integrated_source_review", "native_runner_review", "gate_evidence_review"):
            if parent.get(name) is not None and parent[name] != m[name]:
                raise ValueError("A55 alternate referenced review forbidden")
        phases = parent_attempt_phases_A55()
        index = phases.index(phase) if phase in phases else -1
        if index >= phases.index("source_review_started"):
            validate_worker_evidence_A55(root, parent, c)
            validate_product_A55(Path(assignment["worktree"]), parent.get("implementation_sha"), c)
            if git(Path(assignment["worktree"]), "rev-parse", "HEAD^{tree}") != parent.get("implementation_tree"):
                raise ValueError("A55 worker tree differs from frozen source")
        if index >= phases.index("source_review_passed"):
            validate_review_A55(root, m["source_review"], parent.get("source_review_sha256"), parent.get("implementation_sha"), parent.get("implementation_tree"), parent["author"])
        if index >= phases.index("integrated_review_started"):
            if Path(parent["worktree"]) != root / m["integrated_worktree_relative"]:
                raise ValueError("A55 integrated worktree must be fixed")
            validate_product_A55(Path(parent["worktree"]), parent.get("source_sha"), c, True, authority)
            if git(Path(parent["worktree"]), "rev-parse", "HEAD^{tree}") != parent.get("source_tree"):
                raise ValueError("A55 integrated tree differs from source")
        if index >= phases.index("integrated_review_passed"):
            validate_review_A55(root, m["integrated_source_review"], parent.get("integrated_source_review_sha256"), parent["source_sha"], parent["source_tree"], parent["author"])
        if index >= phases.index("runner_review_passed"):
            validate_review_A55(root, m["native_runner_review"], parent.get("native_runner_review_sha256"), parent["source_sha"], parent["source_tree"], parent["author"])
        if index >= phases.index("gate_0_started"):
            validate_native_binding_A55(root, parent, c, events, authority)
        if parent.get("native_work_authorized") is True or parent.get("compile_test_authorized") is True:
            if not phases.index("runner_review_passed") <= index <= phases.index("gate_6_started"):
                raise ValueError("A55 native/compiler authority requires accepted exact reviews and unused fixed gates")
        if phase == "terminal_failed":
            if parent.get("source_changes_authorized") is True or parent.get("native_work_authorized") is True or parent.get("compile_test_authorized") is True:
                raise ValueError("A55 terminal failure authorizes no source/native correction")
            if "F02e2s" in state.get("active", []) or any(x.get("slice") == "F02e2s" for x in state.get("active_locks", {}).values()):
                raise ValueError("A55 terminal4 must release assignment locks")
    except (OSError, ValueError, TypeError, KeyError, IndexError, AttributeError, RuntimeError) as error:
        return [f"A55 state: {error}"]
    return []


def validate_acceptance_A55(plan, task, record, root, pack=PACK):
    if plan.get("test_helper_repair_A55") is None or task.get("id") != "F02e2s" or record.get("status") != "accepted":
        return []
    try:
        c = contract_A55(pack)
        m = c["metadata"]
        evidence = load(safe_relative(root, record["evidence"]))
        if record.get("attempts") != 4 or evidence.get("gate_evidence_review") != m["gate_evidence_review"] or evidence.get("review_path") != m["gate_evidence_review"]:
            raise ValueError("A55 accepts only its exact source4 and fixed gate review")
        if evidence.get("native_run") != m["native_run"] or evidence.get("lease_closure") != m["native_closure"]:
            raise ValueError("A55 fixed native run/closure required")
        run = load(safe_relative(root, m["native_run"]))
        if [g.get("receipt") for g in run.get("gates", [])] != m["native_gate_receipts"]:
            raise ValueError("A55 native gates require fixed one-time receipt paths")
        if run.get("command_manifest") != m["native_command_manifest"]:
            raise ValueError("A55 fixed final native manifest required")
        manifest_path = safe_relative(root, m["native_command_manifest"])
        manifest = load(manifest_path)
        if digest(manifest_path) != run.get("command_manifest_sha256"):
            raise ValueError("A55 native manifest replaced")
        target = str(root / m["target_relative"])
        if manifest.get("target") != target or manifest.get("gates", [])[1].get("timeout_seconds") != 5400:
            raise ValueError("A55 exact fresh target/engine supervisor required")
        serial = manifest["gates"][-1]["command"]
        helper = str(root / m["private_bus_helper"]["path"])
        cleanup_path = str(root / m["private_bus_cleanup"])
        if serial[:8] != ["python3", helper, "--receipt", cleanup_path, "--", "dbus-run-session", "--", "ctest"] or "--verbose" not in serial:
            raise ValueError("A55 requires exact R5 private bus wrapper and verbose isolated CTest")
        if digest(Path(helper)) != m["private_bus_helper"]["sha256"]:
            raise ValueError("A55 accepted private bus helper changed")
        cleanup = load(Path(cleanup_path))
        if (cleanup.get("status") != "closed" or cleanup.get("command_exit_code") != 0 or cleanup.get("wrapper_exit_code") != 0
                or cleanup.get("cancelled_signal") is not None or cleanup.get("launch_error") or cleanup.get("output_error")
                or cleanup.get("cleanup", {}).get("passed") is not True):
            raise ValueError("A55 private helper cleanup did not pass")
        review = validate_review_A55(root, m["gate_evidence_review"], evidence["gate_evidence_review_sha256"], record["source_sha"], record["source_tree"], record["author"])
        if review.get("helper_regression_assertions_passed") is not True or review.get("original_native_assertions_preserved_and_passed") is not True:
            raise ValueError("A55 requires independent actual helper and original-assertion mapping")
        mapping = review.get("helper_regression_mapping", {})
        original = review.get("original_assertion_mapping", {})
        if set(mapping) != HELPER_CASES or any(not isinstance(v, str) or not v.strip() for v in mapping.values()) or not isinstance(original, dict) or not original:
            raise ValueError("A55 gate review lacks concrete source-to-executed-assertion mappings")
    except (OSError, ValueError, TypeError, KeyError, IndexError, AttributeError, RuntimeError) as error:
        return [f"F02e2s A55: {error}"]
    return []
