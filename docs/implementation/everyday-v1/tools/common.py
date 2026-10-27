"""Standard-library helpers. No network requests and no implicit source mutations."""
from __future__ import annotations
import hashlib,json,os,re,subprocess,tempfile
from pathlib import Path
from datetime import datetime,timezone
PACK=Path(__file__).resolve().parent.parent
REPO="thomasaiwilcox/ferric-browser"
CLONE=f"https://github.com/{REPO}.git"
ATTEMPT_EXCEPTION_AMENDMENTS={"P00a":"A37","W01b":"A37","F02e2":"A48","F02e2i":"A54"}
ATTEMPT_EXCEPTION_LIMITS={"P00a":4,"W01b":4,"F02e2":6,"F02e2i":4}
ATTEMPT_EXCEPTION_TASKS=set(ATTEMPT_EXCEPTION_AMENDMENTS)
TASK_SUPERSESSION_AMENDMENTS={"F02e2":"A52","F02e2r":"A52"}
GLOBAL_MAX_ATTEMPTS=3
A54_IDENTITY_PHASE_NEXT={
    "assigned":{"source_review_started","terminal_failed"},
    "source_review_started":{"source_review_passed","terminal_failed"},
    "source_review_passed":{"integration_started","terminal_failed"},
    "integration_started":{"integrated_review_passed","terminal_failed"},
    "integrated_review_passed":{"focused_started","terminal_failed"},
    "focused_started":{"focused_passed","terminal_failed"},
    "focused_passed":{"regression_started","terminal_failed"},
    "regression_started":{"regression_passed","terminal_failed"},
    "regression_passed":{"accepted","terminal_failed"},
    "accepted":set(),"terminal_failed":set(),
}

def identity_attempt_seed_A54(nonce):
    if not isinstance(nonce,str) or len(nonce)!=32 or any(x not in "0123456789abcdef" for x in nonce):
        raise ValueError("A54 assignment requires a unique32-hex run nonce")
    return {"slice":"F02e2i","attempt":4,"run_nonce":nonce,"sequence":0,
            "previous_sha256":None,"phase":"assigned"}

def identity_attempt_event_hash_A54(event):
    return hashlib.sha256(json.dumps(event,sort_keys=True,separators=(",",":")).encode()).hexdigest()

def identity_review_decision_A54(review):
    values=[review.get("decision"),review.get("recommendation")]
    if any(value in {"request_changes","repair","block","reject"} for value in values):return "reject"
    return values[0] or values[1]

def identity_attempt_artifact_A54(meta,phase):
    return {"source_review_passed":meta["source_review"],"integrated_review_passed":meta["integrated_source_review"],
            "focused_passed":meta["unit_gate_receipts"]["focused"],"regression_passed":meta["unit_gate_receipts"]["application_regression"],
            "accepted":meta["gate_evidence_review"],"terminal_failed":meta["terminal_marker"]}.get(phase)

def identity_failed_artifacts_A54(root,meta):
    failures={}
    paths={**meta["unit_gate_receipts"],**{key:meta[key] for key in ("source_review","integrated_source_review","gate_evidence_review")}}
    for name,value in paths.items():
        path=safe_relative(root,value)
        if not path.exists():continue
        try:
            report=load(path)
            if not isinstance(report,dict):raise ValueError("report must be an object")
            if name in meta["unit_gate_receipts"]:
                failed=(report.get("result")!="passed" or report.get("exit_code")!=0 or report.get("require_tests") is not True
                        or report.get("observed_test_summary_count",0)<=0 or report.get("observed_skips")!=0
                        or report.get("observed_failed",0)!=0 or report.get("timed_out") or report.get("output_limit_exceeded"))
            else:failed=identity_review_decision_A54(report) not in {"accept","accept_source","accept_source_only"}
        except (ValueError,TypeError):failed=True
        if failed:failures[name]={"path":value,"sha256":digest(path)}
    return failures

