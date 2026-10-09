# majordomus-covers: capture
# majordomus-negative: capture
# claim: session-lifecycle
# The provider mod that asks the worker for a handover before the context window fills
# (ADR 0123) is installed by the command that installs the shims, under the shims' rules.
#
# PreCompact derives a checkpoint because the worker cannot be asked at that moment; the mod
# asks earlier, and only a mod can, because the provider measures the window inside the
# conversation and fires no event for it. What this case proves is the half the repository
# owns: the folder is written from the distribution, rewritten when it drifts, left alone
# when it is somebody's own, and removed when the policy turns it off. The mod's behaviour
# is the distribution's own test, `claude plugin test share/mods/claude-code/majordomus-handover`.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
git add . && git commit -qm base

MOD=.claude/skills/majordomus-handover
fail() { printf '    %s\n' "$*"; exit 1; }
pol_set() {
  sed "s/^\(  $1:\) [a-z]*/\1 $2/" .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
  grep -q "^  $1: $2" .ai/repo/policy.yaml || fail "the policy probe did not take: $1: $2"
}

# ---------------------------------------------------------------- the skeleton declares it
grep -q '^  handover_on_fill: true' .ai/repo/policy.yaml \
  || fail 'a new repository'"'"'s policy does not declare session.handover_on_fill'

# ---------------------------------------------------------------- written, and only the mod
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — written"
for f in .claude-plugin/plugin.json hooks/hooks.json hooks/register.ts .gitignore; do
  [ -f "$MOD/$f" ] || fail "capture install did not write $MOD/$f"
done
[ ! -e "$MOD/hooks/register.test.ts" ] || fail 'the distribution'"'"'s own test was installed'
grep -qF 'Written by `majordomus capture install`' "$MOD/hooks/register.ts" \
  || fail 'the installed hooks module carries no marker, so the next install cannot own it'
grep -qF "'session.measure'" "$MOD/hooks/register.ts" || fail 'the installed module measures nothing'
expect_no_grep 'skills' .claude/settings.json

# ---------------------------------------------------------------- idempotent
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — already present; left as it is"
# the provider lays its type declarations beside a mod it loaded; that is not drift
mkdir -p "$MOD/.claude-plugin/types/claude-code" && echo '// laid' > "$MOD/.claude-plugin/types/claude-code/index.d.ts"
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — already present; left as it is"

# ---------------------------------------------------------------- drift is repaired
printf '// edited by hand\n' >> "$MOD/hooks/register.ts"
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — rewritten"
expect_no_grep 'edited by hand' "$MOD/hooks/register.ts"

# ---------------------------------------------------------------- the policy is the switch
pol_set handover_on_fill false
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — removed: session.handover_on_fill is not true"
[ ! -e "$MOD" ] || fail 'a mod the policy turned off is still where the provider loads it'
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — not written: session.handover_on_fill is not true"
# an older policy that never heard of the key: absent is not consent
sed '/^  handover_on_fill:/d' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — not written: session.handover_on_fill is not true"
[ ! -e "$MOD" ] || fail 'a policy without the key installed the mod'
sed 's/^\(  handover_on_end: .*\)$/\1\n  handover_on_fill: true/' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
grep -q '^  handover_on_fill: true' .ai/repo/policy.yaml || fail 'the key could not be restored'
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — written"

# ---------------------------------------------------------------- somebody's own is theirs
rm -rf "$MOD" && mkdir -p "$MOD/hooks" && printf '// mine\n' > "$MOD/hooks/register.ts"
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — already present and not this tool's; left as it is"
grep -qx '// mine' "$MOD/hooks/register.ts" || fail 'capture install rewrote a mod it did not write'
pol_set handover_on_fill false
expect_exit 0 "$MJ" capture install
[ -f "$MOD/hooks/register.ts" ] || fail 'the policy switch removed a mod this tool did not write'
