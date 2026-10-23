#!/usr/bin/env python3
"""Zero-dependency structural pack check; optionally uses installed jsonschema."""
from pathlib import Path
import json,sys
root=Path(__file__).resolve().parents[1]; errors=[]
try: data=json.loads((root/'manifest.json').read_text())
except Exception as ex: print('ERROR manifest:',ex);sys.exit(1)
ids={x['id'] for x in data['missions']}
if len(ids)!=len(data['missions']):errors.append('duplicate mission id')
if not (root/data['entrypoint']).is_file():errors.append('missing entrypoint')
for name,path in data['contracts'].items():
 if not (root/path).is_file():errors.append('missing schema '+path)
for m in data['missions']:
 p=root/m['prompt']
 if not p.is_file() or p.stat().st_size<500:errors.append('missing/short prompt '+str(p))
 for d in m['depends_on']:
  if d not in ids: errors.append('unknown dependency '+d)
 sp=root/'schemas'/(m['id']+'.mission.json')
 if not sp.is_file():errors.append('missing mission contract '+str(sp))
 else:
  spec=json.loads(sp.read_text())
  for field in ('id','title','objective','dependencies','inputs','deliverables','acceptance','gates','rollback'):
   if field not in spec: errors.append(str(sp)+': missing '+field)
visiting=set();visited=set(); byid={m['id']:m for m in data['missions']}
def walk(node):
 if node in visiting: errors.append('dependency cycle '+node);return
 if node in visited:return
 visiting.add(node)
 for d in byid[node]['depends_on']:
  if d in byid:walk(d)
 visiting.remove(node);visited.add(node)
for n in ids:walk(n)
try:
 import jsonschema
 for m in data['missions']:
  jsonschema.validate(json.loads((root/'schemas'/(m['id']+'.mission.json')).read_text()),json.loads((root/'schemas'/'mission.schema.json').read_text()))
 print('Optional jsonschema validation: passed')
except ModuleNotFoundError:print('Optional jsonschema validation: skipped (pip install jsonschema)')
except Exception as ex:errors.append('jsonschema: '+str(ex))
for e in errors:print('ERROR:',e)
print(f'Missions: {len(ids)}; errors: {len(errors)}')
sys.exit(bool(errors))
