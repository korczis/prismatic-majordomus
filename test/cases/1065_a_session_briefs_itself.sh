# majordomus-covers: capture
# majordomus-negative: capture
# claim: session-lifecycle
# A session reports on itself without being asked (ADR 0136).
#
# Two halves the repository owns are held here. The provider mod that keeps a Claude Code
# session briefed (share/mods/claude-code/majordomus-brief) is installed by the command
# that installs the shims, switched by `session.brief_on_interval` under the rules ADR
# 0123 gave the handover mod: written, idempotent, removed when switched off, never
# written for a policy that lacks the key. And `majordomus brief` answers its one line,
# its keep-awake text and its multi-line form with every section present, from a process
# that says it composed the brief itself because no server serves the checkout. The mod's
# own behaviour is the distribution's test: `claude plugin test share/mods/claude-code/majordomus-brief`.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
git add . && git commit -qm base

MOD=.claude/skills/majordomus-brief
fail() { printf '    %s\n' "$*"; exit 1; }
pol_set() {
  sed "s/^\(  $1:\) [a-z0-9]*/\1 $2/" .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
  grep -q "^  $1: $2" .ai/repo/policy.yaml || fail "the policy probe did not take: $1: $2"
}

# ---------------------------------------------------------------- the skeleton declares it
grep -q '^  brief_on_interval: true' .ai/repo/policy.yaml \
  || fail 'a new repository'"'"'s policy does not declare session.brief_on_interval'
grep -q '^  brief_interval_minutes: 20' .ai/repo/policy.yaml \
  || fail 'a new repository'"'"'s policy does not declare session.brief_interval_minutes'

# ---------------------------------------------------------------- written beside the handover mod
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — written"
expect_grep ".claude/skills/majordomus-handover — written"
for f in .claude-plugin/plugin.json hooks/hooks.json hooks/register.ts .gitignore; do
  [ -f "$MOD/$f" ] || fail "capture install did not write $MOD/$f"
done
[ ! -e "$MOD/hooks/register.test.ts" ] || fail 'the distribution'"'"'s own test was installed'
grep -qF 'Written by `majordomus capture install`' "$MOD/hooks/register.ts" \
  || fail 'the installed hooks module carries no marker, so the next install cannot own it'
grep -qF '$.ui.status' "$MOD/hooks/register.ts" || fail 'the installed module shows no status line'
expect_no_grep 'majordomus-brief' .claude/settings.json

# ---------------------------------------------------------------- idempotent
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — already present; left as it is"

# ---------------------------------------------------------------- the policy is the switch
pol_set brief_on_interval false
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — removed: session.brief_on_interval is not true"
[ ! -e "$MOD" ] || fail 'a mod the policy turned off is still where the provider loads it'
[ -d .claude/skills/majordomus-handover ] || fail 'switching the brief off took the handover mod too'
sed '/^  brief_on_interval:/d' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
expect_exit 0 "$MJ" capture install
expect_grep "$MOD — not written: session.brief_on_interval is not true"

# ---------------------------------------------------------------- the brief answers
# Standard output alone: the executable logs on standard error.
pol_set brief_interval_minutes 7
brief() { "$RB" brief --repo . --this-checkout "$@" 2>/dev/null || fail "brief $* failed"; }
LAST_OUT="$(brief --line)"
[ "$(printf '%s\n' "$LAST_OUT" | wc -l | tr -d ' ')" = 1 ] || fail "the one-line brief is not one line: $LAST_OUT"
expect_grep '\| peers 0/1b \| mesh off \|'
LAST_OUT="$(brief --keep-awake)"
expect_grep '^Keep yourself awake and reporting \(ADR 0136\): every 7 minutes'
expect_grep '^Now: '
LAST_OUT="$(brief)"
for section in session peers mesh integration derive; do
  expect_grep "^$section +"
done
expect_grep '^answered by  this_process \(no server answers for this checkout'
LAST_OUT="$(brief --format json)"
expect_grep '"interval_minutes": 7'
