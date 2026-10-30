# majordomus-covers: plan
# Merged work the plan has not recorded is drift, and on the trunk past a window it is red
# (I2741, ADR 0166 decision 2).
#
# `majordomus plan drift` sets every issue's declared status against its delivery
# (plan.delivery) and names each contradiction with its proof; scripts/ci/plan-drift-check
# judges it as a gate against the window .ai/repo/ci/plan.yaml declares. This drives both over
# a fixture repository:
#
#   I0001  READY,  merged by pull request #5 two days ago            merged_not_recorded, old
#   I0002  READY,  merged by pull request #6 a minute ago            merged_not_recorded, young
#   I0003  DONE,   named by no merge                                 done_without_trace
#   I0004  ACTIVE, started two days ago, no branch, PR or merge      active_without_work
#   I0005  ACTIVE, merged two days ago, a follow-up pull request open  no drift
#   I0006  READY,  merged inside a combined top whose merge names it nowhere in git; the
#          forge's title of pull request #8 names it                  merged_not_recorded, via the forge
#
# Every assertion names the mutation of the inference that breaks it. Each marked RUN was
# applied to apps/majordomus-cli/src/plan/drift.rs or delivery.rs on 2026-10-16, one at a
# time, and each failed this case where it says.
. "$ROOT/test/lib.sh"
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX
command -v jq >/dev/null 2>&1 || skip "jq absent"
fail() { printf '    %s\n' "$*"; exit 1; }
GATE="$ROOT/scripts/ci/plan-drift-check"
[ -x "$GATE" ] || fail "scripts/ci/plan-drift-check is not executable"

"$MJ" init >/dev/null
pj_init; pj_milestone M1
for i in I0001 I0002 I0003 I0004 I0005 I0006; do pj_issue "$i" M1; done
git add -A >/dev/null && git commit -qm "plan: six issues"
TRUNK="$(git rev-parse --abbrev-ref HEAD)"
OLD="$(date -u -r $(( $(date +%s) - 2 * 86400 )) +%Y-%m-%dT%H:%M:%SZ 2>/dev/null \
  || date -u -d '2 days ago' +%Y-%m-%dT%H:%M:%SZ)"

merge_pr() { # number branch title [date]
  git checkout -q -b "$2"
  printf 'echo %s\n' "$2" > "$(echo "$2" | tr / -).sh"; git add -A >/dev/null
  git commit -qm "wip"
  git checkout -q "$TRUNK"
  if [ -n "${4:-}" ]; then
    GIT_COMMITTER_DATE="$4" GIT_AUTHOR_DATE="$4" \
      git merge -q --no-ff "$2" -m "Merge pull request #$1 from someone/$2" -m "$3"
  else
    git merge -q --no-ff "$2" -m "Merge pull request #$1 from someone/$2" -m "$3"
  fi
  git branch -D -q "$2"
}
merge_pr 5 feature/old-work "feat: the old work (I0001)" "$OLD"
merge_pr 7 feature/first-half "feat: the first half (I0005)" "$OLD"
merge_pr 6 feature/young-work "feat: the young work (I0002)"
# a combined top: the member's commit and its merge name no issue; only the forge's title does
git checkout -q -b integration/top
printf 'echo top\n' > top.sh; git add top.sh; git commit -qm "chore: a member"
MEMBER="$(git rev-parse HEAD)"
git checkout -q "$TRUNK"
git merge -q --no-ff integration/top -m "Merge branch 'integration/top'"
TOP="$(git rev-parse HEAD)"
git branch -D -q integration/top

# the stamps: I0003 DONE with its evidence, I0004 and I0005 ACTIVE, I0004 since two days ago
"$MJ" plan start I0003 >/dev/null
"$MJ" plan evidence I0003 --covers proof --type test --command true --result "it held" >/dev/null
"$MJ" plan verify I0003 >/dev/null && "$MJ" plan 'done' I0003 >/dev/null
MAJORDOMUS_NOW="$OLD" "$MJ" plan start I0004 >/dev/null && "$MJ" plan start I0005 >/dev/null
git add -A >/dev/null && git commit -qm "plan: stamps"
[ "$(pj_status I0003):$(pj_status I0004):$(pj_status I0001)" = DONE:ACTIVE:READY ] \
  || fail "the fixture's stamps did not derive: $("$MJ" plan list)"

