# majordomus-covers: decision question checkpoint check session handover knowledge finish evidence capture
# majordomus-negative: decision question checkpoint check handover knowledge finish evidence session
# I0701's original no-episode counterexample and ADR 0164's two real episodes.
# A record belongs to its caller. The active task is related unless another episode that is
# still open opened it; nothing is refused, and nobody advances somebody else's task. With
# no episode at all the workers cannot be told apart, and the task is the caller's (the
# answer accepted with ADR 0164): the cost the ADR names, paid without inventing identity.
# The records that decide whether work is done — evidence, a gate verdict, an outcome — have
# no meaning without a task, so those three refuse by name instead, with the command that
# resolves it; a hand-run session close that cannot tell which episode is its own refuses
# too, and an observation no episode would derive names its rule and a remedy that works
# (I4292).
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID MJ_SESSION_KEY
MAJORDOMUS_RUNTIME=off; export MAJORDOMUS_RUNTIME

fixture() {
  mkdir -p "$1"; cd "$1" || exit 1
  git init -q .; git config user.email t@example.com; git config user.name Test
  git commit -q --allow-empty -m init
  "$MJ" init >/dev/null; "$MJ" update >/dev/null
  mkdir -p lib; echo a > lib/a; git add .; git commit -qm base
}
must() { local why="$1"; shift; "$@" || { echo "    $why"; return 1; }; }
a() { MAJORDOMUS_PROVIDER_SESSION=worker-a-0701 USER=shared-user "$@"; }
b() { MAJORDOMUS_PROVIDER_SESSION=worker-b-0701 USER=shared-user "$@"; }
unknown() { MAJORDOMUS_PROVIDER_SESSION=missing-0701 USER=shared-user "$@"; }
ledger=.ai/local/state/ledger.jsonl
current=.ai/local/state/current.yaml
task_id() { sed -n 's/^id: //p' "$current"; }
checkpoint_at() { sed -n 's/^checkpoint_at: //p' "$current"; }
# Inspect the public record, not a resolver's internal variables. An omitted field must
# actually be absent: neither an empty string nor a literal "none" is a ledger relation.
event_is() {
  LAST_EVENT="$(jq -c --arg event "$1" 'select(.event == $event)' "$ledger" | tail -n 1)"
  if ! printf '%s\n' "$LAST_EVENT" | jq -e --arg session "$2" --arg task "$3" '
      (if $session == "" then has("session") | not else .session == $session end) and
      (if $task == "" then has("task_id") | not else .task_id == $task end)' >/dev/null; then
    printf '    %s should name session=%s task=%s, got: %s\n' "$1" "$2" "$3" "$LAST_EVENT"
    return 1
  fi
  must "the ledger contains no $1 event" test -n "$LAST_EVENT"
}
checkpoint_record() {
  CP="$(printf '%s\n' "$LAST_EVENT" | jq -r '.checkpoint_path // empty')"
  must "the checkpoint event names no durable record" test -n "$CP"
  expect_file "$CP"
  expect_grep "^task_id: $1$" "$CP"
}

# ---------------------------------------------------------------- no episode, as originally reproduced
(
  fixture "$T/no-episode"
  expect_exit 0 a "$MJ" start "A task without an episode" --scope lib
  id="$(task_id)"; must "start wrote no task id" test -n "$id"
  event_is task.started "" "$id"
  expect_exit 0 b "$MJ" decision add "B without an episode" --why "no episode tells B from A"
  event_is decision.recorded "" "$id"
  expect_exit 0 "$MJ" decision show "B without an episode"
  expect_grep "^Task: $id$"
  expect_exit 0 b "$MJ" question add "B without an episode asks"
  event_is question.opened "" "$id"
  expect_grep "^- \\[unresolved\\] $id — B without an episode asks" .ai/local/state/open-questions.md
  expect_exit 0 b "$MJ" checkpoint <<'BODY'
B without an episode made progress.
BODY
  event_is task.checkpoint "" "$id"; checkpoint_record "$id"
  expect_grep 'B without an episode made progress' "$CP"
)

