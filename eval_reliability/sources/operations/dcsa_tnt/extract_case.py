#!/usr/bin/env python3
"""Rebuild the pinned DCSA operational schema projection (Python standard library only).

This does not parse arbitrary YAML. It scans named, indentation-delimited scalar
schema nodes in the two archived OpenAPI documents and refuses missing/complex
types. Full raw publisher definition blocks and line numbers are emitted.
"""
import datetime, hashlib, json, pathlib, re
BASE = pathlib.Path(__file__).resolve().parent
PFX = 'tnt__src__main__resources__standards__tnt__'
OLD = PFX + 'schemas__TNT_v2.2.0-resolved.yaml'
NEW = PFX + 'schemas__TNT_v3.0.0.yaml'
TRANSPORT = 'transportDetails.transportCall.'
# (source original key path, old scalar schema, target original key path,
#  new object schema, new property, correspondence justification)
FIELDS = [
('eventID','eventID','eventID','Event','eventID','Both identify the published Track and Trace event; v2 enforces UUID while v3 permits a producer-scoped string.'),
('eventCreatedDateTime','eventCreatedDateTime',None,None,None,'Creation of the technical event is not its last update or the real-world occurrence time. No creation-time property exists in the target projection.'),
('eventDateTime','eventDateTime','eventDateTime','Event','eventDateTime','Both give the occurrence or expected occurrence time of the real-world event.'),
('eventType','eventType','eventClassification.eventType','EventClassification','eventType','Both discriminate the event category; v3 expands supported categories.'),
('eventClassifierCode','eventClassifierCode','eventClassification.eventClassifier','EventClassification','eventClassifier','Both distinguish actual, planned and estimated events; v3 expands the code spelling. The v2 baseEquipmentEvent enum supplies ACT/EST/PLN.'),
('equipmentEventTypeCode','equipmentEventTypeCode','eventClassification.equipmentEventType','EventClassification','equipmentEventType','Both distinguish equipment events, with documented enum continuity such as LOAD (Loaded) and LOADED - Loaded.'),
('equipmentReference','equipmentReference','equipmentDetails.equipmentReference','EquipmentDetails','equipmentReference','Both are the BIC ISO 6346 container identifier where available.'),
('ISOEquipmentCode','ISOEquipmentCode','equipmentDetails.ISOEquipmentCode','EquipmentDetails','ISOEquipmentCode','Both code equipment size and type under ISO 6346.'),
('emptyIndicatorCode','emptyIndicatorCode','equipmentDetails.emptyIndicator','EquipmentDetails','emptyIndicator','Both distinguish empty and laden equipment.'),
('eventLocation.locationName','locationName','eventLocation.locationName','Location','locationName','Both name the event location.'),
('eventLocation.latitude','latitude','eventLocation.geoCoordinate.latitude','GeoCoordinate','latitude','Both express the north/south coordinate of the event location.'),
('eventLocation.longitude','longitude','eventLocation.geoCoordinate.longitude','GeoCoordinate','longitude','Both express the east/west coordinate of the event location.'),
('eventLocation.UNLocationCode','UNLocationCode','eventLocation.UNLocationCode','Location','UNLocationCode','Both identify the event location by UN/LOCODE.'),
('eventLocation.facilityCode','facilityCode','eventLocation.facility.facilityCode','Facility','facilityCode','Both identify the specific event facility under a facility code list.'),
('eventLocation.facilityCodeListProvider','facilityCodeListProvider','eventLocation.facility.facilityCodeListProvider','Facility','facilityCodeListProvider','Both identify the provider defining the event facility code.'),
('eventLocation.address.name','addressName',None,None,None,'The old address name (publisher example Henrik) has no corresponding Address property in v3. A location name is a distinct field retained separately.'),
('eventLocation.address.street','streetName','eventLocation.address.street','Address','street','Both name the street of the event location address.'),
('eventLocation.address.streetNumber','streetNumber','eventLocation.address.streetNumber','Address','streetNumber','Both identify the street number of the event location address.'),
('eventLocation.address.postCode','postCode','eventLocation.address.postCode','Address','postCode','Both give the post code of the event location address.'),
('eventLocation.address.city','cityName','eventLocation.address.city','Address','city','Both name the city of the event location address.'),
('transportCall.transportCallID','transportCallID',TRANSPORT+'transportCallReference','TransportCall','transportCallReference','Both identify the carrier transport call. V3 explicitly describes a carrier-defined reference, including terminal call reference for a vessel at a port/terminal.'),
('transportCall.carrierServiceCode','carrierServiceCode',TRANSPORT+'serviceCodeOrReference.carrierServiceCode','ServiceCodeOrReference','carrierServiceCode','Both identify the carrier service using the carrier-specific code, as distinct from a universal service reference.'),
('transportCall.exportVoyageNumber','exportVoyageNumber',TRANSPORT+'exportVoyageNumberOrReference.carrierVoyageNumber','VoyageNumberOrReference','carrierVoyageNumber','Both are the carrier-specific export voyage identifier. The export parent preserves direction; a universal voyage reference is a separate identifier.'),
('transportCall.importVoyageNumber','importVoyageNumber',TRANSPORT+'importVoyageNumberOrReference.carrierVoyageNumber','VoyageNumberOrReference','carrierVoyageNumber','Both are the carrier-specific import voyage identifier. The import parent preserves direction.'),
('transportCall.transportCallSequenceNumber','transportCallSequenceNumber',TRANSPORT+'transportCallSequenceNumber','TransportCall','transportCallSequenceNumber','Both distinguish individual transport calls, including repeated calls at one location within a voyage.'),
('transportCall.modeOfTransport','modeOfTransport',TRANSPORT+'modeOfTransport','TransportCall','modeOfTransport','Both name the mode of transport for this call.'),
('transportCall.vessel.vesselIMONumber','vesselIMONumber',TRANSPORT+'vesselTransport.vesselIMONumber','VesselTransport','vesselIMONumber','Both are the permanent IMO identifier of the vessel; an MMSI is a distinct identifier.'),
('transportCall.vessel.vesselCallSignNumber','vesselCallSignNumber',TRANSPORT+'vesselTransport.vesselCallSign','VesselTransport','vesselCallSign','Both are the vessel alphanumeric call sign assigned by the ITU.'),
('transportCall.vessel.vesselOperatorCarrierCode','vesselOperatorCarrierCode',TRANSPORT+'vesselTransport.operatorCarrierCode','VesselTransport','operatorCarrierCode','Both code the carrier responsible for vessel operation using an associated provider.'),
('transportCall.vessel.vesselOperatorCarrierCodeListProvider','vesselOperatorCarrierCodeListProvider',TRANSPORT+'vesselTransport.operatorCarrierCodeListProvider','VesselTransport','operatorCarrierCodeListProvider','Both define the code list used for the vessel operator carrier code (SMDG or NMFTA).'),
('seals[].sealNumber','sealNumber','equipmentDetails.seals[].number','Seal','number','Both identify a seal affixed to the container.'),
('seals[].sealSource','sealSource','equipmentDetails.seals[].source','Seal','source','Both identify who affixed the seal; v3 expands enum spelling, including CUS (Customs) to CUSTOMS.'),
('seals[].sealType','sealType','equipmentDetails.seals[].type','Seal','type','Both identify the seal type; v3 expands enum spelling, including WIR (Wire) to WIRE.'),
('transportCall.otherFacility','otherFacility',None,None,None,'This is free text for the facility of the transport call. V3 TransportCall has no location/facility property; the projected eventLocation is the separately modeled equipment event location, not the old transport-call facility.'),
]
EXTRA_TARGETS = [
('eventUpdatedDateTime','Event','eventUpdatedDateTime','Hard negative for creation/occurrence time: this is last technical update time.'),
('isRetracted','Event','isRetracted','New event retraction flag.'),
(TRANSPORT+'serviceCodeOrReference.universalServiceReference','ServiceCodeOrReference','universalServiceReference','Universal service reference is distinct from the carrier service code.'),
(TRANSPORT+'exportVoyageNumberOrReference.universalVoyageReference','VoyageNumberOrReference','universalVoyageReference','Universal export voyage reference is distinct from the carrier voyage number.'),
(TRANSPORT+'vesselTransport.vesselMMSINumber','VesselTransport','vesselMMSINumber','MMSI maritime radio/AIS identity is distinct from vessel IMO identity and ITU call sign.'),
]

