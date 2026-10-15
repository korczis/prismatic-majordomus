# majordomus-covers: none
# A release record's verdict is the newest check on its head, not merely a check on it (I2717).
#
# v0.26.0's publish job dispatched validate on release/record-v0.26.0 at 19:07:23Z and that run
# concluded `ci` success on the record's head f937ac3870. Two seconds later GitHub recorded the
# record pull request's own `pull_request` run (#1147, opened by github-actions[bot], conclusion
# action_required) under the same app, and a pull request shows only the newest check suite of
# each app: the rollup was empty, the required `ci` read as missing and the record stayed
# BLOCKED until a person merged it. The same masking is what refused v0.19.1's record (I2282).
# And the dispatched run's `evidence` job was red on every record: the rust job ran for gates
# that test no crate, and evidence-collect found nothing to record.
#
# This case runs the release workflow's own step, cut out of release.yml, against a `gh` that
# keeps the runs of one branch in a file and stamps each with a logical clock, and holds:
#
#   1  the dispatch is made only once the pull request's run exists
#   2  a pull request run newer than the dispatch is answered with exactly one more dispatch
#   3  no pull request run within the bound: one dispatch, said, and the step passes
#   4  a dispatch that cannot start is a warning and the step passes; it is not retried
#   5  validate's evidence job runs only when the plan selected a lane that leaves a report
# evidence token: record_pr_verdict_can_pass_case
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RL="$ROOT/.github/workflows/release.yml"
VL="$ROOT/.github/workflows/validate.yml"
expect_file "$RL"
expect_file "$VL"

# --- the step, as release.yml runs it ------------------------------------------------------
start="$(grep -nF -- "- name: ask for the record's verdict, as the newest check on its head" "$RL" | head -n 1 | cut -d: -f1)"
[ -n "$start" ] || { echo "    release.yml has no step asking for the record's verdict as the newest check"; exit 1; }
awk -v s="$start" '
  NR <= s {next}
  /^      - (name|uses):/ {exit}
  body {sub(/^          /, ""); print; next}
  /^        run: \|/ {body = 1}' "$RL" \
  | sed 's/\${{ needs\.plan\.outputs\.tag }}/v9.9.9/g' > "$T/step.sh"
[ -s "$T/step.sh" ] || { echo "    the step has no run block"; exit 1; }
grep -q '\${{' "$T/step.sh" && { echo "    the step interpolates an expression other than the tag:"; grep -n '\${{' "$T/step.sh"; exit 1; }

# a repository whose HEAD stands for the record's head
REPO="$T/repo"; mkdir -p "$REPO"
git -C "$REPO" init -q
git -C "$REPO" -c user.name=t -c user.email=t@t commit -q --allow-empty -m record
HEAD_SHA="$(git -C "$REPO" rev-parse HEAD)"

