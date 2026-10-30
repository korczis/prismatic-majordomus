# majordomus-timeout: 300
# majordomus-covers: finish
# A task off the mesh is refused (I2289, majordomus.task-on-the-mesh). On a repository
# that declares its mesh enabled, a task whose scope was never claimed on the mesh is reported
# by `check` and refused by a completed `finish`, the refusal recorded with the rule named;
# partial is not refused. `finish` claims the scope again before it judges, so a server that
# came up after `start` is all the task needed. A repository with no enabled mesh is not judged.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_BIN="$RB"

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

R="$T/repo"
mkdir -p "$R"
cd "$R" || exit 1
git init -q .
git config user.email t@example.com
git config user.name t
"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add -A && git commit -qm base
mkdir -p "$T/state" "$T/bin"
printf '#!/bin/sh\necho "gh: not here in this case" >&2\nexit 4\n' > "$T/bin/gh"
chmod +x "$T/bin/gh"
export PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state"
printf '# Objective\nx\n\n# Current State\ny\n\n# Next Action\nz\n' > "$T/note.md"
LEDGER=.ai/local/state/ledger.jsonl
# whether the newest recorded refusal names this rule among the doctrines that refused
refused_by_mesh() {
  grep '"event":"task.refused"' "$LEDGER" 2>/dev/null | tail -n 1 \
    | jq -e '[.. | objects | .refused? // empty | .[]] | index("majordomus.task-on-the-mesh") != null' >/dev/null 2>&1
}

# ---------------------------------------------------------------- 1. no enabled mesh: not judged
expect_exit 0 "$MJ" start "No mesh here" --scope lib
"$MJ" check >"$T/check0" 2>&1 || true
grep -q '^FAIL mesh' "$T/check0" && { echo "    a repository without a mesh was judged"; cat "$T/check0"; exit 1; }
"$MJ" finish --outcome completed >/dev/null 2>&1 || true
refused_by_mesh && { echo "    a completed finish without a mesh was refused for the mesh"; exit 1; }
"$MJ" finish --outcome partial --note "$T/note.md" >/dev/null 2>&1 || true

# ---------------------------------------------------------------- 2. enabled, no server: reported and refused
mkdir -p .ai/repo/mesh
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: case-3068\n' >> .ai/repo/mesh/majordomus.yaml
git add -A && git commit -qm "mesh enabled"
# `start` brings the checkout's server up itself on an enabled mesh (case 3151); the runtime
# switched off is how a start that reaches no server is made here
expect_exit 0 env MAJORDOMUS_RUNTIME=off "$MJ" start "Off the mesh" --scope lib
grep -q '^mesh_claim: ' .ai/local/state/current.yaml && { echo "    a claim was recorded with no server"; exit 1; }
"$MJ" check >"$T/check1" 2>&1 || true
grep -q '^FAIL mesh .*never claimed on it' "$T/check1" \
  || { echo "    check does not report the unclaimed scope"; grep -E 'mesh' "$T/check1"; exit 1; }
expect_exit 10 "$MJ" finish --outcome completed
refused_by_mesh || { echo "    the refusal does not name the rule"; tail -n 2 "$LEDGER"; exit 1; }
# partial is a way out, and it is not refused for the mesh
"$MJ" finish --outcome partial --note "$T/note.md" >/dev/null 2>&1 \
  || { echo "    partial was refused"; "$MJ" finish --outcome partial --note "$T/note.md"; exit 1; }

# ---------------------------------------------------------------- 3. a server came up after start: finish claims and passes the rule
expect_exit 0 env MAJORDOMUS_RUNTIME=off "$MJ" start "Server later" --scope lib
grep -q '^mesh_claim: ' .ai/local/state/current.yaml && { echo "    a claim was recorded with no server"; exit 1; }
( exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' .ai/local/state/mcp/server.json 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done
"$MJ" finish --outcome completed >/dev/null 2>&1 || true
refused_by_mesh && { echo "    finish did not claim the scope once a server answered"; tail -n 2 "$LEDGER"; exit 1; }
grep -q '^mesh_claim: ' .ai/local/state/current.yaml \
  || { echo "    finish claimed nothing although a server answered"; cat .ai/local/state/current.yaml; exit 1; }
exit 0
