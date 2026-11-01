# claim: derived-data-current
# A local gate reports and CI decides (I3830, project.a-push-runs-what-ci-would-find-first v2).
# The owner's relaxation of 2026-10-21: a check CI runs again prints its finding and lets the
# commit or push through; the integrity checks still refuse. This case runs the repository's
# real pre-commit and pre-push hooks, and the real scripts/ci/run-plan, in a fixture whose
# other checks are stubs with chosen exit codes. It proves, for each branch of the hooks:
#
#   pre-push    run-plan exits 1 (a gate failed)    exits 0, names the gate, CI holds the verdict
#               run-plan exits 1, the push carries  refused with 1, naming the top (rule v3)
#               a refs/heads/integration/train-*
#               ref with a commit
#               run-plan exits 12 (no plan)         refused with 12
#               finish --check fails                refused with its code
#               two cases share a number            refused with 1
#               rustdoc fails, the crate changed    exits 0 with a report line
#   pre-commit  doctor fails                        exits 0 with a report line
#               pages current, the worktree guard,
#               the SSH permissions check fail      refused with the code
#
# and that each call is still on a line doctor's wiring check (lib/doctor.sh) matches. The real
# `pages current` refusal of a stale derive is case 56's; case 96 drives the real guard.
. "$ROOT/test/lib.sh"

fail() { printf '    %s\n' "$*"; exit 1; }
A=1111111111111111111111111111111111111111

mkdir -p fx/.githooks fx/bin fx/scripts/ci
cp "$ROOT/.githooks/pre-push" "$ROOT/.githooks/pre-commit" fx/.githooks/
cp "$ROOT/scripts/ci/run-plan" fx/scripts/ci/run-plan
stub() { printf '#!/bin/sh\n%s\n' "$2" > "fx/$1"; chmod +x "fx/$1"; }
stub bin/majordomus 'case "$1" in finish) exit "${FINISH_RC:-0}" ;; doctor) echo "doctor: a finding"; exit "${DOCTOR_RC:-0}" ;; esac'
stub bin/majordomus-cli 'exit "${GUARD_RC:-0}"'
stub scripts/case-numbers 'echo "case 7 is claimed twice"; exit "${CASES_RC:-0}"'
stub scripts/rust-check 'echo "rustdoc: warning"; exit "${RUSTDOC_RC:-0}"'
stub scripts/pages 'exit "${PAGES_RC:-0}"'
stub scripts/ci/providers-check 'exit "${SSH_RC:-0}"'
# the plan run-plan computes: one structure gate, which fails when GATE_FAILS is set
stub scripts/ci-plan '[ -n "${PLAN_FAILS:-}" ] && exit 3
runs=true; [ -n "${GATE_FAILS:-}" ] && runs="echo the reason it failed; false"
printf "{\"schema\":1,\"mode\":\"test\",\"reason\":\"a fixture\",\"selected\":[],\"gates\":[{\"id\":\"economics-claims\",\"job\":\"structure\",\"runs\":\"%s\",\"selected\":true,\"network\":false,\"reason\":\"t\"}]}\n" "$runs"'
printf 'economics-claims\t2\n' > "$T/durations.tsv"; : > "$T/cases.tsv"
export MJ_PUSH_DURATIONS="$T/durations.tsv" MJ_PUSH_CASES="$T/cases.tsv"
unset MAJORDOMUS_BIN

push() { printf 'refs/heads/a %s refs/heads/a %s\n' "$1" "$A" | (cd fx && sh .githooks/pre-push origin url); }
pushA() { push "$A"; }
# a push of a train top, alone and beside a feature ref, and a train ref that is only deleted
T13=integration/train-13-20261031
pushTop() {
  printf 'refs/heads/%s %s refs/heads/%s %s\n' "$T13" "$A" "$T13" "$A" | (cd fx && sh .githooks/pre-push origin url)
}
pushBoth() {
  printf 'refs/heads/a %s refs/heads/a %s\nrefs/heads/%s %s refs/heads/%s %s\n' \
    "$A" "$A" "$T13" "$A" "$T13" "$A" \
    | (cd fx && sh .githooks/pre-push origin url)
}
pushGone() {
  printf 'refs/heads/a %s refs/heads/a %s\n(delete) %s refs/heads/%s %s\n' "$A" "$A" "${A//1/0}" "$T13" "$A" \
    | (cd fx && sh .githooks/pre-push origin url)
}
commit() { (cd fx && sh .githooks/pre-commit); }

echo "  pre-push: every check green, the push goes on"
expect_exit 0 pushA
expect_grep 'nothing CI would find here first'
expect_no_grep 'reported, not refused'

