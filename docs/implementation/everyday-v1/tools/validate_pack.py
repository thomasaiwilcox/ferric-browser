#!/usr/bin/env python3
"""Validate pack/ledger consistency. This does not establish real browser correctness."""
from __future__ import annotations
import argparse,json,re,shlex,sys
from pathlib import Path
from common import (PACK,load,read_plan,packet,safe_relative,digest,
                    ATTEMPT_EXCEPTION_TASKS,ATTEMPT_EXCEPTION_AMENDMENTS,
                    ATTEMPT_EXCEPTION_LIMITS,GLOBAL_MAX_ATTEMPTS,effective_attempt_limit,
                    TASK_SUPERSESSION_AMENDMENTS)
STATUSES={"not_started","ready","in_progress","submitted","reviewing","accepted","superseded","blocked_environment","blocked_specification","implemented_unqualified","failed"}
LEVELS={"inspection","unit","component","application","native","physical","benchmark","build","fuzz"}
RECIPES={"preflight","baseline","config","rust-policy","application-rust","storage","application","credential","helper","rp","fuzz","benchmark","native","physical","package","ci","docs","review","rebuild","release"}
HEX=re.compile(r"^[0-9a-f]{40}$")
A38_F02E1_PATHS=frozenset({
    "crates/ferric-browser-engine-qt/src/public_page_owner.h",
    "crates/ferric-browser-engine-qt/src/public_page_owner.cpp",
    "crates/ferric-browser-engine-qt/src/public_profile_owner.h",
    "crates/ferric-browser-engine-qt/src/public_profile_owner.cpp",
    "crates/ferric-browser-engine-qt/build.rs",
    "crates/ferric-browser-engine-qt/tests/public-page-host/CMakeLists.txt",
    "crates/ferric-browser-engine-qt/tests/public-page-host/tst_public_page_owner.cpp",
    "crates/ferric-browser-engine-qt/src/request_interceptor.h",
    "crates/ferric-browser-engine-qt/src/request_interceptor.cpp",
})
A38_ADDED_PATH="crates/ferric-browser-engine-qt/src/request_interceptor.cpp"
A42_F02E2_PATHS=frozenset({
    "crates/ferric-browser-engine-qt/src/public_page_host.h",
    "crates/ferric-browser-engine-qt/src/public_page_host.cpp",
    "crates/ferric-browser-engine-qt/qml/components/FerricWidgetChrome.qml",
    "crates/ferric-browser-engine-qt/src/bridge.rs",
    "crates/ferric-browser-engine-qt/src/browser_ui_tab_state.rs",
    "crates/ferric-browser-engine-qt/src/engine_requests.rs",
    "crates/ferric-browser-engine-qt/build.rs",
    "crates/ferric-browser-engine-qt/tests/public-page-host/CMakeLists.txt",
    "crates/ferric-browser-engine-qt/tests/public-page-host/tst_public_page_host_integration.cpp",
})
A42_ADDED_PATHS=[
    "crates/ferric-browser-engine-qt/src/browser_ui_tab_state.rs",
    "crates/ferric-browser-engine-qt/src/engine_requests.rs",
]
A42_REMOVED_PATHS=["crates/ferric-browser-engine-qt/src/gui_application.cpp"]
A44_ADDED_PATH="crates/ferric-browser-engine-qt/src/browser_ui_input.rs"
A44_F02E2_PATHS=A42_F02E2_PATHS|{A44_ADDED_PATH}
A44_LOCKS=["public-page-host","bridge","test-registration","browser-ui-input"]
A52_NATIVE_GATE_SEQUENCE=[
    "read-only-sdk-selection-preflight",
    "exact-source-release-engine-build",
    "public-page-host-harness-configure",
    "integration-target-build",
    "robust-executable-link-order-verification",
    "nonzero-ctest-discovery",
    "T014e2s-serial",
]
A45_CANDIDATE_SHA="58c047c04072bb7efa382e3cd7ca65a11da1b117"
A45_CANDIDATE_TREE="f5cc15efd18496bb209d25866d060347157a00bd"
A45_QT_PREFIX="/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64"
A45_FRESH_TARGET="/home/tom/Projects/ferric-everyday-work/builds/native-F02e2-a5-qt6112-qmake"
A48_BASE_SHA="58c047c04072bb7efa382e3cd7ca65a11da1b117"
A48_BASE_TREE="f5cc15efd18496bb209d25866d060347157a00bd"
A48_SOURCE_PATH="crates/ferric-browser-engine-qt/tests/public-page-host/tst_public_page_host_integration.cpp"
A48_NATIVE_TARGET="/home/tom/Projects/ferric-everyday-work/builds/native-F02e2-a6-qt6112-qmake"
A48_TOOLING_PATHS=[
    "docs/implementation/everyday-v1/tools/common.py",
    "docs/implementation/everyday-v1/tools/validate_pack.py",
    "docs/implementation/everyday-v1/tools/test_tools.py",
]
A48_T014E2_FAILURE_BINDING=(
    "A48 failed: run evidence/F02e2-a6/native/NATIVE_RUN-A48.json "
    "(SHA-256 e6b0695df918b20713c8c86d008e99c1ac3f7a68a8852fea40d37d2fd3ae9eb1); "
    "receipt evidence/F02e2-a6/native/T014e2-A48/receipt.json "
    "(SHA-256 2e35b498a9a5db8802bac48bf3f28fbfed17ace1ac70810170bcc6bc38d9f32a); "
    "CTest exit 8; QtTest 2 passed, 1 failed, 0 skipped; BrowserUi QML registration failed before page-reparent assertions."
)
A49_CHILD_PATHS=[
    "crates/ferric-browser-engine-qt/tests/public-page-host/CMakeLists.txt",
    "crates/ferric-browser-engine-qt/tests/public-page-host/tst_static_qml_registration.cpp",
]
A49_CHILD_LOCKS=["public-page-host","test-registration"]
A50_F02E2_PATHS_ORDERED=[
    "crates/ferric-browser-engine-qt/src/public_page_host.h",
    "crates/ferric-browser-engine-qt/src/public_page_host.cpp",
    "crates/ferric-browser-engine-qt/qml/components/FerricWidgetChrome.qml",
    "crates/ferric-browser-engine-qt/src/bridge.rs",
    "crates/ferric-browser-engine-qt/src/browser_ui_tab_state.rs",
    "crates/ferric-browser-engine-qt/src/engine_requests.rs",
    "crates/ferric-browser-engine-qt/build.rs",
    "crates/ferric-browser-engine-qt/tests/public-page-host/CMakeLists.txt",
    "crates/ferric-browser-engine-qt/tests/public-page-host/tst_public_page_host_integration.cpp",
    "crates/ferric-browser-engine-qt/src/browser_ui_input.rs",
]
A50_F02E2R_TARGET="/home/tom/Projects/ferric-everyday-work/builds/native-F02e2r-a1-qt6112-qmake"
A50_NEW_SLOT="rustBrowserUiRequestReparentsSamePageAcrossHostsWithInitializer"
A50_TERMINAL_FAILURE={
    "result":"failed","attempts":6,"test_id":"T014e2",
    "run_path":"evidence/F02e2-a6/native/NATIVE_RUN-A48.json",
    "run_sha256":"e6b0695df918b20713c8c86d008e99c1ac3f7a68a8852fea40d37d2fd3ae9eb1",
    "receipt_path":"evidence/F02e2-a6/native/T014e2-A48/receipt.json",
    "receipt_sha256":"2e35b498a9a5db8802bac48bf3f28fbfed17ace1ac70810170bcc6bc38d9f32a",
}
A51_CANDIDATE={"source_sha":"c6e29480e08f5e7a1ba54319ec2f2fe932d1f366","source_tree":"f5b7696b1c08d115ba435a521fc2a47fa128f538"}
A51_TARGET="/home/tom/Projects/ferric-everyday-work/builds/native-F02e2r-a3-qt6112-qmake"
A51_ARTIFACTS={
    "target":A51_TARGET,
    "integration_executable_sha256":"da48cdbb935bb128d017c699d91b82dcc802d42e810f90014571f47ecf0b69f5",
    "ctest_file_sha256":"306e7c47fd6d018083285d5e049d55ffe942b8c14e35a218485dc2cdcfbfacb1",
    "link_command_sha256":"efb4315b0c4db1c233254d8d6fcde88498e738ae0c3bc377f245e69dd8344821",
}
A51_A50_RUN={
    "path":"evidence/F02e2r-a3/native/NATIVE_RUN-A50-a3.json",
    "sha256":"69c220a114afb4a05da8dfcf5458292db6d954305cf6900ac4cc8f07111c0906",
    "result":"failed",
}
A51_A50_CLOSURE={
    "path":"evidence/F02e2r-a3/native/NATIVE_LEASE_CLOSED-A50-a3.json",
    "sha256":"c6e059a05d0ba4271400c1dbaf4723818f05b7abf0c3901cfc0605954ce48739",
}
A51_ALLOWED_GATES=[
    "read-only exact-source/artifact/hash verification",
    "nonzero CTest discovery requiring T014e2r",
    "one serial T014e2r run with zero skips",
    "independent A51 gate-evidence review",
]
A51_PROHIBITED=[
    "source changes","engine or harness rebuild","CMake reconfigure",
    "artifact substitution","attempt 4","T014e2","any additional product test",
]
A51_PLAN_RECORD={
    "amendment":"A51","status":"accepted_plan_only",
    "review_path":"evidence/A51/PLAN_REVIEW-r2_d28.json",
    "review_sha256":"e27fb49ba67a27955f5c57576bd38e434b780f2113b1cdf42fa22318db132b1d",
    "review_budget_path":"evidence/A51/PLAN_REVIEW_BUDGET-r2_d28.json",
    "review_budget_sha256":"b20f3d565b95150b22ba4d41f9b862d7228d467a8f80ef4624fa6a35854e32f6",
    "proposal_path":"evidence/A51/PROPOSAL.json",
    "proposal_sha256":"f050004b1df76f4038bd46fb2f9c59ab61abfcfc5a9bcf3146c39719611f8ddf",
    "packet_path":"packets/F02e2-A51-a3-link-check-continuation.md",
    "packet_sha256":"4f37f646b2caaa55949ed0e6cfe5766596536481ecd5efd6064092415aa71e9e",
    "prior_request_changes_review":{"path":"evidence/A51/PLAN_REVIEW_d28.json","sha256":"21dff25c21c3ca78a981127e9c6caa13420dc976f6451b54e3af3941172da4cd"},
    "postmortem":{"path":"evidence/F02e2r-a3/GATE_POSTMORTEM_REVIEW_d28.json","sha256":"2babd0ab2103da96508f101e971555c80a01fa0d967ac3dbb42ea5474572d223","decision":"stop_request_new_reviewed_amendment_before_continuation"},
    "attempt_identity":{"slice":"F02e2r","source_amendment":"A50","attempt":3,"attempt_limit":3,"counter_increment":False},
    "candidate":A51_CANDIDATE,"preserved_A50_run":A51_A50_RUN,
    "preserved_A50_lease_closure":A51_A50_CLOSURE,
    "underlying_integration_target_build":{"path":"evidence/F02e2r-a3/native/integration-target-build-a3/receipt.json","sha256":"92b8518b423c71b34f15a6203f9996e5f0dcb5cbdccf27c398cffd2ce16f2014","result":"passed","exit_code":0},
    "artifacts":A51_ARTIFACTS,"allowed_gates":A51_ALLOWED_GATES,"prohibited":A51_PROHIBITED,
    "continuation_lease":"fresh exclusive test-only lease on the preserved A3 target; close on all outcomes",
    "acceptance":"Only after the remaining gates pass and independent A51 gate-evidence review accepts the supplemental record; preserve A50 failed run unchanged.",
    "failure":"Any continuation gate failure leaves F02e2r failed at 3/3; any further work needs another independently reviewed amendment.",
    "parent_and_downstream":{"F02e2_remains_failed_at_6_of_6_until_F02e2r_acceptance":True,"F02e3_blocked_until_F02e2r_acceptance":True},
    "auth":{"no_attempt_increment":True,"source_changes_authorized":False,"build_or_configure_authorized":False,"artifact_substitution_authorized":False,"T014e2_authorized":False,"T014e2r_authorized_after_pack_validation":True},
}
A51_STATE_POLICY={
    "amendment":"A51","continuation_of_amendment":"A50","continuation_of_attempt":3,
    "attempt_limit":3,"attempt_increment":False,"candidate":A51_CANDIDATE,
    "artifacts":A51_ARTIFACTS,"preserved_A50_run":A51_A50_RUN,
    "preserved_A50_lease_closure":A51_A50_CLOSURE,"allowed_tests":["T014e2r"],
    "allowed_gates":A51_ALLOWED_GATES,"prohibited":A51_PROHIBITED,
    "activation":"Only after the independently accepted A51 amendment is integrated and pack validation passes.",
}
SUPERSESSION_AMENDMENT="A37"

def validate_matrix_execution_status(tests):
    """Allow only the exact, receipt-bound A48 T014e2 historical failure."""
    errors=[]
    rows={x.get("id"):x for x in tests if isinstance(x,dict)}
    parent=rows.get("T014e2")
    if (parent is None or parent.get("slice")!="F02e2"
            or parent.get("result")!="failed"
            or parent.get("binding")!=A48_T014E2_FAILURE_BINDING):
        errors.append("T014e2: A48 historical failure must retain its exact failed result and receipt/run binding")
    for x in tests:
        if x.get("id")!="T014e2" and x.get("result")!="not_run":
            errors.append(f"{x.get('id')}: only the exact A48 T014e2 failure may have an execution result in the plan matrix")
    return errors

