# majordomus-covers: none
# The release dispatches validate on its record's branch, planned as the record's change.
#
# The release pipeline proposes its record as a pull request opened with GITHUB_TOKEN, and
# GitHub starts no workflow from an event that token caused. After v0.15.0 and after v0.16.0
# the proposal sat BLOCKED with an empty check rollup — the run its `pull_request` event
# recorded concluded failure with no job in it — until somebody closed and reopened it, and
# the release's smoke phase failed by design for as long as that took.
#
# A dispatch is the one event that token may cause, so the pipeline dispatches validate on
# the record's branch, and validate plans that dispatch as the pull request's own run will
# be planned. That run is not what lets the record merge: on v0.19.1 (#866) and v0.27.0
# (#1273) its `ci` passed at the record's head and branch protection still refused the
# merge, and the rollups of #1142 and #1147 list no check at all (I2282). What starts the run
# the merge waits for is a person approving it, or closing and reopening the pull request,
# which `majordomus prs drain` performs (I2296) and case 1036 proves. This case observes
# neither a merge, an approval nor a reopen; it reads the two workflows, as case 87 reads the
# release workflow, and docs/DISTRIBUTION.md, and holds them in seven things:
#
#   1  the release workflow asks for validate on release/record-<tag>, with plan=change,
#      only when a record was proposed, after proposing it
#   2  a request that cannot be started is a warning naming the remedy, never the job's
#      failure: the release is published either way and smoke must still run
#   3  validate declares the input, and reads it from the environment — a workflow input
#      interpolated into a script is a script anyone who can dispatch may write
#   4  `change` is planned exactly as a pull request is; an empty input stays every gate
#      with the on-demand ones; anything else is refused by name
#   5  a schedule still plans every gate with the on-demand ones
#   6  nothing calls the dispatch the record's verdict: not the step's name, not its comment,
#      not the pull request it proposes, not docs/DISTRIBUTION.md — and none of them says the
#      pull request or its required check reads the dispatched check-runs
#   7  what lets the record merge is named where a lander reads it: the pull request's body,
#      the publish job's notice, the runbook and the failure table name the approval and the
#      close and reopen, and the documentation names the token remedy as the owner's
. "$ROOT/test/lib.sh"
RL="$ROOT/.github/workflows/release.yml"
VL="$ROOT/.github/workflows/validate.yml"
expect_file "$RL"
expect_file "$VL"

line_of() { grep -nF -- "$2" "$1" | head -n 1 | cut -d: -f1; }

# ---------------------------------------------------------------- 1. the request
ask="$(line_of "$RL" 'gh workflow run validate.yml --ref "$branch" -f plan=change')"
[ -n "$ask" ] || { echo "    the release workflow does not ask for validate on the record's branch with plan=change"; exit 1; }
grep -qF 'branch="release/record-${{ needs.plan.outputs.tag }}"' "$RL" \
  || { echo "    the branch the verdict is asked on is not the record's"; exit 1; }
propose="$(line_of "$RL" 'gh pr create --base')"
[ -n "$propose" ] && [ "$ask" -gt "$propose" ] \
  || { echo "    the verdict is asked for before the record is proposed (line $ask, proposal at ${propose:-none})"; exit 1; }
step="$(line_of "$RL" "- name: read the record's tree early; the record's verdict is its own pull request run")"
[ -n "$step" ] || { echo "    the step that dispatches validate on the record's branch is gone or renamed"; exit 1; }
sed -n "$((step + 1)),$((step + 2))p" "$RL" | grep -qF "if: steps.record.outputs.record_pr != ''" \
  || { echo "    the verdict is asked for whether or not a record was proposed"; exit 1; }

# ---------------------------------------------------------------- 2. a request that cannot start
body="$(sed -n "${step},$((step + 14))p" "$RL")"
printf '%s\n' "$body" | grep -qF 'if gh workflow run validate.yml' \
  || { echo "    the request is not guarded: a dispatch that cannot start would fail the job"; exit 1; }
printf '%s\n' "$body" | grep -qF '::warning::could not start validate' \
  || { echo "    a request that cannot start says nothing"; exit 1; }
printf '%s\n' "$body" | grep -qE '^[[:space:]]*exit [1-9]' \
  && { echo "    a request that cannot start fails the job, and smoke with it"; exit 1; }

# ---------------------------------------------------------------- 3. the input, declared and read safely
awk '/^  workflow_dispatch:/{f=1; next} f && /^  [a-z_]+:/{f=0} f' "$VL" > "$T/dispatch.txt"
grep -qE '^    inputs:' "$T/dispatch.txt" && grep -qE '^      plan:' "$T/dispatch.txt" \
  || { echo "    validate declares no workflow_dispatch input named plan"; exit 1; }
grep -qF 'PLAN_AS: ${{ github.event.inputs.plan }}' "$VL" \
  || { echo "    the input does not reach the plan step through its environment"; exit 1; }
n="$(grep -cF '${{ github.event.inputs.plan }}' "$VL")"
[ "$n" = 1 ] || { echo "    the input is interpolated $n time(s); once, into the environment, is the only safe place"; exit 1; }

