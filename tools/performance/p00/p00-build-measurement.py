#!/usr/bin/env python3
"""Build the P00 measurement executable and retain the Cargo/CMake inputs."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

SOURCE_DIR = Path(__file__).resolve().parent
BASE = Path(os.environ.get('FERRIC_P00_ARTIFACT_DIR', str(SOURCE_DIR.parents[2] / 'target/site-policy-baseline'))).resolve()
ROOT = BASE / 'p00-pinned-source'
PIN = 'f2c5d6deebd97eb861084503f25613a6a819949d'
TARGET = BASE / 'p00-pinned-target'
BUILD = ROOT / 'target/p00-measurement-application'
ENGINE = ROOT / 'crates/ferric-browser-engine-qt'
WRAPPER = ROOT / 'tools/performance/with_qt611.sh'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def inventory():
    output = subprocess.check_output(
        ['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=ROOT
    )
    result = {}
    for name in sorted(set(output.split(b'\0')) - {b''}):
        relative = os.fsdecode(name)
        path = ROOT / relative
        if path.is_symlink() or not path.is_file():
            raise RuntimeError(f'Source must be a regular file: {relative}')
        result[relative] = digest(path)
    return result


def expected_sources(mode, measurement='v7'):
    if measurement not in ('v7', 'v8', 'v9', 'v10', 'v11'):
        raise ValueError('Unknown P00 measurement overlay')
    source = {}
    for line in (SOURCE_DIR / 'p00-pinned-original-source.sha256').read_text().splitlines():
        sha, name = line.split('  ', 1)
        source[name] = sha
    overlays = [(f'p00-measurement-overlay-{measurement}', 'overlay_sha256')]
    if mode == 'diagnostic':
        overlays.append(('p00-broker-diagnostic-overlay', 'diagnostic_sha256'))
    records = []
    for name, field in overlays:
        metadata = json.loads((SOURCE_DIR / f'{name}.json').read_text())
        if metadata['revision'] != PIN or digest(SOURCE_DIR / f'{name}.patch') != metadata['patch_sha256']:
            raise RuntimeError(f'Overlay metadata does not match the pinned patch: {name}')
        for path, hashes in metadata['inputs'].items():
            if source.get(path) != hashes['original_sha256']:
                raise RuntimeError(f'Overlay input does not match the pin: {path}')
            source[path] = hashes[field]
        records.append({'patch': name, 'sha256': metadata['patch_sha256']})
    return source, records


def cargo_paths(path):
    artifacts = set()
    browser_artifacts = set()
    script_dirs = {}
    for line in path.read_text().splitlines():
        message = json.loads(line)
        reason = message.get('reason')
        package = message.get('package_id')
        if reason == 'build-script-executed':
            script_dirs.setdefault(package, set()).add(Path(message['out_dir']))
        if reason != 'compiler-artifact':
            continue
        target = message['target']
        if Path(target['src_path']).resolve() == ENGINE / 'src/lib.rs':
            for name in message['filenames']:
                archive = Path(name)
                if archive.name == 'libferric_browser_engine_qt.a':
                    artifacts.add((package, archive.resolve()))
        if (Path(target['src_path']).resolve() == ROOT / 'crates/ferric-browser/src/main.rs'
                and message.get('executable')):
            browser_artifacts.add(Path(message['executable']).resolve())
    if len(artifacts) != 1 or len(browser_artifacts) != 1:
        raise RuntimeError('Cargo did not identify one pinned archive and browser executable')
    package, archive = artifacts.pop()
    browser = browser_artifacts.pop()
    generated = script_dirs.get(package, set())
    if len(generated) != 1:
        raise RuntimeError('Cargo did not identify one generated-header producer for the archive')
    include_dir = generated.pop().resolve() / 'cxxqtbuild/include'
    if not archive.is_file() or not browser.is_file() or not include_dir.is_dir():
        raise RuntimeError('Cargo artifact or generated headers are missing')
    return archive, include_dir, browser


def reuse_producer(path, current_sources):
    previous = json.loads(path.read_text())
    if (previous['status'] != 'passed' or previous['mode'] != 'plain'
            or previous['revision'] != PIN or previous['source_root'] != str(ROOT)
            or not previous['source_postcheck_exact_match']):
        raise RuntimeError('Only a passing pinned plain build can supply a reused producer')
    overlay = previous['overlays'][0]['patch'].removeprefix('p00-measurement-overlay-')
    expected, overlays = expected_sources('plain', overlay)
    manifest = path.with_name('source-inputs.json')
    if (previous['overlays'] != overlays or digest(manifest) != previous['source_manifest_sha256']
            or json.loads(manifest.read_text()) != expected):
        raise RuntimeError('The prior source manifest does not establish the pinned production source')
    changes = {name for name in expected.keys() | current_sources.keys()
               if expected.get(name) != current_sources.get(name)}
    if any(not name.startswith('crates/ferric-browser-engine-qt/tests/') for name in changes):
        raise RuntimeError('Producer reuse requires identical production sources')
    producer = previous['cargo_producer']
    if (digest(Path(producer['archive'])) != producer['archive_sha256']
            or digest(Path(producer['browser'])) != producer['browser_sha256']
            or not Path(producer['generated_headers']).is_dir()):
        raise RuntimeError('Prior production artifacts no longer match their build receipt')
    return producer


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('plain', 'diagnostic'))
    parser.add_argument('--attempt', type=int, default=1)
    parser.add_argument('--overlay', choices=('v7', 'v8', 'v9', 'v10', 'v11'), default='v7')
    parser.add_argument('--reuse-producer', type=Path)
    args = parser.parse_args()
    if args.reuse_producer and args.mode != 'plain':
        parser.error('Producer reuse requires a plain build')
    evidence = BASE / f'p00-{args.overlay}-build-{args.mode}-{args.attempt}'
    evidence.mkdir()
    env = dict(os.environ, CARGO_NET_OFFLINE='true', CARGO_BUILD_JOBS='2',
               CARGO_TARGET_DIR=str(TARGET), CCACHE_DIR=str(BASE / 'compiler-cache'),
               CCACHE_CONFIGPATH=str(SOURCE_DIR / 'ccache.conf'), CCACHE_REMOTE_STORAGE='',
               CC=(str(BASE / 'build-tools/ccache') + ' cc') if (BASE / 'build-tools/ccache').is_file() else 'cc',
               CXX=(str(BASE / 'build-tools/ccache') + ' c++') if (BASE / 'build-tools/ccache').is_file() else 'c++',
               FERRIC_QTWAYLAND_PREFIX=os.environ.get('FERRIC_QTWAYLAND_PREFIX', str(BASE / 'qtwayland-sdk')))
    record = {'status': 'running', 'pid': os.getpid(), 'source_root': str(ROOT),
              'mode': args.mode, 'builder_sha256': digest(Path(__file__)),
              'started': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'commands': []}

    def save():
        temporary = evidence / 'build-record.tmp'
        temporary.write_text(json.dumps(record, indent=2) + '\n')
        temporary.replace(evidence / 'build-record.json')

    def run(name, arguments, separate_output=False):
        command = ['nice', '-n', '10', str(WRAPPER), *arguments]
        record['commands'].append(command)
        save()
        with (evidence / f'{name}.log').open('x') as log:
            if separate_output:
                with (evidence / f'{name}.jsonl').open('x') as messages:
                    subprocess.run(command, cwd=ROOT, env=env, stdout=messages, stderr=log, check=True)
            else:
                subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)

    before = None
    save()
    try:
        revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
        record['revision'] = revision
        if revision != PIN:
            raise RuntimeError('Pinned HEAD changed')
        expected, overlays = expected_sources(args.mode, args.overlay)
        before = inventory()
        manifest = evidence / 'source-inputs.json'
        manifest.write_text(json.dumps(before, indent=2) + '\n')
        record.update(overlays=overlays, source_manifest_sha256=digest(manifest))
        if before != expected:
            paths = sorted(name for name in before.keys() | expected.keys()
                           if before.get(name) != expected.get(name))
            raise RuntimeError('Unexpected source inputs: ' + ', '.join(paths))
        if args.reuse_producer:
            previous = args.reuse_producer.resolve()
            producer = reuse_producer(previous, before)
            record['reused_producer_record'] = {'path': str(previous), 'sha256': digest(previous)}
            archive = Path(producer['archive'])
            include_dir = Path(producer['generated_headers'])
            browser = Path(producer['browser'])
        else:
            run('cargo', ['cargo', 'build', '--release', '-p', 'ferric-browser-engine-qt',
                          '-p', 'ferric-browser', '--locked', '--offline', '--message-format=json'], True)
            archive, include_dir, browser = cargo_paths(evidence / 'cargo.jsonl')
            producer = {'archive': str(archive), 'archive_sha256': digest(archive),
                        'generated_headers': str(include_dir), 'browser': str(browser),
                        'browser_sha256': digest(browser)}
        record['cargo_producer'] = producer
        run('configure', ['cmake', '-G', 'Unix Makefiles', '-S', str(ENGINE / 'tests/application'),
                          '-B', str(BUILD),
                          f'-DFERRIC_ENGINE_LIBRARY={archive}',
                          f'-DFERRIC_ENGINE_CXX_INCLUDE_DIR={include_dir}',
                          f'-DFERRIC_BROWSER_EXECUTABLE={browser}'])
        run('build', ['cmake', '--build', str(BUILD), '--target', 'application_tests', '--parallel', '2'])
        binary = BUILD / 'application_tests'
        for path in (BUILD / 'CMakeCache.txt', BUILD / 'CMakeFiles/application_tests.dir/link.txt',
                     BUILD / 'CMakeFiles/application_tests.dir/flags.make'):
            shutil.copyfile(path, evidence / path.name)
        if digest(archive) != producer['archive_sha256'] or digest(browser) != producer['browser_sha256']:
            raise RuntimeError('Cargo artifacts changed during native harness build')
        record.update(binary=str(binary), binary_sha256=digest(binary), status='passed')
    except BaseException as error:
        record.update(status='failed', error=f'{type(error).__name__}: {error}')
    finally:
        if before is not None:
            try:
                record['source_postcheck_exact_match'] = inventory() == before
            except Exception as error:
                record['source_postcheck_exact_match'] = False
                record['source_postcheck_error'] = f'{type(error).__name__}: {error}'
            if not record['source_postcheck_exact_match']:
                record['status'] = 'failed'
        record['finished'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        save()
    print(json.dumps(record, indent=2))
    return 0 if record['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
