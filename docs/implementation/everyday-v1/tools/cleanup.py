#!/usr/bin/env python3
"""Dry-run by default; delete only registered inactive run-owned build children."""
import argparse,json,re,shutil,os
from pathlib import Path
from common import load,write_json,no_symlink_chain,now

def candidates(workspace:Path):
    workspace=workspace.expanduser().absolute();no_symlink_chain(workspace)
    owner=load(workspace/".ferric-workspace.json");run_id=owner["run_id"]
    if (workspace/"control/build-active.lock").exists():raise ValueError("An active build lock exists; cleanup refused")
    reg=load(workspace/"control/builds.json")
    if reg["run_id"]!=run_id:raise ValueError("Build registry owner mismatch")
    if any(x.get("active") for x in reg["artifacts"]):raise ValueError("Registered build is active")
    builds=workspace/"builds";no_symlink_chain(builds);found=[]
    for x in reg["artifacts"]:
        name=x.get("name","")
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,100}",name) or name in {".",".."}:raise ValueError("Invalid artifact name")
        p=builds/name;no_symlink_chain(p)
        if not p.exists():continue
        if not p.is_dir() or not p.resolve().is_relative_to(builds.resolve()):raise ValueError("Unsafe artifact path")
        marker=p/".ferric-build-owner.json"
        if marker.is_symlink() or not marker.is_file():raise ValueError("Missing regular owner marker")
        if load(marker).get("run_id")!=run_id:raise ValueError("Artifact owner mismatch")
        for base,dirs,files in os.walk(p,followlinks=False):
            if ".git" in dirs or ".git" in files:
                raise ValueError("Registered artifact contains a source checkout; cleanup refused")
        if not x.get("retain",False):found.append(name)
    return workspace,reg,found

def clean(workspace:Path,names:list[str],apply=False):
    workspace,reg,found=candidates(workspace)
    if not apply:return {"dry_run":True,"eligible":found,"deleted":[]}
    if not names:raise ValueError("--apply also requires explicit --artifact names from the dry run")
    if set(names)-set(found):raise ValueError("Requested artifact is not an eligible registered build")
    if not shutil.rmtree.avoids_symlink_attacks:raise ValueError("Platform lacks fd-safe tree deletion")
    deleted=[]
    for name in names:
        # Revalidate markers and build lock immediately before each removal.
        _,_,fresh=candidates(workspace)
        if name not in fresh:raise ValueError("Artifact eligibility changed")
        shutil.rmtree(workspace/"builds"/name);deleted.append(name)
    reg["artifacts"]=[x for x in reg["artifacts"] if x["name"] not in deleted]
    write_json(workspace/"control/builds.json",reg)
    report={"dry_run":False,"deleted":deleted,"time":now()}
    write_json(workspace/"control/last-cleanup.json",report);return report

def main():
    a=argparse.ArgumentParser(description=__doc__);a.add_argument("--workspace",required=True,type=Path);a.add_argument("--apply",action="store_true");a.add_argument("--artifact",action="append",default=[]);n=a.parse_args()
    try:print(json.dumps(clean(n.workspace,n.artifact,n.apply),indent=2))
    except (ValueError,KeyError,OSError) as e:a.exit(1,f"Cleanup refused: {e}\n")
if __name__=="__main__":main()
