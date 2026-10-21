#!/usr/bin/env python3
"""Exercise the actual stdio MCP binary against a connected test broker."""
import subprocess,sys,json,time
p=subprocess.Popen([sys.argv[1],'mcp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
def call(i,method,params):
    p.stdin.write(json.dumps({'jsonrpc':'2.0','id':i,'method':method,'params':params})+'\n');p.stdin.flush()
    while True:
        line=p.stdout.readline()
        assert line,'MCP unexpectedly closed'
        v=json.loads(line)
        if v.get('id')==i:
            assert 'error' not in v,v
            return v['result']
def tool(i,name,arguments={}):
    r=call(i,'tools/call',{'name':name,'arguments':arguments})
    assert not r.get('isError'),r
    return json.loads(r['content'][0]['text'])
call(1,'initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'acceptance','version':'1'}})
p.stdin.write(json.dumps({'jsonrpc':'2.0','method':'notifications/initialized'})+'\n');p.stdin.flush()
tools=call(2,'tools/list',{})
assert len(tools['tools'])==10
assert 'git_update' in {x['name'] for x in tools['tools']}
assert tool(3,'connection_status')['connected']
assert tool(30,'repository_match',{'local_repository':sys.argv[2]})['state']=='matched'
t=tool(4,'cc_start',{'prompt':'Read README.md','mode':'approval','local_repository':sys.argv[2],'model':'fixture-model-one'})
assert t['workspace'].endswith('/projects/completely-different-name'),t
assert tool(31,'git_update',{'path':t['workspace'],'dry_run':True})['repositories']
finished=tool(40,'cc_result',{'task_id':t['task_id'],'wait_seconds':5})
assert finished['state']=='completed',finished
assert finished['model_resolved']=='fixture-model-one',finished
assert finished['model_requested']=='fixture-model-one',finished
page=tool(41,'cc_status',{'task_id':t['task_id'],'cursor':finished['next_cursor']})
assert page['events']==[],page
for i in range(5,100):
    s=tool(i,'cc_status',{'task_id':t['task_id'],'wait_seconds':2})
    if s['state']=='completed':break
    time.sleep(.1)
else:raise AssertionError(s)
assert 'remote-fixture-code' in s['result']['result'],s
next_turn=tool(101,'cc_reply',{'task_id':t['task_id'],'prompt':'Read it again'})
assert next_turn['session_id']==t['session_id']
assert next_turn['workspace']==t['workspace']
for i in range(102,160):
    s=tool(i,'cc_status',{'task_id':next_turn['task_id'],'wait_seconds':2})
    if s['state']=='completed':break
    time.sleep(.1)
assert 'resume=True' in s['result']['result'],s
assert s['model_requested']=='fixture-model-one',s
assert tool(161,'cc_result',{'task_id':next_turn['task_id']})['state']=='completed'
assert len(tool(162,'cc_list')['tasks'])>=2
auto=tool(163,'cc_start',{'prompt':'WRITE','mode':'automatic','model':'fixture-model-two'})
final=tool(164,'cc_result',{'task_id':auto['task_id'],'wait_seconds':5,'include_events':False})
assert final['state']=='completed' and 'events' not in final,final
assert final['model_resolved']=='fixture-model-two',final
p.stdin.close();p.wait(timeout=5)
print('PASS MCP initialize/list/start/status/result/reply/list; remote file read and exact session resume')
