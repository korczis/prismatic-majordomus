# majordomus-covers: knowledge doctor watch
# majordomus-negative: doctor
# claim: candidates-reviewed-not-accumulated
# A review queue is measured: over the cap, or older than the age, is a finding that names it.
#
# A candidate is written by a machine at every episode boundary and reviewed by a person
# when a person gets to it. Nothing stops the first from outrunning the second, so the
# policy declares two numbers once — how many may wait and how long one may wait — and
# doctor reports the queue against them (project.accumulation-is-measured). Advisory,
# because whether a candidate deserves review is a judgement; dispatched from doctor only,
# because under watch an advisory finding is drift, and a full queue must not fail a watch.
# A candidate's review age is the age of the queue entry, not the date of its evidence.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "${TMPDIR:-/tmp}/mj.pol.$$" \
  && cp "${TMPDIR:-/tmp}/mj.pol.$$" .ai/repo/policy.yaml && rm -f "${TMPDIR:-/tmp}/mj.pol.$$"
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
CAND=.ai/repo/knowledge/candidates
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.knowledge.XXXXXX")"
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
start_event() { printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"clear"}' "$1" | ./.claude/hooks/majordomus-session-end; }
sid_of() { sed -n 's/^session_id: //p' "$STATE/sessions-open/$1.yaml" | head -n 1; }
candidates() { grep -l '^status: candidate$' "$CAND"/*.md 2>/dev/null | wc -l | tr -d ' '; }
pol_set() {
  sed "s/^\(  $1:\) .*/\1 $2/" .ai/repo/policy.yaml > "$S/policy" && cp "$S/policy" .ai/repo/policy.yaml
  grep -q "^  $1: $2" .ai/repo/policy.yaml || { printf '    the policy probe did not take: %s: %s\n' "$1" "$2"; exit 1; }
}
wire() {
  mkdir -p .git/hooks
  printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
  printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
  chmod +x .git/hooks/pre-commit .git/hooks/pre-push
}
wire
episode_with() {   # provider-session count
  local i=1
  start_event "$1" >/dev/null 2>&1
  while [ "$i" -le "$2" ]; do
    "$MJ" decision add "Decision $i of episode $1" --why "to fill the queue" >/dev/null
    i=$((i + 1))
  done
  end_event "$1" >/dev/null 2>&1
  must "episode $1 did not close" [ ! -f "$STATE/sessions-open/$1.yaml" ]
}
doctor_run() { set +e; "$MJ" doctor > "$1" 2>&1; local rc=$?; set -e; return "$rc"; }

# the numbers are the policy's, declared once; the case reads them rather than repeating them
must "the policy declares no knowledge.candidates_max_files" grep -q '^  candidates_max_files: ' .ai/repo/policy.yaml
must "the policy declares no knowledge.candidate_max_age_minutes" grep -q '^  candidate_max_age_minutes: ' .ai/repo/policy.yaml
pol_set candidates_max_files 2

