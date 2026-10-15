# claim: derived-data-current
# A push runs what CI would find first (project.a-push-runs-what-ci-would-find-first).
#
# On 2026-10-09 CI refused a release twice for what a local run finds in seconds: a structure
# gate and a one-second case. Nothing ran either before the push. `scripts/ci/run-plan
# --before-push`, which the pre-push hook calls, runs a budget of the plan's structure gates,
# chosen from the seconds CI recorded, and the suite's fast cases. This case drives its
# choice over a plan and durations it writes. It proves four things: a cheap gate runs and
# its failure refuses the push; a gate that reaches the network, one with no recorded
# duration, one over the per-gate limit and one past the budget are each left out with a line
# that says why; a missing durations file or an uncomputable plan refuses the push; and a case
# that reads stdin no longer hangs the runner. A fifth, since 2026-10-10: nothing it runs
# inherits the repository git hands a hook, so a fixture cannot write into the one being pushed.
# A sixth, since 2026-10-15: a gate that builds the crate (`builds: true`) is left to CI, and
# the model refuses a gate that calls cargo without saying so. CI recorded skills-proof at 5 s
# and economics-claims at 6 s with the build cached; on a workstation each compiled the crate
# first, and the hook took 641, 1178 and 1693 s against its bound of 180.
# A seventh, since 2026-10-15: what runs is what fits the bound on this machine. Without the
# building gates the hook still took 305-455 s, because CI's seconds priced the cheap gates
# at ~120 s and the fast cases at a fraction of what they cost here. Each candidate is now
# priced at what it took here last, the cheapest taken until the bound is spent; the case
# fails when the step's measured time passes its bound, when a gate or case left to CI is not
# named, or when the last line's count disagrees with the names.
. "$ROOT/test/lib.sh"

R="$ROOT/scripts/ci/run-plan"
fail() { printf '    %s\n' "$*"; exit 1; }

# ---------------------------------------------------------------- the choice
gate() { printf '{"id":"%s","job":"structure","runs":"%s","selected":true,"network":%s,"builds":%s,"reason":"t"}' "$1" "$2" "$3" "${4:-false}"; }
plan_with() {
  printf '{"schema":1,"mode":"test","reason":"a fixture","selected":[],"gates":[%s]}' "$(IFS=,; echo "$*")" > "$T/plan.json"
}
# Every gate and case left to CI is named, and the last line counts them: a gate on a line of
# its own, a case on a line of its own or among the names of a line that counts them.
named_and_counted() {
  local g c many
  g="$(grep -cE '^  left out [^ ]+:' <<<"$LAST_OUT" || true)"
  c="$(grep -cE '^  left out case [^ ]+:' <<<"$LAST_OUT" || true)"
  many="$(awk '/^  left out [0-9]+ case\(s\) / { n = $3; sub(/^[^:]*:/, ""); if (NF != n) { print "x"; exit } s += n } END { print s + 0 }' <<<"$LAST_OUT")"
  [ "$many" != x ] || fail "a line that counts the cases left to CI does not name as many: $(grep -E '^  left out [0-9]+ case' <<<"$LAST_OUT" | cut -c1-200)"
  grep -qE "left to CI: $g gate\(s\) and $(( c + many )) case\(s\), each named above" <<<"$LAST_OUT" \
    || fail "the step left $g gate(s) and $(( c + many )) case(s) to CI by name, and its last line counts otherwise: $(tail -n 1 <<<"$LAST_OUT")"
}
# What this machine measured is the fixture's, never the machine's own file.
export MJ_PUSH_LOCAL="$T/local.tsv"
printf 'cheap-ok\t2\ncheap-fails\t3\nnetworked\t1\ncompiles\t1\ntoo-slow\t400\nbudget-a\t10\nbudget-b\t12\ncold\t3\n' > "$T/durations.tsv"
printf '00_yaml_flatten\t1\n' > "$T/cases.tsv"
printf 'gate\tcheap-ok\t0\ngate\tcheap-fails\t0\ngate\tbudget-a\t1\ngate\tbudget-b\t50\ncase\t00_yaml_flatten\t500\n' > "$MJ_PUSH_LOCAL"
export MJ_PUSH_DURATIONS="$T/durations.tsv" MJ_PUSH_CASES="$T/cases.tsv" MJ_PUSH_BOUND=30 MJ_PUSH_RESERVE=0
plan_with "$(gate cheap-ok true false)" "$(gate cheap-fails 'echo the reason it failed; false' false)" \
  "$(gate networked true true)" "$(gate compiles 'touch compiled' false true)" "$(gate unrecorded true false)" \
  "$(gate too-slow true false)" "$(gate budget-a true false)" "$(gate budget-b true false)" "$(gate cold true false)"
