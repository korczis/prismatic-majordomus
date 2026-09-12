# majordomus-covers: none
# The gate of project.development-semantics-are-canonical, driven over a fixture tree.
#
# The gate shipped with no case at all, and it spent its life reporting findings nobody
# believed. Its surface finding key ended in a line number, and a line number is the one
# property of a finding that unrelated work changes: every edit above a baselined line
# re-reported already-accepted debt as a new regression. One unchanged line of
# cockpit/pages.rs was simultaneously a regression at :2173 and a stale baseline entry at
# :1787 — the same text, two keys, two false verdicts from one true finding.
#
# So the property under test is not "the gate finds the line". It is that the key survives
# the file moving underneath it, and that the baseline writer cannot launder a live
# regression into accepted debt.
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/development-semantics-check"

# ---------------------------------------------------------------- wired into the CI model
expect_grep 'id: development-semantics' "$ROOT/.ai/repo/ci/gates.yaml"

# ---------------------------------------------------------------- a fixture tree
# The gate reads three things: the command declarations, the registry projection, and the
# tracked Cockpit files. MJ_ROOT says which tree they come from.
"$MJ" init >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true

mkdir -p share docs/generated apps/majordomus-cli/src/cockpit
cat > share/commands.yaml <<'YAML'
commands:
  - id: backed
    visibility: public
    class: state-mutating
    writes: [.ai/repo/sessions/]
  - id: unbacked
    visibility: public
    class: state-mutating
    writes: [.ai/repo/sessions/]
  - id: reader
    visibility: public
    class: read-only
    writes: []
YAML
cat > docs/generated/registry.json <<'JSON'
{"capabilities": [{"module": "backed", "kind": "command"},
                  {"module": "unbacked", "kind": "query"}]}
JSON

# One offending line, and one innocent line that merely mentions the word.
cockpit_file() {
  { printf '%s\n' "$1"
    printf 'fn render() { statistic(n, k, ".ai/local/state") }\n'
    printf 'fn other() { let _ = "nothing to see"; }\n'
  } > apps/majordomus-cli/src/cockpit/pages.rs
  git add apps/majordomus-cli/src/cockpit/pages.rs >/dev/null 2>&1
}
gate() { MJ_ROOT="$PWD" "$GATE" "$@"; }

# ---------------------------------------------------------------- the key is not a coordinate
# The offending line at the top of the file...
cockpit_file '// header'
expect_exit 10 gate
expect_grep 'surface:apps/majordomus-cli/src/cockpit/pages\.rs:\.ai/local/state:[0-9a-f]{8}'
key_high="$(printf '%s\n' "$LAST_OUT" | sed -n 's/.*\(surface:[^ ]*\).*/\1/p' | head -n 1)"
[ -n "$key_high" ] || { echo "    the gate reported no surface key"; exit 1; }

# ...and the same line pushed down by unrelated edits above it. This is the whole defect:
# nothing about the finding changed, so nothing about its key may change.
cockpit_file '// header
// a paragraph of unrelated commentary
// added above the offending line
// by a session working on something else'
expect_exit 10 gate
key_low="$(printf '%s\n' "$LAST_OUT" | sed -n 's/.*\(surface:[^ ]*\).*/\1/p' | head -n 1)"
[ "$key_high" = "$key_low" ] || {
  printf '    the key moved with the line: %s then %s\n' "$key_high" "$key_low"; exit 1; }

# Reindentation is movement too, and must not move the key either.
cockpit_file '// header'
sed 's/^fn render/      fn render/' apps/majordomus-cli/src/cockpit/pages.rs > "$T/p.$$" \
  && mv "$T/p.$$" apps/majordomus-cli/src/cockpit/pages.rs
git add apps/majordomus-cli/src/cockpit/pages.rs >/dev/null 2>&1
expect_exit 10 gate
key_indented="$(printf '%s\n' "$LAST_OUT" | sed -n 's/.*\(surface:[^ ]*\).*/\1/p' | head -n 1)"
[ "$key_high" = "$key_indented" ] || {
  printf '    reindenting moved the key: %s then %s\n' "$key_high" "$key_indented"; exit 1; }

# A different offending line is a different finding, so the key still discriminates: a
# stable key that collapsed every finding in a file into one would hide new debt.
cockpit_file '// header
fn second() { statistic(n, k, ".ai/repo/sessions") }'
expect_exit 10 gate
n_surface="$(printf '%s\n' "$LAST_OUT" | grep -cE 'FAIL +surface:')"
[ "$n_surface" = 2 ] || { printf '    expected two distinct surface findings, got %s\n' "$n_surface"; exit 1; }
expect_grep 'surface:[^ ]*:\.ai/repo/sessions:[0-9a-f]{8}'

# ---------------------------------------------------------------- backing
cockpit_file '// header'
expect_exit 10 gate
expect_grep 'FAIL +backing:unbacked'      # declared a query, so it mutates nothing typed
expect_no_grep 'backing:backed'           # has a mutating capability
expect_no_grep 'backing:reader'           # read-only commands are owed nothing

# ---------------------------------------------------------------- the baseline writer
BASELINE=.ai/repo/development-semantics-baseline.txt

# Regenerating does not launder a live regression into accepted debt. This was the hole:
# "may shrink and may not grow" was a sentence in a comment, while --baseline wrote back
# everything it had just found, so one command turned any red green.
rm -f "$BASELINE"
expect_exit 0 gate --baseline
expect_grep 'NOT accepted'
expect_grep 'FAIL +backing:unbacked'
expect_no_grep '^backing:unbacked$' "$BASELINE"
expect_exit 10 gate

# Accepting is a separate act that names the key.
expect_exit 0 gate --baseline --accept backing:unbacked
expect_grep '^backing:unbacked$' "$BASELINE"
expect_exit 10 gate                        # the surface finding is still open
expect_no_grep 'backing:unbacked' -

# ...and with the surface finding accepted too, the tree is clean.
expect_exit 0 gate --baseline --accept "$key_high"
expect_exit 0 gate

# A closed finding is retired by regenerating, and the baseline shrinks without --accept.
printf 'fn render() { statistic(n, k, "the local state") }\n' \
  > apps/majordomus-cli/src/cockpit/pages.rs
git add apps/majordomus-cli/src/cockpit/pages.rs >/dev/null 2>&1
expect_exit 10 gate
expect_grep 'STALE'
expect_exit 0 gate --baseline
expect_no_grep '^surface:' "$BASELINE"
expect_grep '^backing:unbacked$' "$BASELINE"
expect_exit 0 gate
