# majordomus-covers: none
# The two commands that keep this repository's self-accumulating quantities bounded, and the
# gate that keeps them honest (rule project.accumulation-is-measured).
#
# Four things here grow without anyone deciding they should — open pull requests, linked
# worktrees, local branches and running servers — and the day they were first counted there
# were sixteen, sixty-four, a hundred and eleven and fourteen of them. The commands that
# reduce them are the dangerous kind: one ends processes, one merges and pushes. So what this
# case proves is not that they run, but that they refuse.
#
# The reaper must never end a server a live session is attached to — sessions here are each
# other's instruments, and a name match would take them all out. The gate must actually fail
# when its subject is broken, because a gate that passes over a missing driver, a missing
# case or a document advertising a merge button that cannot work is how the backlog above was
# allowed to accumulate in silence.
. "$ROOT/test/lib.sh"

REAPER="$ROOT/scripts/reap-orphans"
GATE="$ROOT/scripts/ci/backlog-check"

# ---------------------------------------------------------------- the commands exist
expect_file "$REAPER"
expect_file "$GATE"
[ -x "$REAPER" ] || { echo "    scripts/reap-orphans is not executable"; exit 1; }
[ -x "$GATE" ]   || { echo "    scripts/ci/backlog-check is not executable"; exit 1; }
expect_file "$ROOT/.ai/repo/rules/project/accumulation-is-measured.v1.md"

# ---------------------------------------------------------------- the reaper spares the living
# A process whose parent is alive is not garbage however old it looks. The reaper's whole
# safety rests on that predicate, so the case starts one and insists it survives a dry run.
sleep 600 &
LIVE=$!
trap 'kill "$LIVE" 2>/dev/null || true' EXIT
out="$("$REAPER" --min-age 0s 2>&1 || true)"
if grep -qE "^$LIVE " <<<"$out"; then
  echo "    the reaper listed pid $LIVE, whose parent is this test: it reaps by age, not by ownership"; exit 1
fi

# Dry run is the default and must change nothing: the process it was pointed at is still here.
"$REAPER" --min-age 0s >/dev/null 2>&1 || true
ps -p "$LIVE" >/dev/null 2>&1 || { echo "    the default run ended a process; the reaper must be dry unless --kill"; exit 1; }

# It matches by predicate. A name match is the one implementation that is always wrong here.
grep -nE '^[^#]*\b(pkill|killall)\b' "$REAPER" >/dev/null 2>&1 && {
  echo "    scripts/reap-orphans matches processes by name; it would end the servers live peers use"; exit 1; }

# ---------------------------------------------------------------- the gate passes on the tree it guards
expect_exit 0 "$GATE"

# ---------------------------------------------------------------- and fails when its subject is broken
# A copy, so the findings are provoked without touching the repository. Only the paths the
# gate reads are needed; MJ_ROOT points it at them.
W="$(mktemp -d)"
trap 'kill "$LIVE" 2>/dev/null || true; rm -rf "$W"' EXIT
mkdir -p "$W/scripts/ci" "$W/test/cases" "$W/.ai/repo/rules/project" "$W/docs" "$W/.just"
cp "$ROOT/.gitattributes" "$W/.gitattributes"
cp "$ROOT/scripts/merge-derived" "$W/scripts/merge-derived"
cp "$REAPER" "$W/scripts/reap-orphans"
cp "$GATE" "$W/scripts/ci/backlog-check"
cp "$ROOT/.ai/repo/rules/project/accumulation-is-measured.v1.md" "$W/.ai/repo/rules/project/"
cp "$ROOT/test/cases/131_backlog_hygiene.sh" "$W/test/cases/"
cp "$ROOT/.just/site.just" "$W/.just/site.just" 2>/dev/null || true
touch "$W/justfile"
chmod +x "$W/scripts/merge-derived" "$W/scripts/reap-orphans" "$W/scripts/ci/backlog-check"

MJ_ROOT="$W" expect_exit 0 "$W/scripts/ci/backlog-check"

# the declaration without its driver — the one-way relation this gate exists to close
mv "$W/scripts/merge-derived" "$W/scripts/merge-derived.away"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep 'declares merge=derived but scripts/merge-derived is missing'
mv "$W/scripts/merge-derived.away" "$W/scripts/merge-derived"

# a document that sends a reader to the merge button, which cannot work in this repository
printf 'To land this, click the Merge pull request button on GitHub.\n' > "$W/docs/HOWTO.md"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep 'instructs a reader to merge on the forge'
rm "$W/docs/HOWTO.md"

# a case that backgrounds a server without bounding its life — the shape that actually
# leaked here: `serve`'s --idle defaults to 0, so an interrupted run abandons the server
mkdir -p "$W/test/cases"
printf '#!/bin/sh\n"$BIN" serve --port 0 > serve.log 2>&1 &\n' > "$W/test/cases/99_probe.sh"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep 'backgrounds a server without --idle'
printf '#!/bin/sh\n"$BIN" serve --port 0 --idle 60 > serve.log 2>&1 &\n' > "$W/test/cases/99_probe.sh"
MJ_ROOT="$W" expect_exit 0 "$W/scripts/ci/backlog-check"
rm "$W/test/cases/99_probe.sh"

# the behavioural case itself going missing
rm "$W/test/cases/131_backlog_hygiene.sh"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep '131_backlog_hygiene.sh is missing'
