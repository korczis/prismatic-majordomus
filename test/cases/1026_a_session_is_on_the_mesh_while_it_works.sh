# majordomus-covers: start
# majordomus-covers: finish
# A task is on the mesh while it works (ADR 0128, I2288).
#
# `majordomus start` claims the task's scope on this checkout's running server as an advisory
# claim of the mesh session `task-<id>`, so every linked runtime sees what the task works on; the
# key is kept on the task record. `finish` closes the session, and its claims end with it. With
# no server, or a mesh that does not run, nothing is claimed and nothing fails.
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
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
XDG_STATE_HOME="$T/state"; export XDG_STATE_HOME
trap '"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true' EXIT

# --- no server: start claims nothing and does not fail
expect_exit 0 "$MJ" start "Before any server" --scope lib
grep -q '^mesh_claim: ' .ai/local/state/current.yaml && { echo "    a claim was recorded with no server"; exit 1; }
printf '# Objective\nx\n\n# Current State\ny\n\n# Next Action\nz\n' > "$T/note.md"
expect_exit 0 "$MJ" finish --outcome partial --note "$T/note.md"

# --- a server whose mesh runs
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
expect_exit 0 "$RB" serve ensure --repo "$T" --idle 120
i=0; until "$RB" mesh briefing --repo "$T" --format json 2>/dev/null | jq -e '.active == true' >/dev/null; do
  i=$((i + 1)); [ "$i" -lt 100 ] || { echo "    the mesh did not start"; exit 1; }; sleep 0.2
done

expect_exit 0 "$MJ" start "Teach the parser" --scope lib
expect_grep 'mesh .*task-t-.* the scope is claimed on every linked runtime [(]advisory[)]'
id="$(sed -n 's/^id: //p' .ai/local/state/current.yaml)"
key="$(sed -n 's/^mesh_claim: //p' .ai/local/state/current.yaml)"
[ -n "$key" ] || { echo "    the task record holds no mesh claim"; exit 1; }
"$RB" mesh briefing --repo "$T" --format json > "$T/b.json"
jq -e --arg s "task-$id" '[.briefing.machines[].sessions[] | select(.key | endswith("/" + $s)) | select(.claims == ["lib"]) | select(.intent == "Teach the parser")] | length == 1' "$T/b.json" >/dev/null \
  || { echo "    the task's session and claim are not on the mesh:"; cat "$T/b.json"; exit 1; }

expect_exit 0 "$MJ" finish --outcome partial --note "$T/note.md"
"$RB" mesh briefing --repo "$T" --format json > "$T/b2.json"
jq -e --arg s "task-$id" '[.briefing.machines[]?.sessions[]? | select(.key | endswith("/" + $s))] | length == 0' "$T/b2.json" >/dev/null \
  || { echo "    the finished task is still on the mesh:"; cat "$T/b2.json"; exit 1; }
