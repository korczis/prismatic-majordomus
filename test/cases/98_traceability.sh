# The work graph above the branch, both ways, in a repository built to have every case in it.
#
# A history is constructed here rather than asserted about this repository's own, because the
# interesting states — a branch merged with a merge commit, a branch still open, a branch
# absorbed by a fast-forward, a commit no branch names, a pull request whose head branch
# names nothing — cannot all be relied on to exist in any real history at once, and a case
# that only passes where they happen to exist proves nothing.
#
# The two halves are exercised separately and then together: the git half through the
# executable's `trace` capabilities over real MCP frames, the GitHub half through
# scripts/traceability against a pull-request fixture, so the join is provable with no token
# and no network.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj98.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
command -v jq >/dev/null 2>&1 || { echo "    jq is required by this case"; exit 1; }

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm "chore: install the layer"
git branch -M master

mkdir -p .ai/repo/project/milestones .ai/repo/project/issues
cat > .ai/repo/project/milestones/M0.yaml <<'Y'
id: M0
title: The outcome the two issues serve
slug: the-outcome
order: 10
priority: p1
problem: "Nothing leads from a merged commit back to the contract it satisfied."
outcome: "It does."
acceptance_criteria:
  - The edge is derived
validation:
  - bash test/run.sh
evidence_required:
  - traceability
Y
mk_issue() { # id
  cat > ".ai/repo/project/issues/$1.yaml" <<Y
id: $1
milestone: M0
title: Issue $1
slug: issue-$(printf '%s' "$1" | tr 'A-Z' 'a-z')
priority: p1
profile: implementation
objective: "Something to attribute commits to."
scope:
  - src
acceptance_criteria:
  - It is done
validation:
  - bash test/run.sh
evidence_required:
  - a_thing
Y
}
mk_issue I0001
mk_issue I0002
mk_issue I0003
mk_issue I0004
git add -A >/dev/null && git commit -qm "feat(plan): four issues and the outcome they serve"

commit_file() { printf '%s\n' "$2" > "$1"; git add "$1" >/dev/null; git commit -qm "$3"; }

# I0001: two commits on a branch, merged with a merge commit — the shape a pull request makes
git checkout -q -b feature/I0001-merged
commit_file a.txt one "feat(a): the first commit of I0001"
commit_file b.txt two "feat(a): the second commit of I0001"
git checkout -q master
git merge -q --no-ff -m "Merge pull request #1: feat(a): I0001" feature/I0001-merged

# a commit straight on the trunk: work with no execution contract at all
commit_file c.txt three "chore: a commit no branch named an issue for"

# I0002: still open, one commit ahead of the trunk
git checkout -q -b feature/I0002-open
commit_file d.txt four "feat(d): the only commit of I0002"
git checkout -q master

# I0003: fast-forwarded onto the trunk, so nothing distinguishes its commits from the trunk's
git checkout -q -b fix/I0003-absorbed
commit_file e.txt five "fix(e): the only commit of I0003"
git checkout -q master
git merge -q --ff-only fix/I0003-absorbed

# I0004 has no branch at all: declared, not started
git checkout -q master

# ---------------------------------------------------------------- the git half, over MCP
call() { # tool arguments -> the typed answer on stdout
  { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case98","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2"
  } | "$RB" mcp --standalone --repo "$T" 2>"$S/err" | sed -n 2p
}

# the three tools exist, and none of them claims a command line
"$RB" mcp --inspect --repo "$T" > "$S/inspect.txt" 2>"$S/inspect.err" || { echo "    --inspect failed"; cat "$S/inspect.err"; exit 1; }
for tool in majordomus_trace_issue majordomus_trace_commit majordomus_traceability; do
  grep -q "$tool" "$S/inspect.txt" || { echo "    $tool is not served"; exit 1; }
done
grep -q 'majordomus://traceability' "$S/inspect.txt" || { echo "    the traceability resource is not served"; exit 1; }

# --- an issue realised by a merged branch: its branch, its merge commit, its two commits
call majordomus_trace_issue '{"issue":"I0001"}' | jq '.result.structuredContent' > "$S/i1.json"
jq -e '.issue == "I0001" and .milestone == "M0" and .trunk == "master" and .complete == true' "$S/i1.json" >/dev/null \
  || { echo "    I0001 does not carry its own milestone and trunk:"; cat "$S/i1.json"; exit 1; }
jq -e '.commits == 2 and (.branches | length) == 1' "$S/i1.json" >/dev/null \
  || { echo "    I0001 should have one branch with two commits:"; jq -c '{commits, branches: [.branches[].name]}' "$S/i1.json"; exit 1; }
jq -e '.branches[0].name == "feature/I0001-merged" and .branches[0].integration == "merged" and (.branches[0].merge_commit | length) == 40' "$S/i1.json" >/dev/null \
  || { echo "    the merged branch is not reported as merged by a named commit:"; jq -c '.branches[0]' "$S/i1.json"; exit 1; }
