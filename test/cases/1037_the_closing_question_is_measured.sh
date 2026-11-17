# majordomus-covers: check
# The closing question is answered by measurement (I2301,
# project.a-closing-question-is-answered-by-measurement).
#
# A person ending a session asks the same things each time: is any debt added, are there
# more branches and worktrees than at the start, are there more open pull requests. On
# 2026-10-10 one session was asked seven times and answered seven times from what it
# remembered doing; the eighth answer, measured, differed from all of them. So the three
# are questions of the done invariant now, each answered from the record that holds the
# fact, and this case walks them in a disposable repository:
#
#   no-new-debt          the entries of every .ai/repo/*-baseline.txt at the commit the task
#                        started at, against the working tree: an entry added refuses, an
#                        uncommitted one included; an entry removed does not; a baseline that
#                        appears is said and is not growth
#   nothing-accumulated  the branches and linked worktrees git's reflogs say were created
#                        since the task started and that still exist: owed while one
#                        remains, named, and answered once they are gone
#   backlog-not-grown    the recorded forge observation: a checkout that never observed the
#                        forge says so and is unknown, never a pass
#
# and `majordomus check` reports each one that is not answered by its id and its status.
# claims: the-closing-question-is-measured
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq not installed"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj1037.XXXXXX")"
trap 'git worktree remove --force "$S/wt" >/dev/null 2>&1 || true; rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null

# `majordomus check` asks the executable for the done invariant only where there is a CI
# model to judge, so the fixture declares the smallest one: a gate every plan selects.
mkdir -p .ai/repo/ci
cat > .ai/repo/ci/gates.yaml <<'YAML'
version: 1
gates:
  - id: lint
    job: structure
    always: true
    runs: "true"
    summary: every script parses
