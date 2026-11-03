#!/usr/bin/env python3
"""Owned-window Hyprland proof runner. It never records unrelated client data."""

from __future__ import annotations

import argparse
import json
import os
import re
import select
import socket
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass


EXPECTED_VERSION = "0.56.2"
EXPECTED_COMMIT = "efb50993780079460b0cbed1363e2166a2de1d9f"
APP_ID = "ferric-w00-proof"
TITLE = "Ferric W00 test-owned window"
EXPECTED_INSTANCE = ""


class ProofFailure(RuntimeError):
    pass


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise ProofFailure(reason)


def hyprctl(*args: str, json_output: bool = False) -> str:
    if EXPECTED_INSTANCE:
        require(os.environ.get("HYPRLAND_INSTANCE_SIGNATURE") == EXPECTED_INSTANCE,
                "compositor-instance-epoch-changed")
    command = ["hyprctl"]
    if json_output:
        command.append("-j")
    command.extend(args)
    completed = subprocess.run(command, capture_output=True, text=True, timeout=8)
    if completed.returncode != 0:
        raise ProofFailure("hyprctl-command-failed")
    output = completed.stdout.strip()
    if output.lower().startswith("error:"):
        raise ProofFailure("hyprctl-reported-error")
    return output


def require_target_compositor(expected_version: str = EXPECTED_VERSION) -> str:
    global EXPECTED_INSTANCE
    instance = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE", "")
    require(bool(instance) and re.fullmatch(r"[A-Za-z0-9_-]+", instance) is not None,
            "missing-or-invalid-compositor-instance")
    version = hyprctl("version")
    require(expected_version in version and EXPECTED_COMMIT in version,
            "unsupported-compositor-build")
    EXPECTED_INSTANCE = instance
    return instance


def client_tag(client: dict) -> str:
    for name in ("xdgTag", "xdg_tag", "xdg-toplevel-tag"):
        value = client.get(name)
        if isinstance(value, str):
            return value
    return ""


def client_address(client: dict) -> str:
    value = client.get("address")
    return value if isinstance(value, str) else ""


def client_pid(client: dict) -> int:
    try:
        return int(client.get("pid", -1))
    except (TypeError, ValueError):
        return -1


def client_app_id(client: dict) -> str:
    for name in ("class", "initialClass", "initial_class"):
        value = client.get(name)
        if isinstance(value, str) and value:
            return value
    return ""


def clients() -> list[dict]:
    try:
        result = json.loads(hyprctl("clients", json_output=True))
    except (json.JSONDecodeError, TypeError) as error:
        raise ProofFailure("invalid-client-snapshot") from error
    require(isinstance(result, list), "invalid-client-snapshot")
    return [item for item in result if isinstance(item, dict)]


def lua_string(value: str) -> str:
    # Only locally generated markers and validated scalar identities enter Lua.
    require(re.fullmatch(r"[A-Za-z0-9_-]+", value) is not None,
            "invalid-lua-scalar")
    return "[[" + value + "]]"


def eval_lua(expression: str) -> str:
    # `repl` returns expression values; `eval` reports only command success.
    output = hyprctl("repl", expression)
    require(output and not output.lower().startswith("error"), "lua-eval-unavailable-or-failed")
    return output.splitlines()[-1].strip()


def wait_for_eval(expression: str, expected: str, timeout: float = 3) -> str:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = eval_lua(expression)
        if value == expected:
            return value
        time.sleep(0.05)
    raise ProofFailure("compositor-state-transition-timeout")


def lua_resolve(tag: str) -> tuple[int, str, int]:
    code = (
        "return (function() local found={} "
        f"for _,w in ipairs(hl.get_windows()) do if w.xdg_tag=={lua_string(tag)} then "
        "found[#found+1]=w end end "
        "if #found~=1 then return 'count:'..tostring(#found) end "
        "local w=found[1] "
        "return 'ok|'..tostring(w.stable_id)..'|'..tostring(w.address)..'|'..tostring(w.pid) "
        "end)()"
    )
    result = eval_lua(code)
    require(result.startswith("ok|"), "native-marker-absent-or-ambiguous")
    fields = result.split("|", 3)
    require(len(fields) == 4, "native-identity-response-malformed")
    try:
        stable_id = int(fields[1])
        pid = int(fields[3])
    except ValueError as error:
        raise ProofFailure("native-identity-response-malformed") from error
    address = fields[2]
    require(re.fullmatch(r"0x[0-9a-fA-F]+", address) is not None and stable_id > 0 and pid > 0,
            "native-identity-response-malformed")
    return stable_id, address, pid


