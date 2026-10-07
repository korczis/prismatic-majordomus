# majordomus-covers: none
# claims: none
# The preflight follows what an issue serves, and answers serves, maintenance or refused.
#
# `intent preflight` is the one join a session or a transition consumes before work begins. It
# joined issue to milestone to intent and never read the issue's `serves`: work that served
# nothing, or another intent, was vouched for; with paths, one served issue vouched for every
# other; the critique was never read; and maintenance under a milestone no intent names was
# refused although `intent validate` accepts it. This case builds one intent and one
# maintenance milestone through a fresh `init` and asks the executable, one input at a time:
# maintenance answers 0 where validation is clean, an issue serving nothing under the intent's
# milestone is refused by name, a path two issues cover answers the worse of them, a plan
# never critiqued or with a blocking finding open refuses the work that serves it, and the
# answer carries the served intent and no other.
#
# It never skips: an executable is required, so a machine without one fails here instead of
# reporting a pass it never measured.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
pj_init
mkdir -p .ai/repo/project/intents .ai/repo/project/critiques test/cases
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
reviewed_by: case 893
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
commit() { git add -A >/dev/null && git commit -qm "$1"; }
preflight() { # <want-exit> <args...>: the answer as JSON in $T/p.json
  local want="$1" got=0; shift
  "$RB" intent preflight "$@" --format json > "$T/p.json" 2>"$T/p.err" || got=$?
  [ "$got" = "$want" ] || {
    printf '    preflight %s: expected exit %s, got %s\n' "$*" "$want" "$got"
    sed 's/^/    | /' "$T/p.json" "$T/p.err"; exit 1; }
}
holds() { # <label> <jq-predicate over $T/p.json>
  jq -e "$2" "$T/p.json" >/dev/null || { echo "    $1:"; sed 's/^/    | /' "$T/p.json"; exit 1; }
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

# --- maintenance under a milestone no intent names: validation accepts it, so does this
preflight 0 --issue I0002
holds "maintenance" '.verdict == "maintenance" and .refusals == [] and .intents == []'

# --- an issue serving a criterion of a critiqued intent: the served intent, and no other
preflight 0 --issue I0001
holds "served" '.verdict == "serves" and .matches[0].criteria == ["works"]'
holds "the intent it is held to" '.intents | length == 1 and .[0].id == "probe"
  and .[0].invariants == ["The probe stays a valid repository"] and .[0].non_goals == ["Speed"]
  and .[0].criteria[0].id == "works" and .[0].critique.open_blocking == []'

# --- served work beside maintenance is served; each issue is listed with its own verdict
preflight 0 --path src
holds "served beside maintenance" '.verdict == "serves"
  and ([.issues[] | "\(.issue):\(.verdict)"] == ["I0001:serves", "I0002:maintenance"])'

# --- under the intent's milestone, serving nothing: refused by name, and with paths one
#     served issue never vouches for it
issue I0003 probe src/probe/legacy 'serves: []'
commit "an issue that serves nothing"
preflight 10 --issue I0003
holds "serves nothing" '.verdict == "refused" and .refusals[0].cause == "issue_serves_nothing"
  and .refusals[0].issue == "I0003" and .intents == []'
preflight 10 --path src/probe/legacy/old.c
holds "the worst issue decides" '.verdict == "refused"
  and ([.issues[] | "\(.issue):\(.verdict)"] == ["I0001:serves", "I0003:refused"])
  and ([.refusals[].cause] == ["issue_serves_nothing"])'
expect_exit 10 "$RB" intent preflight --path src/probe/legacy/old.c
expect_grep '^refusal +issue_serves_nothing +I0003 '
rm .ai/repo/project/issues/I0003.yaml

# --- maintenance serving another intent's criterion serves it from outside its plan
issue I0002 ops src/ops 'serves:
  - probe#works'
commit "maintenance claims the intent"
preflight 10 --issue I0002
holds "another intent" '.refusals[0].cause == "serves_another_intent"'
issue I0002 ops src/ops 'serves: []'

# --- the plan of the intent served was never critiqued, then has a blocker open
rm .ai/repo/project/critiques/probe.yaml
commit "no critique"
preflight 10 --issue I0001
holds "not critiqued" '.refusals[0].cause == "intent_not_critiqued" and .intents[0].id == "probe"
  and (.intents[0] | has("critique") | not)'
critique open
commit "an open blocker"
preflight 10 --issue I0001
holds "open blocker" '.refusals[0].cause == "open_blocking_finding"
  and .intents[0].critique.open_blocking[0].id == "thin"'
critique planned
commit "the blocker is planned"
preflight 0 --issue I0001
holds "planned blocker" '.verdict == "serves"'
