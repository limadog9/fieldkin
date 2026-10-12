#!/usr/bin/env python3
"""Rebuild blind healthcare cases/evidence from archived HL7 XML, using only stdlib.
No matcher import, network, predictions, or heuristic labeling. The projections and
labels below were chosen from publisher definitions and official conversion maps.
"""
from pathlib import Path
import hashlib,json,xml.etree.ElementTree as E
B=Path(__file__).resolve().parent
F='{http://hl7.org/fhir}'; S='{urn:schemas-microsoft-com:office:spreadsheet}'

def schema(path):
 root=E.parse(B/path).getroot(); result={}
 if root.tag==F+'StructureDefinition':
  for e in root.findall('.//'+F+'element'):
   get=lambda key: e.find(F+key).get('value') if e.find(F+key) is not None else ''
   result[get('path')]={'definition':get('definition') or get('short'),'type':'|'.join(n.get('value') for n in e.findall(F+'type/'+F+'code')),'locator':f'element[@id="{get("path")}"]'}
 else:
  for sheet in root.findall(S+'Worksheet'):
   if sheet.get(S+'Name')!='Data Elements':continue
   rows=sheet.findall(S+'Table/'+S+'Row'); headers=[]
   for rownum,row in enumerate(rows,1):
    cells=[]
    for c in row.findall(S+'Cell'):
     idx=int(c.get(S+'Index',len(cells)+1));cells.extend(['']*(idx-len(cells)-1)); cells.append(''.join(c.itertext()).strip())
    if rownum==1:headers=cells;continue
    d={h.split('        ')[0]:cells[i] if i<len(cells) else '' for i,h in enumerate(headers)}
    if not d.get('Element'):continue
    result[d['Element']]={'definition':d.get('Definition') or d.get('Short Name'),'type':d.get('Type'),'definition_source':'Definition' if d.get('Definition') else 'Short Name','short_name':d.get('Short Name'),'comments':d.get('Comments'),'workflow_mapping':d.get('Workflow Mapping'),'binding':d.get('Binding'),'locator':f'Worksheet[Data Elements]/Row[{rownum}]'}
 return {k:{**v,'path':path} for k,v in result.items()}

SC={}
for version in ['r4','r5']:
 for resource in ['Encounter','Procedure']:
  tail=resource.lower()+'-spreadsheet.xml' if version=='r4' else 'structuredefinition-'+resource+'.xml'
  SC[version,resource]=schema(f'archive/{version}/source/{resource.lower()}/{tail}')
 for typ in ['period','reference','identifier','codeableconcept','quantity']+(['codeablereference'] if version=='r5' else []):
  SC[version,typ]=schema(f'archive/{version}/source/datatypes/{typ}.xml')

def documentation(version,resource,field,parent,datatype=None,leaf=None):
 d=[{'element':resource+'.'+parent,**SC[version,resource][resource+'.'+parent]}]
 if datatype:
  key=datatype[0].upper()+datatype[1:]+'.'+leaf
  if datatype=='codeableconcept':key='CodeableConcept.'+leaf
  if datatype=='codeablereference':key='CodeableReference.'+leaf
  d.append({'element':key,**SC[version,datatype][key]})
 return d

def get_samples(version,resource,field):
 seen=[]; origins=[]
 for p in sorted((B/f'archive/{version}/source/{resource.lower()}').glob(resource.lower()+'-example*.xml')):
  root=E.parse(p).getroot()
  if root.tag!=F+resource:continue
  xpath='./'+'/'.join(F+x for x in field.split('.'))
  for elem in root.findall(xpath):
   val=elem.get('value')
   if val is None or val in seen:continue
   seen.append(val);origins.append({'value':val,'path':str(p.relative_to(B)),'xpath':resource+'/'+field.replace('.','/')+'/@value'})
   if len(seen)==3:return seen,origins
 return seen,origins

