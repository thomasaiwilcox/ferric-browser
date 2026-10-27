"""Pinned A56 admission and input delivery; A55 retains its historical proof."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

from common import (PACK, load, digest, safe_relative, git, packet,
                    ATTEMPT_EXCEPTION_AMENDMENTS, ATTEMPT_EXCEPTION_LIMITS,
                    GLOBAL_MAX_ATTEMPTS, parent_attempt_lifecycle_A55,
                    parent_attempt_version_A55, parent_repair_metadata_A55)
import a55

CONTRACT_SHA256 = "af701157eba5c1c9e0ba8425507d1de9329e91f3565b3e94b0e1c29764537bfb"


def contract_A56(pack=PACK):
    path = pack / "control/test-helper-repair.A56.json"
    if digest(path) != CONTRACT_SHA256:
        raise ValueError("A56 reviewed contract changed")
    contract = load(path)
    if parent_attempt_version_A55(contract) != ("A56", 5):
        raise ValueError("A56 requires its exact immutable policy pair")
    return contract


def validate_plan_A56(plan, pack=PACK):
    if (plan.get("input_delivery_recovery_A56") is None
            and not (pack / "control/test-helper-repair.A56.json").exists()):
        return []
    try:
        c = contract_A56(pack)
        m = c["metadata"]
        if plan.get("input_delivery_recovery_A56") != m:
            raise ValueError("A56 exact policy metadata missing or changed")
        tasks = {task["id"]: task for task in plan["slices"]}
        if tasks["F02e2s"] != c["task"]:
            raise ValueError("A56 named task differs from exact reviewed scope")
        previous = copy.deepcopy(plan)
        if previous.get("direct_recovery_A57") is not None:
            from a57 import contract_A57
            if previous.pop("direct_recovery_A57") != contract_A57(pack)["metadata"]:
                raise ValueError("A57 direct recovery metadata changed")
        previous.pop("input_delivery_recovery_A56")
        previous["slices"] = [c["prior_task"] if task["id"] == "F02e2s" else task
                              for task in previous["slices"]]
        previous["attempt_exceptions"] = c["prior_attempt_exceptions"]
        if a55.object_sha(previous) != c["prior_plan_sha256"]:
            raise ValueError("A56 unrelated plan data changed")
        if plan["attempt_exceptions"]["F02e2s"] != c["attempt_exception"]:
            raise ValueError("A56 requires exactly one named fifth-attempt exception")
        if (GLOBAL_MAX_ATTEMPTS != 3
                or ATTEMPT_EXCEPTION_AMENDMENTS != dict(c["prior_policy_amendments"], F02e2s="A56")
                or ATTEMPT_EXCEPTION_LIMITS != dict(c["prior_policy_limits"], F02e2s=5)):
            raise ValueError("A56 global or unrelated attempt policy changed")
        if load(pack / "control/state.initial.json")["slices"]["F02e2s"] != c["initial_parent"]:
            raise ValueError("A56 initial snapshot must preserve failed4")
        if a55.object_sha(c["prior_parent"]) != c["prior_parent_sha256"]:
            raise ValueError("A56 frozen terminal4 parent changed")
        for name, expected in c["prior_contract_sha256"].items():
            if digest(pack / "control" / name) != expected:
                raise ValueError("A56 historical canonical contract changed")
        amendments = (pack / "control/AMENDMENTS.md").read_text()
        for tag, expected in c["prior_amendment_sections_sha256"].items():
            section = amendments.split("## " + tag, 1)[1].split("\n## ", 1)[0]
            if hashlib.sha256(section.encode()).hexdigest() != expected:
                raise ValueError("A56 historical amendment section changed")
        mapping = load(pack / "control/source-map.execution.json")
        if plan.get("direct_recovery_A57") is not None:
            from a57 import historical_source_map_A57
            mapping = historical_source_map_A57(mapping, pack)
        if [entry for entry in mapping["execution_amendments"] if entry.get("id") == "A56"] != [c["source_map"]]:
            raise ValueError("A56 exact source map changed")
        mapping["execution_amendments"] = [entry for entry in mapping["execution_amendments"]
                                            if entry.get("id") != "A56"]
        if a55.object_sha(mapping) != c["prior_source_map_sha256"]:
            raise ValueError("A56 earlier source map changed")
        section = amendments.split("## A56", 1)[1].split("\n## ", 1)[0]
        if any(m[key]["sha256"] not in section for key in ("review", "review_budget", "proposal", "user_approval")):
            raise ValueError("A56 amendment lost exact authority pins")
        if any(len(packet(task, pack).encode()) > 24000 for task in plan["slices"]):
            raise ValueError("A56 execution packet exceeds24000 bytes")
    except (OSError, ValueError, KeyError, TypeError, IndexError, RuntimeError) as error:
        return [f"A56: {error}"]
    return []


def validate_historical_state_A56(root, c):
    """Run original full checks and the new shared A55-default implementation."""
    m = c["metadata"]
    reference = m["historical_state"]
    path = safe_relative(root, reference["path"])
    if digest(path) != reference["sha256"]:
        raise ValueError("A56 immutable historical state changed")
    state = load(path)
    if state["slices"]["F02e2s"] != c["prior_parent"]:
        raise ValueError("A56 historical snapshot differs from frozen terminal4")
    repository = root / m["historical_pack_worktree_relative"]
    if (git(repository, "rev-parse", "HEAD") != m["historical_pack_commit"]
            or git(repository, "rev-parse", "HEAD^{tree}") != m["historical_pack_tree"]
            or git(repository, "status", "--porcelain")):
        raise ValueError("A56 retained A55 pack is not its exact clean source")
    pack = repository / "docs/implementation/everyday-v1"
    result = subprocess.run(["python3", str(pack / "tools/validate_pack.py"), "--state", str(path)],
                            capture_output=True, text=True, timeout=60)
    if result.returncode or "VALID: 97 slices" not in result.stdout:
        raise ValueError("A56 original historical A53/A54/A55 validation failed")
    errors = a55.validate_state_A55(load(pack / "control/plan.json"), state, root,
                                    pack=pack, contract=a55.contract_A55(pack))
    if errors:
        raise ValueError("A56 shared strict A55-default validation failed: " + "; ".join(errors))


def validate_delivery_A56(root, c, authority):
    m = c["metadata"]
    delivery = authority.get("input_delivery", {})
    report = load(safe_relative(root, authority["review"]))
    expected_paths = {"manifest": m["delivery_manifest"], "packet": m["dispatch_packet"],
                      "wrapper": m["delivery_wrapper"]["path"], "reader": m["delivery_reader"]["path"]}
    if set(delivery) != set(expected_paths) or report.get("input_delivery") != delivery:
        raise ValueError("A56 accepted pack review lacks fixed exact delivery evidence")
    for key, name in expected_paths.items():
        reference = delivery[key]
        if reference.get("path") != name or digest(safe_relative(root, name)) != reference.get("sha256"):
            raise ValueError("A56 fixed input delivery file missing or changed")
    if delivery["wrapper"] != m["delivery_wrapper"] or delivery["reader"] != m["delivery_reader"]:
        raise ValueError("A56 independently accepted wrapper/reader changed")
    packet_path = safe_relative(root, m["dispatch_packet"])
    if not 0 < packet_path.stat().st_size <= 24000:
        raise ValueError("A56 complete assigned packet exceeds limit")
    specification = importlib.util.spec_from_file_location("a56_reviewed_delivery", safe_relative(root, m["delivery_wrapper"]["path"]))
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    manifest_path = safe_relative(root, m["delivery_manifest"])
    manifest, chunks, reader = module.package(manifest_path.parent, delivery["manifest"]["sha256"])
    if str(reader) != str(safe_relative(root, m["delivery_reader"]["path"])):
        raise ValueError("A56 bundle byte reader differs from reviewed reader")
    first = manifest["views"][0]
    if (first.get("name") != "approved_dispatch_packet" or first["bytes"] != packet_path.stat().st_size
            or first["sha256"] != delivery["packet"]["sha256"]):
        raise ValueError("A56 bundle does not contain the complete approved dispatch packet")
    if manifest.get("scope") != "A56_F02e2s_source5" or manifest.get("outer_output_tokens") != 4096 or manifest.get("nested_output_tokens") != 4096:
        raise ValueError("A56 source scope or outer/nested output allowance differs")
    return delivery, manifest, chunks


def validate_pack_authority_A56(root, parent, c):
    authority = a55.validate_pack_authority_A55(root, parent, c)
    validate_delivery_A56(root, c, authority)
    return authority


def validate_worker_delivery_A56(root, parent, c, require_complete=False):
    m = c["metadata"]
    authority = validate_pack_authority_A56(root, parent, c)
    delivery, manifest, chunks = validate_delivery_A56(root, c, authority)
    assignment = load(safe_relative(root, m["source_assignment"]))
    if assignment.get("input_delivery") != delivery:
        raise ValueError("A56 immutable assignment lacks exact accepted delivery refs")
    ledger = load(safe_relative(root, m["source_read_log"]))
    if ledger.get("assignment_sha256") != parent["assignment_sha256"] or ledger.get("bundle_manifest_sha256") != delivery["manifest"]["sha256"]:
        raise ValueError("A56 source ledger lost immutable assignment/bundle binding")
    if require_complete:
        if ledger["source_bytes_emitted"] != manifest["total_bytes"] or len(ledger["reads"]) != len(chunks):
            raise ValueError("A56 source input delivery is incomplete or has extra unapproved reads")
        for index, read in enumerate(ledger["reads"], 1):
            if read["bytes"] != len(chunks[index]) or read["sha256"] != hashlib.sha256(chunks[index]).hexdigest():
                raise ValueError("A56 actual charged input differs from approved chunk")
    return delivery


def validate_state_A56(plan, state, root, initial=False, pack=PACK):
    if plan.get("input_delivery_recovery_A56") is None:
        return []
    try:
        c = contract_A56(pack)
        m = c["metadata"]
        if initial:
            return a55.validate_state_A55(plan, state, root, True, pack, contract=c)
        validate_historical_state_A56(root, c)
        for key in ("user_approval", "delivery_review", "delivery_wrapper", "delivery_reader"):
            reference = m[key]
            if digest(safe_relative(root, reference["path"])) != reference["sha256"]:
                raise ValueError("A56 user/delivery prerequisite changed")
        parent = state["slices"]["F02e2s"]
        errors = a55.validate_state_A55(plan, state, root, False, pack, contract=c)
        if errors:
            return errors
        if parent["attempts"] == 4:
            frozen = copy.deepcopy(parent)
            for key in ("source_repair_A56_authority", "source_repair_A56"):
                frozen.pop(key, None)
            for key in ("status", "blocker", "attempt_budget", "test_helper_repair_amendment"):
                frozen[key] = c["prior_parent"][key]
            if frozen != c["prior_parent"]:
                raise ValueError("A56 ready4 changed unrelated frozen terminal4 fields")
            if parent["status"] == "ready":
                if parent["attempt_budget"] != 5 or parent["test_helper_repair_amendment"] != "A56":
                    raise ValueError("A56 ready4 must explicitly name the one new policy")
                validate_pack_authority_A56(root, parent, c)
            return []
        authority = validate_pack_authority_A56(root, parent, c)
        delivery, manifest, chunks = validate_delivery_A56(root, c, authority)
        events = parent_attempt_lifecycle_A55(root, parent, c)
        frozen = any(event["phase"] == "source_review_started" for event in events)
        validate_worker_delivery_A56(root, parent, c, require_complete=frozen)
        if frozen and load(safe_relative(root, m["source_freeze"])).get("input_delivery") != delivery:
            raise ValueError("A56 witnessed source freeze lost delivery references")
    except (OSError, ValueError, KeyError, TypeError, IndexError, AttributeError, RuntimeError, subprocess.SubprocessError) as error:
        return [f"A56 state: {error}"]
    return []


def validate_acceptance_A56(plan, task, record, root, pack=PACK):
    if plan.get("input_delivery_recovery_A56") is None or task.get("id") != "F02e2s":
        return []
    return a55.validate_acceptance_A55(plan, task, record, root, pack, contract_A56(pack))
