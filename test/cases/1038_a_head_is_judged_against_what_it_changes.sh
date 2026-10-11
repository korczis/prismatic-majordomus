# majordomus-covers: none
# A head is judged against what it changes, not against GitHub's live state
# (project.github-projection-gated, claim projected-record-edit-is-landable).
#
# `scripts/github-sync --apply` projects the trunk and nothing else, and nothing runs it
# automatically. So a record a pull request edits is `behind` on GitHub for as long as the
# pull request is open, and the gate that refused every `behind` made such a pull request red
# from birth: #855 changed one line of I1900 and could not go green on its own head, #820
# closed eight records and met the same wall. After a landing the trunk itself would have
# been red until somebody applied by hand.
#
# This drives `scripts/ci/github-check` over a fixture repository with a base, a head and two
# landings, against a scripted `gh` that serves the remote and refuses everything else. Every
# assertion names the mutation of the gate that breaks it; the five marked RUN were applied
# to the gate on 2026-10-10, one at a time, and each failed this case where it says.
. "$ROOT/test/lib.sh"
# A fixture's git must answer about the fixture: under a hook these name another repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX

GATE="$ROOT/scripts/ci/github-check"
SYNC="$ROOT/scripts/github-sync"
fail() { printf '    %s\n' "$*"; exit 1; }

# ---------------------------------------------------------------- the scripted forge
# The only `gh` the gate and the adapter can find. It answers the three reads a check makes
# and fails on anything else, so a case that tried to write to GitHub would fail here rather
# than succeed there.
FORGE="$T/forge"; GH_LOG="$T/gh.log"; FX_I="$T/issues.tsv"; FX_M="$T/ms.tsv"; BASE="$T/baseline.txt"
mkdir -p "$FORGE"; : > "$GH_LOG"
cat > "$FORGE/gh" <<'GH'
#!/bin/sh
printf '%s\n' "$*" >> "$GH_LOG"
case "$*" in
  "auth status"*) exit 0 ;;
  "api --paginate repos/example/fixture/milestones?"*) cat "$FX_M" ;;
  "api --paginate repos/example/fixture/issues?"*) cat "$FX_I" ;;
  *) printf 'UNEXPECTED %s\n' "$*" >> "$GH_LOG"; echo "scripted gh: unexpected call: $*" >&2; exit 1 ;;
esac
GH
chmod +x "$FORGE/gh"
export GH_LOG FX_I FX_M

# The gate as a run of one kind sees it: the event variables are this call's and nobody
# else's, because the suite itself runs inside a pull request's or a push's environment.
#   gate_as [NAME=value ...]
gate_as() {
  env -u GITHUB_EVENT_NAME -u GITHUB_BASE_REF -u GITHUB_HEAD_REF -u GITHUB_REF -u MJ_GH_BASE \
    -u MJ_GH_REMOTE -u MJ_GH_FIXTURE_ISSUES -u MJ_GH_FIXTURE_MILESTONES \
    PATH="$FORGE:$PATH" MJ_ROOT="$PWD" MJ_GH_BASELINE="$BASE" "$@" "$GATE"
}
pull_request() { gate_as GITHUB_EVENT_NAME=pull_request GITHUB_BASE_REF=master GITHUB_REF=refs/pull/1/merge; }
trunk_push()   { gate_as GITHUB_EVENT_NAME=push GITHUB_REF=refs/heads/master; }

# ---------------------------------------------------------------- the base
# Three issues under one milestone; I0002 waits for I0001, so its status is derived from a
# file that is not its own. The window is the fixture's own declaration, as the gate reads
# it from the tree under test.
"$MJ" init >/dev/null
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000 I0001
pj_issue I0003 M000
mkdir -p .ai/repo/ci
printf 'version: 1\napply:\n  window: 1800\n' > .ai/repo/ci/github.yaml
printf 'missing 0\nadopt 0\n' > "$BASE"

now="$(date +%s)"
commit_at() { # <seconds ago> <message>
  git add .gitignore .ai >/dev/null
  GIT_AUTHOR_DATE="$((now - $1)) +0000" GIT_COMMITTER_DATE="$((now - $1)) +0000" git commit -qm "$2" >/dev/null
}
commit_at 2592000 "the plan at the base"
base="$(git rev-parse HEAD)"
git update-ref refs/remotes/origin/master "$base"

# what GitHub holds: every record as the base renders it, and one older rendering of I0001
printf '1\tM000 — Milestone M000\topen\n' > "$FX_M"
row() {
  b="$(base64 | tr -d '\n')"
  printf '%s\t%s\topen\t%s\tM000 — Milestone M000\t%s\n' "$1" "$2" "$b" "$3"
}
for id in I0001 I0002 I0003; do "$SYNC" --render "$id" > "$T/at-base.$id"; done
cp .ai/repo/project/issues/I0001.yaml "$T/I0001.keep"
sed 's/^objective: .*/objective: "What GitHub was last told."/' "$T/I0001.keep" > .ai/repo/project/issues/I0001.yaml
"$SYNC" --render I0001 > "$T/stale.I0001"
cp "$T/I0001.keep" .ai/repo/project/issues/I0001.yaml
remote() { # <rendering of I0001> — I0002 and I0003 as the base rendered them
  { row 7 'I0001 — Issue I0001' I0001 < "$1"
    row 8 'I0002 — Issue I0002' I0002 < "$T/at-base.I0002"
    row 9 'I0003 — Issue I0003' I0003 < "$T/at-base.I0003"; } > "$FX_I"
}
remote "$T/at-base.I0001"