expect_exit 1 "$R" --before-push --plan "$T/plan.json"
expect_grep '  ok     cheap-ok'
expect_grep '  FAILED cheap-fails .*: echo the reason it failed; false'
expect_grep '\| the reason it failed'
expect_grep 'left out networked: it reaches the network'
expect_grep 'left out compiles: it builds the crate'
[ ! -e "$ROOT/compiled" ] || { rm -f "$ROOT/compiled"; fail 'a gate that builds the crate ran before the push'; }
expect_grep 'left out unrecorded: no recorded duration'
expect_grep 'left out too-slow: CI recorded 400s, over the 15s a push spends on one gate'
# priced at what it took here, not at CI's 12 s
expect_grep 'left out budget-b: about 50s here \(measured here\), past what is left of the 30s bound'
expect_grep '  ok     budget-a \([0-9]+s; priced 1s\)'
# never measured here: CI's 3 s times this machine's ratio for the gates it did measure, 51/27
expect_grep '  ok     cold \([0-9]+s; priced 6s\)'
expect_grep 'left out case 00_yaml_flatten: about 500s here \(measured here\)'
expect_grep 'left out [0-9]+ case\(s\) with no recorded duration in .*: .*1021_a_push_runs_what_ci_would_find_first'
expect_grep 'failed: cheap-fails. CI would refuse this push for the same reason'
expect_grep 'against a bound of 30s'
named_and_counted
# what each gate took here is kept for the next push
grep -qE '^gate	cold	[0-9]+$' "$MJ_PUSH_LOCAL" || fail "the step did not record what the gate cold took here: $(cat "$MJ_PUSH_LOCAL")"
# without the failing gate, the push goes on
plan_with "$(gate cheap-ok true false)" "$(gate networked true true)"
expect_exit 0 "$R" --before-push --plan "$T/plan.json"
expect_grep 'nothing CI would find here first'
named_and_counted
# over the bound is a report, never a refusal: a busy machine must not fail a push. Priced at
# 0 s from an earlier run, the gate takes 4 against a bound of 3.
printf 'gate\tcheap-ok\t0\n' >> "$MJ_PUSH_LOCAL"
plan_with "$(gate cheap-ok 'sleep 4' false)"
MJ_PUSH_BOUND=3 expect_exit 0 "$R" --before-push --plan "$T/plan.json"
expect_grep 'over it by [0-9]+s; slowest: cheap-ok [0-9]+s; .*the push is not refused for that'
expect_grep 'nothing CI would find here first'

# ---------------------------------------------------------------- the bound is spent, not passed
# Six one-second gates CI recorded at 1 s, never measured here, against a bound of 6 s less a
# reserve of 2: priced at twice CI's seconds, at most two fit. The step's own time stays within
# its bound, every gate it does not run is named, and what each one that ran took is recorded,
# so the next push prices them at this machine's seconds.
: > "$MJ_PUSH_LOCAL"; : > "$T/durations.tsv"; : > "$T/cases.tsv"
plans=(); for i in 1 2 3 4 5 6; do printf 's%s\t1\n' "$i" >> "$T/durations.tsv"; plans+=("$(gate "s$i" 'sleep 1' false)"); done
plan_with "${plans[@]}"
MJ_PUSH_BOUND=6 MJ_PUSH_RESERVE=2 expect_exit 0 "$R" --before-push --plan "$T/plan.json"
took="$(sed -nE 's/.* case\(s\) in ([0-9]+)s, against a bound of 6s.*/\1/p' <<<"$LAST_OUT")"
[ -n "$took" ] && [ "$took" -le 6 ] || fail "the step took ${took:-an unreported time} against its bound of 6 s: $LAST_OUT"
ran="$(grep -cE '^  ok     s[0-9]' <<<"$LAST_OUT" || true)"
[ "$ran" -ge 1 ] && [ "$ran" -le 2 ] || fail "priced at 2 s each, $ran of six gates ran within 4 s: $LAST_OUT"
expect_grep 'left out s6: about 2s here \(never measured here: 1s in CI x 2.00\), past what is left of the 6s bound'
named_and_counted
for g in $(sed -nE 's/^  ok     (s[0-9]) .*/\1/p' <<<"$LAST_OUT"); do
  grep -qE "^gate	$g	[0-9]+$" "$MJ_PUSH_LOCAL" || fail "what $g took here was not recorded: $(cat "$MJ_PUSH_LOCAL")"
