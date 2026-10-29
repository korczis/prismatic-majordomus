# majordomus-covers: none
# claims: none
# An optional step is bounded inside its own bound, so that when it runs out of time it
# degrades as it was written to, rather than being killed and failing the job (I4380).
#
# On 2026-10-29 the `trends` step of pages.yml ran `scripts/pages trends || echo
# "::warning ..."` under `timeout-minutes: 4`. Reading sixty runs one call at a time took
# longer; GitHub killed the step ("The action 'trends' has timed out after 4 minutes"), a step
# killed by its own bound is a failed step and `||` never ran, and publish was skipped on runs
# 40796031540, 40803295553 and 40823254353. scripts/ci/step-bound-check now refuses a step
# that degrades with `|| ... ::warning` and declares its own minutes, unless every degrading
# command runs under `timeout N` or `scripts/ci/timed --bound N` whose bounds and grace add up
# to less than those minutes, or the step is continue-on-error.
. "$ROOT/test/lib.sh"

CHECK="$ROOT/scripts/ci/step-bound-check"
W="$ROOT/.github/workflows/pages.yml"
expect_file "$CHECK"

# --- this repository's publication: every optional step is bounded inside its own bound
expect_exit 0 "$CHECK" "$W"
expect_grep 'every optional step'
awk '/- name: trends$/ { f = 1 } f && /run:/ { print; exit }' "$W" | grep -q 'timeout --kill-after=10 200 scripts/pages trends ||' \
  || { echo "    the trends step does not bound its reading inside its own four minutes"; exit 1; }
[ "$(awk '/- name: the forge, observed$/ { f = 1; next } f && /- name: / { exit } f' "$W" | grep -c 'timeout --kill-after=10 135 ')" = 2 ] \
  || { echo "    the forge observation does not bound both of its reads inside its own five minutes"; exit 1; }

# --- the broken shape of run 40823254353: the step's own bound kills it, and || never runs
cat > unbounded.yml <<'Y'
jobs:
  deploy:
    timeout-minutes: 80
    steps:
      - name: trends
        timeout-minutes: 4
        run: scripts/pages trends || echo "::warning title=No trends::could not read the forge"
Y
expect_exit 10 "$CHECK" unbounded.yml
expect_grep 'unbounded.yml:7  job deploy — the step degrades with \|\| ::warning, but the command it degrades is not bounded inside the step.s own 4m'

# --- bounded, but the bounds and their grace add up past the step's minutes
cat > summed.yml <<'Y'
jobs:
  ui-snapshot:
    timeout-minutes: 45
    steps:
      - name: the forge, observed
        timeout-minutes: 4
        run: |
          timeout --kill-after=10 135 majordomus prs refresh \
            || echo "::warning title=Forge not observed::prs refresh"
          timeout --kill-after=10 135 majordomus served forge \
            || echo "::warning title=Forge not observed::served forge"
Y
expect_exit 10 "$CHECK" summed.yml
expect_grep 'the bounds inside it add up to 290s, not under the step.s own 4m \(240s\)'

# --- a continuation line whose command is unbounded is judged on the line before it
cat > continued.yml <<'Y'
jobs:
  ui-snapshot:
    timeout-minutes: 45
    steps:
      - name: the forge, observed
        timeout-minutes: 5
        run: |
          majordomus prs refresh \
            || echo "::warning title=Forge not observed::prs refresh"
Y
expect_exit 10 "$CHECK" continued.yml
expect_grep 'continued.yml:9  job ui-snapshot — the step degrades'

# --- the shapes that hold: a bound inside the step, in minutes or seconds or through
#     scripts/ci/timed; a continue-on-error step; and a degrading step with no minutes of its
#     own, which only its job can stop and which this section does not judge
cat > fine.yml <<'Y'
jobs:
  deploy:
    timeout-minutes: 80
    steps:
      - name: trends
        timeout-minutes: 4
        run: timeout --kill-after=10 200 scripts/pages trends || echo "::warning title=No trends::x"
      - name: minutes
        timeout-minutes: 4
        run: timeout -k 5s 3m scripts/pages trends || { echo "::warning::x"; }
      - name: timed
        timeout-minutes: 3
        run: scripts/ci/timed --bound 100 pages trends rows.tsv scripts/pages trends || echo "::warning::x"
      - name: reported
        continue-on-error: true
        timeout-minutes: 4
        run: scripts/pages trends || echo "::warning::x"
      - name: a failed branch run ends here
        if: failure()
        run: gh run cancel "$GITHUB_RUN_ID" || echo "::warning::could not be cancelled"
Y
expect_exit 0 "$CHECK" fine.yml

echo "    an optional step is bounded inside its own bound, so it degrades instead of being killed"
