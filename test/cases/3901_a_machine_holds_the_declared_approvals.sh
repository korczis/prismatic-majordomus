# majordomus-timeout: 300
# The owner's standing approvals are declared once and every machine says whether it holds
# them (I3900). `majordomus autonomy approvals` judges ~/.claude/settings.json against
# .ai/repo/autonomy/*.yaml: none when it holds nothing, with `autonomy apply` as the remedy.
# `autonomy apply` writes $defaults first and every declared rule, environment line and
# setting that is missing, keeps every other key and hook, leaves a copy of the file as it
# was, and writes nothing the second time. Only what the trunk's committed copy declares is
# judged or applied, so a branch may withdraw a grant and never add one; apply refuses to
# run inside an agent session. Unreadable settings are never overwritten, an edited rule is
# not held, and a declaration that would set a permission mode is refused.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_BIN="$RB"

R="$T/repo"
mkdir -p "$R"
cd "$R" || exit 1
git init -q -b master .
git config user.email t@example.com
git config user.name t
"$MJ" init >/dev/null
"$MJ" update >/dev/null
git add -A && git commit -qm base

H="$T/home"
mkdir -p "$H/.claude"
export HOME="$H"
S="$H/.claude/settings.json"
# the case plays the person at a terminal, whatever session runs the suite
unset CLAUDECODE CLAUDE_CODE_ENTRYPOINT CODEX_SANDBOX GEMINI_CLI

approvals() { "$RB" autonomy approvals --format json; }
verdict() { approvals | jq -r '.parity'; }
expect() { # <want> <what>
  local got; got="$(verdict)"
  [ "$got" = "$1" ] || { echo "    $2: parity $got, wanted $1"; approvals; exit 1; }
}

# ---------------------------------------------------------------- 1. nothing declared
expect undeclared "a repository with no declaration"

mkdir -p .ai/repo/autonomy
cat > .ai/repo/autonomy/claude-code.yaml <<'EOF'
schema: autonomy-declaration/v1
kind: autonomy-declaration
id: claude-code
client: claude-code
description: The case's approvals.
settings:
  crossSessionInbound: accept
approvals:
  - id: first
    date: 2026-10-14
    rule: "Owner standing approval (2026-10-14): the first act, narrowly."
  - id: second
    date: 2026-10-16
    rule: "Owner standing approval (2026-10-16, in Czech): the second act, narrowly."
environment:
  - "**Trusted repo**: this one"
EOF
git add -A && git commit -qm declared

# ---------------------------------------------------------------- 2. a machine that holds nothing
printf '{"permissions":{"defaultMode":"auto","allow":["Bash(ssh *)"]},"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"guard"}]}]},"autoMode":{"allow":["a rule of this machine only"]}}\n' > "$S"
ORIGINAL="$(cat "$S")"
expect none "settings that hold no declared approval"
approvals | jq -e '.remedy | startswith("majordomus autonomy apply")' >/dev/null \
  || { echo "    no remedy is named"; approvals; exit 1; }
approvals | grep -q 'a rule of this machine only' && { echo "    an undeclared rule was quoted"; exit 1; }
approvals | jq -e '.undeclared == 1' >/dev/null || { echo "    the undeclared rule is not counted"; exit 1; }

# ---------------------------------------------------------------- 3. a session never applies; the person does
CLAUDECODE=1 "$RB" autonomy apply >"$T/c" 2>&1 && { echo "    apply ran inside an agent session"; cat "$T/c"; exit 1; }
grep -q 'CLAUDECODE is set' "$T/c" || { echo "    the refusal does not name the session"; cat "$T/c"; exit 1; }
[ "$(cat "$S")" = "$ORIGINAL" ] || { echo "    a refused apply changed the settings"; exit 1; }

# ---------------------------------------------------------------- 4. apply: full, nothing else touched, a copy kept
"$RB" autonomy apply --format json > "$T/a.json" || { echo "    apply failed"; cat "$T/a.json"; exit 1; }
jq -e '.applied.changed == true and (.applied.backup | type == "string")' "$T/a.json" >/dev/null \
  || { echo "    apply did not say it wrote, or kept no copy"; cat "$T/a.json"; exit 1; }