# ---------------------------------------------------------------- one worker, no identity, no episode
# Nobody else is there to misattribute to: its records keep its task, as before ADR 0164.
(
  fixture "$T/alone"
  expect_exit 0 "$MJ" start "A lone worker's task" --scope lib
  id="$(task_id)"; must "start wrote no task id" test -n "$id"
  expect_exit 0 "$MJ" decision add "The lone worker decided" --why "nobody else is here"
  event_is decision.recorded "" "$id"
  expect_exit 0 "$MJ" decision show "The lone worker decided"
  expect_grep "^Task: $id$"
  expect_exit 0 "$MJ" checkpoint <<'BODY'
The lone worker made progress.
BODY
  event_is task.checkpoint "" "$id"; checkpoint_record "$id"
)

# ---------------------------------------------------------------- the executable's writers (I4291)
# ADR 0164 was implemented in the shell; the executable's writers — the ledger line of every
# plan transition (the MCP tool included), continuity publish and reasoning record — read the
# checkout's pointer, which names whoever opened an episode last. The same two episodes, the
# same questions: each record names its caller, B's record does not relate the task A
# opened, an anonymous caller among two episodes is nobody, and an MCP connection that
# attached an episode names that episode whatever its process environment says.
if RB="$(rust_bin)"; then
(
  fixture "$T/rust-writers"
  pj_init; pj_milestone M000 0
  for i in I0001 I0002 I0003 I0004; do pj_issue "$i" M000; done
  git add -A >/dev/null; git commit -qm plan >/dev/null
  MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
  # a device key of the fixture's own, never this machine's; the shell tool on PATH is what
  # an attach's episode driver runs
  HOME="$T/rust-home"; XDG_STATE_HOME="$T/rust-state"; export HOME XDG_STATE_HOME
  mkdir -p "$HOME"; PATH="$ROOT/bin:$PATH"; export PATH
  anon() { env -u MAJORDOMUS_PROVIDER_SESSION -u CLAUDE_CODE_SESSION_ID -u MJ_SESSION_KEY USER=shared-user "$@"; }
  expect_exit 0 "$MJ" session start --provider-session worker-a-0701 --worker worker-a
  sid_a="$(sed -n 's/^session_id: //p' .ai/local/state/sessions-open/worker-a-0701.yaml)"
  must "A has no actual episode" test -n "$sid_a"
  expect_exit 0 a "$MJ" start "A task the executable writes beside" --scope lib
  id="$(task_id)"; must "A has no task id" test -n "$id"
  event_is task.started "$sid_a" "$id"
  expect_exit 0 "$MJ" session start --provider-session worker-b-0701 --worker worker-b
  sid_b="$(sed -n 's/^session_id: //p' .ai/local/state/sessions-open/worker-b-0701.yaml)"
  must "two episodes were folded together" test "$sid_a" != "$sid_b"

  # One MCP session over the real pipes (case 133's frames); the last result on stdout. The
  # tool's own verdict is in the result: a refusal is a result carrying isError.
  frame() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}' "$1" "$2" "$3"; }
  mcp() { # <wrapper> <frame>... — one frame per argument, one line each
    local w="$1"; shift
    { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3049","version":"0"}}}\n'
      printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
      printf '%s\n' "$@"
    } | "$w" "$RB" mcp 2>"$T/mcp.err" > "$T/mcp.out" \
      || { echo "    the executable's MCP server failed under $w:"; sed 's/^/    | /' "$T/mcp.err"; return 1; }
    must "the transition under $w was refused: $(tail -n 1 "$T/mcp.out" | jq -r '.result.content[0].text // empty')" \
      test "$(tail -n 1 "$T/mcp.out" | jq -r '.result.isError')" = false
  }
  move() { frame 2 majordomus_plan_transition "$(jq -nc --arg i "$1" '{issue:$i,transition:"start"}')"; }
  # B's transition names B; A's names A; an anonymous caller among two episodes is nobody.
  mcp b "$(move I0001)"
  event_is plan_start "$sid_b" ""
  mcp a "$(move I0002)"
  event_is plan_start "$sid_a" ""
  mcp anon "$(move I0003)"
  event_is plan_start "" ""
  must "the executable's plan transition read the pointer" \
    test "$(jq -c 'select(.event == "plan_start" and .session == "'"$sid_b"'")' "$ledger" | wc -l | tr -d ' ')" = 1

  # continuity publish: the record names the publishing caller, not the pointer
  MAJORDOMUS_NOW=2026-09-01T12:00:20Z; export MAJORDOMUS_NOW
  printf '# Objective\n\nB publishes.\n\n# Current State\n\nB is beside A.\n\n# Next Action\n\nB continues.\n' \
    | b "$MJ" handover >/dev/null
  unset MAJORDOMUS_NOW
  b "$RB" continuity publish --format json > "$T/pub-b.json" 2>"$T/pub-b.err" \
    || { echo "    B's publish failed:"; sed 's/^/    | /' "$T/pub-b.err"; exit 1; }
  must "B's published handover names $(jq -c .record.session "$T/pub-b.json"), not B ($sid_b)" \
    test "$(jq -r .record.session "$T/pub-b.json")" = "$sid_b"
  must "B's published handover relates A's task: $(jq -c .record.task "$T/pub-b.json")" \
    test "$(jq -r '.record.task // empty' "$T/pub-b.json")" = ""
  MAJORDOMUS_NOW=2026-09-01T12:00:30Z; export MAJORDOMUS_NOW
  printf '# Objective\n\nA publishes.\n\n# Current State\n\nA owns the task.\n\n# Next Action\n\nA continues.\n' \
    | a "$MJ" handover >/dev/null
  unset MAJORDOMUS_NOW
  a "$RB" continuity publish --format json > "$T/pub-a.json" 2>"$T/pub-a.err" \
    || { echo "    A's publish failed:"; sed 's/^/    | /' "$T/pub-a.err"; exit 1; }
  must "A's published handover names $(jq -c .record.session "$T/pub-a.json"), not A ($sid_a)" \
    test "$(jq -r .record.session "$T/pub-a.json")" = "$sid_a"
  must "A's published handover lost A's task: $(jq -c .record.task "$T/pub-a.json")" \
    test "$(jq -r '.record.task // empty' "$T/pub-a.json")" = "A task the executable writes beside"

  # reasoning record: the record names its caller and relates the task only when it is the
  # caller's; an anonymous caller among two episodes names nobody and no task
  record_as() { # <wrapper> <subject> -> the record on stdout
    printf '{"kind":"assessment","subject":"%s"}' "$2" | "$1" "$RB" reasoning record --format json 2>"$T/rz.err" \
      || { echo "    the reasoning record of $2 was refused:"; sed 's/^/    | /' "$T/rz.err"; return 1; }
  }
  record_as b "B wonders beside A" > "$T/rz-b.json" || exit 1
  must "B's reasoning record names $(jq -c .record.session "$T/rz-b.json"), not B ($sid_b)" \
    test "$(jq -r .record.session "$T/rz-b.json")" = "$sid_b"
  must "B's reasoning record was filed under A's task: $(jq -c .record.task "$T/rz-b.json")" \
    test "$(jq -r .record.task "$T/rz-b.json")" = untasked
  record_as a "A wonders in its own task" > "$T/rz-a.json" || exit 1
  must "A's reasoning record names $(jq -c .record.session "$T/rz-a.json"), not A ($sid_a)" \
    test "$(jq -r .record.session "$T/rz-a.json")" = "$sid_a"
  must "A's reasoning record lost A's task: $(jq -c .record.task "$T/rz-a.json")" \
    test "$(jq -r .record.task "$T/rz-a.json")" = "$id"
  record_as anon "Nobody wonders" > "$T/rz-anon.json" || exit 1
  must "an anonymous reasoning record names an episode: $(jq -c .record.session "$T/rz-anon.json")" \
    test "$(jq -r '.record | has("session")' "$T/rz-anon.json")" = false
  must "an anonymous reasoning record was filed under A's task" \
    test "$(jq -r .record.task "$T/rz-anon.json")" = untasked
  # and the stored file says the same as the answer
  must "the stored record of B is under A's task" test -d .ai/local/state/reasoning/untasked
  must "the stored record of B names no session" \
    test "$(jq -r '.session' .ai/local/state/reasoning/untasked/"$(jq -r .record.id "$T/rz-b.json")".json)" = "$sid_b"
  # A connection that attached A's episode names A, although its process environment is B's:
  # the record is the connection's, not the serving process's. Last, because the connection
  # ending closes the episode it attached — A's — through the same lifecycle a provider's
  # hook runs, and everything above needs A still open.
  mcp b "$(frame 2 majordomus_session_attach '{"external_id":"worker-a-0701"}')" \
        "$(frame 3 majordomus_plan_transition '{"issue":"I0004","transition":"start"}')"
  event_is plan_start "$sid_a" ""
  echo "    ok: the executable's writers name their caller as the shell's do"
)
else
  rc=$?
  [ "$rc" = 3 ] || exit 1
  printf '    skip: no executable of this tree and no cargo to build one, so the executable'"'"'s writers were not measured here\n'
