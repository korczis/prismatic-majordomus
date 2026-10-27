# majordomus-covers: none
# claim: session-lifecycle
# A session is told the brief only when a fact in it changed (I4250, ADR 0136 amended
# 2026-10-27).
#
# On the night of 2026-10-26/27 the majordomus-brief mod woke an idle progresus-isir session
# about 40 times with self-reports that differed only by their `at` and by relative ages
# ("observed 4 d ago"). The rule that ends it lives in the mod
# (share/mods/claude-code/majordomus-brief/hooks/register.ts), and the mod's own test holds it
# against Claude Code's engine. Nothing ran that test but a person who remembered to, so this
# case runs it, on a copy, where `claude` is installed, and declines where it is not.
#
# What it proves:
#   1  the mod's own test passes
#   2  the test fails when the comparison of fingerprints is replaced by `true`, so a self-report
#      on every interval again, whatever changed, cannot pass unseen
#   3  it fails when the clock is no longer taken out of the brief
. "$ROOT/test/lib.sh"
command -v claude >/dev/null 2>&1 || skip "no claude on PATH: the mod's test runs only in Claude Code's engine"

SRC="$ROOT/share/mods/claude-code/majordomus-brief"
fail() { printf '    %s\n' "$*"; exit 1; }
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
