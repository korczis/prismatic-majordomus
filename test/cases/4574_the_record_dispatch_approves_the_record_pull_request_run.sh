# majordomus-covers: none
# The record's dispatch approves the record pull request's own run (I2717, evidence
# record_pr_verdict_can_pass_case).
#
# The release opens its record pull request with GITHUB_TOKEN, and GitHub creates that pull
# request's validate run waiting for approval: on 2026-11-09 v1.0.1's (#2991, run
# 42668789214) waited until a person approved it through the API, while the dispatched
# validate 42668785190 stood green and counted for nothing. scripts/ci/record-run-approve,
# run by the dispatch's plan job, approves the waiting run. This case feeds it the forge
# through a stub `gh` on PATH, offline, and reads what it asked for:
#
#   1  a branch that is no release/record- branch is refused before the forge is asked
#   2  the waiting validate run of pull_request at this branch and head is approved, and only
#      it: another head, another branch, another event, another workflow and a run that does
#      not wait are never approved
#   3  nothing waits: `none`, exit 0, after the bounded number of readings, nothing posted
#   4  the forge refuses the approval: `refused <id>`, exit 10; the runs cannot be read: 12
#   5  it writes no status and no check: approving the pull request's own run is the remedy,
#      a `ci` posted from the dispatch would be a forged verdict
#   6  the wiring: validate.yml's plan job runs it on a dispatch of a record branch alone,
#      before the plan, and a failure of it is a warning, never the dispatch's failure
#   7  the verdict names an `abandoned` job (attempt 2 of 42668789214 never created its site
#      job) with the act that recovers it, and stays red for it
. "$ROOT/test/lib.sh"
RA="$ROOT/scripts/ci/record-run-approve"
V="$ROOT/.github/workflows/validate.yml"
VD="$ROOT/scripts/ci/verdict"
[ -x "$RA" ] || { echo "    scripts/ci/record-run-approve is missing or not executable"; exit 1; }
expect_file "$V"

FX="$T/fx"; LOG="$T/gh.log"; mkdir -p "$T/bin" "$FX"
cat > "$T/bin/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$GH_STUB_LOG"
[ "$1" = api ] || { echo "stub gh: only api" >&2; exit 1; }
shift
case "$*" in
  "-X POST "*"/approve")
    [ ! -f "$GH_STUB_FX/refuse" ] || { echo "HTTP 403: Resource not accessible by integration" >&2; exit 1; }
    echo '{}' ;;
  *"/actions/runs?"*)
    [ ! -f "$GH_STUB_FX/down" ] || { echo "HTTP 503: the forge is down" >&2; exit 1; }
    cat "$GH_STUB_FX/runs.json" ;;
  *) echo "stub gh: unexpected call $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$T/bin/gh"
export PATH="$T/bin:$PATH" GH_STUB_LOG="$LOG" GH_STUB_FX="$FX"
export MJ_RECORD_APPROVE_TRIES=2 MJ_RECORD_APPROVE_WAIT=0
unset GITHUB_ACTIONS GITHUB_STEP_SUMMARY 2>/dev/null || true

B=release/record-v9.9.9; S=1111111111111111111111111111111111111111
run() { : > "$LOG"; out="$("$RA" --repo o/r --branch "$1" --sha "$S" 2>"$T/err")" && code=0 || code=$?; }
run_json() {
  # id, event, branch, sha, path, status, conclusion
  printf '%s\n' "$@" | jq -R -s -c 'split("\n") | map(select(. != "") | split(" ")
    | {id: (.[0] | tonumber), event: .[1], head_branch: .[2], head_sha: .[3], path: .[4],
       status: .[5], conclusion: (if .[6] == "null" then null else .[6] end)})
    | {workflow_runs: .}' > "$FX/runs.json"
}
O=2222222222222222222222222222222222222222
VP=.github/workflows/validate.yml

# ---------------------------------------------------------------- 1. only a record branch
run_json "7 pull_request feature/x $S $VP action_required null"
run feature/x
[ "$code" = 2 ] || { echo "    a feature branch was not refused (exit $code)"; exit 1; }
[ ! -s "$LOG" ] || { echo "    the forge was asked for a branch that is no record:"; cat "$LOG"; exit 1; }

# ---------------------------------------------------------------- 2. the waiting run, and only it
run_json \
  "11 pull_request $B $O $VP action_required null" \
  "12 pull_request release/record-v9.9.8 $S $VP action_required null" \
  "13 workflow_dispatch $B $S $VP action_required null" \
  "14 pull_request $B $S .github/workflows/pages.yml action_required null" \
  "15 pull_request $B $S $VP completed failure" \
  "16 pull_request $B $S $VP completed action_required"
