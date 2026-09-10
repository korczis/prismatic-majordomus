# majordomus-covers: none
# The design system through the executable: one declaration, every projection, no
# registration. A fixture repository carries the tool's data directory and the crate's
# manifest; `majordomus generate design` writes every projection from
# share/design/tokens.yaml; a role and a state word added to the declaration reach every
# stylesheet, the site's dataset and the inventory in one more run with no consumer edited;
# an inconsistent declaration projects nothing; a hand-edited projection is refused; and the
# capabilities — over MCP, with nothing but data between them and the sheets — answer the
# same fingerprint the sheets carry.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the site cases do for zola.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj109.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?

# --- the fixture: a repository the shell tool wrote, carrying the distribution beside it
# so that the design is *this* tree's (the generator projects the design only where the
# data directory and the crate live in the repository being generated)
"$MJ" init >/dev/null
cp -R "$ROOT/share" share
mkdir -p apps/majordomus-cli && cp "$ROOT/apps/majordomus-cli/Cargo.toml" apps/majordomus-cli/Cargo.toml
git add -A >/dev/null && git commit -qm fixture
gen() { env -u MAJORDOMUS_SHARE "$RB" generate design --repo "$T" "$@"; }

# --- every projection, from one run
expect_exit 0 gen
for f in share/design/theme.css share/design/surface.css share/design/status.css \
         apps/majordomus-cli/src/web/tokens.css apps/majordomus-cli/src/design/tokens.yaml \
         apps/majordomus-cli/src/cockpit/logo-mark.svg share/cockpit/favicon.svg \
         site/static/favicon.svg site/static/images/logo-mark.svg site/static/images/logo.svg \
         site/data/registry/design.json docs/generated/design.json docs/generated/design.yaml docs/generated/design.md; do
  expect_file "$f"
done
# every generated file says so, in the comment form its encoding allows
grep -q '^/\* GENERATED FILE' share/design/theme.css
grep -q '^<!-- GENERATED FILE' share/cockpit/favicon.svg
grep -q '^# GENERATED FILE' apps/majordomus-cli/src/design/tokens.yaml
jq -e '.generated | startswith("GENERATED FILE")' site/data/registry/design.json >/dev/null
# the compiled copy is the declaration, byte for byte, behind its banner
diff <(sed '1,3d' apps/majordomus-cli/src/design/tokens.yaml) share/design/tokens.yaml >/dev/null
# the marks are the canonical mark
diff <(sed '1,3d' share/cockpit/favicon.svg) share/design/brand/logo-mark.svg >/dev/null

# --- one fingerprint, everywhere
fp="$(jq -r '.design' site/data/registry/design.json)"
[ "${#fp}" = 12 ] || { echo "    the short fingerprint is '$fp'"; exit 1; }
grep -q -- "--mj-design: \"$fp\";" share/design/surface.css
grep -q -- "--mj-design: \"$fp\";" apps/majordomus-cli/src/web/tokens.css
[ "$(jq -r '.design' docs/generated/design.json)" = "$fp" ]

# --- the dark block is a class after the root block, never :where()
! grep -q ':where(.dark)' share/design/surface.css
awk '/^  :root \{/{r=NR} /^  \.dark \{/{d=NR} END{exit !(r && d && d > r)}' share/design/surface.css

# --- the site's vocabulary resolves to the roles, after Flowbite's own theme
grep -q -- '--color-heading: var(--mj-fg);' share/design/theme.css
grep -q -- '--color-success-soft: var(--mj-ok-bg);' share/design/theme.css
grep -q -- '^:root, .dark {' share/design/theme.css
# the status sheet files every word
grep -q '\.mj-badge--succeeded' share/design/status.css
grep -q -- '--mj-status-fg: var(--mj-ok);' share/design/status.css
[ "$(jq -r '.states.succeeded' site/data/registry/design.json)" = ok ]
[ "$(jq -r '.states.stale' site/data/registry/design.json)" = warn ]
# the theme contract and the widths are the declaration's
[ "$(jq -r '.theme.storage_key' site/data/registry/design.json)" = color-theme ]
jq -e '.theme.bootstrap | contains("color-theme")' site/data/registry/design.json >/dev/null
[ "$(jq -r '.viewports | length' site/data/registry/design.json)" -ge 3 ]

# the projections are part of the fixture from here on, so that what the next runs change
# is visible as a modification rather than an untracked directory
git add -A >/dev/null && git commit -qm projections
# --- generate --check agrees with what generate wrote, and refuses a hand-edited projection
expect_exit 0 gen --check
printf '/* edited by hand */\n' >> share/design/status.css
expect_exit 10 gen --check
expect_grep 'share/design/status.css'
expect_exit 0 gen
expect_exit 0 gen --check

# --- zero registration: a role and a state word, added to the declaration alone
cat >> share/design/tokens.yaml <<'YAML'
YAML
python3 - share/design/tokens.yaml <<'PY'
import sys, re
p = sys.argv[1]; s = open(p).read()
s = s.replace("  on-accent:\n    about: text on a primary action\n    light: white\n    dark: white\n",
              "  on-accent:\n    about: text on a primary action\n    light: white\n    dark: white\n  overlay:\n    about: a scrim behind a dialog\n    light: gray-900\n    dark: gray-50\n")
