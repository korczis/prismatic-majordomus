# The roadmap graph: milestone dependencies, the gate, and the derived sequence.
#
# The issue graph decides what a worker may execute; this graph decides which outcomes are
# reachable at all. "Each step is gated by the previous one being real" is the sentence this
# case turns into an assertion — a milestone whose dependency is not DONE is BLOCKED, and no
# amount of finished work inside it changes that.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
# init adds the local-state ignore line; commit it so the task that follows starts from a clean tree
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
pj_init

# A milestone with a version, a dependency list and no M-prefixed id. Identity is the slug;
# the version is a field, so a renumber cannot invalidate a reference.
rm_milestone() { # ID VERSION ORDER [DEP ...]
  local id="$1" ver="$2" ord="$3"; shift 3
  { cat <<Y
id: $id
title: Milestone $id
slug: $id
version: "$ver"
order: $ord
priority: p1
problem: "A problem worth solving."
outcome: "The outcome once it is solved."
acceptance_criteria:
  - The outcome is reached
validation:
  - true
evidence_required:
  - proof
Y
    if [ $# -gt 0 ]; then printf 'depends_on:\n'; for d in "$@"; do printf -- '  - %s\n' "$d"; done; fi
  } > ".ai/repo/project/milestones/$id.yaml"
}
mstatus() { "$MJ" plan roadmap | awk -v i="$2" '$3==i{print $2}'; }
mstat()   { "$MJ" plan roadmap | awk -v i="$1" '$3==i{print $2}'; }

# --- a chain, declared out of order on purpose: order must come from the graph, not the file
rm_milestone third  0.3 10 second
rm_milestone first  0.1 30
rm_milestone second 0.2 20 first
expect_exit 0 "$MJ" plan validate

# the derived sequence follows the dependencies, not the `order` field, which is reversed here
seq="$("$MJ" plan roadmap | awk 'NR>1 && $3!=""{printf "%s ", $3}')"
case "$seq" in "first second third "*) : ;;
  *) echo "    roadmap order is '$seq', expected the dependency chain first second third"; exit 1 ;; esac

# --- the gate. Nothing is done anywhere, so only the root is reachable.
[ "$(mstat first)"  = PLANNED ] || { echo "    first is $(mstat first), expected PLANNED"; exit 1; }
[ "$(mstat second)" = BLOCKED ] || { echo "    second is $(mstat second), expected BLOCKED"; exit 1; }
[ "$(mstat third)"  = BLOCKED ] || { echo "    third is $(mstat third), expected BLOCKED"; exit 1; }
"$MJ" plan roadmap | grep -q 'next: second .*blocked by first' \
  || { echo "    roadmap does not name what blocks the next milestone"; exit 1; }

# --- accepting the first milestone unblocks exactly one step, not the whole chain
pj_issue W1 first
"$MJ" plan start W1 >/dev/null
"$MJ" plan verify W1 >/dev/null
"$MJ" plan evidence W1 --covers proof --type test --command true --result ok >/dev/null
"$MJ" plan 'done' W1 >/dev/null
printf 'evidence:\n  - covers: proof\n    type: test\n    command: "true"\n    result: "ok"\n    recorded_at: 2026-09-04\n' \
  >> .ai/repo/project/milestones/first.yaml
[ "$(mstat first)"  = DONE    ] || { echo "    first is $(mstat first), expected DONE"; exit 1; }
[ "$(mstat second)" = PLANNED ] || { echo "    second is $(mstat second), expected PLANNED once first is DONE"; exit 1; }
[ "$(mstat third)"  = BLOCKED ] || { echo "    third is $(mstat third), expected still BLOCKED"; exit 1; }

# --- a new milestone appears in the roadmap without any list being edited anywhere
rm_milestone fourth 1.0 5 third
expect_exit 0 "$MJ" plan validate
"$MJ" plan roadmap | grep -q '^1.0 .*fourth' || { echo "    a new milestone did not appear in the roadmap"; exit 1; }
"$MJ" plan rgraph  | grep -q 'third --> fourth' || { echo "    a new milestone edge is not in the roadmap graph"; exit 1; }

# --- the roadmap graph is Mermaid, drawn from the same edges
g="$("$MJ" plan rgraph)"
printf '%s' "$g" | grep -q '^flowchart LR' || { echo "    plan rgraph is not a Mermaid flowchart"; exit 1; }
for e in "first --> second" "second --> third"; do
  printf '%s' "$g" | grep -q -- "$e" || { echo "    missing roadmap edge: $e"; exit 1; }
done

# --- every negative the issue graph refuses, refused one level up and by name
rm_milestone loopa 0.9 1 loopb
rm_milestone loopb 0.9 2 loopa
expect_exit 10 "$MJ" plan validate
expect_grep 'milestone_cycle'
rm -f .ai/repo/project/milestones/loopa.yaml .ai/repo/project/milestones/loopb.yaml

rm_milestone selfdep 0.9 1 selfdep
expect_exit 10 "$MJ" plan validate
expect_grep 'milestone_self_dependency'
rm -f .ai/repo/project/milestones/selfdep.yaml

rm_milestone dangling 0.9 1 nosuchmilestone
expect_exit 10 "$MJ" plan validate
expect_grep 'milestone_unknown_dependency'
rm -f .ai/repo/project/milestones/dangling.yaml
expect_exit 0 "$MJ" plan validate

# --- the JSON projection carries the same graph the text does
"$MJ" plan roadmap --json > rm.json
jq -e . rm.json >/dev/null || { echo "    plan roadmap --json is not valid JSON"; exit 1; }
[ "$(jq -r '.milestones[] | select(.id=="second") | (.blocked_by[0] // "")' rm.json)" = "" ] \
  || { echo "    second is blocked in JSON but not in the model"; exit 1; }
[ "$(jq -r '.milestones[] | select(.id=="third") | .blocked_by[0]' rm.json)" = "second" ] \
  || { echo "    the JSON does not name what blocks third"; exit 1; }
[ "$(jq -r '.milestones[] | select(.id=="first") | .version' rm.json)" = "0.1" ] \
  || { echo "    the JSON lost the version field"; exit 1; }
