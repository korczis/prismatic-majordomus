# majordomus-covers: knowledge capture
# claim: knowledge-derivation-is-idempotent
# A second derivation over the same evidence writes nothing, says so, and leaves the tree alone.
#
# The deriver runs at every compaction and every close, and a provider compacts as often as
# it likes. A writer that produced a new file each time would turn a review queue into a
# pile of duplicates within a day, and a writer that rewrote the same bytes would still
# dirty every checkout that pulled them. So the record id is derived from the episode and a
# digest of the evidence, the file is named by the id, and a re-run over the same ledger
# compares before it writes. This case proves the three consequences a reader depends on:
# the ledger says `unchanged`, `git status` is empty, and the bytes are the same.
#
# The same boundary holds under repetition and race (I3416): an observation recorded twice
# in one episode is one candidate, a task finished partial yields none while the episode's
# end still writes its knowledge.derived line, and two episodes of one checkout closing at
# once each write exactly their own candidate and leave no staging file behind.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "${TMPDIR:-/tmp}/mj.pol.$$" \
  && cp "${TMPDIR:-/tmp}/mj.pol.$$" .ai/repo/policy.yaml && rm -f "${TMPDIR:-/tmp}/mj.pol.$$"
"$MJ" capture install >/dev/null
# The layer and the hooks are committed before the episode opens. A commit made inside the
# episode that touches .ai/repo/rules/ is evidence of its own — every record of the episode
# gains a `commit:` reference to it — so a scaffold committed after the start event would
# change the record between the two runs and the case would be measuring its own fixture.
git add -A >/dev/null 2>&1; git commit -qm "the layer and the hooks" >/dev/null 2>&1 || true
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
LEDGER="$STATE/ledger.jsonl"
CAND=.ai/repo/knowledge/candidates
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.knowledge.XXXXXX")"
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
event_count() { grep -c "\"event\":\"$1\"" "$LEDGER" 2>/dev/null || true; }
start_event() { printf '{"session_id":"%s","source":"%s"}' "$1" "${2:-startup}" | ./.claude/hooks/majordomus-session-start; }
compact_event() { printf '{"session_id":"%s","trigger":"auto"}' "$1" | ./.claude/hooks/majordomus-session-compact; }
sid_of() { sed -n 's/^session_id: //p' "$STATE/sessions-open/$1.yaml" | head -n 1; }
# the newest knowledge.derived line, as written
last_derived() { grep '"event":"knowledge.derived"' "$LEDGER" | tail -n 1; }

# ---------------------------------------------------------------- the first derivation
start_event e1 >/dev/null 2>&1
SID="$(sid_of e1)"
must "the start event opened no episode" [ -n "$SID" ]
"$MJ" start "compact twice" --scope lib >/dev/null
"$MJ" decision add "A record is named by its evidence" --why "so that a re-run finds the file it wrote" >/dev/null

compact_event e1 > "$S/c1.out" 2> "$S/c1.err"
[ -s "$S/c1.out" ] && { echo "    the compact hook wrote to stdout:"; sed 's/^/    | /' "$S/c1.out"; exit 1; }
REC="$(find "$CAND" -maxdepth 1 -name "$SID-*.md" 2>/dev/null | head -n 1)"
[ -n "$REC" ] || { echo "    the compaction derived no candidate"; sed 's/^/    | /' "$S/c1.err"; exit 1; }
[ "$(event_count knowledge.derived)" = 1 ] || { printf '    one compaction left %s knowledge.derived line(s)\n' "$(event_count knowledge.derived)"; exit 1; }
must "the first derivation did not count one written record" \
  grep -q "\"episode\":\"$SID\".*\"written\":1" <<<"$(last_derived)"
SUM="$(sha256_of_file "$REC")"

# Committed, so that the working tree is a thing `git status` can pronounce on: a candidate
# is tracked content, and until it is added the tree is dirty for the right reason. Only the
# knowledge store is added: this commit is inside the episode, and one that touched the
# rules or the ADRs would rightly enter the record's provenance.
git add .ai/repo/knowledge >/dev/null; git commit -qm "the candidate the episode derived" >/dev/null
must "the tree is not clean after the commit" [ -z "$(git status --porcelain)" ]

