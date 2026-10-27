#!/usr/bin/env python3
import json,os,pathlib,subprocess,sys,tempfile,time,uuid
binary=str(pathlib.Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix='cb-queue-') as tmp:
 root=pathlib.Path(tmp).resolve(); work=root/'project';work.mkdir();(work/'child').mkdir();other=root/'other';other.mkdir();state=root/'state';state.mkdir()
 fake=root/'claude';fake.write_text('''#!/usr/bin/env python3
import sys,time,json,os
p=sys.stdin.read(); sid=sys.argv[sys.argv.index('--session-id')+1] if '--session-id' in sys.argv else sys.argv[sys.argv.index('--resume')+1]
print(json.dumps({'type':'system','subtype':'init','session_id':sid,'model':'fake'}),flush=True)
with open(os.path.join(os.getcwd(),'execution.log'),'a') as f:f.write('start '+p+'\\n')
time.sleep(float(p.split(':')[0]))
print(json.dumps({'type':'result','subtype':'success','session_id':sid,'result':p,'is_error':False}),flush=True)
''');fake.chmod(0o700)
 env=dict(os.environ,HOME=tmp,CCBRIDGE_RUNNER_HOME=str(state))
 def rpc(op,**kw):
  p=subprocess.run([binary,'rpc'],input=json.dumps(dict(op=op,**kw)),capture_output=True,text=True,env=env,timeout=15);assert p.returncode==0,p.stderr
  return json.loads(p.stdout)
 def call(op,**kw):
  r=rpc(op,**kw);assert r['ok'],r;return r['data']
 def start(path,prompt,**kw):return call('start',workspace=str(path),claude=str(fake),mode='automatic',task_id=str(uuid.uuid4()),prompt=prompt,**kw)
 def wait(i,states,timeout=30):
  until=time.monotonic()+timeout
  while time.monotonic()<until:
   t=call('status',task_id=i)
   if t['state'] in states:return t
   time.sleep(.15)
  raise AssertionError(t)
 # Upgrade compatibility and history-size fixture, created before scheduler index initialization.
 legacy_path=root/'legacy';legacy_path.mkdir();legacy_id=str(uuid.uuid4());ld=state/legacy_id;ld.mkdir()
 (ld/'task.json').write_text(json.dumps({'id':legacy_id,'workspace':str(legacy_path),'state':'running','worker_pid':os.getpid(),'created_at':int(time.time())-120}))
 for i in range(2000):
  h=state/str(uuid.uuid4());h.mkdir();(h/'task.json').write_text(json.dumps({'state':'completed'}))
 ids=[]
 try:
  before=time.monotonic();assert call('status',task_id=legacy_id)['state']=='running';assert time.monotonic()-before<2
  blocked=start(legacy_path,'0:legacy-blocked',on_conflict='queue');ids.append(blocked['task_id']);time.sleep(.2);legacy_block=call('status',task_id=blocked['task_id']);assert legacy_block['state']=='queued',legacy_block;call('cancel',task_id=blocked['task_id'])

  a=start(work,'18:first',on_conflict='queue');ids.append(a['task_id']);wait(a['task_id'],['running'])
  b=start(work/'child','0.2:second',on_conflict='queue');ids.append(b['task_id'])
  c=start(work,'0.2:third',on_conflict='queue');ids.append(c['task_id'])
  d=start(other,'0.2:independent',on_conflict='queue');ids.append(d['task_id']);wait(d['task_id'],['completed'],5)
  assert not rpc('start',workspace=str(work),claude=str(fake),mode='automatic',task_id=str(uuid.uuid4()),prompt='0:legacy')['ok']
  time.sleep(15.2);s=call('status',task_id=b['task_id']);assert s['state']=='queued',s;assert s['queue']['blocked_by'],s
  call('cancel',task_id=b['task_id']);assert call('status',task_id=b['task_id'])['state']=='cancelled'
  wait(a['task_id'],['completed']);wait(c['task_id'],['completed'])
  assert not (work/'child'/'execution.log').exists()
  assert (work/'execution.log').read_text().splitlines()==['start 18:first','start 0.2:third']
  print('PASS parent/child FIFO, independent parallelism, legacy reject, >15s queue, cancellation')
  idem=dict(workspace=str(other),claude=str(fake),mode='automatic',task_id=str(uuid.uuid4()),prompt='1:idempotent',on_conflict='queue')
  x=call('start',**idem);ids.append(x['task_id']);again=call('start',**idem);assert again['task_id']==x['task_id']
  assert not rpc('start',**dict(idem,prompt='other'))['ok']
  wait(x['task_id'],['completed'])
  reply=dict(workspace=str(other),claude=str(fake),mode='automatic',task_id=str(uuid.uuid4()),prompt='1:reply',on_conflict='queue',reply_to=x['task_id'])
  r=call('start',**reply);ids.append(r['task_id']);assert not rpc('start',**dict(reply,task_id=str(uuid.uuid4())))['ok'];wait(r['task_id'],['completed'])
  assert call('status',task_id=r['task_id'])['session_id']==x['session_id']
  lease=str(uuid.uuid4());assert call('git_update_begin',path=str(work),lease_token=lease)['lease_token']==lease;assert call('git_update_begin',path=str(work),lease_token=lease)['lease_token']==lease
  holder=subprocess.Popen([sys.executable,'-c',"import fcntl,sys,time;f=open(sys.argv[1],'a+');fcntl.flock(f,fcntl.LOCK_EX);print('locked',flush=True);time.sleep(20)",str(state/'operations'/f'{lease}.lock')],stdout=subprocess.PIPE,text=True)
  assert holder.stdout.readline().strip()=='locked'
  assert not rpc('git_update_end',path=str(work),lease_token=lease)['ok']
  holder.terminate();holder.wait(timeout=3)

  q=start(work,'0.1:afterlease',on_conflict='queue');ids.append(q['task_id']);time.sleep(.4);assert call('status',task_id=q['task_id'])['state']=='queued'
  assert not rpc('file_create',path=str(work),name='blocked.txt')['ok']
  call('git_update_end',path=str(work),lease_token=lease);call('git_update_end',path=str(work),lease_token=lease);wait(q['task_id'],['completed'])
  analysis=call('start',**dict(idem,task_id=str(uuid.uuid4()),access='analysis'));ids.append(analysis['task_id']);failed=wait(analysis['task_id'],['failed']);assert failed['access']=='analysis' and '无法启用只读分析' in failed['error']
  # Kill only this fixture worker; unknown ownership must retain the lease.
  import signal
  u=start(other,'10:crash',on_conflict='queue');ids.append(u['task_id']);wait(u['task_id'],['running']);time.sleep(.5)
  udir=state/u['task_id'];t=json.loads((udir/'task.json').read_text());os.kill(t['worker_pid'],signal.SIGKILL);time.sleep(.2)
  unknown=call('status',task_id=u['task_id']);assert unknown['ownership_unknown'] and unknown['state']=='interrupted'
  v=start(other,'0.1:aftercrash',on_conflict='queue');ids.append(v['task_id']);time.sleep(.3);assert call('status',task_id=v['task_id'])['state']=='queued'
  marker=json.loads((udir/'spawn-marker').read_text());os.killpg(marker['pgid'],signal.SIGTERM)
  assert not rpc('task_release',task_id=u['task_id'],confirmed=False)['ok']
  call('task_release',task_id=u['task_id'],confirmed=True);wait(v['task_id'],['completed'])
  assert (udir/'release-audit.json').exists()
  print('PASS idempotency, same-session resume exclusion, Git lease, file exclusion, no analysis downgrade, crash ownership and explicit recovery')
 finally:
  for i in ids:
   try:rpc('cancel',task_id=i)
   except Exception:pass
  time.sleep(1)
