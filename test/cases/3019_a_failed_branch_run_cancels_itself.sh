# majordomus-covers: none
# A validate run on a branch ends at its first red, and the trunk's run never does (I2664).
#
# On 2026-10-13 a validate run with two failed jobs kept nine more running for twenty minutes,
# holding the runners a deployment and a release waited for, and the failed job's log could
# not be read until the run ended. The workflow now cancels a pull request's run from the
# last step of every gate job that failed. What must stay true, each a line somebody could
# change without meaning to change this:
#
#   1  every job but the two that read the others' results (ci, evidence) ends with the
#      self-cancel step, and it is the job's LAST step, so the job's uploads and summary ran;
#   2  the step fires only on `failure()` of a pull_request event: never on a push to master,
#      the nightly schedule or a dispatch (the release pipeline's record verdict) — the
#      trunk's remaining jobs are the only reading of its other lanes;
#   3  it cancels this run by its own id with the job's token, and the token may: the
#      workflow grants actions: write, and a job that narrows its permissions keeps it;
#   4  nothing else cancels a run: concurrency still waits rather than cancels (PR #576 was
#      cancelled four times in a row on 2026-09-15 by a newer commit; case 2999 holds the
#      rest of that);
#   5  a combined top runs to the end: on a pull request whose head is an `integration/*`
#      branch every self-cancel step only says so and cancels nothing, so one run of a top
#      shows every red (on 2026-10-15 the grand top took three full cycles to show three).
#
# Read from the tracked workflow with awk; nothing is built, nothing reaches the network.
# Then each refusal is proved by mutating a copy.
. "$ROOT/test/lib.sh"
W="$ROOT/.github/workflows/validate.yml"
[ -f "$W" ] || { echo "    .github/workflows/validate.yml is missing"; exit 1; }
COND="failure() && github.event_name == 'pull_request'"
EXEMPT=" ci evidence "

