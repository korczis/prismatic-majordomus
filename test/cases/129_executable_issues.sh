# Issues and milestones as executable development scopes, and the provenance on every field.
#
# The model this case is about is a composition, not a new store: the records are the
# canonical ones, the status and the graph are `plan`'s, the branches and commits are
# `trace`'s. So the assertions here are of two kinds and no third:
#
#   1. the composition agrees with what it composed — a devtask's status IS the plan's
#      status, and a milestone's ready set IS the plan's ready set. A surface that
#      re-derived a status of its own fails here, which is what 46_cross_surface.sh does
#      for the shell tool's own surfaces and what this does for the executable's.
#   2. the provenance is honest — a key the record does not carry is `unknown` with a
#      reason, an authored empty list is `explicit` and empty, a link nobody declared is
#      `inferred` with its rule stated, and the external GitHub half is never claimed.
#
# The fixture is built with the pj_* helpers of test/lib.sh, the same vocabulary case 46 and
# case 99 use, so there is one fixture language for the project model and not three.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as every Rust case does.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj129.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# The command under test, in the fixture repository, as JSON. `--no-git` where the answer
# must not depend on the history; without it where the git half is the point.
task()      { "$RB" devtask issue "$1" --no-git --format json 2>"$S/err" || { sed 's/^/    | /' "$S/err"; return 1; }; }
task_git()  { "$RB" devtask issue "$1" --format json 2>"$S/err" || { sed 's/^/    | /' "$S/err"; return 1; }; }
graph()     { "$RB" devtask milestone "$1" --format json 2>"$S/err" || { sed 's/^/    | /' "$S/err"; return 1; }; }

# jq -e, with the failing document shown. A bare `jq -e ... >/dev/null || exit 1` under
# `set -e` reports nothing at all, which is the silent-failure shape this repository has
# been bitten by twice.
holds() { # <label> <json-file> <filter>
  jq -e "$3" < "$2" >/dev/null && return 0
  printf '    %s\n      filter: %s\n' "$1" "$3"
  head -c 2000 "$2" | sed 's/^/    | /'
  return 1
}

# ---------------------------------------------------------------- the fixture
# A chain, a fan-out, a fan-in, a closed dependency, a cancelled issue, a completion whose
# evidence is missing, a milestone gate, an issue naming no milestone, and a scope overlap.
"$MJ" init >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
pj_init
pj_milestone M000 0
pj_milestone M001 1
printf 'depends_on:\n  - M000\n' >> .ai/repo/project/milestones/M001.yaml

# a chain: I0001 -> I0002 -> I0003
pj_issue I0001 M000
pj_issue I0002 M000 I0001
pj_issue I0003 M000 I0002
# a fan-in: I0004 waits on three heads, one of which is already closed
pj_issue I0004 M000 I0001 I0002 I0003
# a closed dependency: I0006 depends on I0005, which is DONE, so I0006 is ready
pj_issue I0005 M000
printf 'completed_at: 2026-01-01T00:00:00Z\nevidence:\n  - covers: proof\n    type: test\n    command: "true"\n    result: ok\n' >> .ai/repo/project/issues/I0005.yaml
pj_issue I0006 M000 I0005
# a completion recorded without the evidence the record requires
pj_issue I0007 M000
printf 'completed_at: 2026-01-01T00:00:00Z\n' >> .ai/repo/project/issues/I0007.yaml
# the milestone gate: everything in M001 waits on M000
pj_issue I0008 M001
git add -A >/dev/null && git commit -qm fixture >/dev/null

# ---------------------------------------------------------------- the composition agrees
# Every issue's devtask status is the plan's status. This is the assertion that keeps the
# model a composition rather than a second registry.
for id in I0001 I0002 I0003 I0004 I0005 I0006 I0007 I0008; do
  want="$(pj_status "$id")"
  task "$id" > "$S/$id.json" || exit 1
  got="$(jq -r '.position.status.value' < "$S/$id.json")"
  [ "$got" = "$want" ] || { echo "    $id is $want to the plan and $got to devtask"; exit 1; }
  holds "$id does not name the status it derived from" "$S/$id.json" \
    '.readiness.canonical_status == .position.status.value' || exit 1
  holds "$id claims its status was authored" "$S/$id.json" \
    '.position.status.provenance == "derived"' || exit 1
done

# and the milestone's ready set is the plan's ready set, with no frontend logic in between
graph M000 > "$S/M000.json" || exit 1
plan_ready="$("$MJ" plan list | awk '$2=="READY"{print $1}' | LC_ALL=C sort | tr '\n' ' ')"
task_ready="$(jq -r '.ready[]' < "$S/M000.json" | LC_ALL=C sort | tr '\n' ' ')"
[ "$plan_ready" = "$task_ready" ] \
  || { echo "    the ready set disagrees:"; echo "      plan:    $plan_ready"; echo "      devtask: $task_ready"; exit 1; }

