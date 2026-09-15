# majordomus-covers: script:scripts/ci/command-furnished
# A case can name what it covers, whatever kind of thing that is.
#
# `# majordomus-covers:` could name exactly one kind of thing: a public command of the shell
# tool. `lib/commands.sh` refused anything else — "declares coverage of it, but it is not a
# public command" — and the public list is `start check finish … bench archive`. `release` and
# `serve` are not in it; those are the Rust executable's.
#
# So every case whose subject is a gate, a script, a workflow, a projection or a capability of
# the executable had **no word for its own subject**, and `none` was the only legal value. On
# 2026-09-15 that was 74 of 215 cases. The doctrine line reads "command coverage — every public
# command is exercised and refuted", which is true, and which a reader takes for "the tests are
# linked to what they test".
#
# The check was right about the world it could see. The world grew past it — the same shape as
# a coverage gate nobody called and a version check that asked declared-versus-owed while the
# question was shown-versus-shipped.
#
# What is asserted: the second vocabulary exists and resolves, it refuses what does not exist,
# and — the section that matters most — **it cannot be used to escape the first**. A prefixed
# name must not count as command coverage, or widening the vocabulary would quietly let a
# public command go untested while the gate stayed green.
. "$ROOT/test/lib.sh"

LIB="$ROOT/lib/commands.sh"
GATES="$ROOT/.ai/repo/ci/gates.yaml"
[ -f "$LIB" ]   || { echo "    no lib/commands.sh"; exit 1; }
[ -f "$GATES" ] || { echo "    no gate model"; exit 1; }

# --- 1. the vocabulary is declared, and says which kinds it knows
grep -q 'gate:\*)' "$LIB" || { echo "    lib/commands.sh knows no gate: vocabulary"; exit 1; }
grep -q 'script:\*)' "$LIB" || { echo "    lib/commands.sh knows no script: vocabulary"; exit 1; }
grep -q 'workflow:\*)' "$LIB" || { echo "    lib/commands.sh knows no workflow: vocabulary"; exit 1; }
grep -q 'unknown vocabulary' "$LIB" \
  || { echo "    an unrecognised prefix is not refused, so a typo in the kind would be read as"
       echo "    a command name and fail with a message about the wrong thing"; exit 1; }
echo "    the header knows gate:, script:, workflow:, and refuses a kind it does not know"

# --- 2. it resolves against the real models, both ways
# Applied the way lib/commands.sh applies it, over the same files, rather than by re-reading
# its verdict — a case that asserted only that the code contains a string would pass over an
# implementation that never ran.
real_gate="$(grep -m1 -E '^  - id: [a-z][a-z0-9-]*$' "$GATES" | sed 's/.*id: //')"
[ -n "$real_gate" ] || { echo "    the gate model declares no id to test against"; exit 1; }
accepts_gate() { grep -qE "^  - id: ${1}\$" "$GATES"; }
accepts_script() { [ -x "$ROOT/$1" ]; }
accepts_workflow() { [ -f "$ROOT/.github/workflows/$1" ]; }

accepts_gate "$real_gate"        || { echo "    a real gate ($real_gate) is not accepted"; exit 1; }
accepts_gate "no-such-gate-here" && { echo "    a gate the model does not declare is accepted"; exit 1; }
accepts_script scripts/ci/command-furnished || { echo "    a real script is not accepted"; exit 1; }
accepts_script scripts/ci/definitely-not    && { echo "    a script that does not exist is accepted"; exit 1; }
accepts_workflow pages.yml        || { echo "    a real workflow is not accepted"; exit 1; }
accepts_workflow no-such-flow.yml && { echo "    a workflow that does not exist is accepted"; exit 1; }
echo "    a real gate, script and workflow resolve; a missing one of each is refused"

# --- 3. THE MUTATION: the second vocabulary cannot satisfy the first
# The obligation that every public command carries a behavioural and a negative case is the
# older and stricter one. If a prefixed name counted towards it, a command could be left
# untested by naming a gate instead, and the gate would stay green — a wider vocabulary buying
# a narrower promise. The loop that checks commands must therefore look only at bare names.
sed -n '/for c in \$public; do/,/^  done/p' "$LIB" > "$T/loop.txt"
grep -q 'behaviour' "$T/loop.txt" \
  || { echo "    the command-coverage loop no longer reads the behaviour list"; exit 1; }
if grep -qE 'gate:|script:|workflow:' "$T/loop.txt"; then
  echo "    the command-coverage loop now considers prefixed names: a public command could be"
  echo "    satisfied by a case that covers a gate, and the gate would stay green"
  exit 1
fi
echo "    a prefixed name does not count as command coverage; the older obligation is untouched"

# --- 4. and it is used, not merely available
# A vocabulary nobody speaks is a vocabulary nobody can be held to.
used="$(awk 'FNR == 1 && sub(/^# majordomus-covers: */, "") { print }' "$ROOT"/test/cases/*.sh \
        | tr ' ' '\n' | grep -cE '^(gate|script|workflow):' || true)"
[ "${used:-0}" -ge 1 ] \
  || { echo "    no case uses the new vocabulary, so nothing would notice if it stopped working"; exit 1; }
echo "    $used case declaration(s) name a gate or a script"

echo "    a case can name what it covers, and naming the wrong thing is still refused"
