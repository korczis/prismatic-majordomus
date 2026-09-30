# majordomus-covers: none
# A verdict assembled from a checker's output needs a verdict for no output.
#
# scripts/site-check delegates two of its checks and relays what they print: `OK …` becomes an
# ok, `FAIL …` a failure, the closing summary counts when the run could not decide, and any
# other line counts when the run failed. Every one of those branches asks "is this line bad?",
# and every one of them needs a line to exist. A checker that dies before printing anything —
# a crash, a missing interpreter, a new code path that exits early — leaves a non-zero status
# and an empty file: the loop never runs, nothing is recorded, and the section reports as
# passed. Silence read as success is how an outage hides in a green check.
#
# It is the same verdict lost that #351 fixed from the other side, where `bad` ran inside a
# subshell of a pipeline and sixteen findings printed while the script exited 0. Printed and
# discarded, or never printed at all: either way the caller is told all is well.
#
# The relay was also written twice, once per checker. Both now go through one function,
# `relay_check` in scripts/site-check, and this case drives that definition as the script
# carries it — lifted out of the file, not copied — with the caller's own `ok`, `bad` and counter.
# The third delegated checker, scripts/ci/homepage-check, relays its lines verbatim in a loop of
# its own; section 5 lifts that block out the same way and holds it to the same verdict.
. "$ROOT/test/lib.sh"

SC="$ROOT/scripts/site-check"
[ "$(grep -c '^relay_check ' "$SC")" -ge 2 ] \
  || { echo "    site-check does not relay both delegated checkers through relay_check; a second copy of the verdict remains"; exit 1; }
awk '/^relay_check\(\) \{$/ { on = 1 } on { print } on && /^\}$/ { exit }' "$SC" > "$T/relay.sh"
grep -q '^}$' "$T/relay.sh" || { echo "    no relay_check() definition found in scripts/site-check"; exit 1; }

# the caller's half, as site-check declares it
fails=0
ok()  { printf 'OK   %-12s %s\n' "$1" "$2"; }
bad() { printf 'FAIL %-12s %s\n' "$1" "$2"; fails=$((fails+1)); }
# shellcheck source=/dev/null
. "$T/relay.sh"

mkdir -p "$T/ci"
stub() { printf '%s\n' "$2" > "$T/ci/$1"; chmod +x "$T/ci/$1"; }
stub silent-check    '#!/bin/sh
exit 12'
stub speaking-check  '#!/bin/sh
echo "speaking-check: cannot decide in a shallow clone"
exit 12'
stub clean-check     '#!/bin/sh
echo "OK   links        every link resolves"
exit 0'
stub finding-check   '#!/bin/sh
echo "FAIL a link points at nothing"
exit 10'

relay() { fails=0; relay_check "$1" "$T/ci/$2" "OK   links        " "FAIL " > "$T/out.txt" 2>&1; }

# ---------------------------------------------------------------- 1. silence is a failure
relay link silent-check
[ "$fails" = 1 ] || { echo "    a checker that exited 12 without a word recorded $fails failure(s), not 1"; cat "$T/out.txt"; exit 1; }
grep -q 'exited 12 and printed nothing' "$T/out.txt" \
  || { echo "    the failure does not say what happened:"; sed 's/^/    | /' "$T/out.txt"; exit 1; }
echo "    a checker that exits without a word is a failure, not a pass"

# ---------------------------------------------------------------- 2. a refusal that speaks is
#     counted once, by its own line, and not again for silence
relay link speaking-check
[ "$fails" = 1 ] || { echo "    a refusal that named itself recorded $fails failure(s), not 1"; cat "$T/out.txt"; exit 1; }
rc=0; grep -q 'printed nothing' "$T/out.txt" || rc=$?
[ "$rc" = 1 ] || { echo "    a checker that spoke was also counted as silent"; exit 1; }
echo "    a refusal that names itself is counted once"

# ---------------------------------------------------------------- 3. a clean run stays clean
relay link clean-check
[ "$fails" = 0 ] || { echo "    a checker that passed recorded $fails failure(s)"; cat "$T/out.txt"; exit 1; }
grep -q '^OK   link *every link resolves$' "$T/out.txt" \
  || { echo "    the passing checker's line was not relayed with its prefix stripped"; cat "$T/out.txt"; exit 1; }
echo "    a checker that passes is relayed as a pass"

# ---------------------------------------------------------------- 4. a finding is a finding,
#     and the silence verdict does not double it
relay link finding-check
[ "$fails" = 1 ] || { echo "    a finding recorded $fails failure(s), not 1"; cat "$T/out.txt"; exit 1; }
rc=0; grep -q 'printed nothing' "$T/out.txt" || rc=$?
[ "$rc" = 1 ] || { echo "    a checker that reported a finding was also counted as silent"; exit 1; }
echo "    a finding is counted once, and silence is not added to it"

# ---------------------------------------------------------------- 5. the homepage relay, which
#     keeps its own loop, owes the same verdict for silence
awk '/^HOMEROWS=/ { on = 1 } on { print } on && /printed nothing/ { exit }' "$SC" > "$T/home.sh"
grep -q 'homepage-check' "$T/home.sh" && grep -q 'printed nothing' "$T/home.sh" \
  || { echo "    the homepage-check relay in scripts/site-check has no verdict for silence"; exit 1; }
mkdir -p "$T/fake/scripts/ci"
home() { printf '%s\n' "$1" > "$T/fake/scripts/ci/homepage-check"; chmod +x "$T/fake/scripts/ci/homepage-check"
  fails=0; ( ROOT="$T/fake"; . "$T/home.sh"; echo "fails=$fails" ) > "$T/out.txt" 2>&1; }
home '#!/bin/sh
exit 12'
grep -q '^fails=1$' "$T/out.txt" && grep -q 'homepage-check exited 12 and printed nothing' "$T/out.txt" \
  || { echo "    a silent homepage-check was not counted as a failure:"; sed 's/^/    | /' "$T/out.txt"; exit 1; }
home '#!/bin/sh
echo "OK   homepage     the story is in its declared order"
exit 0'
grep -q '^fails=0$' "$T/out.txt" || { echo "    a passing homepage-check was counted as a failure:"; sed 's/^/    | /' "$T/out.txt"; exit 1; }
home '#!/bin/sh
echo "FAIL homepage     a section is out of order"
exit 10'
grep -q '^fails=1$' "$T/out.txt" || { echo "    a homepage finding was not counted exactly once:"; sed 's/^/    | /' "$T/out.txt"; exit 1; }
echo "    the homepage relay counts silence as a failure, and a pass or a finding as itself"
