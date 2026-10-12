#!/usr/bin/env python3
"""Reproduce GBFS held-out cases from archived official sources; stdlib only.

No network, matcher, or prediction access. Dotted names are literal JSON key
paths, [] means array traversal. Primitive array samples remain individual
publisher values. Samples are canonical strings required by EvalCase.
"""
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent
RES = ROOT / 'resources' / 'gbfs'
PROVENANCE = json.loads((ROOT / 'gbfs-provenance.json').read_text())
for resource in PROVENANCE:
    assert hashlib.sha256((ROOT / resource['path']).read_bytes()).hexdigest() == resource['sha256']


def section(version, name):
    text = (RES / f'gbfs-{version}.md').read_text()
    start = text.index('### ' + name + '.json')
    end = text.find('\n### ', start + 1)
    return text[start:end if end != -1 else len(text)]


def examples(version, name):
    result = []
    blocks = re.findall(r'```json\s*\n(.*?)\n```', section(version, name), re.S)
    if name == 'geofencing_zones':
        blocks = blocks[:1]
    for code in blocks:
        result.append(json.loads(code))
    return result


def leaves(node, path='', pointer=''):
    if node.get('type') == 'object':
        for key, child in node.get('properties', {}).items():
            yield from leaves(child, path + '.' + key if path else key, pointer + '/properties/' + key)
    elif node.get('type') == 'array':
        yield from leaves(node['items'], path + '[]', pointer + '/items')
    else:
        yield path, node, pointer


def values(obj, path):
    if not path:
        return [obj]
    key, _, rest = path.partition('.')
    array_depth = key.count('[]')
    key = key.replace('[]', '')
    if not isinstance(obj, dict) or key not in obj:
        return []
    selected = [obj[key]]
    for _ in range(array_depth):
        selected = [item for array in selected if isinstance(array, list) for item in array]
    return [value for item in selected for value in values(item, rest)]


def text_sample(value):
    return value if isinstance(value, str) else json.dumps(value, ensure_ascii=False, separators=(',', ':'))


def datatype(node):
    # No inference from observed examples. Retain incomplete declared item types as unknown.
    if node.get('format') == 'date-time':
        return 'timestamp'
    return {'string': 'text', 'integer': 'integer', 'number': 'float', 'boolean': 'boolean'}.get(node.get('type'), 'unknown')


def field_evidence(version, name, path, node, pointer, sample_values):
    schema_file = f'resources/gbfs/{version}-{name}.json'
    leaf = path.rsplit('.', 1)[-1].replace('[]', '')
    docs = (RES / f'gbfs-{version}.md').read_text().splitlines()
    selected_lines = []
    within = False
    for number, line in enumerate(docs, 1):
        if line.startswith('### '):
            within = line == '### ' + name + '.json'
        if within and '|' in line and ('`' + leaf + '`' in line or ('.' + leaf + '`') in line):
            selected_lines.append({'line': number, 'text': line})
    return {'name': path, 'declared_schema_type': node.get('type'), 'declared_schema_format': node.get('format'),
            'normalized_type': datatype(node), 'schema_file': schema_file, 'schema_pointer': pointer,
            'schema_definition': node, 'documentation_file': f'resources/gbfs/gbfs-{version}.md',
            'definition_lines': selected_lines, 'sample_origin': f'Publisher JSON example(s) in {name}.json section',
            'sample_path': path, 'samples': sample_values,
            'missing_sample_evidence': not sample_values}


def extract(version, name, keep):
    schema = json.loads((RES / f'{version}-{name}.json').read_text())
    records = examples(version, name)
    result, evidence = [], []
    selected = set(keep)
    for path, node, pointer in leaves(schema):
        if path not in selected:
            continue
        samples = list(dict.fromkeys(text_sample(v) for record in records for v in values(record, path)))[:8]
        result.append({'name': path, 'data_type': datatype(node), 'samples': samples})
        evidence.append(field_evidence(version, name, path, node, pointer, samples))
    assert {f['name'] for f in result} == selected
    return {'fields': result}, evidence


def all_paths(version, name):
    return [p for p, _, _ in leaves(json.loads((RES / f'{version}-{name}.json').read_text()))]


def write_case(name, left, right, mapping, rationales, projection, limitations):
    source, source_ev = extract(*left)
    target, target_ev = extract(*right)
    targets = {f['name'] for f in target['fields']}
    answers, answers_ev = [], []
    for field in source['fields']:
        src = field['name']
        dest = mapping.get(src)
        answer = {'kind': 'match', 'source': src, 'target': dest} if dest else {'kind': 'no_match', 'source': src}
        if dest:
            assert dest in targets
        answers.append(answer)
        answers_ev.append({**answer, 'reason': rationales[src]})
    case = {'name': name, 'source': source, 'target': target, 'answers': answers}
    ev = {'name': name, 'source': source_ev, 'target': target_ev, 'answers': answers_ev,
          'projection': projection, 'limitations': limitations,
          'label_standard': 'Match means same operational concept, allowing ordinary representation conversion. A combined predicate split into independent predicates has no single equivalent target. Arrays are flattened mechanically. Different scope or operational state is not equivalent.'}
    (ROOT / 'cases').mkdir(exist_ok=True)
    (ROOT / 'evidence').mkdir(exist_ok=True)
    (ROOT / 'cases' / (name + '.json')).write_text(json.dumps(case, indent=2, ensure_ascii=False) + '\n')
    (ROOT / 'evidence' / (name + '.json')).write_text(json.dumps(ev, indent=2, ensure_ascii=False) + '\n')
    return case


