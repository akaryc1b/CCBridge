// Narrow regressions only. Semantic copy/layout approval remains a human check.
import{readFileSync,readdirSync}from'node:fs';import{join}from'node:path';
const root=new URL('../src/',import.meta.url);let errors=[];
function walk(dir){for(const f of readdirSync(dir,{withFileTypes:true})){const p=join(dir,f.name);if(f.isDirectory())walk(p);else if(/\.(tsx|css)$/.test(f.name)){const s=readFileSync(p,'utf8');if(/请勿重复提交|请稍候|沿用上一轮模型/.test(s))errors.push(`${p}: redundant copy`);if(/w-1\/3 animate-pulse/.test(s))errors.push(`${p}: fake progress`);}}}
walk(root.pathname);const css=readFileSync(new URL('../src/index.css',import.meta.url),'utf8');if(!css.includes('border-color: var(--color-border)'))errors.push('missing base border token');const app=readFileSync(new URL('../src/App.tsx',import.meta.url),'utf8');if(!/\[mode, setMode\] = useState\("automatic"\)/.test(app))errors.push('automatic default changed');
if(errors.length){console.error(errors.join('\n'));process.exit(1)}console.log('UI contract checks passed');
