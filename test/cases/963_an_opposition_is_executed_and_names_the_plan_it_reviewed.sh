# majordomus-covers: none
# claims: none
# An opposition is executed by the tool, and its stamp names the plan it reviewed.
#
# A critique was a file somebody wrote, and all that was asked of it was that it exist with
# no blocking finding open (ADR 0112). This case builds one intent through a fresh `init`
# and runs the review as something the executable does:
#
#   the brief      `intent oppose` answers the plan a reviewer reads, what the derivations
#                  find about it now, what was recorded, one disposition, and where the
#                  critique's stamp stands — with no advisor configured
#   the stamp      `intent stamp` writes three lines: the plan revision the executable
#                  derived, the commit, the tool. Every line of the findings is byte for byte
#                  what the reviewer wrote. No disposition and no structural finding is in
#                  the record afterwards
#   the ledger     `opposition.recorded` is appended, an event the vocabulary declares
#   the refusals   a critique whose own findings do not hold is not stamped and nothing is
#                  written; a caller cannot choose the revision
#   the writer     recording is a declared writing capability and reading is not
#
# It never skips: an executable is required, so a machine without one fails here instead of
# reporting a pass it never measured.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN

"$MJ" init >/dev/null
pj_init
mkdir -p .ai/repo/project/intents .ai/repo/project/critiques test/cases src/probe src/ops src/loose
printf '. "$ROOT/test/lib.sh"\ntrue\n' > test/cases/01_probe.sh
milestone() { # <id>
  cat > ".ai/repo/project/milestones/$1.yaml" <<YAML
id: $1
title: Milestone $1
slug: $1
order: 0
priority: p1
problem: "A problem."
outcome: "An outcome."
acceptance_criteria:
  - It is reached
validation:
  - "true"
evidence_required: []
YAML
}
issue() { # <id> <milestone> <scope> <serves-block>
  cat > ".ai/repo/project/issues/$1.yaml" <<YAML
id: $1
milestone: $2
title: Issue $1
slug: issue-$1
priority: p1
profile: implementation
objective: "Do the work of $1."
scope:
  - $3
$4
acceptance_criteria:
  - It is done
validation:
  - "true"
evidence_required:
  - done
YAML
}
critique() { # <resolution-state>
  cat > .ai/repo/project/critiques/probe.yaml <<YAML
intent: probe
reviewed_at: HEAD
reviewed_by: case 963
findings:
  - id: thin
    class: insufficient_work
    subject: probe#works
    finding: One case may not be enough
    blocking: true
    resolution:
      state: $1
      issue: I0001
YAML
}
policy_intent() { # <binding value, or "absent">: replace the policy's intent block
  # the block init wrote from the skeleton, or the one this function wrote, goes first: a
  # top-level `intent:` and the indented lines under it
  awk '/^intent:[[:space:]]*$/ { skip=1; next } skip && /^[^[:space:]#]/ { skip=0 } !skip { print }' \
    .ai/repo/policy.yaml > "$T/policy" && cat "$T/policy" > .ai/repo/policy.yaml
  [ "$1" = absent ] || cat >> .ai/repo/policy.yaml <<YAML
intent:
  binding: $1
  exemptions:
    - id: emergency
      description: Restoring a broken trunk
YAML
}
commit() { git add -A >/dev/null && git commit -qm "$1"; }
cur() { sed -n "s/^$1: //p" .ai/local/state/current.yaml; }
end_task() { "$MJ" handover --derive --close >/dev/null 2>&1 || true; }
started() { # the last task.started event, as JSON
  grep '"event":"task.started"' .ai/local/state/ledger.jsonl | tail -n 1
}

milestone probe
milestone ops
cat > .ai/repo/project/intents/probe.yaml <<'YAML'
id: probe
title: The probe's outcome is true
statement: "The outcome is true for the people it is for."
invariants:
  - The probe stays a valid repository
non_goals:
  - Speed
milestones:
  - probe
satisfaction:
  - id: works
    criterion: The probe works
    evidence: test
    ref: test/cases/01_probe.sh
YAML
issue I0001 probe src/probe 'serves:
  - probe#works'
issue I0002 ops src/ops 'serves: []'
critique planned
commit "one intent, one maintenance milestone"
expect_exit 0 "$RB" intent validate


same() { [ "$2" = "$3" ] || { echo "    $1: expected $2, got $3"; exit 1; }; }
opp() { "$RB" intent oppose probe --format json 2>/dev/null; }
field() { opp | jq -r "$1"; }
stamp() { "$RB" intent stamp probe "$@" --format json > "$T/stamp.json" 2>"$T/stamp.err"; }
C=.ai/repo/project/critiques/probe.yaml

policy_intent absent; commit "no intent policy" 2>/dev/null || true
before="$(sed -n '/^findings:/,$p' "$C")"

