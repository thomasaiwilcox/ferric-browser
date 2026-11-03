#!/usr/bin/env python3
"""Exercise the production Lua transaction using disposable, exact owned windows."""
from __future__ import annotations

import argparse
from dataclasses import replace
import json
from pathlib import Path
import re
import sys
import uuid

from hyprland_proof import (
    FixtureProcess, FocusEvents, OwnedWindow, ProofFailure, eval_lua,
    focus_owned, require, require_target_compositor,
)


TRANSACTION = (Path(__file__).resolve().parents[3] / "src/hyprland/page_group_transaction.lua").read_text()


class TransactionFixture(FixtureProcess):
    def close(self) -> None:
        if self.process.poll() is None:
            try:
                self._send({"op": "exit"})
                self.process.wait(timeout=4)
            except (BrokenPipeError, TimeoutError):
                pass
            except Exception:
                # The base cleanup still owns termination/reaping on failure.
                pass
        natural = self.process.poll() == 0
        super().close()
        require(natural, "fixture-required-forced-cleanup-or-exited-with-error")


def transaction(windows: list[OwnedWindow], action: str, source: int = 1, target: int = 2) -> str:
    entries = []
    for window in windows:
        entries.append("{tag=%s,app=%s,pid=%d,stable=%s,address=%s}" % (
            json.dumps(window.tag), json.dumps(window.fixture.app_id), window.pid,
            json.dumps(str(window.stable_id)), json.dumps(window.address),
        ))
    result = eval_lua("return (function() local apply=(function()\n" + TRANSACTION +
                    "\nend)() return apply({" + ",".join(entries) + "}," +
                    json.dumps(action) + f",{source},{target}) end)()")
    require(re.fullmatch(r"[a-z-]{1,64}", result) is not None, "transaction-lua-result-invalid")
    return result


