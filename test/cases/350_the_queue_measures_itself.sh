# majordomus-covers: none
# The queue's state is measured on a clock; the merge stays a decision.
#
# `majordomus prs` answers what GitHub cannot: what each open pull request is to the current
# master, decided by git on a machine that has the derived merge driver. GitHub cannot run a
# per-clone driver, so it calls nearly every pull request CONFLICTING and its answer is
# worthless — which is how sixteen, fifteen of them cleanly mergeable, once sat open at once.
# The command existed and nothing ran it, so the answer existed only when somebody thought to
# ask: on 2026-09-15 five sessions spent a night relaying it to each other by hand.
#
# So it runs on a schedule. What it must never do is merge. `prs drain` integrates one pull
# request at a time under a lease (ADR 0101) and `prs cleanup --apply` closes them; an
# automation that took either power would delete a deliberate decision, silently, every
# thirty minutes.
#
# This case holds both halves: the clock is declared in one place and agreed in the other, and
# the job can neither ask for the merge nor perform one — it runs only the observing commands,
# and its token cannot write.
. "$ROOT/test/lib.sh"

WF="$ROOT/.github/workflows/queue.yml"
MODEL="$ROOT/.ai/repo/ci/queue.yaml"
[ -f "$WF" ] || { echo "    no queue workflow: the queue's state would be measured only when somebody asks"; exit 1; }
[ -f "$MODEL" ] || { echo "    the queue workflow has no model under .ai/repo/ci/"; exit 1; }

# what the job does is its non-comment lines: a comment may name the acting commands to say
# why they are absent
CODE="$T/queue-code.yml"
grep -vE '^[[:space:]]*#' "$WF" > "$CODE"

# ---------------------------------------------------------------- 1. one clock, two declarations
cron="$(sed -n "s/^    - cron: '\(.*\)'$/\1/p" "$WF" | head -n 1)"
[ -n "$cron" ] || { echo "    the queue workflow has no schedule: it would run only when dispatched"; exit 1; }
declared="$(sed -n "s/^  schedule: '\(.*\)'$/\1/p" "$MODEL" | head -n 1)"
[ -n "$declared" ] || { echo "    the model does not declare the schedule the workflow runs on"; exit 1; }
[ "$cron" = "$declared" ] || { echo "    the model says '$declared' and the workflow says '$cron'; one clock, two answers"; exit 1; }
echo "    the clock is declared once and agreed in both places"

# ---------------------------------------------------------------- 2. it measures
grep -qE '"\$mj" prs refresh' "$CODE" || { echo "    the workflow does not observe the forge (prs refresh); it measures nothing"; exit 1; }
grep -qE '"\$mj" prs status' "$CODE" || { echo "    the workflow does not render the queue (prs status); nobody reads the observation"; exit 1; }
# the driver is the reason the measurement exists, and its contract is core-check's
grep -q 'scripts/merge-derived %O %A %B %P"' "$CODE" \
  || { echo "    the workflow does not install the derived merge driver with the contract scripts/ci/core-check uses"; exit 1; }
echo "    the job runs the commands that answer the question, with the driver GitHub cannot run"

# ---------------------------------------------------------------- 3. and cannot merge
grep -qE 'prs (drain|cleanup)|--apply|gh pr merge' "$CODE" \
  && { echo "    the workflow names a command that acts: an automation must not take the power a person holds"; exit 1; }
grep -qE '^\s+(contents|pull-requests|packages|actions|id-token|deployments|issues|statuses):\s*write' "$CODE" \
  && { echo "    the job is granted a write permission; a measurement needs none, and the grant is the capability"; exit 1; }
grep -q 'contents: read' "$CODE" || { echo "    the workflow does not declare its permissions; the default token may write"; exit 1; }
echo "    it cannot merge: no acting command, and no write permission to do it with"

# ---------------------------------------------------------------- 4. the two exits are not confused
# 10 is a fact about the queue (a stale observation, an unread protection); 12 is the
# measurement failing. A check that conflates them teaches people to ignore it.
grep -qE '\[ "\$rc" != 12 \]' "$CODE" \
  || { echo "    the job does not fail on an unusable measurement; a report nobody can trust is worse than none"; exit 1; }
grep -qE 'exit 10|"\$rc" != 10 \]' "$CODE" \
  && { echo "    the job fails on exit 10; a finding is the queue's state, not this run's fault"; exit 1; }
grep -qE '\[ "\$rc" = 0 \] \|\| \[ "\$rc" = 10 \] \|\|' "$CODE" \
  || { echo "    the job does not let exit 10 through; a finding is the queue's state, not this run's fault"; exit 1; }
grep -q 'fails_on: unusable' "$MODEL" || { echo "    the model does not say which exit fails the job"; exit 1; }
echo "    an unusable measurement fails; a queue with a finding does not"

# ---------------------------------------------------------------- 5. the exits it relies on exist
# The job's verdict is only as good as the command's exit contract: if `prs` stopped
# distinguishing 10 from 12, the guard above would hold nothing.
PRS="$ROOT/apps/majordomus-cli/src/commands/prs.rs"
if ! grep -qE 'const FINDING: u8 = 10;' "$PRS" || ! grep -qE 'const UNUSABLE: u8 = 12;' "$PRS"; then
  echo "    majordomus prs no longer declares exit 10 as a finding and 12 as unusable"; exit 1
fi
echo "    majordomus prs still separates a finding (10) from an unusable answer (12)"
