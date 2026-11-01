# majordomus-timeout: 300
# majordomus-covers: none
# A session is told what to proceed with, and why (I2972, ADR 0177).
#
# Before proceed.plan a session chose its issue by reading plan.next, which names one id and
# knows nothing of who holds what, and then by grepping the board by hand; on 2026-10-17 two
# sessions took the same identifier three times in one day. proceed.plan answers one
# decision from the plan's own hand-out order, every checkout's board and the live mesh
# claims. Proved here over real processes, a running mesh and real MCP clients:
#   1. with no server and a declared mesh, the command line names plan.next's issue with
#      what it unblocks and what waits, but the decision is provisional: it says the mesh
#      was not read, gives no commands and exits 11;
#   2. with the server up, every kind of hold is told apart: a mesh claim on the issue
#      (`claimed`), an announcement naming its id (`named`, quoted), an exclusive mesh claim
#      over its scope (`scope_held`) and an advisory one (`shared_with`, refusing nothing);
#      the first free issue is selected, and the caller's own announcement is never held
#      against it;
#   3. the decision is one capability on MCP, HTTP and the command line; a plain request
#      has no session, so every hold is another's; `--session` leaves out that session's
#      mesh claims, and the commands it gives claim under that session;
#   4. nothing in the repository is written and no task is started.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
RB="$(rust_bin)" || rust_bin_exit $?
LAUNCHER="$ROOT/bin/majordomus-mcp"
[ -x "$LAUNCHER" ] || { echo "    bin/majordomus-mcp is missing or not executable"; exit 1; }

R="$T/repo"; mkdir -p "$R"; R="$(cd "$R" && pwd -P)"
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
( cd "$R" && "$MJ" init >/dev/null )
( cd "$R" && pj_init && pj_milestone M000 && pj_issue I0001 M000 && pj_issue I0002 M000 I0001 \
  && pj_issue I0003 M000 && pj_issue I0004 M000 && pj_issue I0005 M000 )
mkdir -p "$R/.ai/repo/mesh"
{ printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n'
  printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n'
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: case-3367\n'
} > "$R/.ai/repo/mesh/majordomus.yaml"
git -C "$R" add -A >/dev/null && git -C "$R" commit -qm base >/dev/null
mkdir -p "$T/state"
export MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_LOG=warn XDG_STATE_HOME="$T/state"
S="$T/io"; mkdir -p "$S"
PIDS=""
cleanup() {
  exec 3>&- 2>/dev/null || true
  exec 4>&- 2>/dev/null || true
  for p in $PIDS; do kill "$p" 2>/dev/null || true; done
}
trap cleanup EXIT
before="$(git -C "$R" status --porcelain)"
plan() { rc=0; ( cd "$R" && "$RB" proceed plan "$@" ) > "$S/out" || rc=$?; }

# ---------------------------------------------------------------- 1. no server: provisional
plan --format json
[ "$rc" = 11 ] || { echo "    a decision without the declared mesh must exit 11, got $rc:"; cat "$S/out"; exit 1; }
jq -e '.provisional and .next == [] and .selected.issue == "I0001"
  and .selected.terms.unblocks == ["I0002"] and .selected.terms.in_milestone
  and .selected.terms.priority == "p1" and (.selected.finishes_with.need >= 1)
  and ([.candidates[].issue] == ["I0001", "I0003", "I0004", "I0005"])
  and .waiting == {"BLOCKED": 1}
  and (.sources[] | select(.source == "mesh") | .state == "unread")
  and (.reason | startswith("I0001 is the first READY issue that nothing refuses in M000")
       and contains("provisional, decided without all of: mesh"))' "$S/out" >/dev/null \
  || { echo "    the decision is not plan.next's issue, provisional over the unread mesh:"; cat "$S/out"; exit 1; }
plan
grep -q '^selected  I0001  Issue I0001 (M000, wave ' "$S/out" && grep -q '^unblocks  I0002$' "$S/out" \
  || { echo "    the text form does not lead with the selection:"; cat "$S/out"; exit 1; }

