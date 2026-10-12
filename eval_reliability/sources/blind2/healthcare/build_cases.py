#!/usr/bin/env python3
"""Build one blind, source-annotated healthcare case offline using only stdlib.
Only cases.json and evidence.json are written. No matcher imports or predictions.
"""
from pathlib import Path
import json,re,xml.etree.ElementTree as E
B=Path(__file__).resolve().parent;F='{http://hl7.org/fhir}';S='{urn:schemas-microsoft-com:office:spreadsheet}'
R='MedicationRequest'

def schema(path):
 root=E.parse(B/path).getroot();out={}
 if root.tag==F+'StructureDefinition':
  for e in root.findall('.//'+F+'element'):
   get=lambda k:e.find(F+k).get('value') if e.find(F+k) is not None else ''
   out[get('path')]={'definition':get('definition') or get('short'),'type':'|'.join(x.get('value') for x in e.findall(F+'type/'+F+'code')),'locator':f'element[@id="{get("path")}"]'}
 else:
  for sheet in root.findall(S+'Worksheet'):
   if sheet.get(S+'Name')!='Data Elements':continue
   for n,row in enumerate(sheet.findall(S+'Table/'+S+'Row'),1):
    cells=[]
    for c in row.findall(S+'Cell'):
     i=int(c.get(S+'Index',len(cells)+1));cells.extend(['']*(i-len(cells)-1));cells.append(''.join(c.itertext()).strip())
    if n==1:
     # Some publisher spreadsheets use adjacent rich-text font runs, so
     # itertext() joins 'Type' + 'Data type...' without separating spaces.
     keys=['Short Name','Definition','Comments','Element','Type']
     headers=[next((k for k in keys if x.startswith(k)),x.split('        ')[0]) for x in cells]
     continue
    d={h:cells[i] if i<len(cells) else '' for i,h in enumerate(headers)}
    if not d.get('Element'):continue
    out[d['Element']]={'definition':d.get('Definition') or d.get('Short Name'),'type':d.get('Type'),'definition_source':'Definition' if d.get('Definition') else 'Short Name','comments':d.get('Comments'),'locator':f'Worksheet[Data Elements]/Row[{n}]'}
 return {k:{'element':k,'path':path,**v} for k,v in out.items()}

SC={}
for v in ['r4','r5']:
 d={}
 for p in sorted((B/f'archive/{v}/source/datatypes').glob('*.xml')):d.update(schema(str(p.relative_to(B))))
 name='medicationrequest-spreadsheet.xml' if v=='r4' else 'structuredefinition-MedicationRequest.xml'
 d.update(schema(f'archive/{v}/source/medicationrequest/{name}'));SC[v]=d

def docs(v,keys):
 result=[SC[v][k] for k in keys]
 assert all(d['definition'] for d in result),keys
 return result

def samples(v,field):
 values=[];origins=[]
 for p in sorted((B/f'archive/{v}/source/medicationrequest').glob('medicationrequest*.xml')):
  if not re.fullmatch(r'medicationrequest\d+[a-z]?\.xml',p.name):continue
  root=E.parse(p).getroot();assert root.tag==F+R
  for elem in root.findall('./'+'/'.join(F+x for x in field.split('.'))):
   val=elem.get('value')
   if val is None or val in values:continue
   values.append(val);origins.append({'value':val,'path':str(p.relative_to(B)),'xpath':R+'/'+field.replace('.','/')+'/@value'})
   if len(values)==3:return values,origins
 assert values,(v,field)
 return values,origins

# Schema-selected projection. Source path, native primitive type, target path or
# None for an absent semantic concept. Every no_match has a documented contrast.
ROWS=[
 ('medicationReference.reference','string','medication.reference.reference'),
 ('medicationReference.display','string','medication.reference.display'),
 ('medicationCodeableConcept.coding.code','code','medication.concept.coding.code'),
 ('dosageInstruction.asNeededBoolean','boolean','dosageInstruction.asNeeded'),
 ('dispenseRequest.validityPeriod.start','dateTime','dispenseRequest.validityPeriod.start'),
 ('dispenseRequest.validityPeriod.end','dateTime','dispenseRequest.validityPeriod.end'),
 ('dispenseRequest.numberOfRepeatsAllowed','unsignedInt','dispenseRequest.numberOfRepeatsAllowed'),
 ('dosageInstruction.patientInstruction','string','dosageInstruction.patientInstruction'),
 ('dosageInstruction.text','string','dosageInstruction.text'),
 ('dosageInstruction.sequence','integer','dosageInstruction.sequence'),
 ('dosageInstruction.timing.repeat.frequency','positiveInt','dosageInstruction.timing.repeat.frequency'),
 ('dosageInstruction.timing.repeat.period','decimal','dosageInstruction.timing.repeat.period'),
 ('dosageInstruction.timing.repeat.periodUnit','code','dosageInstruction.timing.repeat.periodUnit'),
 ('substitution.allowedBoolean','boolean','substitution.allowedBoolean'),
 ('performer.reference','string','performer.reference'),
 ('recorder.reference','string',None),
 ('requester.reference','string',None),
 ('dispenseRequest.quantity.value','decimal',None),
 ('dispenseRequest.quantity.unit','string',None),
 ('dosageInstruction.maxDosePerLifetime.value','decimal',None),
 ('dosageInstruction.maxDosePerAdministration.value','decimal','dosageInstruction.maxDosePerAdministration.value'),
 ('authoredOn','dateTime',None),
 ('dosageInstruction.timing.event','dateTime',None),
 ('dosageInstruction.doseAndRate.doseQuantity.value','decimal','dosageInstruction.doseAndRate.doseQuantity.value'),
]
TARGET_EXTRA=[('dosageInstruction.doseAndRate.doseQuantity.unit','string')]

