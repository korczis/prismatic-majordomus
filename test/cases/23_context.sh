# The context builder: authority order, what each profile's context block selects, the
# budget and its named exclusions, resolution of the right records, and determinism.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib docs && echo a > lib/a && echo d > docs/d && git add . && git commit -qm base

# with no task at all: still useful, and says what to do next
expect_exit 0 "$MJ" context
expect_grep '^## GIT'
expect_grep '^branch       '"$(git branch --show-current)"'$'
expect_grep '^head         '"$(git rev-parse HEAD)"'$'
expect_grep '^## TASK'
expect_grep 'none active — run: majordomus start'
expect_grep '^## BUDGET'
# absence of a record is reported as absence, never as an unrelated match
expect_grep '^- handover — no record for this worktree and branch'

"$MJ" start "fix the callback" --scope lib --profile debugging --owner alice >/dev/null
id=$(sed -n 's/^id: //p' .ai/local/state/current.yaml)

# sections appear in authority order: git, task, profile, questions, ... history last
expect_exit 0 "$MJ" context
order=$(printf '%s\n' "$LAST_OUT" | grep -E '^## ' | tr '\n' ' ')
case "$order" in "## GIT ## TASK ## PROFILE"*) ;; *) echo "    wrong section order: $order"; exit 1 ;; esac
case "$order" in *"## EXCLUDED ## BUDGET "*) ;; *) echo "    excluded/budget not last: $order"; exit 1 ;; esac
expect_grep '^id           '"$id"'$'
expect_grep '^task         fix the callback$'
expect_grep '^owner        alice$'
expect_grep '^scope        lib$'
expect_grep '^## PROFILE debugging'
expect_grep '^capability   strong$'
expect_grep '^effort       high$'
expect_grep 'verify command, regression test'
# read-only: nothing was written
expect_no_grep 'context' .ai/local/state/ledger.jsonl

# blockers are shown with why they matter, and are never dropped
"$MJ" question add "which environments still run the old client?" >/dev/null
expect_exit 0 "$MJ" context
expect_grep '^## OPEN QUESTIONS \(1 unresolved'
expect_grep 'refuses finish --outcome completed'
expect_grep '^- which environments still run the old client\?'

# the profile's context block decides what is included, and exclusions are always named.
# debugging: decisions true, relevant_files true, architecture_notes false, history 50
"$MJ" decision add "one store, not two" --why "a second source of truth drifts" >/dev/null
echo b >> lib/a
expect_exit 0 "$MJ" context
expect_grep '^## DECISIONS \(5 most recent for this task\)'
expect_grep 'one store, not two'
expect_grep '^## FILES TOUCHED IN SCOPE'
expect_grep '^lib/a$'
expect_grep '^## RECENT HISTORY \(last 50 events\)'
# failing_output is asked for by the profile and honestly reported as not captured
expect_grep '^- failing_output — profile debugging asks for it'

# routine turns most of it off, and every omission is named with the field that caused it
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
git add -A && git commit -qm w1
"$MJ" start "bump a version" --scope lib --profile routine >/dev/null
expect_exit 0 "$MJ" context
expect_no_grep '^## DECISIONS'
expect_no_grep '^## FILES TOUCHED IN SCOPE'
expect_no_grep '^## RECENT HISTORY'
expect_grep '^- decisions — profile routine sets context.decisions: false$'
expect_grep '^- relevant_files — profile routine sets context.relevant_files: false$'
expect_grep '^- history — profile routine sets context.recent_history_depth: 0$'
# deep-work asks for repository-wide decisions instead of task-scoped ones
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
"$MJ" start "design it" --scope lib --profile deep-work >/dev/null
expect_exit 0 "$MJ" context
expect_grep '^## DECISIONS \(5 most recent in this repository\)'
expect_grep 'one store, not two'          # from the earlier task: repository scope, not task scope

