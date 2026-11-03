"""Isolated validator regressions; synthetic fixtures never qualify native code."""
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import a57


class QualificationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="a57-validator-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.compositor = self.root / "synthetic-system/Hyprland"
        self.compositor_control = self.root / "synthetic-system/hyprctl"
        self.put(self.compositor, "synthetic compositor; never executed\n")
        self.put(self.compositor_control, "synthetic compositor control; never executed\n")
        binaries = {Path("/usr/bin/Hyprland"): self.compositor, Path("/usr/bin/hyprctl"): self.compositor_control}
        real_digest = a57.digest
        compositor_digest = patch.object(a57, "digest", side_effect=lambda path: real_digest(
            binaries.get(Path(path), path)))
        compositor_digest.start()
        self.addCleanup(compositor_digest.stop)
        self.source = {"commit": "a" * 40, "tree": "b" * 40}
        self.snapshot = {"sha": self.source["commit"], "tree": self.source["tree"], "dirty": False}
        self.target = self.root / "builds/native-recovery"
        qt, kf = self.root / "sdk/qt", self.root / "sdk/libKF6WindowSystem.so"
        self.put(kf, "synthetic KF artifact")
        config = {"target": str(self.target), "cargo_target": str(self.target / "cargo-target"), "cmake_dir": str(self.target / "cmake"),
                  "qt_prefix": str(qt), "kf_library": str(kf), "environment": {"CARGO_TARGET_DIR": str(self.target / "cargo-target")}}
        supports = {name: self.put("support/" + name, "# synthetic fixture\n") for name in
                    ("run.py", "recovery_gate.py", "run_gate.py", "common.py", "qt_include_guard.py", "watch_qt_includes.py", "verify_link.py", "helper_sanitizer.py", "native_session.py", "hyprland-test.lua")}
        config_ref = self.put("CONFIG.json", config)
        self.exe = self.target / "cmake/public_page_host_integration_tests"
        self.archive = self.target / "cargo-target/release/libferric_browser_engine_qt.a"
        self.link_file = self.target / "cmake/CMakeFiles/public_page_host_integration_tests.dir/link.txt"
        self.put(self.exe, "synthetic executable; never executed")
        self.exe.chmod(0o755)
        self.put(self.archive, "synthetic archive")
        self.put(self.link_file, f"{self.archive} {kf}\n")
        ldd = self.put("gates/link/link-runtime-ldd.log", f"libKF6WindowSystem.so => {kf}\nlibQt6Core.so => {qt}/lib/libQt6Core.so\n")
        link = dict(result="passed", source_sha=self.source["commit"], source_tree=self.source["tree"], target=str(self.target),
                    engine_archive_before_kf6windowsystem=True, matching_qt_and_kf_runtime=True,
                    executable_sha256=a57.digest(self.exe), engine_archive_sha256=a57.digest(self.archive),
                    link_command_sha256=a57.digest(self.link_file), ldd_log_sha256=ldd["sha256"])
        link_ref = self.put("gates/link/LINK.json", link)
        helper = self.put("support/private_bus.py", "# synthetic helper\n")
        owner = "0123456789abcdef0123456789abcdef"
        cleanup = self.put("gates/native/cleanup.json", {"status": "closed", "command_exit_code": 0, "wrapper_exit_code": 0,
                                                       "cleanup": {"passed": True}, "owner_token": owner})
        supervisor = self.put("gates/native/supervisor.json", {"passed": True, "remaining": [], "owner_token": owner, "receipt_sha256": cleanup["sha256"]})
        test_command = ["ctest", "--test-dir", config["cmake_dir"], "--verbose", "--output-on-failure", "--no-tests=error", "--parallel", "1", "--tests-regex", "^T014e2s$"]
        self.session_log = self.root / "gates/native/compositor.log"
        session_log = self.put(self.session_log, "synthetic headless compositor log\n")
        self.session = {"kind": "owned_headless_wayland", "result": "passed", "command_exit_code": 0, "error": None,
                        "cleanup_passed": True, "status": "closed", "runtime_removed": False, "physical_qualification": False, "command": test_command,
                        "bus_command": ["dbus-run-session", "--", *test_command], "owner_token": owner,
                        "runtime_directory": f"/run/user/{os.getuid()}/fn-" + owner[:6],
                        "compositor_command": ["/usr/bin/Hyprland", "--config", str(self.root / supports["hyprland-test.lua"]["path"]), "--socket", "ferric-test", "--wayland-fd", "7"],
                        "compositor_config": str(self.root / supports["hyprland-test.lua"]["path"]),
                        "compositor_config_sha256": supports["hyprland-test.lua"]["sha256"],
                        "compositor_isolation": {"HYPRLAND_NO_SD_VARS": "1", "HYPRLAND_NO_SD_NOTIFY": "1", "HYPRLAND_NO_RT": "1", "LIBSEAT_BACKEND": "ferric-disabled", "AQ_DRM_DEVICES": ""},
                        "config_errors": [], "monitors": [{"name": "ferric-virtual", "width": 1920, "height": 1080, "scale": 1, "dpmsStatus": True, "disabled": False}],
                        "control_binary_sha256": a57.digest(self.compositor_control),
                        "compositor_sha256": a57.digest(self.compositor), "compositor_log": "compositor.log", "compositor_log_sha256": session_log["sha256"]}
        session_ref = self.put("gates/native/NATIVE_SESSION.json", self.session)
        self.session_cleanup = {"passed": True, "runtime_removed": True, "receipt_sha256": session_ref["sha256"],
                                "owner_token": owner, "runtime_directory": self.session["runtime_directory"]}
        session_cleanup_ref = self.put("gates/native/NATIVE_SESSION_SUPERVISOR_CHECK.json", self.session_cleanup)
        commands = {
            "sdk": [str(qt / "bin/qmake"), "-query", "QT_VERSION"],
            "engine": ["cargo", "+stable", "build", "--package", "ferric-browser-engine-qt", "--release", "--locked", "--offline", "--jobs", "2", "--verbose", "--target-dir", config["cargo_target"]],
            "configure": ["cmake", "-S", "crates/ferric-browser-engine-qt/tests/public-page-host", "-B", config["cmake_dir"], "-DFERRIC_ENGINE_LIBRARY=" + str(self.archive)],
            "harness": ["cmake", "--build", config["cmake_dir"], "--verbose", "--target", "public_page_host_integration_tests", "--parallel", "2"],
            "link": ["python3", str(self.root / supports["verify_link.py"]["path"]), "--out", str(self.root / link_ref["path"])],
            "discovery": ["ctest", "--test-dir", config["cmake_dir"], "--show-only=json-v1"],
            "native": ["python3", str(self.root / helper["path"]), "--receipt", str(self.root / cleanup["path"]), "--", "python3", str(self.root / supports["native_session.py"]["path"]), "--receipt", str(self.root / session_ref["path"]), "--", *test_command]}
        self.discovery = {"tests": [{"name": "T014e2s", "command": [str(self.exe), "rustBrowserUiRequestReparentsSamePageAcrossHostsWithProfileBootstrap"], "properties": [{"name": "TIMEOUT", "value": 180.0}]}]}
        gates = []
        for index, stage in enumerate(a57.STAGES):
            prefix = "gates/" + stage + "/"
            output = {"sdk": "6.11.2\n", "discovery": json.dumps(self.discovery), "native": "1/1 Test #1: T014e2s Passed\n100% tests passed, 0 tests failed out of 1\n"}.get(stage, "synthetic output\n")
            log = self.put(prefix + "output.log", output)
            monitor = {"result": "clear", "pinned_qt_prefix": str(qt), "artifact": self.target.name}
            receipt = {"command": commands[stage], "result": "passed", "exit_code": 0, "qt_include_guard": "clear", "qt_include_prefix": str(qt),
                       "source_before": self.snapshot, "source_after": self.snapshot, "started": f"2026-01-01T00:{index:02}:00Z", "finished": f"2026-01-01T00:{index:02}:01Z",
                       "require_tests": stage == "native", "observed_test_summary_count": 1 if stage == "native" else 0, "log": "output.log", "log_sha256": log["sha256"]}
            invocation = {"stage": stage, "target": str(self.target), "source": self.snapshot, "config_sha256": config_ref["sha256"], "runner_sha256": supports["run.py"]["sha256"]}
            closure = {"result": "passed", "lease_released": True, "source": self.snapshot, "monitor": monitor}
            gates.append({"stage": stage, "invocation": self.put(prefix + "INVOCATION.json", invocation), "receipt": self.put(prefix + "receipt.json", receipt),
                          "closure": self.put(prefix + "CLOSURE.json", closure), "monitor": self.put(prefix + "monitor.json", monitor), "runner": supports["run.py"]})
        runner_review = {"decision": "approve", "reviewer": "synthetic-independent-reviewer", "findings": [], "exact_hashes": {name: ref["sha256"] for name, ref in supports.items()}}
        runner_review["exact_hashes"]["CONFIG.json"] = config_ref["sha256"]
        self.q = {"result": "passed", "source_sha": self.source["commit"], "source_tree": self.source["tree"], "configuration": config_ref, "execution_support": supports, "gates": gates,
                  "source_review": self.put("SOURCE_REVIEW.json", {"decision": "approve", "candidate_commit": self.source["commit"], "candidate_tree": self.source["tree"], "reviewer": "synthetic-independent-reviewer", "findings": [], "complete_current_product_delta_covered": True}),
                  "runner_review": self.put("RUNNER_REVIEW.json", runner_review), "link_report": link_ref, "link_script": supports["verify_link.py"],
                  "sdk_kf_abi_log": self.put("gates/sdk/kf-abi.log", "Qt_6.11\n"), "kf_library_sha256": a57.digest(kf), "private_bus_helper": helper, "private_bus_cleanup": cleanup, "private_bus_supervisor": supervisor, "native_session": session_ref, "native_session_supervisor": session_cleanup_ref}
        self.c = {"source": self.source, "target": str(self.target.relative_to(self.root)), "metadata": {}, "private_bus_helper": helper}
        test_path = "crates/ferric-browser-engine-qt/tests/public-page-host/tst_public_page_host_integration.cpp"
        self.source["product_blobs"] = {test_path: "c" * 64}
        generated = self.put("gates/helper/generated/waiter.cpp", "// synthetic helper fixture\n")
        provenance = self.put("gates/helper/generated/PROVENANCE.json", {
            "candidate_file_sha256": "c" * 64, "generated_test_sha256": generated["sha256"],
            "compiler_command": ["c++", "-fsanitize=address,undefined", "-fno-sanitize-recover=all"]})
        helper_gate = copy.deepcopy(gates[-1])
        helper_gate["stage"] = "helper"
        helper_gate["runner"] = self.put("support/run-r3.py", "# synthetic retained earlier helper runner\n")
        helper_receipt = a57.load(self.root / helper_gate["receipt"]["path"])
        helper_receipt["command"] = ["python3", str(self.root / supports["helper_sanitizer.py"]["path"]),
                                     "--out", str((self.root / provenance["path"]).parent)]
        helper_receipt["log_sha256"] = self.put("gates/helper/output.log", "Totals: 5 passed, 0 failed, 0 skipped\n")["sha256"]
        helper_gate["receipt"] = self.put("gates/helper/receipt.json", helper_receipt)
        invocation = a57.load(self.root / helper_gate["invocation"]["path"])
        invocation["stage"] = "helper"
        invocation["runner_sha256"] = helper_gate["runner"]["sha256"]
        helper_gate["invocation"] = self.put("gates/helper/INVOCATION.json", invocation)
        self.q["supplemental_helper"] = {"gate": helper_gate, "provenance": provenance}
        self.review = {"decision": "accept", "reviewer": "synthetic-independent-reviewer", "source_sha": self.source["commit"], "source_tree": self.source["tree"],
                       "original_native_assertions_preserved_and_passed": True, "helper_regression_assertions_passed": True, "original_assertion_mapping": {"identity": "synthetic mapping"},
                       "helper_regression_mapping": {case: "synthetic mapping" for case in a57.HELPER_CASES}, "reviewed_execution_support": supports,
                       "reviewed_gate_runners": {gate["stage"]: gate["runner"] for gate in gates},
                       "reviewed_supplemental_helper_runner": helper_gate["runner"],
                       "effective_gate_timeouts": {stage: 5400 if stage == "engine" else 900 if stage in {"harness", "native"} else 120 for stage in a57.STAGES}}
        self.persist()

    def put(self, path, value):
        path = self.root / path
        path.parent.mkdir(parents=True, exist_ok=True)
        data = json.dumps(value).encode() if isinstance(value, (dict, list)) else value.encode()
        path.write_bytes(data)
        return {"path": str(path.relative_to(self.root)), "sha256": hashlib.sha256(data).hexdigest()}

    def persist(self):
        self.c["metadata"]["qualification"] = self.put("QUALIFICATION.json", self.q)
        self.review["qualification"] = self.c["metadata"]["qualification"]
        self.c["metadata"]["evidence_review"] = self.put("EVIDENCE_REVIEW.json", self.review)

    def persist_session(self, session):
        self.q["native_session"] = self.put("gates/native/NATIVE_SESSION.json", session)
        cleanup = dict(self.session_cleanup, receipt_sha256=self.q["native_session"]["sha256"])
        self.q["native_session_supervisor"] = self.put("gates/native/NATIVE_SESSION_SUPERVISOR_CHECK.json", cleanup)
        self.persist()

    def replace_discovery(self, inventory):
        gate = next(g for g in self.q["gates"] if g["stage"] == "discovery")
        receipt = a57.load(self.root / gate["receipt"]["path"])
        receipt["log_sha256"] = self.put("gates/discovery/output.log", json.dumps(inventory))["sha256"]
        gate["receipt"] = self.put(gate["receipt"]["path"], receipt)
        self.persist()

    def test_valid_synthetic_qualification_shape(self):
        original = (self.root / self.q["native_session"]["path"]).read_bytes()
        a57.validate_qualification_A57(self.root, self.c)
        self.assertEqual((self.root / self.q["native_session"]["path"]).read_bytes(), original)

    def test_current_ctest_pass_summary_without_failed_clause_is_accepted(self):
        gate = next(g for g in self.q["gates"] if g["stage"] == "native")
        receipt = a57.load(self.root / gate["receipt"]["path"])
        output = "1/1 Test #1: T014e2s Passed\n100% tests passed out of 1\n"
        receipt.update(observed_failed=0, observed_skips=0,
                       log_sha256=self.put("gates/native/output.log", output)["sha256"])
        gate["receipt"] = self.put(gate["receipt"]["path"], receipt)
        self.persist()
        a57.validate_qualification_A57(self.root, self.c)

    def test_discovery_must_name_exact_executable_once(self):
        for mutation in ("wrong_executable", "duplicate", "extra_argument"):
            with self.subTest(mutation=mutation):
                inventory = copy.deepcopy(self.discovery)
                if mutation == "wrong_executable":
                    inventory["tests"][0]["command"][0] = "/some/other/executable"
                elif mutation == "duplicate":
                    inventory["tests"].append(copy.deepcopy(inventory["tests"][0]))
                else:
                    inventory["tests"][0]["command"].append("anotherFunction")
                self.replace_discovery(inventory)
                with self.assertRaises(ValueError):
                    a57.validate_qualification_A57(self.root, self.c)

    def test_link_artifacts_and_execution_support_are_rehashed(self):
        for path in (self.exe, self.archive, self.link_file, self.root / self.q["execution_support"]["recovery_gate.py"]["path"]):
            with self.subTest(path=path.name):
                before = path.read_bytes()
                path.write_bytes(before + b"changed")
                with self.assertRaises(ValueError):
                    a57.validate_qualification_A57(self.root, self.c)
                path.write_bytes(before)

    def test_helper_mapping_requires_every_original_case(self):
        self.review["helper_regression_mapping"].pop(next(iter(a57.HELPER_CASES)))
        self.persist()
        with self.assertRaises(ValueError):
            a57.validate_qualification_A57(self.root, self.c)

    def test_retained_executable_must_remain_executable(self):
        self.exe.chmod(0o644)
        with self.assertRaises(ValueError):
            a57.validate_qualification_A57(self.root, self.c)

    def test_failed_or_misrepresented_session_is_rejected(self):
        mutations = {"result": "failed", "command_exit_code": 1, "error": "compositor failed",
                     "cleanup_passed": False, "runtime_removed": True, "status": "claimed", "physical_qualification": True,
                     "command": ["ctest", "-R", "^T014e2$"],
                     "bus_command": ["ctest", "-R", "^T014e2s$"],
                     "compositor_command": ["/usr/bin/Hyprland", "--config", "/user/config.lua"],
                     "compositor_sha256": "f" * 64, "compositor_log_sha256": "e" * 64}
        for field, value in mutations.items():
            with self.subTest(field=field):
                session = dict(self.session, **{field: value})
                self.persist_session(session)
                with self.assertRaises(ValueError):
                    a57.validate_qualification_A57(self.root, self.c)

    def test_compositor_socket_handover_requires_exact_private_arguments(self):
        mutations = []
        for descriptor in ("2", "-1", "not-a-file-descriptor"):
            command = self.session["compositor_command"][:-1] + [descriptor]
            mutations.append(command)
        mutations.append(self.session["compositor_command"] + ["--extra"])
        wrong_config = list(self.session["compositor_command"])
        wrong_config[2] = "/user/config.lua"
        mutations.append(wrong_config)
        for command in mutations:
            with self.subTest(command=command):
                self.persist_session(dict(self.session, compositor_command=command))
                with self.assertRaisesRegex(ValueError, "socket handover"):
                    a57.validate_qualification_A57(self.root, self.c)

    def test_only_one_active_private_headless_output_is_accepted(self):
        monitor = self.session["monitors"][0]
        mutations = [[], [monitor, dict(monitor, name="WAYLAND-1")]]
        mutations.extend([[dict(monitor, **{key: value})] for key, value in
                          (("name", "WAYLAND-1"), ("width", 1280), ("height", 720), ("scale", 2), ("dpmsStatus", False), ("disabled", True))])
        for monitors in mutations:
            with self.subTest(monitors=monitors):
                self.persist_session(dict(self.session, monitors=monitors))
                with self.assertRaisesRegex(ValueError, "private active headless output"):
                    a57.validate_qualification_A57(self.root, self.c)

    def test_compositor_config_and_environment_isolation_remain_bound(self):
        mutations = {"config_errors": ["configuration error"], "compositor_config": "/user/config.lua",
                     "compositor_config_sha256": "f" * 64, "control_binary_sha256": "e" * 64}
        for key, value in mutations.items():
            with self.subTest(key=key):
                self.persist_session(dict(self.session, **{key: value}))
                with self.assertRaisesRegex(ValueError, "isolated native Wayland session"):
                    a57.validate_qualification_A57(self.root, self.c)
        for flag in self.session["compositor_isolation"]:
            with self.subTest(flag=flag):
                isolation = dict(self.session["compositor_isolation"])
                isolation.pop(flag)
                self.persist_session(dict(self.session, compositor_isolation=isolation))
                with self.assertRaisesRegex(ValueError, "isolated native Wayland session"):
                    a57.validate_qualification_A57(self.root, self.c)

    def test_session_supervisor_must_prove_exact_original_receipt_cleanup(self):
        mutations = {"passed": False, "runtime_removed": False, "receipt_sha256": "f" * 64,
                     "owner_token": self.session["owner_token"][:6] + "f" * 26, "runtime_directory": "/another/runtime"}
        for field, value in mutations.items():
            with self.subTest(field=field):
                self.q["native_session_supervisor"] = self.put("gates/native/NATIVE_SESSION_SUPERVISOR_CHECK.json", dict(self.session_cleanup, **{field: value}))
                self.persist()
                with self.assertRaisesRegex(ValueError, "isolated native Wayland session"):
                    a57.validate_qualification_A57(self.root, self.c)

    def test_missing_owner_nonce_cannot_match_other_missing_tokens(self):
        for token in (None, ""):
            with self.subTest(token=token):
                session = dict(self.session, owner_token=token)
                self.q["native_session"] = self.put("gates/native/NATIVE_SESSION.json", session)
                session_cleanup = dict(self.session_cleanup, owner_token=token, receipt_sha256=self.q["native_session"]["sha256"])
                self.q["native_session_supervisor"] = self.put("gates/native/NATIVE_SESSION_SUPERVISOR_CHECK.json", session_cleanup)
                bus = a57.load(self.root / self.q["private_bus_cleanup"]["path"])
                bus["owner_token"] = token
                self.q["private_bus_cleanup"] = self.put("gates/native/cleanup.json", bus)
                supervisor = {"passed": True, "remaining": [], "owner_token": token, "receipt_sha256": self.q["private_bus_cleanup"]["sha256"]}
                self.q["private_bus_supervisor"] = self.put("gates/native/supervisor.json", supervisor)
                self.persist()
                with self.assertRaises(ValueError):
                    a57.validate_qualification_A57(self.root, self.c)

    def test_missing_runtime_directory_cannot_match_missing_supervisor_path(self):
        for directory in (None, ""):
            with self.subTest(directory=directory):
                self.q["native_session"] = self.put("gates/native/NATIVE_SESSION.json", dict(self.session, runtime_directory=directory))
                cleanup = dict(self.session_cleanup, runtime_directory=directory, receipt_sha256=self.q["native_session"]["sha256"])
                self.q["native_session_supervisor"] = self.put("gates/native/NATIVE_SESSION_SUPERVISOR_CHECK.json", cleanup)
                self.persist()
                with self.assertRaises(ValueError):
                    a57.validate_qualification_A57(self.root, self.c)

    def test_session_log_compositor_and_wrapper_tampering_is_rejected(self):
        wrapper = self.root / self.q["execution_support"]["native_session.py"]["path"]
        compositor_config = self.root / self.q["execution_support"]["hyprland-test.lua"]["path"]
        for path in (self.session_log, self.compositor, self.compositor_control, wrapper, compositor_config):
            with self.subTest(path=path.name):
                before = path.read_bytes()
                path.write_bytes(before + b"changed")
                with self.assertRaises(ValueError):
                    a57.validate_qualification_A57(self.root, self.c)
                path.write_bytes(before)

    def test_native_command_cannot_bypass_session_wrapper(self):
        gate = next(g for g in self.q["gates"] if g["stage"] == "native")
        receipt = a57.load(self.root / gate["receipt"]["path"])
        receipt["command"] = receipt["command"][:5] + self.session["command"]
        gate["receipt"] = self.put(gate["receipt"]["path"], receipt)
        self.persist()
        with self.assertRaisesRegex(ValueError, "single serial real native test"):
            a57.validate_qualification_A57(self.root, self.c)

    def test_bus_cannot_be_started_outside_private_wayland_environment(self):
        gate = next(g for g in self.q["gates"] if g["stage"] == "native")
        receipt = a57.load(self.root / gate["receipt"]["path"])
        receipt["command"][5:5] = ["dbus-run-session", "--"]
        gate["receipt"] = self.put(gate["receipt"]["path"], receipt)
        self.persist()
        with self.assertRaisesRegex(ValueError, "single serial real native test"):
            a57.validate_qualification_A57(self.root, self.c)

    def test_retained_helper_runner_requires_its_actual_review_binding(self):
        self.review["reviewed_supplemental_helper_runner"] = self.q["execution_support"]["run.py"]
        self.persist()
        with self.assertRaisesRegex(ValueError, "independent actual-assertion evidence review"):
            a57.validate_qualification_A57(self.root, self.c)

    def test_link_script_must_be_the_reviewed_support(self):
        alternate = self.put("unreviewed/verify_link.py", "# different unreviewed verifier\n")
        self.q["link_script"] = alternate
        gate = next(g for g in self.q["gates"] if g["stage"] == "link")
        receipt = a57.load(self.root / gate["receipt"]["path"])
        receipt["command"][1] = str(self.root / alternate["path"])
        gate["receipt"] = self.put(gate["receipt"]["path"], receipt)
        self.persist()
        with self.assertRaises(ValueError):
            a57.validate_qualification_A57(self.root, self.c)


class IntegrationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="a57-integration-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "retained-import"
        self.repo.mkdir()
        self.git("init", "--quiet")
        self.git("config", "user.name", "Synthetic A57 test")
        self.git("config", "user.email", "a57-test@example.invalid")
        (self.repo / "product.txt").write_text("qualified product\n")
        self.source = self.commit("synthetic tested source")
        doc = self.repo / "docs/implementation/everyday-v1/README.md"
        doc.parent.mkdir(parents=True)
        doc.write_text("synthetic import documentation\n")
        self.imported = self.commit("synthetic import")
        (self.repo / "product.txt").write_text("ordinary future change\n")
        self.future = self.commit("synthetic later task")
        self.git("checkout", "--quiet", "--detach", self.imported)
        self.c = {"source": {"commit": self.source}, "metadata": {"pack_review": "PACK_REVIEW.json"}}
        self.state = {"integration_sha": self.future}
        self.bind_authority(self.imported)

    def git(self, *args):
        return subprocess.check_output(["git", "-c", "core.hooksPath=/dev/null", "-C", str(self.repo), *args], text=True, stderr=subprocess.DEVNULL).strip()

    def commit(self, message):
        self.git("add", "--all")
        self.git("commit", "--quiet", "-m", message)
        return self.git("rev-parse", "HEAD")

    def bind_authority(self, commit):
        tree = self.git("rev-parse", commit + "^{tree}")
        review = self.root / "PACK_REVIEW.json"
        review.write_text(json.dumps({"decision": "accept", "reviewer": "synthetic-independent-reviewer", "candidate": {"commit": commit, "tree": tree}}))
        self.state["reconciliation_A57_authority"] = {"worktree": str(self.repo.relative_to(self.root)), "commit": commit, "tree": tree, "review": {"path": review.name, "sha256": a57.digest(review)}}

    def test_pack_only_import_allows_future_product_change(self):
        a57.validate_integration_A57(self.root, self.state, self.c)

    def test_unqualified_product_change_at_import_is_rejected(self):
        self.git("checkout", "--quiet", "--detach", self.future)
        self.bind_authority(self.future)
        with self.assertRaisesRegex(ValueError, "product paths/modes/blobs"):
            a57.validate_integration_A57(self.root, self.state, self.c)

    def test_mode_or_added_product_path_at_import_is_rejected(self):
        for mutation in ("mode", "addition"):
            with self.subTest(mutation=mutation):
                self.git("checkout", "--quiet", "--detach", self.imported)
                if mutation == "mode":
                    (self.repo / "product.txt").chmod(0o755)
                else:
                    (self.repo / "extra-product.txt").write_text("unqualified addition\n")
                wrong = self.commit("synthetic unqualified " + mutation)
                self.bind_authority(wrong)
                self.state["integration_sha"] = wrong
                with self.assertRaisesRegex(ValueError, "product paths/modes/blobs"):
                    a57.validate_integration_A57(self.root, self.state, self.c)

    def test_current_integration_cannot_precede_import(self):
        self.state["integration_sha"] = self.source
        with self.assertRaisesRegex(ValueError, "does not descend"):
            a57.validate_integration_A57(self.root, self.state, self.c)


