# majordomus-covers: none
# The queue's state is measured on a clock; the merge stays a decision.
#
# `scripts/land` answers what GitHub cannot: which open pull requests merge cleanly here, on a
# machine that has the derived merge driver. GitHub cannot run a per-clone driver, so it calls
# every pull request CONFLICTING and its answer is worthless — which is how sixteen, fifteen of
# them cleanly mergeable, once sat open at once. The command existed and nothing ran it, so the
# answer existed only when somebody thought to ask: on 2026-09-15 five sessions spent a night
# relaying it to each other by hand.
#
# So it runs on a schedule. What it must never do is merge. `scripts/land --run` merges onto an
# integration branch and its own header says it stops before pushing, always — "landing is
# reversible until then, and deploying is a decision, not a side effect". An automation that
# took that power would delete a deliberate decision, silently, every thirty minutes.
#
# This case holds both halves: the clock is declared in one place and agreed in the other, and
# the job can neither ask for the merge nor perform one — it runs the command without `--run`,
# and its token cannot write.
. "$ROOT/test/lib.sh"

WF="$ROOT/.github/workflows/queue.yml"
MODEL="$ROOT/.ai/repo/ci/queue.yaml"
[ -f "$WF" ] || { echo "    no queue workflow: the queue's state would be measured only when somebody asks"; exit 1; }
[ -f "$MODEL" ] || { echo "    the queue workflow has no model under .ai/repo/ci/"; exit 1; }

# ---------------------------------------------------------------- 1. one clock, two declarations
cron="$(sed -n "s/^    - cron: '\(.*\)'$/\1/p" "$WF" | head -n 1)"
[ -n "$cron" ] || { echo "    the queue workflow has no schedule: it would run only when dispatched"; exit 1; }
declared="$(sed -n "s/^  schedule: '\(.*\)'$/\1/p" "$MODEL" | head -n 1)"
[ -n "$declared" ] || { echo "    the model does not declare the schedule the workflow runs on"; exit 1; }
[ "$cron" = "$declared" ] || { echo "    the model says '$declared' and the workflow says '$cron'; one clock, two answers"; exit 1; }
echo "    the clock is declared once and agreed in both places"

# ---------------------------------------------------------------- 2. it measures
grep -q 'scripts/land' "$WF" || { echo "    the workflow does not run scripts/land; it measures nothing"; exit 1; }
echo "    the job runs the command that answers the question"

# ---------------------------------------------------------------- 3. and cannot merge
grep -q -- '--run' "$WF" \
  && { echo "    the workflow passes --run: an automation must not take the power the script withholds"; exit 1; }
grep -qE '^\s+(contents|pull-requests|packages|actions|id-token|deployments|issues|statuses):\s*write' "$WF" \
  && { echo "    the job is granted a write permission; a measurement needs none, and the grant is the capability"; exit 1; }
grep -q 'contents: read' "$WF" || { echo "    the workflow does not declare its permissions; the default token may write"; exit 1; }
echo "    it cannot merge: no --run, and no write permission to do it with"

# ---------------------------------------------------------------- 4. the two exits are not confused
# 10 is a fact about the queue (something was left behind); 12 is the measurement failing. A
# check that conflates them teaches people to ignore it.
grep -qE '\[ "\$rc" != 12 \]' "$WF" \
  || { echo "    the job does not fail on an unusable measurement; a report nobody can trust is worse than none"; exit 1; }
grep -qE 'exit 10|rc.*=.*10.*exit' "$WF" \
  && { echo "    the job fails on exit 10; a pull request left behind is the queue's state, not this run's fault"; exit 1; }
grep -q 'fails_on: unusable' "$MODEL" || { echo "    the model does not say which exit fails the job"; exit 1; }
echo "    an unusable measurement fails; a queue with something left behind does not"
