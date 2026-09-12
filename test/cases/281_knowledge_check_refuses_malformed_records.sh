# majordomus-covers: knowledge check doctor
# majordomus-negative: knowledge check doctor
# claim: knowledge-integrity
# Every knowledge record is evidenced and well-formed, and every way it can fail is refused.
#
# A record is one assertion with provenance that resolves. The integrity check is what
# keeps that true once people and a deriver both write into the store, and a check is only
# worth its exit code if every clause has been shown to fire. Each malformed record below
# is written by hand, alone, so that the finding it produces is the finding for that clause
# and nothing else; a valid one then proves the check can pass. The same validator runs
# under `check` and `doctor`, and neither may be passing because it is unwired.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
CAND=.ai/repo/knowledge/candidates
CUR=.ai/repo/knowledge/curated
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.knowledge.XXXXXX")"
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
mkdir -p "$CAND"

# an episode the records can name: a reference that resolves is the baseline every
# malformed record below departs from in exactly one way
"$MJ" session start >/dev/null
SID="$(sed -n 's/^session_id: //p' "$STATE/session-current.yaml" | head -n 1)"
must "no open episode to reference" [ -n "$SID" ]

# record DIR ID STATUS [EXTRA-FRONT-MATTER-LINE] [BODY-LINE]
record() {
  local dir="$1" id="$2" status="$3" extra="${4:-}" body="${5:-Hand-made for the integrity check.}"
  {
    printf -- '---\nschema: knowledge/v1\nid: %s\nkind: knowledge\nclass: convention\n' "$id"
    printf 'title: "Every candidate is one assertion"\ndescription: "Hand-made for the integrity check."\n'
    printf 'status: %s\n' "$status"
    [ -n "$extra" ] && printf '%s\n' "$extra"
    printf 'epistemics: decided\ndate: 2026-09-12\ntags:\n  - derived\n  - decision\n'
    printf 'provenance:\n  origin: extracted\n  derived_from:\n    - session:%s\n---\n\n# Every candidate is one assertion\n\n%s\n' "$SID" "$body"
  } > "$dir/$id.md"
}
clear_store() { find "$CAND" "$CUR" -maxdepth 1 -name '*.md' ! -name README.md -delete; git add -A .ai/repo/knowledge >/dev/null; }
# one malformed record, refused with a finding that names the clause
refused() {   # pattern
  git add -A .ai/repo/knowledge >/dev/null
  expect_exit 10 "$MJ" knowledge check
  expect_grep "$1"
  expect_grep '^knowledge check: [0-9]+ record\(s\), [1-9][0-9]* failing'
  clear_store
}

# ---------------------------------------------------------------- one clause at a time
# verified under candidates/: only an act writes verified, and an act moves the file
record "$CAND" claims-verified verified
refused 'FAIL +knowledge .*claims-verified.*verified'

# a reference nothing in this checkout resolves
record "$CAND" dangling-episode candidate
sed "s/session:$SID/session:no-such-episode/" "$CAND/dangling-episode.md" > "$S/d" && cp "$S/d" "$CAND/dangling-episode.md"
grep -q 'session:no-such-episode' "$CAND/dangling-episode.md" || { echo "    the dangling probe did not take"; exit 1; }
refused 'FAIL +knowledge .*dangling-episode.*no-such-episode'

# a decision or a task named `none` is a task nobody opened
record "$CAND" names-no-task candidate
printf '    - decision:none\n' > "$S/none"
awk -v add="$(cat "$S/none")" '{ print } /^    - session:/ { print add }' "$CAND/names-no-task.md" > "$S/n" && cp "$S/n" "$CAND/names-no-task.md"
grep -q 'decision:none' "$CAND/names-no-task.md" || { echo "    the decision:none probe did not take"; exit 1; }
refused 'FAIL +knowledge .*names-no-task.*decision:none'

# one id in two files: the file name is the id, and the id is unique across both directories
record "$CAND" twin candidate
git add -A .ai/repo/knowledge >/dev/null
expect_exit 0 "$MJ" knowledge check
record "$CUR" twin candidate
refused 'FAIL +knowledge .*twin'

# an unknown key: a record with a field the schema does not have is stale or foreign
record "$CAND" unknown-key candidate 'episode: e1'
refused 'FAIL +knowledge .*unknown-key.*episode'

# a body that carries a conversation, in the body...
record "$CAND" body-transcript candidate '' 'assistant: yes, that is what we decided'
refused 'FAIL +knowledge .*body-transcript.*(conversation|transcript)'

# ...and in the title, which is the line every surface shows
record "$CAND" title-transcript candidate
sed 's/^title: .*/title: "assistant: every candidate is one assertion"/' "$CAND/title-transcript.md" > "$S/t" && cp "$S/t" "$CAND/title-transcript.md"
grep -q '^title: "assistant:' "$CAND/title-transcript.md" || { echo "    the title probe did not take"; exit 1; }
refused 'FAIL +knowledge .*title-transcript.*(conversation|transcript)'

# superseded by nothing and for no reason
record "$CAND" superseded-silently superseded
refused 'FAIL +knowledge .*superseded-silently.*(superseded_by|Rejected)'

# ---------------------------------------------------------------- and a valid one passes
record "$CAND" valid-candidate candidate
git add -A .ai/repo/knowledge >/dev/null
expect_exit 0 "$MJ" knowledge check
expect_no_grep '^FAIL'
expect_grep '^knowledge check: 1 record\(s\), 0 failing'

# ---------------------------------------------------------------- check and doctor
# The same validator, dispatched from the two commands the rule names. The pattern names
# the category and the finding, and the meta-check is asserted absent: a validator no rule
# declares is a different failure, and one that would satisfy a looser grep.
"$MJ" start "prove the check" --scope lib >/dev/null
record "$CAND" claims-verified verified
git add -A .ai/repo/knowledge >/dev/null

set +e
"$MJ" check > "$S/check.out" 2>&1; CHECK_EXIT=$?
"$MJ" doctor > "$S/doctor.out" 2>&1; DOCTOR_EXIT=$?
set -e
for f in check doctor; do
  grep -qE 'FAIL +knowledge .*claims-verified' "$S/$f.out" || {
    printf '    %s did not refuse the record that claims verified under candidates/\n' "$f"
    sed 's/^/    | /' "$S/$f.out"; exit 1; }
  grep -q 'validator exists but no rule declares it' "$S/$f.out" && {
    printf '    %s runs mj_validate_knowledge_integrity under no rule; the finding above is the meta-check\n' "$f"
    grep 'no rule declares' "$S/$f.out" | sed 's/^/    | /'; exit 1; }
done
[ "$CHECK_EXIT" = 10 ] || { printf '    check reported the finding and exited %s; a blocking rule exits 10\n' "$CHECK_EXIT"; exit 1; }
[ "$DOCTOR_EXIT" = 10 ] || { printf '    doctor reported the finding and exited %s; a blocking rule exits 10\n' "$DOCTOR_EXIT"; exit 1; }
printf '    doctor on a malformed record:\n'; grep -E 'knowledge' "$S/doctor.out" | sed 's/^/    | /'
