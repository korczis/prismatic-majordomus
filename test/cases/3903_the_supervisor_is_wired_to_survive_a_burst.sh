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
grep -q 'group: supervise-lundra' "$CODE" && grep -q 'group: supervise-hosted' "$CODE" \
  || { echo "    the lanes share one concurrency group: a lane without a runner holds the other"; exit 1; }
echo "    one apply per lane, never cancelled by the next request"

# ---------------------------------------------------------------- 3. it cannot be starved
LUNDRA="$T/supervise-lundra.yml"
HOSTED="$T/supervise-hosted.yml"
sed -n '/^  lundra:$/,/^  hosted:$/p' "$CODE" | sed '$d' > "$LUNDRA"
sed -n '/^  hosted:$/,$p' "$CODE" > "$HOSTED"
[ -s "$LUNDRA" ] && [ -s "$HOSTED" ] || { echo "    the supervisor is not a lundra lane and a hosted lane"; exit 1; }
grep -qF "'[\"self-hosted\",\"lundra\"]'" "$LUNDRA" || { echo "    the supervisor does not run on lundra"; exit 1; }
grep -q 'runs-on: ubuntu-latest' "$HOSTED" || { echo "    the supervisor has no GitHub-hosted fallback"; exit 1; }
# the stand-aside sees the lundra lane of its own trigger: a matrix lane saw only the previous
# trigger's, nine minutes old or more on a quiet schedule, and both lanes applied on one tick
grep -q '^    needs: lundra$' "$HOSTED" \
  || { echo "    the hosted lane does not follow the lundra lane of its own run"; exit 1; }
grep -qF "if: \${{ always() && needs.lundra.outputs.supervised != 'true' }}" "$HOSTED" \
  || { echo "    the hosted lane runs whatever lundra did, or not at all when lundra failed"; exit 1; }
grep -qF 'supervised: ${{ steps.supervise.outputs.supervised }}' "$LUNDRA" \
  || { echo "    the lundra lane does not say whether it supervised"; exit 1; }
awk '/\[ "\$rc" = 0 \] \|\|/ { seen = 1 } /echo "supervised=true"/ { exit !seen } END { if (!seen) exit 1 }' "$LUNDRA" \
  || { echo "    the lundra lane claims to have supervised before its exit is checked"; exit 1; }
grep -q 'matrix' "$CODE" && { echo "    a matrix makes the lanes one trigger's twins, blind to each other"; exit 1; }
grep -q 'cargo build' "$CODE" && { echo "    the supervisor compiles itself instead of downloading its executable"; exit 1; }
grep -q 'name majordomus-exe' "$CODE" || { echo "    the supervisor does not download validate's executable"; exit 1; }
# every step bounded on its own, and the bounds add up under the job's: never killed mid-act
for job in "$LUNDRA" "$HOSTED"; do
  awk '
    /^    timeout-minutes:/ { job = $2 }
    /^        timeout-minutes:/ { sum += $2; n++ }
    END { exit !(job > 0 && n == 3 && sum < job) }
  ' "$job" || { echo "    a supervise step has no bound of its own, or the steps add up past the job's"; exit 1; }
  step="$(awk '/- id: supervise/ { s = 1 } s && /timeout-minutes:/ { print $2; exit }' "$job")"
  [ -n "$step" ] && [ "$((step * 60))" -gt 200 ] \
    || { echo "    the supervise step ends before the executable's 180 s forge budget and its last call"; exit 1; }
  [ "$(grep -c 'prs supervise --help >/dev/null' "$job")" -ge 2 ] \
    || { echo "    an executable is taken without asking whether it has prs supervise (a build older than I3903 runs --version)"; exit 1; }
done
grep -q 'timeout 45 gh run download' "$CODE" && grep -q 'timeout 40 sh' "$CODE" \
  || { echo "    a download of the executable or of the release is not bounded"; exit 1; }
grep -q -- '--unless-supervised-within "$COVER"' "$CODE" && grep -q "COVER: '8'" "$HOSTED" \
  || { echo "    the hosted fallback runs a second full apply on every request"; exit 1; }
grep -q -- '--supervised-by lundra' "$CODE" \
  || { echo "    the hosted fallback counts its own stand-aside as a supervision, and stands aside for ever"; exit 1; }
# validate's first wave may take lundra's runners (vars.MJ_RUNNER, #2596); its shards stay
# hosted. A lundra job that waits is replaced in its group and the replaced run's hosted lane
# applies, so no validate job may name lundra outright, past the variable that can be unset
VALIDATE="$ROOT/.github/workflows/validate.yml"
grep -vE '^[[:space:]]*#' "$VALIDATE" | grep -E '^ *runs-on:' | grep -qi 'lundra' \
  && { echo "    validate names lundra outright: the supervisor's lane would wait behind it with no way back"; exit 1; }
echo "    lundra, then a hosted lane of the same run when lundra did not supervise, from a downloaded executable that has the command, each step bounded under its job"

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
grep -q -- '--unless-supervised-within 30 --supervised-by lundra' "$ALIVE" \
  || { echo "    the liveness check counts any lane's success: a lundra lane that always fails is hidden"; exit 1; }
grep -q 'cargo build' "$ALIVE" && { echo "    the liveness check compiles before it reports"; exit 1; }
grep -q 'prs supervise --help >/dev/null' "$ALIVE" \
  || { echo "    the liveness check takes an executable without the command"; exit 1; }
SUP="$ROOT/apps/majordomus-cli/src/integration/supervise.rs"
grep -q 'pub const STALE_MINUTES: u64 = 30;' "$SUP" && grep -q 'pub const SUPERVISE_STALE: &str = "supervise_stale";' "$SUP" \
  || { echo "    the decision no longer names supervise_stale at thirty minutes"; exit 1; }
echo "    a lundra lane with no success for thirty minutes fails the queue workflow"
