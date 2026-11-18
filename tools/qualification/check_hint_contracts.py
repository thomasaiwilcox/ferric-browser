#!/usr/bin/env python3
"""Run existing pure Rust hint contract tests without linking Qt.

The harness imports production modules and their own tests by path. It does not
exercise Qt or qualify native request ownership or runtime delivery.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
ENGINE = ROOT / 'crates/ferric-browser-engine-qt/src'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/hint-contracts')
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / 'target'):
        parser.error('--output must be inside the repository target directory')
    package = out / 'workspace/crates/engine-contracts'
    (package / 'src').mkdir(parents=True, exist_ok=True)

    def link(path, target):
        if path.is_symlink():
            if path.resolve() != target.resolve():
                raise RuntimeError(f'Unexpected existing link: {path}')
        elif path.exists():
            raise RuntimeError(f'Refusing to replace existing content: {path}')
        else:
            path.symlink_to(target, target_is_directory=target.is_dir())

    # Preserve normal Rust submodule lookup and the fixtures' relative paths.
    # No production source or test body is copied or rewritten.
    link(out / 'workspace/fuzz', ROOT / 'fuzz')
    link(package / 'tests', ENGINE.parent / 'tests')
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text())
    versions = {p['name']: p['version'] for p in lock['package']
                if p['name'] in ('serde', 'serde_json', 'uuid')}
    dependencies = {
        'ferric-browser-core': {'path': str(ROOT / 'crates/ferric-browser-core')},
        'ferric-browser-config': {'path': str(ROOT / 'crates/ferric-browser-config')},
        'ferric-browser-application': {'path': str(ROOT / 'crates/ferric-browser-application')},
        'serde': {'version': '=' + versions['serde'], 'features': ['derive']},
        'serde_json': {'version': '=' + versions['serde_json'], 'features': ['raw_value']},
        'uuid': {'version': '=' + versions['uuid'], 'features': ['v4', 'serde']},
    }
    lines = ['[workspace]', '[package]', 'name="ferric-hint-contracts"',
             'version="0.0.0"', 'edition="2024"', '[features]', 'fuzzing=[]',
             '[dependencies]']
    for name, fields in dependencies.items():
        values = ', '.join(f'{key}={json.dumps(value)}' for key, value in fields.items())
        lines.append(f'{name}={{ {values} }}')
    (package / 'Cargo.toml').write_text('\n'.join(lines) + '\n')
    declaration = re.search(r'^const MAX_UNTRUSTED_ARGUMENT_BYTES: usize = [^;]+;',
                            (ENGINE / 'lib.rs').read_text(), re.MULTILINE)
    if declaration is None:
        raise RuntimeError('Could not read the production input limit declaration')
    modules = ['input_validation', 'hint_policy', 'hint_payload', 'interaction_proof',
               'hint_collection_work']
    source = ['#![deny(unsafe_code)]', '#![allow(dead_code)]', declaration.group()]
    for name in modules:
        link(package / 'src' / (name + '.rs'), ENGINE / (name + '.rs'))
        if (ENGINE / name).is_dir():
            link(package / 'src' / name, ENGINE / name)
        source.append(f'mod {name};')
    source.append('pub(crate) use hint_payload::parse_hint_payload;')
    (package / 'src/lib.rs').write_text('\n'.join(source) + '\n')
    # Seed every run from the current production lock, rather than carrying
    # an independently resolved dependency set across implementation changes.
    (package / 'Cargo.lock').write_bytes((ROOT / 'Cargo.lock').read_bytes())
    command = ['cargo', 'test', '--offline', '-j', '2', '--manifest-path', str(package / 'Cargo.toml'),
               '--target-dir', str(out / 'target')]
    if (out / 'test.log').exists():
        attempt = 1
        while (out / f'attempt-{attempt}.log').exists():
            attempt += 1
        (out / 'test.log').rename(out / f'attempt-{attempt}.log')
    with (out / 'test.log').open('w') as log:
        result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    inputs = [ENGINE / 'lib.rs'] + [ENGINE / (name + '.rs') for name in modules]
    for directory in ['interaction_proof', 'hint_collection_work', 'hint_payload']:
        inputs.extend(sorted((ENGINE / directory).rglob('*.rs')))
    record = {'command': command, 'exit_code': result.returncode,
              'scope': 'Actual pure Rust modules and existing tests; excludes native/Qt runtime.',
              'source_revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'],
                                                       cwd=ROOT, text=True).strip(),
              'production_lock_sha256': hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest(),
              'sources': {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                          for p in inputs},
              'generated_lock_sha256': hashlib.sha256((package / 'Cargo.lock').read_bytes()).hexdigest()}
    (out / 'provenance.json').write_text(json.dumps(record, indent=2) + '\n')
    print('\n'.join(line for line in (out / 'test.log').read_text().splitlines()
                    if line.startswith(('test result:', 'error:'))))
    print(f'Full output: {out / "test.log"}')
    return result.returncode


if __name__ == '__main__':
    raise SystemExit(main())