def validate_harness_prerequisite_split_A49(plan,pack=PACK):
    """Enforce A49's separate harness-only task without reopening F02e2."""
    errors=[]
    expected={
        "amendment":"A49",
        "review_path":"evidence/A49/PLAN_REVIEW-r2_d28.json",
        "review_sha256":"60d234b4f02f27604d19ebdf893179de015cd13cbb7963991d5d908e25684926",
        "review_budget_path":"evidence/A49/PLAN_REVIEW_BUDGET-r2_d28.json",
        "review_budget_sha256":"6bb486e64b76685667a20bfbe74992b4287d78e4176e9b39bcfef8e297a515fb",
        "parent_slice":"F02e2","parent_status":"failed","parent_attempts":6,
        "parent_attempt_limit":6,"parent_test_id":"T014e2","parent_test_owner":"F02e2",
        "parent_test_result":"failed",
        "parent_run_sha256":"e6b0695df918b20713c8c86d008e99c1ac3f7a68a8852fea40d37d2fd3ae9eb1",
        "parent_receipt_sha256":"2e35b498a9a5db8802bac48bf3f28fbfed17ace1ac70810170bcc6bc38d9f32a",
        "child_slice":"F02e2q","child_test_id":"T014e2q","child_test_owner":"F02e2q",
        "child_test_result":"not_run","child_allowed_paths":A49_CHILD_PATHS,
        "child_locks":A49_CHILD_LOCKS,"child_dependencies":["F02e1"],
        "child_default_attempt_limit":3,"child_target_name":"static_qml_registration_tests",
        "child_ctest_name":"T014e2q",
        "fresh_native_target":"/home/tom/Projects/ferric-everyday-work/builds/native-F02e2q-a1-qt6112-qmake",
        "cargo_target_dir":"/home/tom/Projects/ferric-everyday-work/builds/native-F02e2q-a1-qt6112-qmake/cargo-target",
        "cmake_dir":"/home/tom/Projects/ferric-everyday-work/builds/native-F02e2q-a1-qt6112-qmake/cmake",
        "sdk":{
            "prefix":"/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64",
            "expected_version":"6.11.2",
            "qmake":"/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64/bin/qmake",
            "prepend_path":"/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64/bin",
            "pkg_config_path":"/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64/lib/pkgconfig",
            "pkg_config_libdir":"/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64/lib/pkgconfig:/usr/lib/pkgconfig:/usr/share/pkgconfig",
            "cmake_prefix_path":"/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64",
            "qt6_dir":"/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64/lib/cmake/Qt6",
            "minimum_kf6_window_system":"6.28","last_verified_kf6_window_system":"6.31.0",
            "reverify_before_lease":True,
        },
        "compile_gate_controls":{
            "cc_enable_debug_output":"1",
            "include_guard":"docs/implementation/everyday-v1/tools/run_gate.py",
            "qt_include_prefix":"/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64",
            "live_compiler_process_monitor":True,"stop_and_close_lease_on_failure":True,
        },
        "prohibitions":{
            "attempt_seven_authorized":False,"parent_test_rerun_authorized":False,
            "same_source_parent_retry_authorized":False,"child_may_accept_parent":False,
            "child_unblocks_F02e3":False,
        },
        "matrix_exception":{
            "only_row":"T014e2","permitted_result":"failed",
            "binding":"A48 run and receipt paths and SHA-256 values exactly as recorded in control/test_matrix.json; all other historical row statuses remain not_run unless separately amended.",
        },
    }
    if plan.get("harness_prerequisite_splits_A49")!=expected:
        errors.append("A49: harness prerequisite split record differs from the independently accepted scope")
    tasks={t.get("id"):t for t in plan.get("slices",[]) if isinstance(t,dict)}
    parent=tasks.get("F02e2");child=tasks.get("F02e2q");downstream=tasks.get("F02e3")
    if (parent is None or parent.get("max_attempts")!=6
            or parent.get("test_ids")!=["T014e2"]
            or parent.get("locks")!=["public-page-host","bridge","test-registration","browser-ui-input"]):
        errors.append("A49: preserve F02e2's terminal six-attempt limit, T014e2 ownership and original locks")
    if (child is None or child.get("allowed_paths")!=A49_CHILD_PATHS
            or child.get("locks")!=A49_CHILD_LOCKS
            or child.get("depends_on")!=["F02e1"]
            or child.get("test_ids")!=["T014e2q"]
            or child.get("max_changed_product_files")!=2
            or child.get("max_attempts") is not None
            or child.get("recipe")!="application"
            or child.get("parent_id")!="F02e"):
        errors.append("F02e2q: child task differs from its independently reviewed harness-only scope")
    current_successor="F02e2s" if plan.get("successor_recovery_A52") is not None else "F02e2r"
    expected_history=["F02e2","F02e2r"] if current_successor=="F02e2s" else ["F02e2"]
    if (downstream is None or downstream.get("depends_on")!=[current_successor]
            or "F02e2q" in downstream.get("depends_on",[])
            or downstream.get("historical_depends_on")!=expected_history):
        errors.append(f"F02e3: require successor {current_successor} and retain exhausted tasks only as history; F02e2q alone cannot unblock the feature chain")
    tests=load(pack/"control/test_matrix.json")["tests"]
    errors+=validate_matrix_execution_status(tests)
    by_id={x.get("id"):x for x in tests}
    if by_id.get("T014e2q",{}).get("slice")!="F02e2q" or by_id.get("T014e2q",{}).get("result")!="not_run":
        errors.append("T014e2q: child must own a separate not_run application test row")
    try:
        source_map=load(pack/"control/source-map.execution.json")
        records=[x for x in source_map.get("execution_amendments",[]) if isinstance(x,dict) and x.get("id")=="A49"]
        if len(records)!=1:
            errors.append("A49: source map must contain exactly one accepted child ownership record")
        else:
            record=records[0]
            if (record.get("parent_test_owner")!="F02e2" or record.get("parent_test")!="T014e2"
                    or record.get("child_test_owner")!="F02e2q" or record.get("child_test")!="T014e2q"
                    or record.get("fresh_native_target")!=expected["fresh_native_target"]
                    or record.get("child_target_name")!="static_qml_registration_tests"
                    or record.get("parent_test_rerun_authorized") is not False
                    or record.get("child_unblocks_F02e3") is not False):
                errors.append("A49: source map changes test ownership, target or terminal boundaries")
            owners=record.get("path_owners",{})
            if (not any("T014e2 registration" in x for x in owners.get("F02e2",[]))
                    or not any("T014e2q registration" in x for x in owners.get("F02e2q",[]))):
                errors.append("A49: source map must assign CMake registrations to their separate task owners")
    except (OSError,ValueError,TypeError,AttributeError) as exc:
        errors.append(f"A49: cannot validate source-map ownership: {exc}")
    amendment_path=pack/"control/AMENDMENTS.md"
    if amendment_path.is_file():
        section=amendment_path.read_text().split("## A49",1)
        if len(section)<2 or any(x not in section[1].split("\n## ",1)[0] for x in ("F02e2q","T014e2q","6/6","T014e2")):
            errors.append("A49: accepted amendment record must document the child and preserved terminal parent")
    else:
        errors.append("A49: missing accepted amendment record")
    return errors

def validate_successor_recovery_A50(plan,pack=PACK):
    """Validate the distinct F02e2r successor and its pending/accepted supersession boundary."""
    errors=[]
    try:
        amendments=(pack/"control/AMENDMENTS.md").read_text()
        section=amendments.split("## A50",1)[1].split("\n## ",1)[0]
        accepted="**Status:** revision 2 independently accepted" in section
    except (IndexError,OSError):
        section="";accepted=False
    record=plan.get("successor_recovery_A50")
    if not accepted:
        if record is not None:
            errors.append("A50: successor metadata requires its independently accepted amendment record")
        return errors
    expected={
        "amendment":"A50",
        "review_path":"evidence/A50/PLAN_REVIEW-r2_d28.json",
        "review_sha256":"f348dfe81f41bda61759f849287b1a5f02aa14d8cdd3797818248629c1f9b932",
        "review_budget_path":"evidence/A50/PLAN_REVIEW_BUDGET-r2_d28.json",
        "review_budget_sha256":"60adf2b5f7daf47b0215ee7f6dc78bf89b6227b8850b21cc5a91a6c9fbb319b6",
        "prior_request_changes_sha256":"3e2dd020eccc06e736d261d61d583e386423fdf5cc722f6379c1b2b8e13cc0ed",
        "terminal_parent":{
            "slice":"F02e2","status":"failed","attempts":6,"attempt_limit":6,"test_id":"T014e2",
            "run_path":A50_TERMINAL_FAILURE["run_path"],"run_sha256":A50_TERMINAL_FAILURE["run_sha256"],
            "receipt_path":A50_TERMINAL_FAILURE["receipt_path"],"receipt_sha256":A50_TERMINAL_FAILURE["receipt_sha256"],
            "attempt_seven_authorized":False,"old_test_execution_authorized":False,
        },
        "accepted_prerequisite":{
            "slice":"F02e2q","test_id":"T014e2q","source_sha":"eb6ff74ad7134ff6fb5a2e934e08fd3f22a777a9",
            "source_tree":"42b809fee461d7a555b79f953fe1c56d6c1a0845",
            "evidence_sha256":"d7527467846770e3b85a54c6cd4d59ed990466f5aa63237450b5f576015724f8",
        },
        "successor":{
            "slice":"F02e2r","test_id":"T014e2r","dependencies":["F02e1","F02e2q"],"attempt_limit":3,
            "allowed_paths":A50_F02E2_PATHS_ORDERED,"locks":A44_LOCKS,"max_changed_product_files":10,
            "target":A50_F02E2R_TARGET,
            "new_slot":"rustBrowserUiRequestReparentsSamePageAcrossHostsWithInitializer",
            "old_slot":"rustBrowserUiRequestReparentsSamePageAcrossHosts","old_slot_execution_authorized":False,
        },
        "supersession":{
            "old_slice":"F02e2","new_slice":"F02e2r","old_test":"T014e2","new_test":"T014e2r",
            "transition":"Keep F02e2 failed while F02e2r is pending/in progress; after F02e2r accepted, set F02e2 workflow status superseded and retain immutable terminal_failure.",
        },
        "plan_tooling_paths":["tools/common.py","tools/validate_pack.py","tools/test_tools.py"],
    }
    if record!=expected:
        errors.append("A50: successor recovery metadata differs from the independently accepted scope")
    tasks={t.get("id"):t for t in plan.get("slices",[]) if isinstance(t,dict)}
    a52_active=plan.get("successor_recovery_A52") is not None
    current_successor="F02e2s" if a52_active else "F02e2r"
    parent=tasks.get("F02e2");successor=tasks.get("F02e2r");downstream=tasks.get("F02e3");aggregate=tasks.get("F02e")
    if (parent is None or parent.get("max_attempts")!=6 or parent.get("test_ids")!=["T014e2"]
            or parent.get("superseded_by")!=[current_successor]
            or parent.get("amendment")!= ("A52" if a52_active else "A50")):
        errors.append(f"F02e2: retain the six-attempt terminal parent and map it only to {current_successor}")
    if (successor is None or successor.get("depends_on")!=["F02e1","F02e2q"]
            or successor.get("test_ids")!=["T014e2r"] or successor.get("allowed_paths")!=A50_F02E2_PATHS_ORDERED
            or successor.get("locks")!=A44_LOCKS or successor.get("max_changed_product_files")!=10
            or successor.get("path_budget_amendment")!="A50" or successor.get("max_attempts") is not None
            or successor.get("worker_may_delegate") is not False
            or (a52_active and (successor.get("superseded_by")!=["F02e2s"] or successor.get("amendment")!="A52"))):
        errors.append("F02e2r: successor dependencies, budget, ownership, or default attempt policy changed")
    expected_historical=["F02e2","F02e2r"] if a52_active else ["F02e2"]
    if (downstream is None or downstream.get("depends_on")!=[current_successor]
            or downstream.get("historical_depends_on")!=expected_historical):
        errors.append(f"F02e3: dependency must move to {current_successor} while retaining historical F02e2 lineage")
    if (aggregate is None or current_successor not in aggregate.get("depends_on",[])
            or any(x in aggregate.get("depends_on",[]) for x in ("F02e2","F02e2r") if x!=current_successor)
            or (a52_active and aggregate.get("historical_depends_on")!=["F02e2","F02e2r"])):
        errors.append(f"F02e: aggregate must depend on {current_successor} and retain old tasks only as history")
    tests=load(pack/"control/test_matrix.json")["tests"]
    by_id={x.get("id"):x for x in tests}
    old=by_id.get("T014e2",{});new=by_id.get("T014e2r",{});child=by_id.get("T014e2q",{})
    if (old.get("slice")!="F02e2" or old.get("result")!="failed"
            or old.get("binding")!=A48_T014E2_FAILURE_BINDING or old.get("superseded_by")!=(["T014e2s"] if a52_active else ["T014e2r"])):
        errors.append(f"T014e2: preserve the exact A48 failure and map it only to {current_successor}")
    if (new.get("slice")!="F02e2r" or new.get("result")!="not_run"
            or new.get("requirement")!=old.get("requirement") or new.get("level")!=old.get("level")
            or (a52_active and new.get("superseded_by")!=["T014e2s"])):
        errors.append("T014e2r: successor test must be a new not_run row with matching requirement/evidence level")
    if child.get("slice")!="F02e2q" or child.get("result")!="not_run":
        errors.append("T014e2q: retain the separate authored A49 matrix row")
    if "distinct QtTest slot" not in section or "F02e2 remains visibly failed" not in section:
        errors.append("A50: accepted amendment record lacks the distinct slot or deferred supersession boundary")
    try:
        source=load(pack/"control/source-map.execution.json")
        records=[x for x in source.get("execution_amendments",[]) if isinstance(x,dict) and x.get("id")=="A50"]
        if len(records)!=1:
            errors.append("A50: source map must contain exactly one successor ownership record")
        else:
            rec=records[0]
            owners=rec.get("path_owners",{})
            if (rec.get("parent_test")!="T014e2" or rec.get("successor_test")!="T014e2r"
                    or rec.get("attempt_seven_authorized") is not False
                    or rec.get("parent_test_execution_authorized") is not False
                    or rec.get("parent_remains_failed_until_successor_acceptance") is not True
                    or rec.get("fresh_target")!=A50_F02E2R_TARGET
                    or owners.get("F02e2r")!=A50_F02E2_PATHS_ORDERED):
                errors.append("A50: source-map ownership weakens the terminal parent or successor boundary")
    except (OSError,ValueError,TypeError,AttributeError) as exc:
        errors.append(f"A50: cannot validate successor source-map record: {exc}")
    return errors

