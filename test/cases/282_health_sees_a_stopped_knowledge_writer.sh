# majordomus-covers: knowledge doctor watch
# majordomus-negative: doctor watch
# claim: knowledge-writer-observed
# Health must go red when the knowledge writer stops, and only then.
#
# Case 132 proved that a stopped handover writer is reported the same day. The knowledge
# writer is the same shape of invariant with the same failure mode: a store that can be read
# is perfectly well-formed while nothing writes to it. The two facts no single check owned
# are that episodes keep closing and that no derivation follows them, and the finding
# compares episode ids rather than timestamps between the two events, because the close
# derives before it closes.
#
# The judgement is gated: until one derivation has run in this checkout, doctor says so and
# judges nothing, so that the day this arrives no checkout turns red for episodes closed
# before the writer existed. The gate is opened here deliberately before the seed. The
# other half of the rule is wiring — the close path must call the deriver at all — and it
# is mutation-tested against a copy of the tool, as case 18 does for the doctrine chain.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "${TMPDIR:-/tmp}/mj.pol.$$" \
  && cp "${TMPDIR:-/tmp}/mj.pol.$$" .ai/repo/policy.yaml && rm -f "${TMPDIR:-/tmp}/mj.pol.$$"
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
LEDGER="$STATE/ledger.jsonl"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.knowledge.XXXXXX")"
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
start_event() { printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"clear"}' "$1" | ./.claude/hooks/majordomus-session-end; }
sid_of() { sed -n 's/^session_id: //p' "$STATE/sessions-open/$1.yaml" | head -n 1; }
pol_set() {
  sed "s/^\(  $1:\) .*/\1 $2/" .ai/repo/policy.yaml > "$S/policy" && cp "$S/policy" .ai/repo/policy.yaml
  grep -q "^  $1: $2" .ai/repo/policy.yaml || { printf '    the policy probe did not take: %s: %s\n' "$1" "$2"; exit 1; }
}
# The two declared enforcement hooks: a fresh checkout has none, and their absence is a
# real doctor failure that would mask the one this case is about.
wire() {
  mkdir -p .git/hooks
  printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
  printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
  chmod +x .git/hooks/pre-commit .git/hooks/pre-push
}
# committed before the hooks are wired: a pre-commit that runs doctor is the thing under
# test, not a gate this seed has to pass
echo seed > seed.txt && git add seed.txt && git commit -qm "a commit for the records to name"
wire

# ---------------------------------------------------------------- before anything: not judged
set +e; "$MJ" doctor > "$S/doctor0.out" 2>&1; set -e
grep -qE 'FAIL +knowledge' "$S/doctor0.out" && {
  echo "    a repository in which no episode has closed is reported as a stopped knowledge writer"
  grep knowledge "$S/doctor0.out" | sed 's/^/    | /'; exit 1; }
grep -qE '(OK|INFO) +knowledge' "$S/doctor0.out" || {
  echo "    doctor says nothing about the knowledge writer on a fresh repository; silence is not health"
  sed 's/^/    | /' "$S/doctor0.out"; exit 1; }

# ---------------------------------------------------------------- open the gate, keep the records fresh
# The seed below moves the clock, and a handover or checkpoint written under the old clock
# would be a stale record of its own — the lifecycle finding of case 132, not this one. So
# the fresh records are written first, by hand, and the seeded episode is told to write
# none. One derivation by hand opens the gate the judgement stands behind.
"$MJ" checkpoint --derive >/dev/null
"$MJ" handover --derive --no-task >/dev/null
"$MJ" session start >/dev/null
expect_exit 0 "$MJ" knowledge derive
expect_grep '^knowledge derive: '
"$MJ" session close >/dev/null
must "the gate did not open: no knowledge.derived line in the ledger" grep -q '"event":"knowledge.derived"' "$LEDGER"

# ---------------------------------------------------------------- seed: an episode closed a week ago, never derived
OLD=2026-09-05T03:45:23Z
pol_set knowledge_on_end false
pol_set checkpoint_on_compact false
pol_set handover_on_end false
MAJORDOMUS_NOW="$OLD" start_event e1 >/dev/null 2>&1
SID="$(sid_of e1)"
must "the seed opened no episode" [ -n "$SID" ]
MAJORDOMUS_NOW="$OLD" end_event e1 >/dev/null 2>&1
must "the seeded episode did not close" [ ! -f "$STATE/sessions-open/e1.yaml" ]
grep -q "\"event\":\"session.closed\".*\"session\":\"$SID\"" "$LEDGER" || { echo "    no session.closed line for the seeded episode"; exit 1; }
grep -q "\"event\":\"knowledge.derived\".*\"episode\":\"$SID\"" "$LEDGER" && {
  echo "    the seed derived knowledge for the episode it was supposed to leave underived; the probe measures nothing"; exit 1; }
[ "$(grep -c '"event":"provider.event.failed"' "$LEDGER" || true)" = 0 ] || {
  echo "    the seed recorded a lifecycle failure, which is a lifecycle finding of its own"; grep failed "$LEDGER" | sed 's/^/    | /'; exit 1; }
pol_set knowledge_on_end true
pol_set checkpoint_on_compact true
pol_set handover_on_end true

# ---------------------------------------------------------------- health must say so
set +e
"$MJ" doctor > "$S/doctor.out" 2>&1; DOCTOR_EXIT=$?
"$MJ" watch > "$S/watch.out" 2>&1; WATCH_EXIT=$?
set -e
grep -qE 'FAIL +knowledge .*(stopped|no knowledge\.derived)' "$S/doctor.out" || {
  echo "    doctor did not report the stopped knowledge writer; this is the outage of ADR 0052 in a new store"
  echo "    doctor exited $DOCTOR_EXIT"; sed 's/^/    | /' "$S/doctor.out"; exit 1; }