def block(lines, key, indent, start=0, stop=None):
    stop = len(lines) if stop is None else stop
    pattern = ' '*indent + key + ':'
    found = [i for i in range(start, stop) if lines[i].rstrip() == pattern]
    if len(found) != 1:
        raise ValueError((key, indent, found))
    begin = found[0]
    end = begin + 1
    while end < stop:
        s=lines[end]
        if s.strip() and len(s)-len(s.lstrip()) <= indent:
            break
        end += 1
    return begin,end

def scalar(value):
    value=value.strip()
    if value.startswith('"'):
        return json.loads(value)
    if value.startswith("'") and value.endswith("'"):
        return value[1:-1].replace("''", "'")
    if value in ('true','false','null'):
        return json.loads(value)
    # Keep unquoted publisher strings as strings. Numeric conversion follows the
    # explicit declared scalar type when samples are emitted.
    return value

def definition(filename, schema, prop=None):
    lines=(BASE/filename).read_text().splitlines()
    sa,sb=block(lines,'schemas',2)
    a,b=block(lines,schema,4,sa,sb)
    if prop:
        a,b=block(lines,prop,8,a,b)
    indent=10 if prop else 6
    direct={}
    for line in lines[a+1:b]:
        m=re.match(r'^'+' '*indent+r'([A-Za-z]+): (.*)$',line)
        if m:
            direct[m[1]]=m[2]
    declared=scalar(direct['type'])
    data_type={'string':'text','integer':'integer','number':'float','boolean':'boolean'}[declared]
    if direct.get('format') and scalar(direct['format']) in ('date','date-time'):
        data_type={'date':'date','date-time':'timestamp'}[scalar(direct['format'])]
    example=scalar(direct['example']) if 'example' in direct else None
    return dict(schema_file=filename, schema_component=schema, property=prop,
        line_start=a+1,line_end=b,declared_type=declared,
        declared_format=scalar(direct['format']) if 'format' in direct else None,
        data_type=data_type,definition='\n'.join(lines[a:b]),schema_example=example)