# ---------------------------------------------------------------- readiness refines, never replaces
holds "a chain head is not ready"            "$S/I0001.json" '.readiness.state == "ready" and .readiness.startable == true'
holds "a chained issue is not blocked"       "$S/I0002.json" '.readiness.state == "blocked" and .readiness.startable == false'
holds "the blocker does not name the issue"  "$S/I0002.json" '[.readiness.blockers[] | select(.kind == "issue" and .subject == "I0001")] | length == 1'
holds "a closed dependency still blocks"     "$S/I0006.json" '.readiness.state == "ready"'
holds "a finished issue is not complete"     "$S/I0005.json" '.readiness.state == "complete" and .readiness.terminal == true'

# the two distinctions the canonical vocabulary conflates
holds "completion without evidence is not its own state" "$S/I0007.json" \
  '.readiness.state == "completion_blocked" and .readiness.canonical_status == "VERIFY"'
holds "the missing evidence token is not named" "$S/I0007.json" \
  '[.readiness.blockers[] | select(.kind == "evidence" and .subject == "proof")] | length == 1'
holds "a milestone gate reads as an ordinary block" "$S/I0008.json" \
  '.readiness.state == "waiting" and (.readiness.blockers | map(.kind) | unique == ["milestone"])'

# ---------------------------------------------------------------- provenance is honest
holds "an authored title is not explicit"    "$S/I0001.json" '.declaration.title.provenance == "explicit"'
holds "an authored field names no source"    "$S/I0001.json" '.declaration.title.source | test("I0001\\.yaml#title")'
holds "an absent key is not unknown"         "$S/I0001.json" '.declaration.why.provenance == "unknown" and (.declaration.why | has("value") | not)'
holds "an unknown field gives no reason"     "$S/I0001.json" '.declaration.why.reason | length > 0'
holds "an authored list is not explicit"     "$S/I0002.json" '.declaration.depends_on.provenance == "explicit" and (.declaration.depends_on.values == ["I0001"])'
holds "a derived field claims authorship"    "$S/I0002.json" '.position.blocked_by.provenance == "derived"'

# an issue naming no milestone: the schema requires the key, so the empty string is the
# only way to author one, and the model must still answer for it
pj_issue I0009 M000
awk '{ sub(/^milestone: M000$/, "milestone: \"\"") } 1' .ai/repo/project/issues/I0009.yaml > "$S/I0009.yaml"
cat "$S/I0009.yaml" > .ai/repo/project/issues/I0009.yaml
git add -A >/dev/null
task I0009 > "$S/I0009.json" || exit 1
holds "an issue with no milestone is refused rather than answered" "$S/I0009.json" '.declared == true'
holds "an empty milestone reads as authored"  "$S/I0009.json" '.declaration.milestone.provenance == "unknown"'
holds "the milestone-derived fields are claimed anyway" "$S/I0009.json" '.position.milestone_status.provenance == "unknown"'
holds "the model does not report the fault"   "$S/I0009.json" '[.diagnostics[] | select(.code == "no_milestone")] | length == 1'
holds "a diagnostic carries no way to reproduce it" "$S/I0009.json" '.diagnostics | map(.reproduce | length > 0) | all'
rm .ai/repo/project/issues/I0009.yaml; git add -A >/dev/null

# ---------------------------------------------------------------- an id nothing declares
task I9999 > "$S/I9999.json" || exit 1
holds "an unknown id is not answered"        "$S/I9999.json" '.declared == false and .readiness.state == "undeclared"'
holds "an unknown id looks like unstarted work" "$S/I9999.json" \
  '.declaration.title.provenance == "unknown" and .position.status.provenance == "unknown"'

# ---------------------------------------------------------------- the external half is never claimed
holds "the GitHub half is claimed"           "$S/I0001.json" \
  '.synchronisation.writable == false and .synchronisation.state.provenance == "unknown" and .synchronisation.external_id.provenance == "unknown"'
holds "the adapter is not named"             "$S/I0001.json" \
  '(.synchronisation.adapter | test("github-sync")) and (.synchronisation.state.reason | test("github-sync"))'
# The repository the projection targets comes from the plan's header. In THIS repository
# that header is empty — `.ai/repo/knowledge/sources.yaml` declares no source class for
# `.ai/repo/project/project.yaml`, so the `project` kind has no objects at all — while the
# fixture built by `"$MJ" init` above does discover it. The assertion is therefore about the
# invariant and not about which of the two happens here: the field is explicit with a value,
# or unknown with a reason, and never a bare empty string that reads as authored.
holds "the target repository is neither authored nor honestly unknown" "$S/I0001.json" \
  '(.synchronisation.repository.provenance == "explicit" and (.synchronisation.repository.value | length > 0))
   or (.synchronisation.repository.provenance == "unknown" and (.synchronisation.repository | has("value") | not) and (.synchronisation.repository.reason | length > 0))'

