#!/bin/sh
# Real Linux binary / detached worker smoke; Claude is a deterministic shell fixture.
set -eu
BIN=$1
ROOT=$(mktemp -d /tmp/ccbridge-linux.XXXXXX)
trap 'rm -rf "$ROOT"' EXIT
mkdir -p "$ROOT/code" "$ROOT/state"
printf 'linux-remote-code\n' > "$ROOT/code/README.md"
cat > "$ROOT/fake-claude" <<'CC'
#!/bin/sh
if [ "${1:-}" = --version ]; then echo 'fixture-claude-linux'; exit; fi
session=''
while [ "$#" -gt 0 ]; do
 case "$1" in --session-id|--resume) session=$2; shift;; esac
 shift
done
cat >/dev/null
[ "$(cat README.md)" = linux-remote-code ]
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","result":"Read linux-remote-code from remote README.md"}\n' "$session"
CC
chmod 700 "$ROOT/fake-claude"
export CCBRIDGE_RUNNER_HOME="$ROOT/state"
ID=4f041bc8-0fe9-4b05-a17d-38674ee1cda0
printf '{"op":"start","task_id":"%s","workspace":"%s/code","claude":"%s/fake-claude","mode":"automatic","prompt":"read README"}' "$ID" "$ROOT" "$ROOT" | "$BIN" rpc
for i in $(seq 1 60); do
 RESULT=$(printf '{"op":"status","task_id":"%s"}' "$ID" | "$BIN" rpc)
 if echo "$RESULT" | grep -q '"state":"completed"'; then
  echo "$RESULT" | grep -q 'Read linux-remote-code'
  echo 'PASS Linux x86_64 executable / detached Runner / remote cwd / final result'
  exit 0
 fi
 sleep 1
done
echo "$RESULT"
exit 1
