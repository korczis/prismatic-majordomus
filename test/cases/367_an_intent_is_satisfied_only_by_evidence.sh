# majordomus-covers: none
# claims: intent-links-resolve, intent-stage-derived
# An intent is satisfied only by evidence, and never by the work around it being finished.
#
# An intent names the milestones that realise it and the tests that settle it. Its stage is
# derived from the plan and its satisfaction from the evidence ledger, and nothing stores
# either (ADR 0070). So this case moves each input on its own and watches the derived stage:
# a recorded passing run meets the criterion but satisfies nothing while the milestone is
# open; closing the milestone satisfies it; changing the test after its run takes the
# satisfaction away again, with no file about the intent touched. Then it breaks a link and
# asserts the refusal names it.
#
# Beside the stage it reads the verdict, which the criteria alone derive (ADR 0107): the
# recorded run makes it `satisfied` while the stage still reads `planned`, and the realization
# reports that disagreement as `evidence_ahead_of_plan` drift without changing the stage.
#
# It never skips: an executable is required, so a machine without one fails here instead of
# reporting a pass it never measured.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

# --- a repository with the layer: `init` seeds the intent class, which is half of what an
#     initialised repository needs to read the kind at all
"$MJ" init >/dev/null
grep -q "kind: intent" .ai/repo/knowledge/sources.yaml \
  || { echo "    init did not seed the intent source class"; exit 1; }

mkdir -p .ai/repo/project/milestones .ai/repo/project/intents test/cases
milestone() { # <evidence-block>
  cat > .ai/repo/project/milestones/probe.yaml <<YAML
id: probe
title: The probe reaches its outcome
slug: probe
order: 0
priority: p1
problem: "A problem."
outcome: "An outcome."
acceptance_criteria:
  - It is reached
validation:
  - "true"
evidence_required:
  - reached
$1
YAML
}
milestone ""
printf '. "$ROOT/test/lib.sh"\ntrue\n' > test/cases/01_probe.sh
cat > .ai/repo/project/intents/probe.yaml <<'YAML'
id: probe
title: The probe's outcome is true
statement: "The outcome is true for the people it is for."
invariants:
  - The probe stays a valid repository
milestones:
  - probe
satisfaction:
  - id: probe-passes
    criterion: The probe's case passes
    evidence: test
    ref: test/cases/01_probe.sh
YAML
git add -A >/dev/null && git commit -qm install

stage() { "$RB" intent show probe --format json | jq -r '.stage'; }
state() { "$RB" intent show probe --format json | jq -r '.satisfaction[0].state'; }
verdict() { "$RB" intent show probe --format json | jq -r '.verdict.state'; }
same() { [ "$2" = "$3" ] || { printf '    %s: expected %s, got %s\n' "$1" "$2" "$3"; exit 1; }; }

expect_exit 0 "$RB" intent validate
expect_grep '1 intent\(s\), 0 failure'
same "nothing recorded" "not_run" "$(state)"
same "an open milestone and no evidence" "planned" "$(stage)"
same "the verdict of a test with no recorded run" "unsatisfied" "$(verdict)"
same "the criterion holding the verdict back" "probe-passes not_run" \
  "$("$RB" intent show probe --format json | jq -r '.verdict.reasons[] | "\(.criterion) \(.state)"')"

# --- a passing run of the case that is in the tree, recorded
record() { # <outcome>
  local digest commit
  digest="sha256:$(sha256_of_file test/cases/01_probe.sh)"
  commit="$(git rev-parse HEAD)"
  mkdir -p .ai/repo/evidence
  jq -n --arg d "$digest" --arg c "$commit" --arg o "$1" '{version: 1, executions: [{
      test: "suite:01_probe", runner: "suite", source: "test/cases/01_probe.sh",
      outcome: $o, seconds: 1, commit: $c, working_tree: "clean", digest: $d,
      at: "2026-09-15T00:00:00Z", origin: "local", command: "bash test/run.sh 01_probe"}]}' \
    > .ai/repo/evidence/ledger.json
  git add -A >/dev/null && git commit -qm "record $1"
}
record pass
same "a recorded passing run" "current" "$(state)"
same "evidence while the milestone is open" "planned" "$(stage)"
same "the verdict while the milestone is open" "satisfied" "$(verdict)"
# the disagreement is drift, reported as a warning: the realization still exits 0
expect_exit 0 "$RB" intent realization --intent probe
expect_grep 'WARN evidence_ahead_of_plan  probe: .*stage is planned \(probe is '

