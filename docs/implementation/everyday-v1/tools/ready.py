#!/usr/bin/env python3
"""Suggest ready nonoverlapping slices. Does not claim locks or accept work."""
import argparse,json
from pathlib import Path
from common import read_plan,load
ACTIVE={"in_progress","submitted","reviewing"}
def suggest(plan,state):
    tasks={t["id"]:t for t in plan["slices"]};states=state["slices"]
    active=[tasks[k] for k,v in states.items() if v["status"] in ACTIVE and k in tasks]
    locks=set(state.get("active_locks",{}))
    paths=set()
    for t in active:locks.update(t["locks"]);paths.update(t["allowed_paths"])
    ready=[];conflicts=[]
    if len(active)>=plan["limits"]["max_noncoordinator_agents"]:
        return {"suggestions":[],"conflicts":[],"active":len(active),"note":"Agent capacity exhausted"}
    for t in plan["slices"]:
        if states[t["id"]]["status"] not in {"not_started","ready"}:continue
        if any(states[d]["status"]!="accepted" for d in t["depends_on"]):continue
        collision=sorted((set(t["locks"])&locks)|(set(t["allowed_paths"])&paths))
        if collision:conflicts.append({"id":t["id"],"conflicts":collision});continue
        ready.append(t["id"]);locks.update(t["locks"]);paths.update(t["allowed_paths"])
        if len(ready)+len(active)>=plan["limits"]["max_noncoordinator_agents"]:break
    return {"suggestions":ready,"conflicts":conflicts,"active":len(active),"note":"Coordinator must validate proof gates, resource slots and acquire locks before dispatch"}
def main():
    a=argparse.ArgumentParser(description=__doc__);a.add_argument("--state",required=True,type=Path);a.add_argument("--plan",type=Path);n=a.parse_args()
    print(json.dumps(suggest(read_plan(n.plan),load(n.state)),indent=2))
if __name__=="__main__":main()