def validate_successor_recovery_A52(plan,pack=PACK):
    """Bind A52's new bootstrap-pump successor and its two-history supersession gate."""
    errors=[]
    try:
        amendments=(pack/"control/AMENDMENTS.md").read_text()
        section=amendments.split("## A52",1)[1].split("\n## ",1)[0]
        accepted="**Status:** revision 2 independently accepted" in section
    except (IndexError,OSError):
        section="";accepted=False
    record=plan.get("successor_recovery_A52")
    if not accepted:
        if record is not None:errors.append("A52: successor metadata requires its independently accepted amendment record")
        return errors
    paths=A50_F02E2_PATHS_ORDERED
    expected={
        "amendment":"A52","status":"revision 2 independently accepted",
        "review_path":"evidence/A52/PLAN_REVIEW-r2_d28.json",
        "review_sha256":"3bb0e7fb9b4469571b5933c7b69839f5201768d872e0cfad223f1089eb02f6bf",
        "review_budget_path":"evidence/A52/PLAN_REVIEW_BUDGET-r2_d28.json",
        "review_budget_sha256":"37018a443696596ac9d38acee51c0045102893c1767c9bc54d87594ae22a9070",
        "proposal_path":"evidence/A52/PROPOSAL.json",
        "proposal_sha256":"2bfa722adc314c7a1493cb580f0c09d5437dfa77a97d0d114ed2f5ca0791138d",
        "packet_path":"packets/F02e2s-A52-profile-bootstrap-pump.md",
        "packet_sha256":"2cc5e7f6bcbf20bab667a8712b309da00a498d98ba6d4738cd33dcb082a9faee",
        "prior_request_changes_review":{"path":"evidence/A52/PLAN_REVIEW_d28.json","sha256":"8b94ae66c55fed56a59d5df43c0477ca174c1a0c9f38a5d50a4b4b8b981c8280"},
        "failed_A51_trigger":{
            "slice":"F02e2r","attempt":3,"attempt_limit":3,"attempt_increment":False,"test_id":"T014e2r","result":"failed",
            "candidate":{"source_sha":"c6e29480e08f5e7a1ba54319ec2f2fe932d1f366","source_tree":"f5b7696b1c08d115ba435a521fc2a47fa128f538"},
            "continuation_path":"evidence/A51/native/NATIVE_CONTINUATION-A51.json","continuation_sha256":"063b5a7d7983fa416bf9ce199469d580ba4188500e4ad09c5928d8704a6ef3d3",
            "test_receipt_path":"evidence/A51/native/T014e2r/receipt.json","test_receipt_sha256":"da9b7b3f9592cd88278ecbad6c193ac28ec3aed9c730b245274ca006c1c4a142",
            "output_log_path":"evidence/A51/native/T014e2r/output.log","output_log_sha256":"e6963912f20a9ac31a892aaa9bff99e05aaa5483909c5bc26e82d6127c122d07",
            "lease_closure_path":"evidence/A51/native/NATIVE_LEASE_CLOSED-A51.json","lease_closure_sha256":"aee55ef7d9f6fdb2251034e9d5cdb0348194c96005eec61a238ac54b388a5b41",
            "failed_condition":"Combined readiness check false; preserved output does not isolate profile ID, tab_count, or profile_bootstrap_pending."
        },
        "root_cause":{
            "classification":"source-inspection hypothesis only",
            "hypothesis":"The standalone integration test may omit the production poll_config() work needed to finish asynchronous profile bootstrap.",
            "required_diagnostic":"Capture readiness values and startup/status diagnostics separately for both primary and foreign BrowserUi objects."
        },
        "terminal_history":{
            "F02e2":{"status":"failed","attempts":6,"attempt_limit":6,"test_id":"T014e2","attempt_seven_authorized":False,"old_test_execution_authorized":False,"terminal_failure_sha256":"e6b0695df918b20713c8c86d008e99c1ac3f7a68a8852fea40d37d2fd3ae9eb1"},
            "F02e2r":{"status":"failed","attempts":3,"attempt_limit":3,"test_id":"T014e2r","attempt_four_authorized":False,"A50_run_sha256":"69c220a114afb4a05da8dfcf5458292db6d954305cf6900ac4cc8f07111c0906","A50_closure_sha256":"c6e059a05d0ba4271400c1dbaf4723818f05b7abf0c3901cfc0605954ce48739","A51_continuation_sha256":"063b5a7d7983fa416bf9ce199469d580ba4188500e4ad09c5928d8704a6ef3d3","A51_closure_sha256":"aee55ef7d9f6fdb2251034e9d5cdb0348194c96005eec61a238ac54b388a5b41"}
        },
        "successor":{
            "slice":"F02e2s","test_id":"T014e2s","dependencies":["F02e1","F02e2q"],"attempt_limit":3,
            "base_sha":"eb6ff74ad7134ff6fb5a2e934e08fd3f22a777a9","base_tree":"42b809fee461d7a555b79f953fe1c56d6c1a0845",
            "allowed_paths":paths,"locks":A44_LOCKS,"max_changed_product_files":10,
            "fresh_target_pattern":"/home/tom/Projects/ferric-everyday-work/builds/native-F02e2s-aN-qt6112-qmake",
            "native_gate_sequence":A52_NATIVE_GATE_SEQUENCE,
            "pump_contract":{"both_UIs_required":True,"operation":"BrowserUi::poll_config()","bounded_timer":True,"stop_and_disconnect_before_each_UI_destroyed":True,"readiness_faked":False},
            "old_test_execution_authorized":False
        },
        "supersession":{
            "old_slices":["F02e2","F02e2r"],"new_slice":"F02e2s","old_tests":["T014e2","T014e2r"],"new_test":"T014e2s",
            "while_pending":"Keep F02e2 failed at 6/6 and F02e2r failed at 3/3 with all A48/A50/A51 evidence unchanged.",
            "transition":"Only after F02e2s is accepted with an independent review of the exact passing T014e2s run may F02e2 and F02e2r transition to workflow-superseded; keep their terminal failure histories unchanged.",
            "downstream":"F02e3 and aggregate F02e depend on F02e2s and remain blocked until it is accepted."
        },
        "validator_requirements":[
            "Bind exact F02e2s paths, locks, base, test identity, attempt limit and current downstream dependencies.",
            "Preserve A48 F02e2 6/6 failure, A50 F02e2r 3/3 failed run/closure, and A51 failed continuation/receipt/closure with exact hashes.",
            "Reject any attempt counter change, premature old-task supersession, missing historical record, T014e2/T014e2r execution authority, wrong mapping or downstream acceptance.",
            "Permit accepted F02e2s only with passing exact-source T014e2s evidence and independent gate-evidence review; only then permit both old task statuses to become superseded.",
            "Keep A51 validation of its failed continuation active even after F02e2r is superseded under A52."
        ],
        "plan_tooling_paths":["tools/common.py","tools/validate_pack.py","tools/test_tools.py"]
    }
    if record!=expected:errors.append("A52: successor recovery metadata differs from the independently accepted review and failure boundary")
    if any(text not in section for text in (expected["review_sha256"],expected["review_budget_sha256"],expected["proposal_sha256"],expected["packet_sha256"],"T014e2s","F02e2r failed at 3/3")):
        errors.append("A52: accepted amendment text omits its bound plan review, proposal, packet or preserved failure")
    tasks={t.get("id"):t for t in plan.get("slices",[]) if isinstance(t,dict)}
    parent=tasks.get("F02e2");old=tasks.get("F02e2r");successor=tasks.get("F02e2s");downstream=tasks.get("F02e3");aggregate=tasks.get("F02e")
    if (parent is None or parent.get("max_attempts")!=6 or parent.get("test_ids")!=["T014e2"]
            or parent.get("superseded_by")!=["F02e2s"] or parent.get("amendment")!="A52"):
        errors.append("F02e2: A52 must preserve the six-attempt parent and map it only to F02e2s")
    if (old is None or old.get("depends_on")!=["F02e1","F02e2q"] or old.get("test_ids")!=["T014e2r"]
            or old.get("superseded_by")!=["F02e2s"] or old.get("amendment")!="A52"
            or old.get("allowed_paths")!=paths or old.get("locks")!=A44_LOCKS or old.get("max_attempts") is not None):
        errors.append("F02e2r: A52 must retain the exact exhausted A50/A51 owner and map it only to F02e2s")
    if (successor is None or successor.get("depends_on")!=["F02e1","F02e2q"]
            or successor.get("test_ids")!=["T014e2s"] or successor.get("allowed_paths")!=paths
            or successor.get("locks")!=A44_LOCKS or successor.get("max_changed_product_files")!=10
            or successor.get("path_budget_amendment")!="A52" or successor.get("max_attempts") is not None
            or successor.get("worker_may_delegate") is not False or "both" not in " ".join(successor.get("steps",[])).lower()
            or not any("poll_config" in x and "disconnect" in x.lower() for x in successor.get("steps",[]))):
        errors.append("F02e2s: successor dependencies, bootstrap pump, test ownership, paths, locks or attempt policy changed")
    if (downstream is None or downstream.get("depends_on")!=["F02e2s"]
            or downstream.get("historical_depends_on")!=["F02e2","F02e2r"]):
        errors.append("F02e3: downstream must wait for F02e2s and retain both failed task IDs as history")
    expected_aggregate=["F02e0","F02e1","F02e2s","F02e3","F02e4","F02e5","F02e6","F02e7","F02e8"]
    if (aggregate is None or aggregate.get("depends_on")!=expected_aggregate
            or aggregate.get("historical_depends_on")!=["F02e2","F02e2r"]):
        errors.append("F02e: aggregate must wait for F02e2s and retain both failed task IDs as history")
    tests=load(pack/"control/test_matrix.json")["tests"];by_id={x.get("id"):x for x in tests}
    parent_test=by_id.get("T014e2",{});old_test=by_id.get("T014e2r",{});new_test=by_id.get("T014e2s",{});child=by_id.get("T014e2q",{})
    if (parent_test.get("result")!="failed" or parent_test.get("binding")!=A48_T014E2_FAILURE_BINDING
            or parent_test.get("superseded_by")!=["T014e2s"]):
        errors.append("T014e2: retain the exact A48 failure and map it only to T014e2s")
    if (old_test.get("slice")!="F02e2r" or old_test.get("result")!="not_run"
            or old_test.get("requirement")!=parent_test.get("requirement") or old_test.get("level")!=parent_test.get("level")
            or old_test.get("superseded_by")!=["T014e2s"]):
        errors.append("T014e2r: retain the not_run A51 history and map it only to T014e2s")
    if (new_test.get("slice")!="F02e2s" or new_test.get("result")!="not_run"
            or new_test.get("requirement")!=parent_test.get("requirement") or new_test.get("level")!=parent_test.get("level")
            or "both" not in new_test.get("expected","").lower() or "poll_config" not in new_test.get("expected","")
            or "T014e2r" not in new_test.get("binding","") or "zero skips" not in new_test.get("binding","")
            or "independent gate-evidence review" not in new_test.get("binding","")
            or new_test.get("native_gate_sequence")!=A52_NATIVE_GATE_SEQUENCE
            or new_test.get("superseded_by") is not None):
        errors.append("T014e2s: require the distinct not_run bootstrap-pump test with its exact gate boundary")
    if child.get("slice")!="F02e2q" or child.get("result")!="not_run":
        errors.append("T014e2q: preserve the separate A49 registration test")
    try:
        source=load(pack/"control/source-map.execution.json")
        records=[x for x in source.get("execution_amendments",[]) if isinstance(x,dict) and x.get("id")=="A52"]
        expected_map={
            "id":"A52",
            "reason":"A51 T014e2r failed its combined profile-readiness condition. A fresh successor drives the poll_config() work used by production and preserves both exhausted histories.",
            "added_paths":[],"removed_paths":[],"added_locks":[],"locks":A44_LOCKS,
            "path_owners":{"F02e2s":paths},"max_changed_product_files":10,"previous_max_changed_product_files":10,
            "base_sha":"eb6ff74ad7134ff6fb5a2e934e08fd3f22a777a9","base_tree":"42b809fee461d7a555b79f953fe1c56d6c1a0845","attempt_limit":3,
            "successor_test":"T014e2s","old_tests_execution_authorized":False,
            "pump_contract":{"both_BrowserUi_instances":True,"operation":"BrowserUi::poll_config()","bounded":True,"stops_before_destruction":True},
            "preserved_failures":{"F02e2":"A48 failed 6/6","F02e2r":"A50 attempt 3/3 and A51 continuation failed"},
            "supersession":{"F02e2":"F02e2s","F02e2r":"F02e2s","T014e2":"T014e2s","T014e2r":"T014e2s","only_after_independent_T014e2s_gate_review":True},
            "downstream_dependencies":{"F02e3":["F02e2s"],"F02e":["F02e0","F02e1","F02e2s","F02e3","F02e4","F02e5","F02e6","F02e7","F02e8"]},
            "native_gate_sequence":A52_NATIVE_GATE_SEQUENCE
        }
        if len(records)!=1 or records[0]!=expected_map:errors.append("A52: source map differs from the reviewed path, pump, history or dependency boundary")
    except (OSError,ValueError,TypeError,AttributeError) as exc:errors.append(f"A52: cannot validate source map: {exc}")
    try:
        initial=load(pack/"control/state.initial.json"); states=initial["slices"]
        if (states.get("F02e2s",{}).get("status")!="not_started" or states.get("F02e2s",{}).get("attempts")!=0
                or states.get("F02e2",{}).get("terminal_failure")!=A50_TERMINAL_FAILURE
                or states.get("F02e2",{}).get("supersession")!={"amendment":"A52","superseded_by":["F02e2s"],"transition":"pending_successor_acceptance"}
                or states.get("F02e2r",{}).get("continuation_policy")!=A51_STATE_POLICY
                or states.get("F02e2r",{}).get("supersession")!={"amendment":"A52","superseded_by":["F02e2s"],"transition":"pending_successor_acceptance"}):
            errors.append("state.initial: add F02e2s and preserve both A48/A51 histories under pending A52 supersession")
    except (OSError,KeyError,TypeError,ValueError) as exc:errors.append(f"A52: cannot validate initial state: {exc}")
    return errors