def definition_keys(v,p):
 if p.startswith('medication'):
  out=[R+('.medication[x]' if v=='r4' else '.medication')]
  if v=='r5':out+=['CodeableReference.concept' if '.concept.' in p else 'CodeableReference.reference']
  out+=['CodeableConcept.coding','Coding.code'] if p.endswith('.coding.code') else ['Reference.'+p.split('.')[-1]]
  return out
 if p.startswith('dosageInstruction.'):
  out=[R+'.dosageInstruction'];tail=p.split('.',1)[1]
  if tail=='asNeededBoolean':return out+['Dosage.asNeeded[x]']
  if tail.startswith('timing.'):return out+['Dosage.timing','Timing.'+tail.split('.',1)[1]]
  if tail.startswith('doseAndRate.doseQuantity.'):
   return out+['Dosage.doseAndRate.dose[x]','Quantity.'+tail.split('.')[-1]]
  if tail.startswith('maxDosePer'):
   return out+['Dosage.'+tail.rsplit('.',1)[0],'Quantity.'+tail.split('.')[-1]]
  return out+['Dosage.'+tail]
 if p.startswith('dispenseRequest.validityPeriod.'):
  return [R+'.dispenseRequest.validityPeriod','Period.'+p.split('.')[-1]]
 if p.startswith('dispenseRequest.quantity.'):
  return [R+'.dispenseRequest.quantity','Quantity.'+p.split('.')[-1]]
 if p=='substitution.allowedBoolean':return [R+'.substitution.allowed[x]']
 if p.endswith('.reference'):return [R+'.'+p.rsplit('.',1)[0],'Reference.reference']
 return [R+'.'+p]

NEG={
 'recorder.reference':('The person entering the order on behalf of another is distinct from the desired administrator of the medication. The target projection omits recorder.', ['performer.reference']),
 'requester.reference':('The initiator responsible for activating the request is distinct from the desired medication administrator. The target projection omits requester.', ['performer.reference']),
 'dispenseRequest.quantity.value':('Quantity supplied per fill is distinct from quantity administered per dose, even when both are measured in tablets or mL. The target projection omits dispense quantity.', ['dosageInstruction.doseAndRate.doseQuantity.value']),
 'dispenseRequest.quantity.unit':('The unit of the dispense quantity is scoped to a different measured amount from the unit of the per-dose quantity; no universal equality is asserted. The target projection omits dispense quantity.', ['dosageInstruction.doseAndRate.doseQuantity.unit']),
 'dosageInstruction.maxDosePerLifetime.value':('Maximum cumulative medication over a lifetime is distinct from the maximum per administration. The target projection omits the lifetime maximum.', ['dosageInstruction.maxDosePerAdministration.value']),
 'authoredOn':('When the prescription was written differs from the start or end of its authorization-to-dispense window, including when samples happen to coincide. The target projection omits authoredOn.', ['dispenseRequest.validityPeriod.start','dispenseRequest.validityPeriod.end']),
 'dosageInstruction.timing.event':('Specific times when medication is to be taken differ from the prescription validity window. The target projection omits the dose-event schedule.', ['dispenseRequest.validityPeriod.start','dispenseRequest.validityPeriod.end']),
}