# --- a gh that knows one branch's runs ------------------------------------------------------
# state: $STATE/runs.json, a JSON array of {event, headSha, createdAt}; $STATE/clock, a counter
# stamped into createdAt; $STATE/script, what happens on each `run list` (APPEAR_PR_AT: the list
# call after which the pull request run appears; PR_AFTER_DISPATCH=1: it appears only after the
# first dispatch, two ticks later); FAIL_DISPATCH=1 refuses `workflow run`.
SHIM="$T/shim"; mkdir -p "$SHIM"
cat > "$SHIM/gh" <<'GH'
#!/usr/bin/env bash
set -u
S="$GH_STATE"
tick() { c="$(cat "$S/clock")"; c=$((c + 1)); echo "$c" > "$S/clock"; printf '2026-10-14T19:%02d:%02dZ' $((c / 60)) $((c % 60)); }
add() { at="$(tick)"; jq --arg e "$1" --arg h "$HEAD_SHA" --arg a "$at" '. + [{event: $e, headSha: $h, createdAt: $a}]' "$S/runs.json" > "$S/runs.tmp" && mv "$S/runs.tmp" "$S/runs.json"; }
case "$1 $2" in
  "workflow run")
    echo "dispatch" >> "$S/log"
    [ "${FAIL_DISPATCH:-0}" = 1 ] && { echo "HTTP 403" >&2; exit 1; }
    add workflow_dispatch
    if [ "${PR_AFTER_DISPATCH:-0}" = 1 ] && [ ! -f "$S/pr-made" ]; then tick > /dev/null; add pull_request; touch "$S/pr-made"; fi
    exit 0 ;;
  "run list")
    n="$(cat "$S/lists" 2>/dev/null || echo 0)"; n=$((n + 1)); echo "$n" > "$S/lists"
    if [ -n "${APPEAR_PR_AT:-}" ] && [ "$n" -ge "$APPEAR_PR_AT" ] && [ ! -f "$S/pr-made" ]; then add pull_request; touch "$S/pr-made"; echo "pr-run" >> "$S/log"; fi
    shift 2; ev=""; q="."
    while [ $# -gt 0 ]; do case "$1" in --event) ev="$2"; shift 2 ;; --jq) q="$2"; shift 2 ;; *) shift ;; esac; done
    jq --arg ev "$ev" '[.[] | select(.event == $ev)]' "$S/runs.json" | jq -r "$q"
    exit 0 ;;
esac
echo "unexpected gh $*" >&2; exit 3
GH
chmod 0755 "$SHIM/gh"

run_step() { # <name> [VAR=value ...]: runs the step in a fresh state, prints its output
  name="$1"; shift
  st="$T/state-$name"; mkdir -p "$st"; echo '[]' > "$st/runs.json"; echo 0 > "$st/clock"; : > "$st/log"
  ( cd "$REPO" && env "$@" GH_STATE="$st" HEAD_SHA="$HEAD_SHA" PATH="$SHIM:$PATH" \
      VERDICT_WAIT_TRIES=4 VERDICT_WAIT_SECONDS=0 bash -e "$T/step.sh" ) > "$st/out" 2>&1
  echo $? > "$st/exit"
}
dispatches() { grep -c '^dispatch$' "$T/state-$1/log"; }

# --- 1. the dispatch waits for the pull request's run ---------------------------------------
run_step waits APPEAR_PR_AT=3
[ "$(cat "$T/state-waits/exit")" = 0 ] || { echo "    the step failed:"; sed 's/^/      /' "$T/state-waits/out"; exit 1; }
[ "$(head -n 1 "$T/state-waits/log")" = pr-run ] \
  || { echo "    the dispatch was made before the pull request's run existed:"; sed 's/^/      /' "$T/state-waits/log"; exit 1; }
[ "$(dispatches waits)" = 1 ] || { echo "    $(dispatches waits) dispatches where the dispatch was already the newest; one is right"; exit 1; }

# --- 2. a newer pull request run is answered once --------------------------------------------
run_step masked PR_AFTER_DISPATCH=1
[ "$(cat "$T/state-masked/exit")" = 0 ] || { echo "    the step failed:"; sed 's/^/      /' "$T/state-masked/out"; exit 1; }
[ "$(dispatches masked)" = 2 ] \
  || { echo "    a pull request run newer than the dispatch was answered with $(dispatches masked) dispatch(es); two are right"; sed 's/^/      /' "$T/state-masked/out"; exit 1; }
grep -q 'would hide its verdict' "$T/state-masked/out" || { echo "    the second dispatch does not say why it was made"; exit 1; }
newest="$(jq -r 'sort_by(.createdAt) | last | .event' "$T/state-masked/runs.json")"
[ "$newest" = workflow_dispatch ] || { echo "    after the step the newest run is a $newest run, not the dispatch"; exit 1; }

# --- 3. no pull request run: one dispatch, said ----------------------------------------------
run_step none
[ "$(cat "$T/state-none/exit")" = 0 ] || { echo "    the step failed when no pull request run appeared"; sed 's/^/      /' "$T/state-none/out"; exit 1; }
[ "$(dispatches none)" = 1 ] || { echo "    $(dispatches none) dispatches with no pull request run; one is right"; exit 1; }
grep -q 'dispatching anyway' "$T/state-none/out" || { echo "    the step does not say it found no pull request run"; exit 1; }

# --- 4. a dispatch that cannot start is a warning, and the step passes -------------------------
run_step refused APPEAR_PR_AT=1 FAIL_DISPATCH=1
[ "$(cat "$T/state-refused/exit")" = 0 ] || { echo "    a refused dispatch failed the step, and the release's smoke with it"; exit 1; }
[ "$(dispatches refused)" = 1 ] || { echo "    a refused dispatch was tried $(dispatches refused) times; once is right"; exit 1; }
grep -qF '::warning::could not start validate' "$T/state-refused/out" || { echo "    a refused dispatch says nothing"; exit 1; }

# --- 5. evidence runs only for a plan with a lane that leaves a report -------------------------
cond="$(awk '/^  evidence:/{p=1; next} p && /^  [a-z]/{exit} p' "$VL" | awk '/^    if: /{f=1} f && /^    [a-z-]+:/ && !/^    if: /{exit} f')"
[ -n "$cond" ] || { echo "    validate's evidence job has no condition"; exit 1; }
for want in "needs.plan.outputs.shell_suite == 'true'" "needs.plan.outputs.rust_check == 'true'" "needs.plan.outputs.rust_integration == 'true'"; do
  printf '%s\n' "$cond" | grep -qF -- "$want" \
    || { echo "    the evidence job does not ask the plan for: $want"; printf '%s\n' "$cond" | sed 's/^/      /'; exit 1; }
done
exit 0
