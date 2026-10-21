#!/bin/sh
# Publish only tested artifacts; serialize callers with the workflow job concurrency group.
set -eu
DIR=${1:?artifact directory required}
: "${REPO:?}" "${TAG:?}"
# Reject incomplete artifacts before creating or changing a Release.
python3 - "$DIR" "$TAG" <<'CHECK'
import json, pathlib, sys
root, tag = pathlib.Path(sys.argv[1]), sys.argv[2]
manifests = list(root.glob('latest-darwin-*.json'))
if len(manifests) != 1:
    raise SystemExit('Exactly one architecture artifact is required')
manifest = json.loads(manifests[0].read_text())
if 'v' + manifest['version'] != tag:
    raise SystemExit('Tag and manifest version mismatch')
platforms = manifest['platforms']
if len(platforms) != 1:
    raise SystemExit('Exactly one signed platform is required')
platform = next(iter(platforms))
files = {
    'darwin-aarch64': ['CCBridge-macos-arm64.zip', 'CCBridge.app.tar.gz'],
    'darwin-x86_64': ['CCBridge-macos-x86_64.zip', 'CCBridge-macos-x86_64.app.tar.gz'],
}[platform]
arch = 'arm64' if platform == 'darwin-aarch64' else 'x86_64'
for name in files + ['CCBridge-macos-' + arch + '.dmg', files[1] + '.sig', 'ccbridge-linux-x86_64']:
    if not (root / name).is_file() or (root / name).stat().st_size == 0:
        raise SystemExit('Missing release artifact: ' + name)
if not platforms[platform].get('signature') or not platforms[platform].get('url'):
    raise SystemExit('Missing signature or update URL')
CHECK
# Distinguish an absent release from authentication/network errors.
RELEASES=$(gh api "repos/$REPO/releases?per_page=100")
EXISTS=$(printf '%s' "$RELEASES" | python3 -c 'import json,sys; print(any(r["tag_name"]==sys.argv[1] for r in json.load(sys.stdin)))' "$TAG")
if [ "$EXISTS" = False ]; then
  gh release create "$TAG" --repo "$REPO" --verify-tag --title "CCBridge $TAG" --generate-notes
fi
mkdir -p "$DIR/existing"
ASSETS=$(gh release view "$TAG" --repo "$REPO" --json assets --jq '.assets[].name')
if printf '%s\n' "$ASSETS" | grep -qx latest.json; then
  gh release download "$TAG" --repo "$REPO" --pattern latest.json --dir "$DIR/existing" --clobber
fi
if [ -f "$DIR/existing/latest.json" ]; then
  python3 scripts/merge-update-manifests.py "$DIR/latest.json" "$DIR/existing/latest.json" "$DIR"/latest-darwin-*.json
else
  python3 scripts/merge-update-manifests.py "$DIR/latest.json" "$DIR"/latest-darwin-*.json
fi
# Upload binaries first: an advertised updater URL must already exist.
for file in "$DIR"/*.dmg "$DIR"/*.zip "$DIR"/*.tar.gz "$DIR"/*.sig "$DIR"/ccbridge-linux-x86_64 "$DIR"/latest-darwin-*.json; do
  [ -f "$file" ] || continue
  gh release upload "$TAG" "$file" --repo "$REPO" --clobber
done
gh release upload "$TAG" "$DIR/latest.json" --repo "$REPO" --clobber
