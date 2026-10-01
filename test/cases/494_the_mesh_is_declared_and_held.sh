# majordomus-covers: capture session
# claims: mesh-declared-is-held
# The mesh is declared on for this repository, and every session start holds it (ADR 0059).
#
# Two halves. The first reads this repository's own declaration, because the drift this
# case exists to refuse is exactly a reverted or never-committed one: on 2026-09-12 three
# machines each ran an uncommitted `enabled: true` while master said `false`, and the one
# that was reset was silently alone. A tree whose declaration is disabled, untracked,
# allowlists fewer keys than docs/MESH.md names machines, or names no rendezvous hub is
# refused. (491 holds the declaration's least-privilege shape; this holds that it is on.)
#
# The second drives a disposable repository through the provider's start event, as 108
# does, and proves the answers the briefing and `mesh doctor` give: no line without a
# declaration, `active`, `off, as declared`, and the one this exists for — DECLARED ENABLED
# BUT NOT ACTIVE with the server's reason, and exit 10 from the doctor — never a quiet off.
# Every server of the fixture declares no transport that opens a socket beyond a probe:
# multicast off, no hub, no seed, and its identity under $T.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?

# ---------------------------------------------------------------- 1. this repository declares its mesh
decl=.ai/repo/mesh/majordomus.yaml
[ -f "$ROOT/$decl" ] || { echo "    this repository has no mesh declaration at $decl"; exit 1; }
git -C "$ROOT" ls-files --error-unmatch -- "$decl" >/dev/null 2>&1 \
  || { echo "    $decl is not tracked: an untracked declaration is one machine's, not the repository's"; exit 1; }
# As the executable's index reads the committed object: comments stripped, the schema applied.
( cd "$ROOT" && MAJORDOMUS_SHARE="$ROOT/share" "$RB" run objects.get \
    --input '{"uri":"majordomus://mesh-declaration/majordomus"}' --format json ) \
  > "$T/object.json" 2> "$T/object.err" || { echo "    the index does not read the declaration:"; cat "$T/object.err"; exit 1; }
jq -e '.output.metadata.enabled == true' "$T/object.json" >/dev/null \
  || { echo "    the committed declaration is not enabled: the fleet this repository is developed on does not see itself"; exit 1; }
policy="$(jq -r '.output.metadata.trust.policy // "deny_unknown"' "$T/object.json")"
[ "$policy" = deny_unknown ] || { echo "    trust.policy is '$policy', not deny_unknown"; exit 1; }
# The fleet is the machines docs/MESH.md names in its trust row; the allowlist holds at
# least that many keys, so a machine dropped from the declaration and not from the page
# (or never added to the declaration) fails here.
fleet="$(awk '/^## This repository.s mesh/{f=1;next} f&&/^## /{exit} f' "$ROOT/docs/MESH.md" \
  | grep -E '^\| trust \|' | grep -oE '`[0-9a-f]{8}`' | wc -l | tr -d ' ')"
[ "${fleet:-0}" -ge 1 ] || { echo "    docs/MESH.md names no machine in the trust row of \"This repository's mesh\""; exit 1; }
keys="$(jq -r '.output.metadata.trust.allow // [] | length' "$T/object.json")"
[ "$keys" -ge "$fleet" ] \
  || { echo "    trust.allow holds $keys key(s) and docs/MESH.md names $fleet machine(s): the allowlist is smaller than the fleet"; exit 1; }
hubs="$(jq -r '.output.metadata.rendezvous.endpoints // [] | length' "$T/object.json")"
[ "$hubs" -ge 1 ] \
  || { echo "    no rendezvous hub: multicast crosses neither the tailnet nor the macOS firewall, so the fleet cannot meet"; exit 1; }
# The self-check in this process, over the real declaration: the runtime check is there,
# and a process no server runs in reports an absence, never a verdict it did not make.
( cd "$ROOT" && MAJORDOMUS_SHARE="$ROOT/share" XDG_STATE_HOME="$T/no-identity" "$RB" run mesh.doctor --format json ) \
  > "$T/doctor.json" 2> "$T/doctor.err" || { echo "    mesh.doctor did not answer:"; cat "$T/doctor.err"; exit 1; }
jq -e '[.output.checks[] | select(.check == "runtime" and .ok and (.detail | startswith("not decided in this process")))] | length == 1' \
  "$T/doctor.json" >/dev/null || { echo "    the in-process runtime check is not an absence:"; jq '.output.checks' "$T/doctor.json"; exit 1; }