class StateBoundaryTests(unittest.TestCase):
    def test_recovery_ownership_is_rejected_but_unrelated_owner_is_allowed(self):
        target = "/isolated/builds/native-recovery"
        cases = [{"active": ["F02e2s"]}, {"active_builds": ["native-recovery"]}, {"active_locks": {"native": {"target": target}}}]
        for key, value in (("reserved_for", "F02e2s-direct-recovery"), ("slice", "F02e2s"), ("artifact", "native-recovery"), ("target", target)):
            cases.append({"native_lease": {"active": True, key: value}})
        for state in cases:
            with self.subTest(state=state):
                self.assertTrue(a57.recovery_open(state, target))
        self.assertFalse(a57.recovery_open({"active": ["F02e3"], "active_builds": ["future-target"], "native_lease": {"active": True, "slice": "F02e3", "artifact": "future-target"}}, target))

    def test_prior_accepted_pin_cannot_be_omitted_or_rebased(self):
        parent = {"status": "accepted", "attempts": 5, "attempt_history": {str(i): {"result": "failed"} for i in range(1, 6)}}
        prior = {"status": "accepted", "attempts": 1, "source_sha": "a" * 40}
        history = {"slices": {"F02e2s": dict(parent, status="failed"), "S00": prior}}
        state = {"slices": {"F02e2s": parent, "S00": dict(prior, status="failed")}}
        c = {"metadata": {"authorization": {}}, "accepted_parent_sha256": a57.object_sha(parent)}
        for pins in ({}, {"S00": a57.object_sha(state["slices"]["S00"])}):
            with self.subTest(pins=pins), patch.object(a57, "contract_A57", return_value=dict(c, accepted_states_sha256=pins)), patch.object(a57, "validate_history_A57", return_value=history), patch.object(a57, "reference"):
                errors = a57.validate_state_A57({"direct_recovery_A57": {}}, state, Path("/synthetic-unused"))
                self.assertTrue(any("preserve every previously accepted slice" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
