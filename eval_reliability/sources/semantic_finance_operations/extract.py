#!/usr/bin/env python3
"""Rebuild held-out cases, independent publisher context and evidence offline."""
import csv, hashlib, json, re
from pathlib import Path

ROOT = Path(__file__).resolve().parent
RAW = ROOT / 'rawsources'

def readj(path):
    return json.loads((ROOT / path).read_text())

def writej(path, value):
    (ROOT / path).write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n')

def section_map(filename):
    text = (RAW / filename).read_text()
    result = {}
    for match in re.finditer(r'^## ([^\n]+)\n(.*?)(?=^## |\Z)', text, re.M | re.S):
        result[match.group(1).strip().lower()] = match.group(2).strip()
    return result

def hmda_dictionary(year):
    if year == 2017:
        filename = 'hmda-2017_Modified_LAR_Data_Dictionary.csv'
        rows = list(csv.DictReader((RAW / filename).open()))
        return {r['Data Field Name']: {'declared_type':r['Data Field Type'], 'valid_values':r['Valid Values'], 'notes':r['Notes'], 'citation':'rawsources/'+filename+'#'+r['Data Field Name']} for r in rows}
    filename = 'hmda-2018_Modified_LAR_Data_Dictionary.md'
    result = {}
    for line in (RAW / filename).read_text().splitlines():
        if '|' not in line or line.startswith(('Field Name','|----')):
            continue
        pieces = line.split('|')
        if len(pieces) >= 4:
            name, typ, values, notes = pieces[:4]
            result[name] = {'declared_type':typ, 'valid_values':values, 'notes':notes, 'citation':'rawsources/'+filename+'#'+name}
    return result


def hmda_specs():
    result = {}
    filename = 'hmda-2017_File_Spec_LAR.csv'
    for row in csv.reader((RAW / filename).open()):
        if len(row) >= 5 and row[0].strip().isdigit():
            result[row[1].strip().lower()] = {'description':row[4].strip(), 'citation':'rawsources/'+filename+'#field-'+row[0].strip()}
    return result


def hmda_side(year, names):
    dic = hmda_dictionary(year)
    cs_file = f'hmda-{year}_Modified_LAR_Codesheet.md'
    sections = section_map(cs_file)
    specs = hmda_specs() if year == 2017 else {}
    fields, context_fields = [], []
    aliases = {
        'Co-applicant Sex':'Co-Applicant Sex',
        'Applicant Race: 1':'Applicant Race',
        'Co-Applicant Race: 1':'Co-Applicant Race',
        'Denial Reason: 1':'Denial Reason',
        'Ethnicity of Applicant or Borrower: 1':'Ethnicity of Applicant or Borrower',
        'Ethnicity of Co-Applicant or Co-Borrower: 1':'Ethnicity of Co-Applicant or Co-Borrower',
        'Race of Applicant or Borrower: 1':'Race of Applicant or Borrower',
        'Race of Co-Applicant or Co-Borrower: 1':'Race of Co-Applicant or Co-Borrower',
        'Reason for Denial: 1':'Reason for Denial',
    }
    for name in names:
        item = dic[name]
        typ = {'Integer':'integer','Numeric':'decimal','Character Varying':'text','Alphanumeric':'text'}[item['declared_type']]
        heading = aliases.get(name, name)
        section = sections.get(heading.lower(), '')
        spec = specs.get(name.lower(), {})
        descriptions = []
        citations = [item['citation']]
        if spec.get('description'):
            descriptions.append(spec['description'])
            citations.append(spec['citation'])
        if section:
            descriptions.append(section)
            citations.append('rawsources/'+cs_file+'#'+heading)
        if item['notes']:
            descriptions.append(item['notes'])
        samples, sample_refs = [], []
        # Only explicit examples, never values inferred from valid-value lists.
        if year == 2017 and name == 'Loan Amount':
            assert 'Example: 111' in spec['description']
            samples = ['111']; sample_refs = [spec['citation']]
        if year == 2017 and name == 'Metropolitan Statistical Area/Metropolitan Division':
            assert 'Example: 40900' in spec['description']
            samples = ['40900']; sample_refs = [spec['citation']]
        if year == 2017 and name == 'Rate Spread':
            assert 'Example: 03.29' in spec['description']
            samples = ['03.29']; sample_refs = [spec['citation']]
        if year == 2018 and name == 'Loan Amount':
            assert 'disclosed as $115000' in item['notes']
            samples = ['115000']; sample_refs = [item['citation']]
        fields.append({'name':name,'data_type':typ,'samples':samples})
        unit_statements = []
        if year == 2017 and name == 'Loan Amount':
            unit_statements = [spec['description']]
        if year == 2018 and name in ['Loan Amount','Property Value']:
            unit_statements = [item['notes']]
        context_fields.append({'name':name, 'path':name, 'table_name':f'{year} Modified LAR', 'declared_type':item['declared_type'], 'description':'\n\n'.join(descriptions), 'publisher_field_heading':name, 'code_description_heading':heading if section else None, 'valid_values_raw':item['valid_values'], 'notes_raw':item['notes'], 'units_raw':unit_statements, 'periods_raw':[], 'roles_raw':[heading] if any(x in heading.lower() for x in ['applicant','borrower']) else [], 'description_citations':citations, 'sample_kind':'publisher_documented_example' if samples else 'schema_only', 'sample_citations':sample_refs})
    return {'fields':fields}, {'publisher':'Consumer Financial Protection Bureau', 'resource_name':f'{year} Modified LAR', 'schema_title':f'{year} Modified LAR Data Dictionary', 'schema_version':str(year), 'fields':context_fields}