@dataclass
class OwnedWindow:
    fixture: "FixtureProcess"
    key: str
    tag: str
    stable_id: int
    address: str
    pid: int


class FixtureProcess:
    def __init__(self, executable: str, app_id: str = APP_ID) -> None:
        environment = os.environ.copy()
        environment["QT_QPA_PLATFORM"] = "wayland"
        require(app_id in (APP_ID, "ferric-w00-neighbor-proof"), "invalid-fixture-app")
        self.app_id = app_id
        environment["FERRIC_W00_APP_ID"] = app_id
        self.process = subprocess.Popen(
            [executable], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, bufsize=0, env=environment,
        )
        self.pid = self.process.pid
        self.windows: dict[str, OwnedWindow] = {}
        self.tags: set[str] = set()
        self.output_buffer = b""
        try:
            ready = self._read_event(10)
            require(ready.get("event") == "ready" and ready.get("platform", "").startswith("wayland"),
                    "fixture-did-not-start-on-wayland")
            require(int(ready.get("pid", -1)) == self.pid, "fixture-pid-mismatch")
        except BaseException:
            self.close()
            raise

    def _read_event(self, timeout: float = 5) -> dict:
        require(self.process.stdout is not None, "fixture-output-unavailable")
        deadline = time.monotonic() + timeout
        while b"\n" not in self.output_buffer:
            remaining = deadline - time.monotonic()
            require(remaining > 0, "fixture-command-timeout")
            ready, _, _ = select.select([self.process.stdout], [], [], remaining)
            require(bool(ready), "fixture-command-timeout")
            chunk = os.read(self.process.stdout.fileno(), 4096)
            require(bool(chunk), "fixture-output-closed")
            self.output_buffer += chunk
            require(len(self.output_buffer) <= 65536, "fixture-output-too-large")
        line, self.output_buffer = self.output_buffer.split(b"\n", 1)
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise ProofFailure("fixture-protocol-invalid") from error
        require(isinstance(event, dict), "fixture-protocol-invalid")
        if event.get("event") == "error":
            raise ProofFailure("fixture-command-rejected")
        return event

    def _send(self, message: dict) -> None:
        require(self.process.stdin is not None, "fixture-input-unavailable")
        self.process.stdin.write((json.dumps(message, separators=(",", ":")) + "\n").encode())
        self.process.stdin.flush()

    def create(self, *, tag: str | None = None, delay_ms: int = 0) -> OwnedWindow:
        key = uuid.uuid4().hex
        marker = tag or str(uuid.uuid4())
        self.create_raw(key, marker, delay_ms)
        return self.bind(key, marker)

    def bind(self, key: str, marker: str) -> OwnedWindow:
        selected = wait_for_tag(marker, self.pid, self.app_id)
        stable_id, address, resolved_pid = lua_resolve(marker)
        require(resolved_pid == self.pid and selected is not None,
                "native-marker-did-not-bind-to-owned-process")
        require(client_address(selected) == address, "lua-and-ipc-identity-disagree")
        require(client_pid(selected) == self.pid and client_app_id(selected) == self.app_id,
                "native-marker-corroboration-mismatch")
        require(selected.get("title") == TITLE and selected.get("initialTitle") == TITLE,
                "bootstrap-marker-leaked-into-title")
        result = OwnedWindow(self, key, marker, stable_id, address, self.pid)
        self.windows[key] = result
        return result

    def create_concurrently(self) -> tuple[OwnedWindow, OwnedWindow]:
        delayed_key, fast_key = uuid.uuid4().hex, uuid.uuid4().hex
        delayed_tag, fast_tag = str(uuid.uuid4()), str(uuid.uuid4())
        for key, tag, delay in ((delayed_key, delayed_tag, 400), (fast_key, fast_tag, 0)):
            self.windows[key] = OwnedWindow(self, key, tag, -1, "", self.pid)
            self.tags.add(tag)
            self._send({"op": "create", "key": key, "tag": tag, "delay_ms": delay})
        first, second = self._read_event(), self._read_event()
        require(first.get("event") == second.get("event") == "shown" and
                first.get("key") == fast_key and second.get("key") == delayed_key,
                "delayed-concurrent-mapping-order-not-observed")
        return self.bind(delayed_key, delayed_tag), self.bind(fast_key, fast_tag)

    def create_raw(self, key: str, tag: str, delay_ms: int = 0) -> None:
        self._send({"op": "create", "key": key, "tag": tag, "delay_ms": delay_ms})
        event = self._read_event(max(5, delay_ms / 1000 + 2))
        require(event.get("event") == "shown" and event.get("key") == key,
                "fixture-did-not-show-window")
        self.windows[key] = OwnedWindow(self, key, tag, -1, "", self.pid)
        self.tags.add(tag)

    def destroy_raw(self, key: str, tag: str, expected_remaining: int = 0) -> None:
        self._send({"op": "destroy", "key": key})
        event = self._read_event()
        require(event.get("event") == "destroyed" and event.get("key") == key,
                "fixture-did-not-destroy-window")
        wait_for_tag_count(tag, expected_remaining)
        self.windows.pop(key, None)
        if expected_remaining == 0:
            self.tags.discard(tag)

    def destroy(self, window: OwnedWindow) -> None:
        require(window.fixture is self and window.pid == self.pid, "foreign-fixture-destroy")
        self._send({"op": "destroy", "key": window.key})
        event = self._read_event()
        require(event.get("event") == "destroyed" and event.get("key") == window.key,
                "fixture-did-not-destroy-window")
        wait_for_absent_tag(window.tag)
        self.windows.pop(window.key, None)
        self.tags.discard(window.tag)

    def close(self) -> None:
        if self.process.poll() is None:
            try:
                self._send({"op": "exit"})
                self.process.wait(timeout=4)
            except (BrokenPipeError, subprocess.TimeoutExpired):
                self.process.terminate()
                try:
                    self.process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=2)
        require(self.process.poll() is not None, "owned-fixture-process-not-reaped")
        for tag in tuple(self.tags):
            wait_for_absent_tag(tag)
        self.tags.clear()


