# majordomus-covers: knowledge capture
# majordomus-negative: knowledge
# claim: knowledge-derived-at-the-boundary
# The provider's end event derives what the episode learned, whether or not a task exists.
#
# An episode records decisions and finishes tasks, and each leaves a typed line in the
# ledger that only this machine can read until the ledger's retention cap removes it. The
# end event is the moment that evidence stops being reachable, so it is the moment the
# evidence becomes a candidate record under the tracked tree (ADR 0058). Nothing here reads
# a conversation: the record is composed from the ledger line, byte for byte, and the same
# line yields the same record on every machine.
#
# Two episodes close below. The first never opened a task, which is the shape that stopped
# every writer of this tool on 2026-09-05 (ADR 0052): a derivation gated on a task would
# have written nothing, and a record with `decision:none` in it would have named a task
# nobody opened. The second finished its task with a verification command and left it
# finished, so the derivation is proven against a task in every state a guard could have
# refused. One decision carries a double quote and a backslash, because the ledger stores
# them escaped and a record that escaped them twice would neither parse nor say what was
# decided.
. "$ROOT/test/lib.sh"

# The suite is itself developed inside a provider session. A case that drives named
# provider sessions must not silently be a member of the one running it.
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
# the start event would ensure a shared server; that is case 108's, and the log line it
# leaves when no executable is built is a doctor finding this case must not inherit
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "${TMPDIR:-/tmp}/mj.pol.$$" \
  && cp "${TMPDIR:-/tmp}/mj.pol.$$" .ai/repo/policy.yaml && rm -f "${TMPDIR:-/tmp}/mj.pol.$$"
"$MJ" capture install >/dev/null
# everything the scaffold wrote is committed, so that the scope check a completed finish
# runs sees only what the episode itself changed
git add -A >/dev/null 2>&1; git commit -qm "the layer and the hooks" >/dev/null 2>&1 || true
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
LEDGER="$STATE/ledger.jsonl"
CAND=.ai/repo/knowledge/candidates
# scratch outside the repository: a file written into the checkout is a changed file, and
# the scope check at finish would name it
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.knowledge.XXXXXX")"
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
event_count() { grep -c "\"event\":\"$1\"" "$LEDGER" 2>/dev/null || true; }
start_event() { printf '{"session_id":"%s","source":"%s"}' "$1" "${2:-startup}" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"%s"}' "$1" "${2:-clear}" | ./.claude/hooks/majordomus-session-end; }
# the episode a provider session opened, by the identity the writer stamps on every line
sid_of() { sed -n 's/^session_id: //p' "$STATE/sessions-open/$1.yaml" | head -n 1; }
# the candidate files of one episode: the id starts with the episode it came from
records_of() { find "$CAND" -maxdepth 1 -name "$1-*.md" 2>/dev/null | LC_ALL=C sort; }
# the ledger line number of an event of one episode; 0 when there is none
line_of() { grep -n "\"event\":\"$1\".*\"$2\":\"$3\"" "$LEDGER" | head -n 1 | cut -d: -f1; }

# ---------------------------------------------------------------- (a) no task at all
# The decision is recorded outside any task, so its ledger line carries `task_id: none`.
# The record it yields names the episode and nothing else: `decision:none` would be a
# reference to a task nobody opened, and the integrity check refuses it.
start_event e1 >/dev/null 2>&1
SID1="$(sid_of e1)"
must "the start event opened no episode for e1" [ -n "$SID1" ]
must "a decision outside a task was refused" \
  "$MJ" decision add "Records are one assertion each" --why "a record that says two things cannot be verified once" >/dev/null
[ -f "$STATE/current.yaml" ] && { echo "    a task record exists, and this half is about the episode without one"; exit 1; }

end_event e1 clear > "$S/end1.out" 2> "$S/end1.err"
[ -s "$S/end1.out" ] && { echo "    the end hook wrote to stdout:"; sed 's/^/    | /' "$S/end1.out"; exit 1; }
must "the episode did not close" [ ! -f "$STATE/sessions-open/e1.yaml" ]

R1="$(records_of "$SID1")"
[ -n "$R1" ] || {
  echo "    the end event derived no candidate for episode $SID1"
  sed 's/^/    | /' "$S/end1.err"; ls -1 "$CAND" 2>/dev/null | sed 's/^/    | /'; exit 1; }
[ "$(printf '%s\n' "$R1" | wc -l | tr -d ' ')" = 1 ] || { echo "    one decision yielded more than one record:"; printf '%s\n' "$R1" | sed 's/^/    | /'; exit 1; }
printf '    derived outside a task:\n'; sed 's/^/    | /' "$R1"

must "the record is not a candidate" grep -q '^status: candidate$' "$R1"
must "the record does not say the deriver wrote it" grep -q '^  origin: extracted$' "$R1"
must "the record does not name its episode" grep -q "^    - session:$SID1$" "$R1"
must "the record is not a convention (every derived decision is one; a person sets the class at promotion)" \
  grep -q '^class: convention$' "$R1"
