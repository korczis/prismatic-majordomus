# majordomus-timeout: 300
# GitHub reaches the stream through `gh`, bounded and shared, and an unusable `gh` is a
# reported source, never a quiet repository (ADR 0133, I2403). A fixture `gh` on the server's
# PATH stands in for the network: a merged and a closed pull request are signals, a failed
# run on a branch is for the reader whose name the branch carries, a deploy on master is
# news and a green run on another branch is folded away. The server reads GitHub, so the
# command line needs no `gh` of its own. Case 1050 proves the other half: a failing `gh` is a
# source in error with gh's own reason.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
unset MAJORDOMUS_MONITOR_FOR

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

R="$T/repo"
mkdir -p "$R"
(
  cd "$R" || exit
  git init -q .
  git config user.email t@example.com
  git config user.name t
  "$MJ" init >/dev/null
  "$MJ" update >/dev/null
  mkdir -p .ai/repo/mesh
  printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
  printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-1052\n' >> .ai/repo/mesh/majordomus.yaml
  git add -A && git commit -qm base
)
mkdir -p "$T/state" "$T/bin"
# GitHub is the network: a gh on PATH that prints fixtures stands in for it
cat > "$T/bin/gh" <<'GH'
#!/bin/sh
# the fixture gh: a merged pull request, a closed one, a failed run on a branch and a deploy
now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
case "$1" in
  pr) printf '[{"number":885,"title":"a server in use is not idle","state":"MERGED","createdAt":"%s","mergedAt":"%s","closedAt":"%s","headRefName":"fix/a-server-in-use-is-not-idle","author":{"login":"korczis"},"url":"https://example.invalid/pull/885"},{"number":870,"title":"a hello names its recipient","state":"CLOSED","createdAt":"%s","mergedAt":null,"closedAt":"%s","headRefName":"fix/1b-hello","author":{"login":"korczis"},"url":""}]\n' "$now" "$now" "$now" "$now" "$now" ;;
  run) printf '[{"status":"completed","conclusion":"failure","workflowName":"validate","headBranch":"fix/1b-hello","updatedAt":"%s","displayTitle":"hello","url":""},{"status":"completed","conclusion":"success","workflowName":"Deploy Pages","headBranch":"master","updatedAt":"%s","displayTitle":"Merge #885","url":""},{"status":"completed","conclusion":"success","workflowName":"validate","headBranch":"fix/other","updatedAt":"%s","displayTitle":"other","url":""}]\n' "$now" "$now" "$now" ;;
  *) echo "gh: unexpected $*" >&2; exit 2 ;;
esac
GH
chmod +x "$T/bin/gh"
( cd "$R" && PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done
cd "$R" || exit

# ---------------------------------------------------------------- 1. pull requests and runs are signals
PAGE="$("$RB" monitor --sources github --for 1b --format json 2>/dev/null)" || { echo "    monitor failed"; exit 1; }
has() { jq -e "$1" <<<"$PAGE" >/dev/null || { echo "    expected $1 in:"; echo "$PAGE" | head -60; exit 1; }; }
has '.sources[] | select(.source == "github") | .state == "ok"'
has '[.signals[] | select(.kind == "pr_merged" and .pr == 885)] | length == 1'
has '[.signals[] | select(.kind == "pr_closed" and .pr == 870 and .for_you)] | length == 1'
has '[.signals[] | select(.kind == "run_failed" and .severity == "attention" and .for_you)] | length == 1'
has '[.signals[] | select(.kind == "deploy_succeeded")] | length == 1'
has '[.signals[] | select(.summary | test("fix/other"))] | length == 0'
has '.suppressed >= 1'

# ---------------------------------------------------------------- 2. the text names what is for the reader
expect_exit 0 "$RB" monitor --sources github --for 1b
expect_grep '^For you \(3\)'
expect_grep '#885 merged: a server in use is not idle'
expect_grep 'Deploy Pages success on master: Merge #885'
