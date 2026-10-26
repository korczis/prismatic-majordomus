# majordomus-covers: none
# The plan job decides the runner of every other Linux job of a validate run (I4242).
#
# The repository's own machines (vars.MJ_RUNNER, e.g. lundra) are the first wave, and a
# pull request from a fork must never reach them: the repository is public and they are
# machines of the fleet. What must stay true, each a line somebody could change without
# meaning to change this:
#
#   1  the plan job runs on ubuntu-latest, so a verdict is planned while the pool is offline;
#   2  every other job names its platform (macos-latest, a matrix) or takes exactly
#      `fromJSON(needs.plan.outputs.runner || '"ubuntu-latest"')`, so a failed plan still
#      lets `ci` report and no job picks a runner of its own;
#   3  the plan's runner step sends a fork's pull request to ubuntu-latest whatever the
#      variable says, a same-repository one and a push to the chosen pool, and an unset or
#      unreadable choice to ubuntu-latest.
#
# Read from the tracked workflow with awk and the step's own script run under bash;
# nothing is built, nothing reaches the network.
. "$ROOT/test/lib.sh"
W="$ROOT/.github/workflows/validate.yml"
[ -f "$W" ] || { echo "    .github/workflows/validate.yml is missing"; exit 1; }
EXPR="\${{ fromJSON(needs.plan.outputs.runner || '\"ubuntu-latest\"') }}"

# "<job>\t<runs-on>" for every job
runs_on() {
  awk '
    /^jobs:/ { injobs = 1; next }
    !injobs { next }
    /^  [a-z0-9_-]+:[ \t]*$/ { job = $0; gsub(/[ :\t]/, "", job); next }
    /^    runs-on:/ { v = $0; sub(/^    runs-on:[ \t]*/, "", v); printf "%s\t%s\n", job, v }' "$1"
}

# 1 and 2
fail=0
while IFS="$(printf '\t')" read -r job value; do
  case "$job:$value" in
    plan:ubuntu-latest) ;;
    plan:*) echo "    the plan job runs on '$value', not ubuntu-latest"; fail=1 ;;
    *:macos-latest|*:'${{ matrix.runner }}') ;;
    *) [ "$value" = "$EXPR" ] || { echo "    job $job runs on '$value', not the planned runner"; fail=1; } ;;
  esac
done <<EOF
$(runs_on "$W")
EOF
[ "$(runs_on "$W" | wc -l)" -gt 10 ] || { echo "    fewer than ten jobs were read from the workflow"; fail=1; }
[ "$fail" = 0 ] || exit 1
echo "    every job but plan takes the planned runner or names its platform"

# 3: the step's own script, out of the workflow
STEP="$(mktemp)"; OUT="$(mktemp)"
trap 'rm -f "$STEP" "$OUT"' EXIT
awk '
  /^      - id: runner[ \t]*$/ { instep = 1; next }
  instep && /^      - / { exit }
  instep && /^        run: \|/ { inrun = 1; next }
  inrun { l = $0; sub(/^          /, "", l); print l }' "$W" > "$STEP"
grep -q 'GITHUB_OUTPUT' "$STEP" || { echo "    the plan job has no runner step"; exit 1; }

planned() { # <event> <head repo> <choice> -> the runner the step writes
  : > "$OUT"
  EVENT="$1" HEAD_REPO="$2" CHOSEN="$3" GITHUB_REPOSITORY=korczis/prismatic-majordomus \
    GITHUB_OUTPUT="$OUT" GITHUB_STEP_SUMMARY=/dev/null bash "$STEP" >/dev/null
  sed -n 's/^runner=//p' "$OUT"
}

expect() { # <want> <event> <head repo> <choice>
  local got; got="$(planned "$2" "$3" "$4")"
  [ "$got" = "$1" ] || { echo "    $2 from '${3:-}' with '${4:-}' plans '$got', not '$1'"; exit 1; }
}

expect '"ubuntu-latest"' pull_request someone/fork lundra
expect '"ubuntu-latest"' pull_request '' lundra
expect '["self-hosted","lundra"]' pull_request korczis/prismatic-majordomus lundra
expect '["self-hosted","lundra"]' push '' lundra
expect '["self-hosted","lundra","X64"]' schedule '' '["self-hosted","lundra","X64"]'
expect '"ubuntu-latest"' push '' ''
expect '"ubuntu-latest"' push '' '[1,2]'
expect '"ubuntu-latest"' workflow_dispatch '' '[not json'
echo "    a fork stays hosted, the repository's own events reach the pool, a bad choice is hosted"
