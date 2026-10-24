"""Standard-library helpers. No network requests and no implicit source mutations."""
from __future__ import annotations
import hashlib,json,os,re,subprocess,tempfile
from pathlib import Path
from datetime import datetime,timezone
PACK=Path(__file__).resolve().parent.parent
REPO="thomasaiwilcox/ferric-browser"
CLONE=f"https://github.com/{REPO}.git"
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
