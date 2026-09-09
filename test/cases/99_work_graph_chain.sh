# majordomus-covers: plan
# One issue, the whole way, and back again.
#
# Every link in this chain is proved somewhere else — case 45 proves the projection's six
# states, case 97 proves the gate can fail, case 98 proves the edges above the branch, and
# the plan cases prove the derivations. A chain is not proved by proving its links. A model
# can pass every one of those and still be unable to answer the question the whole thing
# exists for: which outcome did this merged commit serve, and is that outcome actually
# reached?
#
# So one synthetic issue is carried from an outcome to accepted completion, forwards, and
# then the same chain is walked backwards from the commit that realised it. Both halves run
# offline against fixtures: the remote is a file (MJ_GH_FIXTURE_*), the history is built
# here, and no token is used.
#
# The assertion that matters most is the last one. GitHub closing an issue must NOT complete
# it here. That is the difference between a control plane and a pair of databases that
# happen to agree, and it is the one property no other case in this repository states.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj99c.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?
SYNC="$ROOT/scripts/github-sync"
TRACE="$ROOT/scripts/traceability"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
command -v jq >/dev/null 2>&1 || { echo "    jq is required by this case"; exit 1; }

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm "chore: install the layer"
git branch -M master
pj_init

# ---------------------------------------------------------------- the outcome
mkdir -p .ai/repo/project/milestones .ai/repo/project/issues
cat > .ai/repo/project/milestones/M0.yaml <<'Y'
id: M0
title: A merged commit leads back to the outcome it served
slug: the-chain
order: 10
priority: p1
problem: "Nothing joins an outcome to the code that reached it."
outcome: "One issue can be walked from outcome to accepted completion and back."
acceptance_criteria:
  - The chain is derived in both directions
validation:
  - bash test/run.sh
evidence_required:
  - chain
Y
cat > .ai/repo/project/issues/I0001.yaml <<'Y'
id: I0001
milestone: M0
title: Realise the outcome
slug: realise-the-outcome
priority: p1
profile: implementation
objective: "Be the one issue this case carries the whole way."
scope:
  - src
acceptance_criteria:
  - The thing is built
validation:
  - bash test/run.sh
evidence_required:
  - built
Y

# ---------------------------------------------------------------- the work
# The branch names the issue, which is the edge the topology guard already enforces and the
# only reason any of what follows is derivable.
git add -A >/dev/null && git commit -qm "feat(plan): the outcome and its contract"
git checkout -qb feature/I0001-realise-the-outcome
mkdir -p src && echo "the thing" > src/thing.txt
git add src >/dev/null && git commit -qm "feat(src): build the thing for I0001"
git checkout -q master
git merge -q --no-ff feature/I0001-realise-the-outcome -m "Merge pull request #1: feat(src): build the thing for I0001"
merge_sha="$(git rev-parse HEAD)"
work_sha="$(git rev-parse feature/I0001-realise-the-outcome)"

# ---------------------------------------------------------------- the remotes, as files
# The issue is CLOSED on GitHub from the start. That is the trap this case is built around.
row() { b="$(cat | base64 | tr -d '\n')"; printf '%s\t%s\t%s\t%s\tM0 — %s\t%s\n' "$1" "$2" "$3" "$b" "A merged commit leads back to the outcome it served" "$4"; }
printf '1\tM0 — A merged commit leads back to the outcome it served\topen\n' > "$S/ms.tsv"
"$SYNC" --render I0001 | row 7 'I0001 — Realise the outcome' closed I0001 > "$S/is.tsv"
printf '1\tfeature/I0001-realise-the-outcome\tclosed\t2026-09-09T10:00:00Z\t%s\tfeat(src): build the thing for I0001\thttps://example.invalid/1\n' "$merge_sha" > "$S/pulls.tsv"

sync_fx()  { MJ_GH_FIXTURE_ISSUES="$S/is.tsv" MJ_GH_FIXTURE_MILESTONES="$S/ms.tsv" "$SYNC" "$@"; }
trace_fx() { MJ_GH_FIXTURE_PULLS="$S/pulls.tsv" MAJORDOMUS_BIN="$RB" "$TRACE" --repo "$T" "$@"; }

# ================================================================ FORWARDS
# outcome -> issue -> branch -> commits -> pull request -> checks -> evidence -> accepted

# --- the outcome contains the issue, and the issue is not yet done
"$MJ" plan show M0 > "$S/m0.txt"
grep -q 'I0001' "$S/m0.txt" || { echo "    the milestone does not name the issue that serves it"; exit 1; }

