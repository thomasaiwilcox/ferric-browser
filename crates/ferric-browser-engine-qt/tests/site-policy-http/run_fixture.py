#!/usr/bin/env python3
"""Run canonical-origin fixture tests using the already-built qualified runner."""
import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import subprocess
from threading import Thread


def main():
    directory = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser()
    parser.add_argument('--runner', type=Path, required=True)
    parser.add_argument('--test-function', action='append', default=[])
    parser.add_argument('--test-file', choices=sorted(path.name for path in directory.glob('tst_*.qml')))
    args = parser.parse_args()
    handler = partial(SimpleHTTPRequestHandler, directory=str(directory.parent / 'fixtures/site-policy'))
    # Match the existing frame-probe harness. Binding failure is an error;
    # never reuse an unknown service already listening on this port.
    server = ThreadingHTTPServer(('127.0.0.1', 8766), handler)
    worker = Thread(target=server.serve_forever, daemon=True)
    worker.start()
    env = dict(os.environ, QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
               QT_QPA_PLATFORMTHEME='', NO_PROXY='localhost,127.0.0.1', no_proxy='localhost,127.0.0.1')
    try:
        # Each TestCase file owns its state and cleanup. Keep its existing
        # process ceiling as the suite grows, rather than stretching deadlines
        # for DOM/native operations or cutting cases out of the maintained run.
        if args.test_file:
            inputs = [directory / args.test_file]
        elif args.test_function:
            inputs = [directory]
        else:
            inputs = sorted(directory.glob('tst_*.qml'))
        for source in inputs:
            result = subprocess.run([str(args.runner.resolve()), '-input', str(source)] + args.test_function,
                                    env=env, timeout=180, check=False)
            if result.returncode:
                return result.returncode
        return 0
    finally:
        server.shutdown()
        server.server_close()
        worker.join(timeout=2)


if __name__ == '__main__':
    raise SystemExit(main())
