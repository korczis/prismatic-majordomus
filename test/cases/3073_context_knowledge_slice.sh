# majordomus-covers: context
# claims: context-assembly, context-selection-budget
# The knowledge pointers of `majordomus context` (I0814): a bounded slice of the knowledge
# graph, not the graph. The section lists id, kind, source and title for the task's issue,
# the issues it depends on, its milestone, the claims that milestone declares and the nodes
# its scope names — and nothing else: a document whose words are close to the issue's but
# that nothing references is not selected. It appears only when the profile asks for it, and
# its absence is named under EXCLUDED with the toggle that caused it; the profile's item
# limit cuts it and the cut is counted; under a tight budget it is dropped before git, the
# task, the profile and the open questions, and the drop is named; and when the issue the
# task names is not in the graph the section says the graph is stale instead of vanishing.
. "$ROOT/test/lib.sh"
# the schema check asks the executable's index; this case is about selection, not schema
export MAJORDOMUS_KNOWLEDGE_SCHEMA=unasked
"$MJ" init >/dev/null; "$MJ" update >/dev/null
pj_init
pj_milestone m1 0
printf 'claims:\n  - pointer-claim\n' >> .ai/repo/project/milestones/m1.yaml
pj_milestone m2 1
pj_issue I0001 m1
pj_issue I0003 m2
cat > .ai/repo/project/issues/I0002.yaml <<'Y'
id: I0002
milestone: m1
title: Carry a slice of what the plan states
slug: carry-a-slice
priority: p1
profile: implementation
objective: "Carry a slice of what the plan states."
scope:
  - src/I0002
  - docs/guide.md
acceptance_criteria:
  - The work is done
validation:
  - "true"
evidence_required:
  - proof
depends_on:
  - I0001
Y
mkdir -p docs src/I0002
printf '# The guide the issue names\n' > docs/guide.md
# close in words to the issue, referenced by nothing: similarity must not select it
printf '# Carry a slice of what the plan states\n\nA slice of the plan, carried.\n' > docs/unrelated.md
cat > docs/CLAIMS.yaml <<'Y'
claims:
  - id: pointer-claim
    claim: The milestone declares this claim
    source: docs/guide.md
    status: guaranteed
  - id: other-claim
    claim: No milestone of the issue declares this claim
    source: docs/unrelated.md
    status: guaranteed
Y
echo x > src/I0002/a
git add -A && git commit -qm base

# --- no task: nothing to select from and no profile to ask, so nothing is said
expect_exit 0 "$MJ" context
expect_no_grep '^## KNOWLEDGE POINTERS'
expect_no_grep 'knowledge pointers'

# --- a task with no issue: the profile asks, but there is nothing explicit to follow.
#     With intent binding off that is the ordinary case and costs no line; where the policy
#     binds work to what it serves, the absence is named
"$MJ" start "no issue" --scope src/I0002 --profile implementation >/dev/null
expect_exit 0 "$MJ" context
expect_no_grep '^## KNOWLEDGE POINTERS'
expect_no_grep 'knowledge pointers'
sed -i.bak 's/^  binding: off$/  binding: advisory/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
expect_exit 0 "$MJ" context
expect_no_grep '^## KNOWLEDGE POINTERS'
expect_grep '^- knowledge pointers — the task names no issue'
git checkout -q -- .ai/repo/policy.yaml
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null

# --- the issue's task: exactly what the issue references, in tier order
"$MJ" start "carry the slice" --scope src/I0002 --issue I0002 --profile implementation >/dev/null
expect_exit 0 "$MJ" context
expect_grep '^## KNOWLEDGE POINTERS \(what issue I0002 references; majordomus knowledge related issue:I0002\)$'
body=$(printf '%s\n' "$LAST_OUT" | sed -n '/^## KNOWLEDGE POINTERS/,/^$/p' | sed '1d;/^$/d')
want='issue      issue          issue:I0002  .ai/repo/project/issues/I0002.yaml  Carry a slice of what the plan states
depends_on issue          issue:I0001  .ai/repo/project/issues/I0001.yaml  Issue I0001
milestone  milestone      milestone:m1  .ai/repo/project/milestones/m1.yaml  Milestone m1
claim      claim          claim:pointer-claim  docs/CLAIMS.yaml  The milestone declares this claim
scope      document       document:docs/guide.md  docs/guide.md  The guide the issue names'
[ "$body" = "$want" ] || { echo "    the pointers are not the explicit references, in order:"; diff <(printf '%s\n' "$want") <(printf '%s\n' "$body"); exit 1; }
# explicit references only: the look-alike document, the undeclared claim, the issue of
# another milestone and the other milestone are not selected
expect_no_grep 'docs/unrelated.md'
expect_no_grep 'other-claim'
expect_no_grep 'issue:I0003'
expect_no_grep 'milestone:m2'
expect_no_grep '^- knowledge pointers'
# the section sits after the open questions and the candidates, before the decisions
order=$(printf '%s\n' "$LAST_OUT" | grep -E '^## ' | tr '\n' ' ')
case "$order" in *"## KNOWLEDGE (candidates"*"## KNOWLEDGE POINTERS"*) ;; *) echo "    wrong section order: $order"; exit 1 ;; esac
# read-only: the context wrote nothing
[ -z "$(git status --porcelain -- docs .ai/repo)" ] || { echo "    context wrote into the tree:"; git status --porcelain; exit 1; }