# in agreement, on a pull request's run and on the trunk's: nothing pends and nothing is said
expect_exit 0 pull_request
expect_grep 'OK   github-check  no record is behind'
expect_grep '^github-check: 0 finding\(s\)$'
expect_no_grep 'PENDING|NOTE'
expect_exit 0 trunk_push
expect_no_grep 'PENDING|NOTE'

# ---------------------------------------------------------------- 1. a head, the record it changed
# The head edits I0003. GitHub holds what the base said, which is all it can hold.
git checkout -q -b the-pull-request
sed 's/^objective: .*/objective: "Changed by the head."/' .ai/repo/project/issues/I0003.yaml > "$T/i.$$" \
  && mv "$T/i.$$" .ai/repo/project/issues/I0003.yaml
commit_at 1728000 "the head changes I0003"

# MUTATION, RUN: remove the head half of decide_reference (`KIND="head"` -> `KIND="none"`, so
# a head has no reference) -> the gate exits 10 with `FAIL ... 1 record(s) behind` here.
expect_exit 0 pull_request
expect_grep 'PENDING github-check  1 record\(s\) behind, changed by this head since its merge base with origin/master'
expect_grep 'projected after merge'
expect_grep 'DRIFT  behind +issue I0003 \(#9\)'
expect_grep '^github-check: 0 finding\(s\), 1 pending$'
expect_no_grep 'FAIL'
# a person names the base the same way, with no event at all
expect_exit 0 gate_as MJ_GH_BASE="$base"
expect_grep 'PENDING github-check  1 record\(s\) behind'
# and with neither, git is asked: a tree the remote's default branch does not contain is a head
expect_exit 0 gate_as
expect_grep 'PENDING github-check  1 record\(s\) behind'

# ---------------------------------------------------------------- 2. a head, a record it did not change
# GitHub is behind on I0001 as well, and the head never touched I0001. That drift was there
# before the head and is refused exactly as it always was; the head's own stays pending.
# MUTATION, RUN: make split_lines call every record changed (`($4 in c) ?` -> `1 ?`) -> the
# gate exits 0 here with both records pending.
remote "$T/stale.I0001"
expect_exit 10 pull_request
expect_grep 'FAIL github-check  1 record\(s\) behind'
expect_grep 'this head did not change them'
expect_grep 'DRIFT  behind +issue I0001 \(#7\)'
expect_grep 'PENDING github-check  1 record\(s\) behind'
expect_grep '^github-check: 1 finding\(s\), 1 pending$'
remote "$T/at-base.I0001"

# ---------------------------------------------------------------- 3. a head, a record it moved without touching
# The head closes I0001. I0001's file changed; I0002's did not, and its body moved all the
# same, because it stopped waiting. Both are the head's change, and so is I0001 being open
# on GitHub and closed canonically.
# MUTATION, RUN: never call changed_renderings (`changed_renderings ||` -> `true ||`, judging
# by files alone) -> the gate exits 10 here with `FAIL ... 1 record(s) behind` naming I0002.
expect_exit 0 "$MJ" plan start I0001
expect_exit 0 "$MJ" plan verify I0001
expect_exit 0 "$MJ" plan evidence I0001 --covers proof --type test --command "true" --result "exit 0"
expect_exit 0 "$MJ" plan "done" I0001
commit_at 1641600 "the head closes I0001"
git diff --quiet "$base" -- .ai/repo/project/issues/I0002.yaml || fail 'the fixture is wrong: the head touched I0002.yaml'
expect_exit 0 pull_request
expect_grep 'PENDING github-check  3 record\(s\) behind'
expect_grep 'DRIFT  behind +issue I0002 \(#8\)'
expect_grep 'PENDING github-check  1 record\(s\) state'
expect_grep 'I0001 \(#7\) is open on GitHub, canonically closed'
expect_grep '^github-check: 0 finding\(s\), 2 pending$'

# a state an apply does not write is never pending, whoever changed the record: a person's
# text inside the generated region of I0003 is refused on the head that changed I0003
# MUTATION: add `edited` to PENDABLE -> the gate exits 0 here.
{ row 7 'I0001 — Issue I0001' I0001 < "$T/at-base.I0001"
  row 8 'I0002 — Issue I0002' I0002 < "$T/at-base.I0002"
  "$SYNC" --render I0003 | sed 's/^| status |.*/| status | somebody typed this |/' | row 9 'I0003 — Issue I0003' I0003
} > "$FX_I"
expect_exit 10 pull_request
expect_grep 'FAIL github-check  1 record\(s\) edited'
remote "$T/at-base.I0001"

