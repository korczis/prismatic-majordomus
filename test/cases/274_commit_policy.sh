# The commit message is a value this repository judges.
#
# claims: commit-message-is-judged, commit-scopes-are-learned, commit-plan-is-refusable
# Named here so the relation reads from both ends: docs/CLAIMS.yaml names this case as the
# proof of those three, and a claim whose test does not name it back is what
# scripts/ci/claim-proof-check refuses.
#
# No `majordomus-covers:` header: that declares coverage of a command of the *shell tool*,
# and everything here is the Rust executable's. What this case is the proof of is named the
# other way round, by the rule that cites it — `project.conventional-commits@2`.
#
# Every guarantee is proved against a real git repository built here: a message is written,
# the judge is asked, and what it says is asserted. Nothing is mocked — the vocabulary is
# learned from a history this case makes, the fingerprint is taken of a tree this case
# stages into, and the gate is run against a tree this case can make fail.
#
# The shape to look for: the tree is green, one fact changes, the thing that should refuse
# it goes red naming that fact. A guarantee no mutation tests is a guarantee nothing gives.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
# The share of the checkout this case belongs to, never another worktree's: the schemas and
# allow-lists a fixture is judged against must be the ones this tree declares.
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null

# A repository of its own, so that the history this case reasons about is the one it wrote.
git init -q .
git config user.email t@example.org
git config user.name Tester
git config commit.gpgsign false
mkdir -p apps/thing/src/alpha apps/thing/src/beta docs test/cases
commit() { git add -A; git -c core.hooksPath=/dev/null commit -q --no-verify -m "$1"; }
# Where this case's own history starts. The harness's fixture already carries a commit, and
# a range that included it would be measuring the harness rather than the policy.
BASE="$(git rev-parse HEAD 2>/dev/null || true)"
range() { if [ -n "$BASE" ]; then printf '%s..HEAD' "$BASE"; else printf 'HEAD'; fi; }

echo one > apps/thing/src/alpha/a.rs; commit "feat(alpha): the first thing"
echo two > apps/thing/src/alpha/b.rs; commit "fix(alpha): the second thing"
echo three > apps/thing/src/beta/c.rs; commit "feat(beta): a different subsystem"
echo four > docs/x.md;                 commit "docs: a page"

# ---------------------------------------------------------------- the judge

echo "  a conventional message passes and says nothing"
expect_exit 0 "$RB" commit validate --repo . <<'EOF'
feat(alpha): a subject the policy is content with
EOF
expect_grep 'nothing to report'

echo "  a message that is not conventional is refused, and is one finding"
expect_exit 10 "$RB" commit validate --repo . <<'EOF'
update stuff
EOF
expect_grep 'commit\.not_conventional'
expect_no_grep 'commit\.unknown_scope'

echo "  git's own subjects are exempt rather than wrong"
for subject in "Merge pull request #1 from a/b" 'Revert "feat(alpha): a thing"' "fixup! feat(alpha): a thing"; do
  expect_exit 0 "$RB" commit validate --repo . <<EOF
$subject
EOF
  expect_grep 'exempt'
done

echo "  a subject wider than the policy allows is refused, measured whole"
long="$(printf 'x%.0s' $(seq 1 90))"
expect_exit 10 "$RB" commit validate --repo . <<EOF
feat(alpha): $long
EOF
expect_grep 'commit\.subject_too_long'

echo "  a record id the layer does not hold is refused; nothing is invented"
expect_exit 10 "$RB" commit validate --repo . <<'EOF'
fix(alpha): a thing under I9999
EOF
expect_grep 'commit\.unresolved_reference'
expect_grep 'I9999'

echo "  and one the layer does hold is not"
# A real issue of this repository, copied in rather than invented: the record schema is
# strict and a fixture that wrote its own would be testing the fixture's idea of an issue.
mkdir -p .ai/repo/project/issues
REAL="$(ls "$ROOT"/.ai/repo/project/issues/*.yaml 2>/dev/null | head -1)"
[ -n "$REAL" ] || { echo "    this repository holds no issue to copy"; exit 1; }
cp "$REAL" .ai/repo/project/issues/
HELD="$(basename "$REAL" .yaml)"
# Discovery reads what git tracks, so a record dropped into the tree and not added is a
# record the layer does not hold — which is the correct answer, and not the one under test.
git add .ai/repo/project/issues/
expect_exit 0 "$RB" commit validate --repo . <<EOF
fix(alpha): a thing under $HELD
EOF
expect_no_grep 'unresolved_reference'