# --- the item limit cuts the list and counts what it cut
sed -i.bak 's/^  knowledge_max_items: 8$/  knowledge_max_items: 2/' .ai/repo/profiles/implementation.yaml; rm -f .ai/repo/profiles/implementation.yaml.bak
expect_exit 0 "$MJ" context
expect_grep '^issue      issue          issue:I0002  '
expect_grep '^depends_on issue          issue:I0001  '
expect_no_grep '^milestone  milestone      milestone:m1'
expect_grep '^\(3 more; majordomus knowledge related issue:I0002\)$'
# ... and never more than the policy allows any list
sed -i.bak 's/^  knowledge_max_items: 2$/  knowledge_max_items: 500/' .ai/repo/profiles/implementation.yaml; rm -f .ai/repo/profiles/implementation.yaml.bak
sed -i.bak 's/^  max_list_items: .*/  max_list_items: 3/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
expect_exit 0 "$MJ" context
expect_grep '^\(2 more; majordomus knowledge related issue:I0002\)$'
git checkout -q -- .ai/repo/policy.yaml .ai/repo/profiles/implementation.yaml

# --- the profile decides: switched off, the absence is named with the toggle
sed -i.bak 's/^  knowledge: true$/  knowledge: false/' .ai/repo/profiles/implementation.yaml; rm -f .ai/repo/profiles/implementation.yaml.bak
expect_exit 0 "$MJ" context
expect_no_grep '^## KNOWLEDGE POINTERS'
expect_grep '^- knowledge pointers — profile implementation sets context.knowledge: false$'
# a profile written before the toggle existed does not ask for it, and says so
sed -i.bak '/^  knowledge: false$/d; /^  knowledge_max_items:/d' .ai/repo/profiles/implementation.yaml; rm -f .ai/repo/profiles/implementation.yaml.bak
expect_exit 0 "$MJ" context
expect_no_grep '^## KNOWLEDGE POINTERS'
expect_grep '^- knowledge pointers — profile implementation does not set context.knowledge$'
git checkout -q -- .ai/repo/profiles/implementation.yaml
# the shipped routine profile leaves it off
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
"$MJ" start "routine slice" --scope src/I0002 --issue I0002 --profile routine >/dev/null
expect_exit 0 "$MJ" context
expect_grep '^- knowledge pointers — profile routine sets context.knowledge: false$'
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
"$MJ" start "carry the slice" --scope src/I0002 --issue I0002 --profile implementation >/dev/null

# --- a tight budget drops it before git, the task, the profile and the open questions
"$MJ" question add "which slice is enough?" >/dev/null
full=$("$MJ" context | wc -l | tr -d ' ')
expect_exit 0 "$MJ" context --budget-lines "$((full - 3))"
expect_no_grep '^## KNOWLEDGE POINTERS'
expect_grep '^- knowledge pointers — context budget [0-9]+ lines; run: majordomus knowledge related issue:I0002$'
expect_grep 'dropped:.*knowledge-pointers'
expect_grep '^## GIT'
expect_grep '^## TASK'
expect_grep '^## PROFILE implementation'
expect_grep '^## OPEN QUESTIONS'
if command -v jq >/dev/null; then
  "$MJ" --json context > c.json
  jq -e '[.sections[].id] | index("knowledge-pointers")' c.json >/dev/null \
    || { echo "    the json form does not carry the section"; exit 1; }
  "$MJ" --json context --budget-lines "$((full - 3))" > d.json
  jq -e '.budget.dropped | index("knowledge-pointers")' d.json >/dev/null \
    || { echo "    the json form does not name the drop"; exit 1; }
fi

# --- a stale graph: the issue the task was started for leaves the version-control index
#     (the record is still on disk, untracked) — the section says the graph is stale and
#     why, and keeps offering what the graph does hold
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
sed 's/^id: I0002$/id: I0004/; s/^title: .*/title: Tracked, then not/' .ai/repo/project/issues/I0002.yaml > .ai/repo/project/issues/I0004.yaml
git add .ai/repo/project/issues/I0004.yaml && git commit -qm 'I0004'
"$MJ" start "a record that leaves the index" --scope src/I0002 --issue I0004 --profile implementation >/dev/null
expect_exit 0 "$MJ" context
expect_grep '^issue      issue          issue:I0004  .ai/repo/project/issues/I0004.yaml  Tracked, then not$'
git rm -q --cached .ai/repo/project/issues/I0004.yaml
expect_exit 0 "$MJ" context
expect_grep '^## KNOWLEDGE POINTERS \(what issue I0004 references'
expect_grep '^stale        issue:I0004 is named by the task but is not in the knowledge graph: .ai/repo/project/issues/I0004.yaml is not in the version-control index yet \(git add it\)$'
expect_no_grep '^issue      issue          issue:I0004'
expect_grep '^scope      document       document:docs/guide.md  '
# a record that is gone is said to be gone
mv .ai/repo/project/issues/I0004.yaml I0004.yaml.away
expect_exit 0 "$MJ" context
expect_grep '^stale        issue:I0004 is named by the task but is not in the knowledge graph: .ai/repo/project/issues/I0004.yaml does not exist$'
mv I0004.yaml.away .ai/repo/project/issues/I0004.yaml
git add .ai/repo/project/issues/I0004.yaml
expect_exit 0 "$MJ" context
expect_no_grep '^stale '
expect_grep '^depends_on issue          issue:I0001  '
