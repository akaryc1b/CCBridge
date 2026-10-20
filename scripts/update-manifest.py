#!/usr/bin/env python3
"""Publish only an already signed archive; no credentials in the manifest."""
import json, pathlib, re, sys, datetime
root = pathlib.Path(__file__).resolve().parent.parent
out = pathlib.Path(sys.argv[1])
version = json.loads((root / 'src-tauri/tauri.conf.json').read_text())['version']
assert re.fullmatch(r'\d+\.\d+\.\d+',version)
signature = (out/'CCBridge.app.tar.gz.sig').read_text().strip()
assert signature and (out/'CCBridge.app.tar.gz').is_file()
manifest = {'version':version,'notes':f'CCBridge {version}：并发与状态修复、Tauri 界面、每轮模型选择及签名更新。','pub_date':datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),'platforms':{'darwin-aarch64':{'signature':signature,'url':f'https://github.com/akaryc1b/CCBridge/releases/download/v{version}/CCBridge.app.tar.gz'}}}
(out/'latest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
