#!/usr/bin/env python3
"""Offline publisher-only extraction for one new operational record schema."""
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent
P = 'data.stations[].'
OMITTED = {P + 'vehicle_docks_available[].count', P + 'num_docks_disabled', P + 'is_renting'}
RENAMES = {P + 'num_bikes_available': P + 'num_vehicles_available', P + 'num_bikes_disabled': P + 'num_vehicles_disabled'}
REASONS = {
    P + 'vehicle_docks_available[].count': 'The source counts available dock points compatible with specified vehicle types. The target projection omits this field. vehicle_types_available[].count counts available vehicles, and num_docks_available counts all compatible dock classes together. Neither preserves the per-dock-class quantity.',
    P + 'num_docks_disabled': 'The source counts disabled dock points. The target omits this field; num_docks_available counts functional return points and num_vehicles_disabled counts vehicles, not dock points.',
    P + 'is_renting': 'The source indicates whether the station allows rentals. The target omits this flag; is_returning indicates permission to return vehicles and is_installed indicates street installation. All three being true in the publisher examples does not make them equivalent.',
    P + 'num_bikes_available': 'The v2.3 and v3.0 definitions both specify functional vehicles physically at the station that may be offered for rental. Only the bikes-to-vehicles term changes in the field name.',
    P + 'num_bikes_disabled': 'Both definitions specify the number of disabled vehicles of any type at the station; the field changes bikes to vehicles without changing its entity or state.',
}


def leaves(node, path='', pointer='', ancestors=()):
    if node.get('type') == 'object':
        for key, child in node.get('properties', {}).items():
            child_path = path + '.' + key if path else key
            yield from leaves(child, child_path, pointer + '/properties/' + key,
                              ancestors + ({'path': child_path, 'description': child.get('description')},))
    elif node.get('type') == 'array':
        yield from leaves(node['items'], path + '[]', pointer + '/items', ancestors)
    else:
        yield path, node, pointer, ancestors


def values(value, path):
    if not path:
        return [value]
    head, _, tail = path.partition('.')
    key = head.replace('[]', '')
    if not isinstance(value, dict) or key not in value:
        return []
    selected = value[key] if head.endswith('[]') else [value[key]]
    return [v for item in selected for v in values(item, tail)]


def get_fields(version):
    schema_name = f'resources/station-status-{version}.json'
    schema = json.loads((ROOT / schema_name).read_text())
    doc_name = f'resources/gbfs-{version}.md'
    document = (ROOT / doc_name).read_text()
    start = document.index('### station_status.json')
    end = document.index('\n### ', start + 1)
    section = document[start:end]
    example = json.loads(re.search(r'```json\s*\n(.*?)\n```', section, re.S)[1])
    first_line = document[:start].count('\n') + 1
    fields, evidence = [], []
    for path, node, pointer, ancestors in leaves(schema):
        if version == '3.0' and path in OMITTED:
            continue
        dtype = 'timestamp' if node.get('format') == 'date-time' else {'integer': 'integer', 'string': 'text', 'boolean': 'boolean'}[node['type']]
        samples = list(dict.fromkeys(v if isinstance(v, str) else json.dumps(v) for v in values(example, path)))
        assert samples, f'Unexpected missing publisher sample: {version}/{path}'
        fields.append({'name': path, 'data_type': dtype, 'samples': samples})
        leaf = path.rsplit('.', 1)[-1].replace('[]', '')
        lines = [{'line': first_line + i, 'quote': line} for i, line in enumerate(section.splitlines())
                 if '|' in line and ('`' + leaf + '`' in line or '.' + leaf + '`' in line)]
        evidence.append({'name': path, 'normalized_type': dtype, 'schema': schema_name,
                         'schema_pointer': pointer, 'declared_schema': node,
                         'ancestor_descriptions': ancestors, 'documentation': doc_name,
                         'definition_lines': lines, 'sample_path': path, 'samples': samples,
                         'sample_origin': 'Literal values of the sole publisher-authored JSON example in station_status.json section.'})
    return {'fields': fields}, evidence


def main():
    for resource in json.loads((ROOT / 'provenance.json').read_text()):
        assert hashlib.sha256((ROOT / resource['path']).read_bytes()).hexdigest() == resource['sha256']
    source, source_evidence = get_fields('2.3')
    target, target_evidence = get_fields('3.0')
    target_names = {f['name'] for f in target['fields']}
    answers, answer_evidence = [], []
    for field in source['fields']:
        path = field['name']
        if path in OMITTED:
            answer = {'kind': 'no_match', 'source': path}
        else:
            destination = RENAMES.get(path, path)
            assert destination in target_names
            answer = {'kind': 'match', 'source': path, 'target': destination}
        reason = REASONS.get(path, 'The same publisher-defined field retains its station-state or envelope concept in both versions; JSON key path is unchanged.')
        if path in {'last_updated', P + 'last_reported'}:
            reason += ' Its time encoding changes from POSIX integer seconds to RFC3339 string; conversion is required, but the represented event is unchanged.'
        answers.append(answer)
        answer_evidence.append({**answer, 'reason': reason})
    case = {'name': 'gbfs_station_status_2_3_to_3_0_partial_consumer', 'source': source, 'target': target, 'answers': answers}
    evidence = {'source_fields': source_evidence, 'target_fields': target_evidence, 'answers': answer_evidence,
                'source_projection': 'All 16 scalar leaves of the complete v2.3 station_status endpoint, including envelope and mechanically traversed array item leaves.',
                'target_projection': 'All scalar leaves of the v3.0 endpoint except the three explicitly named omitted fields.',
                'omitted_target_fields': sorted(OMITTED),
                'source_only_annotation_review': 'The v3.0 prose for vehicle_docks_available[].count says available vehicles, despite its parent defining dock counts and the JSON schema defining available docks. The target leaf is omitted, so no positive relies on the inconsistent sentence. Source v2.3 docks count is unambiguous; retained vehicle_types_available count is unambiguously vehicles in both versions.',
                'label_standard': 'Same operational concept, allowing ordinary representation conversion. Different entities, quantity groups, or permission states are no-match when the equivalent target is omitted.',
                'limitations': ['Publisher-authored examples, not live observations.', 'Target is an explicit partial consumer projection; it is not claimed to be a schema-valid full GBFS feed.', 'No supported genuine ambiguity labels.', 'Author audited first-tranche results earlier; no new candidate policy or predictions accessed.']}
    for filename, data in [('case.json', case), ('evidence.json', evidence)]:
        (ROOT / filename).write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n')
    print('1 case; 16 source fields; 13 target fields; 13 matches; 3 no-match; 0 ambiguous')


if __name__ == '__main__':
    main()