done

# ---------------------------------------------------------------- a hook's repository stays its own
# Git exports GIT_DIR to a pre-push hook run from a linked worktree. `outer` stands for the
# repository being pushed: a gate dispatched under that environment must not see it, and a
# case that builds a repository of its own must not move a ref, a branch or a tag of it.
git init -q -b trunk outer
git -C outer -c user.email=o@example.com -c user.name=o commit -q --allow-empty -m outer
outer_before="$(git -C outer for-each-ref --format='%(refname) %(objectname)'; git -C outer symbolic-ref HEAD)"
printf 'no-hook-env\t1\n' >> "$T/durations.tsv"
plan_with "$(gate no-hook-env '! env | grep -q ^GIT_DIR= && ! env | grep -q ^GIT_WORK_TREE= && ! env | grep -q ^GIT_INDEX_FILE=' false)"
GIT_DIR="$PWD/outer/.git" GIT_WORK_TREE="$PWD/outer" GIT_INDEX_FILE="$PWD/outer/.git/index" \
  expect_exit 0 "$R" --before-push --plan "$T/plan.json"
expect_grep '  ok     no-hook-env'
mkdir -p hooked/test/cases hooked/.ai/repo/ci
cp "$ROOT/test/run.sh" hooked/test/
cat > hooked/test/cases/zz_builds_a_repository.sh <<'CASE'
git init -q fixture && cd fixture
git -c user.email=t@example.com -c user.name=t commit -q --allow-empty -m fixture
git branch -M renamed
git tag probe
git config user.name fixture
CASE
: > hooked/.ai/repo/ci/suite-durations.tsv
GIT_DIR="$PWD/outer/.git" bash hooked/test/run.sh zz_builds_a_repository > "$T/hooked.out" 2>&1 \
  || fail "the runner failed under a hook's environment: $(tail -3 "$T/hooked.out")"
grep -q '^ok   zz_builds_a_repository' "$T/hooked.out" || fail "the fixture case did not run: $(tail -3 "$T/hooked.out")"
outer_after="$(git -C outer for-each-ref --format='%(refname) %(objectname)'; git -C outer symbolic-ref HEAD)"
[ "$outer_before" = "$outer_after" ] \
  || fail "a fixture run under a hook's GIT_DIR moved the outer repository: before [$outer_before] after [$outer_after]"
[ -z "$(git -C outer config --local --get user.name || true)" ] \
  || fail 'a fixture run under a hook'"'"'s GIT_DIR wrote an identity into the outer repository'

# ---------------------------------------------------------------- fail closed
MJ_PUSH_DURATIONS="$T/no-such-file" expect_exit 12 "$R" --before-push --plan "$T/plan.json"
expect_grep 'is missing, so nothing can be budgeted and the push is refused'
unset MJ_PUSH_DURATIONS MJ_PUSH_CASES MJ_PUSH_BOUND MJ_PUSH_RESERVE
expect_exit 12 "$R" --before-push --base no-such-ref-anywhere
expect_grep 'the plan could not be computed .* so the push is refused'

# ---------------------------------------------------------------- the model and the wiring
grep -qx 'scripts/ci/run-plan --before-push || exit \$?' "$ROOT/.githooks/pre-push" \
  || fail 'the pre-push hook does not call run-plan --before-push'
