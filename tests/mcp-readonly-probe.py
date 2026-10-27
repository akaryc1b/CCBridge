#!/usr/bin/env python3
"""Read-only stdio MCP probe used after isolated transport recovery; no GUI/Claude calls."""
import json,subprocess,sys
p=subprocess.Popen([sys.argv[1],'mcp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
def call(i,method,params):
 p.stdin.write(json.dumps(dict(jsonrpc='2.0',id=i,method=method,params=params))+'\n');p.stdin.flush()
 while True:
  line=p.stdout.readline();assert line,'MCP closed'
  r=json.loads(line)
  if r.get('id')==i:
   assert 'error' not in r,r
   return r['result']
try:
 call(1,'initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'recovery-readonly','version':'1'}})
 p.stdin.write(json.dumps(dict(jsonrpc='2.0',method='notifications/initialized'))+'\n');p.stdin.flush()
 for i,name in enumerate(['connection_status','project_list','cc_list'],2):
  r=call(i,'tools/call',{'name':name,'arguments':{}});assert not r.get('isError'),r
  data=json.loads(r['content'][0]['text'])
  if name=='connection_status':assert data['connected'],data
 print('PASS actual stdio MCP connection_status/project_list/cc_list after isolated SSH recovery; no GUI or task dispatch')
finally:
 p.stdin.close();p.wait(timeout=10)
