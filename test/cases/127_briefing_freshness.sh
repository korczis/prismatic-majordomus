# majordomus-covers: session context
# The briefing an episode opened with is compared against git, everywhere it is handed over.
#
# The defect this case exists to prevent is a one-way check. Every artefact in this
# repository that records a head is labelled against the current checkout before it is
# believed — the task record by mj_validate_state, the resolved handover and the checkpoint
# by the context builder, the derivation's start head by derive, the closed session record by
# `session show` — and every one of them speaks the same four words. The working context was
# the exception, and it is the one the worker is actually holding: frozen by the provider's
# start hook, delivered once, never mentioned again. A session could work an afternoon from a
# briefing whose repository had moved past it while `context`, `session context` and `doctor`
# all stayed quiet.
#
# So the assertions below are about surfaces, not about a format: the places that hand a
# worker its briefing each say how old it is, they agree with each other, and none of them
# refuses an episode for having drifted — drift is the normal state of a briefing and refusing
# it would stop every commit in the repository.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
PATH="$(dirname "$MJ"):$PATH"; export PATH

"$MJ" start "a task" --scope lib/ >/dev/null
"$MJ" session start >/dev/null
ctx="$("$MJ" session context | head -n 1)"
expect_file "$ctx"
sid="$(sed -n 's/^session_id: //p' "$ctx")"
frozen="$(sed -n 's/^head: //p' "$ctx")"
[ -n "$frozen" ] || { echo "    the document carries no head, so nothing below is a test"; exit 1; }

# The findings are read by name and never through the exit code: a scratch repository has
# failures of its own — no git hooks, nothing pushed — and a case asserting "doctor is green"
# would be asserting those instead of this.
finding() { "$MJ" doctor > "$T/doctor.out" 2>&1 || true; grep -qE -- "$1" "$T/doctor.out" \
  || { printf '    expected /%s/ among the findings\n' "$1"; grep -E 'session' "$T/doctor.out" | sed 's/^/    | /'; return 1; }; }
no_finding() { "$MJ" doctor > "$T/doctor.out" 2>&1 || true; grep -qE -- "$1" "$T/doctor.out" \
  && { printf '    did not expect /%s/ among the findings\n' "$1"; return 1; }; return 0; }

# ---------------------------------------------------------------- frozen at this commit
# The briefing was written a moment ago, so every surface must say `exact`, and none of them
# may warn: a guard that warned about every briefing would pass this file while telling the
# worker nothing, which is what the negative assertions here exist for.
#
# The text form of `session context` is a path and stays one — callers consume it as a path —
# so the label rides in the JSON and in the context builder instead.
expect_exit 0 "$MJ" session context
[ "$(printf '%s\n' "$LAST_OUT" | wc -l | tr -d ' ')" = 1 ] \
  || { echo "    session context printed more than the path: $LAST_OUT"; exit 1; }

expect_exit 0 "$MJ" --json session context
expect_grep '"label":"exact"'
expect_grep '"commits_since":0'
expect_grep "\"recorded_head\":\"$frozen\""

expect_exit 0 "$MJ" context
expect_grep "briefing +exact at ${frozen:0:7}"
expect_no_grep 'no longer describes this checkout'

finding "OK +session +$sid — working context under .*, exact at ${frozen:0:7}"

# ---------------------------------------------------------------- the repository moves on
# Two commits, so the count is a number this case can distinguish from a constant.
echo b > lib/b && git add lib/b && git commit -qm one
echo c > lib/c && git add lib/c && git commit -qm two

expect_exit 0 "$MJ" --json session context
expect_grep '"label":"advanced"'
expect_grep '"commits_since":2'
# the document itself is evidence and is never rewritten underneath the prompt that used it
expect_grep "\"recorded_head\":\"$frozen\""
grep -qF "head: $frozen" "$ctx" \
  || { echo "    the frozen briefing was rewritten; it is evidence, not a cache"; exit 1; }

expect_exit 0 "$MJ" context
expect_grep 'briefing +advanced — 2 commit\(s\) since it was frozen'
# advancing is the normal life of a briefing: it is reported, never warned about, and the
# doctrine that owns the store does not refuse the episode for it
expect_no_grep 'no longer describes this checkout'
finding "OK +session +$sid — working context under .*, advanced: 2 commit\(s\) since"

# ---------------------------------------------------------------- another line of history
# The worker moves to the branch it was told to build. That is the ordinary workflow of this
# repository, so it warns in the context builder — the output supersedes the briefing — and
# still does not fail: a doctrine that refused this would refuse every feature branch.
git checkout -q -b feature/elsewhere
expect_exit 0 "$MJ" --json session context
expect_grep '"label":"different_context"'
expect_exit 0 "$MJ" context
expect_grep 'briefing +different_context'
expect_grep 'no longer describes this checkout'
finding "OK +session +$sid — working context under .*, different_context from"
no_finding 'FAIL +session'
git checkout -q -

# ---------------------------------------------------------------- a briefing that cannot be compared
# A document with no head is the producer having failed, and the one state that must fail: a
# briefing nothing can compare must not read as a current one. `unknown` is the vocabulary's
# own word for it, so nothing here invents a second word for stale.
cp "$ctx" "$ctx.keep"
grep -v '^head: ' "$ctx.keep" > "$ctx"
expect_exit 0 "$MJ" --json session context
expect_grep '"label":"unknown"'
finding 'FAIL +session +.*cannot be compared with git'
cp "$ctx.keep" "$ctx" && rm -f "$ctx.keep"
no_finding 'cannot be compared with git'

# ---------------------------------------------------------------- an episode with no document
# Absence is a different fact from staleness, and the doctrine that already reported it must
# keep reporting it rather than labelling a document that is not there. The document is moved
# aside under another episode's name rather than deleted, because an empty store is a third
# fact again and the validator answers that one before it reaches the open episode.
mv "$ctx" "$(dirname "$ctx")/19700101T000000Z--s-19700101000000-0000.md"
finding 'FAIL +session +.*has no working context under'
no_finding 'cannot be compared with git'