jq -e '[.branches[0].commits[].subject] == ["feat(a): the second commit of I0001","feat(a): the first commit of I0001"]' "$S/i1.json" >/dev/null \
  || { echo "    the merged branch's own commits are wrong:"; jq -c '[.branches[0].commits[].subject]' "$S/i1.json"; exit 1; }

# --- an open branch is measured against the trunk
call majordomus_trace_issue '{"issue":"I0002"}' | jq '.result.structuredContent' > "$S/i2.json"
jq -e '.branches[0].integration == "open" and .commits == 1 and .complete == true' "$S/i2.json" >/dev/null \
  || { echo "    the open branch is wrong:"; cat "$S/i2.json"; exit 1; }

# --- a branch that reached the trunk without a merge commit claims nothing, and says why
call majordomus_trace_issue '{"issue":"I0003"}' | jq '.result.structuredContent' > "$S/i3.json"
jq -e '.branches[0].integration == "absorbed" and .commits == 0 and .complete == false and (.branches[0].note | test("fast-forward"))' "$S/i3.json" >/dev/null \
  || { echo "    the absorbed branch does not report itself as one:"; cat "$S/i3.json"; exit 1; }

# --- an issue nobody has started is an empty trace, not an error; it is declared
call majordomus_trace_issue '{"issue":"I0004"}' | jq '.result.structuredContent' > "$S/i4.json"
jq -e '.issue == "I0004" and .declared == true and (.branches | length) == 0 and .commits == 0' "$S/i4.json" >/dev/null \
  || { echo "    an unstarted issue should trace to nothing and still be declared:"; cat "$S/i4.json"; exit 1; }

# --- an id the project model does not declare answers, and says the model does not have it.
# The two must never look the same: "declared and nothing has realised it" is a fact about
# the work, "not declared" is a fact about the question.
call majordomus_trace_issue '{"issue":"I9999"}' | jq '.result.structuredContent' > "$S/i9.json"
jq -e '.issue == "I9999" and .declared == false and (.branches | length) == 0' "$S/i9.json" >/dev/null \
  || { echo "    an undeclared issue id must answer with declared:false:"; cat "$S/i9.json"; exit 1; }

# --- backwards: a commit of the merged branch names the issue and the milestone it served
sha="$(git rev-parse feature/I0001-merged)"
call majordomus_trace_commit "$(printf '{"commit":"%s"}' "$sha")" | jq '.result.structuredContent' > "$S/c1.json"
jq -e '.attribution == "attributed" and .issue == "I0001" and .milestone == "M0" and (.branches | index("feature/I0001-merged"))' "$S/c1.json" >/dev/null \
  || { echo "    the merged commit does not lead back to its issue and milestone:"; cat "$S/c1.json"; exit 1; }

# --- and a commit no branch named an issue for is reported as unattributed, with a reason
bare="$(git log --format=%H --grep='no branch named an issue' -1 master)"
[ -n "$bare" ] || { echo "    the fixture's unattributed commit is missing"; exit 1; }
call majordomus_trace_commit "$(printf '{"commit":"%s"}' "$bare")" | jq '.result.structuredContent' > "$S/c2.json"
jq -e '.attribution == "unattributed" and .issue == null and (.reason | length) > 20' "$S/c2.json" >/dev/null \
  || { echo "    work with no execution contract must be reported as unattributed:"; cat "$S/c2.json"; exit 1; }

# --- the report: every issue, and the trunk stretch with its unattributed commits counted
call majordomus_traceability '{"limit":50}' | jq '.result.structuredContent' > "$S/r.json"
jq -e '.tallies.issues_declared == 4 and .tallies.issues_with_branch == 3 and (.without_branch | index("I0004"))' "$S/r.json" >/dev/null \
  || { echo "    the report miscounts the plan:"; jq -c '{tallies, without_branch}' "$S/r.json"; exit 1; }
jq -e '.tallies.unattributed >= 1 and (.tallies.attributed + .tallies.unattributed + .tallies.ambiguous) == .examined' "$S/r.json" >/dev/null \
  || { echo "    every examined commit must land in exactly one bucket:"; jq -c '{examined, tallies}' "$S/r.json"; exit 1; }
# the absorbed branch's commit is on the trunk and nothing claims it: the report says so
# rather than leaving it out
jq -e '[.commits[] | select(.commit.subject | test("the only commit of I0003"))] | length == 1' "$S/r.json" >/dev/null \
  || { echo "    the absorbed branch's commit was omitted rather than reported"; exit 1; }

# --- nothing was written: the derivation reads, it does not remember
after="$(git status --porcelain)"
[ -z "$after" ] || { echo "    tracing changed the repository:"; printf '%s\n' "$after"; exit 1; }
grep -rq 'feature/I0001-merged' .ai/repo/project/ && { echo "    a canonical record now names a branch, which is the invariant this must not break"; exit 1; }