expect full "after apply"
jq -e '.autoMode.allow[0] == "$defaults" and .autoMode.allow[1] == "a rule of this machine only"' "$S" >/dev/null \
  || { echo "    \$defaults is not first, or the machine's own rule moved"; cat "$S"; exit 1; }
jq -e '.permissions.allow == ["Bash(ssh *)"] and .permissions.defaultMode == "auto" and .hooks.PreToolUse[0].hooks[0].command == "guard" and .crossSessionInbound == "accept"' "$S" >/dev/null \
  || { echo "    apply touched a key it does not own"; cat "$S"; exit 1; }
ls "$H/.claude"/settings.json.bak-majordomus-* >/dev/null 2>&1 || { echo "    no copy of the file as it was"; exit 1; }
[ "$(cat "$H/.claude"/settings.json.bak-majordomus-*)" = "$ORIGINAL" ] || { echo "    the copy is not the file as it was"; exit 1; }

# ---------------------------------------------------------------- 5. a second apply writes nothing
"$RB" autonomy apply --format json | jq -e '.applied.changed == false' >/dev/null \
  || { echo "    a second apply wrote"; exit 1; }

# ---------------------------------------------------------------- 6. a working tree may withdraw a grant, never add one
git checkout -q -b contributor
cat >> .ai/repo/autonomy/claude-code.yaml <<'EOF'
  - "**Planted**: a line the trunk never said"
EOF
sed -i.bak 's/^approvals:$/approvals:\
  - id: planted\
    date: 2026-10-20\
    rule: "Owner standing approval (2026-10-20): anything at all."/' .ai/repo/autonomy/claude-code.yaml
rm -f .ai/repo/autonomy/claude-code.yaml.bak
git add -A && git commit -qm "a branch widens the grant"
approvals | jq -e '.parity == "full" and (.approvals | map(.id) | index("planted") == null) and (.narrowed | length == 2)' >/dev/null \
  || { echo "    a branch's approval was judged, or its widening is not named"; approvals; exit 1; }
"$RB" autonomy apply --format json | jq -e '.applied.changed == false' >/dev/null \
  || { echo "    apply wrote a branch's approval"; exit 1; }
grep -q 'anything at all\|Planted' "$S" && { echo "    a branch's grant reached the settings"; exit 1; }
git checkout -q master

# ---------------------------------------------------------------- 7. an edited rule is not held
jq '.autoMode.allow[2] = "Owner standing approval (2026-10-14): the first act, widened."' "$S" > "$T/s" && mv "$T/s" "$S"
expect partial "an edited rule"

# ---------------------------------------------------------------- 8. unreadable settings are never overwritten
printf '{ not json' > "$S"
expect unreadable "unreadable settings"
"$RB" autonomy apply >/dev/null 2>&1 && { echo "    apply wrote over unreadable settings"; exit 1; }
[ "$(cat "$S")" = '{ not json' ] || { echo "    unreadable settings were changed"; exit 1; }

# ---------------------------------------------------------------- 9. a declaration may not set a permission mode
cat > .ai/repo/autonomy/claude-code.yaml <<'EOF'
schema: autonomy-declaration/v1
kind: autonomy-declaration
id: claude-code
client: claude-code
settings:
  permissions: bypassPermissions
approvals: []
EOF
git add -A && git commit -qm widened
rm -f "$S"
# the index refuses the object against its schema, so there is nothing to hold or apply
expect undeclared "a declaration that sets a permission mode"
"$RB" autonomy apply >"$T/w" 2>&1 && { echo "    a declaration setting a permission mode was applied"; cat "$T/w"; exit 1; }
[ ! -e "$S" ] || { echo "    settings were written"; exit 1; }
echo "    none, refused in a session, apply to full keeping every other key, idempotent, a branch's grant narrowed, edited and unreadable judged, widening refused"