def validate_continuation_A51(plan,pack=PACK):
    """Bind A51 to the exact unrun gates and immutable A50 attempt-3 artifacts."""
    errors=[]
    try:
        text=(pack/"control/AMENDMENTS.md").read_text()
        section=text.split("## A51",1)[1].split("\n## ",1)[0]
        accepted="**Status:** revision 2 independently accepted" in section
    except (IndexError,OSError):
        section="";accepted=False
    record=plan.get("native_continuation_A51")
    if not accepted:
        if record is not None:errors.append("A51: continuation metadata requires its independently accepted amendment record")
        return errors
    if record!=A51_PLAN_RECORD:
        errors.append("A51: continuation metadata differs from the reviewed exact candidate/artifact/gate scope")
    tasks={t.get("id"):t for t in plan.get("slices",[]) if isinstance(t,dict)}
    successor=tasks.get("F02e2r")
    if successor is None or not any("A51 permits only read-only verification" in x for x in successor.get("steps",[])):
        errors.append("F02e2r: A51 continuation step is missing")
    tests=load(pack/"control/test_matrix.json")["tests"]
    by_id={x.get("id"):x for x in tests}
    if by_id.get("T014e2r",{}).get("result")!="not_run":
        errors.append("T014e2r: keep the authored matrix row not_run; bind continuation only in supplemental evidence")
    if by_id.get("T014e2",{}).get("result")!="failed":
        errors.append("T014e2: A51 must not alter or execute the historical failed test")
    try:
        initial=load(pack/"control/state.initial.json")
        f02e2r=initial["slices"]["F02e2r"]
        if (f02e2r.get("status")!="not_started" or f02e2r.get("attempts")!=0
                or f02e2r.get("continuation_policy")!=A51_STATE_POLICY):
            errors.append("state.initial F02e2r: preserve the zero-attempt template and exact A51 continuation policy")
    except (OSError,KeyError,TypeError,ValueError) as exc:
        errors.append(f"A51: cannot validate initial-state continuation policy: {exc}")
    try:
        source=load(pack/"control/source-map.execution.json")
        records=[x for x in source.get("execution_amendments",[]) if isinstance(x,dict) and x.get("id")=="A51"]
        expected={
            "id":"A51",
            "reason":"Narrow postmortem-bound continuation of only the unrun A50 attempt-3 T014e2r gates after the independent review confirmed the link checker false negative; no attempt increment or source/build work.",
            "added_paths":[],"removed_paths":[],"added_locks":[],
            "continuation":{"slice":"F02e2r","source_amendment":"A50","attempt":3,"attempt_limit":3,"attempt_increment":False,"test_id":"T014e2r","candidate":A51_CANDIDATE,"target":A51_TARGET,"artifact_hashes":A51_ARTIFACTS,"preserved_run":A51_A50_RUN,"preserved_lease_closure":A51_A50_CLOSURE,"postmortem":{"path":"evidence/F02e2r-a3/GATE_POSTMORTEM_REVIEW_d28.json","sha256":"2babd0ab2103da96508f101e971555c80a01fa0d967ac3dbb42ea5474572d223","decision":"stop_request_new_reviewed_amendment_before_continuation"},"allowed_gates":A51_ALLOWED_GATES,"prohibited":A51_PROHIBITED},
            "test_matrix_row_remains":"not_run","independent_gate_evidence_review_required":True,
            "F02e2_remains_failed_until_F02e2r_acceptance":True,"F02e3_remains_blocked_until_F02e2r_acceptance":True,
        }
        if len(records)!=1 or records[0]!=expected:
            errors.append("A51: source-map continuation record differs from the reviewed exact gate boundary")
    except (OSError,ValueError,TypeError,AttributeError) as exc:
        errors.append(f"A51: cannot validate continuation source map: {exc}")
    if ("NATIVE_RUN-A50-a3.json" not in section or "T014e2r" not in section
            or "attempt 4" not in section or "F02e2 remains failed at 6/6" not in section):
        errors.append("A51: accepted amendment record omits the immutable failed attempt, test boundary, or parent status")
    return errors

def validate_continuation_state_A51(state,root):
    """Require separately reviewed supplemental evidence before A51 acceptance."""
    errors=[]
    record=state.get("slices",{}).get("F02e2r",{})
    continuation=record.get("A51_continuation")
    if continuation is None:
        if record.get("status")=="accepted":
            errors.append("F02e2r: A51 acceptance requires a separately reviewed continuation record")
        return errors
    if record.get("attempts")!=3:
        errors.append("F02e2r: A51 continuation must preserve attempt 3/3 without increment")
    expected={
        "amendment":"A51","source_amendment":"A50","attempt":3,"attempt_limit":3,
        "attempt_count_after":3,"candidate":A51_CANDIDATE,"artifacts":A51_ARTIFACTS,
        "preserved_A50_run":A51_A50_RUN,"preserved_A50_lease_closure":A51_A50_CLOSURE,
        "product_tests_run":["T014e2r"],"old_T014e2_ran":False,
        "source_changed":False,"build_or_configure_ran":False,
    }
    for key,value in expected.items():
        if continuation.get(key)!=value:
            errors.append(f"F02e2r: A51 continuation field {key} differs from the reviewed exact scope")
    outcome=continuation.get("result")
    successor_accepted=state.get("slices",{}).get("F02e2s",{}).get("status")=="accepted"
    superseded_after_A52=(record.get("status")=="superseded" and successor_accepted
        and record.get("supersession")=={"amendment":"A52","superseded_by":["F02e2s"],"transition":"successor_accepted"})
    if outcome=="failed":
        if record.get("status")!="failed" and not superseded_after_A52:
            errors.append("F02e2r: failed A51 continuation must remain failed until F02e2s acceptance")
        path=continuation.get("run_path")
        if not path or not continuation.get("run_sha256"):
            errors.append("F02e2r: missing failed A51 continuation run binding")
            return errors
        try:
            run_path=safe_relative(root,path)
            if digest(run_path)!=continuation["run_sha256"]:raise ValueError("failed continuation run digest mismatch")
            run=load(run_path)
            if (run.get("amendment")!="A51" or run.get("result")!="failed"
                    or run.get("source_sha")!=A51_CANDIDATE["source_sha"]
                    or run.get("source_tree")!=A51_CANDIDATE["source_tree"]
                    or run.get("attempt")!=3 or run.get("attempt_incremented") is not False
                    or run.get("source_changed") is not False
                    or run.get("build_or_configure_ran") is not False
                    or run.get("artifacts")!=A51_ARTIFACTS
                    or run.get("preserved_A50_run")!=A51_A50_RUN
                    or run.get("preserved_A50_lease_closure")!=A51_A50_CLOSURE):
                raise ValueError("failed continuation run does not preserve the exact A3 boundary")
            if any(t not in {"T014e2r"} for t in run.get("product_tests_run",[])):
                raise ValueError("failed continuation ran a prohibited test")
            if not run.get("gates") or run["gates"][-1].get("result")!="failed":
                raise ValueError("failed continuation lacks a failed terminal gate")
            if any(g.get("name") not in {"read-only-exact-artifact-verification","nonzero-ctest-discovery","T014e2r-serial"} for g in run["gates"]):
                raise ValueError("failed continuation contains a prohibited gate")
            closure=safe_relative(root,continuation["lease_closure_path"])
            if digest(closure)!=continuation["lease_closure_sha256"]:raise ValueError("A51 lease closure digest mismatch")
            closed=load(closure)
            if closed.get("active_lock_released") is not True or closed.get("active_build_processes_remaining")!=[]:
                raise ValueError("A51 continuation lease is not fully closed")
            for preserved in (A51_A50_RUN,A51_A50_CLOSURE):
                evidence_path=safe_relative(root,preserved["path"])
                if digest(evidence_path)!=preserved["sha256"]:raise ValueError("immutable A50 run or lease closure changed")
        except (OSError,KeyError,TypeError,ValueError) as exc:
            errors.append(f"F02e2r: {exc}")
        return errors
    if outcome!="passed":errors.append("F02e2r: A51 continuation outcome must be passed or failed")
    if record.get("status")!="accepted" and not superseded_after_A52:
        errors.append("F02e2r: a passing A51 continuation may qualify only after independent evidence review")
    path=continuation.get("run_path")
    if not path or not continuation.get("run_sha256"):
        errors.append("F02e2r: missing supplemental A51 continuation run binding")
        return errors
    try:
        run_path=safe_relative(root,path)
        if digest(run_path)!=continuation["run_sha256"]:raise ValueError("continuation run digest mismatch")
        run=load(run_path)
        if (run.get("amendment")!="A51" or run.get("result")!=outcome
                or run.get("source_sha")!=A51_CANDIDATE["source_sha"]
                or run.get("source_tree")!=A51_CANDIDATE["source_tree"]
                or run.get("attempt")!=3 or run.get("attempt_incremented") is not False
                or run.get("product_tests_run")!=["T014e2r"]
                or run.get("source_changed") is not False
                or run.get("build_or_configure_ran") is not False
                or run.get("artifacts")!=A51_ARTIFACTS
                or run.get("preserved_A50_run")!=A51_A50_RUN
                or run.get("preserved_A50_lease_closure")!=A51_A50_CLOSURE):
            raise ValueError("continuation run does not preserve the exact A3 boundary")
        gates=run.get("gates",[])
        names=[g.get("name") for g in gates]
        if names!=["read-only-exact-artifact-verification","nonzero-ctest-discovery","T014e2r-serial"]:
            raise ValueError("continuation run contains missing, reordered, or extra gates")
        discovery=gates[1]
        test=gates[2]
        if (discovery.get("result")!="passed" or not discovery.get("T014e2r_registered")
                or discovery.get("registered_tests",0)<=0):
            raise ValueError("T014e2r discovery was not proven")
        if (test.get("result")!="passed" or test.get("test_id")!="T014e2r"
                or test.get("failed",0)!=0 or test.get("skipped",0)!=0):
            raise ValueError("T014e2r did not pass exactly once with zero skips")
        for obj in (discovery,test):
            receipt=safe_relative(root,obj["receipt"])
            if digest(receipt)!=obj["receipt_sha256"]:raise ValueError("continuation gate receipt digest mismatch")
        review_path=safe_relative(root,continuation["review_path"])
        if digest(review_path)!=continuation["review_sha256"]:raise ValueError("A51 gate review digest mismatch")
        review=load(review_path)
        if (review.get("decision")!="accept" or review.get("continuation_run_sha256")!=continuation["run_sha256"]
                or review.get("amendment")!="A51"):
            raise ValueError("independent A51 gate-evidence review does not accept this continuation")
        closure=safe_relative(root,continuation["lease_closure_path"])
        if digest(closure)!=continuation["lease_closure_sha256"]:raise ValueError("A51 lease closure digest mismatch")
        closed=load(closure)
        if (closed.get("active_lock_released") is not True
                or closed.get("active_build_processes_remaining")!=[]):
            raise ValueError("A51 continuation lease is not fully closed")
        for preserved in (A51_A50_RUN,A51_A50_CLOSURE):
            evidence_path=safe_relative(root,preserved["path"])
            if digest(evidence_path)!=preserved["sha256"]:
                raise ValueError("immutable A50 run or lease closure changed")
    except (OSError,KeyError,TypeError,ValueError) as exc:
        errors.append(f"F02e2r: {exc}")
    return errors

