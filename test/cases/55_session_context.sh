# majordomus-covers: session
# majordomus-negative: session doctor
# The bounded working context: what the open freezes, what the close appends, and every
# mutation of the store doctor has to go red on.
#
# The store this exercises is the one the layer named from the beginning and never filled.
# Each assertion below is therefore about a producer rather than a format: something writes
# the document, the document survives being written into by a person, and nothing may turn
# it into a transcript.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
PATH="$(dirname "$MJ"):$PATH"; export PATH
store=.ai/local/session-contexts

# ---------------------------------------------------------------- absence is an answer
# no episode is an answer, exactly as it is for status; a session that exists and has no
# working context is the other thing, and that one is a missing artifact
expect_exit 0 "$MJ" session context
expect_grep 'No open session'
expect_exit 12 "$MJ" session context s-19700101000000-0000
expect_grep 'No working context'
expect_exit 2 "$MJ" session context one two
expect_grep 'one session id at a time' 

# ---------------------------------------------------------------- the open freezes it
"$MJ" start "a task" --scope lib/ >/dev/null
expect_exit 0 "$MJ" session start
expect_grep 'working context: .ai/local/session-contexts/'
ctx="$("$MJ" session context)"
expect_file "$ctx"
case "$(basename "$ctx")" in
  2[0-9][0-9][0-9][0-9][0-9][0-9][0-9]T[0-9][0-9][0-9][0-9][0-9][0-9]Z--s-*.md) ;;
  *) echo "    the document is not named <stamp>--<session-id>.md: $(basename "$ctx")"; exit 1 ;;
esac

# the contract, and the episode it belongs to
grep -qF 'schema: session-context/v1' "$ctx"
grep -qF 'kind: session-context' "$ctx"
grep -qF "session_id: $("$MJ" --json session status | sed 's/.*"session_id":"//; s/".*//')" "$ctx"
# opened by a person at a terminal, and it says so: opened_by is hook exactly when the open
# named the provider that delivered the event, which only a provider hook can do
grep -qF 'opened_by: hand' "$ctx"
expect_no_grep '^provider:' "$ctx"

# what the builder resolved is in it, verbatim, and the task it was resolved under
grep -qF '## Context at open' "$ctx"
grep -qF '## TASK' "$ctx"
grep -qF 'task_id: ' "$ctx"
# ...and the notes section the worker writes into
grep -qF '## Notes' "$ctx"

# the same path, by session id and by --json
sid="$(sed -n 's/^session_id: //p' "$ctx")"
expect_exit 0 "$MJ" session context "$sid"
expect_grep "$(basename "$ctx")"
expect_exit 0 "$MJ" --json session context
expect_grep '"context":".ai/local/session-contexts/'

# ---------------------------------------------------------------- the close appends
printf 'The extraction boundary landed; the discovery stage is next.\n' >> "$ctx"
"$MJ" session close >/dev/null
grep -qF 'The extraction boundary landed' "$ctx" \
  || { echo "    the close rewrote the document and lost what the worker typed"; exit 1; }
grep -qF '## Close' "$ctx"
grep -qF -- '- outcome: closed' "$ctx"
grep -qF -- '- record: .ai/repo/sessions/' "$ctx"
# the closed record is the shared object; the working context stays where it is
expect_exit 0 "$MJ" session context "$sid"

# ---------------------------------------------------------------- the store stays local
# The findings are read by name rather than through the exit code. A scratch repository has
# failures of its own — no git hooks, and the sessions section's own context document naming
# files the tool's checkout has and this one does not — and a case that asserted "doctor is
# green" would be asserting those instead of this.
finding() { "$MJ" doctor > "$T/doctor.out" 2>&1 || true; grep -qE -- "$1" "$T/doctor.out" \
  || { printf '    expected /%s/ among the findings\n' "$1"; grep -E 'session' "$T/doctor.out" | sed 's/^/    | /'; return 1; }; }

finding 'OK +session +.ai/local/session-contexts — ignored and untracked'
finding 'OK +session +.ai/local/session-contexts — 1 working context'
git add -f "$ctx"
finding 'FAIL +session +.*tracked by git'
git rm --cached -q "$ctx"

# ---------------------------------------------------------------- and contract-shaped
# A document that grew a key nobody declared: the store's shape is closed for the same
# reason the prompt archive's is — a transcript arrives one field at a time.
cp "$ctx" "$T/ctx.keep"
sed 's/^kind: session-context$/kind: session-context\nsurprise: a field nobody declared/' "$T/ctx.keep" > "$ctx"
finding 'FAIL +session +.*undeclared front-matter key'
finding 'surprise'
cp "$T/ctx.keep" "$ctx"

# ...and one that carries the conversation, which is the thing this store must never become
printf '\ntranscript: what the model said back\n' >> "$ctx"
finding 'FAIL +session +.*carry a conversation'
cp "$T/ctx.keep" "$ctx"

# a document that does not open with the contract at all
printf 'notes I made\n' > "$store/20260101T000000Z--s-planted.md"
finding 'FAIL +session +.*do not open with'
rm -f "$store/20260101T000000Z--s-planted.md"

# ---------------------------------------------------------------- a producer that failed
# The writer runs inside a provider hook, so it can never fail loudly; the log is the only
# trace it leaves, and a non-empty one means an episode has no record of what it was told.
printf '2026-01-01T00:00:00Z cannot write .ai/local/session-contexts/x.md\n' > "$store/.session-context.log"
finding 'FAIL +session +.*could not be written'
rm -f "$store/.session-context.log"

# An open episode with no working context is the producer having failed silently, which is
# exactly what the empty store looked like before it had one.
"$MJ" session start >/dev/null
rm -f "$("$MJ" session context)"
finding 'FAIL +session +.*has no working context'
"$MJ" session close >/dev/null
finding 'OK +session +.*working context'
