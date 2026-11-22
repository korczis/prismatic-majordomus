# majordomus-covers: capture session
# A headless or nested provider session is not the worker (I3061): its start opens a local
# episode bound to nothing, and its end, interrupted or not, never hands the checkout's task
# over and never writes a tracked record. The worker's own interrupted end keeps today's
# behaviour (case 54). Every event is driven through the shim the provider runs.
#
# Reported by an adopter on 0.30.0, 2026-10-19: a `claude -p` review run inside a supervised
# worktree was killed mid-run, and its SessionEnd wrote a tracked record under
# .ai/repo/sessions/ and marked the worker's task handed_over.
. "$ROOT/test/lib.sh"

# the suite may itself run inside a provider session; this case names its sessions itself
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID MJ_SESSION_KEY

"$MJ" init >/dev/null; "$MJ" update >/dev/null
# the server the start event would ensure is case 108's
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" \
  && cp "$T/pol" .ai/repo/policy.yaml
"$MJ" capture install >/dev/null
mkdir -p lib && echo a > lib/a && git add -A && git commit -qm base
PATH="$(dirname "$MJ"):$PATH"; export PATH

must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
tracked_records() { find .ai/repo/sessions -maxdepth 1 -name '2*.md' 2>/dev/null | wc -l | tr -d ' '; }
local_records()   { find .ai/local/state/sessions -maxdepth 1 -name '2*.md' 2>/dev/null | wc -l | tr -d ' '; }
sessions_clean()  { [ -z "$(git status --porcelain -- .ai/repo/sessions)" ]; }
task_outcome()    { sed -n 's/^outcome: //p' .ai/local/state/current.yaml | head -n 1; }
start_event() { printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"%s"}' "$1" "$2" | ./.claude/hooks/majordomus-session-end; }
OPEN=.ai/local/state/sessions-open

# ---------------------------------------------------------------- the worker and its task
start_event worker >/dev/null 2>&1
must "the worker's start opened no episode" [ -f "$OPEN/worker.yaml" ]
MAJORDOMUS_PROVIDER_SESSION=worker "$MJ" start "the worker's task" --scope lib >/dev/null 2>&1
must "the worker's task did not start" [ "$(task_outcome)" = active ]
head0="$(git rev-parse HEAD)"; tracked0="$(tracked_records)"; local0="$(local_records)"

# ---------------------------------------------------------------- a session spawned inside it
# The hook's environment names the worker's provider session; the payload names another one.
CLAUDE_CODE_SESSION_ID=worker start_event nested-1 >/dev/null 2>"$T/err"
grep -qF 'not the worker' "$T/err" || { echo "    the nested start was not told apart"; sed 's/^/    | /' "$T/err"; exit 1; }
must "the nested session opened no local episode" [ -f "$OPEN/nested-1.yaml" ]
CLAUDE_CODE_SESSION_ID=worker end_event nested-1 other >/dev/null 2>"$T/err"
must "the nested end handed the worker's task over" [ "$(task_outcome)" = active ]
must "the nested end wrote a tracked record" [ "$(tracked_records)" = "$tracked0" ]
must "the nested end left .ai/repo/sessions dirty" sessions_clean
must "the nested end committed something" [ "$(git rev-parse HEAD)" = "$head0" ]
must "the nested end kept no local record" [ "$(local_records)" = "$((local0 + 1))" ]
must "the nested end left its episode open" [ ! -f "$OPEN/nested-1.yaml" ]
grep -qF 'outcome: interrupted' "$(find .ai/local/state/sessions -maxdepth 1 -name '2*.md' | LC_ALL=C sort | tail -n 1)" \
  || { echo "    the local record does not say the guest was interrupted"; exit 1; }
grep -qF 'is a guest of episode' .ai/local/session-contexts/.session-context.log \
  || { echo "    the guest's end said nothing in its local log"; exit 1; }

# ---------------------------------------------------------------- a second session beside it
# No environment this time: the active task was opened by the worker's episode, still open.
start_event review-2 >/dev/null 2>"$T/err"
grep -qF 'not the worker' "$T/err" || { echo "    the second session was taken for the worker"; sed 's/^/    | /' "$T/err"; exit 1; }
end_event review-2 other >/dev/null 2>&1
must "the second session's interrupted end handed the task over" [ "$(task_outcome)" = active ]
must "the second session's end wrote a tracked record" [ "$(tracked_records)" = "$tracked0" ]
must "the second session's end left .ai/repo/sessions dirty" sessions_clean
must "the second session's end committed something" [ "$(git rev-parse HEAD)" = "$head0" ]
must "the second session's end kept no local record" [ "$(local_records)" = "$((local0 + 2))" ]
# the decision does not outlive the session
must "a guest marker outlived its session" [ ! -e .ai/local/state/sessions-guest/review-2 ]

# ---------------------------------------------------------------- the worker's own end
# No other live session: the interrupted end hands the task over and writes the tracked
# record, as case 54 holds.
must "the worker's episode was closed by a guest" [ -f "$OPEN/worker.yaml" ]
end_event worker other >/dev/null 2>"$T/err"
must "the worker's interrupted end did not hand the task over" [ "$(task_outcome)" = handed_over ]
must "the worker's end wrote no tracked record" [ "$(tracked_records)" = "$((tracked0 + 1))" ]
must "the worker's episode is still open" [ ! -f "$OPEN/worker.yaml" ]
grep -qF 'outcome: interrupted' "$("$MJ" session latest --path)" \
  || { echo "    the worker's record does not say it was interrupted"; exit 1; }

# ---------------------------------------------------------------- two windows, no task
# Without a task between them two windows stay two workers (case 111): each writes its record.
start_event win-a >/dev/null 2>&1
start_event win-b >/dev/null 2>"$T/err"
expect_no_grep 'not the worker' "$T/err"
end_event win-b clear >/dev/null 2>&1
must "a sibling window without a task was treated as a guest" [ "$(tracked_records)" = "$((tracked0 + 2))" ]
end_event win-a clear >/dev/null 2>&1
must "the first window wrote no record" [ "$(tracked_records)" = "$((tracked0 + 3))" ]
