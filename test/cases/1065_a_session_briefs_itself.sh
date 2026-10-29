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
# own behaviour is the distribution's test: `claude plugin test share/mods/claude-code/majordomus-brief`,
# which case 4250 runs where `claude` is installed.
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
# what is installed reports only a changed fact (I4250), by the brief's own section
# fingerprints, and relays the report the brief composed (I4390); case 4250 runs its test
grep -qF 'changedSections(told, b).length > 0' "$MOD/hooks/register.ts" \
  || fail 'the installed module self-reports whether or not anything changed'
grep -qF 'b.report?.prompt' "$MOD/hooks/register.ts" \
  || fail 'the installed module does not relay report.prompt'
! grep -qF 'Report to the person in two to four lines' "$MOD/hooks/register.ts" \
  || fail 'the installed module still carries the placeholder prompt'
! grep -qF 'two to four lines' "$MOD/hooks/register.ts" \
  || fail 'the installed module states a line count'
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
expect_grep '\| mesh off \| peers 0/1b \|'
LAST_OUT="$(brief --keep-awake)"
expect_grep '^Report on yourself every 7 minutes without being asked \(brief-report/v1 #[0-9a-f]{12}, ADR 0136\)'
expect_grep '/loop 7m /mj-brief'
expect_grep 'A brief in which only times and ages moved is owed no report'
expect_no_grep 'two to four lines'
expect_grep '^Now: '
LAST_OUT="$(brief --report)"
expect_grep '^Majordomus report owed: the brief composed at '
expect_grep '^Rules:$'
expect_no_grep 'two to four lines'
expect_no_grep 'a board that could not be read, a superseded server'
LAST_OUT="$(brief)"
expect_grep '^majordomus brief  [0-9T:-]+Z  this_process'
for section in attention session work peers mesh integration ci machine release derive; do
  expect_grep "^$section +"
done
expect_grep '^answered by  this_process \(no server answers for this checkout'
expect_grep '^ci +unread: not the leaseholder'
LAST_OUT="$(brief --format json)"
expect_grep '"interval_minutes": 7'
expect_grep '"contract": "brief-report/v1"'
printf '%s' "$LAST_OUT" | jq -e '(.attention | type) == "array" and (.report.prompt | startswith("Majordomus report owed")) and (.fingerprints | length) == 9' >/dev/null \
  || fail 'the JSON brief carries no attention array, no report.prompt or not nine section fingerprints'