# ---------------------------------------------------------------- git state, and its absence
holds "git was consulted without being asked" "$S/I0001.json" '.execution.git_consulted == false'
holds "an unasked git half reads as empty"    "$S/I0001.json" '.execution.branches.provenance == "unknown"'

git checkout -q -b feature/I0001-the-work
printf 'work\n' > src-I0001.txt; git add src-I0001.txt >/dev/null
git commit -qm "feat: the work for I0001" >/dev/null
git checkout -q master 2>/dev/null || git checkout -q -
task_git I0001 > "$S/I0001.git.json" || exit 1
holds "git was not consulted"                 "$S/I0001.git.json" '.execution.git_consulted == true'
holds "the branch naming the issue is not linked" "$S/I0001.git.json" \
  '[.execution.branches.values[] | select(test("I0001"))] | length >= 1'
holds "a git-derived field claims authorship" "$S/I0001.git.json" '.execution.branches.provenance == "derived"'

# ---------------------------------------------------------------- fan-out, fan-in, parallelism
holds "the fan-in issue is not blocked by all three" "$S/I0004.json" \
  '[.readiness.blockers[] | select(.kind == "issue")] | length == 3'
holds "the chain head does not unblock the chain" "$S/I0001.json" \
  '.position.transitive_dependents.value >= 3'
holds "the milestone reports no critical blocker" "$S/M000.json" \
  '.critical_blockers | length > 0'
holds "the critical blockers are not ordered by weight" "$S/M000.json" \
  '[.critical_blockers[].weight] == ([.critical_blockers[].weight] | sort | reverse)'
holds "the milestone does not partition its issues" "$S/M000.json" \
  '((.ready + .blocked + .waiting + .active + .review + .completion_blocked + .complete + .cancelled) | length) == (.nodes | length)'
holds "the parallelizable sets are empty"     "$S/M000.json" \
  '.parallelizable | length > 0'
holds "a parallel set holds work that is not ready" "$S/M000.json" \
  '([.parallelizable[].issues[]] | sort) == (.ready | sort)'

# ---------------------------------------------------------------- a cycle, and a missing reference
pj_issue I0010 M000 I0011
pj_issue I0011 M000 I0010
git add -A >/dev/null
graph M000 > "$S/cycle.json" || exit 1
holds "the cycle is not reported as a component" "$S/cycle.json" \
  '[.cycles[] | select((. | sort) == ["I0010","I0011"])] | length == 1'
holds "an issue in a cycle was given a wave"  "$S/cycle.json" \
  '[.nodes[] | select(.issue == "I0010") | has("wave")] == [false]'
holds "the cycle is not a failure"            "$S/cycle.json" \
  '[.diagnostics[] | select(.code == "cycle" and .level == "FAIL")] | length >= 1'
rm .ai/repo/project/issues/I0010.yaml .ai/repo/project/issues/I0011.yaml; git add -A >/dev/null

pj_issue I0012 M000 I9999
git add -A >/dev/null
task I0012 > "$S/I0012.json" || exit 1
holds "a dependency that resolves to nothing is silently dropped" "$S/I0012.json" \
  '[.readiness.blockers[] | select(.kind == "reference" and .subject == "I9999")] | length == 1'
holds "the missing reference is not a diagnostic" "$S/I0012.json" \
  '[.diagnostics[] | select(.code == "unknown_dependency")] | length == 1'
rm .ai/repo/project/issues/I0012.yaml; git add -A >/dev/null

# ---------------------------------------------------------------- determinism
# Two runs of the milestone graph produce the same bytes. The graph is a pure function of
# the records: no git, no clock, no network, every list in canonical order.
graph M000 > "$S/det1.json" || exit 1
graph M000 > "$S/det2.json" || exit 1
cmp -s "$S/det1.json" "$S/det2.json" \
  || { echo "    two runs of the same milestone graph disagree:"; diff "$S/det1.json" "$S/det2.json" | head -20 | sed 's/^/    | /'; exit 1; }
holds "the nodes are not in canonical order"  "$S/det1.json" \
  '[.nodes[].issue] == ([.nodes[].issue] | sort)'

# ---------------------------------------------------------------- a milestone nothing declares
graph nonesuch > "$S/nomilestone.json" || exit 1
holds "an unknown milestone is not answered"  "$S/nomilestone.json" \
  '.declared == false and (.nodes | length) == 0 and .status.provenance == "unknown"'

echo "    devtask: composition, provenance, readiness, graph and determinism hold"