def wait_for_tag(tag: str, expected_pid: int, app_id: str, timeout: float = 12) -> dict:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        matches = [item for item in clients() if client_tag(item) == tag]
        if matches:
            require(len(matches) == 1, "native-marker-is-ambiguous")
            require(client_pid(matches[0]) == expected_pid and client_app_id(matches[0]) == app_id,
                    "native-marker-matched-foreign-window")
            require(client_address(matches[0]).startswith("0x"), "native-address-missing")
            return matches[0]
        time.sleep(0.05)
    raise ProofFailure("native-marker-not-published")


def wait_for_absent_tag(tag: str, timeout: float = 8) -> None:
    wait_for_tag_count(tag, 0, timeout)


def wait_for_tag_count(tag: str, expected: int, timeout: float = 8) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if sum(client_tag(item) == tag for item in clients()) == expected:
            return
        time.sleep(0.05)
    raise ProofFailure("native-marker-count-did-not-settle")


class FocusEvents:
    """Observe activewindowv2 without persisting any unrelated event payload."""

    def __init__(self, instance: str) -> None:
        runtime = os.environ.get("XDG_RUNTIME_DIR", "")
        require(bool(runtime), "runtime-directory-unavailable")
        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.socket.settimeout(3)
        self.socket.connect(os.path.join(runtime, "hypr", instance, ".socket2.sock"))
        self.socket.settimeout(0.0)
        self.buffer = b""

    def drain(self, owned_addresses: set[str], quiet_seconds: float = 0.25) -> tuple[int, bool]:
        count = 0
        foreign = False
        deadline = time.monotonic() + quiet_seconds
        absolute_deadline = time.monotonic() + 4
        while True:
            require(time.monotonic() < absolute_deadline, "event-stream-did-not-quiesce")
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                break
            ready, _, _ = select.select([self.socket], [], [], remaining)
            if not ready:
                break
            chunk = self.socket.recv(4096)
            if not chunk:
                raise ProofFailure("event-socket-disconnected")
            self.buffer += chunk
            require(len(self.buffer) <= 65536, "event-stream-line-too-large")
            while b"\n" in self.buffer:
                line, self.buffer = self.buffer.split(b"\n", 1)
                if not line.startswith(b"activewindowv2>>"):
                    continue
                address = line.partition(b">>")[2].decode("ascii", errors="ignore")
                require(not address or re.fullmatch(r"(?:0x)?[0-9a-fA-F]+", address) is not None,
                        "invalid-focus-event-address")
                if address and not address.startswith("0x"):
                    address = "0x" + address
                count += 1
                if address and address not in owned_addresses:
                    foreign = True
                # Only focus events extend the focus-settling interval. A live
                # desktop may continually emit unrelated compositor events.
                deadline = time.monotonic() + quiet_seconds
        return count, foreign

    def close(self) -> None:
        self.socket.close()