fi

# ---------------------------------------------------------------- two real episodes, the same OS user
fixture "$T/two-episodes"
# one gate over lib/, so a gate verdict has something to be about
mkdir -p .ai/repo/ci
printf 'version: 1\ngates:\n  - id: unit\n    job: suite\n    runs: "true"\n    summary: unit\n' \
  > .ai/repo/ci/gates.yaml
printf 'classes:\n  - id: src\n    paths: [lib/**]\n    gates: [unit]\n' >> .ai/repo/ci/gates.yaml
git add -A; git commit -qm gates
MAJORDOMUS_NOW=2026-09-01T12:00:00Z; export MAJORDOMUS_NOW
expect_exit 0 "$MJ" session start --provider-session worker-a-0701 --worker worker-a
sid_a="$(sed -n 's/^session_id: //p' .ai/local/state/sessions-open/worker-a-0701.yaml)"
must "A has no actual episode" test -n "$sid_a"
expect_exit 0 a "$MJ" start "A task with its own history" --scope lib --requires tests
id="$(task_id)"; must "A has no task id" test -n "$id"
event_is task.started "$sid_a" "$id"
expect_exit 0 "$MJ" session start --provider-session worker-b-0701 --worker worker-b
sid_b="$(sed -n 's/^session_id: //p' .ai/local/state/sessions-open/worker-b-0701.yaml)"
must "B has no actual episode" test -n "$sid_b"
must "two episodes were folded together" test "$sid_a" != "$sid_b"
expect_exit 0 "$MJ" session status
expect_grep "Session: +$sid_b"

