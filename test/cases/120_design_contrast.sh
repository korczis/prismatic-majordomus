# majordomus-covers: none
# Contrast, through the executable: the declared colours are measured, the pairs are the
# primitives' and not a list, and a pair that cannot be read is refused by the gate.
#
# ADR 0036 pairs a foreground with a ground in two themes and nothing measured one of them.
# The measurement is crate::design::contrast, answered by `design.contrast`; this case asks
# it the way a client does — one MCP session — over a fixture that carries the distribution,
# so that the stylesheets the derivation reads are files this case can edit. What it proves
# is that the pair set follows those stylesheets: a rule added to a primitive is measured
# with nothing registered anywhere, and a rule that puts a colour on a ground it cannot be
# read on is a finding that names the role, the ground, the theme and both ratios — and
# scripts/ci/design-check refuses it.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj111.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?

# --- the fixture: a repository carrying the distribution and the crate's manifest, so that
# the generator writes every projection the gate's other questions ask about
"$MJ" init >/dev/null
cp -R "$ROOT/share" share
mkdir -p apps/majordomus-cli && cp "$ROOT/apps/majordomus-cli/Cargo.toml" apps/majordomus-cli/Cargo.toml
run_quiet "$S/generate.err" env -u MAJORDOMUS_SHARE "$RB" generate design --repo "$T" >/dev/null
git add -A >/dev/null && git commit -qm fixture

# The report as one JSON document. `design.contrast` is exposed over MCP and HTTP and not on
# the command line, so this is how a client reaches it; --standalone so that no shared server
# of this machine is touched and nothing is written anywhere.
contrast() {
  {
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case111","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_design_contrast","arguments":{}}}\n'
  } | env -u MAJORDOMUS_SHARE "$RB" mcp --standalone --repo "$T" 2>/dev/null \
    | sed -n '2p' | jq -r '.result.content[0].text'
}
gate() { env MAJORDOMUS_BIN="$RB" bash "$ROOT/scripts/ci/design-check" --root "$T"; }

# --- the threshold is the one this repository states, and the declaration reaches it
contrast > "$S/report.json"
jq -e '.standard == "WCAG 2.1 AA" and .minimum_text == 4.5 and .minimum_non_text == 3' "$S/report.json" >/dev/null \
  || { echo "    the threshold is not the one the repository states"; cat "$S/report.json"; exit 1; }
jq -e '.readable == true and (.findings | length) == 0' "$S/report.json" >/dev/null \
  || { echo "    the declaration pairs colours that cannot be read:"; jq -r '.findings[]' "$S/report.json"; exit 1; }
jq -e '.measured > 40' "$S/report.json" >/dev/null
# both themes, and every pair says what it resolved to and where it was read
jq -e '[.pairs[] | select(.theme == "light")] | length > 0' "$S/report.json" >/dev/null
jq -e '[.pairs[] | select(.theme == "dark")] | length > 0' "$S/report.json" >/dev/null
jq -e '[.pairs[] | select((.seen | length) == 0 or (.foreground_entry | length) == 0)] | length == 0' "$S/report.json" >/dev/null
# the pairs the declaration is explicit about: a primary action's label on its fill, and a
# status's text on its own ground — in both themes
jq -e '[.pairs[] | select(.foreground == "on-accent" and .ground == "accent-fill")] | length == 2' "$S/report.json" >/dev/null
jq -e '[.pairs[] | select(.foreground == "ok" and .ground == "ok-bg")] | length == 2' "$S/report.json" >/dev/null
# and never a status on another status's ground: the badge sets all three from one status
jq -e '[.pairs[] | select(.foreground == "ok" and .ground == "bad-bg")] | length == 0' "$S/report.json" >/dev/null
# a border is measured against the non-text threshold and is not a finding
jq -e '[.pairs[] | select(.carries == "non_text" and .enforced)] | length == 0' "$S/report.json" >/dev/null
jq -e '[.pairs[] | select(.carries == "non_text" and .required == 3)] | length > 0' "$S/report.json" >/dev/null
# the gate asks the same question and says how many pairs it measured
expect_exit 0 gate
expect_grep 'pair\(s\) measured in two themes'

# --- the pair set is the primitives', not a list: one rule, and the pair is measured
printf '.mj-case111 { color: var(--mj-fg); background: var(--mj-accent-soft); }\n' >> share/design/primitives.css
contrast > "$S/added.json"
jq -e '[.pairs[] | select(.foreground == "fg" and .ground == "accent-soft")] | length == 2' "$S/added.json" >/dev/null \
  || { echo "    a pair a primitive states was not measured"; exit 1; }
# nothing was registered to get there: the stylesheet is the only file that moved
moved="$(git status --porcelain --untracked-files=all)"
[ "$moved" = " M share/design/primitives.css" ] \
  || { echo "    something was registered to measure a pair:"; printf '%s\n' "$moved"; exit 1; }

# --- a pair that cannot be read is named precisely, and the gate refuses it
printf '.mj-case111-unreadable { color: var(--mj-faint); background: var(--mj-line); }\n' >> share/design/primitives.css
contrast > "$S/unreadable.json"
jq -e '.readable == false' "$S/unreadable.json" >/dev/null \
  || { echo "    an unreadable pair was not reported"; exit 1; }
finding="$(jq -r '.findings[] | select(contains("faint") and contains("on line"))' "$S/unreadable.json" | head -1)"
[ -n "$finding" ] || { echo "    the finding does not name the pair:"; jq -r '.findings[]' "$S/unreadable.json"; exit 1; }
for part in faint line light WCAG 4.5 primitives.css; do
  case "$finding" in *"$part"*) ;; *) echo "    the finding does not name $part: $finding"; exit 1 ;; esac
done
git add -A >/dev/null
expect_exit 10 gate
expect_grep 'faint'
expect_grep 'WCAG 2.1 AA'

# --- with the rules gone, the gate is clean again (from HEAD: the index carries them too)
git checkout -q HEAD -- share/design/primitives.css
expect_exit 0 gate