def focus_owned(window: OwnedWindow) -> None:
    # Resolve and validate the owned fixture inside the same Lua operation
    # that dispatches focus; never use active-window or direction discovery.
    result = eval_lua(
        "return (function() local found={} for _,w in ipairs(hl.get_windows()) do "
        f"if w.xdg_tag=={lua_string(window.tag)} then found[#found+1]=w end end "
        "if #found~=1 then return 'reject-count' end local w=found[1] "
        f"if w.stable_id~={window.stable_id} or tostring(w.address)~={lua_string(window.address)} or "
        f"w.pid~={window.pid} or (w.class~={lua_string(window.fixture.app_id)} and w.initial_class~={lua_string(window.fixture.app_id)}) "
        "then return 'reject-identity' end "
        "local result=hl.dispatch(hl.dsp.focus({window=w})) "
        "return result and result.ok and 'requested' or 'rejected' end)()"
    )
    require(result == "requested", "failed-to-focus-owned-fixture-window")
    deadline = time.monotonic() + 4
    while time.monotonic() < deadline:
        result = eval_lua(
            "return (function() local w=hl.get_active_window(); "
            f"return w and w.stable_id=={window.stable_id} and "
            f"tostring(w.address)=={lua_string(window.address)} and w.pid=={window.pid} "
            "and 'focused' or 'not-focused' end)()"
        )
        if result == "focused":
            return
        time.sleep(0.05)
    raise ProofFailure("owned-focus-precondition-not-met")


def run_t100(executable: str) -> dict:
    fixtures: list[FixtureProcess] = []
    try:
        first = FixtureProcess(executable)
        fixtures.append(first)
        second = FixtureProcess(executable)
        fixtures.append(second)
        delayed, local = first.create_concurrently()
        peer = second.create()
        require(len({delayed.tag, peer.tag, local.tag}) == 3, "fresh-marker-collision")
        require(len({delayed.stable_id, peer.stable_id, local.stable_id}) == 3,
                "distinct-markers-share-stable-identity")
        require(delayed.pid == local.pid and delayed.pid != peer.pid,
                "fixture-process-identity-precondition-failed")

        # An intentional duplicate marker must be ambiguous and resolve to no
        # actionable identity while both owned surfaces exist.
        duplicate_key = uuid.uuid4().hex
        first.create_raw(duplicate_key, local.tag)
        wait_for_tag_count(local.tag, 2)
        ambiguous = eval_lua(
            "return (function() local n=0 for _,w in ipairs(hl.get_windows()) do "
            f"if w.xdg_tag=={lua_string(local.tag)} then n=n+1 end end "
            "return 'count:'..tostring(n) end)()"
        )
        require(ambiguous == "count:2", "duplicate-marker-not-rejected")
        first.destroy_raw(duplicate_key, local.tag, expected_remaining=1)

        # Simulate stale marker reuse after destruction. Old stable identity
        # must not select a replacement, including an actually reused address.
        old_stable, old_address, old_tag = local.stable_id, local.address, local.tag
        first.destroy(local)
        address_reuse_observed = False
        for _ in range(64):
            replacement = first.create(tag=old_tag)
            require(replacement.stable_id != old_stable, "stale-stable-id-reused")
            stale_check = eval_lua(
                "return (function() local found={} "
                f"for _,w in ipairs(hl.get_windows()) do if w.xdg_tag=={lua_string(old_tag)} then "
                "found[#found+1]=w end end "
                "if #found~=1 then return 'reject-count' end "
                f"if found[1].stable_id~={old_stable} then return 'reject-stable-id' end "
                f"if tostring(found[1].address)~={lua_string(old_address)} then return 'reject-address' end "
                "return 'unsafe-match' end)()"
            )
            require(stale_check.startswith("reject-"), "stale-identity-selected-replacement")
            address_reuse_observed = replacement.address == old_address
            if address_reuse_observed:
                break
            first.destroy(replacement)
        require(address_reuse_observed, "address-reuse-not-observed")
        return {"T100": "pass", "duplicate_marker_rejected": True,
                "stale_identity_rejected": True, "address_reuse_observed": True}
    finally:
        cleanup_error: Exception | None = None
        for fixture in reversed(fixtures):
            try:
                fixture.close()
            except Exception as error:
                cleanup_error = error
        if cleanup_error is not None:
            raise cleanup_error