s = re.sub(r"(\n    warn: \[)", r"\1experimental, ", s, count=1)
open(p, "w").write(s)
PY
grep -q '^  overlay:' share/design/tokens.yaml
grep -q 'warn: \[experimental,' share/design/tokens.yaml
expect_exit 10 gen --check            # the declaration moved; the projections did not
expect_exit 0 gen
grep -q -- '--mj-overlay: oklch(21% 0.034 264.665);' share/design/surface.css
grep -q -- '--mj-overlay:' apps/majordomus-cli/src/web/tokens.css
grep -q '\.mj-swatch--overlay' share/design/status.css
grep -q '\.mj-badge--experimental' share/design/status.css
grep -q '\.mj-status--experimental' share/design/status.css
[ "$(jq -r '.states.experimental' site/data/registry/design.json)" = warn ]
jq -e '.roles[] | select(.name == "overlay")' site/data/registry/design.json >/dev/null
jq -e '.tokens[] | select(.name == "experimental" and .kind == "state" and .role == "warn")' docs/generated/design.json >/dev/null
grep -q '`--mj-overlay`' docs/generated/design.md
grep -q '`experimental`' docs/generated/design.md
fp2="$(jq -r '.design' site/data/registry/design.json)"
[ "$fp2" != "$fp" ] || { echo "    the fingerprint did not move with the declaration"; exit 1; }
grep -q -- "--mj-design: \"$fp2\";" share/design/surface.css
# nothing outside the declaration was edited to get there: every changed file is the
# declaration or one of its projections, and no untracked file appeared
moved="$(git status --porcelain --untracked-files=all | grep -vE '^ M (share/design/tokens\.yaml|share/design/(theme|surface|status)\.css|apps/majordomus-cli/src/(web/tokens\.css|design/tokens\.yaml)|site/data/registry/design\.json|docs/generated/design\.(json|yaml|md))$' || true)"
[ -z "$moved" ] || { echo "    a consumer was edited to register the token:"; printf '%s\n' "$moved"; exit 1; }

# --- an inconsistent declaration projects nothing
cp share/design/status.css "$S/status.before"
python3 - share/design/tokens.yaml <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
s = s.replace("    bad: [bad,", "    bad: [bad, experimental,", 1)   # filed twice
open(p, "w").write(s)
PY
expect_exit 10 gen
expect_grep "'experimental' is already filed under 'warn'"
cmp -s share/design/status.css "$S/status.before" || { echo "    a refused declaration was projected"; exit 1; }
git checkout -q -- . 2>/dev/null || true
expect_exit 0 gen

# --- the capabilities answer from the declaration the sheets came from
run_quiet "$S/cap.err" "$RB" capabilities describe design.explain --format json > "$S/cap.json"
jq -e '.id == "design.explain" and .exposure.mcp.tool == "majordomus_design_explain" and .exposure.http.path == "/api/v1/design/explain"' "$S/cap.json" >/dev/null \
  || { echo "    design.explain is not declared as expected"; cat "$S/cap.json"; exit 1; }
{
  printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case109","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_design","arguments":{}}}\n'
  printf '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"majordomus_design_explain","arguments":{"token":"succeeded"}}}\n'
  printf '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"majordomus_design_explain","arguments":{"token":"--mj-fg"}}}\n'
  printf '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"majordomus_design_explain","arguments":{"token":"no-such-token"}}}\n'
} > "$S/session.in"
rc=0; MAJORDOMUS_SHARE="$T/share" "$RB" mcp < "$S/session.in" > "$S/session.out" 2>/dev/null || rc=$?
[ "$rc" = 0 ] || { echo "    mcp exited $rc"; exit 1; }
body() { sed -n "${1}p" "$S/session.out" | jq -r '.result.content[0].text'; }
# the executable's own design is the one its stylesheets carry: the fingerprint the MCP tool
# answers is the one compiled in, which is the repository's — the same declaration this
# fixture copied and has just regenerated from unchanged
[ "$(body 2 | jq -r '.design')" = "$(jq -r '.design' site/data/registry/design.json)" ] \
  || { echo "    the executable answers a different design from the one it projected"; body 2; exit 1; }
[ "$(body 3 | jq -r '.role')" = ok ] || { echo "    succeeded is not filed under ok"; body 3; exit 1; }
[ "$(body 4 | jq -r '.kind')" = role ] || { echo "    --mj-fg is not a role"; body 4; exit 1; }
body 4 | jq -e '.aliases | index("--color-heading")' >/dev/null || { echo "    fg is not the site's heading"; body 4; exit 1; }
sed -n 5p "$S/session.out" | jq -e '.result.isError == true' >/dev/null || { echo "    an unknown token was answered"; sed -n 5p "$S/session.out"; exit 1; }
