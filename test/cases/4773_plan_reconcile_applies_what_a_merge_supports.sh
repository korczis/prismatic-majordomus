# majordomus-covers: plan
# plan reconcile proposes the moves a merge supports and applies them only through
# plan.transition (I4773, ADR 0238).
#
# A fixture plan where I0001 is READY and merged by pull request #5, and I0002 is READY and
# delivered by a commit whose subject names nothing but whose `Delivers:` trailer does. This
# drives the shell's `majordomus plan reconcile`, which hands the command to the executable:
#
#   - without a forge observation the proposal is unsettled and `--apply` writes nothing (12);
#   - with one, `--apply` starts both issues through plan.transition, sealed, and never
#     writes verify or done;
#   - a second run proposes no start, and `--done` writes nothing while the evidence is owed;
#   - once evidence is recorded after the merge, `--apply --done` writes DONE.
. "$ROOT/test/lib.sh"
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX
command -v jq >/dev/null 2>&1 || skip "jq absent"
fail() { printf '    %s\n' "$*"; exit 1; }

"$MJ" init >/dev/null
pj_init; pj_milestone M1
for i in I0001 I0002; do pj_issue "$i" M1; done
git add -A >/dev/null && git commit -qm "plan: two issues"
TRUNK="$(git rev-parse --abbrev-ref HEAD)"
git checkout -q -b feature/the-work
printf 'echo work\n' > work.sh; git add work.sh; git commit -qm "wip"
git checkout -q "$TRUNK"
git merge -q --no-ff feature/the-work -m "Merge pull request #5 from someone/feature/the-work" -m "feat: the work (I0001)"
printf 'echo second\n' > second.sh; git add second.sh
git commit -qm "Add: [Lib] The second piece" -m "A grammar with no room for an id." -m "Delivers: I0002"

out="$("$MJ" --json plan reconcile)" || fail "plan reconcile failed: $out"
[ "$(printf '%s' "$out" | jq -r '.settled')" = false ] || fail "an unobserved forge is not settled: $out"
[ "$(printf '%s' "$out" | jq -r '[.proposals[] | select(.issue == "I0002") | .merge.via][0]')" = trailer ] \
  || fail "the Delivers: trailer is I0002's edge: $out"
rc=0; "$MJ" plan reconcile --apply >/dev/null || rc=$?
[ "$rc" = 12 ] || fail "an unsettled proposal is applied with exit 12, got $rc"
[ "$(pj_status I0001)" = READY ] || fail "nothing is written from an unsettled proposal"

mkdir -p .ai/local/state/integration
cat > .ai/local/state/integration/observation.json <<'J'
{"schema":4,"repository":"someone/fixture","base":"master","base_sha":"0000000000000000000000000000000000000000",
 "observed_at":"2026-11-25T08:00:00Z","required_checks":null,"review_policy":null,"merge_methods":["merge"],
 "pull_requests":[]}
J
out="$("$MJ" plan reconcile --apply)" || fail "apply failed: $out"
printf '%s\n' "$out" | grep -q 'written  start I0001 -> ACTIVE' || fail "I0001 is started: $out"
printf '%s\n' "$out" | grep -q 'written  start I0002 -> ACTIVE' || fail "I0002 is started: $out"
[ "$(pj_status I0001):$(pj_status I0002)" = ACTIVE:ACTIVE ] || fail "both are ACTIVE: $("$MJ" plan list)"
grep -q '^started_event: ' .ai/repo/project/issues/I0001.yaml || fail "the start is sealed by its event"
grep -q '^verified_at:' .ai/repo/project/issues/I0001.yaml && fail "verify is never written"

out="$("$MJ" --json plan reconcile --apply --done)" || fail "a second apply failed: $out"
[ "$(printf '%s' "$out" | jq -r '.applied | length')" = 0 ] || fail "nothing is left to write: $out"
[ "$(printf '%s' "$out" | jq -r '.proposals[] | select(.issue == "I0001") | .class')" = unaccepted ] \
  || fail "ACTIVE, merged, owing evidence is unaccepted: $out"

"$MJ" plan evidence I0001 --covers proof --type test --command true --result "it held" >/dev/null \
  || fail "the evidence could not be recorded"
out="$("$MJ" plan reconcile --apply --done)" || fail "apply --done failed: $out"
printf '%s\n' "$out" | grep -q 'written  done I0001 -> DONE' || fail "I0001 is done on fresh evidence: $out"
[ "$(pj_status I0001)" = DONE ] || fail "I0001 is DONE: $("$MJ" plan list)"
[ "$(pj_status I0002)" = ACTIVE ] || fail "I0002 still owes its evidence"