grep -q 'name: structure-gates-on-push' "$ROOT/.ai/repo/policy.yaml" \
  || fail 'the policy does not declare the pre-push enforcement, so doctor cannot hold it'
jq -e '.gates[] | select(.id == "github-projection") | .network == true' \
  <(cd "$ROOT" && scripts/ci-plan < /dev/null 2>/dev/null) >/dev/null \
  || fail 'the plan does not carry the network flag the model declares'

# ---------------------------------------------------------------- a gate that builds is CI's
# Every gate whose command calls cargo declares `builds: true`, the plan carries it, and the
# model refuses one that does not say so: a new cargo gate cannot reach a push undeclared.
calls_cargo='(^|[;&|(]|[[:space:]])cargo[[:space:]]'
(cd "$ROOT" && scripts/ci-plan --full < /dev/null 2>/dev/null) > "$T/full.json" || fail 'the full plan could not be computed'
undeclared="$(jq -r --arg re "$calls_cargo" '.gates[] | select((.runs | test($re)) and (.builds != true)) | .id' "$T/full.json")"
[ -z "$undeclared" ] || fail "gates that call cargo without builds: true in the plan: $undeclared"
built="$(jq -r --arg re "$calls_cargo" '[.gates[] | select(.job == "structure" and .selected and (.runs | test($re))) | .id] | join(" ")' "$T/full.json")"
for g in skills-proof economics-claims; do
  case " $built " in *" $g "*) ;; *) fail "the full plan no longer selects $g as a structure gate that builds, so this case proves nothing of it" ;; esac
done
# the real model through the step: every structure gate that builds is left out by name and
# nothing runs (no gate is under a per-gate limit of -1 s, and no case is listed), so this
# costs a plan and not a build
printf '' > "$T/no-cases.tsv"
MJ_PUSH_CASES="$T/no-cases.tsv" MJ_PUSH_GATE_SECONDS=-1 expect_exit 0 "$R" --before-push --full
for g in $built; do expect_grep "left out $g: it builds the crate"; done
expect_grep 'run-plan --before-push: 0 gate\(s\) and 0 case\(s\)'
named_and_counted
# the model refuses a cargo gate that does not declare it, and a builds that is not true
awk '/^  - id: skills-proof$/{s=1} s && /^    builds: true$/{s=0; next} {print}' "$ROOT/.ai/repo/ci/gates.yaml" > undeclared.yaml
expect_exit 10 "$ROOT/scripts/ci-plan" --model undeclared.yaml --check
expect_grep 'gate skills-proof calls cargo and does not declare builds: true'
awk '/^  - id: skills-proof$/{s=1} s && /^    builds: true$/{print "    builds: false"; s=0; next} {print}' "$ROOT/.ai/repo/ci/gates.yaml" > false.yaml
expect_exit 10 "$ROOT/scripts/ci-plan" --model false.yaml --check
expect_grep 'gate skills-proof: builds is true or absent, not false'

# ---------------------------------------------------------------- a case that reads stdin
# The runner, copied beside a case of its own that reads stdin, under a pipe that stays open
# for a minute: the case must see end-of-input at once and the runner report it, rather than
# wait as case 54 did. The pipe's writer outlives the runner by design, so completion is read
# from the runner's verdict line and not from the pipeline's exit.
mkdir -p runner/test/cases runner/.ai/repo/ci
cp "$ROOT/test/run.sh" runner/test/
printf 'cat > /dev/null\n' > runner/test/cases/zz_reads_stdin.sh
: > runner/.ai/repo/ci/suite-durations.tsv
( sleep 60 | bash runner/test/run.sh zz_reads_stdin > "$T/stdin.out" 2>&1 ) &
pipe=$!
for _ in $(seq 1 150); do grep -q 'zz_reads_stdin' "$T/stdin.out" 2>/dev/null && break; sleep 0.2; done
pkill -P "$pipe" 2>/dev/null || true; kill "$pipe" 2>/dev/null || true
grep -q '^ok   zz_reads_stdin' "$T/stdin.out" \
  || fail "a case that reads stdin did not finish under an open pipe: $(tail -3 "$T/stdin.out")"