echo "  a breaking change that explains nothing is refused; one that explains is not"
expect_exit 10 "$RB" commit validate --repo . <<'EOF'
feat(alpha)!: the route moved
EOF
expect_grep 'commit\.breaking_unexplained'
expect_exit 0 "$RB" commit validate --repo . <<'EOF'
feat(alpha)!: the route moved

It is under /api/v2 now.
EOF

echo "  a fix with no test among its files is reported, and does not refuse"
expect_exit 0 "$RB" commit validate --repo . --paths apps/thing/src/alpha/a.rs <<'EOF'
fix(alpha): a thing
EOF
expect_grep 'commit\.fix_without_test'
echo "  and one that carries a test says nothing"
expect_exit 0 "$RB" commit validate --repo . --paths apps/thing/src/alpha/a.rs,test/cases/x.sh <<'EOF'
fix(alpha): a thing
EOF
expect_no_grep 'fix_without_test'
echo "  a judge given no paths does not invent the finding"
expect_exit 0 "$RB" commit validate --repo . <<'EOF'
fix(alpha): a thing
EOF
expect_no_grep 'fix_without_test'

# ---------------------------------------------------------------- the vocabulary

echo "  the scope vocabulary is learned from this repository's own history"
expect_exit 0 "$RB" commit scopes --repo .
expect_grep 'alpha'
expect_grep 'beta'
expect_no_grep 'gamma'

echo "  a scope outside it is a warning and does not refuse the first commit of a subsystem"
expect_exit 0 "$RB" commit validate --repo . <<'EOF'
feat(gamma): the first commit of a subsystem nobody has committed about
EOF
expect_grep 'warning +commit\.unknown_scope'

echo "  and the vocabulary follows the history: once gamma is committed, it is known"
mkdir -p apps/thing/src/gamma; echo five > apps/thing/src/gamma/d.rs
commit "feat(gamma): the subsystem now exists"
expect_exit 0 "$RB" commit scopes --repo .
expect_grep 'gamma'

# ---------------------------------------------------------------- the plan

echo "  a plan over a clean tree proposes nothing"
expect_exit 0 "$RB" commit plan --repo .
expect_grep 'nothing staged'

echo "  a plan groups staged paths by the scope the history gives their directory"
echo six > apps/thing/src/alpha/e.rs
echo seven > apps/thing/src/beta/f.rs
git add apps/thing/src/alpha/e.rs apps/thing/src/beta/f.rs
expect_exit 0 "$RB" commit plan --repo .
expect_grep '\(alpha\)'
expect_grep '\(beta\)'
expect_grep 'prior commit'

echo "  the plan carries a fingerprint of the repository, the worktree, HEAD and the change set"
run_quiet plan.err "$RB" commit plan --repo . --format json > plan.json
expect_grep '"repository"' plan.json
expect_grep '"worktree"' plan.json
expect_grep '"head"' plan.json
before="$(sed -n 's/.*"changes": "\([0-9a-f]*\)".*/\1/p' plan.json | head -1)"
[ -n "$before" ] || { echo "    the plan carries no change fingerprint"; exit 1; }

echo "  staging one more file moves the fingerprint, so a plan made before it is stale"
echo eight > apps/thing/src/alpha/g.rs; git add apps/thing/src/alpha/g.rs
run_quiet plan2.err "$RB" commit plan --repo . --format json > plan2.json
after="$(sed -n 's/.*"changes": "\([0-9a-f]*\)".*/\1/p' plan2.json | head -1)"
[ "$before" != "$after" ] || { echo "    the fingerprint did not move when the index did"; exit 1; }

echo "  and committing moves it too, because HEAD is part of it"
head_before="$(sed -n 's/.*"head": "\([0-9a-f]*\)".*/\1/p' plan2.json | head -1)"
commit "feat(alpha): the staged work"
run_quiet plan3.err "$RB" commit plan --repo . --format json > plan3.json
head_after="$(sed -n 's/.*"head": "\([0-9a-f]*\)".*/\1/p' plan3.json | head -1)"
[ "$head_before" != "$head_after" ] || { echo "    the fingerprint's HEAD did not move across a commit"; exit 1; }

