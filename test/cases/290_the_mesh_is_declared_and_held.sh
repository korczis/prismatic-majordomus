# majordomus-covers: capture session
# claims: mesh-declared-is-held
# The mesh is declared on for this repository and every session start holds it (ADR 0059).
#
# Two halves. The first reads this repository's own declaration, because the drift this
# case exists to refuse is exactly a reverted or never-committed declaration: on
# 2026-09-12 three machines each ran an uncommitted `enabled: true` while master said
# `false`, and the one that was reset was silently alone. The second drives a
# disposable repository through the provider's start event, as 108 does, and proves the
# three answers the briefing and `mesh doctor` give: active, off as declared, and the
# one that matters — DECLARED ENABLED BUT NOT ACTIVE, with the server's reason and exit
# 10 — never a quiet off.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || { echo "    curl absent; skipping"; exit 0; }
command -v jq >/dev/null 2>&1 || { echo "    jq absent; skipping"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?

# ---------------------------------------------------------------- 1. this repository declares its mesh
decl="$ROOT/.ai/repo/mesh/majordomus.yaml"
[ -f "$decl" ] || { echo "    this repository has no mesh declaration at $decl"; exit 1; }
git -C "$ROOT" ls-files --error-unmatch .ai/repo/mesh/majordomus.yaml >/dev/null 2>&1 \
  || { echo "    the declaration is not tracked: an untracked declaration is one machine's, not the repository's"; exit 1; }
strip() { sed 's/[[:space:]]*#.*$//' "$decl"; }
strip | grep -qE '^enabled:[[:space:]]*true[[:space:]]*$' \
  || { echo "    the committed declaration is not enabled: the fleet this repository is developed on is not seeing itself"; exit 1; }
strip | grep -qE '^[[:space:]]+policy:[[:space:]]*deny_unknown[[:space:]]*$' \
  || { echo "    trust.policy is not deny_unknown"; exit 1; }
keys="$(strip | sed -n 's/^[[:space:]]*- *\([0-9a-fA-F]\{64\}\)[[:space:]]*$/\1/p' | wc -l | tr -d ' ')"
[ "$keys" -ge 3 ] || { echo "    trust.allow names $keys key(s); the fleet has three machines"; exit 1; }
strip | sed -n 's/^[[:space:]]*- *"\{0,1\}\(http:\/\/[^" ]*\)"\{0,1\}[[:space:]]*$/\1/p' | grep -q . \
  || { echo "    no rendezvous endpoint: multicast does not cross the tailnet or the macOS firewall"; exit 1; }
# The self-check of the machine this runs on, against the real declaration: exit 0 or 10
# only — never an error — and the runtime line is present whichever way it went.
( cd "$ROOT" && MAJORDOMUS_SHARE="$ROOT/share" "$RB" mesh doctor --format json > "$T/doctor.json" 2>"$T/doctor.err" ); code=$?
case "$code" in 0|10) ;; *) echo "    mesh doctor exited $code on this repository:"; cat "$T/doctor.err"; exit 1 ;; esac
jq -e '.checks[] | select(.check == "runtime")' "$T/doctor.json" >/dev/null || { echo "    no runtime check in the report"; exit 1; }
jq -e '.checks[] | select(.check == "trust")' "$T/doctor.json" >/dev/null || { echo "    no trust check in the report"; exit 1; }

# ---------------------------------------------------------------- 2. a disposable repository, through the start event
"$MJ" init >/dev/null; "$MJ" update >/dev/null
# The skeleton `init` writes declares no `mesh-declaration` source class, so a fresh
# repository cannot see a declaration until its sources say where one lives; this
# repository's sources do. (A skeleton that declared the class is ADR 0059's open end.)
cp "$ROOT/.ai/repo/knowledge/sources.yaml" .ai/repo/knowledge/sources.yaml
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
STATE="$T/state"; mkdir -p "$STATE"
XDG_STATE_HOME="$STATE"; export XDG_STATE_HOME
trap '"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true' EXIT
start_event() {
  printf '{"session_id":"cc-%s","hook_event_name":"SessionStart","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start 2>"$T/err"
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

# --- no declaration: no line. A repository without a mesh must not grow a line about it.
out="$(start_event 0)" || { echo "    the start shim failed"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -q '^Shared server: ready http://' || { echo "    no ready server in the briefing:"; printf '%s\n' "$out"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -q '^Mesh:' && { echo "    a Mesh line with no declaration:"; printf '%s\n' "$out"; exit 1; }
"$RB" serve stop --repo "$T" >/dev/null 2>&1

# --- enabled, and the server activated it: the briefing says so, the doctor holds
declare_mesh true
out="$(start_event 1)" || { echo "    the start shim failed"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -q '^Mesh: active — ' || { echo "    an active mesh is not named in the briefing:"; printf '%s\n' "$out"; cat "$T/err"; exit 1; }
expect_exit 0 "$RB" mesh doctor --repo "$T"
expect_grep '^ok    runtime      active as '
expect_grep '^ok    trust '
expect_grep '^verdict    every check holds'
expect_exit 0 "$RB" mesh status --repo "$T"
expect_grep '^mesh       active'
[ -f "$STATE/majordomus/node.json" ] || { echo "    activation did not create the identity under XDG_STATE_HOME"; exit 1; }
"$RB" serve stop --repo "$T" >/dev/null 2>&1

# --- enabled, and the server could not activate it: loud in the briefing, exit 10 from the doctor
# The identity cannot be created when a directory sits where node.json must be written;
# the server still serves (a mesh that cannot start is a reason, never a failed server),
# and that reason is what the briefing and the doctor must carry.
mkdir -p "$T/state-blocked/majordomus/node.json"
out="$(XDG_STATE_HOME="$T/state-blocked" start_event 2)" || { echo "    the start shim failed"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -q '^Shared server: ready http://' || { echo "    a mesh that cannot start failed the server:"; printf '%s\n' "$out"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -q '^Mesh: DECLARED ENABLED BUT NOT ACTIVE — ' || { echo "    an enabled declaration nobody activated is not loud:"; printf '%s\n' "$out"; cat "$T/err"; exit 1; }
expect_exit 10 "$RB" mesh doctor --repo "$T"
expect_grep '^FAIL  runtime      the declaration is enabled and this server'"'"'s mesh is not active'
expect_grep 'serve ensure'
expect_grep '^verdict    a check failed'
# json and text are one answer
expect_exit 10 "$RB" mesh doctor --repo "$T" --format json
expect_grep '"ok": false'
"$RB" serve stop --repo "$T" >/dev/null 2>&1

# --- disabled: off, as declared, and nothing fails
declare_mesh false
out="$(start_event 3)" || { echo "    the start shim failed"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -q '^Mesh: off, as declared$' || { echo "    a disabled declaration is not named as such:"; printf '%s\n' "$out"; cat "$T/err"; exit 1; }
expect_exit 0 "$RB" mesh doctor --repo "$T"
expect_grep '^ok    runtime      off, as declared'
"$RB" serve stop --repo "$T" >/dev/null 2>&1

# --- no server at all: the doctor runs here, and an absence is not a verdict
expect_exit 0 "$RB" mesh doctor --repo "$T"
expect_grep '^ok    runtime      not decided in this process'
echo "    the mesh is declared, and held at every start"
