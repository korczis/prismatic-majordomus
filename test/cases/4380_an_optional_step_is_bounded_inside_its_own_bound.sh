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

# --- a forge outage leaves the deployed stage unknown, never red (I4380 with I4315, ADR 0227).
#     `served forge` says whether it observed the deployments; the step after it judges what
#     Pages published only when it did. Merged without that, a hung or failed `served forge`
#     left the publication unread, the deployed-stage step failed ui-snapshot, and publish was
#     skipped: the defect this issue removes. Both steps are run as written, against a stub
#     executable, so the shape is proved by what they do and not by what they say.
script_of() { step_of "$1" | awk '/^        run: \|$/ { f = 1; next } f { sub(/^          /, ""); print }'; }
script_of 'the deployments and releases, observed' > served.sh
script_of 'the deployed stage reads what Pages published' > stage.sh
[ -s served.sh ] && [ -s stage.sh ] \
  || { echo "    the forge's deployment step or the deployed-stage step is missing"; exit 1; }
grep -q 'steps.forge.outputs.observed' <(step_of 'the deployed stage reads what Pages published') \
  || { echo "    the deployed-stage step does not read whether the forge was observed"; exit 1; }
grep -q '^        id: forge$' served.step \
  || { echo "    the deployments step has no id the deployed-stage step can read"; exit 1; }
mkdir -p apps/majordomus-cli/target/debug runner
stub="apps/majordomus-cli/target/debug/majordomus"
judge() { # $1 FORGE_OBSERVED, then the stub's body
  printf '#!/usr/bin/env bash\n%s\n' "$2" > "$stub"; chmod +x "$stub"
  : > summary.md
  FORGE_OBSERVED="$1" FORGE_REASON="${3:-}" GITHUB_STEP_SUMMARY="$PWD/summary.md" \
    RUNNER_TEMP="$PWD/runner" bash -e stage.sh > stage.out 2>&1
}
observe() { # the stub's body; writes observed to forge.out
  printf '#!/usr/bin/env bash\n%s\n' "$1" > "$stub"; chmod +x "$stub"
  : > forge.out
  GITHUB_OUTPUT="$PWD/forge.out" bash -e served.sh > served.out 2>&1
}
command -v timeout >/dev/null 2>&1 || { echo "    timeout(1) is needed to run the forge step"; exit 1; }
observe 'exit 12' || { echo "    a failed served forge failed its step"; cat served.out; exit 1; }
grep -qx 'observed=false' forge.out && grep -qx 'reason=served forge exit 12' forge.out \
  && grep -q '::warning title=Forge not observed::.*(exit 12)' served.out \
  || { echo "    a failed served forge did not say it observed nothing"; cat forge.out served.out; exit 1; }
observe 'exit 0' && grep -qx 'observed=true' forge.out \
  || { echo "    a served forge that answered did not say so"; cat forge.out; exit 1; }
unread='echo "{\"sources\":[{\"name\":\"publication\",\"state\":\"unread\",\"detail\":\"neither path\"}],\"tallies\":{}}"'
read_ok='echo "{\"sources\":[{\"name\":\"publication\",\"state\":\"read\",\"detail\":\"releases/latest.json\"}],\"tallies\":{}}"'
# an unobserved forge: the stage is unknown with its reason, in the log and the step
# summary; what was counted is still shown; `deployed` is never judged, even when the
# reader would call the publication unread
counted='echo "{\"sources\":[{\"name\":\"publication\",\"state\":\"unread\",\"detail\":\"neither path\"}],\"tallies\":{\"done\":3}}"'
judge false "$counted" 'served forge exit 12' \
  || { echo "    the forge was not observed and the deployed stage failed the job"; cat stage.out; exit 1; }
grep -q '::warning title=The deployed stage is unknown::forge not observed (served forge exit 12)' stage.out \
  && grep -q 'The deployed stage is unknown::forge not observed (served forge exit 12)' summary.md \
  && grep -qx '{"done":3}' stage.out \
  && ! grep -q 'deployed stage is unread' stage.out && ! grep -q '^publication:' stage.out \
  || { echo "    an unobserved forge did not leave the deployed stage unknown, with its reason and tallies"; cat stage.out summary.md; exit 1; }
judge false 'exit 3' 'served forge exit 124' \
  || { echo "    an unobserved forge whose delivery read also failed failed the job"; cat stage.out; exit 1; }
judge '' 'exit 3' || { echo "    a forge step that wrote no output failed the job"; cat stage.out; exit 1; }
judge true "$unread" && { echo "    an unread publication after an observed forge stayed green"; cat stage.out; exit 1; }
grep -q '::error title=The deployed stage is unread::neither path' stage.out \
  || { echo "    an unread publication was not named red"; cat stage.out; exit 1; }
judge true "$read_ok" || { echo "    a read publication failed the job"; cat stage.out; exit 1; }

echo "    an optional step is bounded inside its own bound, so it degrades instead of being killed;"
echo "    a forge outage leaves the deployed stage unknown, and an unread publication stays red"
