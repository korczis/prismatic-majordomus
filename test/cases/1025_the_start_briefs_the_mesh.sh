# majordomus-covers: capture session
# claims: mesh-declared-is-held
# The start of a session tells the mesh as work, not only its health (ADR 0128, I2286).
#
# Under `Mesh: active — …` the start briefing prints what `mesh briefing` answers from the
# running server: who works where on every machine, the handovers waiting here, the open
# reviews. In a fixture whose mesh runs alone, that is the one line saying nobody else works and
# nothing waits — said, not omitted. The health line stays the only line that starts `Mesh:`.
# The fixture's mesh opens no socket beyond a probe: multicast off, no hub, no seed.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?

cd "$T" || exit 1
git init -q .
git config user.email t@example.com
git config user.name t
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
STATE="$T/state"; mkdir -p "$STATE"
XDG_STATE_HOME="$STATE"; export XDG_STATE_HOME
trap '"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true' EXIT
mkdir -p .ai/repo/mesh
cat > .ai/repo/mesh/majordomus.yaml <<YAML
schema: mesh/v1
kind: mesh-declaration
id: majordomus
enabled: true
multicast:
  enabled: false
trust:
  policy: deny_unknown
YAML
git add .ai/repo/mesh/majordomus.yaml && git commit -qm "mesh enabled"

out="$(printf '{"session_id":"cc-1025","hook_event_name":"SessionStart","source":"startup"}' \
  | ./.claude/hooks/majordomus-session-start 2>"$T/err")" || { echo "    the start shim failed"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -q '^Mesh: active — ' \
  || { echo "    no active mesh in the briefing:"; printf '%s\n' "$out"; cat "$T/err"; exit 1; }
# the briefing follows the health line, indented, and says that nobody else works here
printf '%s\n' "$out" | grep -A1 '^Mesh: active — ' \
  | grep -q "^  nobody else works in this repository's mesh; nothing waits for this machine$" \
  || { echo "    the mesh is not told as work under the Mesh line:"; printf '%s\n' "$out"; cat "$T/err"; exit 1; }
[ "$(printf '%s\n' "$out" | grep -c '^Mesh:')" = 1 ] \
  || { echo "    more than one line starts with Mesh:"; printf '%s\n' "$out"; exit 1; }
# the same answer is what the command prints
expect_exit 0 "$RB" mesh briefing --repo "$T"
expect_grep "nobody else works in this repository's mesh"