# --- the issue reaches its branch, its commits and the pull request that proposed them
run_quiet "$S/f.err" trace_fx --issue I0001 --format json > "$S/fwd.json"
jq -e '.issue.issue == "I0001" and .issue.milestone == "M0"' "$S/fwd.json" >/dev/null \
  || { echo "    the issue does not carry its milestone:"; jq -c '.issue' "$S/fwd.json"; exit 1; }
jq -e '.issue.commits == 1 and (.issue.branches | length) == 1' "$S/fwd.json" >/dev/null \
  || { echo "    the issue does not reach its branch and commit:"; jq -c '.issue' "$S/fwd.json"; exit 1; }
jq -e '.issue.branches[0].name == "feature/I0001-realise-the-outcome" and .issue.branches[0].integration == "merged"' "$S/fwd.json" >/dev/null \
  || { echo "    the branch is not reported as merged:"; jq -c '.issue.branches[0]' "$S/fwd.json"; exit 1; }
jq -e '(.pulls | length) == 1 and .pulls[0].number == 1 and .pulls[0].issue == "I0001"' "$S/fwd.json" >/dev/null \
  || { echo "    the pull request that realised the issue is not joined to it:"; jq -c '.pulls' "$S/fwd.json"; exit 1; }

# ================================================================ BACKWARDS
# merged commit -> issue -> milestone, and the same for the commit that did the work

for sha in "$work_sha" "$merge_sha"; do
  run_quiet "$S/b.err" trace_fx --commit "$sha" --format json > "$S/back.json"
  jq -e '.commit.attribution == "attributed" and .commit.issue == "I0001" and .commit.milestone == "M0"' "$S/back.json" >/dev/null \
    || { echo "    $sha does not lead back to the outcome it served:"; jq -c '.commit' "$S/back.json"; exit 1; }
done

# --- and a commit that served no contract says so rather than being dropped
base_sha="$(git rev-list --max-parents=0 HEAD)"
run_quiet "$S/u.err" trace_fx --commit "$base_sha" --format json > "$S/un.json"
jq -e '.commit.attribution == "unattributed"' "$S/un.json" >/dev/null \
  || { echo "    a commit with no contract must be reported, not omitted:"; jq -c '.commit' "$S/un.json"; exit 1; }

# ================================================================ COMPLETION
# The property the whole chain exists to protect.

# --- the remote says closed; the model does not, and says why
[ "$("$MJ" plan list | awk '$1=="I0001"{print $2}')" != DONE ] \
  || { echo "    the issue is DONE before any evidence was recorded"; exit 1; }
expect_exit 10 "$MJ" plan "done" I0001
expect_grep 'built'

# --- and the projection reports the disagreement rather than resolving it either way
sync_fx --check > "$S/drift.txt" 2>&1 || true
grep -qE '^DRIFT  closed +issue I0001' "$S/drift.txt" \
  || { echo "    a remote close of an issue that is not DONE here must be reported:"; cat "$S/drift.txt"; exit 1; }

# --- evidence is what completes it, and nothing else
"$MJ" plan start I0001 >/dev/null
"$MJ" plan evidence I0001 --covers built --type test --command "bash test/run.sh" --result "the thing is built" >/dev/null
"$MJ" plan verify I0001 >/dev/null
expect_exit 0 "$MJ" plan "done" I0001
[ "$("$MJ" plan list | awk '$1=="I0001"{print $2}')" = DONE ] \
  || { echo "    the issue did not complete once its required evidence was recorded"; exit 1; }

# --- with the model and the remote now agreeing, that finding is gone
sync_fx --check > "$S/drift2.txt" 2>&1 || true
grep -qE '^DRIFT  closed +issue I0001' "$S/drift2.txt" \
  && { echo "    the closed-state disagreement survived completion:"; cat "$S/drift2.txt"; exit 1; }

# --- the accepted issue's evidence is readable from the record it completed
"$MJ" plan show I0001 > "$S/done.txt"
grep -q 'the thing is built' "$S/done.txt" \
  || { echo "    the evidence that satisfied the criterion is not on the record"; exit 1; }

# ================================================================ the boundary
# Nothing in this chain wrote a GitHub or git fact into a canonical record.
grep -rlE 'feature/I0001|https://example\.invalid|^github:' .ai/repo/project/ \
  && { echo "    a canonical record was taught something git or GitHub already knows"; exit 1; }
exit 0
