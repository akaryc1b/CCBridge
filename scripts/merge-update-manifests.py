#!/usr/bin/env python3
"""Merge signed platform manifests without losing an existing architecture."""
import json
import pathlib
import sys

output = pathlib.Path(sys.argv[1])
merged = None
for filename in sys.argv[2:]:
    current = json.loads(pathlib.Path(filename).read_text())
    if merged is None:
        merged = {**current, 'platforms': {}}
    if current['version'] != merged['version']:
        raise SystemExit('更新清单版本不一致')
    for platform, entry in current['platforms'].items():
        if platform in merged['platforms'] and merged['platforms'][platform] != entry:
            raise SystemExit(f'架构内容冲突：{platform}')
        if not entry.get('signature') or not entry.get('url'):
            raise SystemExit(f'缺少签名或地址：{platform}')
        merged['platforms'][platform] = entry
if merged is None or not merged['platforms']:
    raise SystemExit('至少提供一个已完成的架构')
if not set(merged['platforms']).issubset({'darwin-aarch64', 'darwin-x86_64'}):
    raise SystemExit('不支持的更新架构')
output.write_text(json.dumps(merged, ensure_ascii=False, indent=2) + '\n')
