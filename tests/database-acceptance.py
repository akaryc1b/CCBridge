import json,subprocess,os,pathlib,uuid,time,tempfile,shutil,select,sys
ENGINE=sys.argv[1] if len(sys.argv)>1 else "postgres"
SOURCE=pathlib.Path(__file__).resolve().parents[1]
ROOT=pathlib.Path(tempfile.mkdtemp(prefix='ccbridge-db-acceptance-')).resolve();BIN=str(pathlib.Path(os.environ.get('CCBRIDGE_TEST_BIN',str(SOURCE/'target/release/ccbridge'))).resolve());name='ccbridge-db-test-'+uuid.uuid4().hex[:8]
password=uuid.uuid4().hex;port=None;children=[]
def sh(*a):return subprocess.check_output(a,text=True).strip()
def rpc(v,env=None,ok=True):
 p=subprocess.run([BIN,'rpc'],input=json.dumps(v),text=True,capture_output=True,env=env or ENV,timeout=40);r=json.loads(p.stdout)
 if ok:assert r['ok'],r;return r['data']
 assert not r['ok'];return r
ENV={**os.environ,'HOME':str(ROOT),'CCBRIDGE_RUNNER_HOME':str(ROOT/'state')}
project=ROOT/'project';project.mkdir();(project/'README.md').write_text('db-test-project')
fixture=str((SOURCE/'tests/fake_claude.py').resolve())
def start(parent=None,work=None):
 v={'op':'start','task_id':str(uuid.uuid4()),'workspace':str(work or project),'claude':fixture,'mode':'automatic','prompt':'Read fixture'}
 if parent:v['reply_to']=parent
 t=rpc(v)
 for _ in range(100):
  s=rpc({'op':'status','task_id':t['task_id']})
  if s['state']=='completed':return t
  assert s['state']!='failed',s
  time.sleep(.1)
 raise AssertionError('task timeout')
def mcp(t,cid):
 p=subprocess.Popen([BIN,'database-server',t['task_id'],cid],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,env=ENV);children.append(p);return p
counter=0
def call(p,method,params):
 global counter
 counter+=1;i=counter;p.stdin.write(json.dumps({'jsonrpc':'2.0','id':i,'method':method,'params':params})+'\n');p.stdin.flush()
 end=time.monotonic()+25
 while time.monotonic()<end:
  ready,_,_=select.select([p.stdout],[],[],1)
  if ready:
   line=p.stdout.readline();assert line,'database MCP closed';v=json.loads(line)
   if v.get('id')==i:return v
 raise AssertionError('MCP timeout')
