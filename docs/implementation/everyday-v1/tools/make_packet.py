#!/usr/bin/env python3
"""Emit one complete bounded worker packet; never silently truncate a contract."""
import argparse
from pathlib import Path
from common import PACK,read_plan,packet

def main():
    a=argparse.ArgumentParser(description=__doc__);a.add_argument("slice_id");a.add_argument("--output",required=True,type=Path);a.add_argument("--plan",type=Path);n=a.parse_args()
    plan=read_plan(n.plan);task=next((x for x in plan["slices"] if x["id"]==n.slice_id),None)
    if task is None:a.exit(1,"Unknown slice\n")
    text=packet(task);size=len(text.encode())
    if size>plan["limits"]["max_packet_bytes"]:a.exit(1,f"Packet {size} bytes exceeds budget; split scope, do not truncate\n")
    if n.output.exists():a.exit(1,"Output exists; use a new attempt packet path\n")
    n.output.parent.mkdir(parents=True,exist_ok=True);n.output.write_text(text)
    print(f"{n.slice_id}: {size} UTF-8 bytes written to {n.output}")
if __name__=="__main__":main()
