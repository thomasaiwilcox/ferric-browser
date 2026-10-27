"""Standard-library helpers. No network requests and no implicit source mutations."""
from __future__ import annotations
import hashlib,json,os,re,subprocess,tempfile
from pathlib import Path
from datetime import datetime,timezone
PACK=Path(__file__).resolve().parent.parent
REPO="thomasaiwilcox/ferric-browser"
CLONE=f"https://github.com/{REPO}.git"
ATTEMPT_EXCEPTION_AMENDMENTS={"P00a":"A37","W01b":"A37","F02e2":"A48","F02e2i":"A54","F02e2s":"A55"}
ATTEMPT_EXCEPTION_LIMITS={"P00a":4,"W01b":4,"F02e2":6,"F02e2i":4,"F02e2s":4}
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

def identity_audit_git_A54(root,contract,assignment_path,create=False):
    """Separate, owned audit refs outlive rollback of mutable state/evidence."""
    meta=contract["metadata"];assignment=load(assignment_path)
    path=safe_relative(root,meta["audit_repository"]);owner_path=path/".ferric-a54-owner.json"
    owner={"kind":meta.get("audit_owner_kind","F02e2i-A54-lifecycle"),"assignment_sha256":digest(assignment_path),"run_nonce":assignment["run_nonce"]}
    environment={k:v for k,v in os.environ.items() if not k.startswith("GIT_")}
    if not path.exists():
        if not create:raise ValueError("A54 independent audit store is missing; never recreate an assigned4 store")
        path.mkdir(parents=True);write_json(owner_path,owner)
        result=subprocess.run(["git","-c","core.hooksPath=/dev/null","init","--bare","--object-format=sha1",str(path)],env=environment,capture_output=True)
        if result.returncode:raise ValueError("A54 could not initialize its owned audit store")
    if not owner_path.is_file() or load(owner_path)!=owner:raise ValueError("A54 audit store owner/assignment changed")
    def command(arguments,data=None,allow_missing=False):
        result=subprocess.run(["git","-c","core.hooksPath=/dev/null","-c","gc.auto=0","--git-dir",str(path),*arguments],
                              env=environment,input=data,capture_output=True)
        if result.returncode:
            if allow_missing and result.returncode==1:return b""
            raise ValueError("A54 audit Git operation failed: "+result.stderr.decode(errors="replace")[:300])
        return result.stdout
    if command(["rev-parse","--is-bare-repository"]).strip()!=b"true":raise ValueError("A54 audit store is not its own bare repository")
    return command,meta["audit_ref_prefix"]+assignment["run_nonce"]

def identity_attempt_audit_A54(root,child,contract,events,create_event=None):
    assignment_path=safe_relative(root,contract["metadata"]["source4_assignment"])
    command,prefix=identity_audit_git_A54(root,contract,assignment_path,create=create_event is not None and not events)
    refs={}
    for line in command(["for-each-ref","--format=%(refname) %(objectname)",prefix+"/events/"]).decode().splitlines():
        name,oid=line.split();refs[name]=oid
    expected={}
    for index,event in enumerate(events):
        encoded=json.dumps(event,sort_keys=True,separators=(",",":")).encode()
        oid=hashlib.sha1(b"blob "+str(len(encoded)).encode()+b"\0"+encoded).hexdigest()
        expected[prefix+f"/events/{index:03d}"]=oid
    head=command(["rev-parse","--verify","--quiet",prefix+"/head"],allow_missing=True).decode().strip()
    last=list(expected.values())[-1] if expected else ""
    if refs!=expected or head!=last:raise ValueError("A54 independent audit witness rejects journal/state/history rollback")
    if create_event is not None:
        encoded=json.dumps(create_event,sort_keys=True,separators=(",",":")).encode()
        oid=command(["hash-object","-w","--stdin"],encoded).decode().strip()
        # Each event ref is created once; the head moves only by compare-and-swap.
        # Per-event refs retain old blobs even when the head advances.
        command(["update-ref",prefix+f"/events/{len(events):03d}",oid,"0"*40])
        command(["update-ref",prefix+"/head",oid,head or "0"*40])

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
    identity_attempt_audit_A54(root,child,contract,events)
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
        events=[]
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
    identity_attempt_audit_A54(root,child,contract,events,create_event=event)
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