def guarded_add_code(source: OwnedWindow, target: OwnedWindow) -> str:
    return (
        "return (function() "
        "local function resolve(token, sid, address, pid) "
        "local found={} for _,w in ipairs(hl.get_windows()) do "
        "if w.xdg_tag==token then found[#found+1]=w end end "
        "if #found~=1 then return nil end local w=found[1] "
        "if w.stable_id~=sid or tostring(w.address)~=address or w.pid~=pid or "
        f"(w.class~={lua_string(APP_ID)} and w.initial_class~={lua_string(APP_ID)}) then return nil end "
        "return w end "
        f"local source=resolve({lua_string(source.tag)},{source.stable_id},{lua_string(source.address)},{source.pid}) "
        f"local target=resolve({lua_string(target.tag)},{target.stable_id},{lua_string(target.address)},{target.pid}) "
        "if not source or not target then return 'reject-identity' end "
        "local group=source.group "
        "if not group or target.group or group.denied or group.locked then return 'reject-state' end "
        # This proof only adds to its registered singleton source group. Refuse
        # pre-existing or mixed membership rather than touching another group.
        "if group.size~=1 or not group.current or group.current.stable_id~=source.stable_id "
        "then return 'reject-membership' end "
        "for _,member in ipairs(group.members) do if member.stable_id~=source.stable_id "
        "or member.pid~=source.pid then return 'reject-membership' end end "
        "local beforeCurrent=group.current local beforeIndex=group.current_index "
        "local beforeMembers={} for _,w in ipairs(group.members) do beforeMembers[#beforeMembers+1]=w.stable_id end "
        "group:add(target) "
        "if beforeCurrent then hl.dispatch(hl.dsp.group.active({window=source,index=beforeIndex})) end "
        "local members={} for _,w in ipairs(group.members) do members[#members+1]=w.stable_id end "
        "table.sort(members) table.sort(beforeMembers) "
        "return 'added|'..tostring(#members)..'|'..tostring(group.current and group.current.stable_id or -1) "
        "end)()"
    )