# ---------------------------------------------------------------- the second compaction
compact_event e1 > "$S/c2.out" 2> "$S/c2.err"
[ -s "$S/c2.out" ] && { echo "    the compact hook wrote to stdout:"; sed 's/^/    | /' "$S/c2.out"; exit 1; }
[ "$(event_count knowledge.derived)" = 2 ] || { printf '    two compactions left %s knowledge.derived line(s)\n' "$(event_count knowledge.derived)"; exit 1; }
printf '    the second derivation said:\n    | %s\n' "$(last_derived)"
must "the second derivation claims to have written something" grep -q '"written":0' <<<"$(last_derived)"
must "the second derivation did not count the record it left unchanged" grep -q '"unchanged":1' <<<"$(last_derived)"
dirty="$(git status --porcelain)"
[ -z "$dirty" ] || { echo "    the second derivation changed the working tree:"; printf '%s\n' "$dirty" | sed 's/^/    | /'; exit 1; }
[ "$(sha256_of_file "$REC")" = "$SUM" ] || { echo "    the record's bytes changed on a re-run over the same evidence"; exit 1; }
[ "$(find "$CAND" -maxdepth 1 -name '*.md' ! -name README.md | wc -l | tr -d ' ')" = 1 ] || {
  echo "    a second file appeared for the same evidence:"; ls -1 "$CAND" | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- the command by hand
# No provider session named: the open episode resolves through the pointer, which is what a
# person at a terminal gets. It says `unchanged` for the record and sums it up on the last
# line, and it leaves a ledger line saying the same.
expect_exit 0 "$MJ" knowledge derive
expect_grep "^unchanged ${REC#./}"
expect_grep "^knowledge derive: 0 written, 1 unchanged, 0 skipped for episode $SID"
[ "$(event_count knowledge.derived)" = 3 ] || { printf '    the command by hand left %s knowledge.derived line(s), expected 3\n' "$(event_count knowledge.derived)"; exit 1; }
[ "$(sha256_of_file "$REC")" = "$SUM" ] || { echo "    the record's bytes changed under knowledge derive by hand"; exit 1; }

# ---------------------------------------------------------------- --dry-run
# Says what it would do and does none of it: no file, no ledger line. Proven on new
# evidence, because a dry run over unchanged evidence cannot be told from a run that
# happened to write nothing.
"$MJ" decision add "A dry run writes nothing" --why "so that it can be run anywhere" >/dev/null
expect_exit 0 "$MJ" knowledge derive --dry-run
expect_grep "^unchanged ${REC#./}"
expect_grep "^would write $CAND/$SID-[0-9a-f]+\.md"
[ "$(event_count knowledge.derived)" = 3 ] || { echo "    --dry-run appended a knowledge.derived line"; exit 1; }
[ "$(find "$CAND" -maxdepth 1 -name '*.md' ! -name README.md | wc -l | tr -d ' ')" = 1 ] || {
  echo "    --dry-run wrote a file:"; ls -1 "$CAND" | sed 's/^/    | /'; exit 1; }
must "--dry-run changed the working tree" [ -z "$(git status --porcelain)" ]

# and the real run then writes exactly the file the dry run named
WOULD="$(printf '%s\n' "$LAST_OUT" | sed -n 's/^would write //p' | head -n 1)"
expect_exit 0 "$MJ" knowledge derive
expect_grep "^written $WOULD"
expect_grep "^knowledge derive: 1 written, 1 unchanged, 0 skipped for episode $SID"
expect_file "$WOULD"

# ---------------------------------------------------------------- an observation recorded twice
# The same fact stated twice in one episode is one candidate, because its canonical evidence
# is the kind, the subject and the statement, and never the second it was recorded. Two
# ledger lines, one file: the first row writes it and the second finds the same bytes there.
# A digest that took in the timestamp would give each line a file of its own, and this is
# the block that would say so.
# Every command below names itself when it fails: under bash -eu an unguarded one ends the
# case with nothing printed.
loud() {
  local why="$1" out rc=0; shift
  out="$("$@" 2>&1)" || rc=$?
  [ "$rc" = 0 ] || { printf '    %s (exit %s):\n' "$why" "$rc"; printf '%s\n' "$out" | sed 's/^/    | /'; exit 1; }
}
OBS="the deriver reads every line of the ledger"
before="$(find "$CAND" -maxdepth 1 -name '*.md' ! -name README.md | wc -l | tr -d ' ')"
obs_lines() { grep '"event":"observation.recorded"' "$LEDGER" | grep -F "\"session\":\"$SID\"" | grep -F "\"statement\":\"$OBS\"" || true; }
loud "the first observe was refused" "$MJ" knowledge observe --kind repetition --subject lib "$OBS"
# The second line is written in a later second than the first, so that the two lines differ
# in their timestamp and a digest that read it could not pass by coincidence.
t1="$(obs_lines | head -n 1 | sed -n 's/^{"ts":"\([^"]*\)".*/\1/p')"
while [ "$(date -u +%Y-%m-%dT%H:%M:%SZ)" = "$t1" ]; do sleep 0.2; done
loud "the second observe was refused" "$MJ" knowledge observe --kind repetition --subject lib "$OBS"
seen="$(obs_lines | grep -c . || true)"
[ "$seen" = 2 ] || { printf '    two observe calls left %s observation.recorded line(s), expected 2\n' "$seen"; exit 1; }
[ "$(obs_lines | sed -n 's/^{"ts":"\([^"]*\)".*/\1/p' | LC_ALL=C sort -u | grep -c .)" = 2 ] \
  || { echo "    the two observation lines carry one timestamp, so they cannot show the digest ignores it"; exit 1; }
expect_exit 0 "$MJ" knowledge derive
OBSREC="$(grep -lx 'about: "lib"' "$CAND"/"$SID"-*.md 2>/dev/null || true)"
[ "$(printf '%s\n' "$OBSREC" | grep -c .)" = 1 ] || {
  echo "    an observation recorded twice should derive one candidate, found:"
  printf '%s\n' "$OBSREC" | sed 's/^/    | /'; printf '%s\n' "$LAST_OUT" | sed 's/^/    > /'; exit 1; }
expect_grep "^written ${OBSREC#./}"
expect_grep "^unchanged ${OBSREC#./}"
after="$(find "$CAND" -maxdepth 1 -name '*.md' ! -name README.md | wc -l | tr -d ' ')"
[ "$after" = $((before + 1)) ] || { printf '    the twice-recorded observation took the store from %s to %s files\n' "$before" "$after"; exit 1; }

# ---------------------------------------------------------------- a task finished partial
# A partial outcome is not a lesson and not a verified fact, so it yields no candidate; the
# episode's end still evaluates it, and says so on the ledger with a knowledge.derived line.
loud "the candidates could not be staged" git add .ai/repo/knowledge
loud "the candidates could not be committed" git commit -qm "the candidates before the partial finish"
printf '# Objective\n\ncompact twice\n\n# Current State\n\nhalf of it\n\n# Next Action\n\nthe other half\n' > "$S/partial.md"
loud "finish --outcome partial was refused" "$MJ" finish --outcome partial --note "$S/partial.md"
grep -q '"event":"task.finished".*"outcome":"partial"' "$LEDGER" \
  || { echo "    the partial finish left no task.finished line with outcome partial"; exit 1; }
end_event() { printf '{"session_id":"%s","reason":"clear"}' "$1" | ./.claude/hooks/majordomus-session-end; }
derived_of() { grep '"event":"knowledge.derived"' "$LEDGER" | grep -cF "\"episode\":\"$1\"" || true; }
n_before="$(derived_of "$SID")"
rc=0; end_event e1 > "$S/e1.out" 2> "$S/e1.err" || rc=$?
[ "$rc" = 0 ] || { printf '    the end hook of e1 exited %s:\n' "$rc"; sed 's/^/    | /' "$S/e1.err"; exit 1; }
[ "$(derived_of "$SID")" = $((n_before + 1)) ] || {
  echo "    the end of the episode that finished partial wrote no knowledge.derived line"; sed 's/^/    | /' "$S/e1.err"; exit 1; }
printf '    the end after the partial finish said:\n    | %s\n' "$(last_derived)"
must "the end after a partial finish wrote a candidate" grep -q '"written":0' <<<"$(last_derived)"
[ "$(find "$CAND" -maxdepth 1 -name '*.md' ! -name README.md | wc -l | tr -d ' ')" = "$after" ] || {
  echo "    a task finished partial derived a candidate:"; git status --porcelain -- "$CAND" | sed 's/^/    | /'; exit 1; }
! grep -lx '  - task' "$CAND"/*.md >/dev/null 2>&1 || {
  echo "    a candidate was derived from a task outcome:"; grep -lx '  - task' "$CAND"/*.md | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- two episodes closing at once
# Two provider sessions of one checkout, each with an observation of its own, end in
# parallel. Each derivation reads only its own episode's lines and stages inside the
# directory before it renames, so each writes exactly its own candidate, the store passes
# its integrity check, and no staging file is left behind.
loud "the start hook of e2 failed" start_event e2; SID2="$(sid_of e2)"
loud "the start hook of e3 failed" start_event e3; SID3="$(sid_of e3)"
[ -n "$SID2" ] && [ -n "$SID3" ] && [ "$SID2" != "$SID3" ] \
  || { printf '    two start events should open two episodes, got "%s" and "%s"\n' "$SID2" "$SID3"; exit 1; }
loud "e2's observe was refused" env MAJORDOMUS_PROVIDER_SESSION=e2 "$MJ" knowledge observe --kind friction --subject lib "the second window met this"
loud "e3's observe was refused" env MAJORDOMUS_PROVIDER_SESSION=e3 "$MJ" knowledge observe --kind friction --subject lib "the third window met that"
end_event e2 > "$S/e2.out" 2> "$S/e2.err" &
p2=$!
end_event e3 > "$S/e3.out" 2> "$S/e3.err" &
p3=$!
r2=0; wait "$p2" || r2=$?; r3=0; wait "$p3" || r3=$?
[ "$r2" = 0 ] && [ "$r3" = 0 ] || {
  printf '    the parallel end hooks exited %s and %s:\n' "$r2" "$r3"; sed 's/^/    e2| /' "$S/e2.err"; sed 's/^/    e3| /' "$S/e3.err"; exit 1; }
for pair in "$SID2:the second window met this" "$SID3:the third window met that"; do
  sid="${pair%%:*}"; said="${pair#*:}"
  own="$(find "$CAND" -maxdepth 1 -name "$sid-*.md" | LC_ALL=C sort)"
  [ "$(printf '%s\n' "$own" | grep -c .)" = 1 ] || {
    printf '    episode %s should leave exactly one candidate, found:\n' "$sid"; printf '%s\n' "$own" | sed 's/^/    | /'
    sed 's/^/    e2| /' "$S/e2.err"; sed 's/^/    e3| /' "$S/e3.err"; exit 1; }
  grep -qF "# $said" "$own" || { printf '    the candidate of %s does not carry its own observation:\n' "$sid"; sed 's/^/    | /' "$own"; exit 1; }
  [ "$(derived_of "$sid")" = 1 ] || { printf '    episode %s left %s knowledge.derived line(s), expected 1\n' "$sid" "$(derived_of "$sid")"; exit 1; }
done
[ "$(find "$CAND" -maxdepth 1 -name '*.md' ! -name README.md | wc -l | tr -d ' ')" = $((after + 2)) ] || {
  echo "    two episodes closing at once did not add exactly two candidates:"; ls -1 "$CAND" | sed 's/^/    | /'; exit 1; }
expect_exit 0 "$MJ" knowledge check
left="$(find "$CAND" -name '.tmp.*')"
[ -z "$left" ] || { echo "    a staging file was left under candidates/:"; printf '%s\n' "$left" | sed 's/^/    | /'; exit 1; }
rm -rf "$S"
exit 0
