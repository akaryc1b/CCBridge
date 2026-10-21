#!/usr/bin/env python3
"""Publish only an already signed archive; no credentials in the manifest."""
import json, pathlib, re, sys, datetime
root = pathlib.Path(__file__).resolve().parent.parent
out = pathlib.Path(sys.argv[1])
version = json.loads((root / 'src-tauri/tauri.conf.json').read_text())['version']
assert re.fullmatch(r'\d+\.\d+\.\d+',version)
arch = sys.argv[2] if len(sys.argv)>2 else 'arm64'
platform = {'arm64':'darwin-aarch64','x86_64':'darwin-x86_64'}[arch]
asset = sys.argv[3] if len(sys.argv)>3 else 'CCBridge.app.tar.gz'
assert '/' not in asset and asset.endswith('.app.tar.gz')
signature = (out/(asset+'.sig')).read_text().strip()
assert signature and (out/asset).is_file()
release_notes = root / 'docs' / f'release-{version}.md'
notes = release_notes.read_text() if release_notes.is_file() else f'CCBridge {version}'
manifest = {'version':version,'notes':notes,'pub_date':datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),'platforms':{platform:{'signature':signature,'url':f'https://github.com/akaryc1b/CCBridge/releases/download/v{version}/{asset}'}}}
(out/'latest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')

(out/f'latest-{platform}.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