def run_t101(executable: str, instance: str) -> dict:
    fixture = FixtureProcess(executable)
    neighbor: FixtureProcess | None = None
    events: FocusEvents | None = None
    try:
        events = FocusEvents(instance)
        source = fixture.create()
        target = fixture.create(delay_ms=250)
        neighbor = FixtureProcess(executable, "ferric-w00-neighbor-proof")
        sentinel = neighbor.create()
        # Create the exact source group on its owned source window, then focus a
        # separate owned sentinel so the placement itself is background work.
        result = eval_lua(
            "return (function() local found={} "
            f"for _,w in ipairs(hl.get_windows()) do if w.xdg_tag=={lua_string(source.tag)} then "
            "found[#found+1]=w end end if #found~=1 then return 'reject-source' end "
            "local s=found[1] "
            f"if s.stable_id~={source.stable_id} or tostring(s.address)~={lua_string(source.address)} or "
            f"s.pid~={source.pid} or (s.class~={lua_string(APP_ID)} and s.initial_class~={lua_string(APP_ID)}) "
            "then return 'reject-identity' end "
            "if s.group then return 'reject-already-grouped' end "
            "hl.dispatch(hl.dsp.group.toggle({window=s})) return 'group-requested' end)()"
        )
        require(result == "group-requested", "source-group-setup-failed")
        wait_for_eval(
            "return (function() local s=nil for _,w in ipairs(hl.get_windows()) do "
            f"if w.xdg_tag=={lua_string(source.tag)} then s=w end end "
            "return s and s.group and tostring(s.group.size) or 'none' end)()",
            "1",
        )
        focus_owned(sentinel)
        _, pre_foreign = events.drain({source.address, target.address, sentinel.address})
        require(not pre_foreign, "foreign-focus-event-during-precondition")
        before = eval_lua(
            "return (function() local s=nil for _,w in ipairs(hl.get_windows()) do "
            f"if w.xdg_tag=={lua_string(source.tag)} then s=w end end "
            "if not s or not s.group then return 'missing-group' end "
            "return tostring(s.group.current.stable_id)..'|'..tostring(s.group.current_index)..'|'.."
            "tostring(s.group.size) end)()"
        )
        fields = before.split("|")
        require(len(fields) == 3, "source-group-state-unavailable")
        before_current = int(fields[0])
        require(before_current == source.stable_id and int(fields[2]) == 1,
                "source-group-precondition-invalid")

        result = eval_lua(guarded_add_code(source, target))
        require(result.startswith("added|2|"), "guarded-background-add-failed")
        after_expression = (
            "return (function() local s=nil local t=nil "
            "for _,w in ipairs(hl.get_windows()) do "
            f"if w.xdg_tag=={lua_string(source.tag)} then s=w end "
            f"if w.xdg_tag=={lua_string(target.tag)} then t=w end end "
            "if not s or not t or not s.group or s.group~=t.group then return 'membership-error' end "
            "local ids={} for _,w in ipairs(s.group.members) do ids[#ids+1]=w.stable_id end "
            "table.sort(ids) "
            "return tostring(#ids)..'|'..tostring(s.group.current.stable_id)..'|'.."
            "tostring(hl.get_active_window().stable_id)..'|'..tostring(ids[1])..'|'..tostring(ids[2]) end)()"
        )
        expected_after = (
            f"2|{before_current}|{sentinel.stable_id}|"
            f"{min(source.stable_id, target.stable_id)}|{max(source.stable_id, target.stable_id)}"
        )
        wait_for_eval(after_expression, expected_after)
        event_count, foreign = events.drain({source.address, target.address, sentinel.address})
        require(event_count == 0 and not foreign, "group-operation-emitted-focus-event")
        require(lua_resolve(sentinel.tag)[0] == sentinel.stable_id,
                "foreign-fixture-neighbor-identity-changed")
        return {"T101": "pass", "group_size": 2, "focus_events": event_count,
                "active_window_preserved": True, "current_member_preserved": True}
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


