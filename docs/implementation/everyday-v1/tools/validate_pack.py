#!/usr/bin/env python3
"""Validate pack/ledger consistency. This does not establish real browser correctness."""
from __future__ import annotations
import argparse,json,re,sys
from pathlib import Path
from common import (PACK,load,read_plan,packet,safe_relative,digest,
                    ATTEMPT_EXCEPTION_TASKS,ATTEMPT_EXCEPTION_AMENDMENTS,
                    ATTEMPT_EXCEPTION_LIMITS,GLOBAL_MAX_ATTEMPTS,effective_attempt_limit)
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
A45_CANDIDATE_SHA="58c047c04072bb7efa382e3cd7ca65a11da1b117"
A45_CANDIDATE_TREE="f5cc15efd18496bb209d25866d060347157a00bd"
A45_QT_PREFIX="/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64"
A45_FRESH_TARGET="/home/tom/Projects/ferric-everyday-work/builds/native-F02e2-a5-qt6112-qmake"
SUPERSESSION_AMENDMENT="A37"

def validate_edit_path_budget(task,pack=PACK):
    """Enforce the 8-path default and exact reviewed F02e1/A38 or F02e2/A42/A44 exceptions."""
    errors=[];paths=task.get("allowed_paths",[]);cap=task.get("max_changed_product_files",8)
    amendment=task.get("path_budget_amendment")
    if amendment is None:
        if len(paths)>8 or cap>8:
            errors.append(f"{task.get('id')}: edit-path budget exceeded")
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
    """Allow only the accepted A45 same-source Qt selection retry; never a sixth source attempt."""
    errors=[]
    if not isinstance(plan,dict):return ["plan must be an object"]
    entries=plan.get("native_gate_retries",{})
    if not isinstance(entries,dict):return ["native_gate_retries must be an object"]
    accepted_a45=False
    try:
        amendments=(pack/"control/AMENDMENTS.md").read_text()
        section=amendments.split("## A45",1)[1].split("\n## ",1)[0]
        accepted_a45=("**Status:** independently reviewed and accepted" in section)
    except (IndexError,OSError):
        section=""
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
    if (task is None or task.get("max_attempts")!=5 or task.get("path_budget_amendment")!="A44"
            or len(task.get("allowed_paths",[]))!=10 or task.get("locks")!=A44_LOCKS):
        errors.append("F02e2: A45 must retain A44's 5/5 source limit, exact ten paths and four locks")
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
        if t.get("amendment")!=SUPERSESSION_AMENDMENT:
            errors.append(f"{sid}: supersession must cite {SUPERSESSION_AMENDMENT}")
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

def validate_supersession_state(plan,state,strict=False):
    errors=[];tasks={t["id"]:t for t in plan["slices"]};states=state["slices"]
    for sid,t in tasks.items():
        s=states[sid];marked=bool(t.get("superseded_by"))
        if not marked:
            if s.get("status")=="superseded":errors.append(f"{sid}: state is superseded but plan has no supersession mapping")
            if s.get("supersession"):errors.append(f"{sid}: state supersession record has no plan mapping")
            continue
        expected={"amendment":t.get("amendment"),"superseded_by":t["superseded_by"]}
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
            if x["result"]!="not_run":errors.append(f"Authoring matrix must not claim execution: {x['id']}")
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

def validate_state(plan,state_path,strict=False,pack=PACK):
    errors=[]
    try:
        state=load(state_path);root=state_path.resolve().parent.parent
        tests={x["id"]:x for x in load(pack/"control/test_matrix.json")["tests"]}
        tasks={x["id"]:x for x in plan["slices"]}
        if set(state["slices"])!=set(tasks):raise ValueError("State task inventory differs from plan")
        errors+=validate_attempt_limits(plan,tasks)
        errors+=validate_supersession_state(plan,state,strict)
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