# field, resource-definition parent, native leaf type, shared datatype, leaf
ENC_SOURCE=[
 ('identifier.value','identifier','string','identifier','value'),
 ('period.start','period','dateTime','period','start'),
 ('period.end','period','dateTime','period','end'),
 ('participant.individual.reference','participant.individual','string','reference','reference'),
 ('participant.period.start','participant.period','dateTime','period','start'),
 ('participant.period.end','participant.period','dateTime','period','end'),
 ('hospitalization.preAdmissionIdentifier.value','hospitalization.preAdmissionIdentifier','string','identifier','value'),
 ('hospitalization.origin.reference','hospitalization.origin','string','reference','reference'),
 ('hospitalization.destination.reference','hospitalization.destination','string','reference','reference'),
 ('status','status','code',None,None),
 ('location.status','location.status','code',None,None),
 ('location.period.start','location.period','dateTime','period','start'),
 ('subject.reference','subject','string','reference','reference'),
 ('serviceProvider.reference','serviceProvider','string','reference','reference'),
 ('length.value','length','decimal','quantity','value'),
]
ENC_TARGET=[
 ('identifier.value','identifier','string','identifier','value'),
 ('actualPeriod.start','actualPeriod','dateTime','period','start'),
 ('actualPeriod.end','actualPeriod','dateTime','period','end'),
 ('participant.actor.reference','participant.actor','string','reference','reference'),
 ('admission.preAdmissionIdentifier.value','admission.preAdmissionIdentifier','string','identifier','value'),
 ('admission.destination.reference','admission.destination','string','reference','reference'),
 ('location.status','location.status','code',None,None),
 ('location.period.start','location.period','dateTime','period','start'),
 ('location.period.end','location.period','dateTime','period','end'),
 ('subject.reference','subject','string','reference','reference'),
 ('serviceProvider.reference','serviceProvider','string','reference','reference'),
 ('length.unit','length','string','quantity','unit'),
]
PRO_SOURCE=[
 ('performedDateTime','performed[x]','dateTime',None,None),
 ('performedPeriod.start','performed[x]','dateTime','period','start'),
 ('performedPeriod.end','performed[x]','dateTime','period','end'),
 ('reasonCode.text','reasonCode','string','codeableconcept','text'),
 ('reasonReference.reference','reasonReference','string','reference','reference'),
 ('usedCode.text','usedCode','string','codeableconcept','text'),
 ('usedReference.display','usedReference','string','reference','display'),
 ('recorder.reference','recorder','string','reference','reference'),
 ('performer.actor.reference','performer.actor','string','reference','reference'),
 ('performer.onBehalfOf.reference','performer.onBehalfOf','string','reference','reference'),
 ('subject.reference','subject','string','reference','reference'),
 ('encounter.reference','encounter','string','reference','reference'),
 ('statusReason.text','statusReason','string','codeableconcept','text'),
 ('bodySite.text','bodySite','string','codeableconcept','text'),
 ('location.display','location','string','reference','display'),
]
PRO_TARGET=[
 ('occurrenceDateTime','occurrence[x]','dateTime',None,None),
 ('occurrencePeriod.start','occurrence[x]','dateTime','period','start'),
 ('occurrencePeriod.end','occurrence[x]','dateTime','period','end'),
 ('reason.concept.text','reason','string','codeableconcept','text'),
 ('reason.reference.reference','reason','string','reference','reference'),
 ('used.concept.text','used','string','codeableconcept','text'),
 ('used.reference.display','used','string','reference','display'),
 ('reportedReference.reference','reported[x]','string','reference','reference'),
 ('performer.actor.reference','performer.actor','string','reference','reference'),
 ('subject.reference','subject','string','reference','reference'),
 ('encounter.reference','encounter','string','reference','reference'),
 ('location.display','location','string','reference','display'),
 ('recorded','recorded','dateTime',None,None),
 ('performer.period.start','performer.period','dateTime','period','start'),
 ('performer.period.end','performer.period','dateTime','period','end'),
]

# None means no equivalent exists in the declared target projection, not that
# the full R5 resource has no successor field.
ENC_MATCH={
 'identifier.value':'identifier.value','period.start':'actualPeriod.start','period.end':'actualPeriod.end',
 'participant.individual.reference':'participant.actor.reference','participant.period.start':None,'participant.period.end':None,
 'hospitalization.preAdmissionIdentifier.value':'admission.preAdmissionIdentifier.value','hospitalization.origin.reference':None,
 'hospitalization.destination.reference':'admission.destination.reference','status':None,'location.status':'location.status',
 'location.period.start':'location.period.start','subject.reference':'subject.reference','serviceProvider.reference':'serviceProvider.reference','length.value':None,
}
PRO_MATCH=dict(zip([x[0] for x in PRO_SOURCE],[
 'occurrenceDateTime','occurrencePeriod.start','occurrencePeriod.end','reason.concept.text','reason.reference.reference','used.concept.text','used.reference.display',None,'performer.actor.reference',None,'subject.reference','encounter.reference',None,None,'location.display']))
