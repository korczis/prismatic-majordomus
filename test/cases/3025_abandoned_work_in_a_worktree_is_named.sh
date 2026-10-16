# majordomus-covers: none
# Work a session left uncommitted in a worktree is named, not found by luck (I1995).
#
# On 2026-10-07 and 2026-10-08 finished work sat as uncommitted and staged files in the
# worktrees of sessions that had ended; doctor named branches whose commits reach no remote
# and said nothing about an index that holds work no commit carries. Both were found because
# somebody happened to read those directories.
#
# A repository with four linked worktrees, each holding uncommitted work:
#
#   abandoned  three staged paths, months old, an open episode silent for years: nothing
#              holds it — named by `worktree uncommitted`, by the MCP tool and by doctor,
#              with its path, its count, its age and a rescue command
#   tasked     an old unstaged change, an active task record: held, listed, not warned
#   attended   an old untracked file, an open episode that spoke a moment ago: held
#   recent     a file written now, nothing holding it: recent, listed, not warned
#
# The pre-commit hook runs doctor with MAJORDOMUS_DOCTOR_CONTEXT=commit, and there doctor
# names the reading it leaves out and never asks the executable for it (a spy records every
# call); a plain doctor asks for it and warns. A context doctor does not know is refused.
#
# And none of it writes: `git status --porcelain` of every worktree, and the index file of
# every linked one, are byte-identical before and after the reading, the tool and doctor.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
W="$(mktemp -d "${TMPDIR:-/tmp}/mj3025.XXXXXX")"; trap 'rm -rf "$W"' EXIT
W="$(cd "$W" && pwd -P)"

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm "the layer"
printf 'one\n' > tracked.txt && git add tracked.txt && git commit -qm "a tracked file"

OLD=202001010000     # touch -t: years past any threshold a policy would declare
now_ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
for b in abandoned tasked attended recent; do git worktree add -q -b "work/$b" "$W/$b"; done

# abandoned: two new files and one modification, all staged, all old; an open episode in it
# that has been silent since 2020 is stranded and holds nothing
( cd "$W/abandoned" && printf 'a\n' > a.txt && printf 'b\n' > b.txt && printf 'two\n' > tracked.txt \
  && git add a.txt b.txt tracked.txt && TZ=UTC touch -t "$OLD" a.txt b.txt tracked.txt )
mkdir -p "$W/abandoned/.ai/local/state/sessions-open"
printf 'session_id: s-20200101000000-0a0a\nstarted_at: 2020-01-01T00:00:00Z\nprovider: "claude-code"\n' \
  > "$W/abandoned/.ai/local/state/sessions-open/gone.yaml"

# tasked: an old unstaged change, and the active task record `majordomus start` writes
( cd "$W/tasked" && printf 'three\n' > tracked.txt && TZ=UTC touch -t "$OLD" tracked.txt )
mkdir -p "$W/tasked/.ai/local/state"
printf 'id: t-20260101000000-beef\ntask: "a task in flight"\noutcome: active\n' \
  > "$W/tasked/.ai/local/state/current.yaml"

# attended: an old untracked file, and an episode that spoke a moment ago
( cd "$W/attended" && printf 'c\n' > c.txt && TZ=UTC touch -t "$OLD" c.txt )
mkdir -p "$W/attended/.ai/local/state/sessions-open"
printf 'session_id: s-%s-0b0b\nstarted_at: %s\nprovider: "claude-code"\n' \
  "$(date -u +%Y%m%d%H%M%S)" "$now_ts" > "$W/attended/.ai/local/state/sessions-open/here.yaml"

# recent: a file written now, nothing holding it
printf 'd\n' > "$W/recent/d.txt"

for d in abandoned tasked attended; do
  git -C "$W/$d" check-ignore -q .ai/local/state \
    || { echo "    .ai/local is not ignored in the fixture; the counts below would include it"; exit 1; }
done

# fingerprint: every worktree's status, read without optional locks, and every linked
# worktree's index file, bytes and modification time
fingerprint() {
  local d
  for d in "$T" "$W/abandoned" "$W/tasked" "$W/attended" "$W/recent"; do
    printf '%s\n' "$d"
    git -C "$d" --no-optional-locks status --porcelain=v1 -z --untracked-files=all | od -c
  done
  for d in "$W/abandoned" "$W/tasked" "$W/attended" "$W/recent"; do
    idx="$(git -C "$d" rev-parse --path-format=absolute --git-path index)"
    printf '%s %s\n' "$(sha256_of_file "$idx")" "$(ls -lT "$idx" 2>/dev/null || ls -l --time-style=full-iso "$idx")"
  done
}
fingerprint > "$W/before.txt"