def values_at(value,path):
    parts=path.split('.')
    values=[value]
    for part in parts:
        array=part.endswith('[]');key=part[:-2] if array else part
        following=[]
        for v in values:
            if not isinstance(v,dict) or key not in v:
                continue
            child=v[key]
            following.extend(child if array else [child])
        values=following
    return values

def load_records(version):
    # Use success fixtures only. Never use publisher deliberately invalid fixtures.
    names=([PFX+'messages__tnt-220-response.json'] if version=='old' else
           [PFX+'messages__tnt-300-response.json',PFX+'messages__tnt-300-response-nextpage.json']+
           [PFX+'messages__tnt-300-request-'+s+'.json' for s in ['equipment','transport','shipment','iot','reefer']])
    out=[]
    for name in names:
        doc=json.loads((BASE/name).read_text())
        records=doc if isinstance(doc,list) else doc['events']
        for i,record in enumerate(records):
            # Equipment-event-only sample projection, matching the declared case.
            category=record.get('eventType') if version=='old' else record.get('eventClassification',{}).get('eventType')
            if category=='EQUIPMENT':
                out.append((name,i,record))
    return out

def field(path, definition, records):
    samples=[];evidence=[]
    for file,i,record in records:
        for value in values_at(record,path):
            text=json.dumps(value,ensure_ascii=False,separators=(',',':')) if not isinstance(value,str) else value
            if text not in samples:samples.append(text)
            evidence.append(dict(file=file,record_index=i,path=path,original_value=value))
    # Schema-authored example is used only when no success fixture covers this
    # projected field. Empty is retained when neither publisher source covers it.
    if not samples and definition['schema_example'] is not None:
        value=definition['schema_example'];samples=[value if isinstance(value,str) else json.dumps(value)]
        evidence=[dict(file=definition['schema_file'],line_start=definition['line_start'],kind='schema_example',original_value=value)]
    return {'name':path,'data_type':definition['data_type'],'samples':samples},{**definition,'sample_evidence':evidence}

