"""The shared Cargo/CMake recipe. All outputs belong in the build directory."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--scanner", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    manifest = json.loads((root / "provenance.json").read_text())
    args.output.mkdir(parents=True, exist_ok=True)
    for entry in manifest["inputs"]:
        source = root / entry["file"]
        if hashlib.sha256(source.read_bytes()).hexdigest() != entry["sha256"]:
            raise ValueError(f"Protocol input checksum mismatch: {source}")
        name = source.stem
        for mode, suffix in [("client-header", "client.h"),
                             ("server-header", "server.h"),
                             ("private-code", "protocol.c")]:
            target = args.output / f"{name}-{suffix}"
            # Use actual filenames: scanners also use these for generated
            # comments/guards. Stage outputs so unchanged builds stay clean.
            with tempfile.TemporaryDirectory(dir=args.output) as staging:
                generated = Path(staging) / target.name
                subprocess.run([args.scanner, mode, str(source), str(generated)], check=True)
                if not target.exists() or target.read_bytes() != generated.read_bytes():
                    generated.replace(target)


if __name__ == "__main__":
    main()
