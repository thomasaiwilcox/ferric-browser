#!/usr/bin/env python3
"""Run one explicit command synchronously, bound its log and record real source/results."""
from __future__ import annotations
import argparse,json,os,re,selectors,shlex,signal,subprocess,time
from pathlib import Path
from common import repo_snapshot,write_json,digest,now

def test_counts(text):
    passed=0;skipped=0
    for pat in (r"test result: ok\.\s+(\d+) passed",r"Totals:\s*(\d+) passed",r"\b(\d+) passed(?:,| in |$)",r"out of\s+(\d+)\s*\n",r"Ran\s+(\d+) tests?"):
        passed+=sum(int(x) for x in re.findall(pat,text,re.M))
    for pat in (r"\b(\d+) skipped\b",r"\b(\d+) ignored\b"):
        skipped+=sum(int(x) for x in re.findall(pat,text))
    if re.search(r"\*\*\*Skipped|\(Skipped\)",text):skipped=max(1,skipped)
    return passed,skipped

def terminate_owned(p):
    try:os.killpg(p.pid,signal.SIGTERM)
    except ProcessLookupError:return
    try:
        p.wait(timeout=1)
        # The group can still contain children after its leader has exited.
        try:os.killpg(p.pid,signal.SIGKILL)
        except ProcessLookupError:pass
    except subprocess.TimeoutExpired:
        try:os.killpg(p.pid,signal.SIGKILL)
        except ProcessLookupError:pass
        p.wait(timeout=5)

def qt_include_escape(text:str,prefix:Path):
    """Return the first Qt include path outside the required SDK, if any."""
    prefix=prefix.resolve()
    try:tokens=shlex.split(text,posix=True)
    except ValueError:tokens=text.split()
    include_flags=("-I","-isystem","-iquote","-idirafter","-F")
    for i,token in enumerate(tokens):
        path=None
        if token in include_flags and i+1<len(tokens):path=tokens[i+1]
        else:
            for flag in include_flags:
                if token.startswith(flag) and len(token)>len(flag):
                    path=token[len(flag):].lstrip("=")
                    break
        if not path:continue
        candidate=Path(path)
        if not candidate.is_absolute():continue
        if not any("qt" in part.lower() for part in candidate.parts):continue
        lexical=Path(os.path.abspath(candidate))
        resolved=candidate.resolve()
        if ((lexical!=prefix and prefix not in lexical.parents)
                or (resolved!=prefix and prefix not in resolved.parents)):
            return str(candidate)
    return None

def run(repo:Path,out:Path,label:str,cmd:list[str],timeout:float,require_tests=False,max_log_bytes=64*1024*1024,qt_include_prefix:Path|None=None):
    if not cmd:raise ValueError("An explicit command after -- is required")
    if timeout<=0:raise ValueError("timeout must be positive")
    if out.exists() and any(out.iterdir()):raise ValueError("Evidence output directory must be new or empty")
    out.mkdir(parents=True,exist_ok=True)
    before=repo_snapshot(repo);start=now();tick=time.monotonic();log=out/"output.log"
    timed_out=False;overflow=False;launch_error=None;exit_code=None;p=None;qt_violation=None;scan_buffer=""
    try:
        with log.open("wb") as f:
            p=subprocess.Popen(cmd,cwd=repo,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
            assert p.stdout is not None
            sel=selectors.DefaultSelector();sel.register(p.stdout,selectors.EVENT_READ);total=0
            while sel.get_map():
                if time.monotonic()-tick>timeout:
                    timed_out=True;terminate_owned(p);break
                for key,_ in sel.select(0.1):
                    data=os.read(key.fileobj.fileno(),65536)
                    if not data:sel.unregister(key.fileobj);continue
                    remaining=max_log_bytes-total
                    if len(data)>remaining:
                        f.write(data[:max(0,remaining)]);total=max_log_bytes;overflow=True;terminate_owned(p);break
                    f.write(data);total+=len(data)
                    if qt_include_prefix is not None:
                        scan_buffer=(scan_buffer+data.decode(errors="replace"))[-1024*1024:]
                        qt_violation=qt_include_escape(scan_buffer,qt_include_prefix)
                        if qt_violation is not None:
                            terminate_owned(p)
                            break
                if overflow:break
                if qt_violation is not None:break
            sel.close()
            if p.poll() is None:
                left=max(0.01,timeout-(time.monotonic()-tick))
                try:p.wait(timeout=left)
                except subprocess.TimeoutExpired:timed_out=True;terminate_owned(p)
            exit_code=p.returncode
    except OSError as e:launch_error=str(e)
    finally:
        if p is not None and p.poll() is None:terminate_owned(p)
        if p is not None and p.stdout is not None:p.stdout.close()
    if not log.exists():log.write_bytes(b"")
    after=repo_snapshot(repo);text=log.read_text(errors="replace")
    count,skips=test_counts(text)
    failures=sum(int(x) for x in re.findall(r"\b(\d+) failed\b",text))
    passed=exit_code==0 and not timed_out and not overflow and not launch_error and qt_violation is None
    if failures>0:passed=False
    if require_tests and (count<=0 or skips>0):passed=False
    result={"schema_version":1,"label":label,"command":cmd,"cwd":str(repo),"started":start,"finished":now(),
            "elapsed_seconds":round(time.monotonic()-tick,3),"exit_code":exit_code,"timed_out":timed_out,
            "output_limit_exceeded":overflow,"launch_error":launch_error,"require_tests":require_tests,
            "observed_test_summary_count":count,"observed_skips":skips,"observed_failed":failures,
            "result":"passed" if passed else "failed","source_before":before,"source_after":after,
            "qt_include_prefix":str(qt_include_prefix.resolve()) if qt_include_prefix is not None else None,
            "qt_include_guard":"stopped" if qt_violation is not None else ("clear" if qt_include_prefix is not None else "not_requested"),
            "qt_include_violation":qt_violation,
            "log":"output.log","log_sha256":digest(log),
            "note":"Summary counts are a guard, not authoritative unique test counts or proof of assertion quality. Reviewer must map real tests to case IDs."}
    write_json(out/"receipt.json",result);return result

def main():
    a=argparse.ArgumentParser(description=__doc__);a.add_argument("--repo",required=True,type=Path);a.add_argument("--out",required=True,type=Path)
    a.add_argument("--label",required=True);a.add_argument("--timeout",type=float,default=1800);a.add_argument("--require-tests",action="store_true")
    a.add_argument("--qt-include-prefix",type=Path,help="stop the owned process immediately if a Qt include flag resolves outside this SDK")
    a.add_argument("command",nargs=argparse.REMAINDER);n=a.parse_args()
    cmd=n.command[1:] if n.command[:1]==["--"] else n.command
    try:
        r=run(n.repo,n.out,n.label,cmd,n.timeout,n.require_tests,qt_include_prefix=n.qt_include_prefix)
        print(json.dumps({k:r[k] for k in ("result","exit_code","timed_out","observed_test_summary_count","observed_skips","qt_include_guard","qt_include_violation")},indent=2))
        raise SystemExit(0 if r["result"]=="passed" else 1)
    except (ValueError,RuntimeError,OSError) as e:a.exit(1,f"Gate failed: {e}\n")
if __name__=="__main__":main()
