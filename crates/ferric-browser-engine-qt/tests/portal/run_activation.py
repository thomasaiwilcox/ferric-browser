"""Test D-Bus activation without enabling any desktop service directories."""

from pathlib import Path
import os
import shlex
import signal
import subprocess
import sys
import tempfile
from xml.sax.saxutils import escape


def main():
    build = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix="ferric-portal-activation-") as temporary:
        root = Path(temporary)
        env = os.environ.copy()
        for key in ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_RUNTIME_DIR"]:
            directory = root / key
            directory.mkdir(mode=0o700)
            env[key] = str(directory)
        env["FERRIC_PORTAL_TEST_BUS"] = "isolated"
        service_dir = root / "services"
        service_dir.mkdir()
        (service_dir / "org.freedesktop.portal.Desktop.service").write_text(
            "[D-BUS Service]\nName=org.freedesktop.portal.Desktop\n"
            f"Exec={shlex.quote(str(build / 'portal_service'))}\n")
        config = root / "bus.conf"
        template = Path(__file__).resolve().parent.parent / "application/notification-test-bus.conf"
        config.write_text(template.read_text().replace("</busconfig>",
            f"<servicedir>{escape(str(service_dir))}</servicedir></busconfig>"))
        process = subprocess.Popen([sys.argv[2], f"--config-file={config}", "--",
            str(build / "portal_activation_tests")], env=env, start_new_session=True)
        try:
            return process.wait(timeout=60)
        finally:
            # All descendants belong to this disposable fixture session. The
            # activated service may outlive the test client, so retire it too.
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            process.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