# ---------------------------------------------------------------- the reading
"$RB" worktree uncommitted --repo "$T" --format json > "$W/r.json" 2> "$W/r.err" \
  || { echo "    worktree uncommitted refused:"; cat "$W/r.err"; exit 1; }
row() { jq -c --arg p "$W/$1" '.worktrees[] | select(.path == $p)' "$W/r.json"; }
bad() { echo "    $1"; jq -c . "$W/r.json" | head -c 2000; echo; exit 1; }

jq -e '.schema == "majordomus-worktree-uncommitted/v1" and .threshold.source == "session.stranded_after" and .threshold.declared == "12h"' "$W/r.json" >/dev/null \
  || bad "the threshold is not the policy's session.stranded_after:"
jq -e '.tallies == {"measured":4,"uncommitted":4,"held":2,"recent":1,"abandoned":1}' "$W/r.json" >/dev/null \
  || bad "four linked worktrees with work were not tallied 2 held, 1 recent, 1 abandoned:"
[ "$(jq -r '.worktrees[0].path' "$W/r.json")" = "$W/abandoned" ] || bad "the abandoned worktree is not listed first:"

row abandoned | jq -e '.standing == "abandoned" and .paths == 3 and .work.staged == 3 and .held_by == []
  and (.age | test("^[0-9]+d$")) and .age_seconds > 86400*365
  and (.oldest_change | startswith("2020-01-01"))
  and (.episodes | length == 1 and .[0].stranded == true)
  and (.remedy | contains("push origin HEAD:refs/heads/rescue/work-abandoned-"))' >/dev/null \
  || bad "the abandoned worktree is not named with three staged paths, its age, a stranded episode and a rescue command:"
row tasked | jq -e '.standing == "held" and .task == "t-20260101000000-beef" and .paths == 1 and .work.unstaged == 1
  and (.held_by == ["task t-20260101000000-beef"])' >/dev/null \
  || bad "the worktree with an active task is not held by it:"
row attended | jq -e '.standing == "held" and .paths == 1 and .work.untracked == 1
  and (.episodes | length == 1 and .[0].stranded == false) and (.held_by | .[0] | startswith("episode s-"))' >/dev/null \
  || bad "the worktree with a live episode is not held by it:"
row recent | jq -e '.standing == "recent" and .paths == 1 and .held_by == [] and .age_seconds < 43200' >/dev/null \
  || bad "the fresh worktree nobody holds is not recent:"

# the caller's own threshold replaces the policy's, for both of its questions: against 100000
# days nothing is old, and the episode silent since 2020 is not stranded, so it holds its work
"$RB" worktree uncommitted --repo "$T" --older-than 100000d --format json > "$W/o.json" \
  || { echo "    --older-than was refused"; exit 1; }
jq -e '.threshold.source == "older_than" and .tallies.abandoned == 0 and .tallies.held == 3 and .tallies.recent == 1' "$W/o.json" >/dev/null \
  || { echo "    --older-than did not replace the policy's threshold:"; jq -c '.threshold, .tallies' "$W/o.json"; exit 1; }
expect_exit 2 "$RB" worktree uncommitted --repo "$T" --older-than soon
expect_grep 'is not a duration'

# the human form names the same four, and the rescue command for the abandoned one only
"$RB" worktree uncommitted --repo "$T" > "$W/r.txt" || { echo "    the text form refused"; exit 1; }
grep -q "^  abandoned work/abandoned  $W/abandoned  3 path(s) (3 staged), oldest changed [0-9]*d ago" "$W/r.txt" \
  || { echo "    the text form does not name the abandoned worktree:"; cat "$W/r.txt"; exit 1; }
[ "$(grep -c 'preserve: ' "$W/r.txt")" = 1 ] || { echo "    a rescue is suggested for work something holds:"; cat "$W/r.txt"; exit 1; }
grep -q '1 abandoned, 1 recent, 2 held; nothing was changed$' "$W/r.txt" \
  || { echo "    the text form's tallies disagree with the JSON:"; cat "$W/r.txt"; exit 1; }

# ---------------------------------------------------------------- the tool, as a client asks it
{ printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3025","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_worktree_uncommitted","arguments":{}}}\n'
} | "$RB" mcp --standalone --repo "$T" 2>"$W/mcp.err" | sed -n 2p > "$W/frame.json"
jq -e '.result.structuredContent' "$W/frame.json" > "$W/tool.json" 2>/dev/null \
  || { echo "    majordomus_worktree_uncommitted returned no typed answer:"; head -c 800 "$W/frame.json"; echo; head -5 "$W/mcp.err"; exit 1; }
