#!/usr/bin/env python3
"""Verify the small, explicitly recorded patch over the pinned library tree."""

import hashlib
import json
from pathlib import Path


def verify(source, manifest, hash_key):
    original = manifest[hash_key]
    modified = set(manifest["modified_files"])
    added = set(manifest["added_files"])
    observed = set()
    changed = set()
    for path in source.rglob("*"):
        if path.is_symlink():
            raise SystemExit(f"Unexpected symlink: {path.relative_to(source)}")
        if not path.is_file():
            continue
        name = path.relative_to(source).as_posix()
        observed.add(name)
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if name in original and digest != original[name]:
            changed.add(name)
    if observed != set(original) | added or changed != modified:
        raise SystemExit("Vendor tree differs from its documented patch; review UPSTREAM.json")
    print(f"{source.name}: {len(original)} original files, {len(modified)} modified, {len(added)} added")


def main():
    vendor = Path(__file__).resolve().parents[1] / "vendor"
    verify(vendor / "libwebauthn", json.loads((vendor / "UPSTREAM.json").read_text()), "original_library_sha256")
    verify(vendor / "snow", json.loads((vendor / "SNOW_UPSTREAM.json").read_text()), "original_sha256")


if __name__ == "__main__":
    main()