run "$B"
[ "$code" = 0 ] && [ "$out" = "approved 16" ] || { echo "    the waiting run 16 was not approved (exit $code): $out"; cat "$T/err"; exit 1; }
grep -qxF -- "api -X POST repos/o/r/actions/runs/16/approve" "$LOG" || { echo "    run 16 was not approved through the forge:"; cat "$LOG"; exit 1; }
[ "$(grep -c -- '-X POST' "$LOG")" = 1 ] || { echo "    more than the one waiting run was approved:"; cat "$LOG"; exit 1; }
grep -qF "head_sha=$S" "$LOG" || { echo "    the runs were not read at the record's head"; cat "$LOG"; exit 1; }
# a run whose status itself says it waits is the same run
run_json "21 pull_request $B $S $VP action_required null"
run "$B"
[ "$out" = "approved 21" ] || { echo "    a run with status action_required was not approved: $out"; exit 1; }

# ---------------------------------------------------------------- 3. nothing waits
run_json "11 pull_request $B $O $VP action_required null" "15 pull_request $B $S $VP in_progress null"
run "$B"
[ "$code" = 0 ] && [ "$out" = none ] || { echo "    nothing waiting did not read none, 0 (exit $code): $out"; exit 1; }
[ "$(grep -c 'actions/runs?' "$LOG")" = 2 ] || { echo "    the readings were not bounded by MJ_RECORD_APPROVE_TRIES:"; cat "$LOG"; exit 1; }
! grep -q -- '-X POST' "$LOG" || { echo "    something was approved with nothing waiting:"; cat "$LOG"; exit 1; }

# ---------------------------------------------------------------- 4. refused, unreadable
run_json "16 pull_request $B $S $VP completed action_required"
touch "$FX/refuse"
run "$B"
[ "$code" = 10 ] && [ "$out" = "refused 16" ] || { echo "    a refused approval did not read refused, 10 (exit $code): $out"; exit 1; }
grep -qF 'Resource not accessible' "$T/err" || { echo "    a refusal does not say what the forge answered"; cat "$T/err"; exit 1; }
rm -f "$FX/refuse"; touch "$FX/down"
run "$B"
[ "$code" = 12 ] || { echo "    unreadable runs did not exit 12 (exit $code): $out"; exit 1; }
rm -f "$FX/down"

# ---------------------------------------------------------------- 5. no status, no check
grep -v '^#' "$RA" | grep -qE 'statuses|check-runs|check_runs' \
  && { echo "    record-run-approve writes a status or a check; only the pull request's own run may say ci"; exit 1; }

# ---------------------------------------------------------------- 6. the wiring
awk '/^  plan:$/{f=1; next} f && /^  [a-z-]+:$/{exit} f' "$V" > "$T/plan.yml"
step="$(grep -nF -- "- name: the record's own pull request run is approved" "$T/plan.yml" | head -n 1 | cut -d: -f1)"
[ -n "$step" ] || { echo "    validate.yml's plan job does not approve the record's own run"; exit 1; }
planned="$(grep -nF -- '- id: plan' "$T/plan.yml" | head -n 1 | cut -d: -f1)"
[ -n "$planned" ] && [ "$step" -lt "$planned" ] || { echo "    the approval comes after the plan (line $step, plan at ${planned:-none})"; exit 1; }
body="$(sed -n "${step},$((step + 5))p" "$T/plan.yml")"
printf '%s\n' "$body" | grep -qF "if: github.event_name == 'workflow_dispatch' && startsWith(github.ref_name, 'release/record-')" \
  || { echo "    the approval is not limited to a dispatch of a record branch"; printf '%s\n' "$body"; exit 1; }
printf '%s\n' "$body" | grep -qE 'run: scripts/ci/record-run-approve .*\|\| echo "::warning::' \
  || { echo "    a failed approval would fail the dispatch; it must be a warning"; printf '%s\n' "$body"; exit 1; }

# ---------------------------------------------------------------- 7. an abandoned job, named
cat > "$T/plan.json" <<'JSON'
{"schema":1,"mode":"change","reason":"r","gates":[{"id":"site-build","job":"site","selected":true,"reason":"x"}]}
JSON
printf '{"plan":{"result":"success"},"site":{"result":"abandoned"}}\n' > "$T/needs.json"
vcode=0; "$VD" --plan "$T/plan.json" --needs "$T/needs.json" --receipts "$T/none" --summary "$T/summary.md" > "$T/v.out" 2>&1 || vcode=$?
[ "$vcode" = 1 ] || { echo "    an abandoned selected job did not make the verdict red (exit $vcode)"; cat "$T/v.out"; exit 1; }
grep -qF 'GitHub never created the job (abandoned)' "$T/summary.md" \
  || { echo "    the verdict does not name an abandoned job and its remedy"; cat "$T/summary.md"; exit 1; }
echo "    ok: the record's waiting run is approved, only it; refusals warn; abandoned is named"