def initialize(p):
 assert 'result'in call(p,'initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'db-integration-test','version':'1'}})
 p.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');p.stdin.flush()
try:
 sh('docker','run','-d','--name',name,'-e',('POSTGRES_PASSWORD=' if ENGINE=='postgres' else 'MYSQL_ROOT_PASSWORD=')+password,'-p','127.0.0.1::'+('5432' if ENGINE=='postgres' else '3306'),'postgres:16-alpine' if ENGINE=='postgres' else 'mysql:8')
 port=int(sh('docker','port',name,('5432' if ENGINE=='postgres' else '3306')+'/tcp').split(':')[-1])
 for _ in range(150):
  cmd=['docker','exec',name,'pg_isready','-U','postgres'] if ENGINE=='postgres' else ['docker','exec',name,'mysql','-uroot','-p'+password,'-e','SELECT 1']
  if subprocess.run(cmd,capture_output=True).returncode==0:break
  time.sleep(.3)
 if ENGINE=='postgres':sh('docker','exec',name,'psql','-U','postgres','-c','CREATE DATABASE ccbridge_second;')
 else:sh('docker','exec',name,'mysql','-uroot','-p'+password,'-e','CREATE DATABASE postgres; CREATE DATABASE ccbridge_second;')
 ids=[]
 for label,db in [('business','postgres'),('workflow','ccbridge_second')]:
  d={'id':str(uuid.uuid4()),'name':label,'kind':ENGINE,'host':'127.0.0.1','port':port,'user':'postgres' if ENGINE=='postgres' else 'root','password':password,'database':db,'enabled':True,'node':str(pathlib.Path(sh('which','node')).resolve()),'package_root':os.environ.get('UDB_PACKAGE_ROOT',str(pathlib.Path(sh('which','universal-db-mcp')).resolve().parent.parent))}
  r=rpc({'op':'db_save','connection':d});assert 'password'not in r and password not in json.dumps(r);ids.append(d['id'])
  assert rpc({'op':'db_protocol_test','id':d['id']})['mcp_connected']
  assert rpc({'op':'db_test','id':d['id']})['connected']
 assert rpc({'op':'db_list','workspace':str(project)})['connections'][0]['password_saved']
 # Password blank edits preserve credentials; protocol handshake must not contact the DB.
 saved={**d,'password':'','host':'unreachable.invalid'}
 rpc({'op':'db_save','connection':saved})
 assert rpc({'op':'db_protocol_test','id':d['id']})['database_connected'] is False
 rpc({'op':'db_save','connection':{**d,'password':''}})
 assert (ROOT/'state/databases.json').stat().st_mode&0o777==0o600
 bindings=[{'alias':a,'connection_id':i} for a,i in zip(['business_dev','workflow_dev'],ids)]
 rpc({'op':'db_bind','workspace':str(project),'bindings':bindings})
 rpc({'op':'db_bind','workspace':str(project),'bindings':[bindings[0],bindings[0]]},ok=False)
 rpc({'op':'db_remove','id':ids[0]},ok=False)
 rpc({'op':'db_bind','workspace':'/tmp','bindings':[]},ok=False)
 project_b=ROOT/'project-b';project_b.mkdir();(project_b/'README.md').write_text('project-b')
 rpc({'op':'db_project_save','workspace':str(project),'name':'项目A'})
 rpc({'op':'db_project_save','workspace':str(project_b),'name':'项目B'})
 rpc({'op':'db_project_save','workspace':str(project_b),'name':'项目A'},ok=False)
 bindings_b=[{'alias':'business_dev','connection_id':ids[1]},{'alias':'reporting','connection_id':ids[0]}]
 rpc({'op':'db_bind','workspace':str(project_b),'bindings':bindings_b})
 projects=rpc({'op':'db_projects'})['projects'];assert len(projects)==2 and all(len(x['databases'])==2 for x in projects)
 assert rpc({'op':'db_resolve','project':'项目B'})['workspace']==str(project_b)
 assert rpc({'op':'db_list','workspace':str(project)})['bindings']==bindings
 tb=start(work=project_b);cb=json.loads((ROOT/'state'/tb['task_id']/'mcp.json').read_text())['mcpServers'];assert set(cb)=={'ccbridge_permissions','db_business_dev','db_reporting'}
 assert cb['db_business_dev']['args'][-1]==ids[1] and cb['db_reporting']['args'][-1]==ids[0]
 t=start();config=json.loads((ROOT/'state'/t['task_id']/'mcp.json').read_text());assert set(config['mcpServers'])=={'ccbridge_permissions','db_business_dev','db_workflow_dev'};assert password not in json.dumps(config)
 assert (ROOT/'state'/t['task_id']/'databases.json').stat().st_mode&0o777==0o600
 for cid,expected in zip(ids,['postgres','ccbridge_second']):
  p=mcp(t,cid);initialize(p)
  tools=call(p,'tools/list',{})['result']['tools'];names={x['name']for x in tools};assert 'execute_query'in names and 'connect_database'not in names and 'disconnect_database'not in names
  r=call(p,'tools/call',{'name':'execute_query','arguments':{'query':'SELECT current_database() AS db' if ENGINE=='postgres' else 'SELECT DATABASE() AS db'}});assert expected in json.dumps(r),r
  assert 'error'in call(p,'tools/call',{'name':'connect_database','arguments':{}})
  r=call(p,'tools/call',{'name':'execute_query','arguments':{'query':'CREATE TABLE should_not_exist (id int)'}});assert r['result']['isError'],r
  p.stdin.close();p.wait(timeout=8)
 # Project mapping changes affect new conversations, not an existing continuation snapshot.
 assert len(rpc({'op':'status','task_id':t['task_id']})['database_snapshot']['mappings'])==2
 rpc({'op':'db_bind','workspace':str(project),'bindings':[]})
 assert rpc({'op':'status','task_id':t['task_id']})['database_snapshot']['changed']
 r=start(t['task_id']);assert r['session_id']==t['session_id'];assert (ROOT/'state'/r['task_id']/'databases.json').read_text()==(ROOT/'state'/t['task_id']/'databases.json').read_text()
 isolated={**ENV,'CCBRIDGE_RUNNER_HOME':str(ROOT/'other-user-state')};assert not rpc({'op':'db_list'},isolated)['connections']
 assert rpc({'op':'db_list','workspace':str(project_b)})['bindings']==bindings_b
 print('PASS two projects x two databases, shared connection reuse, same alias routes independently by project, project name validation and exact routing')
 print('PASS real universal-db-mcp 2.14.0 + disposable '+ENGINE+': two named databases isolated, SELECT works, DDL rejected, dynamic reconnect rejected, secrets omitted from responses/argv/MCP config, files 600, mapping validation, account-store isolation, continuation snapshot retained')
finally:
 for p in children:
  if p.poll() is None:p.terminate();p.wait(timeout=8)
 subprocess.run(['docker','rm','-f',name],capture_output=True)
 shutil.rmtree(ROOT)