grep -qE "FAIL +knowledge .*$SID" "$S/doctor.out" || {
  echo "    the finding does not name the episode that was closed and never derived ($SID)"
  grep knowledge "$S/doctor.out" | sed 's/^/    | /'; exit 1; }
grep -q 'validator exists but no rule declares it' "$S/doctor.out" && {
  echo "    mj_validate_knowledge_lifecycle runs under no rule; the finding above is the meta-check"
  grep 'no rule declares' "$S/doctor.out" | sed 's/^/    | /'; exit 1; }
[ "$DOCTOR_EXIT" = 10 ] || { printf '    doctor reported the finding and exited %s; a blocking rule exits 10\n' "$DOCTOR_EXIT"; exit 1; }
printf '    doctor on a week-silent knowledge writer:\n'; grep knowledge "$S/doctor.out" | sed 's/^/    | /'

grep -qE 'DRIFT +knowledge .*(stopped|no knowledge\.derived)' "$S/watch.out" || {
  echo "    watch did not report the stopped knowledge writer as drift (exit $WATCH_EXIT)"; sed 's/^/    | /' "$S/watch.out"; exit 1; }
[ "$WATCH_EXIT" = 11 ] || { printf '    watch found drift and exited %s; drift exits 11\n' "$WATCH_EXIT"; exit 1; }

# ---------------------------------------------------------------- and must stop saying so
# The derivation for the closed episode is run by hand, named by id, which is the remedy the
# finding offers. A gate that cannot be satisfied is as useless as one that cannot fail.
expect_exit 0 "$MJ" knowledge derive --episode "$SID"
expect_grep "for episode $SID"
grep -q "\"event\":\"knowledge.derived\".*\"episode\":\"$SID\"" "$LEDGER" || { echo "    derive --episode left no knowledge.derived line naming $SID"; exit 1; }

set +e; "$MJ" doctor > "$S/doctor2.out" 2>&1; DOCTOR2_EXIT=$?; set -e
grep -qE 'FAIL +knowledge' "$S/doctor2.out" && {
  echo "    doctor still reports a stopped knowledge writer after the derivation ran (exit $DOCTOR2_EXIT)"
  grep knowledge "$S/doctor2.out" | sed 's/^/    | /'; exit 1; }
grep -qE 'OK +knowledge' "$S/doctor2.out" || {
  echo "    doctor stopped reporting on the knowledge writer altogether; silence is not health"
  sed 's/^/    | /' "$S/doctor2.out"; exit 1; }
[ "$DOCTOR2_EXIT" = 0 ] || {
  printf '    the finding cleared and doctor still exits %s:\n' "$DOCTOR2_EXIT"
  grep -E '^FAIL' "$S/doctor2.out" | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- the wiring half
# A copy of the tool with the deriver's call removed from the close path. The validator
# reads the source, the way mj_validate_doctrine_wiring does, because a switch that is on
# and an adapter that never calls the deriver is a stopped writer that no ledger will ever
# show — there is nothing to be late.
mkdir -p "$S/tool"
fixture_repo "$S/tool"
MJ2="$S/tool/bin/majordomus"
LIB2="$S/tool/lib"
# The call is replaced, not deleted: it sits inside a command substitution that spans
# lines, and deleting the line leaves a file bash refuses to parse, which is a different
# failure from the one this half is about. `true` takes the deriver's arguments and writes
# nothing, which is exactly an adapter that never calls it.
had=0
for f in capture session; do
  grep -qF 'mj_cmd_knowledge derive' "$LIB2/$f.sh" || continue
  had=1
  sed 's/mj_cmd_knowledge derive/true/g' "$ROOT/lib/$f.sh" > "$LIB2/$f.sh"
  grep -qF 'mj_cmd_knowledge derive' "$LIB2/$f.sh" && { printf '    the probe did not take: the call is still in lib/%s.sh\n' "$f"; exit 1; }
  bash -n "$LIB2/$f.sh" || { printf '    the mutated lib/%s.sh does not parse; the probe broke the tool rather than unwiring it\n' "$f"; exit 1; }
done
[ "$had" = 1 ] || { echo "    neither lib/capture.sh nor lib/session.sh calls the deriver; nothing to remove"; exit 1; }

W="$S/wire"; mkdir -p "$W"
( cd "$W" && git init -q . && git config user.email t@example.com && git config user.name t && git commit -q --allow-empty -m init )
cd "$W"
"$MJ2" init >/dev/null; "$MJ2" update >/dev/null
wire
PATH="$(dirname "$MJ2"):$PATH"; export PATH
# the gate, opened the same way, so that the wiring finding is not hiding behind it
"$MJ2" session start >/dev/null
"$MJ2" knowledge derive >/dev/null
"$MJ2" session close >/dev/null
set +e; "$MJ2" doctor > "$S/doctor3.out" 2>&1; DOCTOR3_EXIT=$?; set -e
grep -qE 'FAIL +knowledge .*lib/(capture|session)\.sh' "$S/doctor3.out" || {
  echo "    doctor did not notice that the close path never calls the deriver (exit $DOCTOR3_EXIT)"
  grep -E 'knowledge' "$S/doctor3.out" | sed 's/^/    | /'; exit 1; }
[ "$DOCTOR3_EXIT" = 10 ] || { printf '    doctor reported the unwired deriver and exited %s\n' "$DOCTOR3_EXIT"; exit 1; }
printf '    doctor on an unwired deriver:\n'; grep -E 'FAIL +knowledge' "$S/doctor3.out" | sed 's/^/    | /'
