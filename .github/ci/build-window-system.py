"""Build the qualified public window export dependency into an owned prefix."""

import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile
import tomllib
import urllib.request


def run(command, env):
    subprocess.run(command, env=env, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--prefix", type=Path, required=True)
    args = parser.parse_args()
    work, prefix = args.work.resolve(), args.prefix.resolve()
    if prefix in {Path("/"), Path("/usr"), Path("/usr/local"), Path.home()}:
        parser.error("Use an isolated build/install directory")
    work.mkdir(parents=True, exist_ok=True)
    prefix.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["CMAKE_PREFIX_PATH"] = str(prefix) + os.pathsep + env.get("CMAKE_PREFIX_PATH", "")
    env["PKG_CONFIG_PATH"] = str(prefix / "lib/pkgconfig") + os.pathsep + env.get("PKG_CONFIG_PATH", "")
    manifest = Path(__file__).with_name("window-system-sources.toml")
    for source in tomllib.loads(manifest.read_text())["source"]:
        name = source["name"]
        archive = work / (name + ".tar.xz")
        if not archive.exists():
            with urllib.request.urlopen(source["url"], timeout=60) as response:
                archive.write_bytes(response.read())
        if hashlib.sha256(archive.read_bytes()).hexdigest() != source["sha256"]:
            raise RuntimeError(f"Source checksum mismatch: {name}")
        source_dir, build_dir = work / name, work / (name + "-build")
        if not source_dir.exists():
            with tarfile.open(archive) as tar:
                tar.extractall(work, filter="data")
        if source["build"] == "meson":
            run(["meson", "setup", str(build_dir), str(source_dir), "--prefix", str(prefix),
                "--libdir", "lib", "-Dtests=false", *(["--reconfigure"] if build_dir.exists() else [])], env)
            run(["meson", "install", "-C", str(build_dir)], env)
        else:
            options = ["-DBUILD_TESTING=OFF", "-DBUILD_QCH=OFF", "-DBUILD_QDOC=OFF",
                "-DCMAKE_BUILD_TYPE=Release", f"-DCMAKE_INSTALL_PREFIX={prefix}",
                "-DKDE_INSTALL_USE_QT_SYS_PATHS=OFF", "-DKDE_INSTALL_LIBDIR=lib",
                "-DKDE_INSTALL_PLUGINDIR=lib/qt6/plugins"]
            if name.startswith("kwindowsystem-"):
                options += ["-DKWINDOWSYSTEM_X11=OFF", "-DKWINDOWSYSTEM_QML=OFF"]
            run(["cmake", "-S", str(source_dir), "-B", str(build_dir), *options], env)
            run(["cmake", "--build", str(build_dir), "--parallel", "2"], env)
            run(["cmake", "--install", str(build_dir)], env)


if __name__ == "__main__":
    main()