def validate_edit_path_budget(task,pack=PACK):
    """Enforce the eight-path default and reviewed F02e1/F02e2 lineage exceptions."""
    errors=[];paths=task.get("allowed_paths",[]);cap=task.get("max_changed_product_files",8)
    amendment=task.get("path_budget_amendment")
    if amendment is None:
        if len(paths)>8 or cap>8:
            errors.append(f"{task.get('id')}: edit-path budget exceeded")
        return errors
    if task.get("id")=="F02e2r" and amendment=="A50":
        if len(paths)!=10 or set(paths)!=A44_F02E2_PATHS or cap!=10:
            errors.append("F02e2r: A50 may reuse only the exact A44 ten-path F02e2 scope")
        if task.get("locks")!=A44_LOCKS:
            errors.append("F02e2r: A50 requires the exact A44 four-lock set")
        if task.get("max_attempts") is not None:
            errors.append("F02e2r: use the global default three-attempt limit")
        try:
            text=(pack/"control/AMENDMENTS.md").read_text()
            section=text.split("## A50",1)[1].split("\n## ",1)[0]
            if "**Status:** revision 2 independently accepted" not in section:
                errors.append("F02e2r: A50 path reuse requires its accepted amendment record")
        except (IndexError,OSError):
            errors.append("F02e2r: A50 accepted amendment record is missing")
        return errors
    if task.get("id")=="F02e2s" and amendment=="A52":
        if len(paths)!=10 or set(paths)!=A44_F02E2_PATHS or cap!=10:
            errors.append("F02e2s: A52 may reuse only the exact A44/A50 ten-path F02e2 scope")
        if task.get("locks")!=A44_LOCKS:
            errors.append("F02e2s: A52 requires the exact A44/A50 four-lock set")
        if task.get("max_attempts") is not None:
            errors.append("F02e2s: use the global default three-attempt limit")
        try:
            text=(pack/"control/AMENDMENTS.md").read_text()
            section=text.split("## A52",1)[1].split("\n## ",1)[0]
            if "**Status:** revision 2 independently accepted" not in section:
                errors.append("F02e2s: A52 path reuse requires its accepted amendment record")
        except (IndexError,OSError):
            errors.append("F02e2s: A52 accepted amendment record is missing")
        return errors
    if task.get("id")=="F02e2" and amendment=="A44":
        if len(paths)!=10 or set(paths)!=A44_F02E2_PATHS or cap!=10:
            errors.append("F02e2: A44 permits only its exact ten-path owner list")
        notice=task.get("new_paths_notice","")
        if ("A44" not in notice or "browser_ui_input.rs" not in notice
                or "tenth" not in notice or "A42" not in notice):
            errors.append("F02e2: A44 path exception lacks its exact tenth-path notice")
        if task.get("locks")!=A44_LOCKS:
            errors.append("F02e2: A44 requires the three retained locks plus browser-ui-input")
        try:
            amendment_text=(pack/"control/AMENDMENTS.md").read_text()
            section=amendment_text.split("## A44",1)[1].split("\n## ",1)[0]
            source_map=load(pack/"control/source-map.execution.json")
            records={x.get("id"):x for x in source_map.get("execution_amendments",[])}
            a42=records.get("A42");a44=records.get("A44")
            a42_owner=a42.get("path_owners",{}).get("F02e2") if a42 else None
            a44_owner=a44.get("path_owners",{}).get("F02e2") if a44 else None
            if ("**Status:** independently reviewed and accepted" not in section
                    or "browser_ui_input.rs" not in section):
                errors.append("F02e2: A44 path exception is missing its accepted amendment record")
            if (not isinstance(a42_owner,list) or len(a42_owner)!=9
                    or set(a42_owner)!=A42_F02E2_PATHS):
                errors.append("F02e2: A44 must preserve A42's exact historical nine-path owner record")
            if (a44 is None or a44.get("added_paths")!=[A44_ADDED_PATH]
                    or a44.get("removed_paths")!=[] or a44_owner!=paths
                    or a44.get("added_locks")!=["browser-ui-input"]
                    or a44.get("locks")!=A44_LOCKS
                    or a44.get("max_changed_product_files")!=10
                    or a44.get("previous_max_changed_product_files")!=9):
                errors.append("F02e2: A44 source-map owner record does not match its exact tenth-path scope")
        except (IndexError,OSError,ValueError,TypeError) as exc:
            errors.append(f"F02e2: cannot validate A44 path amendment metadata: {exc}")
        return errors
    if task.get("id")=="F02e2" and amendment=="A42":
        if len(paths)!=9 or set(paths)!=A42_F02E2_PATHS or cap!=9:
            errors.append("F02e2: A42 permits only its exact nine-path owner list")
        notice=task.get("new_paths_notice","")
        if "A42" not in notice or "engine_requests.rs" not in notice or "ninth" not in notice:
            errors.append("F02e2: A42 path exception lacks its exact nine-path notice")
        try:
            amendment_text=(pack/"control/AMENDMENTS.md").read_text()
            section=amendment_text.split("## A42",1)[1].split("\n## ",1)[0]
            source_map=load(pack/"control/source-map.execution.json")
            record=next((x for x in source_map.get("execution_amendments",[]) if x.get("id")=="A42"),None)
            if ("**Status:** independently reviewed and accepted" not in section
                    or "engine_requests.rs" not in section):
                errors.append("F02e2: A42 path exception is missing its accepted amendment record")
            if (record is None or record.get("added_paths")!=A42_ADDED_PATHS
                    or record.get("removed_paths")!=A42_REMOVED_PATHS
                    or record.get("path_owners",{}).get("F02e2")!=paths
                    or record.get("max_changed_product_files")!=9
                    or record.get("previous_max_changed_product_files")!=8):
                errors.append("F02e2: A42 source-map owner record does not match its exact nine-path scope")
        except (IndexError,OSError,ValueError,TypeError) as exc:
            errors.append(f"F02e2: cannot validate A42 path amendment metadata: {exc}")
        return errors
    if task.get("id")!="F02e1" or amendment!="A38":
        return [f"{task.get('id')}: unapproved edit-path budget amendment {amendment}"]
    if len(paths)!=9 or set(paths)!=A38_F02E1_PATHS or cap!=9:
        errors.append("F02e1: A38 permits only its exact nine-path owner list")
    notice=task.get("new_paths_notice","")
    if "A38" not in notice or A38_ADDED_PATH not in notice:
        errors.append("F02e1: A38 path exception lacks its exact path notice")
    try:
        amendment_text=(pack/"control/AMENDMENTS.md").read_text()
        source_map=load(pack/"control/source-map.execution.json")
        record=next((x for x in source_map.get("execution_amendments",[]) if x.get("id")=="A38"),None)
        if "### A38" not in amendment_text or A38_ADDED_PATH not in amendment_text:
            errors.append("F02e1: A38 path exception is missing its amendment record")
        if (record is None or record.get("added_paths")!=[A38_ADDED_PATH]
                or record.get("path_owners",{}).get("F02e1")!=task.get("allowed_paths")):
            errors.append("F02e1: A38 source-map owner record does not match its exact path list")
    except (OSError,ValueError,TypeError) as exc:
        errors.append(f"F02e1: cannot validate A38 path amendment metadata: {exc}")
    return errors

def validate_attempt_limits(plan,tasks):
    errors=[];default=plan.get("limits",{}).get("max_attempts")
    if not isinstance(default,int) or isinstance(default,bool) or default!=GLOBAL_MAX_ATTEMPTS:
        return [f"Global max_attempts must remain {GLOBAL_MAX_ATTEMPTS}"]
    exceptions=plan.get("attempt_exceptions",{})
    if not isinstance(exceptions,dict):return ["attempt_exceptions must be an object"]
    for sid in exceptions:
        if sid not in tasks:errors.append(f"attempt exception for unknown task {sid}")
        if sid not in ATTEMPT_EXCEPTION_TASKS:errors.append(f"attempt exception is not authorized for {sid}")
    for sid,t in tasks.items():
        limit=t.get("max_attempts",default)
        if not isinstance(limit,int) or isinstance(limit,bool) or limit<1:
            errors.append(f"{sid}: max_attempts must be a positive integer");continue
        entry=exceptions.get(sid)
        if entry is None:
            if limit>default:errors.append(f"{sid}: max_attempts increase lacks explicit attempt exception")
            continue
        if not isinstance(entry,dict):
            errors.append(f"{sid}: attempt exception must be an object");continue
        if set(entry)!={"amendment","default_limit","authorized_limit","reason"}:
            errors.append(f"{sid}: attempt exception fields must be amendment/default_limit/authorized_limit/reason")
        expected_amendment=ATTEMPT_EXCEPTION_AMENDMENTS.get(sid)
        if expected_amendment is None or entry.get("amendment")!=expected_amendment:
            errors.append(f"{sid}: attempt exception must cite {expected_amendment or 'an authorized amendment'}")
        if entry.get("default_limit")!=default:
            errors.append(f"{sid}: attempt exception default_limit differs from global limit")
        expected_limit=ATTEMPT_EXCEPTION_LIMITS.get(sid)
        if entry.get("authorized_limit")!=limit or limit!=expected_limit:
            errors.append(f"{sid}: attempt exception must authorize exactly {expected_limit or 'the reviewed'} limit matching max_attempts")
        if not isinstance(entry.get("reason"),str) or not entry["reason"].strip():
            errors.append(f"{sid}: attempt exception requires a nonempty reason")
    return errors

def validate_native_gate_retries(plan,pack=PACK):
    """Validate historical A45-A47 retries; A48 separately authorizes one final source attempt."""
    errors=[]
    if not isinstance(plan,dict):return ["plan must be an object"]
    entries=plan.get("native_gate_retries",{})
    if not isinstance(entries,dict):return ["native_gate_retries must be an object"]
    accepted_a45=False
    try:
        amendments=(pack/"control/AMENDMENTS.md").read_text()
        section=amendments.split("## A45",1)[1].split("\n## ",1)[0]
        accepted_a45=("**Status:** independently reviewed and accepted" in section)
        a48_section=amendments.split("## A48",1)[1].split("\n## ",1)[0]
        accepted_a48=("**Status:** independently reviewed and accepted" in a48_section)
    except (IndexError,OSError):
        section=""
        accepted_a48=False
    if accepted_a45 and "F02e2" not in entries:
        errors.append("F02e2: accepted A45 amendment lacks its exact native-gate retry entry")
    for sid in entries:
        if sid!="F02e2":errors.append(f"native gate retry is not authorized for {sid}")
    entry=entries.get("F02e2")
    if entry is None:return errors
    expected={
        "amendment":"A45","source_attempt":5,"max_same_source_retries":1,
        "candidate_sha":A45_CANDIDATE_SHA,"candidate_tree":A45_CANDIDATE_TREE,
        "trigger":"A5 native gate stopped after Qt C++ flags selected /usr/include/qt6 instead of the pinned 6.11.2 SDK.",
        "same_source_only":True,"source_changes_allowed":False,"attempt_six_authorized":False,
        "fresh_target":A45_FRESH_TARGET,
        "cargo_target_dir":f"{A45_FRESH_TARGET}/cargo-target",
        "cmake_dir":f"{A45_FRESH_TARGET}/cmake",
        "qmake":f"{A45_QT_PREFIX}/bin/qmake","prepend_path":f"{A45_QT_PREFIX}/bin",
        "pkg_config_path":f"{A45_QT_PREFIX}/lib/pkgconfig",
        "pkg_config_libdir":f"{A45_QT_PREFIX}/lib/pkgconfig",
        "cmake_prefix_path":A45_QT_PREFIX,"qt6_dir":f"{A45_QT_PREFIX}/lib/cmake/Qt6",
        "strict_qt_guard_tool":"tools/run_gate.py --qt-include-prefix",
        "emit_cxx_commands_env":"CC_ENABLE_DEBUG_OUTPUT=1",
        "stop_if_qt_include_outside_prefix":True,"only_test_id":"T014e2"
    }
    if not isinstance(entry,dict) or set(entry)!=set(expected):
        errors.append("F02e2: A45 native retry must have exactly the reviewed fields")
        return errors
    for key,value in expected.items():
        if entry.get(key)!=value:errors.append(f"F02e2: A45 native retry has invalid {key}")
    task=next((t for t in plan.get("slices",[]) if t.get("id")=="F02e2"),None)
    expected_source_limit=6 if accepted_a48 else 5
    if (task is None or task.get("max_attempts")!=expected_source_limit or task.get("path_budget_amendment")!="A44"
            or len(task.get("allowed_paths",[]))!=10 or task.get("locks")!=A44_LOCKS):
        errors.append("F02e2: A45 must retain A44's historical 5/5 source record and exact ten paths/four locks; any sixth attempt requires accepted A48")
    if task is None:return errors
    if ("**Status:** independently reviewed and accepted" not in section
            or "QMAKE" not in section or "attempt 6" not in section):
        errors.append("F02e2: A45 native retry lacks its accepted amendment record")
    try:
        source_map=load(pack/"control/source-map.execution.json")
        if not isinstance(source_map,dict):
            errors.append("F02e2: source-map execution metadata must be an object")
            return errors
        amendment_records=source_map.get("execution_amendments",[])
        if not isinstance(amendment_records,list):
            errors.append("F02e2: source-map execution_amendments must be a list")
            return errors
        a44_records=[x for x in amendment_records if isinstance(x,dict) and x.get("id")=="A44"]
        a45_records=[x for x in amendment_records if isinstance(x,dict) and x.get("id")=="A45"]
        if len(a44_records)!=1 or len(a45_records)!=1:
            errors.append("F02e2: source-map must contain exactly one A44 record and one A45 retry record")
            return errors
        a44=a44_records[0];a45=a45_records[0]
        owner=a45.get("path_owners",{}).get("F02e2") if a45 else None
        if (a45 is None or a45.get("added_paths")!=[] or a45.get("removed_paths")!=[]
                or a45.get("added_locks")!=[] or a45.get("locks")!=A44_LOCKS
                or a45.get("path_owners",{}).get("F02e2")!=task.get("allowed_paths")
                or a45.get("max_changed_product_files")!=10
                or a45.get("previous_max_changed_product_files")!=10
                or a45.get("source_attempt_count")!=5
                or a45.get("max_same_source_retries")!=1
                or a45.get("same_source_only") is not True
                or a45.get("source_changes_allowed") is not False
                or a45.get("attempt_six_authorized") is not False
                or a45.get("candidate_sha")!=A45_CANDIDATE_SHA
                or a45.get("candidate_tree")!=A45_CANDIDATE_TREE
                or a45.get("fresh_target")!=expected["fresh_target"]
                or a45.get("cargo_target_dir")!=expected["cargo_target_dir"]
                or a45.get("cmake_dir")!=expected["cmake_dir"]
                or a45.get("qmake")!=expected["qmake"]
                or a45.get("prepend_path")!=expected["prepend_path"]
                or a45.get("pkg_config_path")!=expected["pkg_config_path"]
                or a45.get("pkg_config_libdir")!=expected["pkg_config_libdir"]
                or a45.get("cmake_prefix_path")!=expected["cmake_prefix_path"]
                or a45.get("qt6_dir")!=expected["qt6_dir"]
                or a45.get("strict_qt_guard_tool")!=expected["strict_qt_guard_tool"]
                or a45.get("emit_cxx_commands_env")!=expected["emit_cxx_commands_env"]
                or a45.get("stop_if_qt_include_outside_prefix") is not True
                or a45.get("only_test_id")!="T014e2"
                or not isinstance(owner,list) or len(owner)!=10
                or a44 is None or a44.get("path_owners",{}).get("F02e2")!=owner):
            errors.append("F02e2: A45 source-map retry record must preserve A44 scope and exact same-source environment exception")
    except (OSError,ValueError,TypeError,AttributeError) as exc:
        errors.append(f"F02e2: cannot validate A45 native retry metadata: {exc}")
    return errors

