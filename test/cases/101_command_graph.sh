# The command graph and every projection derived from it, against the real executable in a
# disposable repository: the graph composes the three declarations, the workflow bridge it
# writes forwards arguments rather than splicing them, materialising twice writes once,
# the completion of both surfaces is answered from the one graph, and a hand-written bridge
# is rejected by the gate that exists to reject it.
#
# The invariant the last section proves is the one this architecture is for: a command whose
# only declaration is the clap tree reaches the workflow projection and the shell completion
# with no other file edited. The gate's own probe is a mutation the case makes and then
# asserts took effect before asking the gate, so a probe that stops mutating fails here
# rather than passing silently.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

# --- the graph composes, and says what it composed
expect_exit 0 "$RB" commands graph
expect_grep 'majordomus/command-graph/v1'
expect_grep 'fingerprint'
expect_grep 'executable'

# every command of the executable's own command line is in it, and so is the shell tool's
expect_exit 0 "$RB" commands list --origin executable
expect_grep 'majordomus worktree status'
expect_exit 0 "$RB" commands list --origin tool
expect_grep 'majordomus check'

# --- the identity is derived, not written: one command, in full, and why each surface has it
expect_exit 0 "$RB" commands show executable.worktree.status
expect_grep 'read-only'
expect_grep 'projections'
expect_exit 0 "$RB" commands explain executable.serve
expect_grep 'declared in'
# a server is never a machine tool, and the reason is the product
expect_grep 'stopped'

# an identity nothing carries is refused, not answered emptily
expect_exit 12 "$RB" commands show executable.nonesuch
expect_grep 'no command carries the identity'