# ---------------------------------------------------------------- under the cap
episode_with e1 2
[ "$(candidates)" = 2 ] || { printf '    two decisions left %s candidate(s)\n' "$(candidates)"; exit 1; }
# tracked, so that the untracked-candidates finding (its own clause) stays out of this one
git add -A .ai/repo/knowledge >/dev/null; git commit -qm "two candidates" >/dev/null
set +e; "$MJ" watch > "$S/watch1.out" 2>&1; WATCH1=$?; set -e
rc=0; doctor_run "$S/doctor1.out" || rc=$?
grep -qE 'WARN +knowledge .*(cap|candidates_max_files)' "$S/doctor1.out" && {
  echo "    two candidates under a cap of two is reported as over it:"; grep knowledge "$S/doctor1.out" | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- over the cap: WARN, exit 0
episode_with e2 1
[ "$(candidates)" = 3 ] || { printf '    three decisions left %s candidate(s)\n' "$(candidates)"; exit 1; }
git add -A .ai/repo/knowledge >/dev/null; git commit -qm "a third candidate" >/dev/null
rc=0; doctor_run "$S/doctor2.out" || rc=$?
grep -qE 'WARN +knowledge .*(over|cap|candidates_max_files)' "$S/doctor2.out" || {
  echo "    doctor did not report three candidates over a cap of two (exit $rc):"; grep -E 'knowledge' "$S/doctor2.out" | sed 's/^/    | /'; exit 1; }
grep -qE 'WARN +knowledge .*2' "$S/doctor2.out" || { echo "    the finding does not name the cap it measured against"; grep -E 'WARN +knowledge' "$S/doctor2.out" | sed 's/^/    | /'; exit 1; }
grep -qE 'FAIL +knowledge' "$S/doctor2.out" && { echo "    an advisory rule produced a FAIL:"; grep -E 'FAIL +knowledge' "$S/doctor2.out" | sed 's/^/    | /'; exit 1; }
grep -q 'validator exists but no rule declares it' "$S/doctor2.out" && {
  echo "    mj_validate_knowledge_accumulation runs under no rule"; grep 'no rule declares' "$S/doctor2.out" | sed 's/^/    | /'; exit 1; }
[ "$rc" = 0 ] || { printf '    doctor exited %s on an advisory finding; advisory exits 0:\n' "$rc"; grep -E '^FAIL' "$S/doctor2.out" | sed 's/^/    | /'; exit 1; }
printf '    doctor over the cap:\n'; grep -E 'WARN +knowledge' "$S/doctor2.out" | sed 's/^/    | /'

# watch is not the queue's business: the rule is dispatched from doctor only, so a full
# review queue changes neither watch's findings nor its exit code
set +e; "$MJ" watch > "$S/watch2.out" 2>&1; WATCH2=$?; set -e
grep -qE 'DRIFT +knowledge .*(over|cap|candidates_max_files)' "$S/watch2.out" && {
  echo "    watch reports the review queue; the advisory rule is dispatched from doctor only"; exit 1; }
[ "$WATCH1" = "$WATCH2" ] || { printf '    a third candidate changed the exit of watch from %s to %s\n' "$WATCH1" "$WATCH2"; exit 1; }

# ---------------------------------------------------------------- older than the age
# A record dated in January, written by hand: no knowledge.derived line names its path and
# git has never seen it, so its review age falls back to its date — which is what the
# finding must say it used. The episode it names is real, so integrity has nothing to say.
SID="$(sed -n 's/^session_id: //p' .ai/repo/sessions/*.md 2>/dev/null | head -n 1)"
must "no session record to reference" [ -n "$SID" ]
cat > "$CAND/old-candidate.md" <<Y
---
schema: knowledge/v1
id: old-candidate
kind: knowledge
class: convention
title: "A candidate nobody reviewed since January"
description: "Hand-made to be older than the policy allows."
status: candidate
epistemics: decided
date: 2026-01-01
tags:
  - derived
  - decision
provenance:
  origin: extracted
  derived_from:
    - session:$SID
---

# A candidate nobody reviewed since January

Hand-made to be older than the policy allows.
Y
rc=0; doctor_run "$S/doctor3.out" || rc=$?
grep -qE 'WARN +knowledge .*old-candidate.*(older|age|candidate_max_age_minutes)' "$S/doctor3.out" || {
  echo "    doctor did not report the candidate older than knowledge.candidate_max_age_minutes:"; grep -E 'knowledge' "$S/doctor3.out" | sed 's/^/    | /'; exit 1; }
grep -qE 'WARN +knowledge .*old-candidate.*date' "$S/doctor3.out" || {
  echo "    the finding does not say which source it took the age from (the record's date):"; grep -E 'old-candidate' "$S/doctor3.out" | sed 's/^/    | /'; exit 1; }
[ "$rc" = 0 ] || { printf '    doctor exited %s on an advisory finding; advisory exits 0\n' "$rc"; grep -E '^FAIL' "$S/doctor3.out" | sed 's/^/    | /'; exit 1; }
printf '    doctor on an old candidate:\n'; grep -E 'WARN +knowledge .*old-candidate' "$S/doctor3.out" | sed 's/^/    | /'

# the untracked file is its own finding: a candidate committed nowhere reaches no surface
grep -qE 'WARN +knowledge .*old-candidate.*(untracked|committed nowhere|git add)' "$S/doctor3.out" || \
grep -qE 'WARN +knowledge .*(untracked|committed nowhere).*old-candidate' "$S/doctor3.out" || {
  echo "    doctor did not name the untracked candidate:"; grep -E 'knowledge' "$S/doctor3.out" | sed 's/^/    | /'; exit 1; }
rm -f "$CAND/old-candidate.md"

# ---------------------------------------------------------------- the number must be declared
# A cap that is not in the policy is not a cap of zero and not a cap of infinity; it is a
# missing declaration, reported at the rule's own class.
grep -v '^  candidates_max_files: ' .ai/repo/policy.yaml > "$S/policy" && cp "$S/policy" .ai/repo/policy.yaml
grep -q 'candidates_max_files' .ai/repo/policy.yaml && { echo "    the removal probe did not take"; exit 1; }
rc=0; doctor_run "$S/doctor4.out" || rc=$?
grep -qE 'WARN +knowledge .*candidates_max_files' "$S/doctor4.out" || {
  echo "    doctor did not report the missing knowledge.candidates_max_files:"; grep -E 'knowledge' "$S/doctor4.out" | sed 's/^/    | /'; exit 1; }
reset_policy
