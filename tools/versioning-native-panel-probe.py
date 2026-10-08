#!/usr/bin/python3
"""Check one disposable Ferric process for a rendered commit-list row.

This is a final-campaign diagnostic, not an application interface. It reads
only the accessibility subtree whose process ID matches the explicit target.
It never prints labels, commit IDs, page data, or other applications' state.
"""

import argparse
import re
import sys
import time
from collections import deque

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib  # noqa: E402


COMMIT_ID = re.compile(
    r"commit:[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\Z"
)
MAX_NODES = 4096
MAX_CHILDREN_PER_NODE = 128


def target_apps(pid):
    try:
        desktop = Atspi.get_desktop(0)
        child_count = min(desktop.get_child_count(), 256) if desktop is not None else 0
    except (GLib.Error, RuntimeError):
        return []
    if desktop is None:
        return []
    matches = []
    for index in range(child_count):
        try:
            candidate = desktop.get_child_at_index(index)
            if candidate is not None and candidate.get_process_id() == pid:
                matches.append(candidate)
        except (GLib.Error, RuntimeError):
            continue
    return matches


def inspect_target(app, expected_id, pid):
    queue = deque([(app, False)])
    seen = 0
    while queue and seen < MAX_NODES:
        item, within_list = queue.popleft()
        seen += 1
        try:
            child_pid = item.get_process_id()
            if child_pid > 0 and child_pid != pid:
                continue
            name = item.get_name()
            within_list = within_list or name == "Saved commits"
            if within_list and name == "Commit identifier " + expected_id:
                return True
            children = min(item.get_child_count(), MAX_CHILDREN_PER_NODE)
            for index in range(children):
                if len(queue) + seen >= MAX_NODES:
                    break
                child = item.get_child_at_index(index)
                if child is not None:
                    queue.append((child, within_list))
        except (GLib.Error, RuntimeError):
            continue
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--commit-id", required=True)
    parser.add_argument("--timeout-seconds", type=int, default=20)
    args = parser.parse_args()
    if args.pid <= 0 or not COMMIT_ID.fullmatch(args.commit_id):
        parser.error("a positive process ID and exact commit ID are required")
    if not 1 <= args.timeout_seconds <= 60:
        parser.error("timeout must be between 1 and 60 seconds")

    deadline = time.monotonic() + args.timeout_seconds
    while time.monotonic() < deadline:
        if any(inspect_target(app, args.commit_id, args.pid)
               for app in target_apps(args.pid)):
            print("PASS native commit-list accessibility row matched the saved ID")
            return 0
        time.sleep(0.1)
    print("UNOBSERVED native commit-list accessibility row", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