# ---------------------------------------------------------------- the join, offline
# The pull requests come from a fixture in the shape the live read produces: number, head
# branch, state, merged-at, merge commit, title, url. Three of them: one whose head branch
# names an issue, one merged branch that was deleted (so only GitHub still knows the name),
# and one whose head branch names nothing at all.
merge_sha="$(git log --format=%H --grep='Merge pull request #1' -1 master)"
printf '1\tfeature/I0001-merged\tclosed\t2026-09-09T10:00:00Z\t%s\tfeat(a): I0001\thttps://example.invalid/1\n' "$merge_sha" > "$S/pulls.tsv"
printf '2\tfeature/I0002-open\topen\t\t\tfeat(d): I0002\thttps://example.invalid/2\n' >> "$S/pulls.tsv"
printf '3\tchore/tidy-up\tclosed\t2026-09-09T11:00:00Z\t\tchore: tidy up\thttps://example.invalid/3\n' >> "$S/pulls.tsv"

TRACE="$ROOT/scripts/traceability"
[ -x "$TRACE" ] || { echo "    scripts/traceability is not executable"; exit 1; }
run_trace() { MJ_GH_FIXTURE_PULLS="$S/pulls.tsv" MAJORDOMUS_BIN="$RB" "$TRACE" --repo "$T" "$@"; }

run_quiet "$S/t1.err" run_trace --issue I0001 --format json > "$S/t1.json"
jq -e '.pull_requests_source == "fixture" and (.pulls | length) == 1 and .pulls[0].number == 1 and .pulls[0].issue == "I0001"' "$S/t1.json" >/dev/null \
  || { echo "    the pull request that realised I0001 is not joined to it:"; cat "$S/t1.json"; exit 1; }
jq -e '.issue.commits == 2 and .issue.milestone == "M0"' "$S/t1.json" >/dev/null \
  || { echo "    the git half is missing from the joined answer:"; jq -c '.issue' "$S/t1.json"; exit 1; }

# a pull request whose head branch names no issue is reported as unattributed, not dropped
run_quiet "$S/t3.err" run_trace --pull 3 --format json > "$S/t3.json"
jq -e '.attribution == "unattributed" and .issue == null and .branch == "chore/tidy-up"' "$S/t3.json" >/dev/null \
  || { echo "    a pull request with no execution contract must be reported as unattributed:"; cat "$S/t3.json"; exit 1; }
run_quiet "$S/t2.err" run_trace --pull 2 --format json > "$S/t2.json"
jq -e '.issue == "I0002" and .milestone == "M0"' "$S/t2.json" >/dev/null \
  || { echo "    an open pull request does not lead to its issue and milestone:"; cat "$S/t2.json"; exit 1; }

# the whole report counts both halves, and names the unattributed pull request
run_quiet "$S/t4.err" run_trace --format json > "$S/t4.json"
jq -e '.tallies.pulls == 3 and .tallies.pulls_attributed == 2 and .tallies.pulls_unattributed == 1' "$S/t4.json" >/dev/null \
  || { echo "    the joined tallies are wrong:"; jq -c '.tallies' "$S/t4.json"; exit 1; }
jq -e '[.pulls[] | select(.issue == null) | .number] == [3]' "$S/t4.json" >/dev/null \
  || { echo "    the unattributed pull request is not named:"; jq -c '.pulls' "$S/t4.json"; exit 1; }

# the text rendering says the same, in the words a person reads
run_quiet "$S/t5.err" run_trace > "$S/t5.txt"
grep -q 'unattributed #3  chore/tidy-up' "$S/t5.txt" || { echo "    the text report hides the unattributed pull request:"; cat "$S/t5.txt"; exit 1; }
grep -q 'read from the fixture' "$S/t5.txt" || { echo "    the report does not say where the pull requests came from"; exit 1; }

# a commit merged by a pull request whose branch is gone: git alone cannot name the issue,
# and the joined answer says which pull request merged it
run_quiet "$S/t6.err" run_trace --commit "$bare" --format json > "$S/t6.json"
jq -e '.commit.attribution == "unattributed"' "$S/t6.json" >/dev/null \
  || { echo "    the joined commit answer lost the git verdict:"; cat "$S/t6.json"; exit 1; }

# --strict is the shape a gate would use: a commit with no contract is a non-zero exit
expect_exit 10 env MJ_GH_FIXTURE_PULLS="$S/pulls.tsv" MAJORDOMUS_BIN="$RB" "$TRACE" --repo "$T" --strict --commit "$bare"

# --- the script reaches GitHub and the executable does not: the boundary, asserted
grep -qE '(^|[^a-zA-Z_./-])(gh|curl|wget)[[:space:]]' "$TRACE" || { echo "    scripts/traceability makes no GitHub call at all"; exit 1; }
grep -rnE '(^|[^a-zA-Z_./-])(gh|curl|wget)[[:space:]]' "$ROOT/apps/majordomus-cli/src/worktree/trace.rs" "$ROOT/apps/majordomus-cli/src/capability/builtin/trace.rs" | grep -v '^[^:]*:[0-9]*: *//' \
  && { echo "    the executable's half reaches for the network"; exit 1; }
exit 0
