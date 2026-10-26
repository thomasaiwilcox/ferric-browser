#!/usr/bin/env python3
"""Validate pack/ledger consistency. This does not establish real browser correctness."""
from __future__ import annotations
import argparse,json,re,sys
from pathlib import Path
from common import (PACK,load,read_plan,packet,safe_relative,digest,
                    ATTEMPT_EXCEPTION_TASKS,ATTEMPT_EXCEPTION_AMENDMENT,
                    GLOBAL_MAX_ATTEMPTS,effective_attempt_limit)
STATUSES={"not_started","ready","in_progress","submitted","reviewing","accepted","superseded","blocked_environment","blocked_specification","implemented_unqualified","failed"}
LEVELS={"inspection","unit","component","application","native","physical","benchmark","build","fuzz"}
RECIPES={"preflight","baseline","config","rust-policy","application-rust","storage","application","credential","helper","rp","fuzz","benchmark","native","physical","package","ci","docs","review","rebuild","release"}
HEX=re.compile(r"^[0-9a-f]{40}$")

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
        if entry.get("amendment")!=ATTEMPT_EXCEPTION_AMENDMENT:
            errors.append(f"{sid}: attempt exception must cite {ATTEMPT_EXCEPTION_AMENDMENT}")
        if entry.get("default_limit")!=default:
            errors.append(f"{sid}: attempt exception default_limit differs from global limit")
        if entry.get("authorized_limit")!=limit or limit!=default+1:
            errors.append(f"{sid}: attempt exception must authorize exactly one extra attempt matching max_attempts")
        if not isinstance(entry.get("reason"),str) or not entry["reason"].strip():
            errors.append(f"{sid}: attempt exception requires a nonempty reason")
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
        if t.get("amendment")!=ATTEMPT_EXCEPTION_AMENDMENT:
            errors.append(f"{sid}: supersession must cite {ATTEMPT_EXCEPTION_AMENDMENT}")
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
            if len(t["allowed_paths"])>8:errors.append(f"{t['id']}: edit-path budget exceeded")
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