# A is still A while the pointer names B. Task ownership and authored identity are
# independent of USER, which is deliberately identical for every writer in this fixture.
expect_exit 0 a "$MJ" decision add "A while the pointer names B" --why "the explicit caller wins"
event_is decision.recorded "$sid_a" "$id"
expect_exit 0 a "$MJ" decision show "A while the pointer names B"
expect_grep "^Task: $id$"
expect_exit 0 a "$MJ" question add "A question resolved by B"
event_is question.opened "$sid_a" "$id"
expect_exit 0 b "$MJ" question resolve "A question resolved by B" --answer "B supplies the answer"
event_is question.resolved "$sid_b" ""
expect_grep "^- \\[resolved [0-9-]+\\] $id — A question resolved by B" .ai/local/state/open-questions.md

expect_exit 0 b "$MJ" decision add "B chose the narrower fix" --why "B is not the task opener"
event_is decision.recorded "$sid_b" ""
expect_exit 0 b "$MJ" decision show "B chose the narrower fix"
expect_grep '^Task: none$'
expect_exit 0 b "$MJ" question add "B question resolved by A"
event_is question.opened "$sid_b" ""
expect_grep '^- \[unresolved\] none — B question resolved by A' .ai/local/state/open-questions.md
expect_exit 0 a "$MJ" question resolve "B question resolved by A" --answer "A supplies the answer"
event_is question.resolved "$sid_a" "$id"
expect_grep '^- \[resolved [0-9-]+\] none — B question resolved by A' .ai/local/state/open-questions.md
expect_exit 0 "$MJ" question list --all
expect_grep 'A question resolved by B'
expect_grep 'B question resolved by A'