echo "  pre-push: a failed gate is named, reported, and CI holds the verdict"
GATE_FAILS=1 expect_exit 0 pushA
expect_grep 'FAILED economics-claims'
expect_grep 'failed: economics-claims\. CI would refuse this push for the same reason, and CI holds the verdict'
expect_grep '^pre-push: run-plan --before-push failed \(exit 1\).*; reported, not refused: CI holds the verdict$'

echo "  pre-push: a failed gate on a train top is refused, naming the top (rule v3)"
GATE_FAILS=1 expect_exit 1 pushTop
expect_grep 'FAILED economics-claims'
expect_grep "^pre-push: run-plan --before-push failed \(exit 1\).*; refused: this push carries the train top $T13, \
and a train top is judged before it is pushed$"
expect_no_grep 'reported, not refused'
GATE_FAILS=1 expect_exit 1 pushBoth
expect_grep "refused: this push carries the train top $T13,"
echo "  pre-push: a green train top goes on, and a deleted train ref is no top"
expect_exit 0 pushTop
expect_grep 'nothing CI would find here first'
GATE_FAILS=1 expect_exit 0 pushGone
expect_grep '; reported, not refused: CI holds the verdict$'
expect_no_grep 'refused: this push carries'

echo "  pre-push: an uncomputable plan, or no durations, refuses with 12"
PLAN_FAILS=1 expect_exit 12 pushA
expect_grep 'the plan could not be computed'
MJ_PUSH_DURATIONS="$T/no-such-file" expect_exit 12 pushA
expect_grep 'is missing, so nothing can be budgeted and the push is refused'

echo "  pre-push: a failed finish --check, and a duplicate case number, refuse"
FINISH_RC=10 expect_exit 10 pushA
expect_no_grep 'run-plan'
CASES_RC=1 expect_exit 1 pushA
expect_grep 'case 7 is claimed twice'
expect_no_grep 'run-plan'

echo "  pre-push: a push that changes the crate and whose rustdoc fails is reported"
(
  cd fx && git init -q . && git config user.email t@example.invalid && git config user.name t
  echo base > README && git add README && git commit -qm base
  git update-ref refs/remotes/origin/master HEAD
  mkdir -p apps/majordomus-cli/src && echo 'fn x() {}' > apps/majordomus-cli/src/x.rs
  git add apps && git commit -qm crate
)
H="$(git -C fx rev-parse HEAD)"
pushH() { push "$H"; }
RUSTDOC_RC=1 expect_exit 0 pushH
expect_grep 'rustdoc: warning'
expect_grep "^pre-push: the crate's rustdoc, warnings denied \(scripts/rust-check --doc\) failed \(exit 1\); reported, not refused: CI holds the verdict$"
expect_grep 'nothing CI would find here first'

echo "  pre-commit: every check green, the commit goes on"
expect_exit 0 commit
expect_no_grep 'reported, not refused'

echo "  pre-commit: a failed doctor is reported, and the integrity checks still run"
: > "$T/ran"; stub scripts/pages '[ -z "${RAN:-}" ] || echo pages >> "$RAN"; exit "${PAGES_RC:-0}"'
RAN="$T/ran" DOCTOR_RC=10 expect_exit 0 commit
expect_grep 'doctor: a finding'
expect_grep '^pre-commit: doctor failed \(exit 10\); reported, not refused: CI holds the verdict$'
grep -qx pages "$T/ran" || fail "a reported doctor stopped the hook before pages current"

echo "  pre-commit: pages current, the worktree guard and the SSH permissions refuse"
RAN="$T/ran" PAGES_RC=10 expect_exit 10 commit
GUARD_RC=10 expect_exit 10 commit
SSH_RC=1 expect_exit 1 commit

echo "  each call is on a line doctor's wiring check matches"
# the expression lib/doctor.sh applies: "<program>[[:space:]]+<first argument>([[:space:]]|$)"
for pair in 'pre-commit majordomus doctor' 'pre-commit majordomus-cli worktree' \
  'pre-commit pages current' 'pre-commit providers-check --permissions-only' \
  'pre-push majordomus finish' 'pre-push run-plan --before-push'; do
  # shellcheck disable=SC2086 # the words of one pair are its three fields
  set -- $pair
  grep -qE "$2[[:space:]]+$3([[:space:]]|\$)" "$ROOT/.githooks/$1" \
    || fail "doctor would not find '$2 $3' wired in .githooks/$1"
done
for want in 'scripts/case-numbers check >/dev/null || { scripts/case-numbers check >&2; exit 1; }' \
  'bin/majordomus finish --check || exit $?'; do
  grep -qxF "$want" "$ROOT/.githooks/pre-push" || fail "pre-push no longer refuses with: $want"
done
for want in 'bin/majordomus-cli worktree guard --quiet || exit $?' 'scripts/pages current || exit $?' \
  'scripts/ci/providers-check --permissions-only || exit $?'; do
  grep -qxF "$want" "$ROOT/.githooks/pre-commit" || fail "pre-commit no longer refuses with: $want"
done
