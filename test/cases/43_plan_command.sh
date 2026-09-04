# majordomus-covers: plan
# majordomus-negative: plan
# The plan command surface: every subcommand reachable, self-documenting, and refusing
# what it does not understand. The list is read from the source rather than written here,
# so a subcommand added to lib/plan.sh is checked here from the moment it exists.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null

SUBS="$(grep -oE '^    [a-z|]+\) sub="\$1"' "$ROOT/lib/plan.sh" | sed -E 's/^ *//; s/\).*//' | tr '|' '\n' | sed '/^$/d')"
[ -n "$SUBS" ] || { echo "    the subcommand table in lib/plan.sh changed shape; update this case"; exit 1; }
for s in validate status list show ready blocked waves graph next body start verify evidence "done"; do
  printf '%s\n' "$SUBS" | grep -qx "$s" || { echo "    plan lost the $s subcommand"; exit 1; }
done

# --- help, and refusal, work before any model exists
expect_exit 0 "$MJ" plan --help
expect_grep 'usage: majordomus plan'
for s in $SUBS; do printf '%s\n' "$LAST_OUT" | grep -qE "^  $s" || { echo "    plan --help does not list $s"; exit 1; }; done
expect_exit 2 "$MJ" plan --no-such-option
expect_grep 'unknown (option|subcommand)'
expect_exit 2 "$MJ" plan nonsense
expect_grep "unknown subcommand 'nonsense'"
expect_exit 2 "$MJ" plan
expect_grep 'usage: majordomus plan'

pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000 I0001

# --- every read subcommand answers, and each names the thing it is about
for s in validate status list ready blocked waves graph next; do
  expect_exit 0 "$MJ" plan "$s" || { echo "    plan $s failed"; exit 1; }
done
expect_exit 0 "$MJ" plan next
expect_grep 'I0001'
expect_grep 'majordomus plan start I0001'

# --- show and body render the canonical record, and say where the canonical record is
expect_exit 0 "$MJ" plan show I0001
expect_grep '## Objective'
expect_grep '## Acceptance Criteria'
expect_grep '## Dependencies'
expect_grep '\.majordomus/project/issues/I0001\.yaml'
expect_exit 0 "$MJ" plan show M000
expect_grep '## Issue DAG'
expect_grep 'flowchart LR'
expect_exit 12 "$MJ" plan show I9999
expect_grep "no milestone or issue 'I9999'"
expect_exit 2 "$MJ" plan show
expect_grep 'needs an id'

# --- machine-readable output for the read subcommands that a surface consumes
expect_exit 0 "$MJ" --json plan validate
expect_grep '"issues":2'
expect_exit 0 "$MJ" --json plan status
expect_grep '"active_milestone":"M000"'
expect_grep '"next_ready":"I0001"'
expect_exit 0 "$MJ" --json plan list
expect_grep '"id":"I0002","milestone":"M000","status":"BLOCKED"'
expect_exit 0 "$MJ" --json plan next
expect_grep '"id":"I0001"'

# --- restricting to a milestone restricts every view that takes it
pj_milestone M001 1
pj_issue I0100 M001
expect_exit 0 "$MJ" plan list --milestone M001
expect_grep 'I0100'
expect_no_grep 'I0001'
expect_exit 0 "$MJ" plan graph --milestone M001
expect_no_grep 'I0002'

# --- the transitions refuse what the graph forbids, and each refusal has its own exit code
expect_exit 15 "$MJ" plan start I0002
expect_exit 12 "$MJ" plan start I9999
expect_exit 2 "$MJ" plan start
expect_exit 15 "$MJ" plan verify I0001      # not started yet
expect_grep 'only an ACTIVE issue can move to VERIFY'
expect_exit 2 "$MJ" plan evidence I0001 --covers proof
expect_grep 'needs --type'
expect_exit 2 "$MJ" plan evidence I0001 --type nonsense --covers proof --command true
expect_grep "unknown evidence type 'nonsense'"

# --- a transition is recorded in the append-only ledger, like every other durable act
"$MJ" plan start I0001 >/dev/null
grep -q '"event":"plan_start"' .majordomus/state/ledger.jsonl || { echo "    plan start wrote no ledger event"; exit 1; }
"$MJ" plan evidence I0001 --covers proof --type test --command "true" --result ok >/dev/null
grep -q '"event":"plan_evidence"' .majordomus/state/ledger.jsonl || { echo "    plan evidence wrote no ledger event"; exit 1; }
"$MJ" plan "done" I0001 >/dev/null
grep -q '"event":"plan_done"' .majordomus/state/ledger.jsonl || { echo "    plan done wrote no ledger event"; exit 1; }
expect_exit 0 "$MJ" history --validate

# --- the read subcommands never write
before="$(find .majordomus/project -type f -exec shasum -a 256 {} \; | sort)"
for s in validate status list ready blocked waves graph next; do "$MJ" plan "$s" >/dev/null 2>&1 || true; done
"$MJ" plan show I0001 >/dev/null; "$MJ" plan body M000 >/dev/null
after="$(find .majordomus/project -type f -exec shasum -a 256 {} \; | sort)"
[ "$before" = "$after" ] || { echo "    a read subcommand wrote to the canonical model"; exit 1; }