classes:
  - id: layer
    paths: [.ai/repo/**, AGENTS.md, CLAUDE.md, .gitignore]
    gates: [lint]
YAML

BASELINE=.ai/repo/fixture-baseline.txt
printf '# known debt, one entry a line\nREADME.md\tjust a\nREADME.md\tjust b\n' > "$BASELINE"
git add -A >/dev/null && git commit -qm base

export MAJORDOMUS_SHARE="$ROOT/share"
completion() { "$BIN" run gates.completion --input '{}' --quiet --format json --repo . 2>/dev/null | jq -c '.output'; }
question() { completion | jq -c --arg q "$1" '.questions[] | select(.id == $q)'; }
status_of() { question "$1" | jq -r '.status'; }
evidence_of() { question "$1" | jq -r '.evidence'; }
expect_status() {   # expect_status QUESTION STATUS WHEN
  local got; got="$(status_of "$1")"
  [ "$got" = "$2" ] || { echo "    $1 is $got, not $2 ($3): $(evidence_of "$1")"; exit 1; }
}
expect_evidence() { # expect_evidence QUESTION PATTERN
  evidence_of "$1" | grep -qE -- "$2" || { echo "    $1 does not say /$2/: $(evidence_of "$1")"; exit 1; }
}

# ---------------------------------------------------------------- 0. no task, no start
# with nothing to measure from, the three are unknown and say that, rather than pass
for q in no-new-debt nothing-accumulated backlog-not-grown; do
  expect_status "$q" unknown "before any task"
  expect_evidence "$q" 'no task record says where this work started'
done

# A reflog records whole seconds. The trunk was created a moment ago; a task started in the
# same second would count it as created since, so the start is put in a second of its own.
sleep 1
expect_exit 0 "$MJ" start "closing questions" --scope .ai/

[ "$(completion | jq -r '.questions | length')" = 22 ] \
  || { echo "    the done invariant is $(completion | jq -r '.questions | length') questions, not 22"; exit 1; }
for q in no-new-debt nothing-accumulated backlog-not-grown; do
  question "$q" | jq -e '(.source | length) > 0 and (.remediation | length) > 0 and (.evidence | length) > 0' >/dev/null \
    || { echo "    $q carries no source, remediation or evidence"; exit 1; }
done

# ---------------------------------------------------------------- 1. recorded debt
expect_status no-new-debt pass "nothing changed"
expect_evidence no-new-debt '1 baseline\(s\) held 2 entries when the task started and hold 2 now'

# an entry added and not committed is debt already
printf 'README.md\tjust c\n' >> "$BASELINE"
expect_status no-new-debt fail "an entry was added"
expect_evidence no-new-debt 'fixture-baseline\.txt 2 -> 3 \(\+1\)'
expect_exit 0 "$MJ" check
expect_grep 'the done invariant is not yet answered: .*no-new-debt=fail'

# committing it does not make it less of an addition: the start is the task's, not HEAD
git add -A >/dev/null && git commit -qm 'more debt'
expect_status no-new-debt fail "the added entry was committed"

# a comment and a blank line are not entries, and taking an entry out pays the debt back
printf '# known debt, one entry a line\n\nREADME.md\tjust a\n# b was settled\n' > "$BASELINE"
expect_status no-new-debt pass "an entry was removed"
expect_evidence no-new-debt 'held 2 entries when the task started and hold 1 now'

# a baseline that appears records debt that was there; it is said, and it is not growth
printf 'lib/x.sh\nlib/y.sh\n' > .ai/repo/new-gate-baseline.txt
expect_status no-new-debt pass "a baseline was recorded for the first time"
expect_evidence no-new-debt 'recorded for the first time: \.ai/repo/new-gate-baseline\.txt'
# while growth in another is still growth beside it
printf 'README.md\tjust a\nREADME.md\tjust b\nREADME.md\tjust c\nREADME.md\tjust d\n' > "$BASELINE"
expect_status no-new-debt fail "one baseline grew beside a new one"
expect_evidence no-new-debt '2 -> 4 \(\+2\)'
rm -f .ai/repo/new-gate-baseline.txt
printf '# known debt, one entry a line\nREADME.md\tjust a\nREADME.md\tjust b\n' > "$BASELINE"
expect_status no-new-debt pass "the baseline is as it was at the start"

# ---------------------------------------------------------------- 2. branches and worktrees
expect_status nothing-accumulated pass "nothing was created"
expect_evidence nothing-accumulated 'nothing created since .* remains'

git branch feature/kept
git worktree add -q -b feature/held "$S/wt"
expect_status nothing-accumulated queued "a branch and a worktree were created"
expect_evidence nothing-accumulated '1 worktree\(s\) and 2 branch\(es\) created since .* are still here'
expect_evidence nothing-accumulated 'feature/held'
expect_evidence nothing-accumulated '1 linked worktree\(s\) and 3 local branch\(es\) now'
expect_exit 0 "$MJ" check
expect_grep 'the done invariant is not yet answered: .*nothing-accumulated=queued'

# removing one leaves the other owed; removing both answers the question
git worktree remove "$S/wt"
expect_status nothing-accumulated queued "the worktree is gone and its branches are not"
expect_evidence nothing-accumulated '0 worktree\(s\) and 2 branch\(es\) created since'
git branch -q -D feature/held feature/kept
expect_status nothing-accumulated pass "everything created since is gone again"

# ---------------------------------------------------------------- 3. the backlog
# this checkout never observed the forge: unknown, with the reason and what would answer it
expect_status backlog-not-grown unknown "the forge was never observed"
expect_evidence backlog-not-grown 'the forge has never been observed from this checkout'
question backlog-not-grown | jq -r '.source' | grep -q 'majordomus prs refresh' \
  || { echo "    the backlog question does not name what records an observation"; exit 1; }

# ---------------------------------------------------------------- 4. the mutation
# If the questions were not read from where the task started, breaking that start would
# change nothing. So the task record is made to name a commit this repository does not
# have: the debt can no longer be compared, and the answer must stop being a pass.
expect_status no-new-debt pass "before the start is broken"
record=.ai/local/state/current.yaml
expect_file "$record"
cp "$record" "$S/current.yaml"
sed 's/^head: .*/head: 0123456789012345678901234567890123456789/' "$S/current.yaml" > "$record"
expect_status no-new-debt unknown "the task names a commit the repository does not have"
expect_evidence no-new-debt 'is not in this repository'
# and with a start that is not a moment, the two questions measured from it are unknown too
sed 's/^started_at: .*/started_at: this-morning/' "$S/current.yaml" > "$record"
expect_status nothing-accumulated unknown "the task's start is not a time"
expect_evidence nothing-accumulated 'not an RFC 3339'
expect_status no-new-debt pass "the commit is still readable when only the moment is not"
cp "$S/current.yaml" "$record"
expect_status no-new-debt pass "the record is restored"
expect_status nothing-accumulated pass "the record is restored"