def validate_source_attempt_extension_A48(plan,pack=PACK):
    """Accept exactly one reviewed F02e2 attempt 6 while preserving A47's terminal retry record."""
    errors=[]
    try:
        amendments=(pack/"control/AMENDMENTS.md").read_text()
        section=amendments.split("## A48",1)[1].split("\n## ",1)[0]
        accepted=("**Status:** independently reviewed and accepted" in section)
    except (IndexError,OSError):
        section="";accepted=False
    extensions=plan.get("source_attempt_extensions_A48")
    if not accepted:
        if extensions is not None:
            errors.append("F02e2: A48 source-attempt extension requires its accepted amendment record")
        return errors
    if not isinstance(extensions,dict) or set(extensions)!={"F02e2"}:
        return ["F02e2: A48 source-attempt extension is authorized only for F02e2"]
    record=extensions["F02e2"]
    expected={
        "amendment":"A48","prior_limit":5,"authorized_limit":6,
        "prior_attempts_used":5,"attempt_number":6,
        "base_sha":A48_BASE_SHA,"base_tree":A48_BASE_TREE,
        "allowed_source_paths":[A48_SOURCE_PATH],"max_source_paths_changed":1,
        "allowed_source_change":"add #include <QQuickWidget>",
        "product_path_cap":10,"locks":A44_LOCKS,
        "attempt_seven_authorized":False,
        "same_source_retry_after_failure_authorized":False,
        "native_target":A48_NATIVE_TARGET,"test_id":"T014e2",
        "tooling_paths":A48_TOOLING_PATHS,"tooling_file_cap":3,
        "source_review_before_native":True,"stop_after_any_failure":True,
    }
    if not isinstance(record,dict) or set(record)!=set(expected):
        errors.append("F02e2: A48 source-attempt extension must have exactly the reviewed fields")
        return errors
    for key,value in expected.items():
        if record.get(key)!=value:errors.append(f"F02e2: A48 source-attempt extension has invalid {key}")
    task=next((t for t in plan.get("slices",[]) if t.get("id")=="F02e2"),None)
    if (task is None or task.get("max_attempts")!=6 or task.get("path_budget_amendment")!="A44"
            or task.get("max_changed_product_files")!=10 or task.get("allowed_paths") is None
            or len(task.get("allowed_paths",[]))!=10 or task.get("locks")!=A44_LOCKS
            or A48_SOURCE_PATH not in task.get("allowed_paths",[])):
        errors.append("F02e2: A48 must preserve the exact A44 ten-path/four-lock task and set only the attempt limit to 6")
    if ("attempt 7" not in section.lower() or "T014e2" not in section
            or "QQuickWidget" not in section or "zero skips" not in section):
        errors.append("F02e2: A48 accepted amendment record lacks the exact source repair, terminal limit, or native gate")
    try:
        source_map=load(pack/"control/source-map.execution.json")
        records=[x for x in source_map.get("execution_amendments",[]) if isinstance(x,dict) and x.get("id")=="A48"]
        if len(records)!=1:
            errors.append("F02e2: source-map must contain exactly one A48 attempt record")
        else:
            a48=records[0]
            owners=a48.get("path_owners",{}).get("F02e2")
            if (a48.get("added_paths")!=[] or a48.get("removed_paths")!=[]
                    or a48.get("added_locks")!=[] or a48.get("locks")!=A44_LOCKS
                    or owners!=task.get("allowed_paths")
                    or a48.get("max_changed_product_files")!=10
                    or a48.get("previous_max_changed_product_files")!=10
                    or a48.get("source_attempt_count")!=6
                    or a48.get("previous_source_attempt_limit")!=5
                    or a48.get("attempt_seven_authorized") is not False
                    or a48.get("same_source_retry_after_failure_authorized") is not False
                    or a48.get("candidate_base_sha")!=A48_BASE_SHA
                    or a48.get("candidate_base_tree")!=A48_BASE_TREE
                    or a48.get("allowed_source_paths")!=[A48_SOURCE_PATH]
                    or a48.get("max_source_paths_changed")!=1
                    or a48.get("allowed_source_change")!="add #include <QQuickWidget>"
                    or a48.get("native_target")!=A48_NATIVE_TARGET
                    or a48.get("tooling_paths")!=A48_TOOLING_PATHS
                    or a48.get("tooling_file_cap")!=3):
                errors.append("F02e2: A48 source-map record widens the reviewed attempt or path scope")
    except (OSError,ValueError,TypeError,AttributeError) as exc:
        errors.append(f"F02e2: cannot validate A48 source-map metadata: {exc}")
    return errors

def validate_supersession_structure(tasks,tests):
    errors=[];task_edges={};test_edges={};test_by_id={x["id"]:x for x in tests}
    for sid,t in tasks.items():
        replacements=t.get("superseded_by")
        has_amendment="amendment" in t
        if replacements is None:
            if has_amendment:errors.append(f"{sid}: supersession amendment has no superseded_by mapping")
            continue
        if (not isinstance(replacements,list) or not replacements
            or any(not isinstance(x,str) or not x for x in replacements)
            or len(set(replacements))!=len(replacements)):
            errors.append(f"{sid}: superseded_by must contain unique nonempty task IDs");continue
        expected_amendment=TASK_SUPERSESSION_AMENDMENTS.get(sid,SUPERSESSION_AMENDMENT)
        if t.get("amendment")!=expected_amendment:
            errors.append(f"{sid}: supersession must cite {expected_amendment}")
        for rid in replacements:
            if rid not in tasks:errors.append(f"{sid}: unknown superseding task {rid}")
            if rid==sid:errors.append(f"{sid}: task cannot supersede itself")
        task_edges[sid]=replacements
        owned=[x for x in tests if x.get("slice")==sid]
        if owned and any(not x.get("superseded_by") for x in owned):
            errors.append(f"{sid}: every historical test needs a superseded_by mapping")
    for sid,t in tasks.items():
        for dep in t.get("depends_on",[]):
            if dep in task_edges:errors.append(f"{sid}: dependency {dep} is superseded; rewire to its replacement")
        historical=t.get("historical_depends_on",[])
        if not isinstance(historical,list):
            errors.append(f"{sid}: historical_depends_on must be a list")
            continue
        if sid not in task_edges:
            current=set(t.get("depends_on",[]))
            for dep in historical:
                if dep in task_edges and not set(task_edges[dep]).issubset(current):
                    errors.append(f"{sid}: historical dependency {dep} was not explicitly rewired to every replacement")
    for tid,x in test_by_id.items():
        replacements=x.get("superseded_by")
        if replacements is None:continue
        if (not isinstance(replacements,list) or not replacements
            or any(not isinstance(r,str) or not r for r in replacements)
            or len(set(replacements))!=len(replacements)):
            errors.append(f"{tid}: superseded_by must contain unique nonempty test IDs");continue
        owner=tasks.get(x.get("slice"))
        if owner is None or x.get("slice") not in task_edges:
            errors.append(f"{tid}: only tests owned by a superseded task may be superseded")
        test_edges[tid]=replacements
        for rid in replacements:
            target=test_by_id.get(rid)
            if target is None:
                errors.append(f"{tid}: unknown replacement test {rid}");continue
            if rid==tid:errors.append(f"{tid}: test cannot supersede itself")
            if target.get("superseded_by"):
                errors.append(f"{tid}: replacement test {rid} is not a surviving test")
            if target.get("requirement")!=x.get("requirement") or target.get("level")!=x.get("level"):
                errors.append(f"{tid}: replacement test {rid} has different requirement or evidence level")
            if owner and target.get("slice") not in owner.get("superseded_by",[]):
                errors.append(f"{tid}: replacement test {rid} is not owned by a superseding task")
    def cycles(edges,label):
        visiting=set();seen=set()
        def visit(node):
            if node in visiting:raise ValueError(f"{label} supersession cycle at {node}")
            if node in seen:return
            visiting.add(node)
            for nxt in edges.get(node,[]):
                if nxt in edges:visit(nxt)
            visiting.remove(node);seen.add(node)
        try:
            for node in edges:visit(node)
        except ValueError as e:errors.append(str(e))
    cycles(task_edges,"Task");cycles(test_edges,"Test")
    return errors

def validate_supersession_state(plan,state,strict=False,allow_template_pending=False):
    errors=[];tasks={t["id"]:t for t in plan["slices"]};states=state["slices"]
    for sid,t in tasks.items():
        s=states[sid];marked=bool(t.get("superseded_by"))
        if not marked:
            if s.get("status")=="superseded":errors.append(f"{sid}: state is superseded but plan has no supersession mapping")
            if s.get("supersession"):errors.append(f"{sid}: state supersession record has no plan mapping")
            continue
        expected={"amendment":t.get("amendment"),"superseded_by":t["superseded_by"]}
        if plan.get("successor_recovery_A52") is not None and sid in {"F02e2","F02e2r"}:
            successor=states.get("F02e2s",{})
            accepted=successor.get("status")=="accepted"
            expected_state={**expected,"transition":"successor_accepted" if accepted else "pending_successor_acceptance"}
            required_status="superseded" if accepted else "failed"
            expected_attempts=6 if sid=="F02e2" else 3
            if allow_template_pending and sid=="F02e2r" and not accepted and s.get("status")=="not_started" and s.get("attempts")==0:
                if s.get("supersession")!=expected_state:
                    errors.append(f"{sid}: initial A52 supersession record differs from the pending transition")
                continue
            if s.get("attempts")!=expected_attempts:
                errors.append(f"{sid}: A52 must preserve exhausted attempt count {expected_attempts}/{expected_attempts}")
            if sid=="F02e2" and s.get("terminal_failure")!=A50_TERMINAL_FAILURE:
                errors.append("F02e2: A52 must retain the exact immutable A48 terminal-failure record")
            if s.get("status")!=required_status:
                errors.append(f"{sid}: status must remain {required_status} until F02e2s is accepted")
            if s.get("supersession")!=expected_state:
                errors.append(f"{sid}: A52 supersession record differs from the pending/accepted transition")
            continue
        if sid=="F02e2" and t.get("amendment")=="A50":
            successor=states.get("F02e2r",{})
            accepted=successor.get("status")=="accepted"
            terminal=s.get("terminal_failure")
            if terminal!=A50_TERMINAL_FAILURE:
                errors.append("F02e2: A50 must retain the exact immutable A48 terminal-failure record")
            if s.get("attempts")!=6:
                errors.append("F02e2: A50 must preserve the exhausted 6/6 attempt count")
            expected_state={**expected,"transition":"successor_accepted" if accepted else "pending_successor_acceptance"}
            required_status="superseded" if accepted else "failed"
            if s.get("status")!=required_status:
                errors.append(f"F02e2: status must remain {required_status} at the A50 successor transition")
            if s.get("supersession")!=expected_state:
                errors.append("F02e2: state supersession record differs from the A50 pending/accepted transition")
            continue
        if s.get("status")!="superseded":errors.append(f"{sid}: plan-superseded task must retain state status superseded")
        if s.get("supersession")!=expected:errors.append(f"{sid}: state supersession record differs from plan or is empty")
    if strict:
        def surviving_replacements(sid,seen=None):
            seen=set() if seen is None else seen
            if sid in seen:return set()
            seen.add(sid);t=tasks.get(sid)
            if not t:return set()
            if not t.get("superseded_by"):return {sid}
            out=set()
            for rid in t["superseded_by"]:out|=surviving_replacements(rid,seen.copy())
            return out
        for sid,t in tasks.items():
            if t.get("superseded_by"):
                leaves=surviving_replacements(sid)
                if not leaves:errors.append(f"{sid}: supersession has no surviving replacement")
                for rid in leaves:
                    if rid in states and states[rid].get("status")!="accepted":
                        errors.append(f"{sid}: replacement dependency {rid} is not accepted")
    return errors

