#!/usr/bin/env python3
"""Incremental releases: either order, retry, and fail-closed conflicts."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts/merge-update-manifests.py'

class ManifestTests(unittest.TestCase):
    def test_incremental_release(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            def manifest(name, version, platforms):
                path = root / name
                path.write_text(json.dumps(dict(version=version, platforms=platforms)))
                return path
            arm = {'darwin-aarch64': dict(signature='signed-arm', url='https://example.com/arm')}
            intel = {'darwin-x86_64': dict(signature='signed-intel', url='https://example.com/intel')}
            a = manifest('arm', '0.7.0', arm)
            i = manifest('intel', '0.7.0', intel)
            out = root / 'latest'
            def run(*inputs, ok=True):
                result = subprocess.run(['python3', str(SCRIPT), str(out), *map(str, inputs)], capture_output=True)
                self.assertEqual(result.returncode == 0, ok, result.stderr.decode())
            for first, second in [(a, i), (i, a)]:
                run(first)
                self.assertEqual(len(json.loads(out.read_text())['platforms']), 1)
                run(out, second)
                self.assertEqual(json.loads(out.read_text())['platforms'], {**arm, **intel})
                run(out, second)  # same artifact retry is idempotent
            old = manifest('old', '0.6.3', arm)
            run(out, old, ok=False)
            conflict = manifest('conflict', '0.7.0', {'darwin-aarch64':dict(signature='different', url='https://example.com/new')})
            run(out, conflict, ok=False)
            invalid = manifest('invalid', '0.7.0', {'darwin-aarch64': dict(url='https://example.com/arm')})
            run(invalid, ok=False)
            run(ok=False)

if __name__ == '__main__':
    unittest.main()
