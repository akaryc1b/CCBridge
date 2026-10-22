#!/usr/bin/env python3
"""Deterministic CC protocol fixture. Never contacts a model or reads user credentials."""
import sys,json,os,time,subprocess
args=sys.argv[1:]
if '--version' in args:
    print('fixture-claude 1.0 (NOT a real model)');sys.exit(0)
if '--input-format' in args:
    request=json.loads(sys.stdin.readline())
    assert request['type']=='control_request' and request['request']['subtype']=='initialize'
    assert '--no-session-persistence' in args
    print(json.dumps({'type':'control_response','response':{'subtype':'success','request_id':request['request_id'],'response':{'models':[{'value':'default','resolvedModel':'fixture-default'},{'value':'fixture-fast','displayName':'Fixture fast'}]}}}),flush=True)
    time.sleep(30) # The probe must reap this process after initialize, without a user prompt.
    sys.exit(0)
prompt=sys.stdin.read()
get=lambda flag:args[args.index(flag)+1] if flag in args else None
session=get('--resume') or get('--session-id')
print(json.dumps({'type':'system','subtype':'init','session_id':('00000000-0000-4000-8000-000000000999' if 'SESSION_MISMATCH' in prompt else session),'model':get('--model') or 'fixture-default'}),flush=True)
if 'SLOW' in prompt:
    time.sleep(30)
if 'FAIL' in prompt:
    print(json.dumps({'type':'result','subtype':'error_during_execution','is_error':True,'session_id':session,'result':'intentional fixture failure'}),flush=True);sys.exit(1)
if 'APPROVAL' in prompt and '--dangerously-skip-permissions' not in args:
    config=json.load(open(get('--mcp-config')))['mcpServers']['ccbridge_permissions']
    child=subprocess.Popen([config['command']]+config['args'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
    def send(v):child.stdin.write(json.dumps(v)+'\n');child.stdin.flush()
    def receive(i):
        while True:
            line=child.stdout.readline()
            if not line:raise RuntimeError('permission MCP closed')
            v=json.loads(line)
            if v.get('id')==i:return v
    send({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'fixture-cc','version':'1'}}})
    receive(1)
    send({'jsonrpc':'2.0','method':'notifications/initialized'})
    send({'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'approve','arguments':{'tool_name':'Edit','input':{'file_path':os.path.join(os.getcwd(),'result.txt'),'new_string':'approved'},'tool_use_id':'fixture-tool'}}})
    response=receive(2)
    permission=json.loads(response['result']['content'][0]['text'])
    child.stdin.close();child.wait(timeout=5)
    allowed=permission['behavior']=='allow'
    if allowed:open('result.txt','w').write('approved')
else:
    allowed=True
    if 'WRITE' in prompt:open('result.txt','w').write('automatic')
content=open('README.md').read().strip()
print(json.dumps({'type':'assistant','message':{'content':[{'type':'text','text':'Read remote README.md'}]},'session_id':session}),flush=True)
print(json.dumps({'type':'result','subtype':'success','is_error':False,'session_id':session,'result':f'fixture | code={content} | resume={"--resume" in args} | allowed={allowed}'}),flush=True)
