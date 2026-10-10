# majordomus-covers: knowledge capture
# majordomus-negative: knowledge
# claim: an-observation-becomes-a-grouped-candidate
# What a session observed becomes a candidate at its end, and the same thing observed by
# several sessions reads as one group (ADR 0118).
#
# Four provider sessions open and end through the hooks, and nobody asks for a derivation:
#
#   e1  observes two things — a defect in lib/a.sh and friction with scripts/build — and its
#       end derives two candidates, each saying what it is about; a second derivation over the
#       same ledger writes nothing
#   e2  observes lib/a.sh again, in other words: its end writes one file of its own, and the
#       listing shows one group for lib/a.sh with both episodes behind it
#   e3  observes lib/a.sh a third time after e1's record was rejected: the rejection stays,
#       and the new candidate is listed beside it in the same group
#   e4  observes nothing: its end writes no candidate and still records that the episode was
#       evaluated (`knowledge.derived` with `written: 0`) — the NO_CHANGE of an ordinary session
#
# Every file stays one episode's own; nothing is read, merged and written back.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "${TMPDIR:-/tmp}/mj.pol.$$" \
  && cp "${TMPDIR:-/tmp}/mj.pol.$$" .ai/repo/policy.yaml && rm -f "${TMPDIR:-/tmp}/mj.pol.$$"
