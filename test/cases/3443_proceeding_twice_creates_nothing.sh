# majordomus-timeout: 300
# majordomus-covers: none
# Proceeding claims and starts the chosen work once (I2974, ADR 0177 decision 3).
#
# `majordomus proceed take --session S` takes proceed.plan's selection: one exclusive mesh
# claim on the primary checkout's server, the issue's canonical worktree, a take record in
# it and the task bound to the issue started there. On a fixture with a declared mesh:
#   1. with no server for the primary checkout nothing is claimed or created (exit 11);
#   2. the first take claims I0001, creates feature/I0001-issue-I0001 and starts its task;
#   3. taking again from the primary checkout, and from the worktree, finds all of it and
#      creates no claim, worktree or task;
#   4. a second session is given the other issue;
#   5. after the primary server restarts, once the old run's claims have expired (three
#      heartbeats, the shortest expiry the declaration admits) the take re-claims and creates
#      nothing else;
#   6. after the task finishes, the take ends: its claim is released, nothing is started.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
RB="$(rust_bin)" || rust_bin_exit $?

R="$T/repo"; mkdir -p "$R"; R="$(cd "$R" && pwd -P)"
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
mkdir -p "$T/state"
export MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_LOG=warn XDG_STATE_HOME="$T/state"
S="$T/io"; mkdir -p "$S"
( cd "$R" && "$MJ" init >/dev/null && pj_init )
mkdir -p "$R/.ai/repo/project/intents" "$R/.ai/repo/project/critiques" "$R/test/cases" "$R/.ai/repo/mesh"
printf '. "$ROOT/test/lib.sh"\ntrue\n' > "$R/test/cases/01_probe.sh"
cat > "$R/.ai/repo/project/milestones/probe.yaml" <<'YAML'
id: probe
title: Milestone probe
slug: probe
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
for id in I0001 I0002; do
  cat > "$R/.ai/repo/project/issues/$id.yaml" <<YAML
id: $id
milestone: probe
title: Issue $id
slug: issue-$id
priority: p1
profile: implementation
objective: "Do the work of $id."
scope:
  - src/$id
serves:
  - probe#works
acceptance_criteria:
  - It is done
validation:
  - "true"
evidence_required:
  - done
YAML
done
cat > "$R/.ai/repo/project/intents/probe.yaml" <<'YAML'
id: probe
title: The probe's outcome is true
statement: "The outcome is true for the people it is for."
invariants:
  - The probe stays a valid repository
non_goals:
  - Speed
milestones:
  - probe
satisfaction:
  - id: works
    criterion: The probe works
    evidence: test
    ref: test/cases/01_probe.sh
YAML
cat > "$R/.ai/repo/project/critiques/probe.yaml" <<'YAML'
intent: probe
reviewed_at: HEAD
reviewed_by: case 3443
findings:
  - id: thin
    class: insufficient_work
    subject: probe#works
    finding: One case may not be enough
    blocking: true
    resolution:
      state: planned
      issue: I0001
YAML
{ printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n'
  printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n'
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 3\n  repository: case-3443\n'
} > "$R/.ai/repo/mesh/majordomus.yaml"
git -C "$R" add -A >/dev/null && git -C "$R" commit -qm base >/dev/null
W="$R-wt/feature/I0001-issue-I0001"
W2="$R-wt/feature/I0002-issue-I0002"