def validate_structure(pack=PACK,plan=None):
    errors=[]
    try:
        plan=plan or load(pack/"control/plan.json")
        tests=load(pack/"control/test_matrix.json")["tests"]
        reqs=load(pack/"control/requirements.json")["requirements"]
        contracts=load(pack/"control/contracts.json")
        ids=[t["id"] for t in plan["slices"]];tasks={t["id"]:t for t in plan["slices"]}
        if len(set(ids))!=len(ids):errors.append("Duplicate slice IDs")
        testids=[t["id"] for t in tests]
        if len(set(testids))!=len(testids):errors.append("Duplicate test IDs")
        errors+=validate_attempt_limits(plan,tasks)
        errors+=validate_native_gate_retries(plan,pack)
        errors+=validate_source_attempt_extension_A48(plan,pack)
        errors+=validate_harness_prerequisite_split_A49(plan,pack)
        errors+=validate_successor_recovery_A50(plan,pack)
        errors+=validate_successor_recovery_A52(plan,pack)
        errors+=validate_continuation_A51(plan,pack)
        errors+=validate_supersession_structure(tasks,tests)
        reqids=[r["id"] for r in reqs]
        if len(set(reqids))!=len(reqids):errors.append("Duplicate requirement IDs")
        for f in ["START_HERE.md","README.md","BASELINE.md","ORCHESTRATOR.md","WORKER.md","REVIEWER.md","ENVIRONMENT.md","TESTING.md","RESOURCE_POLICY.md","RELEASE.md","SOURCES.md","TRACEABILITY.md","GOAL_PROMPT.txt"]:
            if not (pack/f).is_file():errors.append(f"Missing required document {f}")
        if len((pack/"GOAL_PROMPT.txt").read_bytes())>=1000:errors.append("Goal must be below 1000 UTF-8 bytes")
        for t in tasks.values():
            for d in t["depends_on"]:
                if d not in tasks:errors.append(f"{t['id']}: unknown dependency {d}")
            if t["recipe"] not in RECIPES:errors.append(f"Unknown recipe {t['recipe']}")
            for c in t["contracts"]:
                if c not in contracts:errors.append(f"{t['id']}: unknown contract {c}")
            errors+=validate_edit_path_budget(t,pack)
            for path in t["allowed_paths"]:safe_relative(pack,path)
            if not safe_relative(pack,t["slice_file"]).is_file():errors.append(f"{t['id']}: missing slice file")
            owned={x["id"] for x in tests if x["slice"]==t["id"]}
            if set(t["test_ids"])!=owned:errors.append(f"{t['id']}: inconsistent test mapping")
            if t["requirement_id"] not in reqids:errors.append(f"{t['id']}: missing requirement")
            if len(packet(t,pack).encode())>plan["limits"]["max_packet_bytes"]:errors.append(f"{t['id']}: packet budget exceeded")
        for x in tests:
            if x["slice"] not in tasks:errors.append(f"{x['id']}: unknown owner")
            if x["level"] not in LEVELS:errors.append(f"{x['id']}: unknown evidence level")
        for r in reqs:
            if r["owner"] not in tasks:errors.append(f"{r['id']}: unknown owner")
        visiting=set();seen=set()
        def visit(k):
            if k in visiting:raise ValueError(f"Dependency cycle at {k}")
            if k in seen:return
            visiting.add(k)
            for d in tasks[k]["depends_on"]:
                if d in tasks:visit(d)
            visiting.remove(k);seen.add(k)
        for k in tasks:visit(k)
        # Catch local markdown links, without interpreting repository code paths as files in this pack.
        for doc in pack.rglob("*.md"):
            if "reference" in doc.relative_to(pack).parts:continue
            for target in re.findall(r"\[[^\]]*\]\(([^)]+)\)",doc.read_text()):
                if "://" in target or target.startswith(("#","mailto:")):continue
                rel=target.split("#",1)[0]
                if rel and not (doc.parent/rel).exists():errors.append(f"Broken local link {doc.relative_to(pack)} -> {rel}")
        return errors
    except (ValueError,KeyError,TypeError,OSError) as e:
        errors.append(f"Malformed pack: {e}");return errors

def validate_acceptance(task,record,root,tests):
    errors=[];sid=task["id"]
    try:
        if not record.get("author") or not record.get("reviewer") or record["author"]==record["reviewer"]:
            raise ValueError("requires distinct actual author and reviewer")
        if not record.get("evidence"):raise ValueError("missing evidence path")
        ev=load(safe_relative(root,record["evidence"]));sha=ev["source_sha"]
        if not HEX.fullmatch(sha):raise ValueError("invalid integrated source SHA")
        if not HEX.fullmatch(ev["implementation_sha"]):raise ValueError("invalid implementation SHA")
        if ev["slice_id"]!=sid or ev["author"]!=record["author"] or ev["reviewer"]!=record["reviewer"]:
            raise ValueError("evidence owner mismatch")
        review_path=safe_relative(root,ev["review_path"])
        if digest(review_path)!=ev["review_sha256"]:raise ValueError("review digest mismatch")
        review=load(review_path)
        if review["reviewer"]!=ev["reviewer"] or review["source_sha"]!=sha or review["decision"]!="accept":
            raise ValueError("review does not accept this integrated source")
        cases={c["test_id"]:c for c in ev["cases"]}
        if len(cases)!=len(ev["cases"]) or set(cases)!=set(task["test_ids"]):raise ValueError("case coverage mismatch")
        for tid in task["test_ids"]:
            c=cases[tid];expected=tests[tid]["level"]
            if c["result"]!="passed" or c["source_sha"]!=sha or c["level"]!=expected:
                raise ValueError(f"{tid}: status, source or level mismatch")
            if not c.get("assertion") or not c.get("artifacts"):raise ValueError(f"{tid}: no assertion/evidence")
            command=False;qualification=False;report=False
            for artifact in c["artifacts"]:
                path=safe_relative(root,artifact["path"])
                if digest(path)!=artifact["sha256"]:raise ValueError(f"{tid}: artifact digest mismatch")
                kind=artifact["kind"]
                if kind=="command_receipt":
                    rc=load(path)
                    if rc["result"]!="passed" or rc["exit_code"]!=0 or rc.get("timed_out") or rc.get("output_limit_exceeded") or rc.get("observed_failed",0)>0:
                        raise ValueError(f"{tid}: failed command receipt")
                    for key in ("source_before","source_after"):
                        if rc[key]["sha"]!=sha or rc[key]["dirty"]:raise ValueError(f"{tid}: dirty/different receipt source")
                    if expected in {"unit","component","application","native","physical"} and (rc.get("observed_test_summary_count",0)<=0 or rc.get("observed_skips",0)>0):
                        raise ValueError(f"{tid}: no executed tests or skipped tests")
                    log=safe_relative(path.parent,rc["log"])
                    if digest(log)!=rc["log_sha256"]:raise ValueError(f"{tid}: log digest mismatch")
                    command=True
                elif kind=="qualification_record":
                    q=load(path)
                    if q["source_sha"]!=sha or q["level"]!=expected or q["passed"] is not True or not q.get("operator") or not q.get("environment") or not q.get("observations"):
                        raise ValueError(f"{tid}: incomplete qualification record")
                    if expected=="physical" and not q.get("devices"):raise ValueError(f"{tid}: physical evidence lacks devices")
                    qualification=True
                elif kind=="report":report=True
                else:raise ValueError(f"{tid}: unknown artifact kind")
            if expected in {"native","physical","benchmark"} and not qualification:raise ValueError(f"{tid}: actual environment qualification missing")
            if expected in {"unit","component","application","build","fuzz"} and not command:raise ValueError(f"{tid}: executable evidence missing")
            if expected=="inspection" and not (report or command):raise ValueError(f"{tid}: reviewable inspection output missing")
        if record.get("source_sha")!=sha:raise ValueError("accepted state's source_sha missing or differs")
    except (OSError,KeyError,TypeError,ValueError) as e:errors.append(f"{sid}: {e}")
    return errors

