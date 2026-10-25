#!/usr/bin/env python3
"""Isolated RPC acceptance and measured scan cost; no account/model/network access."""
import json, os, pathlib, statistics, subprocess, sys, tempfile, time, uuid
binary=str(pathlib.Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix='ccbridge-sessions-') as tmp:
 root=pathlib.Path(tmp).resolve();state=root/'state';state.mkdir(mode=0o700)
 work=root/'project';work.mkdir();(work/'README.md').write_text('sessions acceptance')
 env=dict(os.environ,HOME=tmp,CCBRIDGE_RUNNER_HOME=str(state))
 def rpc(op,**kw):
  p=subprocess.run([binary,'rpc'],input=json.dumps(dict(op=op,**kw)),text=True,capture_output=True,env=env,check=True)
  return json.loads(p.stdout)
 def call(op,**kw):
  r=rpc(op,**kw);assert r['ok'],r;return r['data']
 # Project entrypoints reuse the registry; create-only does not change legacy upsert.
 call('project_save', workspace=str(work), name='普通目录', create_only=True)
 assert not rpc('project_save', workspace=str(work), name='意外覆盖', create_only=True)['ok']
 assert call('project_list')['projects'][0]['name']=='普通目录'
 call('project_save', workspace=str(work), name='已有接口改名')
 assert call('project_list')['projects'][0]['name']=='已有接口改名'
 other=root/'other';other.mkdir()
 assert not rpc('project_save', workspace=str(other), name='已有接口改名', create_only=True)['ok']
 assert not rpc('project_save', workspace='/', name='越界', create_only=True)['ok']
 print('PASS project create-only duplicate protection, ordinary directory, legacy upsert and HOME boundary')
 session=str(uuid.uuid4());first=None
 def seed(i):
  global first
  ident=str(uuid.uuid4());d=state/ident;d.mkdir()
  if i==0:first=ident
  task=dict(id=ident,session_id=session if i<2 else ident,workspace=str(work),claude='/not/invoked',mode='automatic',prompt='\n首轮 · 认证分析' if i==0 else '后续输入',resume=i==1,reply_to=first if i==1 else None,state='completed',created_at=1700000000+i,updated_at=1700000000+i,worker_pid=0,result={'result':'权限 HELLO <script>alert(1)</script>'},error=None)
  (d/'task.json').write_text(json.dumps(task,ensure_ascii=False))
 for i in range(110):seed(i)
 # Unregister is metadata-only; files, sessions and legacy database config remain byte-identical.
 before_task=(state/first/'task.json').read_bytes();before_file=(work/'README.md').read_bytes()
 assert not rpc('project_remove',workspace=str(work),expected_name='陈旧名称')['ok']
 removed=call('project_remove',workspace=str(work),expected_name='已有接口改名')
 assert not any(p['path']==str(work) for p in removed['projects'])
 assert work.is_dir() and (work/'README.md').read_bytes()==before_file and (state/first/'task.json').read_bytes()==before_task
 assert call('status',task_id=first)['state']=='completed'
 assert not rpc('project_resolve',project=str(work))['ok']
 call('project_save',workspace=str(work),name='重新登记',create_only=True)
 assert call('project_resolve',project='重新登记')['workspace']==str(work)
 stale=root/'stale';stale.mkdir();call('project_save',workspace=str(stale),name='不存在目录');stale.rmdir();call('project_remove',workspace=str(stale))
 legacy_dir=root/'legacy-project';legacy_dir.mkdir();call('db_project_save',workspace=str(legacy_dir),name='数据库旧登记')
 before_db=(state/'databases.json').read_bytes();call('project_remove',workspace=str(legacy_dir),expected_name='数据库旧登记')
 assert not any(p['path']==str(legacy_dir) for p in call('project_list')['projects'])
 assert (state/'databases.json').read_bytes()==before_db
 call('project_save',workspace=str(legacy_dir),name='恢复旧登记',create_only=True)
 assert any(p['path']==str(legacy_dir) for p in call('project_list')['projects'])
 assert (state/'databases.json').read_bytes()==before_db
 # Parallel registrations serialize on projects.lock, without the task registry lock.
 import concurrent.futures
 paths=[root/f'parallel-{i}' for i in range(6)]
 for p in paths:p.mkdir()
 with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:list(pool.map(lambda p:call('project_save',workspace=str(p),name=p.name),paths))
 assert all(any(x['path']==str(p) for x in call('project_list')['projects']) for p in paths)
 print('PASS unregister preserves directory/content/history/DB; stale name, missing folder, re-add and parallel registry safety')
 r=call('session_search',q='权限');assert r['total']==109
 ids=[s['session_id'] for s in r['sessions']];cursor=r['next_cursor']
 while cursor:
  page=call('session_search',q='权限',cursor=cursor);ids.extend(s['session_id'] for s in page['sessions']);cursor=page['next_cursor']
 assert len(ids)==len(set(ids))==109
 r=call('session_search',q='首轮');assert r['sessions'][0]['title']=='首轮 · 认证分析'
 assert call('session_search',q='hello')['total']==109
 old=(state/first/'task.json').read_text()
 m=call('session_meta_set',session_id=session,title='自定义名称',archived=True,expected_revision=0);assert m['revision']==1
 assert call('session_search',q='自定义')['total']==0
 assert call('session_search',q='自定义',archive_filter='archived')['sessions'][0]['turn_count']==2
 assert (state/first/'task.json').read_text()==old
 assert not rpc('session_meta_set',session_id=session,title='覆盖',expected_revision=0)['ok']
 assert not rpc('session_meta_set',session_id=session,title='bad\nname',expected_revision=1)['ok']
 assert not rpc('session_meta_set',session_id=session,title='越权',expected_revision=1,allowed_home='/other',allowed_workspace='/other')['ok']
 m=call('session_meta_set',session_id=session,archived=False,expected_revision=1)
 assert call('session_meta_get',session_id=session)['archived'] is False
 assert not rpc('session_search',q='权限',cursor=r.get('next_cursor') or '../bad:0')['ok']
 (state/str(uuid.uuid4())).mkdir()
 assert call('session_list')['incomplete'] is True
 legacy=str(uuid.uuid4());d=state/legacy;d.mkdir();(d/'task.json').write_text(json.dumps(dict(id=legacy,workspace=str(work),claude='/not/invoked',mode='automatic',prompt='旧记录',resume=False,state='completed',created_at=1,updated_at=1,worker_pid=0,result={'result':'legacy result'},error=None)))
 assert call('session_search',q='旧记录')['sessions'][0]['legacy'] is True
 assert call('status',task_id=legacy)['result']['result']=='legacy result'
 print('PASS: >100, stable pagination, Chinese/English, safe snippets, rename/archive/restore, revision, scope, incomplete, unchanged worker file')
 timings=[];count=110
 for n in (110,1000,10000):
  for i in range(count,n):seed(i)
  count=n;elapsed=[]
  for _ in range(5):
   start=time.perf_counter();result=call('session_search',q='权限');elapsed.append(time.perf_counter()-start)
  timings.append(dict(tasks=n,seconds=elapsed,p95_seconds=sorted(elapsed)[-1],total=result['total']))
 print(json.dumps({'scan_measurements':timings,'machine':os.uname().machine},ensure_ascii=False))
