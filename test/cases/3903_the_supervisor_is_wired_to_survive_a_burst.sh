# majordomus-covers: none
# The CI supervisor cannot be starved, cancelled by a burst, or stop unnoticed (I3903).
#
# On 2026-10-28 two trains' runs waited hours behind their own superseded runs and same-commit
# twins. The supervisor written for it (#2645) would not have helped: it compiled itself for up
# to twenty minutes on the hosted pool that validate's jobs filled, and each requested validate
# run cancelled the apply before it (`cancel-in-progress: true`), so a burst of requests never
# let one act. Nothing said when it had stopped.
#
# This case holds the wiring the decision depends on, read from the files themselves; the
# decision is apps/majordomus-cli/src/integration/supervise.rs and its replays of that day's
# runs are supervise_replay.rs, which also simulates five requests within a minute against
# this workflow's concurrency.
. "$ROOT/test/lib.sh"

WF="$ROOT/.github/workflows/supervise.yml"
QUEUE="$ROOT/.github/workflows/queue.yml"
MODEL="$ROOT/.ai/repo/ci/supervise.yaml"
[ -f "$WF" ] || { echo "    no supervise workflow: nothing retires a stuck or duplicate run"; exit 1; }
[ -f "$MODEL" ] || { echo "    the supervise workflow has no model under .ai/repo/ci/"; exit 1; }

CODE="$T/supervise-code.yml"
grep -vE '^[[:space:]]*#' "$WF" > "$CODE"

# ---------------------------------------------------------------- 1. started by what it supervises
grep -q 'workflows: \[validate\]' "$CODE" && grep -q 'types: \[requested\]' "$CODE" \
  || { echo "    a requested validate run does not start the supervisor"; exit 1; }
grep -qE "^    - cron: '" "$CODE" || { echo "    the supervisor has no schedule to catch what an event missed"; exit 1; }
echo "    every requested validate run starts it, and the clock is its watchdog"

# ---------------------------------------------------------------- 2. a burst does not cancel it
grep -q 'cancel-in-progress: true' "$CODE" \
  && { echo "    cancel-in-progress: true lets the next request cancel the apply that runs"; exit 1; }
grep -q 'cancel-in-progress: false' "$CODE" || { echo "    the supervisor declares no concurrency"; exit 1; }
grep -qF 'group: supervise-${{ matrix.lane }}' "$CODE" \
  || { echo "    the lanes share one concurrency group: a lane without a runner holds the other"; exit 1; }
echo "    one apply per lane, never cancelled by the next request"

# ---------------------------------------------------------------- 3. it cannot be starved
grep -qF '[self-hosted, lundra]' "$CODE" || { echo "    the supervisor does not run on lundra"; exit 1; }
grep -q 'runner: ubuntu-latest' "$CODE" || { echo "    the supervisor has no GitHub-hosted fallback"; exit 1; }
grep -q 'continue-on-error: true' "$CODE" && grep -q 'fail-fast: false' "$CODE" \
  || { echo "    one lane's failure ends the other"; exit 1; }
grep -q 'cargo build' "$CODE" && { echo "    the supervisor compiles itself instead of downloading its executable"; exit 1; }
grep -q 'name majordomus-exe' "$CODE" || { echo "    the supervisor does not download validate's executable"; exit 1; }
bad="$(sed -n 's/^ *timeout-minutes: *//p' "$CODE" | awk '$1 > 5')"
[ -z "$bad" ] || { echo "    a supervise job is bounded above five minutes: $bad"; exit 1; }
sed -n 's/^ *timeout-minutes: *//p' "$CODE" | grep -q . || { echo "    the supervise job has no bound"; exit 1; }
[ "$(grep -c 'prs supervise --help >/dev/null' "$CODE")" -ge 2 ] \
  || { echo "    an executable is taken without asking whether it has prs supervise (a build older than I3903 runs --version)"; exit 1; }
grep -q -- '--unless-supervised-within "$COVER"' "$CODE" && grep -q "cover: '8'" "$CODE" \
  || { echo "    the hosted fallback runs a second full apply on every request"; exit 1; }
grep -q -- '--supervised-by lundra' "$CODE" \
  || { echo "    the hosted fallback counts its own stand-aside as a supervision, and stands aside for ever"; exit 1; }
echo "    two lanes, lundra and a hosted fallback that stands aside, from a downloaded executable that has the command, bounded at five minutes"

# ---------------------------------------------------------------- 4. its stopping is reported
QCODE="$T/queue-code.yml"
grep -vE '^[[:space:]]*#' "$QUEUE" > "$QCODE"
grep -q '"$mj" prs supervise --format json' "$QCODE" \
  || { echo "    the queue workflow does not read the supervisor's liveness"; exit 1; }
grep -q '.liveness.finding' "$QCODE" || { echo "    the queue workflow does not fail on supervise_stale"; exit 1; }
grep -q 'actions: read' "$QCODE" || { echo "    the queue workflow cannot read the supervisor's runs"; exit 1; }
ALIVE="$T/queue-alive.yml"
sed -n '/^  alive:/,$p' "$QCODE" > "$ALIVE"
[ -s "$ALIVE" ] || { echo "    the supervisor's liveness is not a job of its own"; exit 1; }
grep -qF '[self-hosted, lundra]' "$ALIVE" \
  || { echo "    the supervisor's liveness is read only on the hosted pool the supervisor waits on"; exit 1; }
grep -q 'cargo build' "$ALIVE" && { echo "    the liveness check compiles before it reports"; exit 1; }
grep -q 'prs supervise --help >/dev/null' "$ALIVE" \
  || { echo "    the liveness check takes an executable without the command"; exit 1; }
SUP="$ROOT/apps/majordomus-cli/src/integration/supervise.rs"
grep -q 'pub const STALE_MINUTES: u64 = 30;' "$SUP" && grep -q 'pub const SUPERVISE_STALE: &str = "supervise_stale";' "$SUP" \
  || { echo "    the decision no longer names supervise_stale at thirty minutes"; exit 1; }
echo "    a supervisor with no success for thirty minutes fails the queue workflow"
