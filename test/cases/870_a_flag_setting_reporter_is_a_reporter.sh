# majordomus-covers: none
# A reporter that sets a flag is a reporter (I1967).
#
# shell-lint refuses a call to a reporting function inside a loop a pipe feeds: bash runs that
# loop in a subshell, so what the function records is printed and then lost. The discovery
# found only reporters that count (`fails=$((fails+1))`). scripts/ci/pages-check reports with
# `bad() { ...; fail=1; }`, a flag, so a call to `bad` inside `... | while read` would have set
# the flag in the subshell, lost it, and let the gate exit 0 while printing the failure.
#
# This case holds the three things I1967 asks for, on fixtures written here:
#   1. a function that sets a flag is discovered as a reporter, and a local flag, quoted text,
#      `export` and a non-flag value are not;
#   2. a call to the flag reporter inside a pipeline-fed loop is refused with its line, and the
#      same call in a loop fed by a here-string is not;
#   3. shell-lint's known answer for the discovery fails a discovery that stopped finding flags.
. "$ROOT/test/lib.sh"

CF="$ROOT/scripts/lib/counting-functions.awk"
PC="$ROOT/scripts/lib/pipeline-counts.awk"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj-870.XXXXXX")"
trap 'rm -rf "$S"' EXIT

# 1. discovery
cat > "$S/gate.sh" <<'SH'
bad() { echo "FAIL $*"; fail=1; }
kept() { local fail=0; fail=1; }
words() { echo "fail=1 is words"; }
exported() { export FLAG=1; }
mode() { speed=fast; }
SH
got="$(LC_ALL=C awk -f "$CF" "$S/gate.sh" | tr '\n' ' ' | sed 's/ $//')"
[ "$got" = "bad" ] || { echo "    the discovery found [$got] in a gate whose only reporter is the flag-setting 'bad'"; exit 1; }

# 2. the call inside a pipeline loop is refused with its line; the here-string loop is not
cat >> "$S/gate.sh" <<'SH'
printf x | while read -r l; do bad "$l"; done
while read -r h; do bad "$h"; done <<< "$(awk '{print "x"}' f)"
SH
rc=0; out="$(LC_ALL=C awk -v FNS="$got" -f "$PC" "$S/gate.sh")" || rc=$?
[ "$rc" = 0 ] || { echo "    the scanner could not read the gate to the end: $out"; exit 1; }
lines="$(printf '%s\n' "$out" | sed -E 's#^[^:]*:([0-9]+):.*#\1#' | tr '\n' ' ' | sed 's/ $//')"
[ "$lines" = "6" ] || { echo "    the scanner reported lines [$lines]; the call lost to a subshell is on line 6, and line 7 reads a here-string"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }

# 3. the known answer refuses a discovery that no longer finds flags
fixture="$ROOT/scripts/lib/counting-functions.fixture"
want="$(sed -n 's/^# The known answer for scripts\/lib\/counting-functions.awk is: //p' "$fixture")"
[ -n "$want" ] || { echo "    $fixture names no known answer"; exit 1; }
got="$(LC_ALL=C awk -f "$CF" "$fixture" | tr '\n' ' ' | sed 's/ $//')"
[ "$got" = "$want" ] || { echo "    the discovery found [$got] in its fixture, whose answer is [$want]"; exit 1; }
# a discovery with its flag pass removed: everything after the counter pass's closing loop
awk '/# a flag: quoted text removed first/ { skip = 1 } skip && /^}$/ { skip = 0 } !skip' "$CF" > "$S/blind.awk"
grep -q 'removed first' "$S/blind.awk" && { echo "    the mutation did not remove the flag pass"; exit 1; }
blind="$(LC_ALL=C awk -f "$S/blind.awk" "$fixture" | tr '\n' ' ' | sed 's/ $//')"
[ "$blind" != "$want" ] || { echo "    a discovery that cannot see flags still matches the known answer [$want]: the answer holds no flag"; exit 1; }

echo "    a flag-setting reporter is discovered, its call in a pipeline loop is refused on its line, and a blind discovery fails its known answer"
