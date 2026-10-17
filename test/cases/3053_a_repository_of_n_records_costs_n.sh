# majordomus-covers: none
# A repository of N records costs N parses, not N squared (I0836, milestone M004).
#
# Quadratic behaviour hides in small repositories: a validator that re-reads every record
# for every record costs nothing at three issues and minutes at three hundred, and wall clock
# notices only when it is already a complaint. The work counters MJ_TIMING turns on
# (yaml_flatten, yaml_flatten_many: one per YAML parse) notice it first and on any machine,
# because they count work rather than time — this case asserts no wall clock at all.
#
# synthetic_repo (test/lib.sh) grows one repository to N, 2N and 4N of each record the
# validators read — milestones, issues, project rules and context documents — and `plan
# validate` and `doctor` are measured at each size. Linear growth adds about as much from 2N
# to 4N as twice the step from N to 2N; quadratic growth adds four times as much. The bound
# below sits between the two, so a validator that turns quadratic fails here.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A && git commit -qm base
# the two hooks doctor requires, as case 03 wires them, so its verdict is about the records
printf '#!/bin/sh\n%s doctor || exit $?\n' "$MJ" > .git/hooks/pre-commit
printf '#!/bin/sh\n%s finish --check || exit $?\n' "$MJ" > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push

# parses <command...> — the YAML parses one run of the command made, from its timing report.
# It runs inside a command substitution, so what it says on failure goes to stderr: on stdout
# it would become the number the caller reads, and the case would stop without a word.
parses() {
  MJ_TIMING=1 "$@" > "$T/out.txt" 2> "$T/timing.txt" \
    || { { echo "    $* failed on the synthetic repository:"; grep -vE '^(OK|INFO) ' "$T/out.txt" | sed 's/^/    | /' | head -20; } >&2; exit 1; }
  awk '$1 == "count" && ($3 == "yaml_flatten" || $3 == "yaml_flatten_many") { y += $2 }
       $1 == "count" && $3 == "git" { g += $2 } END { print y + 0, g + 0 }' "$T/timing.txt"
}

N=8
for size in $N $((2 * N)) $((4 * N)); do
  synthetic_repo "$size"
  [ "$(find .ai/repo/project/issues -name 'I*.yaml' | wc -l | tr -d ' ')" = "$size" ] \
    && [ "$(find .ai/repo/rules/project -name 'synthetic-*.md' | wc -l | tr -d ' ')" = "$size" ] \
    && [ "$(find src -name README.md | wc -l | tr -d ' ')" = "$size" ] \
    || { echo "    synthetic_repo $size did not leave $size issues, rules and context documents"; exit 1; }
  eval "plan_$size=\"\$(parses \"\$MJ\" plan validate)\""
  eval "doctor_$size=\"\$(parses \"\$MJ\" doctor)\""
done

# at_most_linear <what> <at N> <at 2N> <at 4N>: the step from 2N to 4N is at most three times
# the step from N to 2N, plus a constant for the work every run does whatever the size.
# Linear growth makes it twice, quadratic four times; a batch read that does not grow at all
# (plan validate parses every record in one call) passes, as it should.
at_most_linear() {
  local what="$1" a="$2" b="$3" c="$4"
  echo "    $what: $a at N=$N, $b at $((2 * N)), $c at $((4 * N))"
  [ $((c - b)) -le $((3 * (b - a) + 8)) ] \
    || { echo "    $what grows faster than linearly: +$((b - a)) from N to 2N, +$((c - b)) from 2N to 4N"; exit 1; }
}
for cmd in plan doctor; do
  eval "set -- \$${cmd}_$N \$${cmd}_$((2 * N)) \$${cmd}_$((4 * N))"
  at_most_linear "$cmd: YAML parses" "$1" "$3" "$5"
  at_most_linear "$cmd: git calls" "$2" "$4" "$6"
done
# the counters are measuring something: doctor reads every rule and context document
eval "set -- \$doctor_$N \$doctor_$((4 * N))"
[ "$3" -gt "$1" ] || { echo "    doctor made no more YAML parses at $((4 * N)) records than at $N; the counter measures nothing here"; exit 1; }
echo "    ok: plan validate and doctor grow at most linearly in the size of the repository"