# --- the milestone's own evidence closes it; now, and only now, the intent is satisfied
milestone 'evidence:
  - covers: reached
    type: command
    command: "true"'
git add -A >/dev/null && git commit -qm "close the milestone"
same "a closed milestone and current evidence" "satisfied" "$(stage)"
same "the verdict once the milestone closes" "satisfied" "$(verdict)"
expect_exit 0 "$RB" intent realization --intent probe
expect_no_grep 'evidence_ahead_of_plan|closed_work_not_satisfied'

# --- the MCP tool answers the same record the command shows
# intents.record is projected as the MCP tool majordomus_intent_record, and a tool nothing
# sends a tools/call is indistinguishable from one that does not work
# (scripts/ci/mcp-tool-run-check). One standalone session — no port, no lease, nothing
# written — asks it for the probe and for an id the repository does not hold: the first
# answer must be the record `intent show` derives, stage and criterion states alike; the
# second a refusal, not an empty record. Its files stay outside the fixture repository.
M="$(mktemp -d "${TMPDIR:-/tmp}/mj-367-mcp.XXXXXX")"; trap 'rm -rf "$M"' EXIT
mcp_intent() { # <id>
  { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case367","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_intent_record","arguments":{"id":"%s"}}}\n' "$1"
  } | "$RB" mcp --standalone --repo "$PWD" --share "$ROOT/share" 2>"$M/mcp.err" \
    | jq -c 'select(.id == 2)' 2>/dev/null || true
}
mcp_intent probe > "$M/mcp.json"
jq -e '.result.isError != true and (.result.structuredContent | type) == "object"' \
  "$M/mcp.json" >/dev/null 2>&1 || {
  echo "    majordomus_intent_record returned no typed answer over MCP:"
  head -c 600 "$M/mcp.json" | sed 's/^/      /'; echo
  sed 's/^/      | /' "$M/mcp.err" | head -5; exit 1; }
"$RB" intent show probe --format json > "$M/cli.json"
view='{id, stage, verdict, criteria: [.satisfaction[] | {id, state}]}'
jq -e --slurpfile cli "$M/cli.json" \
  "(.result.structuredContent | $view) == (\$cli[0] | $view) and .result.structuredContent.stage == \"satisfied\"" \
  "$M/mcp.json" >/dev/null || {
  echo "    majordomus_intent_record over MCP and \`intent show\` disagree:"
  echo "      mcp: $(jq -c ".result.structuredContent | $view" "$M/mcp.json" | head -c 400)"
  echo "      cli: $(jq -c "$view" "$M/cli.json" | head -c 400)"; exit 1; }
mcp_intent absent > "$M/mcp-absent.json"
jq -e '.result.isError == true or .error != null' "$M/mcp-absent.json" >/dev/null || {
  echo "    majordomus_intent_record answered an intent the repository does not hold:"
  head -c 400 "$M/mcp-absent.json" | sed 's/^/      /'; echo; exit 1; }
echo "    majordomus_intent_record over MCP answers the record intent show derives, and refuses an absent id"

# --- the test changes after its run: the run proves nothing about the test that is there
printf '. "$ROOT/test/lib.sh"\nfalse\n' > test/cases/01_probe.sh
git add -A >/dev/null && git commit -qm "change the probe"
same "a run of a test that has since changed" "stale" "$(state)"
same "stale evidence" "verifying" "$(stage)"
same "the verdict over stale evidence" "unsatisfied" "$(verdict)"

# --- a failing run is not evidence either
record fail
same "a failing run" "failing" "$(state)"

# --- a link that resolves to nothing is refused, named, with a non-zero exit
sed 's#test/cases/01_probe.sh#test/cases/404_absent.sh#' .ai/repo/project/intents/probe.yaml > "$T/probe.yaml"
cp "$T/probe.yaml" .ai/repo/project/intents/probe.yaml
git add -A >/dev/null && git commit -qm "break the reference"
expect_exit 10 "$RB" intent validate
expect_grep 'FAIL unresolved_evidence_ref  probe'
expect_grep 'invalid'
same "an unresolved reference" "unresolved" "$(state)"