before="$(checkpoint_at)"
MAJORDOMUS_NOW=2026-09-01T12:00:10Z
expect_exit 0 b "$MJ" checkpoint <<'BODY'
B made progress while A owns the active task.
BODY
event_is task.checkpoint "$sid_b" ""; checkpoint_record none
expect_grep 'B made progress' "$CP"
must "B's checkpoint advanced A's task" test "$(checkpoint_at)" = "$before"
expect_exit 0 b "$MJ" checkpoint </dev/null
event_is task.checkpoint "$sid_b" ""
must "B's bodyless checkpoint advanced A's task" test "$(checkpoint_at)" = "$before"
expect_exit 0 b "$MJ" check --checkpoint
event_is task.checkpoint "$sid_b" ""
must "B's check --checkpoint advanced A's task" test "$(checkpoint_at)" = "$before"

# A commit made after A opened its task is visible in A's derived progress only. A
# foreign checkpoint must not claim that history even when its front matter says none.
echo changed >> lib/a; git add lib/a; git commit -qm Attribution-only-A-commit
expect_exit 0 b "$MJ" checkpoint --derive
event_is task.checkpoint "$sid_b" ""; checkpoint_record none
expect_no_grep 'since the task started|Attribution-only-A-commit' "$CP"
must "B's derived checkpoint advanced A's task" test "$(checkpoint_at)" = "$before"
expect_exit 0 a "$MJ" checkpoint --derive
event_is task.checkpoint "$sid_a" "$id"; checkpoint_record "$id"
expect_grep 'Commits since the task started' "$CP"
expect_grep 'Attribution-only-A-commit' "$CP"
must "A's own checkpoint did not advance its task" test "$(checkpoint_at)" = "$MAJORDOMUS_NOW"
MAJORDOMUS_NOW=2026-09-01T12:00:20Z
expect_exit 0 a "$MJ" check --checkpoint
event_is task.checkpoint "$sid_a" "$id"
must "A's own check --checkpoint did not advance its task" test "$(checkpoint_at)" = "$MAJORDOMUS_NOW"

# B's handover and observation belong to B: A's task is neither named nor closed by them.
MAJORDOMUS_NOW=2026-09-01T12:00:30Z
expect_exit 0 b "$MJ" handover --close <<'BODY'
# Objective

B hands over its own work.

# Current State

B worked beside A's task.

# Next Action

B's successor continues B's work.
BODY
event_is task.handed_over "$sid_b" ""
HO="$(printf '%s\n' "$LAST_EVENT" | jq -r '.handover_path')"
expect_grep '^task_id: none$' "$HO"
must "B's handover --close closed A's task" test "$(sed -n 's/^outcome: //p' "$current")" = active
must "B's handover advanced A's task" test "$(checkpoint_at)" = 2026-09-01T12:00:20Z
expect_exit 0 b "$MJ" knowledge observe --kind friction --subject lib "B met friction beside A's task"
event_is observation.recorded "$sid_b" ""
expect_exit 0 a "$MJ" knowledge observe --kind friction --subject lib "A met friction in its own task"
event_is observation.recorded "$sid_a" "$id"
# An observation nobody's episode would stamp is refused by its rule (ADR 0118), with the
# remedy that works for that caller: `session start` alone leaves an anonymous caller among
# two episodes as nameless as before.
obs_before="$(grep -c '"event":"observation.recorded"' "$ledger")"
expect_exit 10 "$MJ" knowledge observe --kind friction --subject lib "Nobody met friction"
expect_grep 'refused by adr-0118'
expect_grep 'MJ_SESSION_KEY=<key> majordomus knowledge observe'
expect_grep 'worker-a-0701, worker-b-0701'
expect_exit 10 unknown "$MJ" knowledge observe --kind friction --subject lib "Unknown met friction"
expect_grep 'refused by adr-0118'
expect_grep 'majordomus session start --provider-session missing-0701'
must "a refused observation was written" \
  test "$(grep -c '"event":"observation.recorded"' "$ledger")" = "$obs_before"