PID=""
cleanup() {
  for d in "$W" "$W2"; do [ -d "$d" ] && "$RB" serve stop --repo "$d" >/dev/null 2>&1; done
  [ -n "$PID" ] && kill "$PID" 2>/dev/null
  true
}
trap cleanup EXIT
serve_up() {
  rm -f "$R/.ai/local/state/mcp/server.json"
  ( cd "$R" && exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >>"$S/serve.log" 2>&1 ) &
  PID=$!
  local i=0
  until [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; do
    i=$((i + 1)); [ "$i" -lt 300 ] || { echo "    the server never published a lease:"; cat "$S/serve.log"; exit 1; }; sleep 0.2
  done
  url="$(jq -r '.url' "$R/.ai/local/state/mcp/server.json")"
}
take() { # <checkout> <session>
  rc=0; ( cd "$1" && "$RB" proceed take --session "$2" --format json ) > "$S/out" 2> "$S/err" || rc=$?
}
steps() { jq -c '[.steps[] | "\(.[0])=\(.[1])"]' "$S/out"; }
held() { # live exclusive claims of a session on the primary server
  curl -s "$url/api/v1/mesh/state?view=full&claims=live&session=$1" \
    | jq '[.state.claims[] | select(.mode == "exclusive" and .state.state == "held")] | length'
}
worktrees() { git -C "$R" worktree list | wc -l | tr -d ' '; }
tasks() { ls "$W/.ai/local/state/archive" 2>/dev/null | wc -l | tr -d ' '; }

# ---------------------------------------------------------------- 1. no server: nothing taken
take "$R" s1
[ "$rc" = 11 ] || { echo "    a take with no server must exit 11, got $rc:"; cat "$S/out" "$S/err"; exit 1; }
grep -q 'serve ensure' "$S/err" || { echo "    the refusal does not name the command that starts a server:"; cat "$S/err"; exit 1; }
[ "$(worktrees)" = 1 ] || { echo "    a refused take created a worktree"; git -C "$R" worktree list; exit 1; }

# ---------------------------------------------------------------- 2. the first take
serve_up
take "$R" s1
[ "$rc" = 0 ] || { echo "    the first take failed ($rc):"; cat "$S/out" "$S/err"; exit 1; }
jq -e --arg w "$W" '.issue == "I0001" and .worktree == $w and .branch == "feature/I0001-issue-I0001"
  and (.task | startswith("t-"))' "$S/out" >/dev/null \
  || { echo "    the first take is not I0001 in its canonical worktree:"; cat "$S/out"; exit 1; }
[ "$(steps)" = '["decision=created","claim=created","worktree=created","task=created"]' ] \
  || { echo "    the first take did not create each step: $(steps)"; exit 1; }
claim="$(jq -r .claim "$S/out")"; task="$(jq -r .task "$S/out")"
[ "$(sed -n 's/^issue: //p' "$W/.ai/local/state/current.yaml")" = I0001 ] \
  && [ "$(sed -n 's/^outcome: //p' "$W/.ai/local/state/current.yaml")" = active ] \
  || { echo "    the worktree holds no active task bound to I0001:"; cat "$W/.ai/local/state/current.yaml"; exit 1; }
jq -e --arg c "$claim" '.session == "s1" and .issue == "I0001" and .claim == $c' "$W/.ai/local/state/proceed.json" >/dev/null \
  || { echo "    the take record is not the take:"; cat "$W/.ai/local/state/proceed.json"; exit 1; }
[ "$(held s1)" = 1 ] && [ "$(worktrees)" = 2 ] || { echo "    after one take: $(held s1) claim(s), $(worktrees) worktree(s)"; exit 1; }

# ---------------------------------------------------------------- 3. again, from either checkout
for from in "$R" "$W"; do
  take "$from" s1
  [ "$rc" = 0 ] || { echo "    taking again from $from failed ($rc):"; cat "$S/out" "$S/err"; exit 1; }
  [ "$(steps)" = '["decision=found","claim=found","worktree=found","task=found"]' ] \
    || { echo "    taking again from $from created something: $(steps)"; cat "$S/out"; exit 1; }
  jq -e --arg c "$claim" --arg t "$task" '.claim == $c and .task == $t and .issue == "I0001"' "$S/out" >/dev/null \
    || { echo "    taking again from $from did not give back the same work:"; cat "$S/out"; exit 1; }
  [ "$(held s1)" = 1 ] && [ "$(worktrees)" = 2 ] && [ "$(tasks)" = 0 ] \
    || { echo "    taking again from $from: $(held s1) claim(s), $(worktrees) worktree(s), $(tasks) archived task(s)"; exit 1; }
done

# ---------------------------------------------------------------- 4. another session, the other issue
take "$R" s2
[ "$rc" = 0 ] && jq -e '.issue == "I0002"' "$S/out" >/dev/null \
  || { echo "    a second session was not given I0002 ($rc):"; cat "$S/out" "$S/err"; exit 1; }

# ---------------------------------------------------------------- 5. a restart ends the claim; the take re-claims
kill "$PID"; wait "$PID" 2>/dev/null || true; PID=""
serve_up
i=0; until [ "$(held s1)" = 0 ]; do
  i=$((i + 1)); [ "$i" -lt 100 ] || { echo "    the old run's claim never expired"; exit 1; }; sleep 0.2
done
take "$W" s1
[ "$rc" = 0 ] || { echo "    the take after a restart failed ($rc):"; cat "$S/out" "$S/err"; exit 1; }
[ "$(steps)" = '["decision=found","claim=reclaimed","worktree=found","task=found"]' ] \
  || { echo "    after a restart the take did not only re-claim: $(steps)"; cat "$S/out"; exit 1; }
jq -e --arg t "$task" '.task == $t' "$S/out" >/dev/null || { echo "    a restart changed the task"; cat "$S/out"; exit 1; }
[ "$(held s1)" = 1 ] && [ "$(worktrees)" = 3 ] || { echo "    after the restart: $(held s1) claim(s), $(worktrees) worktree(s)"; exit 1; }

# ---------------------------------------------------------------- 6. a finished task ends the take
printf '# Objective\nI0001\n# Current State\nstopped\n# Next Action\nnone\n' > "$S/note.md"
( cd "$W" && "$MJ" finish --outcome partial --note "$S/note.md" ) > "$S/finish" 2>&1 \
  || { echo "    finish failed:"; cat "$S/finish"; exit 1; }
take "$W" s1
[ "$rc" = 0 ] || { echo "    the take after finish failed ($rc):"; cat "$S/out" "$S/err"; exit 1; }
[ "$(steps)" = '["decision=found","claim=released","task=found"]' ] \
  || { echo "    after finish the take did not end: $(steps)"; cat "$S/out"; exit 1; }
[ "$(held s1)" = 0 ] && [ ! -f "$W/.ai/local/state/proceed.json" ] && [ "$(worktrees)" = 3 ] \
  || { echo "    after finish: $(held s1) claim(s), record $(ls "$W/.ai/local/state/")"; exit 1; }
[ -z "$(git -C "$R" status --porcelain)" ] || { echo "    a take changed the primary checkout:"; git -C "$R" status --porcelain; exit 1; }
exit 0