def run(executable: str, instance: str) -> dict:
    fixture = TransactionFixture(executable)
    neighbor = None
    events = None
    try:
        events = FocusEvents(instance)
        source, target, third = fixture.create(), fixture.create(), fixture.create()
        neighbor = TransactionFixture(executable, "ferric-w00-neighbor-proof")
        sentinel = neighbor.create()
        owned = {source.address, target.address, third.address, sentinel.address}
        all_members = [source, target, third]
        focus_owned(source)
        require(transaction(all_members, "create") == "created", "transaction-create-failed")
        focus_owned(sentinel)
        events.drain(owned)
        require(transaction(all_members, "add") == "added", "transaction-add-failed")
        require(transaction(all_members, "add", target=3) == "added", "transaction-third-add-failed")
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, "background-add-changed-focus")

        # An unadmitted existing member must reject before any write, even when
        # every window belongs to this test's process and desktop app ID.
        require(transaction([source, third], "focus") == "reject-membership",
                "unknown-group-member-was-touched")
        stale = replace(target, stable_id=target.stable_id + 1)
        require(transaction([source, stale, third], "add") == "reject-identity",
                "stale-native-handle-was-touched")
        require(transaction([source, source], "focus") == "reject-request",
                "duplicate-native-handle-was-touched")
        require(transaction(all_members, "remove") == "defer-active",
                "hidden-current-removal-was-not-deferred")
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, "rejected-transaction-changed-focus")

        require(transaction(all_members, "focus", source=2) == "focused",
                "exact-group-member-focus-failed")
        count, foreign = events.drain(owned)
        require(count > 0 and not foreign, "explicit-focus-did-not-select-owned-window")
        removed = transaction(all_members, "remove", source=1)
        require(removed == "defer-active", "background-noncurrent-removal-not-deferred-" + removed)
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, "deferred-removal-changed-focus")
        require(transaction(all_members, "focus", source=1) == "focused",
                "removed-member-focus-failed")
        events.drain(owned)
        require(transaction(all_members, "remove", source=1) == "removed",
                "active-first-member-removal-failed")
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, "active-first-removal-changed-focus")
        # Removing the currently focused member, including a group's final
        # member, must retain that native window's focus and identity.
        require(transaction(all_members, "focus", source=2) == "focused",
                "second-member-focus-failed")
        events.drain(owned)
        require(transaction(all_members, "remove", source=2) == "removed",
                "active-member-removal-failed")
        require(transaction(all_members, "focus", source=3) == "focused",
                "remaining-member-focus-failed")
        events.drain(owned)
        require(transaction(all_members, "remove", source=3) == "removed",
                "singleton-removal-failed")
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, "active-removal-changed-focus")

        focus_owned(source)
        require(transaction(all_members, "create") == "created", "active-group-create-failed")
        events.drain(owned)
        require(transaction(all_members, "add") == "defer-active", "active-group-add-not-deferred")
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, "deferred-active-group-add-changed-focus")
        focus_owned(sentinel)
        events.drain(owned)
        require(transaction(all_members, "add") == "added", "inactive-group-add-after-deferral-failed")
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, "inactive-group-add-after-deferral-changed-focus")
        focus_owned(source)
        require(transaction(all_members, "remove") == "removed", "active-group-source-cleanup-failed")
        focus_owned(target)
        require(transaction(all_members, "remove", source=2) == "removed", "active-group-target-cleanup-failed")

        # Creation may wait for an admitted active window, never focus one as
        # a setup side effect while another application is active.
        focus_owned(sentinel)
        events.drain(owned)
        require(transaction(all_members, "create") == "defer-active",
                "background-group-creation-was-not-deferred")
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, "deferred-creation-changed-focus")

        duplicate_key = uuid.uuid4().hex
        fixture.create_raw(duplicate_key, source.tag)
        require(transaction(all_members, "focus") == "reject-identity",
                "ambiguous-mapping-tag-was-touched")
        fixture.destroy_raw(duplicate_key, source.tag, expected_remaining=1)
        focus_owned(source)
        require(transaction(all_members, "create") == "created", "second-create-failed")
        focus_owned(third)
        require(transaction(all_members, "create", source=3) == "created", "separate-group-create-failed")
        focus_owned(sentinel)
        events.drain(owned)
        require(transaction(all_members, "add", target=3) == "reject-target-group",
                "separate-target-group-was-merged")
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, "separate-target-group-rejection-changed-focus")

        focus_owned(source)
        # Lock only after the exact owned source has been independently bound
        # and verified active in this isolated compositor. Never lock a desktop
        # group selected merely by app ID or active-window position.
        lock = eval_lua("return (function() local w=hl.get_active_window() "
                        f"if not w or tostring(w.stable_id)~={json.dumps(str(source.stable_id))} "
                        f"or w.xdg_tag~={json.dumps(source.tag)} or w.pid~={source.pid} "
                        "or not w.group or w.group.size~=1 then return 'reject' end "
                        "hl.dispatch(hl.dsp.group.lock_active({action='set'})) "
                        "return w.group.locked and 'locked' or 'failed' end)()")
        require(lock == "locked", "owned-group-lock-failed")
        focus_owned(sentinel)
        events.drain(owned)
        require(transaction(all_members, "add") == "reject-locked", "locked-group-was-touched")
        require(transaction(all_members, "focus") == "reject-locked", "locked-focus-was-not-declined")
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, "locked-rejection-changed-focus")
        return {"result": "pass", "operations": 27,
                "background_focus_events": 0, "native_identity_preserved": True}
    finally:
        try:
            if events is not None:
                events.close()
        finally:
            try:
                if neighbor is not None:
                    neighbor.close()
            finally:
                fixture.close()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture", required=True)
    args = parser.parse_args()
    try:
        result = run(args.fixture, require_target_compositor())
        print(json.dumps(result, sort_keys=True))
        return 0
    except Exception as error:
        reason = str(error) if isinstance(error, ProofFailure) else type(error).__name__
        print(json.dumps({"result": "failed", "reason": reason}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
