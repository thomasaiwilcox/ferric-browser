#!/usr/bin/env python3
"""Check helper source pins and the release-packaged official suffix data."""

import argparse
import datetime
import hashlib
import json
import tomllib
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", action="store_true", help="also require recent suffix data")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    lock = tomllib.loads((root / "Cargo.lock").read_text())
    allowed_git = {
        "git+https://github.com/linux-credentials/libwebauthn?rev=9c0e5f78ac445d35dca05170efbb0afefc5a89ae#9c0e5f78ac445d35dca05170efbb0afefc5a89ae",
        "git+https://github.com/Nitrokey/fido-authenticator.git?tag=v0.1.1-nitrokey.27#5ebb4a48302e4c80a2abe1c2d86201a3df2a1d2d",
        "git+https://github.com/trussed-dev/trussed.git?rev=024e0eca5fb7dbd2457831f7c7bffe4341e08775#024e0eca5fb7dbd2457831f7c7bffe4341e08775",
        "git+https://github.com/trussed-dev/trussed-staging.git?rev=7922d67e9637a87e5625aaff9e5111f0d4ec0346#7922d67e9637a87e5625aaff9e5111f0d4ec0346",
    }
    sources = {package.get("source", "") for package in lock["package"]}
    if any(source.startswith("git+") and source not in allowed_git for source in sources):
        raise SystemExit("Unreviewed Git commit in helper lockfile")
    data = (root / "data/public_suffix_list.dat").read_bytes()
    provenance = json.loads((root / "data/provenance.json").read_text())
    if provenance["source"] != "https://publicsuffix.org/list/public_suffix_list.dat":
        raise SystemExit("Suffix source must be the official list")
    if hashlib.sha256(data).hexdigest() != provenance["sha256"]:
        raise SystemExit("Suffix data hash differs from provenance")
    text = data.decode("utf-8")
    if f'// VERSION: {provenance["version"]}' not in text or f'// COMMIT: {provenance["upstream_commit"]}' not in text:
        raise SystemExit("Suffix data version differs from provenance")
    if args.release:
        published = datetime.date.fromisoformat(provenance["version"][:10])
        age = (datetime.datetime.now(datetime.timezone.utc).date() - published).days
        if not 0 <= age <= 45:
            raise SystemExit("Refresh official suffix data and provenance before release")
    print("Helper Git pins and official suffix-data provenance passed")


if __name__ == "__main__":
    main()