echo "  two worktrees of one repository share a repository and not a plan"
# Beside the fixture and not inside it: a checkout inside a work tree is an embedded
# repository, which `git add -A` then tries to add and the harness's own commits trip over.
WT="$(mktemp -d "${TMPDIR:-/tmp}/mj-274-wt.XXXXXX")" && rmdir "$WT"
run_quiet wt.err git worktree add -q -b other-272 "$WT"
run_quiet other.err "$RB" commit plan --repo "$WT" --format json > other.json
repo_here="$(sed -n 's/.*"repository": "\(.*\)",/\1/p' plan3.json | head -1)"
repo_there="$(sed -n 's/.*"repository": "\(.*\)",/\1/p' other.json | head -1)"
wt_here="$(sed -n 's/.*"worktree": "\(.*\)",/\1/p' plan3.json | head -1)"
wt_there="$(sed -n 's/.*"worktree": "\(.*\)",/\1/p' other.json | head -1)"
[ "$repo_here" = "$repo_there" ] || { echo "    one repository read as two: '$repo_here' vs '$repo_there'"; exit 1; }
[ "$wt_here" != "$wt_there" ] || { echo "    two worktrees read as one: both '$wt_here'"; exit 1; }

# ---------------------------------------------------------------- the history and the gate

echo "  the history is judged in one pass, and says what it read"
expect_exit 0 "$RB" commit history --repo . "$(range)"
expect_grep 'commit\(s\): .* exempt, 0 failing'

echo "  a commit that does not satisfy the policy makes the range fail"
echo nine > apps/thing/src/alpha/h.rs; commit "not a conventional subject at all"
expect_exit 10 "$RB" commit history --repo . "$(range)"
expect_grep 'commit\.not_conventional'

echo "  the gate reads the same verdict, and fails on the branch's own failing commit"
export MJ_ROOT="$PWD" MAJORDOMUS_BIN="$RB"
expect_exit 10 "$ROOT/scripts/ci/commit-policy"

echo "  a checkout with no trunk to compare against says it measured nothing, not that it passed"
expect_grep 'branch .*(not measured|\.\.HEAD)'

echo "  and the baseline is a deliberate act that makes today's debt the ceiling"
expect_exit 0 "$ROOT/scripts/ci/commit-policy" --write-baseline
expect_grep 'baseline written'
echo "  one more failing commit is new debt, and is refused"
echo ten > apps/thing/src/alpha/i.rs; commit "another subject with no type"
expect_exit 10 "$ROOT/scripts/ci/commit-policy"
expect_grep "debt grew"

echo "  under --strict the baseline is ignored and any debt fails"
expect_exit 0 "$ROOT/scripts/ci/commit-policy" --write-baseline
expect_exit 10 "$ROOT/scripts/ci/commit-policy" --strict

# ---------------------------------------------------------------- the hook

echo "  the commit-msg hook refuses a bad message and lets a good one through"
mkdir -p .githooks bin
cp "$ROOT/.githooks/commit-msg" .githooks/commit-msg
# the hook calls bin/majordomus-cli; in the fixture that is the executable under test
printf '#!/bin/sh\nexec %s "$@"\n' "$RB" > bin/majordomus-cli
chmod +x bin/majordomus-cli .githooks/commit-msg
git config core.hooksPath .githooks
echo eleven > apps/thing/src/alpha/j.rs; git add apps/thing/src/alpha/j.rs
# git reports any hook refusal as 1, whatever the hook exited with; what the commit policy
# said is in the output, and that a commit was not made is the fact under test.
expect_exit 1 git commit -q -m "still not a conventional subject"
expect_grep 'commit\.not_conventional'
git log -1 --format=%s > refused.txt
expect_no_grep 'still not a conventional subject' refused.txt
expect_exit 0 git commit -q -m "feat(alpha): a subject the hook is content with"
git log -1 --format=%s > last.txt
expect_grep 'feat\(alpha\): a subject the hook is content with' last.txt