def parent_attempt_phases_A55():
    phases=["assigned","source_review_started","source_review_passed","integration_started",
            "integrated_review_started","integrated_review_passed","runner_review_started","runner_review_passed"]
    for index in range(7):phases.extend((f"gate_{index}_started",f"gate_{index}_passed"))
    return phases+["gate_evidence_review_started","accepted"]

def parent_attempt_seed_A55(nonce):
    if not isinstance(nonce,str) or re.fullmatch(r"[0-9a-f]{32}",nonce) is None:
        raise ValueError("A55 requires one unique32-hex assignment nonce")
    return {"slice":"F02e2s","attempt":4,"run_nonce":nonce,"sequence":0,"previous_sha256":None,"phase":"assigned"}

def parent_attempt_artifact_A55(meta,phase):
    fixed={"source_review_started":"source_freeze","source_review_passed":"source_review",
           "integrated_review_started":"integrated_freeze","integrated_review_passed":"integrated_source_review",
           "runner_review_started":"native_policy_freeze","runner_review_passed":"native_runner_review",
           "gate_0_started":"manifest_binding","gate_evidence_review_started":"native_run",
           "accepted":"gate_evidence_review","terminal_failed":"terminal_marker"}
    if phase in fixed:return meta[fixed[phase]]
    if re.fullmatch(r"gate_[0-6]_passed",phase):return meta["native_gate_receipts"][int(phase[5])]
    return None

def parent_attempt_failure_artifacts_A55(root,contract):
    meta=contract["metadata"];failures={}
    paths={key:meta[key] for key in ("source_review","integrated_source_review","native_runner_review","gate_evidence_review","native_run")}
    paths.update({f"gate_{i}":path for i,path in enumerate(meta["native_gate_receipts"])})
    paths["worker_result"]=meta["worker_result"];paths["source_budget"]=meta["source_budget"]
    for name,value in paths.items():
        path=safe_relative(root,value)
        if not path.exists():continue
        try:
            report=load(path)
            if not isinstance(report,dict):raise ValueError("artifact must be an object")
            if name=="worker_result":failed=report.get("result")!=meta["worker_success_result"]
            elif name=="source_budget":
                amount=report.get("source_bytes_emitted")
                failed=type(amount) is not int or not 0<=amount<=96000 or report.get("hard_limit",96000)!=96000
            elif name.endswith("review"):
                failed=identity_review_decision_A54(report)!="accept"
            else:
                failed=report.get("result")!="passed" or report.get("timed_out") or report.get("output_limit_exceeded")
                if name.startswith("gate_"):
                    index=int(name[5:])
                    if index!=4:failed=failed or report.get("exit_code")!=0
                    failed=failed or report.get("observed_failed",0)!=0 or report.get("observed_skips",0)!=0
                    if index==6:
                        failed=failed or report.get("require_tests") is not True or report.get("observed_test_summary_count",0)<=0
        except (ValueError,TypeError):failed=True
        if failed:failures[name]={"path":value,"sha256":digest(path)}
    return failures

