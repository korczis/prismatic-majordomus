# majordomus-covers: capability:devtask.implement
# majordomus-timeout: 300
# devtask.implement runs readiness, intents.preflight, the claim, worktree.create and one
# execution, in that order, and once (I4003, Mission Control B4).
#
# Before I4003 taking an issue was a sequence a session ran by hand, and two sessions could
# run it at once; a double request started two runs. Proven here from outside the
# executable, against a real server whose mesh runs, on a port the system picks, with a real
# MCP session as the worker:
#
#   plain HTTP, nobody attached   -> pending, not failed; no worktree, no execution
#   an issue whose intent link
#     points at nothing           -> refused invalid_intent; nothing created
#   an issue somebody else claims -> refused claim_conflict naming the holder; nothing created
#   the attached worker asks      -> started: a new worktree, one execution of devtask.run
#                                    that holds the issue's exclusive claim (execution-<id>)
#   the same request again        -> the same execution, deduplicated; still one execution
#   the worker leaves             -> the run succeeds and gives its claim back
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4003.XXXXXX")"
# the issue's worktree is made beside this checkout, at <checkout>-wt, and goes with the case
WT="$(pwd -P)-wt"
SRV=""; trap '[ -n "$SRV" ] && kill "$SRV" 2>/dev/null; rm -rf "$S" "$WT"' EXIT

"$MJ" init >/dev/null
pj_init
# one maintenance milestone: issues under it serve no live intent, which the preflight allows
cat > .ai/repo/project/milestones/m-work.yaml <<'YAML'
id: m-work
title: The work
slug: m-work
order: 0
priority: p1
problem: "A problem."
outcome: "An outcome."
acceptance_criteria:
  - It is reached
validation:
  - "true"
evidence_required: []
YAML
issue() { # <id> <scope> [serves-block]
  cat > ".ai/repo/project/issues/$1.yaml" <<YAML
id: $1
milestone: m-work
title: The work of $1
slug: work-$1
priority: p1
profile: implementation
objective: "Do the work of $1."
scope:
  - $2
${3:-}
acceptance_criteria:
  - It is done
validation:
  - "true"
evidence_required:
  - done
YAML
}
issue I0001 src/ready
issue I0002 src/nowhere "serves:
  - nobody#nothing"
# a mesh that runs in the server and links nobody (the declaration of cases 3067 and 4004)
mkdir -p .ai/repo/mesh
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: case-4003\n' >> .ai/repo/mesh/majordomus.yaml
git add -A >/dev/null && git commit -qm layer

# the server, from a copy of the executable, on loopback and a port the system picks; its
# address is read from the lease it publishes (the harness of cases 3861, 4000 and 4004)
cp "$RB" "$S/majordomus"
( exec "$S/majordomus" serve --host 127.0.0.1 --port 0 --idle 0 --discovery filesystem </dev/null >"$S/serve.log" 2>&1 ) &
SRV=$!
U=""
for _ in $(seq 1 600); do
  U="$(jq -r --argjson pid "$SRV" 'select(.pid == $pid) | .url // empty' .ai/local/state/mcp/server.json 2>/dev/null || true)"
  [ -n "$U" ] && break
  kill -0 "$SRV" 2>/dev/null || { echo "    the server exited before listening:"; sed 's/^/      /' "$S/serve.log"; exit 1; }
  sleep 0.2
done
[ -n "$U" ] || { echo "    the server did not publish an address in 120 s:"; sed 's/^/      /' "$S/serve.log"; exit 1; }
curl -sS --max-time 30 "$U/api/v1/mesh/cooperation" > "$S/coop.json" || true
jq -e '.active == true' "$S/coop.json" >/dev/null \
  || { echo "    the server's mesh does not run:"; sed 's/^/      /' "$S/coop.json"; exit 1; }

# the worker: one MCP session over HTTP, attached to the server's board
mcp() { # id method params-json [extra curl args...]
  local id="$1" method="$2" params="$3"; shift 3
  curl -sS --max-time 60 -X POST "$U/mcp" -H 'content-type: application/json' \
    -H 'accept: application/json, text/event-stream' "$@" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":$id,\"method\":\"$method\",\"params\":$params}" \
    | tr -d '\r' | sed -e 's/^data: //' | jq -cs '[.[] | select(type == "object")][0]'
}
mcp 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4003-worker","version":"0"}}' \
  -D "$S/mcp.headers" > "$S/init.json"
SID="$(tr -d '\r' < "$S/mcp.headers" | sed -n 's/^[Mm]cp-[Ss]ession-[Ii]d: //p' | head -n 1)"
[ -n "$SID" ] || { echo "    the MCP endpoint opened no session:"; cat "$S/mcp.headers" "$S/init.json"; exit 1; }
implement() { # name arguments-json: the worker's call; leaves the JSON-RPC answer in $S/<name>.json
  mcp 2 tools/call "{\"name\":\"majordomus_devtask_implement\",\"arguments\":$2}" -H "mcp-session-id: $SID" > "$S/$1.json"
}
ok_of() { jq -c '.result.structuredContent' "$S/$1.json"; }
# a refusal's text, without the error code MCP puts before it
refusal_of() { jq -r 'select(.result.isError == true) | .result.content[0].text' "$S/$1.json" | sed 's/^refused: //'; }
runs() { curl -sS --max-time 30 "$U/api/v1/executions?capability=devtask.run" | jq '.count'; }
get() { curl -sS --max-time 30 "$U/api/v1/executions/get?id=$1"; }
until_state() { # name id jq-expression
  for _ in $(seq 1 300); do
    get "$2" > "$S/$1.now.json" 2>/dev/null || true
    jq -e "$3" "$S/$1.now.json" >/dev/null 2>&1 && return 0
    sleep 0.1
  done
  echo "    $1 never reached $3:"; jq -c '{state, issue_claim, error}' "$S/$1.now.json" 2>/dev/null || cat "$S/$1.now.json"
  exit 1
}
nothing_created() { # why
  [ ! -e "$WT" ] || { echo "    $1, and a worktree was made anyway:"; ls -R "$WT" | head; exit 1; }
  [ "$(runs)" = 0 ] || { echo "    $1, and an execution was started anyway"; exit 1; }
}