# --- the brief: derived, with nothing recorded about which plan was reviewed
same "an unstamped critique" not_stamped "$(field .review.state)"
same "a planned blocking finding" accept_with_required_changes "$(field .disposition)"
same "the reviewer's finding is carried" thin "$(field '.recorded[0].id')"
same "the serving issue is in the brief" I0001 "$(field '.issues[0].id')"
same "the invariant is in the brief" "The probe stays a valid repository" "$(field '.invariants[0]')"
rev="$(field .reviewed_plan)"
[ "${#rev}" = 64 ] || { echo "    no plan revision: $rev"; exit 1; }
opp | jq -e '[.structural[].id] | index("plan_not_critiqued") == null' >/dev/null \
  || { echo "    the review lists its own absence among its findings"; exit 1; }

# --- a check writes nothing
stamp --check || { echo "    the check failed:"; cat "$T/stamp.err"; exit 1; }
jq -e --arg r "$rev" '.written == false and .created == false and .reviewed_revision == $r' "$T/stamp.json" >/dev/null \
  || { echo "    the check answered:"; cat "$T/stamp.json"; exit 1; }
expect_no_grep '^reviewed_revision:' "$C"

# --- the stamp: three lines, and the findings untouched
stamp || { echo "    the stamp failed:"; cat "$T/stamp.err"; exit 1; }
jq -e '.written == true and .event == "opposition.recorded" and .disposition == "accept_with_required_changes"' "$T/stamp.json" >/dev/null \
  || { echo "    the stamp answered:"; cat "$T/stamp.json"; exit 1; }
expect_grep "^reviewed_revision: $rev\$" "$C"
expect_grep '^reviewed_with: "majordomus-cli ' "$C"
# quoted: a commit's first ten characters can be all digits, and must still read as text
expect_grep "^reviewed_at: \"$(git rev-parse HEAD | cut -c1-10)\"\$" "$C"
same "the findings after the stamp" "$before" "$(sed -n '/^findings:/,$p' "$C")"
# nothing derived was stored
expect_no_grep '^(disposition|structural|state):' "$C"
commit "the review is stamped"
same "a stamped critique" current "$(field .review.state)"
expect_exit 0 "$RB" intent validate
expect_no_grep 'critique_not_stamped|critique_stale'
# the shell flattener reads the stamped record as the Rust reader does
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
expect_no_grep 'critiques/probe.yaml.*(does not parse|unknown key)' "$T/doctor.out"

# --- the ledger carries the run, as an event the vocabulary declares
grep '"event":"opposition.recorded"' .ai/local/state/ledger.jsonl | tail -n 1 > "$T/event.json"
jq -e --arg r "$rev" '.intent == "probe" and .reviewed_revision == $r and .disposition == "accept_with_required_changes"' "$T/event.json" >/dev/null \
  || { echo "    the ledger does not carry the review:"; cat "$T/event.json"; exit 1; }
expect_grep '^  - id: opposition\.recorded$' "$ROOT/share/events.yaml"

# --- a caller cannot choose the revision
if "$RB" run intent_opposition.record --input '{"intent":"probe","reviewed_revision":"0000"}' --format json > "$T/forged.out" 2>&1; then
  echo "    a caller supplied the revision and was not refused:"; cat "$T/forged.out"; exit 1
fi
expect_grep "reviewed_revision" "$T/forged.out"
expect_no_grep '^reviewed_revision: 0000' "$C"
# and a critique whose own findings do not hold is not stamped
cp "$C" "$T/keep.yaml"
sed 's/class: insufficient_work/class: a_feeling/' "$T/keep.yaml" > "$C"; commit "an unknown class"
if stamp; then echo "    a critique with an unknown class was stamped"; exit 1; fi
# with or without a reviewer named, the record that does not validate is neither stamped nor
# replaced by an empty one: what a reviewer wrote is never overwritten to certify silence
if stamp --by "somebody else"; then echo "    an unreadable critique was replaced by an empty one"; exit 1; fi
expect_grep 'critique_unknown_class|is not a critique this repository can read' "$T/stamp.err"
expect_grep 'class: a_feeling' "$C"
same "a refused stamp changed no record" "" "$(git status --porcelain -- .ai/repo)"
cp "$T/keep.yaml" "$C"; commit "the critique as it was"

# --- the stamp is a declared writer, and reading the opposition is not: every surface asks
#     before the first runs (the whole set is pinned by the_capabilities_that_write_the_repository_are_these)
"$RB" capabilities describe intent_opposition.record --format json > "$T/writer.json" 2>/dev/null
expect_grep 'repository_mutation' "$T/writer.json"
"$RB" capabilities describe intent_opposition.review --format json > "$T/reader.json" 2>/dev/null
expect_no_grep 'repository_mutation' "$T/reader.json"
