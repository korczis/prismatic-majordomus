# majordomus-covers: none
# claim: session-lifecycle
# A session is told the brief only when a fact in it changed (I4250, ADR 0136 amended
# 2026-10-27).
#
# On the night of 2026-10-26/27 the majordomus-brief mod woke an idle session in an adopter repository
# about 40 times with self-reports that differed only by their `at` and by relative ages
# ("observed 4 d ago"). The rule that ends it lives in the mod
# (share/mods/claude-code/majordomus-brief/hooks/register.ts), and the mod's own test holds it
# against Claude Code's engine. Nothing ran that test but a person who remembered to, so this
# case runs it, on a copy, where `claude` is installed.
#
# What it proves:
#   0  everywhere: each mutation below still applies to the module, and the test it must fail
#      still exists, so the case keeps testing something wherever the engine is
#   1  the mod's own test passes
#   2  the test fails when the comparison of fingerprints is replaced by `true`, so a self-report
#      on every interval again, whatever changed, cannot pass unseen
#   3  it fails when the clock is no longer taken out of the brief
#
# 1-3 need Claude Code's engine. Where there is no `claude` on PATH (CI) the case holds 0 and
# ends there, as case 2980 does, rather than declining under --no-skips; MJ_CLAUDE_E2E=1 makes a
# missing `claude` a failure.
. "$ROOT/test/lib.sh"

SRC="$ROOT/share/mods/claude-code/majordomus-brief"
fail() { printf '    %s\n' "$*"; exit 1; }

# ---------------------------------------------------------------- 0  the mutations still apply
grep -qF 'fingerprint(b) !== fingerprint(told)' "$SRC/hooks/register.ts" \
  || fail "the fingerprint comparison is no longer in the module: mutation 2 tests nothing"
grep -qF ".replace(DURATION, '<age>')" "$SRC/hooks/register.ts" \
  || fail "the clock is no longer taken out in the module: mutation 3 tests nothing"
grep -qF "test('an idle session whose surroundings only aged is never woken'" \
  "$SRC/hooks/register.test.ts" || fail "the mod's test no longer holds the quiet session"
if ! command -v claude >/dev/null 2>&1; then
  [ "${MJ_CLAUDE_E2E:-}" != 1 ] || fail "MJ_CLAUDE_E2E=1, and there is no claude on PATH"
  exit 0
fi
# A copy of the mod, so that the engine's laid type declarations never land in the checkout.
copy() {
  rm -rf "$T/mod" && mkdir -p "$T/mod" && cp -R "$SRC/." "$T/mod/" && rm -rf "$T/mod/.claude-plugin/types"
}
# Replace the first occurrence of $1 with $2 in the copy's hooks module, or fail: a mutation
# that no longer applies is a case that no longer tests anything.
mutate() {
  node -e '
    const fs = require("fs"); const [p, a, b] = process.argv.slice(1);
    const s = fs.readFileSync(p, "utf8");
    if (!s.includes(a)) { console.error("not in the module: " + a); process.exit(1); }
    fs.writeFileSync(p, s.replace(a, b));
  ' "$T/mod/hooks/register.ts" "$1" "$2" || fail "the mutation does not apply: $1"
}
plugin_test() { env -u MAJORDOMUS_SHARE claude plugin test "$T/mod" > "$T/out" 2>&1; }

# ---------------------------------------------------------------- 1  the mod's test passes
copy
plugin_test || { cat "$T/out"; fail "the mod's own test fails"; }

# ---------------------------------------------------------------- 2  the dedupe is held
copy
mutate 'fingerprint(b) !== fingerprint(told)' 'true'
if plugin_test; then fail 'a self-report on every interval, changed or not, passes the mod'"'"'s test'; fi
grep -q 'an idle session whose surroundings only aged is never woken' "$T/out" \
  || fail "the mutation failed the test, but not where the quiet session is held: $(tail -5 "$T/out")"

# ---------------------------------------------------------------- 3  the clock is taken out
copy
mutate ".replace(DURATION, '<age>')" ''
if plugin_test; then fail 'a brief whose ages count as facts passes the mod'"'"'s test'; fi
exit 0