# the forge observed: a follow-up of I0005 is open, and #8 merged the combined top
mkdir -p .ai/local/state/integration
cat > .ai/local/state/integration/observation.json <<'J'
{"schema":4,"repository":"someone/fixture","base":"master","base_sha":"0000000000000000000000000000000000000000",
 "observed_at":"2026-10-16T08:00:00Z","required_checks":null,"review_policy":null,"merge_methods":["merge"],
 "pull_requests":[{"number":9,"title":"feat: the second half (I0005)","author":"someone","head_ref":"feature/second-half",
  "head_sha":"1111111111111111111111111111111111111111","base_ref":"master","draft":false,
  "created_at":"2026-10-16T07:00:00Z","updated_at":"2026-10-16T07:30:00Z"}]}
J
observe_merged() { # merged_pull_requests JSON, or nothing for an observation without them
  printf '{"schema":1,"repository":"someone/fixture","repository_url":"https://example.invalid/f","observed_at":"2026-10-16T09:00:00Z","deployments":[],"releases":[]%s}\n' \
    "${1:+,\"merged_pull_requests\":$1}" > .ai/local/state/integration/deployments.json
}
observe_merged "[{\"number\":8,\"title\":\"feat: a piece merged in a top (I0006)\",\"head_ref\":\"feature/in-a-top\",\"head_sha\":\"$MEMBER\",\"merge_commit\":\"$TOP\",\"merged_at\":\"2026-10-16T08:00:00Z\"}]"

model() { # window enforcement
  mkdir -p .ai/repo/ci
  printf 'version: 1\ndrift:\n  window: %s\n  enforcement: %s\n' "$1" "$2" > .ai/repo/ci/plan.yaml
}
model 3600 blocking

dr() { "$MJ" --json plan drift > dr.json 2> dr.err || { cat dr.err; fail "plan drift exited non-zero"; }; }
find_() { jq -r --arg i "$1" "[.findings[] | select(.issue == \$i)] | $2" dr.json; }

# ---------------------------------------------------------------- the report
dr
# MUTATION, RUN: never find merged_not_recorded (`if false && d.merged.is_some()` in judge)
# -> I0001 has no finding.
[ "$(find_ I0001 '.[0].class')" = merged_not_recorded ] || fail "I0001 is not merged_not_recorded: $(find_ I0001 .)"
[ "$(find_ I0001 '.[0].proof.pull_request')" = 5 ] || fail "I0001's proof does not name pull request #5"
[ "$(find_ I0001 '.[0].proof.merge_commit')" = "$(git log --merges --format=%H --grep '#5 ' -1)" ] \
  || fail "I0001's proof does not name its merge commit"
[ "$(find_ I0001 '.[0].past_window')" = true ] || fail "a merge two days old is not past an hour's window"
[ "$(find_ I0002 '.[0].class'):$(find_ I0002 '.[0].past_window')" = merged_not_recorded:false ] \
  || fail "I0002 is not a young merged_not_recorded: $(find_ I0002 .)"
# MUTATION, RUN: ignore the open pull request (`&& d.pr_open.is_none()` dropped) -> I0005 is a finding.
[ "$(find_ I0005 length)" = 0 ] || fail "an open follow-up of I0005 was called drift: $(find_ I0005 .)"
[ "$(find_ I0003 '.[0].class'):$(find_ I0003 '.[0].advisory')" = done_without_trace:true ] \
  || fail "I0003 is not an advisory done_without_trace: $(find_ I0003 .)"
[ "$(find_ I0004 '.[0].class')" = active_without_work ] || fail "I0004 is not active_without_work: $(find_ I0004 .)"
# MUTATION, RUN: read no forge title (the merged-titles loop in delivery::infer skipped) -> I0006 none.
[ "$(find_ I0006 '.[0].class'):$(find_ I0006 '.[0].proof.via')" = merged_not_recorded:forge_title ] \
  || fail "the forge's title of #8 did not name I0006: $(find_ I0006 .)"
[ "$(find_ I0006 '.[0].proof.pull_request'):$(find_ I0006 '.[0].proof.merge_commit')" = "8:$TOP" ] \
  || fail "I0006's proof is not #8 and its merge"
[ "$(jq -r .verdict dr.json):$(jq -r .exit_code dr.json)" = report:0 ] || fail "a report without a gate judged something"
"$MJ" plan drift > dr.txt
grep -q 'I0001 .*merged_not_recorded #5 ' dr.txt || { cat dr.txt; fail "the text does not name I0001's pull request"; }