# ---------------------------------------------------------------- 2. a server whose mesh runs, and every kind of hold
( cd "$R" && exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$S/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
i=0; until [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; do
  i=$((i + 1)); [ "$i" -lt 300 ] || { echo "    the server never published a lease:"; cat "$S/serve.log"; exit 1; }; sleep 0.2
done
url="$(jq -r '.url' "$R/.ai/local/state/mcp/server.json")"
mesh_claim() { ( cd "$R" && "$RB" mesh claim "$@" ) > "$S/claim" 2>&1 || { echo "    mesh claim $* failed:"; cat "$S/claim"; exit 1; }; }
mesh_claim src/I0001 --session worker-a --issue I0001 --intent "taking it"
mesh_claim src/I0004/parser --session worker-a --intent "the parser"
mesh_claim src/I0005 --session worker-a --advisory --intent "reading around"

req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
await_lines() {
  local file="$1" want="$2" i=0
  until [ "$(wc -l < "$file" | tr -d ' ')" -ge "$want" ]; do
    i=$((i+1)); [ "$i" -lt 600 ] || { echo "    $file never reached $want frames:"; cat "$file"; return 1; }
    sleep 0.1
  done
}
mkfifo "$S/in_a" "$S/in_b"
( cd "$R" && exec "$LAUNCHER" ) < "$S/in_a" > "$S/out_a" 2> "$S/err_a" & PIDS="$PIDS $!"
exec 3> "$S/in_a"
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-a","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_announce","arguments":{"intent":"stacked after I0003 lands","scope":["src/I0003"]}}'
} >&3
await_lines "$S/out_a" 2 || exit 1
( cd "$R" && exec "$LAUNCHER" ) < "$S/in_b" > "$S/out_b" 2> "$S/err_b" & PIDS="$PIDS $!"
exec 4> "$S/in_b"
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-b","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_announce","arguments":{"intent":"looking at I0005","scope":["src/I0005"]}}'
  req 3 tools/list
  req 4 tools/call '{"name":"majordomus_proceed_plan","arguments":{}}'
} >&4
await_lines "$S/out_b" 4 || exit 1
sed -n 3p "$S/out_b" | jq -e '[.result.tools[].name] | index("majordomus_proceed_plan") != null' >/dev/null \
  || { echo "    MCP does not list majordomus_proceed_plan"; exit 1; }
sed -n 4p "$S/out_b" | jq -e '.result.isError == false' >/dev/null \
  || { echo "    majordomus_proceed_plan is an error:"; sed -n 4p "$S/out_b"; exit 1; }
sed -n 4p "$S/out_b" | jq '.result.structuredContent' > "$S/b.json"
# the constraints each candidate failed, as a set
jq -e 'def why($i): [.candidates[] | select(.issue == $i) | .refused[].constraint] | unique;
  (.sources | map(.state) | unique) == ["read"] and (.provisional | not)
  and why("I0001") == ["claimed"]
  and why("I0003") == ["named"]
  and why("I0004") == ["scope_held"]
  and why("I0005") == []
  and .selected.issue == "I0005"
  and (.selected.shared_with | length) >= 1
  and (.reason | contains("after 3 held by others"))
  and (.candidates[] | select(.issue == "I0001") | .refused[0] | (.holder | contains("worker-a")) and (.detail == "claims I0001 as its issue"))
  and (.candidates[] | select(.issue == "I0003") | any(.refused[]; .detail | contains("names I0003: stacked after I0003 lands")))
  and (.candidates[] | select(.issue == "I0004") | .refused[0].detail == "holds src/I0004/parser exclusively")' \
  "$S/b.json" >/dev/null \
  || { echo "    worker-b is not told who holds what, or its own announcement is held against it:"; cat "$S/b.json"; exit 1; }

# ---------------------------------------------------------------- 3. HTTP and the command line
curl -fsS "$url/api/v1/proceed/plan" > "$S/http.json" || { echo "    GET /api/v1/proceed/plan failed"; exit 1; }
# a plain request has no session: worker-b's announcement is another worker's too
jq -e '.selected == null and .next == [] and (.provisional | not)
  and ([.candidates[] | select(.issue == "I0005") | .refused[].constraint] | index("named") != null)
  and (.reason == "every one of the 4 READY issue(s) is held by another worker")' "$S/http.json" >/dev/null \
  || { echo "    the HTTP route does not refuse every held issue:"; cat "$S/http.json"; exit 1; }
plan --format json
[ "$rc" = 10 ] || { echo "    nothing selectable must exit 10, got $rc:"; cat "$S/out"; exit 1; }
# only the leaseholder holds the mesh: a decision composed by the command line itself would read
# it as unread and be provisional. The holds are compared as constraints per issue, because the
# board's announcements reach the mesh as advisory claims of their own between two readings.
jq -e --slurpfile h "$S/http.json" '(.provisional | not)
  and (.sources[] | select(.source == "mesh") | .state == "read")
  and ([.candidates[] | {issue, why: ([.refused[].constraint] | unique)}]
       == [$h[0].candidates[] | {issue, why: ([.refused[].constraint] | unique)}])' "$S/out" >/dev/null \
  || { echo "    the command line did not ask the leaseholder (its decision differs from the route's):"; cat "$S/out"; exit 1; }
# worker-a's own mesh claims are not held against worker-a
plan --session worker-a --format json
[ "$rc" = 0 ] || { echo "    worker-a's own claim was held against it (exit $rc):"; cat "$S/out"; exit 1; }
jq -e '.selected.issue == "I0001"
  and (.next[0] | startswith("majordomus mesh claim src/I0001 --session worker-a --issue I0001"))
  and (.next[2] | contains("majordomus start") and contains("--issue I0001"))' "$S/out" >/dev/null \
  || { echo "    the decision for worker-a does not take its own issue, or its commands do not claim under its session:"; cat "$S/out"; exit 1; }

# ---------------------------------------------------------------- 4. nothing is written
[ "$(git -C "$R" status --porcelain)" = "$before" ] \
  || { echo "    proceed.plan wrote to the repository:"; git -C "$R" status --porcelain; exit 1; }
[ ! -e "$R/.ai/local/state/current.yaml" ] || { echo "    a task was started"; exit 1; }
exit 0