# No declared caller, or an explicit key with no episode: neither may borrow B's pointer.
for caller in unknown anonymous; do
  if [ "$caller" = unknown ]; then
    expect_exit 0 unknown "$MJ" decision add "Unknown caller decision" --why "missing means unknown"
  else
    expect_exit 0 "$MJ" decision add "Anonymous caller decision" --why "two episodes are ambiguous"
  fi
  event_is decision.recorded "" ""
  expect_exit 0 "$MJ" decision show "$([ "$caller" = unknown ] && echo Unknown || echo Anonymous) caller decision"
  expect_grep '^Task: none$'
done
expect_exit 0 "$MJ" session status
expect_grep "Session: +$sid_b"

# ---------------------------------------------------------------- the records that decide done
# Evidence, a gate verdict and an outcome mean nothing without a task, so where ADR 0164
# would drop the relation they refuse instead, naming the rule and the command that resolves
# it, and write nothing. A's own evidence, gate and finish name A's episode and A's task.
count_of() { grep -c "\"event\":\"$1\"" "$ledger" || true; }
refused_by_name() { # <what> <command...>: refused with the rule and the remedy, nothing written
  local what="$1"; shift
  local ev=0 gt=0 fin=0 ref=0
  ev="$(count_of task.evidence)"; gt="$(count_of task.gate)"
  fin="$(count_of task.finished)"; ref="$(count_of task.refused)"
  expect_exit 15 "$@" || { echo "    $what was not refused"; return 1; }
  expect_grep 'refused by adr-0164'
  expect_grep "task $id was opened by episode $sid_a"
  expect_grep 'MJ_SESSION_KEY=worker-a-0701 majordomus '
  expect_grep 'majordomus session close --provider-session worker-a-0701'
  local now
  now="$(count_of task.evidence) $(count_of task.gate)"
  now="$now $(count_of task.finished) $(count_of task.refused)"
  must "$what wrote a ledger line under A's task" test "$now" = "$ev $gt $fin $ref"
}
envelope_is() { # <event> <session> <field> <value>: the newest such line names them
  local line
  line="$(jq -c --arg e "$1" 'select(.event == $e)' "$ledger" | tail -n 1)"
  printf '%s\n' "$line" | jq -e --arg s "$2" --arg f "$3" --arg v "$4" \
    '.session == $s and .[$f] == $v' >/dev/null \
    || { printf '    %s should name session=%s %s=%s, got: %s\n' "$1" "$2" "$3" "$4" "$line"
         return 1; }
}

refused_by_name "B's evidence" b "$MJ" evidence --covers tests --command "sh test/unit.sh"
expect_grep 'majordomus evidence --covers tests --command "sh test/unit.sh"'
refused_by_name "an anonymous caller's evidence" "$MJ" evidence --covers tests --command true
expect_grep 'caller is no episode it can name \(2 are open here\)'
refused_by_name "B's gate verdict" b "$MJ" evidence --gate unit --exit 0 --command true
expect_grep 'majordomus evidence --gate unit --exit 0'
refused_by_name "B's finish" b "$MJ" finish --outcome blocked
expect_grep 'majordomus finish --outcome blocked'
refused_by_name "an anonymous caller's finish" "$MJ" finish --outcome blocked
must "a refused finish moved A's task" test "$(sed -n 's/^outcome: //p' "$current")" = active
expect_exit 0 a "$MJ" evidence --covers tests --command "sh test/unit.sh"
envelope_is task.evidence "$sid_a" task "$id"
expect_exit 0 a "$MJ" evidence --gate unit --exit 0 --command true
envelope_is task.gate "$sid_a" task "$id"

