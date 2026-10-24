#!/usr/bin/env python3
"""Disposable loopback HTTP/HTTPS origin and deterministic request barriers."""

import argparse
import http.server
import json
import pathlib
import socket
import ssl
import subprocess
import tempfile
import threading
import urllib.request


ARRIVED = {}
RELEASED = {}
LOCK = threading.Lock()


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        path = self.path.split("?", 1)[0]
        if path.startswith("/barrier/"):
            parts = path.strip("/").split("/")
            name = parts[1]
            with LOCK:
                arrived = ARRIVED.setdefault(name, threading.Event())
                released = RELEASED.setdefault(name, threading.Event())
            if len(parts) == 3 and parts[2] == "waiting":
                self.send_response(200 if arrived.is_set() else 425)
                self.end_headers()
                return
            if len(parts) == 3 and parts[2] == "release":
                released.set()
                self.send_response(204)
                self.end_headers()
                return
            arrived.set()
            if not released.wait(15):
                self.send_error(504, "fixture barrier deadline")
                return
            body = b"<!doctype html><title>barrier released</title><p id='barrier'>released</p>"
        elif path == "/secondary":
            body = b"<!doctype html><title>Secondary fixture</title><h1>Secondary</h1>"
        else:
            body = (b"<!doctype html><title>Primary fixture</title><h1>Primary</h1>"
                    b"<input aria-label='Fixture search'><a href='/secondary'>Secondary</a>")
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


class Server:
    def __init__(self, tls=False, port=0):
        self.temp = tempfile.TemporaryDirectory(prefix="ferric-everyday-fixture-")
        self.httpd = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
        self.httpd.daemon_threads = True
        if tls:
            cert = pathlib.Path(self.temp.name) / "fixture.pem"
            key = pathlib.Path(self.temp.name) / "fixture.key"
            subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                            "-keyout", str(key), "-out", str(cert), "-days", "1",
                            "-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1"],
                           check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.load_cert_chain(cert, key)
            self.httpd.socket = context.wrap_socket(self.httpd.socket, server_side=True)
        self.thread = threading.Thread(target=self.httpd.serve_forever, daemon=True)

    @property
    def url(self):
        scheme = "https" if isinstance(self.httpd.socket, ssl.SSLSocket) else "http"
        return f"{scheme}://127.0.0.1:{self.httpd.server_port}"

    def start(self):
        self.thread.start()

    def close(self):
        self.httpd.shutdown()
        self.httpd.server_close()
        self.thread.join(timeout=2)
        self.temp.cleanup()


def check_discovery(build_dir, test_filter):
    import subprocess
    if not test_filter or not test_filter.strip():
        raise SystemExit("empty CTest filter is not allowed")
    result = subprocess.run(["ctest", "--test-dir", build_dir, "--show-only=json-v1"],
                            check=True, text=True, capture_output=True)
    tests = json.loads(result.stdout).get("tests", [])
    names = {entry.get("name", "") for entry in tests}
    required = {
        "everyday_T004_production_navigation",
        "everyday_T004_production_search_and_hints",
        "everyday_T005_discovery",
        "everyday_T006_disposable_profile_isolation",
        "everyday_T006_isolated_portal_fixture",
        "everyday_fixture_http_https_isolation",
    }
    missing = sorted(required - names)
    if not tests or missing:
        raise SystemExit(f"empty or incomplete CTest inventory ({len(tests)} tests); missing={missing}")
    filtered = subprocess.run(["ctest", "--test-dir", build_dir, "--show-only=json-v1",
                               "-R", test_filter], check=True, text=True, capture_output=True)
    matched = json.loads(filtered.stdout).get("tests", [])
    if not matched:
        raise SystemExit(f"CTest filter matched zero tests: {test_filter!r}")
    print(f"CTest discovery found {len(tests)} tests; filter matched {len(matched)}; "
          f"S01 inventory present: {', '.join(sorted(required))}")


def run_filter(build_dir, test_filter):
    import subprocess
    if not test_filter or not test_filter.strip():
        raise SystemExit("empty CTest filter is not allowed")
    subprocess.run(["ctest", "--test-dir", build_dir, "--output-on-failure",
                    "--no-tests=error", "-R", test_filter], check=True)


def self_test():
    servers = [Server(), Server(tls=True)]
    try:
        for server in servers:
            server.start()
        plain = urllib.request.urlopen(servers[0].url + "/primary", timeout=3).read()
        context = ssl._create_unverified_context()  # disposable self-signed localhost fixture only
        secure = urllib.request.urlopen(servers[1].url + "/secondary", context=context, timeout=3).read()
        assert b"Primary fixture" in plain and b"Secondary fixture" in secure
        print("HTTP and HTTPS disposable fixture checks passed")
    finally:
        for server in servers:
            server.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check-discovery", metavar="CTEST_BUILD")
    parser.add_argument("--run-filter", metavar="CTEST_BUILD")
    parser.add_argument("--filter", default="", help="required nonempty CTest regex filter")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--serve", action="store_true")
    parser.add_argument("--https", action="store_true")
    parser.add_argument("--port", type=int, default=0)
    args = parser.parse_args()
    if args.check_discovery:
        check_discovery(args.check_discovery, args.filter)
    elif args.run_filter:
        run_filter(args.run_filter, args.filter)
    elif args.self_test:
        self_test()
    elif args.serve:
        server = Server(tls=args.https, port=args.port)
        server.start()
        print(json.dumps({"origin": server.url}), flush=True)
        try:
            server.thread.join()
        except KeyboardInterrupt:
            server.close()
    else:
        parser.error("choose --check-discovery, --self-test, or --serve")


if __name__ == "__main__":
    main()
