// Credentials are read on the remote host, never placed in argv, MCP config or prompts.
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import path from 'node:path';
const entries=JSON.parse(readFileSync(process.argv[2],'utf8'));
const selected=entries.find(([,d])=>d.id===process.argv[3]);
if(!selected) throw new Error('数据库映射不存在');
const d=selected[1];
const req=createRequire(path.join(d.package_root,'package.json'));
const {DatabaseMCPServer}=await import(pathToFileURL(path.join(d.package_root,'dist/mcp/mcp-server.js')));
const {createAdapter}=await import(pathToFileURL(path.join(d.package_root,'dist/utils/adapter-factory.js')));
const {StdioServerTransport}=await import(pathToFileURL(req.resolve('@modelcontextprotocol/sdk/server/stdio.js')));
const config={type:d.kind,host:d.host,port:d.port,user:d.user,password:d.password,database:d.database,permissionMode:'safe',allowWrite:false};
if(process.argv[4]==='--protocol-test') {
 const {Client}=await import(pathToFileURL(req.resolve('@modelcontextprotocol/sdk/client/index.js')));
 const {StdioClientTransport}=await import(pathToFileURL(req.resolve('@modelcontextprotocol/sdk/client/stdio.js')));
 const client=new Client({name:'ccbridge-runtime-check',version:'1'});
 try {
  await client.connect(new StdioClientTransport({command:process.execPath,args:[process.argv[1],process.argv[2],process.argv[3],'--handshake'],stderr:'ignore'}));
  const {tools}=await client.listTools();
  if(!tools.some(t=>t.name==='execute_query') || tools.some(t=>['connect_database','disconnect_database'].includes(t.name))) throw new Error('工具列表不符合只读映射协议');
  await client.close();process.exit(0);
 } catch {await client.close().catch(()=>{});process.exit(1);}
}
const server=new DatabaseMCPServer(process.argv[4]==='--handshake'?undefined:config);
if(process.argv[4]!=='--handshake') server.setAdapter(createAdapter(config));
// Lock this named instance to its configured database and read-only mode.
const base=new StdioServerTransport();
const blocked=new Set(['connect_database','disconnect_database']);
const transport={
 async start(){base.onmessage=(m)=>{if(m.method==='tools/call'&&blocked.has(m.params?.name)){base.send({jsonrpc:'2.0',id:m.id,error:{code:-32602,message:'连接由 CCBridge 项目映射管理，不允许模型改连接或权限'}});return;}this.onmessage?.(m);};base.onerror=(e)=>this.onerror?.(e);base.onclose=()=>this.onclose?.();return base.start();},
 async send(m){if(m.result?.tools)m={...m,result:{...m.result,tools:m.result.tools.filter(t=>!blocked.has(t.name))}};return base.send(m);},
 async close(){return base.close();}
};
try {if(process.argv[4]!=='--handshake') await server.connectDatabase();if(process.argv[4]==='--test'){await server.stop();process.exit(0);}await server.connect(transport);} catch {console.error('数据库连接失败，请在 CCBridge 检查地址、认证、网络和数据库权限。');process.exit(1);}
let stopping=false;async function stop(){if(stopping)return;stopping=true;try{await server.stop();}finally{process.exit(0);}}
process.stdin.on('end',stop);process.on('SIGTERM',stop);process.on('SIGINT',stop);