# ---------------------------------------------------------------- the gate
gate() { # tree -> exit code, output in gate.txt
  local rc=0
  MJ_ROOT="$PWD" MJ_DRIFT_ON="$1" "$GATE" > gate.txt 2>&1 || rc=$?
  echo "$rc"
}
# MUTATION, RUN: refuse nothing on the trunk (`(DriftGate::Trunk, Some(true)) => Pending`) -> exit 0.
[ "$(gate trunk)" = 10 ] || { cat gate.txt; fail "the trunk's gate did not refuse I0001 past the window"; }
grep -q '^REFUSED .*I0001 .*majordomus plan verify|done I0001' gate.txt || { cat gate.txt; fail "the refusal does not name the settling command"; }
grep -q '^PENDING .*I0002 ' gate.txt || { cat gate.txt; fail "I0002, inside the window, is not pending"; }
# MUTATION, RUN: refuse done_without_trace (`DriftClass::DoneWithoutTrace => DriftVerdict::Refused`)
# -> I0003 reads REFUSED, not ADVISORY.
grep -q '^ADVISORY .*I0003 ' gate.txt || { cat gate.txt; fail "done_without_trace is not advisory"; }
grep -q '^NOTE .*I0004 ' gate.txt || { cat gate.txt; fail "active_without_work is not reported"; }
# the fact taken away: inside a wider window the old merge is pending too
model 864000 blocking
[ "$(gate trunk)" = 0 ] || { cat gate.txt; fail "the trunk's gate refused inside a ten-day window"; }
grep -q '^PENDING .*I0001 ' gate.txt || { cat gate.txt; fail "I0001 inside the window is not pending"; }
# a head is not the shared plan
model 3600 blocking
[ "$(gate head)" = 0 ] || { cat gate.txt; fail "a head's gate refused"; }
grep -q '^PENDING .*I0001 ' gate.txt || { cat gate.txt; fail "on a head I0001 is not pending"; }
# advisory enforcement reports the refusal and passes
model 3600 advisory
[ "$(gate trunk)" = 0 ] || { cat gate.txt; fail "advisory enforcement failed the gate"; }
grep -q '^REFUSED .*I0001 ' gate.txt && grep -q 'advisory' gate.txt || { cat gate.txt; fail "advisory enforcement hid the refusal"; }
# done_without_trace never refuses: with only I0003 drifting, the trunk is clean
model 3600 blocking
rc=0; MJ_ROOT="$PWD" MJ_DRIFT_ON=trunk "$MJ" plan drift I0003 --gate trunk > gate.txt 2>&1 || rc=$?
[ "$rc" = 0 ] || { cat gate.txt; fail "done_without_trace refused the trunk (exit $rc)"; }
# a missing or unreadable window refuses
# MUTATION, RUN: judge without a window (`Some(_) if !window_read` arm dropped) -> exit 0, not 12.
rm .ai/repo/ci/plan.yaml
[ "$(gate head)" = 12 ] || { cat gate.txt; fail "a missing window did not refuse"; }
grep -q 'cannot be read' gate.txt || { cat gate.txt; fail "the refusal does not say the window is missing"; }
printf 'version: 1\ndrift:\n  window: soon\n  enforcement: blocking\n' > .ai/repo/ci/plan.yaml
[ "$(gate trunk)" = 12 ] || { cat gate.txt; fail "an unreadable window did not refuse"; }

# ---------------------------------------------------------------- unread is not none
# an observation without merged pull requests leaves the titles unread, and I0006 undetected
# by them; the source says so instead of calling the forge empty
model 3600 blocking
observe_merged ""
dr
[ "$(find_ I0006 length)" = 0 ] || fail "I0006 was found with no title read"
jq -e '.sources[] | select(.name == "merged_titles") | .state == "unread"' dr.json >/dev/null \
  || fail "the merged titles are not reported unread"
rm .ai/local/state/integration/observation.json
dr
[ "$(find_ I0001 '.[0].settled'):$(find_ I0001 '.[0].unread | join(",")')" = false:forge ] \
  || fail "an unread forge left I0001 settled: $(find_ I0001 .)"

# ---------------------------------------------------------------- this repository's own window
# The gate this repository runs reads its own model, and lands advisory (I2748 makes it blocking).
[ "$(sed -n 's/^  enforcement: *//p' "$ROOT/.ai/repo/ci/plan.yaml")" = advisory ] \
  || fail "the repository's drift gate is not advisory"
grep -q 'runs: scripts/ci/plan-drift-check' "$ROOT/.ai/repo/ci/gates.yaml" || fail "the gate is not registered"
