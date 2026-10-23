"""Windows native process/RPC acceptance; uses no real Claude/Git credentials."""
import json, os, pathlib, shutil, subprocess, sys, tempfile, time, uuid
binary, fixture = map(lambda p: str(pathlib.Path(p).resolve()), sys.argv[1:])
root = pathlib.Path(tempfile.mkdtemp(prefix='CCBridge Windows 测试 ')).resolve()
env = dict(os.environ, USERPROFILE=str(root), HOME=str(root), LOCALAPPDATA=str(root/'AppData'/'Local'), CCBRIDGE_RUNNER_HOME=str(root/'state'))
def rpc(**v):
    p = subprocess.run([binary,'rpc'],input=json.dumps(v,ensure_ascii=False),text=True,capture_output=True,env=env,timeout=40,encoding='utf-8')
    assert p.returncode == 0, p.stderr
    value=json.loads(p.stdout)
    assert value['ok'],value
    return value['data']
def bad(**v):
    p=subprocess.run([binary,'rpc'],input=json.dumps(v),text=True,capture_output=True,env=env,timeout=40)
    assert not json.loads(p.stdout)['ok'],p.stdout
def wait(task):
    deadline=time.monotonic()+30
    while time.monotonic()<deadline:
        result=rpc(op='result',task_id=task)
        if result['state'] in ('completed','failed','cancelled'):return result
        time.sleep(.15)
    raise AssertionError('Task did not finish')
try:
    project=root/'项目 with spaces';project.mkdir();(project/'README.md').write_text('fixture remote file',encoding='utf-8')
    probe=rpc(op='probe',workspace=str(project),claude='')
    assert probe['runner_version'] and probe['claude_version'] is None
    probe=rpc(op='probe',workspace=str(project),claude=fixture)
    assert probe['claude_version']=='fixture-claude-windows'
    listing=rpc(op='fs_list',path=str(project))
    assert listing['entries'][0]['name']=='README.md'
    assert rpc(op='fs_preview',path=str(project/'README.md'))['text']=='fixture remote file'
    rpc(op='fs_mkdir',path=str(project),name='文件夹')
    bad(op='fs_mkdir',path=str(project),name='文件夹')
    bad(op='fs_mkdir',path=str(project),name='NUL')
    rpc(op='file_create',path=str(project),name='test.txt')
    rpc(op='file_mutate',path=str(project/'test.txt'),action='rename',name='renamed.txt')
    rpc(op='file_mutate',path=str(project/'renamed.txt'),action='move',destination=str(project/'文件夹'))
    moved=project/'文件夹'/'renamed.txt'
    deleted=rpc(op='file_mutate',path=str(moved),action='trash')
    assert not moved.exists()
    rpc(op='trash_restore',id=deleted['id']);assert moved.exists()
    subprocess.run(['git','init','-b','main',str(project)],check=True,capture_output=True)
    subprocess.run(['git','-C',str(project),'-c','user.name=fixture','-c','user.email=fixture@example.test','add','.'],check=True)
    subprocess.run(['git','-C',str(project),'-c','user.name=fixture','-c','user.email=fixture@example.test','commit','-m','fixture'],check=True,capture_output=True)
    assert rpc(op='branch_change',path=str(project),action='create',branch='fixture-branch')['current']=='fixture-branch'
    assert rpc(op='branch_change',path=str(project),action='switch',branch='main')['current']=='main'
    bad(op='start',task_id=str(uuid.uuid4()),workspace=str(project),claude='',mode='automatic',prompt='must not create a fake task')
    print('PASS native Windows probe / files / safe names / trash restore / Git branches')
    task=str(uuid.uuid4())
    request=dict(op='start',task_id=task,workspace=str(project),claude=fixture,mode='automatic',prompt='read fixture')
    rpc(**request);first=wait(task);assert first['state']=='completed',first
    rpc(**request) # same ID is idempotent
    second=str(uuid.uuid4());request.update(task_id=second,reply_to=task,prompt='continue fixture')
    rpc(**request);result=wait(second);assert result['state']=='completed',result
    assert result['session_id']==first['session_id']
    assert result['model_resolved']=='fixture-windows-model'
    third=str(uuid.uuid4());request.update(task_id=third,reply_to=second,prompt='wait-for-cancel')
    rpc(**request);time.sleep(.5);rpc(op='cancel',task_id=third)
    assert wait(third)['state']=='cancelled'
    print('PASS native Windows detached Worker / automatic / resume / model / cancel (fixture only)')
finally:
    shutil.rmtree(root,ignore_errors=True)
