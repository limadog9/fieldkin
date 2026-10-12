#!/usr/bin/env python3
"""Verify frozen corpus archives and offline regeneration without a matcher.

Run normally for all integrated bundles, or with --self-test for corruption checks.
Only temporary copies are rebuilt; the integrated bundles are never modified.
"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


SOURCES = Path(__file__).resolve().parents[2] / 'eval_reliability' / 'sources'
BUNDLES = {
    'financial': ('FROZEN.json', ['extract_stripe.py', 'treasury/extract_cases.py']),
    'healthcare': ('freeze.json', ['build_cases.py']),
    'operations': ('FREEZE.json', ['extract_gbfs.py', 'dcsa_tnt/extract_case.py', 'assemble.py']),
    'blind2/financial': ('FROZEN.json', ['extract.py']),
    'blind2/healthcare': ('freeze.json', ['build_cases.py']),
    'blind2/operations': ('FREEZE.json', ['extract.py']),
}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def frozen_hashes(path):
    manifest = json.loads(path.read_text())
    artifacts = manifest.get('artifacts', manifest.get('sha256'))
    if isinstance(artifacts, list):
        pairs = [(item['path'], item['sha256']) for item in artifacts]
        hashes = dict(pairs)
        if len(hashes) != len(pairs):
            raise ValueError(f'{path}: duplicate artifact paths')
    elif isinstance(artifacts, dict):
        hashes = artifacts
    else:
        raise ValueError(f'{path}: no frozen artifact hashes')
    if not hashes:
        raise ValueError(f'{path}: empty frozen artifact list')
    return hashes


def verify_hashes(directory, hashes):
    root = directory.resolve()
    for relative, expected in hashes.items():
        artifact = (root / relative).resolve()
        if not artifact.is_relative_to(root):
            raise ValueError(f'Artifact escapes bundle: {relative}')
        if not artifact.is_file():
            raise ValueError(f'Missing frozen artifact: {relative}')
        actual = sha256(artifact)
        if actual != expected:
            raise ValueError(f'Frozen artifact differs: {relative}; expected {expected}, got {actual}')


def verify_financial_aggregate(directory):
    stripe = json.loads((directory / 'stripe_plan_price.json').read_text())
    treasury = json.loads((directory / 'treasury/treasury_cases.frozen.json').read_text())
    aggregate = json.loads((directory / 'financial_cases.frozen.json').read_text())
    if aggregate != [stripe] + treasury:
        raise ValueError('Financial aggregate differs from regenerated component cases')


def verify_bundle(name, temporary_root):
    freeze_name, scripts = BUNDLES[name]
    original = SOURCES / name
    hashes = frozen_hashes(original / freeze_name)
    freeze_digest = sha256(original / freeze_name)
    verify_hashes(original, hashes)
    copied = temporary_root / name
    shutil.copytree(original, copied)
    verify_hashes(copied, hashes)
    for script in scripts:
        result = subprocess.run([sys.executable, '-I', str(copied / script)],
                                cwd=copied, capture_output=True, text=True, timeout=60)
        if result.returncode:
            raise RuntimeError(f'{name}/{script} failed:\n{result.stdout}{result.stderr}')
    if sha256(copied / freeze_name) != freeze_digest:
        raise ValueError(f'{name}: extraction changed its freeze manifest')
    verify_hashes(copied, hashes)
    if name == 'financial':
        verify_financial_aggregate(copied)
    # Verify that running extraction left the integrated original untouched.
    verify_hashes(original, hashes)
    if sha256(original / freeze_name) != freeze_digest:
        raise ValueError(f'{name}: original freeze manifest changed during verification')
    print(f'{name}: {len(hashes)} frozen artifacts verified before and after offline extraction')


class CorruptionTests(unittest.TestCase):
    def test_corrupt_source_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / 'source.json'
            source.write_bytes(b'{"original":true}\n')
            hashes = {'source.json': sha256(source)}
            verify_hashes(root, hashes)
            source.write_bytes(b'{"original":false}\n')
            with self.assertRaisesRegex(ValueError, 'Frozen artifact differs'):
                verify_hashes(root, hashes)

    def test_corrupt_fingerprint_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'source.json').write_bytes(b'{}\n')
            with self.assertRaisesRegex(ValueError, 'Frozen artifact differs'):
                verify_hashes(root, {'source.json': '0' * 64})

    def test_path_escape_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(ValueError, 'escapes bundle'):
                verify_hashes(Path(temporary), {'../outside': '0' * 64})


if __name__ == '__main__':
    if sys.argv[1:] == ['--self-test']:
        unittest.main(argv=[sys.argv[0]])
    elif sys.argv[1:]:
        raise SystemExit('Usage: verify_extraction.py [--self-test]')
    else:
        with tempfile.TemporaryDirectory(prefix='fieldkin-corpus-verify-') as temporary:
            for bundle in BUNDLES:
                verify_bundle(bundle, Path(temporary))
