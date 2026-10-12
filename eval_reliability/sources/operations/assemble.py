#!/usr/bin/env python3
"""Assemble and structurally validate the frozen corpus; stdlib, no matcher."""
import json
from pathlib import Path

root = Path(__file__).resolve().parent
paths = sorted((root / 'cases').glob('*.json')) + [root / 'dcsa_tnt' / 'case.json']
cases = [json.loads(p.read_text()) for p in paths]
types = {'text', 'integer', 'float', 'decimal', 'boolean', 'date', 'timestamp', 'unknown'}
for case in cases:
    source = {f['name'] for f in case['source']['fields']}
    target = {f['name'] for f in case['target']['fields']}
    assert len(source) == len(case['source']['fields'])
    assert len(target) == len(case['target']['fields'])
    assert {a['source'] for a in case['answers']} == source
    assert len(case['answers']) == len(source)
    for side in ['source', 'target']:
        for field in case[side]['fields']:
            assert field['data_type'] in types
            assert isinstance(field['samples'], list) and all(isinstance(v, str) for v in field['samples'])
    for answer in case['answers']:
        assert answer['kind'] in {'match', 'no_match', 'ambiguous'}
        if answer['kind'] == 'match':
            assert answer['target'] in target
        elif answer['kind'] == 'ambiguous':
            assert len(set(answer['targets'])) > 1 and set(answer['targets']) <= target
(root / 'cases.json').write_text(json.dumps(cases, indent=2, ensure_ascii=False) + '\n')
print(f'{len(cases)} cases; {sum(len(c["source"]["fields"]) for c in cases)} source fields; all labels and targets structurally valid')
