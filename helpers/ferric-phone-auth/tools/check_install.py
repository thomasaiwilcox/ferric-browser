#!/usr/bin/env python3
"""Exercise the real one-shot binary in a disposable installed directory layout.

The empty network policy forbids radio/channel/relay work. Successful admission
therefore proves package lookup and framing without starting an authenticator.
"""

import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--helper", required=True, type=Path)
    args = parser.parse_args()
    source = Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(prefix="ferric-phone-install-") as temporary:
        prefix = Path(temporary)
        binary = prefix / "bin/ferric-phone-auth-helper"
        data = prefix / "share/ferric-browser/public_suffix_list.dat"
        binary.parent.mkdir(parents=True)
        data.parent.mkdir(parents=True)
        shutil.copy2(args.helper.resolve(strict=True), binary)
        shutil.copy2(source / "data/public_suffix_list.dat", data)
        request = {
            "v": 1, "id": "disposable-install-probe", "op": "get",
            "origin": "https://login.example.com",
            "options": {"rpId": "example.com", "challenge": "AQIDBAUGBwgJCgsMDQ4PEA"},
            "network": {},
        }
        result = subprocess.run([str(binary)], input=json.dumps(request).encode() + b"\n",
                                capture_output=True, timeout=10, check=True)
        if result.stderr or len(result.stdout.splitlines()) != 1:
            raise SystemExit("Unexpected diagnostic or framing from installed helper")
        response = json.loads(result.stdout)
        if (response.get("id") != request["id"] or response.get("kind") != "error"
                or response.get("body", {}).get("code") != "transport_unavailable"):
            raise SystemExit("Installed helper did not admit the request with networking disabled")
    print("Disposable installed helper: suffix lookup, admission and disabled transport passed")


if __name__ == "__main__":
    main()