[ "$(jq -c '.tallies, [.worktrees[] | [.path, .standing, .paths]]' "$W/tool.json")" = \
  "$(jq -c '.tallies, [.worktrees[] | [.path, .standing, .paths]]' "$W/r.json")" ] \
  || { echo "    the tool and the command line disagree:"; jq -c '.tallies' "$W/tool.json" "$W/r.json"; exit 1; }

# ---------------------------------------------------------------- doctor
# The executable doctor asks goes through a spy that writes down every command it is asked,
# so "doctor did not read the worktrees" is a measurement and not an inference from output.
printf '#!/bin/sh\nprintf "%%s\\n" "$*" >> "%s/spy.log"\nexec "%s" "$@"\n' "$W" "$RB" > "$W/spy"
chmod +x "$W/spy"

# the commit hook: this repository's hook declares the context, and in it doctor names the
# reading it leaves out — and never asks for it
grep -q '^MAJORDOMUS_DOCTOR_CONTEXT=commit bin/majordomus doctor || exit \$?$' "$ROOT/.githooks/pre-commit" \
  || { echo "    .githooks/pre-commit does not run doctor in the commit context"; exit 1; }
: > "$W/spy.log"
MAJORDOMUS_DOCTOR_CONTEXT=commit MAJORDOMUS_BIN="$W/spy" "$MJ" doctor > "$W/hook.txt" 2>&1 || true
grep -qF "INFO clone       uncommitted — not read in the commit hook (MAJORDOMUS_DOCTOR_CONTEXT=commit): it takes one git status per linked worktree; run it on its own  [reproduce: majordomus worktree uncommitted]" "$W/hook.txt" \
  || { echo "    in the commit hook doctor does not say it left the reading out:"; grep -i 'clone' "$W/hook.txt"; exit 1; }
if grep -q 'worktree uncommitted' "$W/spy.log"; then
  echo "    in the commit hook doctor still read every worktree:"; cat "$W/spy.log"; exit 1
fi
if grep -q "WARN clone .*$W/" "$W/hook.txt"; then
  echo "    in the commit hook doctor warned about a worktree it was not to read:"; grep -i 'clone' "$W/hook.txt"; exit 1
fi
# a context doctor does not know is refused, never read as unset
expect_exit 2 env MAJORDOMUS_DOCTOR_CONTEXT=comit MAJORDOMUS_BIN="$RB" "$MJ" doctor
expect_grep "MAJORDOMUS_DOCTOR_CONTEXT='comit' is not a context"

# a plain doctor reads them, and warns about the abandoned one only
: > "$W/spy.log"
MAJORDOMUS_BIN="$W/spy" "$MJ" doctor > "$W/doctor.txt" 2>&1 || true
grep -q '^worktree uncommitted --repo ' "$W/spy.log" \
  || { echo "    a plain doctor did not ask for the reading:"; cat "$W/spy.log"; exit 1; }
if grep -q 'not read in the commit hook' "$W/doctor.txt"; then
  echo "    a plain doctor says it runs in the commit hook"; exit 1
fi
grep -qF "WARN clone       $W/abandoned — 3 uncommitted path(s), the oldest changed " "$W/doctor.txt" \
  || { echo "    doctor does not name the abandoned worktree with its path and count:"; grep -i 'clone' "$W/doctor.txt"; exit 1; }
grep -E "WARN clone +$W/abandoned — 3 uncommitted path\(s\), the oldest changed [0-9]+d ago, .*\[reproduce: git -C '$W/abandoned' add -A && .*rescue/work-abandoned-[0-9]{8}\]" "$W/doctor.txt" >/dev/null \
  || { echo "    doctor's warning lacks the age or the rescue command:"; grep -i 'clone' "$W/doctor.txt"; exit 1; }
for d in tasked attended recent; do
  if grep -q "WARN clone .*$W/$d " "$W/doctor.txt"; then
    echo "    doctor warns about $d, whose work is held or recent:"; grep -i 'clone' "$W/doctor.txt"; exit 1
  fi
done

# ---------------------------------------------------------------- nothing was written
fingerprint > "$W/after.txt"
cmp -s "$W/before.txt" "$W/after.txt" \
  || { echo "    the reading, the tool or doctor changed a worktree's status or index:"; diff "$W/before.txt" "$W/after.txt" | head -20; exit 1; }
[ -f "$W/abandoned/.ai/local/state/sessions-open/gone.yaml" ] && [ -f "$W/tasked/.ai/local/state/current.yaml" ] \
  || { echo "    a record in another worktree's .ai/local was removed"; exit 1; }
exit 0
