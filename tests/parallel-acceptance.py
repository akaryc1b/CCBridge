#!/usr/bin/env python3
"""Account-isolated RPC regression. Fake Claude only; no real credentials/model."""
import json,os,pathlib,subprocess,sys,tempfile,time,uuid
binary=str(pathlib.Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix='cb-parallel-') as tmp:
 root=pathlib.Path(tmp).resolve(); work=root/'project';work.mkdir();child=work/'child';child.mkdir();other=root/'other';other.mkdir(); state=root/'state';state.mkdir()
 fake=root/'claude';fake.write_text('''#!/usr/bin/env python3
import sys,json,time
args=sys.argv[1:]
if '--version' in args:
 print('2.1.308 (Fixture Claude)');sys.exit(0)
if '--help' in args:
 print('fixture help deliberately omits hidden supported flags');sys.exit(0)
p=sys.stdin.read();sid=args[args.index('--session-id')+1] if '--session-id' in args else args[args.index('--resume')+1]
analysis='--tools' in args
if analysis:
 assert args[args.index('--tools')+1]=='Read,Glob,Grep'
 assert '--safe-mode' in args and '--bare' not in args and '--dangerously-skip-permissions' not in args
 assert args[args.index('--setting-sources')+1]=='user'
 assert args[args.index('--permission-mode')+1]=='dontAsk'
 assert json.load(open(args[args.index('--mcp-config')+1]))=={'mcpServers':{}}
print(json.dumps({'type':'system','subtype':'init','session_id':sid,'model':'fixture-model','tools':['Bash'] if 'badtools' in p else ['Read','Glob','Grep','EndConversation'],'mcp_servers':[],'plugins':[],'slash_commands':[],'permissionMode':'dontAsk'}),flush=True)
time.sleep(float(p.split(':')[0]))
print(json.dumps({'type':'result','subtype':'success','is_error':False,'session_id':sid,'result':'analysis='+str(analysis)+' resume='+str('--resume' in args)}),flush=True)
''');fake.chmod(0o700)
 env=dict(os.environ,HOME=tmp,CCBRIDGE_RUNNER_HOME=str(state));env.pop('CLAUDE_CONFIG_DIR',None)
 ids=[]
 def rpc(op,**kw):
  p=subprocess.run([binary,'rpc'],input=json.dumps(dict(op=op,**kw)),capture_output=True,text=True,env=env,timeout=15);assert p.returncode==0,p.stderr;return json.loads(p.stdout)
 def call(op,**kw):
  r=rpc(op,**kw);assert r['ok'],r;return r['data']
 def start(path,prompt,**kw):
  data=dict(workspace=str(path),claude=str(fake),mode='automatic',task_id=str(uuid.uuid4()),prompt=prompt,on_conflict='queue',**kw)
  t=call('start',**data);ids.append(t['task_id']);return t,data
 def wait(i,states,timeout=12):
  until=time.monotonic()+timeout
  while time.monotonic()<until:
   t=call('status',task_id=i)
   if t['state'] in states:return t
   time.sleep(.1)
  raise AssertionError(t)
 def policy(v):
  p=call('policy_get');return call('policy_set',origin='app',confirmed=True,expected_revision=p['revision'],read_only=v)
 try:
  assert call('policy_get')['read_only']==False
  w,_=start(work,'4:write',access='write');wait(w['task_id'],['running'])
  a,original=start(child,'2:analysis',access='analysis');b,_=start(work,'2:analysis',access='analysis')
  wait(a['task_id'],['running']);wait(b['task_id'],['running']);assert call('status',task_id=w['task_id'])['state']=='running'
  q,_=start(work,'0.1:queued-write',access='write');time.sleep(.2);assert call('status',task_id=q['task_id'])['state']=='queued'
  assert not rpc('git_update_begin',path=str(work))['ok']
  policy(True)
  assert call('status',task_id=w['task_id'])['state']=='running'
  assert not rpc('policy_set',read_only=False,confirmed=True,expected_revision=1)['ok']
  assert not rpc('start',**dict(original,task_id=str(uuid.uuid4()),access='write'))['ok']
  assert call('start',**original)['task_id']==a['task_id']
  x,_=start(other,'0.1:policy-default');assert wait(x['task_id'],['completed'])['access']=='analysis'
  for origin in ['mcp','app']:
   lease=call('git_update_begin',path=str(other),origin=origin)['lease_token'];call('git_update_end',path=str(other),lease_token=lease)
  # Protection limits Claude tools, not controlled fast-forward Git. Exercise a real repo.
  source=root/'source';source.mkdir()
  def git(path,*args):return subprocess.check_output(['git','-C',str(path),*args],env=env,stderr=subprocess.PIPE,text=True).strip()
  git(source,'init','-q','-b','main');git(source,'config','user.name','fixture');git(source,'config','user.email','fixture@example.test')
  (source/'README').write_text('before');git(source,'add','README');git(source,'commit','-qm','before')
  target=other/'repo';git(other,'clone','-q',str(source),str(target))
  plan=call('git_update_plan',path=str(target))['repositories'][0]
  (source/'README').write_text('after');git(source,'commit','-qam','after');git(target,'fetch','-q')
  lease=call('git_update_begin',path=str(target),origin='mcp')['lease_token']
  call('git_update_preflight',path=str(target),origin='mcp',expected=plan,lease_token=lease)
  result=call('git_update_apply',path=str(target),origin='mcp',expected=plan,lease_token=lease)
  assert result['state']=='updated' and (target/'README').read_text()=='after',result
  call('git_update_end',path=str(target),lease_token=lease)
  (target/'README').write_text('own change')
  assert not rpc('git_update_preflight',path=str(target),origin='mcp')['ok']
  assert (target/'README').read_text()=='own change'

  wait(a['task_id'],['completed']);wait(b['task_id'],['completed']);wait(w['task_id'],['completed'])
  time.sleep(.5);paused=call('status',task_id=q['task_id']);assert paused['state']=='queued' and paused['blocked_reason']=='policy',paused
  print('PASS same-workspace 2 analysis + write overlap, write exclusion, human policy, queued pause, protected MCP/manual Git lease + fast-forward + dirty guard',flush=True)
  policy(False);wait(q['task_id'],['completed'])
  r,reply=start(child,'1:reply',reply_to=a['task_id']);assert not rpc('start',**dict(reply,task_id=str(uuid.uuid4())))['ok']
  resumed=wait(r['task_id'],['completed']);assert resumed['access']=='analysis' and resumed['session_id']==a['session_id'];assert 'resume=True' in resumed['result']['result']
  u,_=start(work,'4:cancel-one',access='analysis');v,_=start(work,'1:other-analysis',access='analysis');wait(u['task_id'],['running']);wait(v['task_id'],['running']);call('cancel',task_id=u['task_id']);assert wait(u['task_id'],['cancelled'])['state']=='cancelled';assert wait(v['task_id'],['completed'])['state']=='completed'
  bad,_=start(work,'0.2:badtools',access='analysis');bad=wait(bad['task_id'],['failed']);assert '未允许' in bad['error'],bad
  config=root/'.claude';config.mkdir();(config/'remote-settings.json').write_text('{"settings":{"permissions":{"defaultMode":"default"}}}')
  benign,_=start(work,'0.1:managed-benign',access='analysis');assert wait(benign['task_id'],['completed'])['state']=='completed'
  (config/'remote-settings.json').write_text('{"settings":{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"echo managed"}]}]}}}')
  blocked,_=start(work,'0.1:managed',access='analysis');blocked=wait(blocked['task_id'],['failed']);assert '受管' in blocked['error'],blocked
  assert not (state/blocked['task_id']/'spawn-marker').exists()
  print('PASS analysis resume without escalation, same-session exclusion, cancel isolation, init tool refusal, managed-policy preflight refusal',flush=True)
 finally:
  for i in ids:
   try:rpc('cancel',task_id=i)
   except Exception:pass
  time.sleep(1)
