# majordomus-timeout: 300
# majordomus-covers: start finish
# A task is on the mesh while it works (I2288). `majordomus start` claims the task's scope on
# the checkout's running server as an advisory claim of the mesh session `task-<id>`, with the
# task's title as its intent, and keeps the claim's key on the task record; `finish` closes the
# session, and its claims end with it on every linked runtime. With no server, nothing is
# claimed and nothing fails.
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

# ---------------------------------------------------------------- 1. no server: nothing claimed, nothing fails
expect_exit 0 "$MJ" start "Before any server" --scope lib
grep -q '^mesh_claim: ' .ai/local/state/current.yaml && { echo "    a claim was recorded with no server"; exit 1; }
expect_exit 0 "$MJ" finish --outcome partial --note "$T/note.md"

# ---------------------------------------------------------------- 2. a server whose mesh runs
mkdir -p .ai/repo/mesh
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: case-3067\n' >> .ai/repo/mesh/majordomus.yaml
git add -A && git commit -qm "mesh enabled"
( exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' .ai/local/state/mcp/server.json 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done

# The live claims of session `s`, as compact JSON objects, one per line.
claims_of() {
  "$RB" mesh state --full --format json 2>/dev/null \
    | jq -c --arg s "$1" '.. | objects | select(has("scope") and ((.session? // "") | type == "string" and endswith("/" + $s)))'
}

expect_exit 0 "$MJ" start "Teach the parser" --scope lib
expect_grep 'mesh .*task-t-.* the scope is claimed on every linked runtime [(]advisory[)]'
id="$(sed -n 's/^id: //p' .ai/local/state/current.yaml)"
key="$(sed -n 's/^mesh_claim: //p' .ai/local/state/current.yaml)"
[ -n "$key" ] || { echo "    the task record holds no mesh claim"; exit 1; }
# the record's schema declares the key it now carries
"$MJ" check >"$T/check" 2>&1 || true
grep -q 'unknown key(s).*mesh_claim' "$T/check" && { echo "    the current schema does not declare mesh_claim"; grep mesh_claim "$T/check"; exit 1; }
C="$(claims_of "task-$id")"
jq -e -s '[.[] | select(.scope == ["lib"] and .intent == "Teach the parser")] | length >= 1' <<<"$C" >/dev/null \
  || { echo "    the task's claim is not on the mesh: $C"; "$RB" mesh state --full --format json | head -80; exit 1; }

# ---------------------------------------------------------------- 3. finish takes the task off the mesh
expect_exit 0 "$MJ" finish --outcome partial --note "$T/note.md"
S="$("$RB" mesh state --full --format json 2>/dev/null \
  | jq -c --arg s "task-$id" '[.. | objects | select((.key? // "") | type == "string" and endswith("/" + $s))][0] // empty')"
jq -e '.state == "closed"' <<<"$S" >/dev/null \
  || { echo "    the finished task's session is not closed: $S"; exit 1; }
exit 0
