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
# that degrades with a `::warning` and declares its own minutes, unless every degrading command
# runs under `timeout N` or `scripts/ci/timed --bound N` and every bounded command of the step,
# with its grace, adds up to less than those minutes, or the step is continue-on-error. The
# forge observation is two steps, each with a bound measured from runs that published: `prs
# refresh` took 128-174 s with 77-87 pull requests open (runs 40573560897, 40652175281,
# 40696771044), about 1.2 s a head fetched, so its 135 s bound would have killed a healthy
# forge with the 124 open now; it runs under 420 s in 8 minutes, `served forge` (11-12 s)
# under 135 s in 3.
. "$ROOT/test/lib.sh"

CHECK="$ROOT/scripts/ci/step-bound-check"
W="$ROOT/.github/workflows/pages.yml"
expect_file "$CHECK"

# --- this repository's publication: every optional step is bounded inside its own bound
expect_exit 0 "$CHECK" "$W"
expect_grep 'every optional step'
awk '/- name: trends$/ { f = 1 } f && /run:/ { print; exit }' "$W" | grep -q 'timeout --kill-after=10 200 scripts/pages trends ||' \
  || { echo "    the trends step does not bound its reading inside its own four minutes"; exit 1; }
step_of() { awk -v n="$1" '$0 == "      - name: " n { f = 1; next } f && /^      - / { exit } f' "$W"; }
step_of 'the pull requests, observed' > prs.step
grep -q '^        timeout-minutes: 8$' prs.step && grep -q 'timeout --kill-after=10 420 .*majordomus prs refresh' prs.step \
  || { echo "    prs refresh is not bounded at 420 s inside its own eight minutes"; cat prs.step; exit 1; }
step_of 'the deployments and releases, observed' > served.step
grep -q '^        timeout-minutes: 3$' served.step && grep -q 'timeout --kill-after=10 135 .*majordomus served forge' served.step \
  || { echo "    served forge is not bounded at 135 s inside its own three minutes"; cat served.step; exit 1; }
grep -q 'served forge' prs.step && { echo "    the two reads of the forge share one step and one bound again"; exit 1; }

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
expect_grep 'unbounded.yml:7  job deploy — the step degrades with a ::warning, but the command it degrades is not bounded inside the step.s own 4m'

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

# --- a bounded command that does not degrade still uses the step's time: it is added up
cat > blocking.yml <<'Y'
jobs:
  deploy:
    timeout-minutes: 80
    steps:
      - name: trends
        timeout-minutes: 4
        run: |
          # timeout 999 in a comment is not a command
          timeout 150 scripts/pages evidence
          timeout --kill-after=10 100 scripts/pages trends || echo "::warning::x"
Y
expect_exit 10 "$CHECK" blocking.yml
expect_grep 'the bounds inside it add up to 260s, not under the step.s own 4m'

# --- the degrading shapes a line-by-line reading missed: `|| {` with the warning in its block,
#     and `if ! cmd; then` with the warning before its `fi`
cat > blocks.yml <<'Y'
jobs:
  deploy:
    timeout-minutes: 80
    steps:
      - name: braced
        timeout-minutes: 4
        run: |
          scripts/pages trends || {
            echo "::warning title=No trends::could not read the forge"
          }
      - name: negated
        timeout-minutes: 4
        run: |
          if ! scripts/pages trends; then
            echo "::warning title=No trends::could not read the forge"
          fi
      - name: in its else
        timeout-minutes: 4
        run: |
          if gh workflow run validate.yml; then
            echo started
          else
            echo "::warning::could not start validate"
          fi
Y
expect_exit 10 "$CHECK" blocks.yml
expect_grep 'blocks.yml:8  job deploy — the step degrades'
expect_grep 'blocks.yml:14  job deploy — the step degrades'
expect_grep 'blocks.yml:20  job deploy — the step degrades'

# --- the limit, said out loud: a command with no bound that does not degrade is taken to be
#     quick and is not judged (scripts/ci/step-bound-check, section 5). Closing it changes this
cat > limit.yml <<'Y'
jobs:
  deploy:
    timeout-minutes: 80
    steps:
      - name: trends
        timeout-minutes: 4
        run: |
          sleep 1000
          timeout --kill-after=10 100 scripts/pages trends || echo "::warning::x"
Y
expect_exit 0 "$CHECK" limit.yml

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
      - name: timed, spelled with =
        timeout-minutes: 3
        run: scripts/ci/timed --bound=100 pages trends rows.tsv scripts/pages trends || echo "::warning::x"
      - name: braced and bounded
        timeout-minutes: 4
        run: |
          timeout --kill-after=10 200 scripts/pages trends || {
            echo "::warning::x"
          }
      - name: a test is not a command that can hang
        timeout-minutes: 1
        run: |
          if [ -f trends.json ]; then echo kept; else echo "::warning::no trends"; fi
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