must "the record does not carry the decision as its title" grep -qF 'title: "Records are one assertion each"' "$R1"
grep -qE '^    - (decision|task):' "$R1" && {
  echo "    a decision recorded outside a task was attributed to one:"; grep -E '^    - ' "$R1" | sed 's/^/    | /'; exit 1; }
grep -q 'none' "$R1" && { echo "    the literal 'none' entered the record"; grep -n none "$R1" | sed 's/^/    | /'; exit 1; }

# The directory arrived with its contract (ADR 0011): a directory of the layer without a
# README is a tree the tool refuses, and the deriver is the thing that created it.
must "the deriver created the candidates directory without its README" [ -f "$CAND/README.md" ]

# The evidence that it ran, before the evidence that the episode closed: the close derives
# first, so a reader comparing episode ids finds the derivation for every closed episode.
D1="$(line_of knowledge.derived episode "$SID1")"
C1="$(line_of session.closed session "$SID1")"
[ -n "$D1" ] || { echo "    no knowledge.derived line names episode $SID1"; grep knowledge "$LEDGER" | sed 's/^/    | /'; exit 1; }
[ -n "$C1" ] || { echo "    no session.closed line names episode $SID1"; exit 1; }
[ "$D1" -lt "$C1" ] || { printf '    knowledge.derived (line %s) follows session.closed (line %s) for one episode\n' "$D1" "$C1"; exit 1; }
must "the derivation did not count one written record" \
  grep -q "\"event\":\"knowledge.derived\".*\"episode\":\"$SID1\".*\"written\":1" "$LEDGER"

# ---------------------------------------------------------------- (b) a finished task
# The task is completed with a verification command and left finished. Until ADR 0052 a
# finished task turned every writer of this tool off; the deriver is the episode's, and a
# completed task whose verification passed is itself a fact worth keeping.
start_event e2 >/dev/null 2>&1
SID2="$(sid_of e2)"
must "the start event opened no episode for e2" [ -n "$SID2" ]
"$MJ" start "prove the boundary" --scope lib >/dev/null
TASK="$(sed -n 's/^id: //p' "$STATE/current.yaml" | head -n 1)"
must "the task record has no id" [ -n "$TASK" ]
must "a decision with a quote and a backslash was refused" \
  "$MJ" decision add 'Paths are quoted as "a\b" on every surface' --why "the escaping is the test" >/dev/null
printf '# Objective\n\nProve the boundary.\n\n# Current State\n\nDone.\n\n# Next Action\n\nNothing.\n' > "$S/note.md"
expect_exit 0 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
must "the task is not finished" grep -q '^outcome: completed$' "$STATE/current.yaml"

end_event e2 clear > "$S/end2.out" 2> "$S/end2.err"
[ -s "$S/end2.out" ] && { echo "    the end hook wrote to stdout:"; sed 's/^/    | /' "$S/end2.out"; exit 1; }
must "the episode with a finished task did not close" [ ! -f "$STATE/sessions-open/e2.yaml" ]

R2="$(records_of "$SID2")"
[ "$(printf '%s\n' "$R2" | sed '/^$/d' | wc -l | tr -d ' ')" = 2 ] || {
  echo "    a decision and a verified completion should yield two records for $SID2:"
  printf '%s\n' "$R2" | sed 's/^/    | /'; sed 's/^/    | /' "$S/end2.err"; exit 1; }
must "the derivation did not count two written records" \
  grep -q "\"event\":\"knowledge.derived\".*\"episode\":\"$SID2\".*\"written\":2" "$LEDGER"

DEC="$(grep -l '^class: convention$' $R2 | head -n 1)"
FACT="$(grep -l '^class: fact$' $R2 | head -n 1)"
[ -n "$DEC" ] || { echo "    no convention record for the decision"; exit 1; }
[ -n "$FACT" ] || { echo "    no fact record for the verified completion"; exit 1; }
printf '    derived from a finished task:\n'; sed 's/^/    | /' "$DEC" "$FACT"

must "the decision record does not name the task it was decided under" grep -q "^    - decision:$TASK$" "$DEC"
must "the decision record does not name its episode" grep -q "^    - session:$SID2$" "$DEC"
must "the decision record is not decided" grep -q '^epistemics: decided$' "$DEC"
# Escaped once. The ledger holds \"a\\b\"; the record holds the same text inside YAML
# double quotes, which is the same escaping applied exactly once — not twice, not zero times.
must "the quote and the backslash did not survive as one YAML escaping" \
  grep -qF 'title: "Paths are quoted as \"a\\b\" on every surface"' "$DEC"

must "the fact record does not name the task" grep -q "^    - task:$TASK$" "$FACT"
must "the fact record is not observed" grep -q '^epistemics: observed$' "$FACT"
must "the fact record does not say what verified it" grep -qF 'verified by: true (exit 0)' "$FACT"

# What was written parses and validates: a record the integrity check refuses is a record
# nobody will promote, and the deriver is the one writer that must never produce one.
expect_exit 0 "$MJ" knowledge check
expect_no_grep '^FAIL'

# ---------------------------------------------------------------- the surface
expect_exit 2 "$MJ" knowledge bogus
expect_grep 'unknown subcommand'