# ---------------------------------------------------------------- the mesh join
# A start event joins the mesh naming the task only when it is that session's to carry. The
# executable is a stand-in that says a server is ready and writes down what it was asked.
mkdir -p "$T/fake"
cat > "$T/fake/majordomus" <<'FAKE'
#!/bin/sh
case "$1 $2 $3" in
  "serve ensure"*) echo "ready http://127.0.0.1:9"; exit 0 ;;
  "mesh session open") printf '%s\n' "$*" >> "$MESH_LOG" ;;
esac
exit 0
FAKE
chmod +x "$T/fake/majordomus"
MESH_LOG="$T/mesh.log"; export MESH_LOG; : > "$MESH_LOG"
join_as() { # <worker wrapper> <provider session>
  printf '{"session_id":"%s","source":"resume"}' "$2" \
    | MAJORDOMUS_BIN="$T/fake/majordomus" "$1" "$MJ" capture session --provider claude-code \
      --event start >/dev/null 2>"$T/join.err" \
    || { echo "    the start event of $2 failed"; cat "$T/join.err"; return 1; }
  JOINED="$(grep -F -- "--session claude-code-$2 " "$MESH_LOG" | tail -n 1)"
  must "the start event of $2 did not join the mesh" test -n "$JOINED"
}
join_as b worker-b-0701
case "$JOINED" in *" --task "*) echo "    B joined the mesh claiming A's task: $JOINED"; exit 1 ;; esac
join_as a worker-a-0701
case "$JOINED" in *" --task $id "*) ;;
  *) echo "    A joined the mesh without its own task: $JOINED"; exit 1 ;; esac

# ---------------------------------------------------------------- finish counts its own checkpoints
printf '# Objective\n\nA stops.\n\n# Current State\n\nA is blocked.\n\n' > "$T/note.md"
printf '# Next Action\n\nA waits.\n' >> "$T/note.md"
mine="$(grep -l "^task_id: $id$" .ai/local/state/checkpoints/*.md | wc -l | tr -d ' ')"
all="$(find .ai/local/state/checkpoints -name '*.md' | wc -l | tr -d ' ')"
must "the fixture has no checkpoint outside A's task to miscount" test "$all" -gt "$mine"
expect_exit 0 a "$MJ" finish --outcome blocked --note "$T/note.md"
envelope_is task.finished "$sid_a" task_id "$id"
counted="$(jq -c 'select(.event == "task.finished") | .checkpoints' "$ledger" | tail -n 1)"
must "finish counted $counted checkpoints, of which $mine are its task's ($all in the store)" \
  test "$counted" = "$mine"

# ---------------------------------------------------------------- session close
# By hand, with two episodes open, a close names neither and is refused; each worker's own
# close names its episode, and B's record does not name the task A opened.
expect_exit 15 "$MJ" session close
expect_grep 'refused by adr-0164'
expect_grep '2 episodes are open in this checkout'
expect_grep 'majordomus session close --provider-session <key>, or MJ_SESSION_KEY=<key>'
must "a refused close closed an episode" test -f .ai/local/state/sessions-open/worker-a-0701.yaml
must "a refused close closed an episode" test -f .ai/local/state/sessions-open/worker-b-0701.yaml
expect_exit 0 b "$MJ" session close </dev/null
envelope_is session.closed "$sid_b" outcome closed
SR="$(jq -r 'select(.event == "session.closed") | .session_path' "$ledger" | tail -n 1)"
expect_file "$SR"
expect_grep '^task_id: none$' "$SR"
expect_grep "^session_id: $sid_b$" "$SR"
expect_exit 0 "$MJ" session close --provider-session worker-a-0701 </dev/null
envelope_is session.closed "$sid_a" outcome closed
SR="$(jq -r 'select(.event == "session.closed") | .session_path' "$ledger" | tail -n 1)"
expect_grep "^task_id: $id$" "$SR"
must "a ledger line carries the literal task_id none" \
  test "$(jq -c 'select(.task_id == "none")' "$ledger" | wc -l | tr -d ' ')" = 0
echo "    ok: records name their caller, task relations require that caller, and foreign checkpoints preserve the active task"