# "<job>\t<step number>\t<key>\t<value>" for every step key this case reads, "<job>\tsteps\t<n>"
# for every job, and "<job>\tperm\t<permission line>" for a job's own permissions block;
# top-level permissions are reported under the job name "-".
steps_of() { # <workflow>
  awk '
    function flush() { if (job != "") printf "%s\tsteps\t%d\n", job, n }
    function key(line,   k, v) {
      if (match(line, /^[A-Za-z_-]+:/) == 0) return
      k = substr(line, 1, RLENGTH - 1); v = substr(line, RLENGTH + 1); sub(/^[ \t]+/, "", v)
      if (k == "if" || k == "run" || k == "name") printf "%s\t%d\t%s\t%s\n", job, n, k, v
    }
    /^permissions:/ { top = 1; next }
    top && /^  [a-z-]+:/ { l = $0; sub(/^  /, "", l); printf "-\tperm\t%s\n", l; next }
    top && /^[^ #]/ { top = 0 }
    /^jobs:/ { injobs = 1; next }
    !injobs { next }
    /^  [a-z0-9_-]+:[ \t]*$/ { flush(); job = $0; gsub(/[ :\t]/, "", job); n = 0; perm = 0; next }
    /^    permissions:/ { perm = 1; next }
    perm && /^      [a-z-]+:/ { l = $0; sub(/^ +/, "", l); printf "%s\tperm\t%s\n", job, l; next }
    /^    [a-z]/ { perm = 0 }
    /^      - / { n++; l = $0; sub(/^      - /, "", l); key(l); next }
    n > 0 && /^        [A-Za-z_-]+:/ { l = $0; sub(/^        /, "", l); key(l); next }
    n > 0 && /^          GH_TOKEN:/ { l = $0; sub(/^ +GH_TOKEN:[ \t]*/, "", l); printf "%s\t%d\ttoken\t%s\n", job, n, l }
    END { flush() }' "$1"
}

judge() { # <workflow> -> the findings, one per line; nothing when it holds
  local s; s="$(steps_of "$1")"
  local jobs; jobs="$(printf '%s\n' "$s" | awk -F'\t' '$2 == "steps" { print $1 }')"
  [ -n "$jobs" ] || { echo "no job was read from the workflow"; return; }
  local top_actions; top_actions="$(printf '%s\n' "$s" | awk -F'\t' '$1 == "-" && $2 == "perm" && $3 ~ /^actions:[ \t]*write/ { print "yes" }')"
  for j in $jobs; do
    case "$EXEMPT" in *" $j "*) continue ;; esac
    last="$(printf '%s\n' "$s" | awk -F'\t' -v j="$j" '$1 == j && $2 == "steps" { print $3 }')"
    cancel="$(printf '%s\n' "$s" | awk -F'\t' -v j="$j" '$1 == j && $3 == "run" && $4 ~ /gh run cancel/ { print $2 }' | tail -n 1)"
    if [ -z "$cancel" ]; then echo "job $j has no step that cancels its run when it fails"; continue; fi
    [ "$cancel" = "$last" ] || echo "job $j cancels at step $cancel of $last: the steps after it never run"
    cond="$(printf '%s\n' "$s" | awk -F'\t' -v j="$j" -v n="$cancel" '$1 == j && $2 == n && $3 == "if" { print $4 }')"
    [ "$cond" = "$COND" ] || echo "job $j cancels on '${cond:-every outcome}', not on '$COND' alone: it could end a run of master, the schedule or a dispatch, or a green one"
    run="$(printf '%s\n' "$s" | awk -F'\t' -v j="$j" -v n="$cancel" '$1 == j && $2 == n && $3 == "run" { print $4 }')"
    printf '%s\n' "$run" | grep -qE 'gh run cancel "?\$(GITHUB_RUN_ID|\{\{ github\.run_id \}\})' \
      || echo "job $j cancels something other than its own run: $run"
    # 5: an integration/* top is exempt, and its branch of the step cancels nothing
    if ! printf '%s\n' "$run" | grep -qE '^case "\$GITHUB_HEAD_REF" in integration/\*\) [^;]*;; \*\) gh run cancel ' \
      || printf '%s\n' "$run" | sed 's/;; \*).*//' | grep -q 'gh run cancel'; then
      echo "job $j would cancel the run of an integration/* top: $run"
    fi
    tok="$(printf '%s\n' "$s" | awk -F'\t' -v j="$j" -v n="$cancel" '$1 == j && $2 == n && $3 == "token" { print $4 }')"
    [ "$tok" = '${{ github.token }}' ] || [ "$tok" = '${{ secrets.GITHUB_TOKEN }}' ] \
      || echo "job $j cancels without the job's token (GH_TOKEN is '${tok:-unset}')"
    own="$(printf '%s\n' "$s" | awk -F'\t' -v j="$j" '$1 == j && $2 == "perm" { print $3 }')"
    if [ -n "$own" ]; then
      printf '%s\n' "$own" | grep -qE '^actions:[[:space:]]*write' \
        || echo "job $j narrows its permissions without actions: write, so its token cannot cancel the run"
    elif [ -z "$top_actions" ]; then
      echo "job $j inherits the workflow's permissions, which do not grant actions: write"
    fi
  done
  # 2, from the other side: no step anywhere cancels a run on any other condition
  printf '%s\n' "$s" | awk -F'\t' '$3 == "run" && $4 ~ /gh run cancel/ { print $1 "\t" $2 }' | while IFS="$(printf '\t')" read -r j n; do
    cond="$(printf '%s\n' "$s" | awk -F'\t' -v j="$j" -v n="$n" '$1 == j && $2 == n && $3 == "if" { print $4 }')"
    [ "$cond" = "$COND" ] || echo "job $j step $n cancels the run on '${cond:-every outcome}'"
  done
  # 4: concurrency waits; a newer commit never cancels a run in flight
  awk '/^concurrency:/ {on=1; next} on && /^[^[:space:]#]/ {exit} on {print}' "$1" \
    | grep -qE '^[[:space:]]+cancel-in-progress:[[:space:]]+false[[:space:]]*$' \
    || echo "concurrency may cancel a run in flight: cancel-in-progress is not false"
}