# ---------------------------------------------------------------- 2. a disposable repository, through the start event
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
STATE="$T/state"; mkdir -p "$STATE"
XDG_STATE_HOME="$STATE"; export XDG_STATE_HOME
trap '"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true' EXIT
start_event() {
  printf '{"session_id":"cc-%s","hook_event_name":"SessionStart","source":"startup"}' "$1" \
    | ./.claude/hooks/majordomus-session-start 2>"$T/err"
}
declare_mesh() {
  mkdir -p .ai/repo/mesh
  cat > .ai/repo/mesh/majordomus.yaml <<YAML
schema: mesh/v1
kind: mesh-declaration
id: majordomus
enabled: $1
multicast:
  enabled: false
trust:
  policy: deny_unknown
YAML
  git add .ai/repo/mesh/majordomus.yaml && git commit -qm "mesh enabled: $1"
}
briefing_line() { printf '%s\n' "$out" | grep '^Mesh:' || true; }

# --- no declaration: no line. A repository without a mesh does not grow a line about it.
out="$(start_event 0)" || { echo "    the start shim failed"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -q '^Shared server: ready http://' \
  || { echo "    no ready server in the briefing:"; printf '%s\n' "$out"; cat "$T/err"; exit 1; }
[ -z "$(briefing_line)" ] || { echo "    a Mesh line with no declaration:"; printf '%s\n' "$out"; exit 1; }
"$RB" serve stop --repo "$T" >/dev/null 2>&1

# --- enabled, and the server activated it: the briefing says so, in the block naming the server
declare_mesh true
out="$(start_event 1)" || { echo "    the start shim failed"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -A1 '^Shared server: ready http://' | grep -q '^Mesh: active — ' \
  || { echo "    an active mesh is not named under the server line:"; printf '%s\n' "$out"; cat "$T/err"; exit 1; }
[ "$(briefing_line | wc -l | tr -d ' ')" = 1 ] || { echo "    more than one Mesh line:"; printf '%s\n' "$out"; exit 1; }
[ -f "$STATE/majordomus/node.json" ] || { echo "    activation did not create the identity under XDG_STATE_HOME"; exit 1; }
expect_exit 0 "$RB" mesh doctor --repo "$T"
expect_grep '^ok    runtime      active as '
expect_grep '^verdict    every check holds'
"$RB" serve stop --repo "$T" >/dev/null 2>&1

# --- enabled, and the server could not activate it: loud in the briefing, exit 10 from the doctor
# The identity cannot be created when a directory stands where node.json must be written.
# The server still serves (a mesh that cannot start is a reason, never a failed server), and
# its reason is what the briefing and the doctor carry.
mkdir -p "$T/state-blocked/majordomus/node.json"
out="$(XDG_STATE_HOME="$T/state-blocked" start_event 2)" || { echo "    the start shim failed"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -q '^Shared server: ready http://' \
  || { echo "    a mesh that cannot start failed the server:"; printf '%s\n' "$out"; cat "$T/err"; exit 1; }
line="$(briefing_line)"
case "$line" in
  "Mesh: DECLARED ENABLED BUT NOT ACTIVE — "?*) ;;
  *) echo "    an enabled declaration the server did not activate is not loud, or carries no reason:"; printf '%s\n' "$out"; cat "$T/err"; exit 1 ;;
esac
expect_exit 10 "$RB" mesh doctor --repo "$T"
expect_grep "^FAIL  runtime      the declaration is enabled and this server's mesh is not active — ."
expect_grep 'remedy .*serve ensure'
expect_grep '^verdict    a check failed'
# one answer in both formats, from the server (stdout alone: the log goes to stderr)
code=0; "$RB" mesh doctor --repo "$T" --format json > "$T/blocked.json" 2> "$T/blocked.err" || code=$?
[ "$code" = 10 ] || { echo "    mesh doctor --format json exited $code, not 10:"; cat "$T/blocked.err"; exit 1; }
jq -e '.ok == false and ([.checks[] | select(.check == "runtime" and (.ok | not))] | length == 1)' "$T/blocked.json" >/dev/null \
  || { echo "    the JSON report does not carry the failed runtime check:"; cat "$T/blocked.json"; exit 1; }
"$RB" serve stop --repo "$T" >/dev/null 2>&1

# --- disabled: off, as declared, and nothing fails
declare_mesh false
out="$(start_event 3)" || { echo "    the start shim failed"; cat "$T/err"; exit 1; }
[ "$(briefing_line)" = "Mesh: off, as declared" ] \
  || { echo "    a disabled declaration is not named as such:"; printf '%s\n' "$out"; cat "$T/err"; exit 1; }
expect_exit 0 "$RB" mesh doctor --repo "$T"
expect_grep '^ok    runtime      off, as declared'
"$RB" serve stop --repo "$T" >/dev/null 2>&1

# --- no server at all: the doctor runs here, and an absence is not a verdict
expect_exit 0 "$RB" mesh doctor --repo "$T"
expect_grep '^ok    runtime      not decided in this process'
echo "    the mesh is declared, and every session start says whether the server holds it"