# ---------------------------------------------------------------- nobody attached: pending
code="$(curl -sS --max-time 60 -o "$S/http.json" -w '%{http_code}' -X POST -H 'Content-Type: application/json' \
  -d '{"issue":"I0001"}' "$U/api/v1/devtask/implement" || true)"
[ "$code" = 200 ] || { echo "    a request nobody can take was not answered ($code):"; sed 's/^/      /' "$S/http.json"; exit 1; }
jq -e '.standing == "pending" and (.pending | startswith("no worker is attached"))
       and ([.steps[] | [.step, .ok]] == [["readiness",true],["preflight",true],["claim",true],["worker",false]])
       and (has("execution") | not) and (has("worktree") | not)' "$S/http.json" >/dev/null \
  || { echo "    with no worker attached the answer is not pending:"; jq -c . "$S/http.json"; exit 1; }
nothing_created "pending"

# ---------------------------------------------------------------- an invalid intent is refused
implement invalid '{"issue":"I0002"}'
refusal_of invalid | grep -q '^invalid_intent: I0002: .*nobody' \
  || { echo "    an issue whose intent link points at nothing was not refused invalid_intent:"; jq -c . "$S/invalid.json"; exit 1; }
nothing_created "invalid_intent"

# ---------------------------------------------------------------- an overlapping claim is refused
curl -sS --max-time 30 -X POST -H 'Content-Type: application/json' \
  -d '{"session":"somebody-else","scope":["src/ready"],"intent":"already on it"}' "$U/api/v1/mesh/claims" > "$S/other.json"
OTHER="$(jq -r '.key // empty' "$S/other.json")"
[ -n "$OTHER" ] || { echo "    the competing claim was not taken:"; cat "$S/other.json"; exit 1; }
implement overlap '{"issue":"I0001","idempotency_key":"click-1"}'
refusal_of overlap | grep -q "^claim_conflict: I0001 .*somebody-else.*$OTHER" \
  || { echo "    an overlapping claim was not refused naming its holder:"; jq -c . "$S/overlap.json"; exit 1; }
nothing_created "claim_conflict"
curl -sS --max-time 30 -X POST -H 'Content-Type: application/json' \
  -d "{\"claim\":\"$OTHER\"}" "$U/api/v1/mesh/claims/release" > "$S/released.json"

# ---------------------------------------------------------------- the worker asks: one run, claimed
implement first '{"issue":"I0001","idempotency_key":"click-1"}'
ok_of first > "$S/first.ok.json"
jq -e '.standing == "started" and .branch == "feature/I0001-work-I0001" and .worker != null
       and ([.steps[] | .step] == ["readiness","preflight","claim","worker","worktree","execution"])
       and ([.steps[] | .ok] | all) and .execution.capability == "devtask.run"
       and .execution.deduplicated == false' "$S/first.ok.json" >/dev/null \
  || { echo "    the worker's request did not start one run:"; jq -c . "$S/first.json"; exit 1; }
RUN="$(jq -r .execution.id "$S/first.ok.json")"
TREE="$(jq -r .worktree "$S/first.ok.json")"
[ "$TREE" = "$WT/feature/I0001-work-I0001" ] && [ -e "$TREE/.git" ] \
  || { echo "    the run's worktree is not the issue's canonical one: $TREE"; exit 1; }
[ "$(git -C "$TREE" rev-parse --abbrev-ref HEAD)" = "feature/I0001-work-I0001" ] \
  || { echo "    the worktree is not on the issue's branch"; exit 1; }
until_state first "$RUN" '.state == "running"'
jq -e --arg s "execution-$RUN" '.issue_claim.standing == "held" and .issue_claim.session == $s
       and .issue_claim.scope == ["src/ready"] and (.issue_claim.key | type == "string")' "$S/first.now.json" >/dev/null \
  || { echo "    the run does not hold the issue's claim:"; jq -c .issue_claim "$S/first.now.json"; exit 1; }
KEY="$(jq -r .issue_claim.key "$S/first.now.json")"

# ---------------------------------------------------------------- the same request is the same run
implement again '{"issue":"I0001","idempotency_key":"click-1"}'
ok_of again > "$S/again.ok.json"
jq -e --arg id "$RUN" --arg t "$TREE" '.standing == "deduplicated" and .execution.id == $id
       and .worktree == $t' "$S/again.ok.json" >/dev/null \
  || { echo "    the same request again was not answered the same execution:"; jq -c . "$S/again.json"; exit 1; }
[ "$(runs)" = 1 ] || { echo "    a repeated request started a second run: $(runs)"; exit 1; }

# ---------------------------------------------------------------- the worker leaves: the run ends
curl -sS --max-time 30 -o /dev/null -X DELETE -H "mcp-session-id: $SID" "$U/mcp" || true
until_state first "$RUN" '.state == "succeeded"'
jq -e '.issue_claim.standing == "released" and (.output.ended | contains("detached"))' "$S/first.now.json" >/dev/null \
  || { echo "    the run did not end with its worker and give its claim back:"; jq -c '{state, output, issue_claim}' "$S/first.now.json"; exit 1; }

echo "    pending with nobody attached; invalid_intent and claim_conflict created nothing; $RUN held $KEY in $TREE, answered twice, released when its worker left"
