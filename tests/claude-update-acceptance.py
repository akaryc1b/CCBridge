#!/usr/bin/env python3
"""Only synthetic private installations. Never executes a real Claude updater."""
import tempfile,pathlib,os,subprocess,json,sys,uuid
binary=str(pathlib.Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix='ccbridge-cli-update-') as tmp:
 root=pathlib.Path(tmp);home=root/'home';home.mkdir();state=home/'state';state.mkdir();versions=home/'.local/share/claude/versions';versions.mkdir(parents=True);bin=home/'.local/bin';bin.mkdir(parents=True)
 actual=versions/'2.1.308';actual.write_text('''#!/usr/bin/env python3
import os,sys,pathlib
root=pathlib.Path(os.environ['HOME'])
v=root/'version'
if '--version' in sys.argv:print(v.read_text());raise SystemExit()
assert sys.argv[1:]==['update']
if (root/'fail-update').exists():print('EACCES permission denied',file=sys.stderr);raise SystemExit(1)
(root/'updated').write_text('synthetic only');v.write_text('2.1.317');print('fixture updated')
''');actual.chmod(0o700);claude=bin/'claude';claude.symlink_to(actual);(home/'version').write_text('2.1.308')
 curl=bin/'curl';curl.write_text('''#!/bin/sh
[ ! -f "$HOME/network-offline" ] || exit 7
printf 2.1.317
''');curl.chmod(0o700)
 env={**os.environ,'HOME':str(home),'CCBRIDGE_RUNNER_HOME':str(state),'PATH':str(bin)+':'+os.environ['PATH']}
 env.pop('CLAUDE_CONFIG_DIR',None)
 def call(op,**kw):
  p=subprocess.run([binary,'rpc'],input=json.dumps({'op':op,'claude':str(claude),**kw}),text=True,capture_output=True,env=env,timeout=20,check=True);return json.loads(p.stdout)
 r=call('claude_update_check');assert r['ok'] and r['data']['can_update'] and r['data']['update_available'],r
 assert not (home/'updated').exists(),'check must not update'
 for kw in [{'origin':'mcp','confirmed':True},{'origin':'app','confirmed':False},{'origin':'app','confirmed':True,'expected_version':'2.1.1'}]:
  r=call('claude_update',**kw);assert not r['ok'],r
 assert not (home/'updated').exists()
 task=str(uuid.uuid4());d=state/task;d.mkdir();(d/'task.json').write_text(json.dumps({'id':task,'session_id':task,'workspace':str(home),'state':'running','access':'analysis','ownership_unknown':True,'prompt':'synthetic active analysis'}))
 active=state/'active';active.mkdir(exist_ok=True);(active/task).touch();(state/'active-index-v1').touch()
 r=call('claude_update',origin='app',confirmed=True,expected_version='2.1.308');assert not r['ok'] and not (home/'updated').exists(),r
 (active/task).unlink();(d/'task.json').unlink();d.rmdir()
 (home/'fail-update').touch();r=call('claude_update',origin='app',confirmed=True,expected_version='2.1.308');assert not r['ok'] and '权限' in r['error'],r
 (home/'fail-update').unlink();r=call('claude_update',origin='app',confirmed=True,expected_version='2.1.308');assert r['ok'] and r['data']['current_version']=='2.1.317' and r['data']['changed'],r
 (home/'network-offline').touch();r=call('claude_update_check');assert r['ok'] and r['data']['current_version']=='2.1.317' and r['data']['check_error'],r
 (home/'network-offline').unlink();r=call('claude_update_check');assert r['ok'] and not r['data']['update_available'],r
 print('PASS synthetic check-only/no update; human confirmation; stale-version rejection; active analysis blocks; failure/retry; verified version readback; network failure/recovery')