def parent_attempt_lifecycle_A55(root,parent,contract):
    meta=contract["metadata"];assignment=load(safe_relative(root,meta["source4_assignment"]))
    path=safe_relative(root,meta["lifecycle"]);raw=path.read_bytes();binding=parent.get("lifecycle_A55",{})
    if binding.get("path")!=meta["lifecycle"] or binding.get("sha256")!=hashlib.sha256(raw).hexdigest():
        raise ValueError("A55 journal missing, replaced or rolled back")
    phases=parent_attempt_phases_A55();events=[]
    for index,line in enumerate(raw.splitlines()):
        event=json.loads(line)
        if line!=json.dumps(event,sort_keys=True,separators=(",",":")).encode():raise ValueError("A55 journal encoding changed")
        if index==0:
            if event!=parent_attempt_seed_A55(assignment.get("run_nonce")) or identity_attempt_event_hash_A54(event)!=assignment.get("lifecycle_seed_sha256"):
                raise ValueError("A55 journal seed differs from the sole assignment")
        else:
            previous=events[-1];phase=event.get("phase");old=previous["phase"]
            next_phase=phases[phases.index(old)+1] if old in phases[:-1] else None
            if (old in {"accepted","terminal_failed"} or phase not in {next_phase,"terminal_failed"}
                    or event.get("slice")!="F02e2s" or event.get("attempt")!=4
                    or event.get("run_nonce")!=assignment["run_nonce"] or type(event.get("sequence")) is not int
                    or event["sequence"]!=index or event.get("previous_sha256")!=identity_attempt_event_hash_A54(previous)):
                raise ValueError("A55 journal reordered, restarted or continued after terminal outcome")
        expected=parent_attempt_artifact_A55(meta,event["phase"])
        if expected is not None:
            artifact=event.get("artifact",{})
            if artifact.get("path")!=expected or digest(safe_relative(root,expected))!=artifact.get("sha256"):
                raise ValueError("A55 fixed artifact deleted, replaced, moved or on an alternate path")
        elif "artifact" in event:raise ValueError("A55 unexpected phase artifact")
        if event["phase"]=="terminal_failed":
            history=parent.get("attempt_history",{}).get("4")
            if (not isinstance(history,dict) or history.get("attempt")!=4 or history.get("result")!="failed_terminal"
                    or history.get("assignment_sha256")!=parent.get("assignment_sha256") or history.get("terminal")!=event.get("artifact")
                    or identity_attempt_event_hash_A54(history)!=event.get("history_record_sha256")):
                raise ValueError("A55 terminal failure history changed")
        events.append(event)
    if not events or binding.get("events")!=len(events) or binding.get("phase")!=events[-1]["phase"]:
        raise ValueError("A55 journal head/length changed")
    # Same create-once Git event refs and CAS head used by A54; owner namespace,
    # assignment, nonce and separate store are pinned by the A55 contract.
    identity_attempt_audit_A54(root,parent,contract,events)
    return events