NEG={
 ('Encounter','participant.period.start'):('The start of this participant\'s involvement is distinct from the overall encounter start and location-presence start; R5 participant.period is omitted.', ['actualPeriod.start','location.period.start']),
 ('Encounter','participant.period.end'):('The end of this participant\'s involvement is distinct from the overall encounter end and location-presence end; R5 participant.period is omitted.', ['actualPeriod.end','location.period.end']),
 ('Encounter','hospitalization.origin.reference'):('Where the patient came from before admission is distinct from the discharge destination, even though the publisher examples reuse Location/2. R5 admission.origin is omitted.', ['admission.destination.reference']),
 ('Encounter','status'):('Overall encounter state is distinct from participant presence at a specific location. R5 Encounter.status is omitted.', ['location.status']),
 ('Encounter','length.value'):('Duration magnitude is distinct from the unit label or either identifier value. R5 length.value is omitted.', ['length.unit','identifier.value','admission.preAdmissionIdentifier.value']),
 ('Procedure','recorder.reference'):('Person recording and taking responsibility for the content is distinct from who performed the procedure and from the source of a secondary report. R5 recorder is omitted.', ['reportedReference.reference','performer.actor.reference']),
 ('Procedure','performer.onBehalfOf.reference'):('Organization on whose behalf the performer acted is distinct from the performer and the patient/subject. R5 performer.onBehalfOf is omitted.', ['performer.actor.reference','subject.reference']),
 ('Procedure','statusReason.text'):('Reason for the current status is distinct from the clinical reason the procedure was performed. R5 statusReason is omitted.', ['reason.concept.text']),
 ('Procedure','bodySite.text'):('Anatomical body location is distinct from the physical facility/location where a procedure happened. R5 bodySite is omitted.', ['location.display']),
}
MAPS={
 'Encounter':{
 'identifier.value':['src.identifier -> tgt.identifier;'],
 'period.start':['src.period -> tgt.actualPeriod;'], 'period.end':['src.period -> tgt.actualPeriod;'],
 'participant.individual.reference':['src.participant as s -> tgt.participant as t then EncounterParticipant(s, t);','src.individual -> tgt.actor;'],
 'hospitalization.preAdmissionIdentifier.value':['src.hospitalization as s -> tgt.admission as t then EncounterAdmission(s, t);','src.preAdmissionIdentifier -> tgt.preAdmissionIdentifier;'],
 'hospitalization.destination.reference':['src.hospitalization as s -> tgt.admission as t then EncounterAdmission(s, t);','src.destination -> tgt.destination;'],
 'location.status':['src.location as s -> tgt.location as t then EncounterLocation(s, t);',"src.status as v -> tgt.status = translate(v, 'http://hl7.org/fhir/uv/xver/ConceptMap/enc.lo.status-4to5', 'code');"],
 'location.period.start':['src.location as s -> tgt.location as t then EncounterLocation(s, t);','src.period -> tgt.period;'],
 'subject.reference':['src.subject -> tgt.subject;'],'serviceProvider.reference':['src.serviceProvider -> tgt.serviceProvider;']},
 'Procedure':{
 'performedDateTime':['src.performed : dateTime -> tgt.occurrence "performedDateTime";'],
 'performedPeriod.start':['src.performed : Period -> tgt.occurrence "performedPeriod";'],
 'performedPeriod.end':['src.performed : Period -> tgt.occurrence "performedPeriod";'],
 'reasonCode.text':['src.reasonCode -> tgt.reason;'],'reasonReference.reference':['src.reasonReference -> tgt.reason;'],
 'usedCode.text':['src.usedCode -> tgt.used;'],'usedReference.display':['src.usedReference -> tgt.used;'],
 'performer.actor.reference':['src.performer as s -> tgt.performer as t then ProcedurePerformer(s, t);','src.actor -> tgt.actor;'],
 'subject.reference':['src.subject -> tgt.subject;'],'encounter.reference':['src.encounter -> tgt.encounter;'],'location.display':['src.location -> tgt.location;']}}

def mapped_lines(path,quotes):
 lines=(B/path).read_text().splitlines();out=[]
 for quote in quotes:
  line=next(i for i,s in enumerate(lines,1) if s.strip()==quote)
  out.append({'path':path,'line':line,'quote':quote})
 return out