# the resolved handover is the one for this worktree and branch, with its git relationship
expect_grep '^## LATEST COMPATIBLE HANDOVER'
expect_grep '^match        same_worktree_same_branch$'
expect_grep '^git state    '
# ... and a record from an unrelated branch is never offered
git checkout -qb elsewhere
expect_exit 0 "$MJ" context
expect_no_grep '^## LATEST COMPATIBLE HANDOVER'
expect_grep '^- handover — no record for this worktree and branch'
git checkout -q -

# the newest checkpoint for this task is included whole
printf 'reproduced with the fixture\nnext: write the test\n' | "$MJ" checkpoint >/dev/null
expect_exit 0 "$MJ" context
expect_grep '^## LATEST CHECKPOINT'
expect_grep '^reproduced with the fixture$'

# --budget-lines drops the least reliable evidence first and names every drop
expect_exit 0 "$MJ" context --budget-lines 50
expect_grep '^- history — context budget 50 lines$'
expect_grep '^## GIT'         # never dropped
expect_grep '^## TASK'
expect_grep 'dropped:.*history'
# the reported number is the number of lines actually printed, not of the sections alone
n=$(printf '%s\n' "$LAST_OUT" | wc -l | tr -d ' ')
[ "$n" -le 50 ] || { echo "    printed $n lines under a budget of 50"; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -qE "^$n of 50 lines" || { echo "    budget line does not report $n"; exit 1; }
# a record body degrades to a pointer rather than vanishing
expect_exit 0 "$MJ" context --budget-lines 44
expect_grep 'body omitted for budget — read .ai/local/state/'
expect_grep '^## LATEST CHECKPOINT'          # the section header survives
# what cannot be dropped is over budget: exit 10, and it still prints so the worker sees why
expect_exit 10 "$MJ" context --budget-lines 5
expect_grep 'over the budget of 5'
expect_grep '^## GIT'
expect_exit 2 "$MJ" context --budget-lines x

# the policy's own budget is the default
sed -i.bak 's/^  builder_budget_lines: 300/  builder_budget_lines: 50/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
expect_exit 0 "$MJ" context
expect_grep 'of 50 lines'
expect_grep '^- history — context budget 50 lines$'
sed -i.bak 's/^  builder_budget_lines: 50/  builder_budget_lines: 300/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak

# --for names a provider from the policy and refuses one that is not there
expect_exit 0 "$MJ" context --for claude-code
expect_grep '^# provider claude-code — its always-loaded instructions are CLAUDE.md$'
expect_grep '^## GIT'
expect_exit 2 "$MJ" context --for nosuch
expect_grep 'no provider .nosuch.'
expect_grep 'have:.*claude-code'

# deterministic: the same state produces the same body, timestamp line aside
a=$("$MJ" context | grep -v '^# Majordomus context'); b=$("$MJ" context | grep -v '^# Majordomus context')
[ "$a" = "$b" ] || { echo "    context is not deterministic"; exit 1; }

# --json describes the same selection as the text form
if command -v jq >/dev/null; then
  "$MJ" --json context > c.json
  jq -e '.schema == 1' c.json >/dev/null
  jq -e --arg i "$id" '.task.profile == "deep-work"' c.json >/dev/null
  jq -e '.git.head == "'"$(git rev-parse HEAD)"'"' c.json >/dev/null
  jq -e '.task.scope == ["lib"]' c.json >/dev/null
  jq -e '[.sections[].id] | index("checkpoint")' c.json >/dev/null
  jq -e '.budget.limit == 300' c.json >/dev/null
  "$MJ" --json context --budget-lines 50 > d.json
  jq -e '.excluded | map(select(.reason == "context budget 50 lines")) | length > 0' d.json >/dev/null
  # the json budget count is the same count the text form printed
  jq -e '.budget.lines == '"$("$MJ" context --budget-lines 50 | wc -l | tr -d ' ')" d.json >/dev/null
  jq -e '.budget.dropped | index("history")' d.json >/dev/null
  jq -e '[.sections[].id] | index("history") == null' d.json >/dev/null
fi

# a task naming a profile that no longer exists degrades instead of crashing
rm .ai/repo/profiles/deep-work.yaml
expect_exit 0 "$MJ" context
expect_grep '^- profile deep-work — the task names a profile with no file'
expect_grep '^## GIT'