# --- the graph is deterministic
a="$("$RB" commands graph --format json | sed -n 's/.*"fingerprint": *"\([^"]*\)".*/\1/p' | head -1)"
b="$("$RB" commands graph --format json | sed -n 's/.*"fingerprint": *"\([^"]*\)".*/\1/p' | head -1)"
[ -n "$a" ] || { echo "    the graph carries no fingerprint"; exit 1; }
[ "$a" = "$b" ] || { echo "    two builds over one tree disagreed: $a vs $b"; exit 1; }

# --- the workflow bridge
expect_exit 0 "$RB" commands bridge
expect_grep 'recipe'
BRIDGE=".ai/local/cache/command-graph/bridge.just"
expect_file "$BRIDGE"
# it is generated, and it says so
grep -q 'GENERATED FILE' "$BRIDGE" || { echo "    the bridge does not declare itself generated"; exit 1; }
# every body forwards its arguments through the shell rather than splicing them
if grep -n '{{args}}' "$BRIDGE" >/dev/null; then
  echo "    the bridge interpolates arguments instead of forwarding them"; exit 1
fi
grep -q '"\$@"' "$BRIDGE" || { echo "    no recipe forwards \"\$@\""; exit 1; }
# and no recipe calls the runner back: a bridge over a bridge is the cycle this forbids
if grep -nE '^\s+@?"?just[[:space:]]' "$BRIDGE" >/dev/null; then
  echo "    a bridge recipe calls the workflow runner"; exit 1
fi
# a destructive command asks before it runs, because its effect said so
grep -q '\[confirm(' "$BRIDGE" || { echo "    no destructive command asks first"; exit 1; }

# materialising again writes nothing: the stamp answers before the graph is built
expect_exit 0 "$RB" commands bridge
expect_grep 'already current'
expect_exit 0 "$RB" commands bridge --check

# nothing tracked changed: entering a repository must not dirty it
[ -z "$(git status --porcelain)" ] || {
  echo "    materialising the bridge dirtied the working tree:"; git status --porcelain; exit 1; }

# --- the completion: one engine, two surfaces, no network, no index
expect_exit 0 "$RB" completion query --surface cli -- majordomus work
expect_grep 'worktree'
# the workflow surface resolves a recipe name back to the command and completes its own flags
expect_exit 0 "$RB" completion query --surface workflow -- just worktree-status --
expect_grep 'repo'
# an unknown command answers nothing and does not fail: a completion never breaks a shell
expect_exit 0 "$RB" completion query --surface cli -- majordomus nonesuch --unknown
# the adapters carry no command of their own
expect_exit 0 "$RB" completion init --shell zsh
expect_grep 'completion query'
for shell in zsh bash; do
  if "$RB" completion init --shell "$shell" | grep -E '(^|[^-a-z])worktree([^-a-z]|$)' >/dev/null; then
    echo "    the $shell adapter names a command"; exit 1
  fi
done

# --- the gate rejects a hand-written bridge, and can be proved to
#
# The mutation is made by shape, not by naming today's files: a copy of the workflow tree
# with one recipe added whose body calls the program. The probe asserts it took effect
# before the gate is asked, so a mutation that stopped mutating fails here rather than
# passing silently.
PROBE="$T/probe"; mkdir -p "$PROBE/.just" "$PROBE/scripts/ci" "$PROBE/bin"
cp "$ROOT/scripts/ci/command-graph" "$PROBE/scripts/ci/"
printf '%s\n' '# A recipe that only spells a command.' \
  "[group('probe')]" 'probe-status *args:' '    @bin/majordomus-cli worktree status "$@"' \
  > "$PROBE/.just/probe.just"
grep -q 'bin/majordomus-cli' "$PROBE/.just/probe.just" || { echo "    the probe did not take"; exit 1; }
: > "$PROBE/justfile"
cp "$ROOT/bin/majordomus-cli" "$PROBE/bin/" 2>/dev/null || true
( cd "$PROBE" && ./scripts/ci/command-graph >"$T/gate.out" 2>&1; echo $? > "$T/gate.code" ) || true
grep -q 'whose body is one call to the program' "$T/gate.out" || {
  echo "    the gate did not reject a hand-written bridge:"; cat "$T/gate.out"; exit 1; }

# ...and does not reject a composition. A recipe that runs two commands in an order is not
# a spelling of either: no node of the graph projects to it, so the bridge cannot write it
# and forbidding it would forbid composition. The distinction the gate makes is structural
# — how many program calls the body holds — rather than a list of blessed recipe names,
# which is the catalogue this whole gate exists to remove. Asserted with a second probe so
# that a gate which regressed to matching any call fails here.
printf '%s\n' '# Two commands, in an order, as one gate.' \
  "[group('probe')]" 'probe-check:' '    @bin/majordomus-cli distribution validate' \
  '    @bin/majordomus-cli generate distribution --check' \
  > "$PROBE/.just/probe.just"
( cd "$PROBE" && ./scripts/ci/command-graph >"$T/gate2.out" 2>&1 ) || true
if grep -q 'probe-check' "$T/gate2.out"; then
  echo "    the gate rejected a composition of two commands:"; cat "$T/gate2.out"; exit 1
fi

# --- adding a command reaches every projection without editing one
#
# The graph is asked for a command that exists only because clap declares it. If a surface
# kept a list, this would be the command missing from it.
NEW="$("$RB" commands list --origin executable --search completion | grep -c 'majordomus completion' || true)"
[ "$NEW" -ge 2 ] || { echo "    the completion commands are not all in the graph"; exit 1; }
grep -q '^completion-query \*args:' "$BRIDGE" || {
  echo "    a command declared only in clap did not reach the workflow projection"; exit 1; }
"$RB" completion query --surface workflow -- just completion-que | grep -q 'completion-query' || {
  echo "    a command declared only in clap did not reach the completion"; exit 1; }

# --- the shell integration installs itself, idempotently and reversibly
#
# The one command here that writes outside the repository, so it is exercised against a real
# file: what it writes, that a second run writes nothing, that removing restores the original
# byte for byte, and that a person's own lines are never touched.
RC="$T/rc-probe"; printf 'export PERSONAL=1\nalias mine=yours\n' > "$RC"
ORIGINAL="$(cat "$RC")"
expect_exit 0 "$RB" completion install --shell zsh --rc "$RC"
grep -q '>>> MAJORDOMUS >>>' "$RC" || { echo "    the install wrote no managed block"; exit 1; }
grep -q 'completion init --shell zsh' "$RC" || { echo "    the block does not load the integration"; exit 1; }
grep -q 'export PERSONAL=1' "$RC" || { echo "    the install ate the person's own lines"; exit 1; }
BEFORE="$(cat "$RC")"
expect_exit 0 "$RB" completion install --shell zsh --rc "$RC"
[ "$BEFORE" = "$(cat "$RC")" ] || { echo "    a second install changed the file"; exit 1; }
expect_exit 0 "$RB" completion install --shell zsh --rc "$RC" --remove
[ "$ORIGINAL" = "$(cat "$RC")" ] || {
  echo "    removing did not restore the file:"; diff <(printf '%s\n' "$ORIGINAL") "$RC" | head -5; exit 1; }
# it never writes without being asked: --dry-run leaves the file alone
expect_exit 0 "$RB" completion install --shell zsh --rc "$RC" --dry-run
[ "$ORIGINAL" = "$(cat "$RC")" ] || { echo "    --dry-run wrote to the file"; exit 1; }

# --- no secret ever reaches a projection
#
# Nothing in the graph reads the environment, so the assertion is cheap and the proof is
# the interesting part: every surface is asked while two sentinels are set, and every byte
# of every answer is searched for them.
export OPENAI_API_KEY=SENTINEL_OPENAI_DO_NOT_LEAK
export ANTHROPIC_API_KEY=SENTINEL_ANTHROPIC_DO_NOT_LEAK
{
  "$RB" commands graph --format json
  "$RB" commands list
  "$RB" commands show executable.worktree.status --format json
  "$RB" commands explain executable.serve
  "$RB" completion query --surface cli -- majordomus
  "$RB" completion query --surface workflow -- just
  "$RB" completion init --shell zsh
  "$RB" completion init --shell bash
  cat "$BRIDGE"
  cat .ai/local/cache/command-graph/bridge.json
  cat .ai/local/cache/command-graph/graph.json
} > "$T/everything.txt" 2>&1
if grep -c 'SENTINEL_' "$T/everything.txt" | grep -qv '^0$'; then
  echo "    a sentinel reached a projection:"; grep -n 'SENTINEL_' "$T/everything.txt" | head -5; exit 1
fi
unset OPENAI_API_KEY ANTHROPIC_API_KEY

# --- one repository at a time
#
# The graph is a property of the repository the caller is in, so a command of this
# repository must not appear when the question is asked about another one. Leaving a
# repository leaves nothing behind because there is nothing to leave: the answer is
# recomputed from where the caller is.
# The identity is set here as every other fixture in this suite sets it: `git init` inherits
# none, and a runner has no global one, so the commit below failed on CI with "empty ident
# name" and nowhere else. test/run.sh configures the case's own $T repository; this is a
# second repository the case makes for itself.
OTHER="$T/other"; mkdir -p "$OTHER"
( cd "$OTHER" && git init -q . && git config user.email t@example.com && git config user.name t \
  && git commit -q --allow-empty -m x )
"$RB" --repo "$OTHER" completion query --surface workflow -- just > "$T/other.txt" 2>&1 || true
if grep -q 'site-build' "$T/other.txt"; then
  echo "    a workflow of one repository was offered in another"; exit 1
fi
