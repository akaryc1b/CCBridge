"""Local HTTPS Git fixture: no public repository, account or real token."""
import http.server, pathlib, shutil, ssl, subprocess, threading, sys, tempfile

def exercise(root, project, env, rpc, bad):
    git=lambda *args: subprocess.run(['git',*map(str,args)],env=env,check=True,capture_output=True)
    web=root/'https';web.mkdir()
    bare=web/'fixture.git';git('init','--bare','-b','main',bare)
    git('-C',project,'push',bare,'main')
    git('-C',bare,'update-server-info')
    tls=tempfile.TemporaryDirectory(prefix='ccbridge-tls-')
    tlsroot=pathlib.Path(tls.name);cert=tlsroot/'fixture-cert.pem';key=tlsroot/'fixture-key.pem'
    config=tlsroot/'openssl.cnf';config.write_text('[req]\ndistinguished_name=dn\n[dn]\n')
    openssl=shutil.which('openssl') or r'C:\Program Files\Git\usr\bin\openssl.exe'
    generated=subprocess.run([openssl,'req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(key),'-out',str(cert),'-days','1','-config',str(config),'-subj','/CN=127.0.0.1','-addext','subjectAltName=IP:127.0.0.1'],capture_output=True)
    assert generated.returncode==0,generated.stderr.decode(errors='replace')
    class Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self,*args,**kwargs): super().__init__(*args,directory=str(web),**kwargs)
        def log_message(self,*args): pass
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler)
    context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(cert,key)
    server.socket=context.wrap_socket(server.socket,server_side=True)
    thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    url=f'https://127.0.0.1:{server.server_port}/fixture.git'
    env['GIT_SSL_CAINFO']=str(cert)
    try:
        bad(op='git_native_access',url=url.replace('fixture.git','missing.git'))
        assert rpc(op='git_native_access',url=url)['accessible']
        rpc(op='git_native_clone',path=str(root),name='克隆 with spaces',url=url)
        clone=root/'克隆 with spaces'
        assert (clone/'README.md').exists()
        bad(op='git_native_clone',path=str(root),name='克隆 with spaces',url=url)
        plan=rpc(op='git_update_plan',path=str(clone))['repositories'][0]
        (project/'README.md').write_text('fixture updated',encoding='utf-8')
        git('-C',project,'-c','user.name=fixture','-c','user.email=fixture@example.test','commit','-am','second')
        git('-C',project,'push',bare,'main');git('-C',bare,'update-server-info')
        rpc(op='git_native_fetch',path=str(clone),remote='origin')
        rpc(op='git_update_apply',path=str(clone),expected=plan)
        assert (clone/'README.md').read_text()=='fixture updated'
        print(f'PASS {sys.platform} native HTTPS access / clone / collision / fetch / fast-forward')
    finally:
        server.shutdown();server.server_close();thread.join();env.pop('GIT_SSL_CAINFO',None);tls.cleanup()
    # Host-scoped fake credential only; never contacts Codeup.
    rpc(op='git_vps_auth_install',server_storage_ack=True,username='fixture-user',secret='fixture-token-not-real')
    assert rpc(op='git_vps_auth_status')['configured']
    configenv=dict(env,GIT_CONFIG_GLOBAL=str(root/'.gitconfig'),GIT_CONFIG_NOSYSTEM='1',GIT_TERMINAL_PROMPT='0')
    result=subprocess.run(['git','credential','fill'],env=configenv,input='protocol=https\nhost=codeup.aliyun.com\n\n',capture_output=True,text=True,check=True)
    assert 'password=fixture-token-not-real' in result.stdout
    rpc(op='git_vps_auth_remove',confirm=True)
    assert not rpc(op='git_vps_auth_status')['configured']
    print(f'PASS {sys.platform} native account-local Codeup credential save / reuse / removal (fake token)')