def identity_attempt_lifecycle_A54(root,child,contract):
    """Validate the retained append-only coordinator journal, not process liveness."""
    meta=contract["metadata"];assignment=load(safe_relative(root,meta["source4_assignment"]))
    path=safe_relative(root,meta["lifecycle"]);raw=path.read_bytes()
    binding=child.get("lifecycle_A54",{})
    if binding.get("path")!=meta["lifecycle"] or binding.get("sha256")!=hashlib.sha256(raw).hexdigest():
        raise ValueError("A54 lifecycle is missing, deleted, truncated or differs from its state binding")
    lines=raw.splitlines();events=[];previous=None
    for index,line in enumerate(lines):
        event=json.loads(line)
        if line!=json.dumps(event,sort_keys=True,separators=(",",":")).encode():raise ValueError("A54 lifecycle encoding changed")
        if type(event.get("sequence")) is not int:raise ValueError("A54 lifecycle sequence must be an integer")
        if index==0:
            if event!=identity_attempt_seed_A54(assignment.get("run_nonce")) or identity_attempt_event_hash_A54(event)!=assignment.get("lifecycle_seed_sha256"):
                raise ValueError("A54 lifecycle seed is not bound to the sole assignment")
        elif (event.get("slice")!="F02e2i" or event.get("attempt")!=4 or event.get("run_nonce")!=assignment["run_nonce"]
                or event.get("sequence")!=index or event.get("previous_sha256")!=previous
                or event.get("phase") not in A54_IDENTITY_PHASE_NEXT[events[-1]["phase"]]):
            raise ValueError("A54 lifecycle was reordered, reset, retried or continued after terminal state")
        expected_artifact=identity_attempt_artifact_A54(meta,event["phase"])
        if expected_artifact is not None:
            ref=event.get("artifact",{})
            if ref.get("path")!=expected_artifact or digest(safe_relative(root,ref["path"]))!=ref.get("sha256"):
                raise ValueError("A54 lifecycle artifact was deleted, replaced or moved")
        elif "artifact" in event:raise ValueError("A54 lifecycle cannot reference an alternate review/gate")
        if event["phase"]=="terminal_failed":
            history=child.get("attempt_history",{}).get("4")
            if (not isinstance(history,dict) or history.get("result")!="failed_terminal"
                    or history.get("attempt")!=4 or history.get("assignment_sha256")!=child.get("assignment_sha256")
                    or history.get("terminal")!=event.get("artifact")
                    or identity_attempt_event_hash_A54(history)!=event.get("history_record_sha256")):
                raise ValueError("A54 retained terminal source4 history was deleted or changed")
        previous=identity_attempt_event_hash_A54(event);events.append(event)
    if not events or binding.get("events")!=len(events) or binding.get("phase")!=events[-1]["phase"]:
        raise ValueError("A54 lifecycle head/length is missing or changed")
    return events