def validate_acceptance_A52(task,record,root):
    """Require ordered, receipt-bound native gates and an independent review for F02e2s."""
    if task.get("id")!="F02e2s" or record.get("status")!="accepted":return []
    errors=[]
    try:
        evidence_path=safe_relative(root,record["evidence"]);evidence=load(evidence_path)
        source_sha=evidence["source_sha"];source_tree=evidence["source_tree"]
        if not HEX.fullmatch(source_sha) or not HEX.fullmatch(source_tree):
            raise ValueError("evidence source SHA/tree is invalid")
        attempt=record.get("attempts")
        if not isinstance(attempt,int) or isinstance(attempt,bool) or not 1<=attempt<=3:
            raise ValueError("accepted F02e2s attempt is outside the three-attempt limit")
        review_rel=evidence["gate_evidence_review"];review_sha=evidence["gate_evidence_review_sha256"]
        review_path=safe_relative(root,review_rel)
        if digest(review_path)!=review_sha:raise ValueError("gate-evidence review digest mismatch")
        review=load(review_path);candidate=review.get("candidate",{})
        if (review.get("decision")!="accept" or not review.get("reviewer")
                or review.get("reviewer")==record.get("author")
                or review.get("reviewer")!=evidence.get("reviewer")
                or candidate.get("slice")!="F02e2s" or candidate.get("amendment")!="A52"
                or candidate.get("commit")!=source_sha or candidate.get("tree")!=source_tree
                or candidate.get("attempt")!=attempt):
            raise ValueError("gate-evidence review does not independently accept the exact F02e2s candidate")
        run_path=safe_relative(root,evidence["native_run"])
        if digest(run_path)!=evidence["native_run_sha256"]:raise ValueError("native-run digest mismatch")
        run=load(run_path)
        target_dir=(root/"builds"/f"native-F02e2s-a{attempt}-qt6112-qmake").resolve()
        target=str(target_dir)
        if (run.get("result")!="passed" or run.get("slice")!="F02e2s" or run.get("amendment")!="A52"
                or run.get("attempt")!=attempt or run.get("attempt_limit")!=3
                or run.get("source_sha")!=source_sha or run.get("source_tree")!=source_tree
                or run.get("product_tests_run")!=["T014e2s"] or run.get("target")!=target
                or run.get("lease_released") is not True or run.get("active_build_processes_remaining")!=[]):
            raise ValueError("native run is not the exact passing A52 T014e2s run")
        run_gates=run.get("gates")
        if not isinstance(run_gates,list) or [g.get("name") for g in run_gates]!=A52_NATIVE_GATE_SEQUENCE:
            raise ValueError("native run does not contain the exact ordered A52 gate sequence")
        review_gates=review.get("gates")
        if not isinstance(review_gates,list) or [g.get("name") for g in review_gates]!=A52_NATIVE_GATE_SEQUENCE:
            raise ValueError("independent gate review does not cover the exact ordered A52 gate sequence")
        seen_receipts=set();preflight_kf6_library=None
        for index,(gate,review_gate,gate_name) in enumerate(zip(run_gates,review_gates,A52_NATIVE_GATE_SEQUENCE)):
            if (gate.get("result")!="passed" or gate.get("target")!=target
                    or not isinstance(gate.get("receipt"),str) or not isinstance(gate.get("receipt_sha256"),str)):
                raise ValueError(f"{gate_name}: native run is missing its passing receipt or exact target binding")
            receipt_rel=gate["receipt"]
            if receipt_rel in seen_receipts:raise ValueError(f"{gate_name}: gate receipts must be distinct")
            seen_receipts.add(receipt_rel)
            receipt_path=safe_relative(root,receipt_rel)
            receipt_sha=gate["receipt_sha256"]
            if digest(receipt_path)!=receipt_sha:raise ValueError(f"{gate_name}: receipt digest mismatch")
            if (review_gate.get("decision")!="pass" or review_gate.get("target")!=target
                    or review_gate.get("receipt")!=receipt_rel or review_gate.get("receipt_sha256")!=receipt_sha):
                raise ValueError(f"{gate_name}: independent review does not accept the exact run receipt")
            receipt=load(receipt_path)
            if gate_name=="read-only-sdk-selection-preflight":
                if (receipt.get("result")!="passed" or receipt.get("exit_code")!=0
                        or receipt.get("no_compilation") is not True or receipt.get("target")!=target
                        or receipt.get("attempt")!=attempt):
                    raise ValueError("SDK preflight did not pass read-only against the exact fresh target")
                before=receipt.get("source_before",{});after=receipt.get("source_after",{})
                before=before.get("candidate",before);after=after.get("candidate",after)
                if any(snapshot.get("sha")!=source_sha or snapshot.get("tree")!=source_tree or snapshot.get("dirty") is not False
                       for snapshot in (before,after)):
                    raise ValueError("SDK preflight is not bound to the exact clean candidate before and after")
                sdk=receipt.get("sdk",{})
                qt_version=sdk.get("qt_version",receipt.get("pinned_qt_version"))
                kf_version=sdk.get("version",receipt.get("selected_cmake",{}).get("KF6WindowSystem_version"))
                kf_abi=sdk.get("abi",receipt.get("selected_cmake",{}).get("KF6_qt_abi"))
                preflight_kf6_library=sdk.get("library",receipt.get("selected_cmake",{}).get("KF6WindowSystem_library"))
                try:kf_version_tuple=tuple(int(part) for part in str(kf_version).split(".")[:3])
                except ValueError:kf_version_tuple=()
                kf_version_tuple=(kf_version_tuple+(0,0,0))[:3]
                if (qt_version!="6.11.2" or kf_version_tuple<(6,28,0) or kf_abi!="Qt_6.11"
                        or not isinstance(preflight_kf6_library,str) or not preflight_kf6_library):
                    raise ValueError("SDK preflight does not prove pinned Qt 6.11.2 and matching KF6WindowSystem")
            elif gate_name=="robust-executable-link-order-verification":
                if (receipt.get("result")!="passed" or receipt.get("source_sha")!=source_sha
                        or receipt.get("source_tree")!=source_tree or receipt.get("target")!=target):
                    raise ValueError("link-order report is not bound to the exact source and fresh target")
                checks=receipt.get("checks",{})
                if (checks.get("engine_archive_before_kf6windowsystem") is not True
                        or checks.get("matching_kf6_runtime") is not True
                        or checks.get("qt_version")!="6.11.2"
                        or checks.get("kf6_window_system_qt_abi")!="Qt_6.11"
                        or checks.get("kf6_window_system_library")!=preflight_kf6_library):
                    raise ValueError("link-order report does not prove archive ordering and matching Qt 6.11 runtime")
                artifacts=receipt.get("artifacts",{})
                artifact_paths={}
                for artifact_name in ("executable","link_command"):
                    artifact=artifacts.get(artifact_name,{})
                    artifact_path=safe_relative(root,artifact.get("path",""))
                    if not artifact_path.is_relative_to(target_dir) or digest(artifact_path)!=artifact.get("sha256"):
                        raise ValueError(f"link-order report {artifact_name} artifact is missing, outside target, or has wrong digest")
                    artifact_paths[artifact_name]=artifact_path
                if not (artifact_paths["executable"].stat().st_mode&0o111):
                    raise ValueError("link-order report executable artifact is not executable")
                link_tokens=shlex.split(artifact_paths["link_command"].read_text(errors="replace"))
                engine_positions=[i for i,token in enumerate(link_tokens) if "libferric_browser_engine_qt.a" in token]
                kf6_positions=[i for i,token in enumerate(link_tokens) if token==preflight_kf6_library]
                if (len(engine_positions)!=1 or len(kf6_positions)!=1
                        or engine_positions[0]>=kf6_positions[0]):
                    raise ValueError("actual target link command does not place the engine archive before the matching KF6WindowSystem library")
            else:
                if (receipt.get("result")!="passed" or receipt.get("exit_code")!=0
                        or receipt.get("timed_out") or receipt.get("output_limit_exceeded")
                        or receipt.get("observed_failed",0)>0):
                    raise ValueError(f"{gate_name}: command receipt did not pass cleanly")
                binding=receipt.get("execution_binding",{})
                if (receipt.get("native_target")!=target or binding.get("native_target")!=target):
                    raise ValueError(f"{gate_name}: underlying command receipt does not bind the fresh native target")
                for key in ("source_before","source_after"):
                    snapshot=receipt.get(key,{})
                    if (snapshot.get("sha")!=source_sha or snapshot.get("tree")!=source_tree
                            or snapshot.get("dirty") is not False):
                        raise ValueError(f"{gate_name}: receipt is not bound to the exact clean source")
                log_path=safe_relative(receipt_path.parent,receipt.get("log",""))
                if digest(log_path)!=receipt.get("log_sha256"):
                    raise ValueError(f"{gate_name}: output log is missing or has wrong digest")
                log_text=log_path.read_text(errors="replace")
                command=receipt.get("command")
                if not isinstance(command,list) or not command:
                    raise ValueError(f"{gate_name}: command receipt omits the command")
                cmake_dir=(target_dir/"cmake").resolve()
                if gate_name=="exact-source-release-engine-build":
                    cargo_target=(target_dir/"cargo-target").resolve()
                    if (binding.get("cargo_target_dir")!=str(cargo_target)
                            or binding.get("environment",{}).get("CARGO_TARGET_DIR")!=str(cargo_target)
                            or "--target-dir" not in command or command.index("--target-dir")+1>=len(command)
                            or Path(command[command.index("--target-dir")+1]).resolve()!=cargo_target):
                        raise ValueError("engine build receipt does not prove CARGO_TARGET_DIR uses the fresh target")
                elif gate_name=="public-page-host-harness-configure":
                    if (binding.get("cmake_binary_dir")!=str(cmake_dir) or "-B" not in command
                            or command.index("-B")+1>=len(command)
                            or Path(command[command.index("-B")+1]).resolve()!=cmake_dir):
                        raise ValueError("harness configure receipt does not prove CMake used the fresh target directory")
                elif gate_name=="integration-target-build":
                    if (binding.get("cmake_binary_dir")!=str(cmake_dir) or "--build" not in command
                            or command.index("--build")+1>=len(command)
                            or Path(command[command.index("--build")+1]).resolve()!=cmake_dir):
                        raise ValueError("integration build receipt does not prove CMake built in the fresh target directory")
                elif gate_name in {"nonzero-ctest-discovery","T014e2s-serial"}:
                    if (binding.get("cmake_binary_dir")!=str(cmake_dir) or "--test-dir" not in command
                            or command.index("--test-dir")+1>=len(command)
                            or Path(command[command.index("--test-dir")+1]).resolve()!=cmake_dir):
                        raise ValueError(f"{gate_name}: CTest receipt does not prove it used the fresh target directory")
                if gate_name in {"exact-source-release-engine-build","public-page-host-harness-configure","integration-target-build","T014e2s-serial"} and receipt.get("qt_include_guard")!="clear":
                    raise ValueError(f"{gate_name}: strict Qt include guard did not remain clear")
                if gate_name=="nonzero-ctest-discovery":
                    if (gate.get("registered_tests",0)<=0 or gate.get("T014e2s_registered") is not True
                            or not re.search(r"Total Tests:\s*[1-9][0-9]*",log_text)
                            or not re.search(r"Test\s+#\d+:\s*T014e2s(?:\s|$)",log_text)):
                        raise ValueError("discovery receipt does not prove a nonzero CTest inventory containing T014e2s")
                if gate_name=="T014e2s-serial":
                    command_text=" ".join(str(arg) for arg in command)
                    selector=None
                    for option in ("--tests-regex","-R"):
                        if option in command:
                            position=command.index(option)
                            if position+1<len(command):selector=command[position+1]
                            break
                    target_option=(command.index("--test-dir")+1 if "--test-dir" in command
                                   and command.index("--test-dir")+1<len(command) else None)
                    if (gate.get("test_id")!="T014e2s" or gate.get("run_count")!=1
                            or gate.get("failed")!=0 or gate.get("skipped")!=0 or gate.get("parallelism")!=1
                            or receipt.get("require_tests") is not True or receipt.get("observed_test_summary_count",0)<=0
                            or receipt.get("observed_skips")!=0 or receipt.get("observed_failed",0)!=0
                            or selector!="^T014e2s$" or target_option is None
                            or Path(command[target_option]).resolve()!=(target_dir/"cmake").resolve()
                            or "--repeat" in command or "T014e2" in command_text.replace("T014e2s", "")
                            or not re.search(r"(?:--parallel(?:\s+|=)1|(?:^|\s)-j1(?:\s|$))",command_text)):
                        raise ValueError("serial gate does not prove one isolated T014e2s run with zero skips")
                    if (not re.search(r"1/1\s+Test\s+#\d+:\s*T014e2s\s+Passed",log_text)
                            or not re.search(r"100% tests passed, 0 tests failed out of 1",log_text)):
                        raise ValueError("serial gate output does not show the single T014e2s case passed")
        discovery=review_gates[5]
        test_gate=review_gates[6]
        if (discovery.get("T014e2s_registered") is not True or discovery.get("registered_tests",0)<=0):
            raise ValueError("gate-evidence review does not prove nonzero T014e2s discovery")
        if (test_gate.get("test_id")!="T014e2s" or test_gate.get("executed_test_count")!=1
                or test_gate.get("skips")!=0 or test_gate.get("failures")!=0
                or test_gate.get("parallelism")!=1):
            raise ValueError("gate-evidence review does not accept one serial T014e2s run with zero skips")
        monitor=run.get("live_process_monitor",{})
        monitor_path=safe_relative(root,monitor.get("path",""))
        if monitor.get("result")!="clear" or digest(monitor_path)!=monitor.get("sha256"):
            raise ValueError("live compiler-process monitor is missing, uncleared, or has a digest mismatch")
        monitor_record=load(monitor_path)
        if (monitor_record.get("result")!="clear" or monitor_record.get("source_sha")!=source_sha
                or monitor_record.get("source_tree")!=source_tree or monitor_record.get("attempt")!=attempt
                or monitor_record.get("artifact")!=target_dir.name):
            raise ValueError("live compiler-process monitor is not bound to the exact candidate and target")
        closure_path=safe_relative(root,evidence["lease_closure"])
        if digest(closure_path)!=evidence["lease_closure_sha256"]:raise ValueError("lease-closure digest mismatch")
        closure=load(closure_path)
        if (closure.get("artifact")!=target_dir.name or closure.get("source_sha")!=source_sha
                or closure.get("source_tree")!=source_tree or closure.get("attempt")!=attempt
                or closure.get("live_process_monitor")!=monitor.get("path")
                or closure.get("live_process_monitor_sha256")!=monitor.get("sha256")
                or closure.get("active_lock_released") is not True
                or closure.get("active_build_processes_remaining")!=[]):
            raise ValueError("A52 native lease is not fully closed")
        summary=review.get("run_and_closure",{})
        if (summary.get("target")!=target or summary.get("all_commands_passed") is not True or summary.get("lease_released") is not True
                or summary.get("active_build_processes_remaining")!=[]
                or summary.get("old_T014e2_rerun") is not False
                or summary.get("old_T014e2r_ran") is not False
                or summary.get("other_product_tests_run") not in (False,[])):
            raise ValueError("gate review omits closure or old-test execution prohibitions")
    except (OSError,KeyError,TypeError,ValueError,AttributeError,IndexError) as exc:
        errors.append(f"F02e2s: {exc}")
    return errors

def validate_state(plan,state_path,strict=False,pack=PACK):
    errors=[]
    try:
        state=load(state_path);root=state_path.resolve().parent.parent
        tests={x["id"]:x for x in load(pack/"control/test_matrix.json")["tests"]}
        tasks={x["id"]:x for x in plan["slices"]}
        if set(state["slices"])!=set(tasks):raise ValueError("State task inventory differs from plan")
        errors+=validate_attempt_limits(plan,tasks)
        errors+=validate_source_attempt_extension_A48(plan,pack)
        errors+=validate_harness_prerequisite_split_A49(plan,pack)
        errors+=validate_successor_recovery_A50(plan,pack)
        errors+=validate_successor_recovery_A52(plan,pack)
        errors+=validate_continuation_A51(plan,pack)
        errors+=validate_continuation_state_A51(state,root)
        errors+=validate_supersession_state(plan,state,strict,state_path.name=="state.initial.json")
        if state.get("run_id") is not None:
            parent_state=state["slices"].get("F02e2",{})
            successor_state=state["slices"].get("F02e2r",{})
            current_successor=state["slices"].get("F02e2s",{}) if plan.get("successor_recovery_A52") is not None else successor_state
            if parent_state.get("status") not in {"failed","superseded"} or parent_state.get("attempts")!=6:
                errors.append("F02e2: A49/A50 require the parent failure and 6/6 history to remain visible")
            if plan.get("successor_recovery_A52") is not None:
                if successor_state.get("attempts")!=3 or successor_state.get("A51_continuation",{}).get("result")!="failed":
                    errors.append("F02e2r: A52 requires the immutable failed 3/3 A51 continuation record")
                for sid in ("F02e2","F02e2r"):
                    if state["slices"].get(sid,{}).get("status")=="superseded" and current_successor.get("status")!="accepted":
                        errors.append(f"{sid}: cannot supersede before F02e2s acceptance")
                if state["slices"].get("F02e3",{}).get("status")=="accepted" and current_successor.get("status")!="accepted":
                    errors.append("F02e3: cannot be accepted before successor F02e2s")
            else:
                if parent_state.get("status")=="failed" and successor_state.get("status")=="accepted":
                    errors.append("F02e2: transition to workflow-superseded after F02e2r acceptance")
                if parent_state.get("status")=="superseded" and successor_state.get("status")!="accepted":
                    errors.append("F02e2: cannot supersede the failed parent before F02e2r acceptance")
                if state["slices"].get("F02e3",{}).get("status")=="accepted" and successor_state.get("status")!="accepted":
                    errors.append("F02e3: cannot be accepted before successor F02e2r")
        for sid,t in tasks.items():
            s=state["slices"][sid]
            if s["status"] not in STATUSES:errors.append(f"{sid}: invalid status")
            attempts=s.get("attempts");limit=effective_attempt_limit(plan,t)
            if not isinstance(attempts,int) or isinstance(attempts,bool) or not 0<=attempts<=limit:
                errors.append(f"{sid}: attempt limit")
            if t.get("superseded_by"):
                continue
            if s["status"]=="accepted":
                if any(state["slices"][d]["status"]!="accepted" for d in t["depends_on"]):errors.append(f"{sid}: unaccepted dependency")
                errors+=validate_acceptance(t,s,root,tests)
                errors+=validate_acceptance_A52(t,s,root)
            elif s["status"]=="superseded":
                continue
            elif strict:errors.append(f"{sid}: not accepted ({s['status']})")
        if strict:
            if not state.get("run_id") or not HEX.fullmatch(state.get("integration_sha") or ""):
                errors.append("Strict completion requires real run/source")
            cap=root/"control/release-capabilities.json"
            if not cap.exists():errors.append("Missing final release-capabilities.json")
            else:
                caps=load(cap)
                if caps["source_sha"]!=state["integration_sha"]:errors.append("Final capability source differs from integration")
                for name in ("hot_internal","hot_windows","phone_top_level","native_page_windows","hyprland_grouping"):
                    r=caps["capabilities"][name]
                    if r["implementation"]!="complete" or r["qualification"]!="passed" or not r["evidence"]:
                        errors.append(f"{name}: final capability not qualified")
                    for a in r.get("evidence",[]):
                        p=safe_relative(root,a["path"])
                        if digest(p)!=a["sha256"]:errors.append(f"{name}: final evidence digest differs")
                        q=load(p)
                        if q.get("source_sha")!=state["integration_sha"] or q.get("passed") is not True:
                            errors.append(f"{name}: final evidence is not passed at final source")
    except (OSError,ValueError,TypeError,KeyError) as e:errors.append(f"Malformed execution ledger: {e}")
    return errors

def main():
    a=argparse.ArgumentParser(description=__doc__);a.add_argument("--state",type=Path);a.add_argument("--strict",action="store_true");a.add_argument("--plan",type=Path);n=a.parse_args()
    if n.strict and not n.state:a.error("--strict requires --state")
    plan=read_plan(n.plan);errors=validate_structure(PACK,plan)
    if n.state:errors+=validate_state(plan,n.state,n.strict)
    if errors:
        for e in errors:print("ERROR:",e,file=sys.stderr)
        raise SystemExit(1)
    print(f"VALID: {len(plan['slices'])} slices; structure"+(" and supplied evidence ledger" if n.state else " only"))
    print("Validation does not prove the browser implementation or physical observations.")
if __name__=="__main__":main()