cases=[];evidence=[]
for resource,source_specs,target_specs,answers in [('Encounter',ENC_SOURCE,ENC_TARGET,ENC_MATCH),('Procedure',PRO_SOURCE,PRO_TARGET,PRO_MATCH)]:
 fields={};docs={};origins={}
 for role,version,specs in [('source','r4',source_specs),('target','r5',target_specs)]:
  fields[role]=[]
  for field,parent,native,datatype,leaf in specs:
   values,sample_sources=get_samples(version,resource,field);assert values,(version,resource,field)
   # Primitive normalization is explicit; the publisher's native type remains evidence.
   dtype={'dateTime':'timestamp','decimal':'decimal'}.get(native,'text')
   fields[role].append({'name':field,'data_type':dtype,'samples':values})
   docs[role,field]=documentation(version,resource,field,parent,datatype,leaf)
   if version=='r5' and parent in ['reason','used']:
    branch='concept' if '.concept.' in field else 'reference'
    docs[role,field].append({'element':'CodeableReference.'+branch,**SC['r5','codeablereference']['CodeableReference.'+branch]})
   origins[role,field]={'native_leaf_type':native,'sample_sources':sample_sources}
 case={'name':'heldout_hl7_'+resource.lower()+'_r4_r5_projection','source':{'fields':fields['source']},'target':{'fields':fields['target']},'answers':[]}
 ev={'case':case['name'],'domain':'healthcare','sample_nature':'Publisher examples, not real-world patient observations','projection_policy':'Selected primitive leaf paths retain exact FHIR wire names and semantic nesting (resource prefix omitted). Repeated elements pooled in XML order; first 3 distinct literal @value samples from lexicographically sorted archived publisher example filenames. No fabricated values. Target projection is intentionally partial. no_match means no equivalent within this projection, not no migration path in full FHIR R5. All selected R4/R5 resource semantics checked against definitions and official conversion maps before prediction exposure.','fields':[],'target_fields':[]}
 for spec in target_specs:
  field=spec[0];ev['target_fields'].append({'field':field,'documentation':docs['target',field],**origins['target',field]})
 for spec in source_specs:
  field=spec[0];target=answers[field]
  answer={'kind':'match','source':field,'target':target} if target else {'kind':'no_match','source':field}
  case['answers'].append(answer)
  row={'field':field,'answer':answer,'source_documentation':docs['source',field],**origins['source',field]}
  if target:
   row['target_documentation']=docs['target',target]
   row['mapping_evidence']=mapped_lines('archive/xver/input/R4toR5/'+resource+'4to5.fml',MAPS[resource][field])
   if field in ['reasonCode.text','usedCode.text']:
    row['mapping_evidence']+=mapped_lines('archive/xver/input/R4toR5/CodeableConcept4to5.fml',['src as s -> tgt.concept as t then CodeableConcept(s, t) "CodeableConcept2CodeableReference";','src.text -> tgt.text;'])
   if field in ['reasonReference.reference','usedReference.display']:
    path='archive/xver/input/R4toR5/Reference4to5.fml'
    lines=(B/path).read_text().splitlines()
    row['mapping_evidence'] += [{'path':path,'line':i,'quote':s.strip()} for i,s in enumerate(lines,1) if 'tgt.reference' in s or 'src.display' in s]
   row['reason']='Official HL7 cross-version map explicitly preserves this concept; both schema definitions and shared datatype leaf definitions support the pairing. It is a field-concept equivalence, not a claim that every possible value requires no conversion.'
  else:
   reason,decoys=NEG[resource,field];row['reason']=reason
   row['contrasting_target_definitions']=[{'field':t,'documentation':docs['target',t]} for t in decoys]
   row['all_target_fields_reviewed']=[s[0] for s in target_specs]
   omitted={
    'participant.period.start':'participant.period','participant.period.end':'participant.period',
    'hospitalization.origin.reference':'admission.origin','status':'status','length.value':'length',
    'recorder.reference':'recorder','performer.onBehalfOf.reference':'performer.onBehalfOf',
    'statusReason.text':'statusReason','bodySite.text':'bodySite',
   }[field]
   row['omitted_equivalent_parent_definition']={'element':resource+'.'+omitted,**SC['r5',resource][resource+'.'+omitted]}
  ev['fields'].append(row)
 assert len(case['answers'])==len(case['source']['fields'])
 assert len({f['name'] for f in case['source']['fields']})==len(case['source']['fields'])
 cases.append(case);evidence.append(ev)
(B/'cases.json').write_text(json.dumps(cases,indent=2)+'\n')
(B/'evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
print('Built',len(cases),'cases;',sum(len(c['source']['fields']) for c in cases),'source fields;',sum(len(c['target']['fields']) for c in cases),'target fields')
print('Labels:',{kind:sum(a['kind']==kind for c in cases for a in c['answers']) for kind in ['match','no_match','ambiguous']})