# ---------------------------------------------------------------- 4. how each value is planned
awk '/workflow_dispatch\)/{f=1} f{print} f && /esac ;;/{exit}' "$VL" > "$T/arm.txt"
grep -qF "change) scripts/ci-plan --base HEAD^1 --head HEAD > plan.json ;;" "$T/arm.txt" \
  || { echo "    plan=change is not planned as a pull request is"; cat "$T/arm.txt"; exit 1; }
pr="$(grep -F '*) scripts/ci-plan --base HEAD^1 --head HEAD > plan.json ;;' "$VL" | head -n 1)"
[ -n "$pr" ] || { echo "    a pull request is no longer planned from HEAD^1 to HEAD, so 'as a pull request' names nothing"; exit 1; }
grep -qE "^[[:space:]]*''\) scripts/ci-plan --full .* --on-demand > plan.json ;;" "$T/arm.txt" \
  || { echo "    an empty input no longer plans every gate with the on-demand ones"; cat "$T/arm.txt"; exit 1; }
grep -qE '^[[:space:]]*\*\) echo "::error::.*"; exit 2 ;;' "$T/arm.txt" \
  || { echo "    an unknown value is not refused"; cat "$T/arm.txt"; exit 1; }

# ---------------------------------------------------------------- 5. the schedule is untouched
grep -qE '^[[:space:]]*schedule\) scripts/ci-plan --full "\$EVENT on \$REF" --on-demand > plan.json ;;' "$VL" \
  || { echo "    a scheduled run no longer plans every gate with the on-demand ones"; exit 1; }

# ---------------------------------------------------------------- 6. the dispatch is no verdict
DIST="$ROOT/docs/DISTRIBUTION.md"
expect_file "$DIST"
# the comment block above the step, from the last blank line before it
top="$(awk -v s="$step" 'NR < s && /^[[:space:]]*$/ {b = NR} END {print b}' "$RL")"
sed -n "$((top + 1)),$((step - 1))p" "$RL" > "$T/comment.txt"
grep -qF 'The dispatch below is not that verdict.' "$T/comment.txt" \
  || { echo "    the comment above the dispatch no longer says it is not the record's verdict"; exit 1; }
grep -qiE "asks? for the (record's )?verdict|ask(s|ing)? for the verdict itself" "$RL" "$DIST" \
  && { echo "    the workflow or docs/DISTRIBUTION.md still says the dispatch asks for the record's verdict:"; grep -niE "asks? for the (record's )?verdict|ask(s|ing)? for the verdict itself" "$RL" "$DIST"; exit 1; }
for f in "$RL" "$DIST"; do
  tr '\n' ' ' < "$f" | tr -s ' #' ' ' > "$T/flat.txt"
  grep -qiE 'which is what the pull request and its required check read' "$T/flat.txt" \
    && { echo "    $(basename "$f") says the pull request's required check reads the dispatched check-runs"; exit 1; }
done
tr '\n' ' ' < "$DIST" | tr -s ' ' > "$T/dist.txt"
grep -qF 'That run is not the record'"'"'s verdict.' "$T/dist.txt" \
  || { echo "    docs/DISTRIBUTION.md no longer says the dispatched run is not the record's verdict"; exit 1; }
grep -qF 'branch protection and the pull request'"'"'s check rollup read no check suite a dispatch created' "$T/dist.txt" \
  || { echo "    docs/DISTRIBUTION.md no longer says what branch protection does not read"; exit 1; }

# ---------------------------------------------------------------- 7. what lets it merge, named
body_line="$(grep -F -- '--body "Written by scripts/release-record' "$RL" | head -n 1)"
for want in 'approve its waiting validate run' 'close and reopen it' 'majordomus prs drain' \
  'not the merge'"'"'s verdict'; do
  printf '%s\n' "$body_line" | grep -qF "$want" \
    || { echo "    the record pull request's body does not say: $want"; exit 1; }
done
grep -qF '::notice::the release record is proposed in $url; it merges on a run of its own, which starts when a person approves its waiting run or closes and reopens it' "$RL" \
  || { echo "    the publish job's notice does not name the person's step"; exit 1; }
awk '/^# 6\. /,/^```$/' "$DIST" > "$T/runbook.txt"
grep -qF 'approve its' "$T/runbook.txt" && grep -qF 'close and reopen it' "$T/runbook.txt" \
  || { echo "    the release runbook does not name starting the record's own run as a step"; cat "$T/runbook.txt"; exit 1; }
row="$(grep -F '| `smoke`, while the record is a proposal |' "$DIST")"
for want in 'gh pr close <n>' 'gh pr reopen <n>' 'approve its waiting `validate` run' 'majordomus prs drain' \
  'the dispatched run'"'"'s `ci` does not let it merge'; do
  printf '%s\n' "$row" | grep -qF "$want" \
    || { echo "    the failure table's record row does not say: $want"; exit 1; }
done
grep -qF 'a credential only the repository'"'"'s owner can create and hold, and is the owner'"'"'s decision' "$T/dist.txt" \
  || { echo "    docs/DISTRIBUTION.md does not name the token remedy as the owner's decision"; exit 1; }

echo "    the release dispatches validate on the record's branch, planned as the change it is, and nothing calls it the record's verdict; a request that cannot start is a warning; the input is read from the environment and an unknown value is refused; the body, the notice, the runbook and the failure table name the person's step"