def adyen_side(filename, names):
    raw = readj('rawsources/'+filename)
    schema = raw['components']['schemas']['PaymentRequest']
    fields, context_fields = [], []
    for name in names:
        prop = schema['properties'][name]
        assert prop.get('type') in ['string','integer','number','boolean']
        typ = {'string':'text','integer':'integer','number':'decimal','boolean':'boolean'}[prop['type']]
        if prop.get('format') == 'date': typ = 'date'
        if prop.get('format') == 'date-time': typ = 'timestamp'
        desc = prop.get('description','')
        samples=[]
        if name in ['deliverAt','deliveryDate']:
            example = '2017-07-17T13:42:40.428+01:00'
            assert 'Example: '+example in desc
            samples = [example]
        if name == 'countryCode':
            assert 'Example: NL or DE' in desc
            samples = ['NL','DE']
        if name == 'shopperLocale' and 'For example, **nl-NL**.' in desc:
            samples = ['nl-NL']
        fields.append({'name':name,'data_type':typ,'samples':samples})
        ptr = '/components/schemas/PaymentRequest/properties/'+name
        citation = 'rawsources/'+filename+'#'+ptr
        # Metadata is extracted from this property only, independently of labels.
        context_fields.append({'name':name, 'path':ptr, 'resource_name':'PaymentRequest', 'api_title':raw['info']['title'], 'declared_type':prop['type'], 'declared_format':prop.get('format'), 'description':desc, 'publisher_field_heading':name, 'units_raw':[desc] if name == 'captureDelayHours' else [], 'periods_raw':[], 'roles_raw':[desc.split('\n\n')[0]] if any(x in desc.lower() for x in ['shopper','merchant','payment','transaction']) else [], 'description_citations':[citation], 'publisher_extensions':{k:v for k,v in prop.items() if k.startswith('x-') or k == 'deprecated'}, 'sample_kind':'publisher_documented_example' if samples else 'schema_only', 'sample_citations':[citation] if samples else []})
    return {'fields':fields}, {'publisher':'Adyen', 'resource_name':'PaymentRequest', 'api_title':raw['info']['title'], 'schema_version':raw['info']['version'], 'projection':'Selected documented top-level scalar properties; arrays and object-valued properties are excluded.', 'fields':context_fields}


def main():
    selection = readj('SELECTION.json')
    annotations = readj('ANNOTATIONS.json')
    cases, context, evidence = [], [], []
    for cfg in selection:
        if cfg['publisher'] == 'cfpb':
            src, srcctx = hmda_side(2017,cfg['source_fields'])
            tgt, tgtctx = hmda_side(2018,cfg['target_fields'])
        else:
            src, srcctx = adyen_side('adyen-PaymentService-v52.excerpt.json',cfg['source_fields'])
            tgt, tgtctx = adyen_side('adyen-CheckoutService-v72.excerpt.json',cfg['target_fields'])
        anns = annotations[cfg['name']]
        assert [x['source'] for x in anns] == cfg['source_fields']
        target_set = set(cfg['target_fields'])
        answers=[]
        ev=[]
        for ann in anns:
            answer={k:ann[k] for k in ['source','kind','target','targets'] if k in ann}
            assert answer['kind'] in ['match','no_match','ambiguous']
            references = [answer['target']] if 'target' in answer else answer.get('targets',[])
            assert all(x in target_set for x in references)
            answers.append(answer)
            sfield=next(x for x in srcctx['fields'] if x['name']==ann['source'])
            contrast_names=[x['target'] for x in ann.get('negative_contrasts',[])]
            tfields=[x for x in tgtctx['fields'] if x['name'] in references+contrast_names]
            ev.append({'source':ann['source'],'answer':answer,'rationale':ann['rationale'],'source_evidence':{'description':sfield['description'],'citations':sfield['description_citations']},'target_evidence':[{'field':x['name'],'description':x['description'],'citations':x['description_citations'],'publisher_extensions':x.get('publisher_extensions',{})} for x in tfields], 'absence_scope':ann.get('absence_scope'), 'negative_contrasts':ann.get('negative_contrasts',[])})
        cases.append({'name':cfg['name'],'source':src,'target':tgt,'answers':answers})
        context.append({'name':cfg['name'],'source':srcctx,'target':tgtctx})
        evidence.append({'name':cfg['name'],'labels':ev,'excluded_fields_and_limitations':cfg['limitations']})
    writej('cases.json',cases)
    writej('context.json',context)
    writej('evidence.json',evidence)
    print(json.dumps({'cases':len(cases),'source_fields':[len(x['source']['fields']) for x in cases],'target_fields':[len(x['target']['fields']) for x in cases],'answers':[len(x['answers']) for x in cases]}))

if __name__ == '__main__': main()