def main():
    src_prefix, dst_prefix = 'data.bikes[].', 'data.vehicles[].'
    src = [p for p in all_paths('2.3', 'free_bike_status') if p.startswith(src_prefix)]
    missing = {'station_id', 'is_disabled', 'last_reported'}
    dst = [p for p in all_paths('3.0', 'vehicle_status') if p.startswith(dst_prefix) and p[len(dst_prefix):] not in missing] + ['last_updated']
    mapping, why = {}, {}
    for p in src:
        leaf = p[len(src_prefix):]
        if leaf not in missing:
            mapping[p] = dst_prefix + ('vehicle_id' if leaf == 'bike_id' else leaf)
            why[p] = ('Both releases define a rotating identifier of a vehicle; v3.0 explicitly replaces free_bike_status with vehicle_status and bike_id with vehicle_id.' if leaf == 'bike_id' else 'Both release definitions describe the same per-vehicle concept. The containing bikes array is renamed vehicles in v3.0; the original leaf key is unchanged.')
        elif leaf == 'station_id':
            why[p] = 'The current physical station is absent from the target projection. home_station_id is the required return station, which need not be the current station. vehicle_type_id and vehicle_id also identify different entities.'
        elif leaf == 'is_disabled':
            why[p] = 'The disabled-state field is absent from the target projection. is_reserved records a reservation, not a mechanical issue or low battery. The publisher examples happen to use false for both, which does not make the concepts equivalent.'
        else:
            why[p] = 'The last vehicle-to-backend report time is absent from the target projection. last_updated is the publisher/feed update time; Data Latency explicitly distinguishes these. available_until is a rental-completion deadline.'
    one = write_case('gbfs_vehicle_2_3_to_3_0_projected', ('2.3', 'free_bike_status', src), ('3.0', 'vehicle_status', dst), mapping, why,
        {'source': 'All scalar leaves under data.bikes[]; envelope excluded.', 'target': 'All scalar leaves under data.vehicles[] except station_id, is_disabled, last_reported; plus envelope last_updated. This explicit partial-consumer projection tests current-vs-home station, disabled-vs-reserved state and report-vs-feed-update time.'},
        ['Publisher-authored illustrative examples, not observed live fleet records.', 'The v2.3 carsharing example has an undeclared home_station key. It is not silently repaired; only the declared home_station_id sample from the other record is extracted.', 'rental_uris.web has no value in the examples; samples remain empty.', 'vehicle_equipment item schemas contain an enum without an explicit type, retained as unknown instead of inferring a declaration.', 'No supported genuine one-to-many ambiguity label is manufactured.'])
    src = all_paths('2.3', 'geofencing_zones')
    local = 'data.geofencing_zones.features[].properties.rules[].'
    dst = [p for p in all_paths('3.0', 'geofencing_zones') if p not in {local + 'ride_through_allowed', local + 'station_parking'}]
    mapping, why = {}, {}
    for p in src:
        if p == local + 'ride_allowed':
            why[p] = 'v2.3 ride_allowed jointly governs starting and ending undocked rides. v3.0 has independent ride_start_allowed and ride_end_allowed. Neither one alone represents the original joint predicate, so this is not a one-to-one equivalent match or a pair of interchangeable targets.'
        elif p in {local + 'ride_through_allowed', local + 'station_parking'}:
            why[p] = 'The zone-local rule is absent from the target projection. The same leaf name under global_rules governs default restrictions outside an explicitly overriding zone, not this polygon. The v3.0 Rule Precedence prose distinguishes the scopes.'
        else:
            dest = p
            if p.endswith('vehicle_type_id[]'):
                dest = p.replace('vehicle_type_id[]', 'vehicle_type_ids[]')
                why[p] = 'Both definitions are arrays of vehicle type IDs to which this polygon rule applies; v3.0 pluralizes the original key.'
            elif p.endswith('properties.name'):
                dest = p + '[].text'
                why[p] = 'Both are the public geofencing-zone name. v3.0 stores localized text objects; the projected text value is the name, while language is its locale code. This is semantic matching, with locale-aware transformation required in deployment.'
            elif p in {'last_updated', 'data.geofencing_zones.features[].properties.start', 'data.geofencing_zones.features[].properties.end'}:
                why[p] = 'Both definitions describe the same time concept. Physical encoding changes from POSIX integer seconds to RFC3339 string; a conversion is required, not a different event meaning.'
            else:
                why[p] = 'Same original JSON leaf and role in both releases; retained as an exact-name control with actual publisher example values.'
            mapping[p] = dest
    two = write_case('gbfs_geofencing_2_3_to_3_0_scopes', ('2.3', 'geofencing_zones', src), ('3.0', 'geofencing_zones', dst), mapping, why,
        {'source': 'All scalar leaves of the v2.3 geofencing endpoint; arrays mechanically traversed.', 'target': 'All scalar leaves of v3.0 geofencing endpoint except polygon-local ride_through_allowed and station_parking. Their global-rule counterparts are retained as explicit scope distractors. Only the first complete endpoint example is sampled; later polygon-overlap fragments are not mixed into feed records.'},
        ['Publisher-authored illustrative examples, not observed service boundaries.', 'The v3.0 example omits some schema-defined global rule values; these produce empty samples.', 'One source predicate splits into two independent target predicates. It is labeled no_match under the one-to-one equivalence standard, not ambiguous.', 'Coordinates are mechanically flattened numeric leaves, not invented longitude/latitude columns.', 'Mapping a localized name requires selecting the intended locale; the matcher case evaluates the name concept only.'])
    for case in (one, two):
        print(case['name'], len(case['source']['fields']), len(case['target']['fields']), {k: sum(a['kind'] == k for a in case['answers']) for k in ['match', 'no_match', 'ambiguous']})


if __name__ == '__main__':
    main()