def add_structure_evidence(evidence):
    # Full original component blocks preserve the allOf/$ref chain proving each
    # flattened parent path. Per-field chains point into this shared dictionary.
    structures={}
    old_names=['equipmentEvent','baseEvent','baseEventBody','baseEquipmentEvent',
               'location','address','transportCall','vessel','seals','seal']
    new_names=['Event','EventClassification','EquipmentDetails','Location','Address',
               'Facility','GeoCoordinate','TransportDetails','TransportCall',
               'ServiceCodeOrReference','VoyageNumberOrReference','VesselTransport','Seal']
    for side,filename,names in [('source',OLD,old_names),('target',NEW,new_names)]:
        lines=(BASE/filename).read_text().splitlines()
        sa,sb=block(lines,'schemas',2)
        for name in names:
            a,b=block(lines,name,4,sa,sb)
            structures[side+':'+name]={'schema_file':filename,'schema_component':name,
                'line_start':a+1,'line_end':b,'definition':'\n'.join(lines[a:b])}
    for path,details in evidence['source'].items():
        chain=['equipmentEvent']
        if path in ['eventID','eventCreatedDateTime','eventDateTime','eventType','eventClassifierCode']:
            chain+=['baseEvent']
            if path in ['eventDateTime','eventType','eventClassifierCode']:chain+=['baseEventBody']
            if path in ['eventType','eventClassifierCode']:chain+=['baseEquipmentEvent']
        else:
            chain+=['baseEquipmentEvent']
            if path.startswith('eventLocation.'):
                chain+=['location']
                if '.address.' in path:chain+=['address']
            elif path.startswith('transportCall.'):
                chain+=['transportCall']
                if '.vessel.' in path:chain+=['vessel']
            elif path.startswith('seals[].'):
                chain+=['seals','seal']
        details['parent_schema_components']=['source:'+x for x in chain]
    for path,details in evidence['target'].items():
        chain=['Event']
        if path.startswith('eventClassification.'):chain+=['EventClassification']
        elif path.startswith('equipmentDetails.'):
            chain+=['EquipmentDetails']
            if '.seals[].' in path:chain+=['Seal']
        elif path.startswith('eventLocation.'):
            chain+=['Location']
            for key,name in [('address','Address'),('facility','Facility'),('geoCoordinate','GeoCoordinate')]:
                if '.'+key+'.' in path:chain+=[name]
        elif path.startswith('transportDetails.transportCall.'):
            chain+=['TransportDetails','TransportCall']
            for key,name in [('serviceCodeOrReference','ServiceCodeOrReference'),
                             ('exportVoyageNumberOrReference','VoyageNumberOrReference'),
                             ('importVoyageNumberOrReference','VoyageNumberOrReference'),
                             ('vesselTransport','VesselTransport')]:
                if '.'+key+'.' in path:chain+=[name]
        details['parent_schema_components']=['target:'+x for x in chain]
    evidence['schema_structure_evidence']=structures
    evidence['version_status']={'source':'official 2.2.0 schema','target':'official published 3.0.0 draft; WORK IN PROGRESS; not ready for general adoption','target_notice_file':NEW,'target_notice_lines':[20,22]}

def main():
    manifest=json.loads((BASE/'manifest.json').read_text())
    for entry in manifest:
        p=BASE/pathlib.Path(entry['path']).name
        assert hashlib.sha256(p.read_bytes()).hexdigest()==entry['sha256'],p
    old_records,new_records=load_records('old'),load_records('new')
    case={'name':'dcsa_tnt_equipment_events_2_2_to_3_0','source':{'fields':[]},'target':{'fields':[]},'answers':[]}
    evidence={'case':case['name'],'projection':'Scalar leaves of equipment events only; dots retain original nesting keys and [] denotes the original array item. Schemas define types; success fixtures provide samples, with schema examples only as a documented fallback. Field count constrained by the pre-recorded selection policy.','source':{},'target':{},'labels':[]}
    for oldpath,oldschema,newpath,newschema,newprop,reason in FIELDS:
        f,e=field(oldpath,definition(OLD,oldschema),old_records);case['source']['fields'].append(f);evidence['source'][oldpath]=e
        if newpath:
            f,e=field(newpath,definition(NEW,newschema,newprop),new_records);case['target']['fields'].append(f);evidence['target'][newpath]=e
            case['answers'].append({'kind':'match','source':oldpath,'target':newpath})
        else:
            case['answers'].append({'kind':'no_match','source':oldpath})
        evidence['labels'].append({**case['answers'][-1],'rationale':reason})
    for path,schema,prop,reason in EXTRA_TARGETS:
        f,e=field(path,definition(NEW,schema,prop),new_records);case['target']['fields'].append(f);evidence['target'][path]={**e,'inclusion_rationale':reason}
    for side in ('source','target'):
        assert len({f['name'] for f in case[side]['fields']})==len(case[side]['fields'])
        assert 15<=len(case[side]['fields'])<=40
    add_structure_evidence(evidence)
    (BASE/'case.json').write_text(json.dumps(case,indent=2,ensure_ascii=False)+'\n')
    (BASE/'field_evidence.json').write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n')
    print(json.dumps({'source_fields':len(case['source']['fields']),'target_fields':len(case['target']['fields']),'match':sum(a['kind']=='match' for a in case['answers']),'no_match':sum(a['kind']=='no_match' for a in case['answers'])}))
if __name__=='__main__':main()
