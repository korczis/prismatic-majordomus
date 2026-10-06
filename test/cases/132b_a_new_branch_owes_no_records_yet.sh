# majordomus-covers: doctor
# majordomus-negative: doctor
# A branch created a minute ago owes no checkpoint and no handover.
#
# The lifecycle doctrine (case 132) reports a stopped writer: episodes opening while the
# records they should produce do not exist or have stopped moving. The records are resolved
# for this worktree and branch; the episodes were counted over the whole checkout. So the
# first `git switch -c` in a checkout whose trunk had seen two episodes failed doctor with
# "no handovers record exists for this worktree and branch", and the pre-commit hook that
# runs doctor refused the new branch's first commit. Adopters worked around it by writing a
# checkpoint and a handover nobody needed before every first commit.
#
# Both halves are now judged on the branch: the trunk's episodes say nothing about a new
# branch, and two episodes on that branch with no records are still a stopped writer.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null 2>&1; git commit -qm "layer" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
start_event() { printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start; }
episodes() { for w in "$@"; do
  MJ_CAPTURE_NO_HANDOVER=1 start_event "win-$w" >/dev/null 2>&1
  rm -f "$STATE/sessions-open/win-$w.yaml"
done; }

# the trunk: episodes, and the records they produced
trunk="$(git symbolic-ref --short HEAD)"
episodes a b c
"$MJ" start "trunk work" --scope . >/dev/null
"$MJ" checkpoint --derive >/dev/null
"$MJ" handover --derive --close >/dev/null
"$MJ" doctor > "$T/trunk.out" 2>&1 || true
if grep -qE 'FAIL +lifecycle' "$T/trunk.out"; then
  echo "    the trunk, with its records written, already fails lifecycle"; sed 's/^/    | /' "$T/trunk.out"; exit 1
fi

# a new branch: no episode has opened on it, so nothing is owed yet
git switch -q -c feature/fresh
"$MJ" doctor > "$T/fresh.out" 2>&1 || true
if grep -qE 'FAIL +lifecycle +(handovers|checkpoints)' "$T/fresh.out"; then
  echo "    a branch with no episode of its own was told it owes records ($trunk's episodes were counted)"
  grep -E 'lifecycle' "$T/fresh.out" | sed 's/^/    | /'; exit 1
fi
grep -qE 'OK   lifecycle +activity — 0 episode\(s\)' "$T/fresh.out" || {
  echo "    the new branch's activity was not judged on its own episodes"
  grep -E 'lifecycle' "$T/fresh.out" | sed 's/^/    | /'; exit 1; }

# the doctrine still holds on that branch: episodes open there and nothing writes
episodes d e
"$MJ" doctor > "$T/stopped.out" 2>&1 || true
grep -qE 'FAIL +lifecycle +handovers — 2 episode\(s\) have opened on feature/fresh here and no handovers record exists' "$T/stopped.out" || {
  echo "    two episodes on the branch with no records were not reported"
  grep -E 'lifecycle' "$T/stopped.out" | sed 's/^/    | /'; exit 1; }