def append_parent_attempt_event_A55(root,parent,contract,phase,artifact=None):
    meta=contract["metadata"];assignment=load(safe_relative(root,meta["source4_assignment"]))
    path=safe_relative(root,meta["lifecycle"]);phases=parent_attempt_phases_A55()
    if phase!="terminal_failed" and safe_relative(root,meta["terminal_marker"]).exists():
        raise ValueError("A55 terminal marker forbids all subsequent activity")
    if path.exists():
        events=parent_attempt_lifecycle_A55(root,parent,contract);old=events[-1]["phase"]
        if old not in phases[:-1] or phase not in {phases[phases.index(old)+1],"terminal_failed"}:
            raise ValueError("A55 cannot repeat, resume or restart a phase")
        event={"slice":"F02e2s","attempt":4,"run_nonce":assignment["run_nonce"],"sequence":len(events),
               "previous_sha256":identity_attempt_event_hash_A54(events[-1]),"phase":phase};mode="ab"
    else:
        if (phase!="assigned" or parent.get("lifecycle_A55") or parent.get("attempts")!=3 or parent.get("status")!="ready"
                or "4" in parent.get("attempt_history",{})):
            raise ValueError("A55 seed only at the one ready3-to-assigned4 transition; never recreate an assigned store")
        event=parent_attempt_seed_A55(assignment["run_nonce"]);events=[];mode="xb"
        if identity_attempt_event_hash_A54(event)!=assignment.get("lifecycle_seed_sha256"):raise ValueError("A55 seed digest mismatch")
    if artifact!=parent_attempt_artifact_A55(meta,phase):raise ValueError("A55 phase requires its fixed artifact")
    if phase!="terminal_failed" and parent_attempt_failure_artifacts_A55(root,contract):
        raise ValueError("A55 known failure is permanent; only terminal failure recording is permitted")
    if phase=="source_review_started" and any(not safe_relative(root,meta[key]).is_file() for key in ("worker_result","source_budget","source_read_log")):
        raise ValueError("A55 source freeze requires the actual fixed worker result/budget/read log")
    if artifact is not None:
        file=safe_relative(root,artifact);event["artifact"]={"path":artifact,"sha256":digest(file)};report=load(file)
        if not isinstance(report,dict):raise ValueError("A55 artifact must be an object")
        worker_phase=phase in {"source_review_started","source_review_passed"}
        source=parent.get("implementation_sha") if worker_phase else parent.get("source_sha")
        tree=parent.get("implementation_tree") if worker_phase else parent.get("source_tree")
        if phase!="terminal_failed" and any(not isinstance(value,str) or re.fullmatch(r"[0-9a-f]{40}",value) is None for value in (source,tree)):
            raise ValueError("A55 phase requires the frozen actual source/tree")
        if phase.endswith("review_passed") or phase in {"source_review_passed","accepted"}:
            if identity_review_decision_A54(report)!="accept":raise ValueError("A55 review did not accept")
            candidate=report.get("candidate",{})
            actual=report.get("source_commit",report.get("source_sha",candidate.get("commit")))
            actual_tree=report.get("source_tree",candidate.get("tree"))
            if (actual!=source or actual_tree!=tree or not report.get("reviewer")
                    or report["reviewer"] in {parent.get("author"),"/root"}
                    or candidate.get("slice")!="F02e2s" or candidate.get("amendment")!="A55"
                    or type(candidate.get("attempt")) is not int or candidate.get("attempt")!=4):
                raise ValueError("A55 review does not independently bind this exact source4")
        elif phase.endswith("_passed") or phase=="gate_evidence_review_started":
            if report.get("result")!="passed":raise ValueError("A55 gate did not pass")
            if phase.startswith("gate_") and phase.endswith("_passed"):
                index=int(phase[5]);target=str((root/meta["target_relative"]).resolve())
                if index==4:
                    if report.get("source_sha")!=source or report.get("source_tree")!=tree or report.get("target")!=target:
                        raise ValueError("A55 link report source/target mismatch")
                else:
                    for key in ("source_before","source_after"):
                        snapshot=report.get(key,{})
                        if index==0:snapshot=snapshot.get("candidate",{})
                        if snapshot.get("sha")!=source or snapshot.get("tree")!=tree or snapshot.get("dirty") is not False:
                            raise ValueError("A55 native gate requires exact clean source before/after")
                    if index==0:
                        if report.get("target")!=target:raise ValueError("A55 SDK target mismatch")
                    elif report.get("native_target")!=target or report.get("execution_binding",{}).get("native_target")!=target:
                        raise ValueError("A55 command target mismatch")
        elif phase in {"source_review_started","integrated_review_started","runner_review_started","gate_0_started"}:
            if report.get("source_sha")!=source or report.get("source_tree")!=tree:
                raise ValueError("A55 phase freeze does not bind the actual source/tree")
    if phase=="terminal_failed":
        history=parent.get("attempt_history",{}).get("4",{})
        if (history.get("attempt")!=4 or history.get("result")!="failed_terminal"
                or history.get("assignment_sha256")!=parent.get("assignment_sha256") or history.get("terminal")!=event.get("artifact")):
            raise ValueError("A55 terminal requires retained failed4 history")
        event["history_record_sha256"]=identity_attempt_event_hash_A54(history)
    identity_attempt_audit_A54(root,parent,contract,events,create_event=event)
    with path.open(mode) as output:
        output.write((json.dumps(event,sort_keys=True,separators=(",",":"))+"\n").encode());output.flush();os.fsync(output.fileno())
    parent["lifecycle_A55"]={"path":meta["lifecycle"],"sha256":digest(path),"events":event["sequence"]+1,"phase":phase}
    parent_attempt_lifecycle_A55(root,parent,contract)
    return event
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
