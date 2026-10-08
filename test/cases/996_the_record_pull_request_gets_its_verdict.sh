# majordomus-covers: none
# The release record's pull request gets a verdict without a person.
#
# The release pipeline proposes its record as a pull request opened with GITHUB_TOKEN, and
# GitHub starts no workflow from an event that token caused. After v0.15.0 and after v0.16.0
# the proposal sat BLOCKED with an empty check rollup — the run its `pull_request` event
# recorded concluded failure with no job in it — until somebody closed and reopened it, and
# the release's smoke phase failed by design for as long as that took.
#
# A dispatch is the one event that token may cause. So the pipeline dispatches validate on
# the record's branch, and validate plans that dispatch as the pull request's own run would
# have been planned. This case reads the two workflows, as case 87 reads the release
# workflow, and holds five things:
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
step="$(line_of "$RL" "- name: ask for the record's verdict, because its pull request starts none")"
[ -n "$step" ] || { echo "    the step that asks for the record's verdict is gone or renamed"; exit 1; }
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

echo "    the release asks for the record's verdict on its own branch, planned as the change it is; a request that cannot start is a warning; the input is read from the environment and an unknown value is refused"