def append_identity_attempt_event_A54(root,child,contract,phase,artifact=None):
    """Coordinator-only append; never erase/replace a source4 event or restart it."""
    meta=contract["metadata"];assignment=load(safe_relative(root,meta["source4_assignment"]))
    path=safe_relative(root,meta["lifecycle"])
    if path.exists():
        events=identity_attempt_lifecycle_A54(root,child,contract)
        if phase not in A54_IDENTITY_PHASE_NEXT[events[-1]["phase"]]:raise ValueError("A54 lifecycle forbids this retry/transition")
        event={"slice":"F02e2i","attempt":4,"run_nonce":assignment["run_nonce"],"sequence":len(events),
               "previous_sha256":identity_attempt_event_hash_A54(events[-1]),"phase":phase}
        mode="ab"
    else:
        if (phase!="assigned" or child.get("lifecycle_A54") or child.get("attempts")!=3 or child.get("status")!="ready"
                or "4" in child.get("attempt_history",{})):
            raise ValueError("A54 lifecycle seed is created only during the one ready3-to-assigned4 transition; never recreated")
        event=identity_attempt_seed_A54(assignment["run_nonce"]);mode="xb"
        if identity_attempt_event_hash_A54(event)!=assignment.get("lifecycle_seed_sha256"):raise ValueError("A54 assignment seed digest mismatch")
    expected_artifact=identity_attempt_artifact_A54(meta,phase)
    if artifact!=expected_artifact:raise ValueError("A54 event must bind only its exact required artifact")
    if artifact is not None:
        file=safe_relative(root,artifact);event["artifact"]={"path":artifact,"sha256":digest(file)}
        report=load(file)
        if phase in {"source_review_passed","integrated_review_passed","accepted"}:
            if identity_review_decision_A54(report) not in {"accept","accept_source","accept_source_only"}:
                raise ValueError("A54 cannot append a failed review as passed")
        elif phase in {"focused_passed","regression_passed"}:
            if (report.get("result")!="passed" or report.get("exit_code")!=0
                    or report.get("require_tests") is not True
                    or report.get("observed_test_summary_count",0)<=0 or report.get("observed_skips")!=0
                    or report.get("observed_failed",0)!=0 or report.get("timed_out") or report.get("output_limit_exceeded")):
                raise ValueError("A54 cannot append a failed/zero/skip command as passed")
    if phase=="terminal_failed":
        history=child.get("attempt_history",{}).get("4")
        if (not isinstance(history,dict) or history.get("result")!="failed_terminal" or history.get("attempt")!=4
                or history.get("assignment_sha256")!=child.get("assignment_sha256") or history.get("terminal")!=event.get("artifact")):
            raise ValueError("A54 terminal event requires retained assignment/terminal source4 failure history")
        event["history_record_sha256"]=identity_attempt_event_hash_A54(history)
    encoded=(json.dumps(event,sort_keys=True,separators=(",",":"))+"\n").encode()
    with path.open(mode) as output:output.write(encoded);output.flush();os.fsync(output.fileno())
    child["lifecycle_A54"]={"path":meta["lifecycle"],"sha256":digest(path),"events":event["sequence"]+1,"phase":phase}
    identity_attempt_lifecycle_A54(root,child,contract)
    return event
def load(path:Path):
    with path.open(encoding="utf-8") as f:return json.load(f)
def write_json(path:Path,value):
    path.parent.mkdir(parents=True,exist_ok=True)
    fd,tmp=tempfile.mkstemp(prefix=".ferric-",dir=path.parent)
    try:
        with os.fdopen(fd,"w",encoding="utf-8") as f:
            json.dump(value,f,indent=2,ensure_ascii=False);f.write("\n");f.flush();os.fsync(f.fileno())
        os.replace(tmp,path)
    finally:
        if os.path.exists(tmp):os.unlink(tmp)
def digest(path:Path):
    h=hashlib.sha256()
    with path.open("rb") as f:
        for b in iter(lambda:f.read(1024*1024),b""):h.update(b)
    return h.hexdigest()
def now():return datetime.now(timezone.utc).isoformat()
def git(repo:Path|None,*args:str,check=True):
    cmd=["git","-c","core.hooksPath=/dev/null"]
    if repo is not None:cmd += ["-C",str(repo)]
    r=subprocess.run(cmd+list(args),capture_output=True,text=True,timeout=180)
    if check and r.returncode:raise RuntimeError(f"git {args[0]} failed: {r.stderr.strip()[:500]}")
    return r.stdout.strip() if check else r

def canonical_remote(value:str):
    v=value.strip().rstrip("/")
    if v.endswith(".git"):v=v[:-4]
    for prefix in ("https://github.com/","ssh://git@github.com/","git@github.com:"):
        if v.startswith(prefix):return v[len(prefix):].lower()
    return None