# ---------------------------------------------------------------- the workflow holds
out="$(judge "$W")"
[ -z "$out" ] || { printf '    %s\n' "validate.yml does not end a failed branch run, or could end another:"; printf '      %s\n' "$out"; exit 1; }
n="$(steps_of "$W" | awk -F'\t' '$3 == "run" && $4 ~ /gh run cancel/' | wc -l | tr -d ' ')"
[ "$n" -ge 10 ] || { echo "    only $n job(s) carry the self-cancel; the reader has lost the jobs"; exit 1; }

# ---------------------------------------------------------------- each refusal, by mutation
# A mutant is the workflow with one change, made by awk so that "the first occurrence" means
# the same on BSD and GNU (sed's 0,/re/ address is GNU's alone).
mutant() { # <name> <awk program> <expected finding (ERE)>
  awk "$2" "$W" > "$T/$1.yml"
  cmp -s "$W" "$T/$1.yml" && { echo "    mutation $1 changed nothing"; exit 1; }
  out="$(judge "$T/$1.yml")"
  printf '%s\n' "$out" | grep -qE -- "$3" || { echo "    mutation $1 was not refused as /$3/; findings:"; printf '      %s\n' "$out"; exit 1; }
}
PR="github.event_name == 'pull_request'"
# it would fire on master too: every job's condition loses the event
mutant on-master "{ sub(/if: failure\(\) && $PR\$/, \"if: failure()\") } 1" \
  "cancels on 'failure\\(\\)', not on"
# one job's would fire on a push as well
mutant on-push "!d && sub(/$PR\$/, \"($PR || github.event_name == 'push')\") { d = 1 } 1" \
  "could end a run of master"
# a job without it: its first self-cancel is gone
mutant missing '!d && /run: .*gh run cancel/ { sub(/gh run cancel.*/, "echo no"); d = 1 } 1' \
  "has no step that cancels its run"
# the token cannot cancel: the workflow no longer grants actions: write
mutant no-permission '!/^  actions: write$/' \
  "inherits the workflow's permissions, which do not grant actions: write"
# a newer commit cancels the run in flight again
mutant newer-commit '{ sub(/^  cancel-in-progress: false$/, "  cancel-in-progress: true") } 1' \
  "concurrency may cancel a run in flight"
# a step added after the cancel never runs once the cancel fires
mutant late-step '{ print } /run: .*gh run cancel/ && !d { print "      - run: echo after"; d = 1 }' \
  'cancels at step [0-9]+ of [0-9]+'
# 5: one job cancels an integration/* top again — its guard is gone
mutant top-unguarded '!d && /run: .*gh run cancel/ { sub(/case "\$GITHUB_HEAD_REF" in integration\/\*\) [^;]*;; \*\) /, ""); sub(/;; esac$/, ""); d = 1 } 1' \
  'would cancel the run of an integration/\* top'
# 5: one job's top branch cancels as well
mutant top-cancels '!d && /run: .*gh run cancel/ { sub(/integration\/\*\) echo/, "integration/*) gh run cancel \"$GITHUB_RUN_ID\"; echo"); d = 1 } 1' \
  'would cancel the run of an integration/\* top'
# 5: one job exempts another prefix than integration/
mutant top-renamed '!d && /run: .*gh run cancel/ { sub(/in integration\/\*\)/, "in integrate/*)"); d = 1 } 1' \
  'would cancel the run of an integration/\* top'

echo "    every gate job of validate.yml ends a failed pull request's run as its last step, on failure() of a pull_request event alone, with a token allowed to, and never on an integration/* top; master, the schedule and a dispatch run to the end, and concurrency still cancels nothing"
