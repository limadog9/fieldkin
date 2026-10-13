#!/usr/bin/env python3
"""Verify pinned official sources and reproduce raw excerpts without evaluation."""
import argparse, hashlib, json, urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--restore', action='store_true', help='Restore raw artifacts with exactly the frozen expected bytes.')
args = parser.parse_args()

for item in json.loads((ROOT/'PROVENANCE.json').read_text()):
    with urllib.request.urlopen(item['original_url'], timeout=60) as response:
        full = response.read()
    assert hashlib.sha256(full).hexdigest() == item['original_file_sha256'], item['original_url']
    artifact = full
    if item['excerpt_selectors']:
        doc = json.loads(full)
        selected = {'openapi':doc['openapi'],'info':{'title':doc['info']['title'],'version':doc['info'].get('version')},'components':{'schemas':{'PaymentRequest':doc['components']['schemas']['PaymentRequest']}}}
        artifact = (json.dumps(selected,indent=2,ensure_ascii=False)+'\n').encode()
    assert hashlib.sha256(artifact).hexdigest() == item['artifact_sha256'], item['artifact']
    path = ROOT/item['artifact']
    if args.restore:
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(artifact)
    else:
        assert path.read_bytes() == artifact, item['artifact']
print('All pinned publisher sources and frozen raw excerpts verified.')