def validate_repo(repo:Path,clean=False):
    if repo.is_symlink() or not repo.is_dir():raise ValueError("Repository must be an existing real directory")
    top=Path(git(repo,"rev-parse","--show-toplevel")).resolve()
    if top!=repo.resolve():raise ValueError("Specify the repository root, not a subdirectory")
    if canonical_remote(git(repo,"remote","get-url","origin"))!=REPO:
        raise ValueError("origin is not the exact Ferric GitHub repository")
    if clean and git(repo,"status","--porcelain","--untracked-files=all"):
        raise ValueError("Checkout is dirty; preserve it and use a fresh clone/worktree")
    return top

def repo_snapshot(repo:Path):
    sha=git(repo,"rev-parse","HEAD")
    status=git(repo,"status","--porcelain","--untracked-files=all")
    diff=git(repo,"diff","--binary","HEAD")
    return {"sha":sha,"tree":git(repo,"rev-parse","HEAD^{tree}"),"dirty":bool(status),
            "working_state_digest":hashlib.sha256((status+"\n"+diff).encode()).hexdigest()}

def safe_relative(root:Path,value:str):
    p=Path(value)
    if p.is_absolute() or ".." in p.parts:raise ValueError(f"Unsafe relative path: {value}")
    q=(root/p).resolve()
    if not q.is_relative_to(root.resolve()):raise ValueError(f"Path escapes root: {value}")
    return q

def read_plan(path:Path|None=None):return load(path or PACK/"control/plan.json")

def workspace_root(pack:Path|None=None):
    """Find the external run workspace from either the main checkout or an isolated worktree."""
    start=(pack or PACK).resolve()
    for candidate in (start,*start.parents):
        if all((candidate/name).exists() for name in ("RUN_CONTEXT.json","control","evidence","packets")):
            return candidate
    raise FileNotFoundError("Could not locate the Ferric execution workspace above the pack")

def effective_attempt_limit(plan,task):
    """Return only a schema-authorized per-task override; malformed overrides fail closed."""
    default=plan.get("limits",{}).get("max_attempts",3)
    task_limit=task.get("max_attempts",default)
    exceptions=plan.get("attempt_exceptions",{})
    if not isinstance(exceptions,dict):return 0
    entry=exceptions.get(task.get("id"))
    if not isinstance(default,int) or isinstance(default,bool) or default!=GLOBAL_MAX_ATTEMPTS:return 0
    if not isinstance(task_limit,int) or isinstance(task_limit,bool) or task_limit<1:return 0
    if entry is None:return min(default,task_limit)
    sid=task.get("id")
    expected_amendment=ATTEMPT_EXCEPTION_AMENDMENTS.get(sid)
    expected_limit=ATTEMPT_EXCEPTION_LIMITS.get(sid)
    if (expected_amendment is not None
        and expected_limit is not None
        and isinstance(entry,dict)
        and entry.get("amendment")==expected_amendment
        and entry.get("default_limit")==default
        and entry.get("authorized_limit")==task_limit==expected_limit
        and isinstance(entry.get("reason"),str)
        and bool(entry["reason"].strip())):
        return task_limit
    return default

def attempt_limited(task):
    """Attempt counts constrain dispatchable implementation/proof work, not aggregates."""
    return bool(task.get("allowed_paths")) and task.get("kind")!="aggregate"

def packet(task,pack=PACK):
    index=load(pack/"control/contracts.json")
    if isinstance(index,list):index={x["id"]:x for x in index}
    parts=[(pack/"WORKER.md").read_text(),(pack/task["slice_file"]).read_text()]
    for c in task["contracts"]:
        record=index[c]
        path=record if isinstance(record,str) else record.get("file",record.get("path"))
        parts.append((pack/path).read_text())
    return "\n\n---\n\n".join(parts)

def no_symlink_chain(path:Path):
    for part in [path,*path.parents]:
        if part.is_symlink():raise ValueError(f"Symlink not permitted in controlled path: {part}")