def map_quotes(p):
 mr='MedicationRequest';d='Dosage'
 if p=='medicationCodeableConcept.coding.code':
  return [(mr,'src.medication : CodeableConcept -> tgt.medication "medicationCodeableConcept";'),('CodeableConcept','src as s -> tgt.concept as t then CodeableConcept(s, t) "CodeableConcept2CodeableReference";'),('CodeableConcept','src.coding -> tgt.coding;'),('Coding','src.code -> tgt.code;')]
 if p.startswith('medicationReference.'):
  leaf=p.split('.')[-1]
  return [(mr,'src.medication : Reference -> tgt.medication "medicationReference";'),('Reference','src.'+leaf+' as v ->  tgt.reference as t,  t.'+leaf+' = v;'),('Reference','src.'+leaf+' -> tgt.'+leaf+';')]
 if p.startswith('dosageInstruction.'):
  result=[(mr,'src.dosageInstruction -> tgt.dosageInstruction;')];tail=p.split('.',1)[1]
  if tail=='asNeededBoolean':return result+[(d,'src.asNeeded : boolean -> tgt.asNeeded "asNeededBoolean";')]
  if tail.startswith('timing.'):
   return result+[(d,'src.timing -> tgt.timing;'),('Timing','src.'+tail.split('.')[-1]+' -> tgt.'+tail.split('.')[-1]+';')]
  if tail.startswith('doseAndRate.'):
   return result+[(d,'src.doseAndRate as s -> tgt.doseAndRate as t then DosageDoseAndRate(s, t);'),(d,'src.dose : Quantity -> tgt.dose "doseQuantity";')]
  key=tail.split('.')[0];return result+[(d,f'src.{key} -> tgt.{key};')]
 if p.startswith('dispenseRequest.'):
  key=p.split('.')[1]
  return [(mr,'src.dispenseRequest as s -> tgt.dispenseRequest as t then MedicationRequestDispenseRequest(s, t);'),(mr,f'src.{key} -> tgt.{key};')]
 if p=='substitution.allowedBoolean':
  return [(mr,'src.substitution as s -> tgt.substitution as t then MedicationRequestSubstitution(s, t);'),(mr,'src.allowed : boolean -> tgt.allowed "allowedBoolean";')]
 return [(mr,'src.performer -> tgt.performer;')]

def map_evidence(p):
 out=[]
 for resource,quote in map_quotes(p):
  path='archive/xver/input/R4toR5/'+resource+'4to5.fml';lines=(B/path).read_text().splitlines()
  line=next(i for i,s in enumerate(lines,1) if s.strip()==quote)
  out.append({'path':path,'line':line,'quote':quote})
 return out

NORM={'dateTime':'timestamp','unsignedInt':'integer','integer':'integer','positiveInt':'integer','decimal':'decimal','boolean':'boolean','string':'text','code':'text'}
case={'name':'heldout2_hl7_medicationrequest_r4_r5_projection','source':{'fields':[]},'target':{'fields':[]},'answers':[]}
evidence={'case':case['name'],'domain':'healthcare','sample_nature':'Publisher illustrative examples; not empirical clinical records','projection':'Exact primitive wire paths, with resource prefix omitted. Include only numbered publisher examples, pool repeated leaves in XML order, retain first three distinct literal values by lexicographically sorted filename. No match means absent in the explicitly partial target projection. No aggregation or changed measured population is equated.','target_fields':[],'fields':[]}
target_specs=[(t,n) for s,n,t in ROWS if t]+TARGET_EXTRA
assert len({t for t,n in target_specs})==len(target_specs)
for role,v,specs in [('source','r4',[(s,n) for s,n,t in ROWS]),('target','r5',target_specs)]:
 for field,native in specs:
  values,origins=samples(v,field);documentation=docs(v,definition_keys(v,field))
  case[role]['fields'].append({'name':field,'data_type':NORM[native],'samples':values})
  row={'field':field,'native_leaf_type':native,'documentation':documentation,'sample_sources':origins}
  if role=='source':
   target=next(t for s,n,t in ROWS if s==field)
   answer={'kind':'match','source':field,'target':target} if target else {'kind':'no_match','source':field}
   case['answers'].append(answer);row['answer']=answer
   if target:
    row['target_documentation']=docs('r5',definition_keys('r5',target));row['mapping_evidence']=map_evidence(field)
    row['reason']='Official migration explicitly preserves this field concept; schema/type definitions preserve its role and measured quantity. This is a directional R4-to-R5 mapping, not a claim of universal bidirectional value-domain equality.'
   else:
    reason,decoys=NEG[field];row['reason']=reason
    row['contrasting_target_definitions']=[{'field':p,'documentation':docs('r5',definition_keys('r5',p))} for p in decoys]
    row['omitted_equivalent_definition']=docs('r5',definition_keys('r5',field))
    row['all_target_fields_reviewed']=[p for p,n in target_specs]
   evidence['fields'].append(row)
  else:evidence['target_fields'].append(row)
(B/'cases.json').write_text(json.dumps([case],indent=2)+'\n')
(B/'evidence.json').write_text(json.dumps([evidence],indent=2)+'\n')
print('Built one case with',len(ROWS),'source fields and',len(target_specs),'target fields.')
