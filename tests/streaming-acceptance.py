#!/usr/bin/env python3
"""Observe live speed before the fake model finishes, never use account/network."""
import pathlib,tempfile,os,subprocess,json,time,sys,uuid
binary=str(pathlib.Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix='ccbridge-stream-') as tmp:
 root=pathlib.Path(tmp).resolve();work=root/'project';work.mkdir();(work/'README.md').write_text('preserve')
 fake=root/'claude';fake.write_text('''#!/usr/bin/env python3
import sys,json,time,os
args=sys.argv[1:]
assert '--include-partial-messages' in args
if '--effort' in args: assert os.environ['CLAUDE_CODE_EFFORT_LEVEL']==args[args.index('--effort')+1]
sys.stdin.read()
def emit(e): print(json.dumps({'type':'stream_event','event':e}),flush=True)
emit({'type':'message_start','message':{'id':'request-a'}})
emit({'type':'content_block_start','index':0,'content_block':{'type':'text'}})
for i in range(8):
 emit({'type':'content_block_delta','index':0,'delta':{'type':'text_delta','text':'中文 incremental output '}});time.sleep(.4)
emit({'type':'message_delta','usage':{'output_tokens':80}})
emit({'type':'message_stop'})
print(json.dumps({'type':'assistant','message':{'id':'request-a','content':[{'type':'text','text':'final output'}]}}),flush=True)
print(json.dumps({'type':'result','subtype':'success','is_error':False,'session_id':(args[args.index('--resume')+1] if '--resume' in args else args[args.index('--session-id')+1]),'usage':{'output_tokens':80},'duration_api_ms':3200,'result':'final output'}),flush=True)
''');fake.chmod(0o700)
 env=dict(os.environ,HOME=str(root),CCBRIDGE_RUNNER_HOME=str(root/'state'))
 def call(op,**kw):
  p=subprocess.run([binary,'rpc'],input=json.dumps(dict(op=op,**kw)),text=True,capture_output=True,env=env,check=True)
  r=json.loads(p.stdout);assert r['ok'],r;return r['data']
 task=call('start',task_id=str(uuid.uuid4()),workspace=str(work),claude=str(fake),mode='automatic',access='write',effort='low',prompt='only synthetic output')
 ident=task.get('task_id') or task['id'];seen=False;deadline=time.monotonic()+20
 while time.monotonic()<deadline:
  r=call('status',task_id=ident);m=r.get('telemetry') or {}
  if r['state']=='running' and m.get('tokens_per_second',0) and m['speed_source']=='text_heuristic':seen=True
  if r['state'] in ['completed','failed','cancelled']:
   assert r['state']=='completed',r
   assert r['effort']=='low',r
   assert seen,'No speed observed while running'
   assert m['average_tokens_per_second']==25.0,m
   assert (work/'README.md').read_text()=='preserve'
   print('PASS live speed before completion / partial protocol / final mean / private fixture / unchanged file');break
  time.sleep(.15)
 else:raise AssertionError('fixture task did not finish')

 revision=call('policy_get')['revision']
 call('policy_set',origin='app',confirmed=True,expected_revision=revision,read_only=True)
 revision=call('policy_get')['revision']
 other=root/'other';other.mkdir()
 def rejected(op,**kw):
  p=subprocess.run([binary,'rpc'],input=json.dumps(dict(op=op,**kw)),text=True,capture_output=True,env=env,check=True)
  assert not json.loads(p.stdout)['ok']
 rejected('start',task_id=str(uuid.uuid4()),workspace=str(other),claude=str(fake),mode='automatic',access='write',prompt='denied')
 rejected('policy_set',origin='mcp',confirmed=True,expected_revision=revision,workspace=str(work),write_enabled=True)
 call('policy_set',origin='app',confirmed=True,expected_revision=revision,workspace=str(work),write_enabled=True)
 assert call('policy_get')['read_only'] is True
 rejected('start',task_id=str(uuid.uuid4()),workspace=str(other),claude=str(fake),mode='automatic',access='write',prompt='still denied')
 follow=call('start',task_id=str(uuid.uuid4()),reply_to=ident,workspace=str(work),claude=str(fake),mode='automatic',access='write',effort='high',prompt='continue in permitted workspace')
 assert follow['session_id']==r['session_id']
 deadline=time.monotonic()+15
 while time.monotonic()<deadline:
  result=call('status',task_id=follow['task_id'])
  if result['state']=='completed':
   assert result['effort']=='high',result
   break
  assert result['state']!='failed',result
  time.sleep(.2)
 else:raise AssertionError('continuation did not finish')
 revision=call('policy_get')['revision']
 call('policy_set',origin='app',confirmed=True,expected_revision=revision,workspace=str(work),write_enabled=False)
 rejected('start',task_id=str(uuid.uuid4()),reply_to=follow['task_id'],workspace=str(work),claude=str(fake),mode='automatic',access='write',prompt='permission revoked')
 print('PASS same-session effort low -> high / flag and child env agree')
 print('PASS workspace exception / global protection retained / MCP cannot grant / automatic continuation / revocation')