def run_t102(executable: str, instance: str) -> dict:
    fixture = FixtureProcess(executable)
    events: FocusEvents | None = None
    try:
        events = FocusEvents(instance)
        source = fixture.create()
        target = fixture.create()
        sentinel = fixture.create()
        setup = eval_lua(
            "return (function() local found={} for _,w in ipairs(hl.get_windows()) do "
            f"if w.xdg_tag=={lua_string(source.tag)} then found[#found+1]=w end end "
            "if #found~=1 then return 'reject-count' end local s=found[1] "
            f"if s.stable_id~={source.stable_id} or tostring(s.address)~={lua_string(source.address)} or "
            f"s.pid~={source.pid} or (s.class~={lua_string(APP_ID)} and s.initial_class~={lua_string(APP_ID)}) "
            "then return 'reject-identity' end "
            "if s.group then return 'reject-already-grouped' end "
            "hl.dispatch(hl.dsp.group.toggle({window=s})) "
            "return 'ok' end)()"
        )
        require(setup == "ok", "negative-case-group-setup-failed")
        wait_for_eval(
            "return (function() local s=nil for _,w in ipairs(hl.get_windows()) do "
            f"if w.xdg_tag=={lua_string(source.tag)} then s=w end end "
            "return s and s.group and tostring(s.group.size) or 'none' end)()",
            "1",
        )
        focus_owned(source)
        lock_command = (
            "return (function() local found={} for _,w in ipairs(hl.get_windows()) do "
            f"if w.xdg_tag=={lua_string(source.tag)} then found[#found+1]=w end end "
            "if #found~=1 then return 'reject-count' end local s=found[1] "
            "if not s.group then return 'missing' end "
            f"if s.stable_id~={source.stable_id} or tostring(s.address)~={lua_string(source.address)} or "
            f"s.pid~={source.pid} or (s.class~={lua_string(APP_ID)} and s.initial_class~={lua_string(APP_ID)}) "
            "then return 'reject-identity' end "
            "local active=hl.get_active_window() "
            "if not active or active.stable_id~=s.stable_id or s.group.size~=1 "
            "then return 'reject-active-or-membership' end "
            "for _,member in ipairs(s.group.members) do if member.stable_id~=s.stable_id "
            "then return 'reject-membership' end end "
            "hl.dispatch(hl.dsp.group.lock_active({action='set'})) "
            "return 'requested' end)()"
        )
        require(eval_lua(lock_command) == "requested", "owned-group-lock-unavailable")
        locked = wait_for_eval(
            "return (function() local s=nil for _,w in ipairs(hl.get_windows()) do "
            f"if w.xdg_tag=={lua_string(source.tag)} then s=w end end "
            "return s and s.group and tostring(s.group.locked) or 'missing' end)()",
            "true",
        )
        focus_owned(sentinel)
        _, foreign_setup = events.drain({source.address, target.address, sentinel.address})
        require(not foreign_setup, "foreign-focus-event-during-negative-setup")
        before = eval_lua(
            "return (function() local s=nil local t=nil for _,w in ipairs(hl.get_windows()) do "
            f"if w.xdg_tag=={lua_string(source.tag)} then s=w end "
            f"if w.xdg_tag=={lua_string(target.tag)} then t=w end end "
            "return tostring(s.group.size)..'|'..tostring(t.group==nil)..'|'.."
            "tostring(s.group.current.stable_id)..'|'..tostring(hl.get_active_window().stable_id) end)()"
        )
        require(before == f"1|true|{source.stable_id}|{sentinel.stable_id}",
                "locked-group-precondition-invalid")
        # Exercise the actual installed-build admission gate with an
        # incompatible requirement; query the real compositor, without
        # dispatching any operation or changing its advertised capabilities.
        try:
            require_target_compositor(expected_version="ferric-intentionally-unsupported-build")
        except ProofFailure as error:
            require(str(error) == "unsupported-compositor-build",
                    "unexpected-unsupported-build-failure")
        else:
            raise ProofFailure("unsupported-build-not-rejected")
        rejected = eval_lua(guarded_add_code(source, target))
        require(rejected == "reject-state", "locked-add-was-not-rejected")
        event_count, foreign = events.drain({source.address, target.address, sentinel.address})
        require(event_count == 0 and not foreign, "rejected-operation-changed-focus")
        after = eval_lua(
            "return (function() local s=nil local t=nil for _,w in ipairs(hl.get_windows()) do "
            f"if w.xdg_tag=={lua_string(source.tag)} then s=w end "
            f"if w.xdg_tag=={lua_string(target.tag)} then t=w end end "
            "return tostring(s.group.size)..'|'..tostring(t.group==nil)..'|'.."
            "tostring(s.group.current.stable_id)..'|'..tostring(hl.get_active_window().stable_id) end)()"
        )
        require(after == before, "locked-rejection-mutated-owned-state")

        # Destroy only our locked singleton; no unlock dispatcher or global
        # group state change is necessary for the missing-source proof.
        old = source
        fixture.destroy(source)
        stale = eval_lua(guarded_add_code(old, target))
        require(stale == "reject-identity", "missing-source-was-not-rejected")
        event_count, foreign = events.drain({old.address, target.address, sentinel.address})
        require(event_count == 0 and not foreign, "missing-source-rejection-changed-focus")

        return {"T102": "pass", "locked_add_rejected": True,
                "missing_source_rejected": True, "incompatible_build_requirement_rejected": True,
                "focus_events": event_count}
    finally:
        try:
            if events is not None:
                events.close()
        finally:
            fixture.close()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--case", required=True, choices=("T100", "T101", "T102"))
    args = parser.parse_args()
    try:
        instance = require_target_compositor()
        if args.case == "T100":
            result = run_t100(args.fixture)
        elif args.case == "T101":
            result = run_t101(args.fixture, instance)
        else:
            result = run_t102(args.fixture, instance)
        print(json.dumps(result, sort_keys=True))
        return 0
    except Exception as error:
        # Exceptions contain stable reason codes only; no client snapshots,
        # addresses, titles, user window names, or raw Hyprland output.
        reason = str(error) if isinstance(error, ProofFailure) else type(error).__name__
        print(json.dumps({"case": args.case, "result": "blocked_or_failed", "reason": reason},
                         sort_keys=True), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
