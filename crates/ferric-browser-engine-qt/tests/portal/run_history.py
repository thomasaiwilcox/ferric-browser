"""Retain actual download history across two owned browser processes."""

from pathlib import Path
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
import uuid

HERE = Path(__file__).resolve().parent


def history(profile):
    required = {"id", "source_url", "destination", "state", "bytes_received", "created_at", "completed_at"}
    matches = []
    for path in profile.rglob("*"):
        if path.is_symlink() or not path.is_file():
            continue
        with path.open("rb") as stream:
            if stream.read(16) != b"SQLite format 3\0":
                continue
        connection = sqlite3.connect(path.as_uri() + "?mode=ro", uri=True)
        try:
            columns = {row[1] for row in connection.execute("PRAGMA table_info(downloads)")}
            if not required.issubset(columns):
                continue
            rows = connection.execute(
                "SELECT id,source_url,destination,state,bytes_received,created_at,completed_at FROM downloads ORDER BY id"
            )
            matches.append([dict(zip(
                ["id", "source_url", "destination", "state", "bytes_received", "created_at", "completed_at"], row
            )) for row in rows])
        finally:
            connection.close()
    if len(matches) != 1:
        raise RuntimeError("Expected one owned browser download-history database")
    return matches[0]


def main():
    build = Path(sys.argv[1]).resolve()
    bus_runner = sys.argv[2]
    with tempfile.TemporaryDirectory(prefix="ferric-download-history-") as temporary:
        profile = Path(temporary) / "profile"
        profile.mkdir()
        (profile / ".ferric-owned-fixture").write_text("Owned download-history test only.\n")
        env = os.environ.copy()
        env.update(FERRIC_PORTAL_TEST_BUS="isolated", FERRIC_DOWNLOAD_HISTORY_PROFILE=str(profile))
        command = [
            bus_runner, "--config-file=" + str(HERE.parent / "application/notification-test-bus.conf"),
            "--", sys.executable, str(HERE / "run.py"), str(build), "browser", "downloadCanRetry",
        ]
        token = uuid.uuid4().hex
        tags = ["first-" + token, "second-" + token]
        snapshots = []
        for tag in tags:
            env["FERRIC_DOWNLOAD_HISTORY_TAG"] = tag
            result = subprocess.run(command, env=env, timeout=175)
            if result.returncode:
                raise RuntimeError("Owned browser journey failed before history could be compared")
            snapshots.append(history(profile))
        before, after = snapshots
        output = env.get("FERRIC_PORTAL_TEST_ARTIFACTS")
        if output:
            destination = Path(output)
            destination.mkdir(parents=True, exist_ok=True)
            (destination / "download-history.json").write_text(json.dumps(
                {"after_first": before, "after_second": after}, indent=2
            ) + "\n")
        if len(before) != 2 or any(tags[0] not in row["source_url"] for row in before):
            raise AssertionError("First browser must create two tagged download records")
        old = {row["id"]: row for row in before}
        retained = {row["id"]: row for row in after}
        if len(after) != 4 or any(retained.get(key) != row for key, row in old.items()):
            raise AssertionError("A later browser process replaced or lost existing download history")
        added = [row for row in after if row["id"] not in old]
        if len(added) != 2 or any(tags[1] not in row["source_url"] for row in added):
            raise AssertionError("Second browser must have distinct download identities")
        for row in after:
            if row["state"] == "completed":
                if Path(row["destination"]).read_bytes() != b"fixture!":
                    raise AssertionError("A retained completed download lost its saved payload")
        print("Download history retained four records and both saved files across two browser processes", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
