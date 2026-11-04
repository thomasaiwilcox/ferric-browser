#!/usr/bin/env python3
"""Run an Arch qualification command with KDE build/runtime files unavailable.

An unprivileged mount namespace leaves desktop packages and configuration
untouched. Every installed KDE package's executable, library, plugin, header and
build-discovery file is masked, together with explicitly named historical
prefixes. The root filesystem is read-only except declared build/cache paths
and /tmp. Network access is disabled. Record this as namespace isolation, not as
a claim that packages were uninstalled from the host.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
from urllib.parse import urlparse


def fields(path):
    result, key = {}, None
    for line in path.read_text().splitlines():
        if line.startswith("%") and line.endswith("%"):
            key = line.strip("%")
            result[key] = []
        elif line and key:
            result[key].append(line)
    return result


def inventory():
    packages, files = [], set()
    database = Path("/var/lib/pacman/local")
    if not database.is_dir():
        raise RuntimeError("This isolation collector requires Arch package ownership metadata")
    for description in sorted(database.glob("*/desc")):
        metadata = fields(description)
        hostname = urlparse(next(iter(metadata.get("URL", [])), "")).hostname or ""
        if hostname != "kde.org" and not hostname.endswith(".kde.org") and "kf6" not in metadata.get("GROUPS", []):
            continue
        packages.append({"name": metadata["NAME"][0], "version": metadata["VERSION"][0], "url": metadata.get("URL", [])})
        for name in fields(description.with_name("files")).get("FILES", []):
            if name.endswith("/") or not name.startswith(("usr/lib/", "usr/include/", "usr/bin/", "usr/share/cmake", "usr/share/ECM", "usr/share/pkgconfig/")):
                continue
            path = Path("/") / name
            if path.is_file():
                files.add(str(path.resolve()))
    return {"method": "bwrap unprivileged read-only mount namespace; KDE files masked with /dev/null; no network",
            "packages": packages, "masked_files": sorted(files)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inventory", required=True, type=Path)
    parser.add_argument("--writable", action="append", default=[], type=Path)
    parser.add_argument("--hide-prefix", action="append", default=[], type=Path)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("A real build/test/install command is required")
    evidence = inventory()
    evidence["hidden_prefixes"] = [str(p.resolve(strict=True)) for p in args.hide_prefix]
    evidence["command"] = command
    args.inventory.parent.mkdir(parents=True, exist_ok=True)
    args.inventory.write_text(json.dumps(evidence, indent=2) + "\n")
    arguments = ["--die-with-parent", "--unshare-net", "--ro-bind", "/", "/",
                 "--dev-bind", "/dev", "/dev", "--proc", "/proc", "--bind", "/tmp", "/tmp"]
    for writable in args.writable:
        path = str(writable.resolve(strict=True))
        arguments += ["--bind", path, path]
    for path in evidence["masked_files"]:
        arguments += ["--ro-bind", "/dev/null", path]
    for path in evidence["hidden_prefixes"]:
        arguments += ["--tmpfs", path]
    # bwrap's NUL-delimited argument file avoids ARG_MAX for large installed
    # package inventories. No shell interpolation or host filesystem changes.
    with tempfile.TemporaryFile() as stream:
        stream.write(b"\0".join(os.fsencode(arg) for arg in arguments) + b"\0")
        stream.seek(0)
        result = subprocess.run(["bwrap", "--args", str(stream.fileno()), "--", *command], pass_fds=[stream.fileno()])
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
