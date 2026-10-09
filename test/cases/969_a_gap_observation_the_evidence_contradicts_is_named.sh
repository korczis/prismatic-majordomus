# majordomus-covers: none
# claims: intent-remains-derived
# A gap observation the evidence now contradicts is named, and nothing is written.
#
# A gap records what a worker observed when the plan was made. A criterion it marked
# satisfied with an observation is served by no issue: coverage counts it `observed`, owes no
# work for it, and `criterion_uncovered` stays silent. When that criterion's evidence later
# fails, nothing used to say the plan rests on an observation that no longer holds (ADR 0117).
#
# This case declares one intent with two criteria through a fresh `init`: `a`, served by an
# issue, and `b`, which the gap observed satisfied. Then:
#
#   b's case was never run          no warning: an unconfirmed observation is not contradicted
#   b's case fails, recorded        intent validate warns gap_observation_contradicted on
#                                   probe#b, with the same exit and failure count as before;
#                                   intent oppose shows it as an advisory finding; the
#                                   realization answers failed (gap_observation) and plan_work
#
# and that none of it writes: every answer leaves the working tree byte-identical.
#
# It never skips: an executable is required.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
W="$(mktemp -d "${TMPDIR:-/tmp}/mj969.XXXXXX")"; trap 'rm -rf "$W"' EXIT

same() { [ "$2" = "$3" ] || { printf '    %s: expected %s, got %s\n' "$1" "$2" "$3"; exit 1; }; }
commit() { git add -A >/dev/null && git commit -qm "$1" >/dev/null; }

"$MJ" init >/dev/null; "$MJ" update >/dev/null

# ---------------------------------------------------------------- declare
pj_init
mkdir -p .ai/repo/project/intents .ai/repo/project/gaps test/cases lib
cat > .ai/repo/project/milestones/probe.yaml <<'Y'
id: probe
title: The probe reaches its outcome
slug: probe
order: 0
priority: p1
problem: "A behaviour is missing."
outcome: "The behaviour exists and is proven."
acceptance_criteria:
  - The behaviour exists
validation:
  - "true"
evidence_required:
  - reached
Y
cat > .ai/repo/project/issues/I0001.yaml <<'Y'
id: I0001
milestone: probe
title: Behaviour a
slug: behaviour-a
priority: p1
profile: implementation
objective: "Make behaviour a exist."
scope:
  - lib/a
serves:
  - probe#a
acceptance_criteria:
  - The behaviour exists
validation:
  - "true"
evidence_required:
  - proof
Y
cat > .ai/repo/project/intents/probe.yaml <<'Y'
id: probe
title: Behaviours a and b are true for the people who rely on them
statement: "Behaviour a is built, and behaviour b, which already holds, keeps holding."
invariants:
  - Nothing about what remains is written anywhere
milestones:
  - probe
satisfaction:
  - id: a
    criterion: Behaviour a exists
    evidence: test
    ref: test/cases/01_a.sh
  - id: b
    criterion: Behaviour b exists
    evidence: test
    ref: test/cases/02_b.sh
Y
cat > .ai/repo/project/gaps/probe.yaml <<'Y'
intent: probe
observed_at: 0000000
recorded_by: case 969
recorded_at: 2026-10-09
observations:
  - id: b-is-there
    statement: lib/b says ok
    source: file:lib/b
conditions:
  - criterion: a
    state: missing
    because: nothing builds a yet
  - criterion: b
    state: satisfied
    because: lib/b already says ok
    observations: [b-is-there]
work:
  - id: build-a
    title: Build behaviour a
    serves: [a]
    issue: I0001
Y
printf 'grep -qx ok lib/a\n' > test/cases/01_a.sh
printf 'grep -qx ok lib/b\n' > test/cases/02_b.sh
echo ok > lib/b
commit "declare the intent"
sed -i.bak "s/^observed_at: 0000000$/observed_at: $(git rev-parse --short=10 HEAD)/" .ai/repo/project/gaps/probe.yaml
rm -f .ai/repo/project/gaps/probe.yaml.bak
commit "the gap names the commit it observed"

# run a case the way the runner does and record its report with the real recorder
run() { # <case name>
  local word=ok
  bash "test/cases/$1.sh" || word=FAIL
  printf '%s\t%s\t0\tserial\n' "$1" "$word" > "$W/report.tsv"
  "$RB" evidence record --suite "$W/report.tsv" >/dev/null
  commit "record $1: $word"
}
# the exit of intent validate and its failure count, on one line
judged() {
  local code=0
  "$RB" intent validate > "$W/validate.out" 2>/dev/null || code=$?
  printf '%s %s\n' "$code" "$(sed -n 's/.* \([0-9]*\) failure(s).*/\1/p' "$W/validate.out")"
}
remains_of_b() {
  "$RB" intent realization --intent probe --format json \
    | jq -r '.intents[0].unmet[] | select(.id == "b") | "\(.remains) \(.basis) \(.next.action)"'
}

# ---------------------------------------------------------------- an observation nobody ran
same "coverage takes the observation as served" "observed" \
  "$("$RB" intent coverage --format json | jq -r '.criteria[] | select(.criterion == "b") | .strength')"
before="$(judged)"
grep -q gap_observation_contradicted "$W/validate.out" \
  && { echo "    an observation nobody ran was called contradicted"; exit 1; }
same "an unconfirmed observation" "needs_evidence gap_observation record_evidence" "$(remains_of_b)"

# ---------------------------------------------------------------- the evidence contradicts it
echo broken > lib/b
commit "behaviour b breaks"
run 02_b
status_before="$(git status --porcelain)"; head_before="$(git rev-parse HEAD)"
after="$(judged)"
grep -q '^WARN gap_observation_contradicted  probe#b: ' "$W/validate.out" \
  || { echo "    intent validate did not name the contradicted observation:"; cat "$W/validate.out"; exit 1; }
same "the warning changes neither the exit nor the failure count" "$before" "$after"
same "what remains of b" "failed gap_observation plan_work" "$(remains_of_b)"
"$RB" intent realization --intent probe --format json \
  | jq -e '.intents[0].unmet[] | select(.id == "b") | .next.serves == "probe#b"' >/dev/null \
  || { echo "    the next action does not name the criterion to plan for"; exit 1; }
"$RB" intent oppose probe > "$W/oppose.out" 2>/dev/null || true
grep -q 'advisory  gap_observation_contradicted  probe#b' "$W/oppose.out" \
  || { echo "    intent oppose does not show the warning as advisory:"; cat "$W/oppose.out"; exit 1; }

# ---------------------------------------------------------------- nothing was written
same "the working tree" "$status_before" "$(git status --porcelain)"
same "the head" "$head_before" "$(git rev-parse HEAD)"
git diff --quiet HEAD -- .ai/repo/project \
  || { echo "    a record changed: something was written"; git diff --stat HEAD; exit 1; }