"$MJ" capture install >/dev/null
mkdir -p lib scripts; printf 'echo a\n' > lib/a.sh; printf 'echo build\n' > scripts/build
git add -A >/dev/null 2>&1; git commit -qm "the layer, the hooks and two files" >/dev/null 2>&1 || true
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
LEDGER="$STATE/ledger.jsonl"
CAND=.ai/repo/knowledge/candidates
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.observe.XXXXXX")"
start_event() { printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1; }
end_event()   { printf '{"session_id":"%s","reason":"clear"}' "$1" | ./.claude/hooks/majordomus-session-end >/dev/null 2>&1; }
sid_of() { sed -n 's/^session_id: //p' "$STATE/sessions-open/$1.yaml" "$STATE"/sessions-open/*"$1"* 2>/dev/null | head -n 1; }
records_of() { find "$CAND" -maxdepth 1 -name "$1-*.md" 2>/dev/null | LC_ALL=C sort; }
derived_written() { grep "\"event\":\"knowledge.derived\".*\"episode\":\"$1\"" "$LEDGER" | tail -n 1 | sed -n 's/.*"written":\([0-9]*\).*/\1/p'; }
groups() { "$MJ" knowledge candidates --json > "$S/c.json"; jq -c "$1" "$S/c.json"; }

# ---------------------------------------------------------------- e1: two observations
start_event e1; SID1="$(sid_of e1)"
[ -n "$SID1" ] || { echo "    the start event opened no episode"; exit 1; }
"$MJ" knowledge observe --kind defect --subject lib/a.sh --evidence file:lib/a.sh \
  "a.sh prints the wrong word" >/dev/null
"$MJ" knowledge observe --kind friction --subject scripts/build "the build needs a step nobody wrote down" >/dev/null
end_event e1
R1="$(records_of "$SID1")"
[ "$(printf '%s\n' "$R1" | grep -c .)" = 2 ] || { echo "    e1 should derive two candidates:"; printf '%s\n' "$R1" | sed 's/^/    | /'; exit 1; }
[ "$(derived_written "$SID1")" = 2 ] || { echo "    e1's knowledge.derived does not say two were written"; exit 1; }
A1="$(grep -l '^about: "lib/a.sh"$' $R1)"
[ -n "$A1" ] || { echo "    no e1 candidate says it is about lib/a.sh"; exit 1; }
grep -q '^class: lesson$' "$A1" && grep -q '^epistemics: observed$' "$A1" && grep -q '^status: candidate$' "$A1" \
  || { echo "    the candidate is not an observed lesson awaiting review:"; sed 's/^/    | /' "$A1"; exit 1; }
grep -q '^    - file:lib/a.sh$' "$A1" || { echo "    the tracked evidence file is not in derived_from"; exit 1; }
"$MJ" knowledge check >/dev/null 2>&1 || { echo "    the derived candidates fail the integrity check:"; "$MJ" knowledge check 2>&1 | sed 's/^/    | /'; exit 1; }
# the same ledger, derived again: nothing new
"$MJ" knowledge derive --episode "$SID1" > "$S/again" 2>&1
grep -q '0 written' "$S/again" || { echo "    a second derivation wrote:"; sed 's/^/    | /' "$S/again"; exit 1; }

# ---------------------------------------------------------------- e2: the same subject again
start_event e2; SID2="$(sid_of e2)"
"$MJ" knowledge observe --kind repetition --subject lib/a.sh "a.sh is wrong again, in another way" >/dev/null
end_event e2
[ "$(records_of "$SID2" | grep -c .)" = 1 ] || { echo "    e2 should derive one file of its own"; exit 1; }
cmp -s "$A1" "$A1" && grep -q '^status: candidate$' "$A1" || { echo "    e2 touched e1's record"; exit 1; }
got="$(groups '[.groups[] | select(.about == "lib/a.sh") | {n: (.candidates|length), e: (.episodes|length)}]')"
[ "$got" = '[{"n":2,"e":2}]' ] || { echo "    lib/a.sh should be one group of two candidates from two episodes, got $got"; exit 1; }
got="$(groups '[.groups[] | .about] | sort')"
[ "$got" = '["lib/a.sh","scripts/build"]' ] || { echo "    expected two groups, got $got"; exit 1; }

# ---------------------------------------------------------------- e3: after a rejection
"$MJ" knowledge reject "$(basename "$A1" .md)" --reason "the word was right; the reader was wrong" >/dev/null
start_event e3
"$MJ" knowledge observe --kind defect --subject lib/a.sh "a.sh prints the wrong word" >/dev/null
end_event e3
grep -q '^status: superseded$' "$A1" || { echo "    the rejected record was reopened"; exit 1; }
got="$(groups '[.groups[] | select(.about == "lib/a.sh") | {n: (.candidates|length), r: (.rejected|length), e: (.episodes|length)}]')"
[ "$got" = '[{"n":2,"r":1,"e":3}]' ] || { echo "    the rejection and the later observation should share the group, got $got"; exit 1; }

# ---------------------------------------------------------------- e4: nothing observed
start_event e4; SID4="$(sid_of e4)"
end_event e4
[ -z "$(records_of "$SID4")" ] || { echo "    an episode that observed nothing derived a candidate"; exit 1; }
[ "$(derived_written "$SID4")" = 0 ] || { echo "    an episode that observed nothing left no knowledge.derived with written 0"; exit 1; }

# ---------------------------------------------------------------- the text listing says the same
"$MJ" knowledge candidates > "$S/text"
grep -q '^about lib/a.sh  route project  awaiting 2  rejected 1  episodes 3$' "$S/text" \
  || { echo "    the text listing does not show the group:"; sed 's/^/    | /' "$S/text"; exit 1; }
# ---------------------------------------------------------------- the Rust reader groups alike
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
norm='[.groups[] | {about, route, candidates: (.candidates // []), rejected: (.rejected // []), episodes: (.episodes // [])}]'
# What the reader said and what it complained of are kept apart and both shown on a failure:
# with its stderr discarded, a reader that refused the fixture read as one that found nothing.
rc=0; MAJORDOMUS_SHARE="$ROOT/share" "$RB" knowledge candidates --discovery filesystem --format json > "$S/rust.json" 2> "$S/rust.err" || rc=$?
[ "$rc" = 0 ] || { echo "    the Rust reader exited $rc:"; sed 's/^/    | /' "$S/rust.err" "$S/rust.json"; exit 1; }
jq -c "$norm" "$S/rust.json" > "$S/rust" 2> "$S/rust.jq" \
  || { echo "    the Rust reader's answer has no groups to read:"; sed 's/^/    | /' "$S/rust.jq" "$S/rust.json"; exit 1; }
"$MJ" knowledge candidates --json | jq -c "$norm" > "$S/shell"
cmp -s "$S/rust" "$S/shell" || { echo "    the shell and the Rust reader group differently:"; diff "$S/shell" "$S/rust" | sed 's/^/    | /'; exit 1; }
rm -rf "$S"
exit 0
