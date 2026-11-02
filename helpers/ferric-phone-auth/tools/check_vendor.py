#!/usr/bin/env python3
"""Verify the small, explicitly recorded patch over the pinned library tree."""

import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path


def hashes(source):
    observed = {}
    for path in source.rglob("*"):
        if path.is_symlink():
            raise SystemExit(f"Unexpected symlink: {path.relative_to(source)}")
        if not path.is_file():
            continue
        name = path.relative_to(source).as_posix()
        observed[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    return observed


def verify(source, manifest, hash_key, patch):
    original = manifest[hash_key]
    modified = set(manifest["modified_files"])
    added = set(manifest["added_files"])
    observed = hashes(source)
    changed = {name for name in original if observed.get(name) != original[name]}
    if set(observed) != set(original) | added or changed != modified:
        raise SystemExit("Vendor tree differs from its documented patch; review UPSTREAM.json")
    # A list of modified filenames cannot detect extra edits within those files.
    # Reversing the recorded patch must recover every original byte and remove
    # every added file. Work on a disposable copy, never on the checkout.
    with tempfile.TemporaryDirectory(prefix="ferric-vendor-check-") as temporary:
        restored = Path(temporary) / "source"
        shutil.copytree(source, restored)
        result = subprocess.run(
            ["git", "apply", "--reverse", "--whitespace=nowarn", str(patch.resolve())],
            cwd=restored, capture_output=True, timeout=10, check=False,
        )
        if result.returncode or hashes(restored) != original:
            raise SystemExit("Vendor contents do not match the recorded patch and upstream hashes")
    print(f"{source.name}: {len(original)} original files, {len(modified)} modified, {len(added)} added")


def main():
    vendor = Path(__file__).resolve().parents[1] / "vendor"
    verify(vendor / "libwebauthn", json.loads((vendor / "UPSTREAM.json").read_text()),
           "original_library_sha256", vendor / "libwebauthn-ferric.patch")
    verify(vendor / "snow", json.loads((vendor / "SNOW_UPSTREAM.json").read_text()),
           "original_sha256", vendor / "snow-ferric.patch")


if __name__ == "__main__":
    main()