# ---------------------------------------------------------------- 4. the trunk, inside the window
# The head lands now. Its commits were written weeks ago; what is young is the landing, and
# the landing is the first-parent commit.
# MUTATION, RUN: drop `--first-parent` from the rev-list in decide_reference (the reference
# becomes the pull request's own old commit) -> the gate exits 10 here, all four refused.
# MUTATION: make the reference HEAD itself (`--min-age` removed) -> the same failure.
git checkout -q -b landed-just-now "$base"
git merge -q --no-ff the-pull-request -m "land the head" >/dev/null
expect_exit 0 trunk_push
expect_grep 'PENDING github-check  3 record\(s\) behind, landed on master less than 1800s ago'
expect_grep 'the executor applies after landing'
expect_grep 'PENDING github-check  1 record\(s\) state'
expect_grep '^github-check: 0 finding\(s\), 2 pending$'
expect_no_grep 'FAIL'

# ---------------------------------------------------------------- 5. the trunk, past the window
# The same landing, two hours old, and nobody applied. Refused, and the refusal names what
# to run.
# MUTATION, RUN: remove the age test (`REF=""` after the rev-list, so every landing is
# young) -> the gate exits 0 here with all four pending.
git checkout -q -b landed-long-ago "$base"
GIT_AUTHOR_DATE="$((now - 7200)) +0000" GIT_COMMITTER_DATE="$((now - 7200)) +0000" \
  git merge -q --no-ff the-pull-request -m "land the head" >/dev/null
expect_exit 10 trunk_push
expect_grep 'FAIL github-check  3 record\(s\) behind'
expect_grep 'FAIL github-check  1 record\(s\) state'
expect_grep 'inside the 1800s the executor is given after a landing'
expect_grep 'run: scripts/github-sync --apply from master'
expect_no_grep 'PENDING'

# the bound is the declaration's, read from the tree under test: the same two-hour-old
# landing is inside a window of a day
# MUTATION: read the window from anywhere but the declaration -> one of these two fails.
printf 'version: 1\napply:\n  window: 86400\n' > .ai/repo/ci/github.yaml
expect_exit 0 trunk_push
expect_grep 'landed on master less than 86400s ago'
git checkout -q .ai/repo/ci/github.yaml
# and this repository declares one, which the gate carries no copy of
declared="$(sed -n 's/^ *window: *\([0-9][0-9]*\) *$/\1/p' "$ROOT/.ai/repo/ci/github.yaml")"
[ -n "$declared" ] || fail '.ai/repo/ci/github.yaml declares no apply.window'
if grep -qw -- "$declared" "$ROOT/scripts/ci/github-check"; then
  fail "scripts/ci/github-check carries the literal $declared; the window is declared in .ai/repo/ci/github.yaml and read from there"
fi

# ---------------------------------------------------------------- 6. what cannot be measured is not a pass
# No declared window: no landing can be called young, the gate says so and refuses.
# MUTATION: default the window when the declaration is missing -> the gate exits 0 here.
git checkout -q landed-just-now
rm .ai/repo/ci/github.yaml
expect_exit 10 trunk_push
expect_grep 'NOTE github-check  what this tree changed could not be measured: .ai/repo/ci/github.yaml declares no apply.window'
expect_grep 'FAIL github-check  3 record\(s\) behind'
expect_no_grep 'PENDING'
git checkout -q .ai/repo/ci/github.yaml
# a base that does not resolve is not "the head changed nothing", and is not "everything"
git checkout -q the-pull-request
expect_exit 10 gate_as MJ_GH_BASE=no-such-ref
expect_grep 'NOTE github-check  what this tree changed could not be measured: the base no-such-ref does not resolve here'
expect_grep 'FAIL github-check  3 record\(s\) behind'
expect_exit 10 gate_as GITHUB_EVENT_NAME=pull_request GITHUB_BASE_REF=gone GITHUB_REF=refs/pull/1/merge
expect_grep 'the base origin/gone does not resolve here'
# nor is a clone that cannot see the default branch at all
git update-ref -d refs/remotes/origin/master
expect_exit 10 gate_as
expect_grep 'NOTE github-check  what this tree changed could not be measured: origin/master does not resolve here'
expect_no_grep 'PENDING'

# ---------------------------------------------------------------- the forge was only read
grep -q '^api --paginate repos/example/fixture/issues' "$GH_LOG" || fail 'the scripted gh was never asked for the issues: the gate read something else'
expect_no_grep 'UNEXPECTED' "$GH_LOG"
if grep -vE '^(auth status|api --paginate repos/example/fixture/(milestones|issues)\?)' "$GH_LOG" | grep -q .; then
  fail "the gate made a call that is not a read: $(grep -vE '^(auth status|api --paginate)' "$GH_LOG" | head -1)"
fi
