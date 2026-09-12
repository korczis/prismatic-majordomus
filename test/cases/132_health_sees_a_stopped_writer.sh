# majordomus-covers: doctor
# majordomus-negative: doctor
# Health must go red when the writer stops — the other half of the 2026-09-05 outage.
#
# Case 130 proves the lifecycle now runs against the state that stopped it. This one proves
# the complementary thing: that if it ever stops again, somebody is told the same day.
#
# For six days every health check passed, and each passed honestly. `retention` counted the
# files and found them under their caps; `layout` found the directories present; `resolver`
# found a record and reported its divergence label. A store that nothing writes to is
# perfectly reachable, perfectly well-formed and perfectly under its retention cap.
#
# The invariant that catches it spans two facts no single check owned: that episodes are
# opening, and that the records those episodes produce have not moved. Either alone is
# unremarkable — an idle repository is not a broken one. Together they are a stopped writer.
# This case seeds exactly that pair and requires a finding, then lets the lifecycle run and
# requires the finding to go away.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
start_event() { printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"clear"}' "$1" | ./.claude/hooks/majordomus-session-end; }

echo seed > seed.txt && git add seed.txt && git commit -qm "a commit for the stale records to name"

# ---------------------------------------------------------------- seed: the writer stopped
# Records six days old, and episodes opening ever since. The clock is moved rather than the
# records forged, so every writer below is the real one.
OLD=2026-09-05T03:45:23Z
MAJORDOMUS_NOW="$OLD" "$MJ" start "the task that stopped being worked on" --scope . >/dev/null
MAJORDOMUS_NOW="$OLD" "$MJ" checkpoint --derive >/dev/null
MAJORDOMUS_NOW="$OLD" "$MJ" handover --derive --close >/dev/null

# Episodes since then. Two is the minimum the validator will judge on: below that it cannot
# tell a stopped writer from a checkout nobody has used yet, and saying so is better than
# guessing. These use the real clock, so the gap between them and the records is the finding.
for w in a b c; do
  MJ_CAPTURE_NO_HANDOVER=1 start_event "win-$w" >/dev/null 2>&1
  rm -f "$STATE/sessions-open/win-$w.yaml"
done

# The records must genuinely still be the six-day-old ones: if the episodes above wrote
# anything, this case is testing nothing.
NEWEST_H="$(find "$STATE/handovers" -name '2*.md' | LC_ALL=C sort | tail -n 1)"
grep -q "^created_at: $OLD" "$NEWEST_H" || {
  echo "    the seed did not leave a six-day-old handover as the newest one"; exit 1; }

# ---------------------------------------------------------------- health must say so
# 10 is the exit the rule declares for a blocking failure.
set +e
"$MJ" doctor > "$T/doctor.out" 2>&1
DOCTOR_EXIT=$?
set -e

# The pattern names the category AND the finding. An earlier version of this assertion
# matched any line containing "lifecycle", and passed on the doctrine meta-check —
# "mj_validate_lifecycle exists but no rule declares it" — which is a real failure of a
# different thing entirely. A gate that can be satisfied by the wrong finding is not a gate.
grep -qE 'FAIL +lifecycle.*(handovers|checkpoints).*(stopped|stale)' "$T/doctor.out" || {
  echo "    doctor did not report the stopped writer; this is the outage going unseen again"
  echo "    doctor exited $DOCTOR_EXIT"
  sed 's/^/    | /' "$T/doctor.out"; exit 1; }

# And nothing here may pass because the validator is unwired. That finding is about this
# repository's rule graph, not about the lifecycle, and if it is present the case above is
# not measuring what it claims to.
grep -q 'validator exists but no rule declares it' "$T/doctor.out" && {
  echo "    mj_validate_lifecycle runs under no rule; the finding above is the meta-check, not the lifecycle"
  sed 's/^/    | /' "$T/doctor.out"; exit 1; }

[ "$DOCTOR_EXIT" = 10 ] || {
  printf '    doctor reported the finding and exited %s; a blocking rule exits 10\n' "$DOCTOR_EXIT"
  sed 's/^/    | /' "$T/doctor.out"; exit 1; }

printf '    doctor on a six-day-silent lifecycle:\n'
grep -i lifecycle "$T/doctor.out" | sed 's/^/    | /'

# ---------------------------------------------------------------- and must stop saying so
# The lifecycle runs once, for real, through the shims a provider would drive. The records
# it writes are today's, so the invariant is satisfied and the finding must clear. A gate
# that cannot be satisfied is as useless as one that cannot fail.
start_event win-live >/dev/null 2>&1
end_event win-live >/dev/null 2>&1

set +e
"$MJ" doctor > "$T/doctor2.out" 2>&1
DOCTOR2_EXIT=$?
set -e

grep -qE 'FAIL +lifecycle' "$T/doctor2.out" && {
  echo "    doctor still reports a stopped writer after the lifecycle ran and wrote records (exit $DOCTOR2_EXIT)"
  sed 's/^/    | /' "$T/doctor2.out"; exit 1; }

grep -qE 'lifecycle' "$T/doctor2.out" || {
  echo "    doctor stopped reporting on the lifecycle altogether; silence is not health"
  sed 's/^/    | /' "$T/doctor2.out"; exit 1; }
