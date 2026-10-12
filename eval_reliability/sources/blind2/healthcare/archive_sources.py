#!/usr/bin/env python3
"""Verify archived publisher bytes; --fetch retrieves missing files by pinned URL.
Only stdlib. Does not mutate existing archives or overwrite checksum mismatches.
"""
from pathlib import Path
import argparse,hashlib,json,urllib.request
B=Path(__file__).resolve().parent
ap=argparse.ArgumentParser();ap.add_argument('--fetch',action='store_true');args=ap.parse_args()
manifest=json.loads((B/'sources.json').read_text())
for source in manifest['sources']:
 path=B/source['path']
 if not path.exists() and args.fetch:
  with urllib.request.urlopen(source['url'],timeout=60) as response:data=response.read()
  assert hashlib.sha256(data).hexdigest()==source['sha256'],source['path']
  path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
 data=path.read_bytes()
 assert hashlib.sha256(data).hexdigest()==source['sha256'],source['path']
 assert len(data)==source['bytes'],source['path']
print('Verified',len(manifest['sources']),'publisher-source checksums.')
