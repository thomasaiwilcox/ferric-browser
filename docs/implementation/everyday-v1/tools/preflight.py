#!/usr/bin/env python3
"""Read-only checkout/environment inventory. Does not fetch or execute repository scripts."""
from __future__ import annotations
import argparse,os,platform,shutil,subprocess
from pathlib import Path
from common import PACK,load,git,validate_repo,repo_snapshot,write_json,now

def version(cmd):
    try:
        r=subprocess.run(cmd,capture_output=True,text=True,timeout=10)
        return {"command":cmd,"exit_code":r.returncode,"text":(r.stdout+r.stderr).strip()[:2000]}
    except (OSError,subprocess.TimeoutExpired) as e:return {"command":cmd,"unavailable":type(e).__name__}

def inventory(repo):
    repo=validate_repo(repo)
    baseline=load(PACK/"control/baseline.json");manifest=load(PACK/"control/source_manifest.json")
    anchors=manifest if isinstance(manifest,list) else manifest.get("files",manifest.get("anchors",[]))
    result=[]
    for entry in anchors:
        path=entry["path"];r=git(repo,"rev-parse",f"HEAD:{path}",check=False)
        result.append({"path":path,"head_blob":r.stdout.strip() if r.returncode==0 else None,
                       "working_file_exists":(repo/path).is_file(),"authoring_blob":entry.get("git_blob",entry.get("blob"))})
    usage=shutil.disk_usage(repo);fs=os.statvfs(repo)
    main=git(repo,"rev-parse","refs/remotes/origin/main",check=False)
    present=git(repo,"cat-file","-e",baseline["authoring_sha"]+"^{commit}",check=False).returncode==0
    return {"schema_version":1,"time":now(),"repository":str(repo),"source":repo_snapshot(repo),
            "fetched_main_sha":main.stdout.strip() if main.returncode==0 else None,
            "authoring_commit_available":present,"source_anchors":result,
            "platform":platform.platform(),"machine":platform.machine(),
            "tools":[version(c) for c in [["git","--version"],["rustc","-Vv"],["cargo","-V"],["cmake","--version"],["pkg-config","--modversion","Qt6Core","Qt6WebEngineCore","KF6WindowSystem"]]],
            "resources":{"free_bytes":usage.free,"total_bytes":usage.total,"free_inodes":fs.f_favail,"total_inodes":fs.f_files,
                         "btrfs_metadata":"not_probed; read separately without elevation if applicable"},
            "native":{"wayland_display_present":bool(os.getenv("WAYLAND_DISPLAY")),"hyprland_instance_present":bool(os.getenv("HYPRLAND_INSTANCE_SIGNATURE")),
                      "physical_devices":"not_probed","note":"Environment variable presence is not qualification"},
            "note":"No compile/test/runtime claim. Check current CI/SDK and reconcile paths in S00."}

def main():
    a=argparse.ArgumentParser(description=__doc__);a.add_argument("--repo",required=True,type=Path);a.add_argument("--output",required=True,type=Path);n=a.parse_args()
    try:
        if n.output.exists():raise ValueError("Output exists; choose a new evidence path")
        write_json(n.output,inventory(n.repo));print(f"Wrote {n.output}")
    except (ValueError,RuntimeError,OSError) as e:a.exit(1,f"Preflight failed: {e}\n")
if __name__=="__main__":main()
