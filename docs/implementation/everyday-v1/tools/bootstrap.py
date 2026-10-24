#!/usr/bin/env python3
"""Clone/read a clean Ferric checkout and install the pack. Never reset or push."""
from __future__ import annotations
import argparse,json,shutil,uuid
from pathlib import Path
from common import PACK,CLONE,git,load,write_json,validate_repo,now,no_symlink_chain

def bootstrap(workspace:Path,existing:Path|None=None):
    workspace=workspace.expanduser().absolute()
    no_symlink_chain(workspace)
    if workspace.exists() and (not workspace.is_dir() or any(workspace.iterdir())):
        raise ValueError("Workspace must be nonexistent or empty; resume via RUN_CONTEXT.json instead")
    branch="work/everyday-v1"
    if existing:
        existing=existing.expanduser().absolute();no_symlink_chain(existing)
        repo=validate_repo(existing,clean=True)
        if git(repo,"show-ref","--verify","--quiet",f"refs/heads/{branch}",check=False).returncode==0:
            raise ValueError("Local integration branch already exists; do not overwrite it")
        if (repo/"docs/implementation/everyday-v1").exists():
            raise ValueError("Pack is already installed; resume rather than replace it")
    workspace.mkdir(parents=True,exist_ok=True)
    run_id="everyday-"+uuid.uuid4().hex
    write_json(workspace/".ferric-workspace.json",{"schema_version":1,"run_id":run_id,"created":now()})
    try:
        if not existing:
            repo=workspace/"repo"
            git(None,"clone","--single-branch","--branch","main",CLONE,str(repo))
        validate_repo(repo,clean=True)
        git(repo,"fetch","origin","refs/heads/main:refs/remotes/origin/main")
        before=git(repo,"rev-parse","HEAD")
        fetched=git(repo,"rev-parse","refs/remotes/origin/main")
        # Existing checkouts are never implicitly merged or fast-forwarded.
        git(repo,"switch","-c",branch, before if existing else fetched)
        target=repo/"docs/implementation/everyday-v1"
        shutil.copytree(PACK,target,ignore=shutil.ignore_patterns("__pycache__","*.pyc"))
        for folder in ("control","evidence","packets","builds","worktrees"):(workspace/folder).mkdir()
        state=load(PACK/"control/state.initial.json");state["run_id"]=run_id
        state["execution_baseline_sha"]=None;state["integration_sha"]=git(repo,"rev-parse","HEAD")
        write_json(workspace/"control/state.json",state)
        write_json(workspace/"control/capabilities.json",load(PACK/"control/capabilities.initial.json"))
        write_json(workspace/"control/builds.json",{"run_id":run_id,"artifacts":[]})
        write_json(workspace/"control/test_results.json",load(PACK/"control/test_matrix.json"))
        ctx={"schema_version":1,"run_id":run_id,"created":now(),"workspace":str(workspace),
             "repository":str(repo),"pack":str(target),"branch":branch,"head_at_setup":git(repo,"rev-parse","HEAD"),
             "fetched_main_sha":fetched,"authoring_sha":load(PACK/"control/baseline.json")["authoring_sha"],
             "requires_s00_reconciliation":True,"existing_checkout_preserved":bool(existing)}
        write_json(workspace/"RUN_CONTEXT.json",ctx)
        (workspace/"control/CHECKPOINT.md").write_text("# Initial checkpoint\n\nSetup finished. Pack is uncommitted. Complete S00; do not launch product work yet.\n")
        return ctx
    except Exception as e:
        write_json(workspace/"SETUP_FAILED.json",{"run_id":run_id,"error":str(e),"time":now(),
                 "instruction":"Partial files preserved. Inspect them; do not reset/delete an existing checkout."})
        raise

def main():
    a=argparse.ArgumentParser(description=__doc__);a.add_argument("--workspace",required=True,type=Path);a.add_argument("--repo",type=Path)
    n=a.parse_args()
    try:print(json.dumps(bootstrap(n.workspace,n.repo),indent=2))
    except (ValueError,RuntimeError,OSError) as e:a.exit(1,f"Setup refused/failed: {e}\n")
if __name__=="__main__":main()
