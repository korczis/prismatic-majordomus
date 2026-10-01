# majordomus-covers: none
# The gate of project.interfaces-are-projections, driven against fixture trees.
#
# Every interface of the executable but one is built by walking the capability registry.
# The command line is the exception — declared a second time, in clap — so a command can
# exist there that no capability claims. Some of those are legitimate and always will be:
# `serve` starts a process, `generate` writes files. What the rule refuses is a command that
# is neither claimed nor classified: an operation that left the API by accident rather than
# by decision.
#
# `scripts/ci/projection-check` is the decision over that set, and it was named by no case
# in the suite. The risk in it is entirely in the branching: four outcomes, two of them
# non-zero for different reasons, and one exit (12) that must never be confused with a
# verdict about the tree. On this checkout only one of those four branches is ever taken, so
# running the gate here proves one quarter of it at best.
#
# The subject under test is therefore the gate, not the executable: each fixture carries a
# stub that answers `capabilities projections` with a written inventory, which is how every
# branch is reached. What the real executable answers is 76_capabilities_projections.sh's
# question, and this case is deliberately not a second copy of it.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/projection-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }

# A tree whose executable answers whatever the fixture wrote. The stub honours
# MJ_STUB_FAIL, so a case can also make the answer unavailable rather than wrong.
fixture() {   # fixture <n> -> prints the tree
  F="$T/tree$1"
  rm -rf "$F"; mkdir -p "$F/bin"
  cat > "$F/bin/majordomus-cli" <<'STUB'
#!/usr/bin/env bash
[ "${MJ_STUB_FAIL:-0}" = 1 ] && exit 3
cat "$(dirname "$0")/../projections.json"
STUB
  chmod +x "$F/bin/majordomus-cli"
  printf '%s' "$F"
}
answer() { cat > "$1/projections.json"; }   # answer <tree> <<'J' ... J

# ---------------------------------------------------------------- the tree it accepts
# Three commands no capability claims, every one of them classified with a reason beside the
# command line. That is the shape the rule asks for: not "no hand-written commands", but
# "no hand-written command that says nothing".
F="$(fixture 0)"
answer "$F" <<'J'
{ "rows": [ { "id": "cap.plan", "closed": true } ],
  "unbacked": ["serve", "mcp", "generate"],
  "unclassified": [] }
J
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep '3 command\(s\) claimed by no capability, every one classified in cli::LOCAL'

# ---------------------------------------------------------------- 1. a command that says nothing
# The violation the gate exists for: a command that is claimed by no capability and appears
# in no classification. It is named, and the message says both remedies — because the
# finding is a question ("should this be a capability, or is it local?") and a gate that
# reports the question without the two answers sends people to read the source.
F="$(fixture 1)"
answer "$F" <<'J'
{ "rows": [ { "id": "cap.plan", "closed": true } ],
  "unbacked": ["serve", "mcp", "generate", "sniff"],
  "unclassified": ["sniff"] }
J
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep '1 command\(s\) are claimed by no capability and say why nowhere'
expect_grep '\+ sniff'
expect_grep 'rule project.interfaces-are-projections@1'
expect_grep 'give the capability a CliExposure, or declare the command in'
expect_grep 'apps/majordomus-cli/src/cli/local.rs with the reason it is local'

# restored, the first way: the command becomes a capability and leaves the unbacked set
F="$(fixture 2)"
answer "$F" <<'J'
{ "rows": [ { "id": "cap.plan", "closed": true }, { "id": "cap.sniff", "closed": true } ],
  "unbacked": ["serve", "mcp", "generate"],
  "unclassified": [] }
J
expect_exit 0 env MJ_ROOT="$F" "$GATE"

# restored, the second way: the command stays local and is classified with its reason. Both
# are correct outcomes, and a gate that accepted only the first would be a gate that refuses
# `serve` for ever.
F="$(fixture 3)"
answer "$F" <<'J'
{ "rows": [ { "id": "cap.plan", "closed": true } ],
  "unbacked": ["serve", "mcp", "generate", "sniff"],
  "unclassified": [] }
J
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep '4 command\(s\) claimed by no capability, every one classified in cli::LOCAL'

# several at once are all named: a gate that stopped at the first would make a landing that
# adds three commands take three runs to clear
F="$(fixture 4)"
answer "$F" <<'J'
{ "rows": [],
  "unbacked": ["sniff", "poke", "prod"],
  "unclassified": ["sniff", "poke", "prod"] }
J
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep '3 command\(s\) are claimed by no capability and say why nowhere'
expect_grep '\+ sniff'
expect_grep '\+ poke'
expect_grep '\+ prod'

# ---------------------------------------------------------------- 2. the other direction
# A capability that claims a command line clap does not answer is a broken tree, not debt,
# and the gate refuses to measure debt on one: it says which capability and stops. Measuring
# the classification of a tree whose declarations already disagree would report a number
# about a state nobody should be in.
F="$(fixture 5)"
answer "$F" <<'J'
{ "rows": [ { "id": "cap.plan", "closed": true }, { "id": "cap.ghost", "closed": false } ],
  "unbacked": ["sniff"],
  "unclassified": ["sniff"] }
J
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'a capability claims a command line that does not exist: cap.ghost'
expect_grep 'reproduce: majordomus capabilities projections --unmet'
# and it stopped there: the unclassified command was not also reported
expect_no_grep 'say why nowhere'

# ---------------------------------------------------------------- 3. --strict
# The sweep the classification deliberately is not: under --strict any command no capability
# claims is a finding, classified or not. It must refuse a tree the default mode accepts, or
# the two modes are one mode with two names.
F="$(fixture 6)"
answer "$F" <<'J'
{ "rows": [], "unbacked": ["serve", "mcp"], "unclassified": [] }
J
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_exit 10 env MJ_ROOT="$F" "$GATE" --strict
expect_grep '2 command\(s\) are claimed by no capability \(--strict\)'
expect_grep '  mcp'
expect_grep '  serve'

F="$(fixture 7)"
answer "$F" <<'J'
{ "rows": [], "unbacked": [], "unclassified": [] }
J
expect_exit 0 env MJ_ROOT="$F" "$GATE" --strict

# ---------------------------------------------------------------- an unusable tree
# Three ways the gate can fail to reach its subject, each of which must be 12 and not a
# verdict. A check that reports 0 from no information is worse than no check: it is a green
# tick standing where a measurement was supposed to be.
F="$(fixture 8)"
answer "$F" <<'J'
{ "rows": [], "unbacked": {} }
J
expect_exit 12 env MJ_ROOT="$F" "$GATE"
expect_grep 'capabilities projections did not answer the expected shape'

F="$(fixture 9)"
answer "$F" <<'J'
{ "rows": [], "unbacked": [], "unclassified": [] }
J
expect_exit 12 env MJ_ROOT="$F" MJ_STUB_FAIL=1 "$GATE"
expect_grep 'the executable could not answer capabilities projections'

F="$(fixture 10)"
rm -f "$F/bin/majordomus-cli"
expect_exit 12 env MJ_ROOT="$F" "$GATE"
expect_grep 'bin/majordomus-cli is not executable'

# and an option it does not know is a usage error about the invocation, not about the tree
F="$(fixture 11)"
answer "$F" <<'J'
{ "rows": [], "unbacked": [], "unclassified": [] }
J
expect_exit 2 env MJ_ROOT="$F" "$GATE" --loose
expect_grep 'unknown option --loose'

echo "    a command is claimed by a capability or classified as local; anything else is refused"
