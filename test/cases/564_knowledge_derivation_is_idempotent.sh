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
