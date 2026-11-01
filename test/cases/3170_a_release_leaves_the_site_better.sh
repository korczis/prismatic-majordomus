# A release leaves the site better than it found it (project.a-release-leaves-the-site-better),
# against the real executable in a repository with a recorded release and a real history.
#
# The owner's order of 2026-10-16: every release leaves the published site better, nicer and
# more derived than the release before it. `majordomus release site` is the verdict and the
# release workflow refuses a tag that fails it, so this case proves the verdict says the right
# thing for each kind of window, each one a commit of one linear history judged with --head:
#
#   a window with no site change                      refused, site-not-improved
#   a window whose only site change is a hand edit    refused, naming the commit as generated
#     of generated output                               output
#   a window with an improvement of a site source     passes
#   a canonical record whose commit names the site    passes
#   a measure that regressed                          refused, naming the measure and its delta
#   a commit no recorded release precedes             exit 12, never a pass
#
# The repository is built here, because every question is about a history and a fixture whose
# history is real is the only thing those answers can be checked against. The design it is
# measured with is the checkout's own, copied, so the contrast measures are taken on what ships.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3170.XXXXXX")"
trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm 'chore: install'
C0="$(git rev-parse HEAD)"

# ---------------------------------------------------------------- the declarations
#
# The two the verdict reads: the CI model's classes, which say which paths are the site's own
# sources, and the publication model's `release` section, which names them and the scopes and
# types that count. And the attribute that says which files are generated.
mkdir -p .ai/repo/ci .ai/repo/releases share/design share/cockpit/src site/content/docs \
  site/content-src site/templates src docs
cat > .ai/repo/ci/gates.yaml <<'Y'
version: 1
gates:
  - id: site-build
    job: site
    runs: "true"
    summary: the site, built
classes:
  - id: site
    paths: [site/**]
    gates: [site-build]
  - id: design
    paths: [share/design/**]
    gates: [site-build]
Y
cat > .ai/repo/ci/pages.yaml <<'Y'
version: 1
release:
  classes:
    - site
    - design
  scopes:
    - site
  kinds:
    - feat
    - fix
    - docs
Y
printf 'site/content/** merge=derived\n' > .gitattributes
cp "$ROOT/share/design/"*.css "$ROOT/share/design/tokens.yaml" share/design/
cp "$ROOT/share/cockpit/src/cockpit.css" share/cockpit/src/
printf -- '---\ntitle: generated\n---\nA page the generator wrote.\n' > site/content/docs/a.md
printf -- '---\ntitle: by hand\n---\nA page written by hand.\n' > site/content-src/b.md
printf '<main>{{ page.content }}</main>\n' > site/templates/page.html
git add -A >/dev/null && git commit -qm 'feat(site): the first site'
C1="$(git rev-parse HEAD)"

cat > .ai/repo/releases/v0.1.0.yaml <<Y
schema: release/v1
version: "0.1.0"
tag: v0.1.0
channel: stable
commit: $C1
published_at: "2026-10-16T00:00:00Z"
artifacts: []
Y
git add -A >/dev/null && git commit -qm 'chore(release): record v0.1.0'

# verdict <head> [--since REF] -> $S/out (text), $S/json, and the exit code in $code
verdict() {
  local head="$1"; shift
  set +e
  "$RB" release site --head "$head" "$@" > "$S/out" 2>&1
  code=$?
  "$RB" release site --head "$head" "$@" --format json > "$S/json" 2>/dev/null
  set -e
}

# ---------------------------------------------------------------- no site change
echo "  a window with no site change is refused, and says why"
printf 'fn main() {}\n' > src/main.rs
git add -A >/dev/null && git commit -qm 'feat(cli): something that is not the site'
C2="$(git rev-parse HEAD)"
verdict "$C2"
[ "$code" = 10 ] || { echo "    exit $code, not 10:"; sed 's/^/      /' "$S/out"; exit 1; }
grep -q 'FAIL site-not-improved: none of the 2 commits since v0.1.0 touches the site' "$S/out" \
  || { echo "    the finding does not say no commit touched the site:"; sed 's/^/      /' "$S/out"; exit 1; }
[ "$(jq -r '.previous.tag' "$S/json")" = v0.1.0 ] \
  || { echo "    the baseline is not the recorded release v0.1.0"; cat "$S/json"; exit 1; }
[ "$(jq -r '.passes' "$S/json")" = false ] || { echo "    the JSON verdict passes"; exit 1; }

# ---------------------------------------------------------------- a hand edit of generated output
echo "  a window whose only site change is a hand edit of generated output is refused, naming it"
printf 'A typo, fixed by hand in the generated page.\n' >> site/content/docs/a.md
git add -A >/dev/null && git commit -qm 'docs(site): fix a typo in the generated page'
C3="$(git rev-parse HEAD)"
verdict "$C3"
[ "$code" = 10 ] || { echo "    exit $code, not 10:"; sed 's/^/      /' "$S/out"; exit 1; }
grep -q 'site-not-improved' "$S/out" || { echo "    no site-not-improved finding:"; sed 's/^/      /' "$S/out"; exit 1; }
grep -q "${C3:0:9} docs(site): fix a typo in the generated page — changes only generated output" "$S/out" \
  || { echo "    the hand edit is not named as generated output:"; sed 's/^/      /' "$S/out"; exit 1; }
[ "$(jq -r '.changes[0].generated[0]' "$S/json")" = site/content/docs/a.md ] \
  || { echo "    the change does not carry the generated file it edited"; cat "$S/json"; exit 1; }

# ---------------------------------------------------------------- an improvement of a site source
echo "  a window with an improvement of a site source passes, every measure held"
printf '<main class="page">{{ page.content }}</main>\n' > site/templates/page.html
git add -A >/dev/null && git commit -qm 'fix(site): the template marks its page'
C4="$(git rev-parse HEAD)"
verdict "$C4"
[ "$code" = 0 ] || { echo "    exit $code, not 0:"; sed 's/^/      /' "$S/out"; exit 1; }
grep -q "improves    ${C4:0:9} fix(site): the template marks its page" "$S/out" \
  || { echo "    the improvement is not named:"; sed 's/^/      /' "$S/out"; exit 1; }
[ "$(jq -r '.improvements' "$S/json")" = 1 ] || { echo "    not exactly one improvement"; cat "$S/json"; exit 1; }
for m in contrast.light.failing contrast.light.weakest contrast.dark.failing contrast.dark.weakest site.pages-by-hand; do
  [ "$(jq -r --arg m "$m" '.measures[] | select(.id == $m) | .verdict' "$S/json")" = held ] \
    || { echo "    the measure $m did not hold:"; sed 's/^/      /' "$S/out"; exit 1; }
done
[ "$(jq -r '.measures[] | select(.id == "site.pages-by-hand") | .current == 1' "$S/json")" = true ] \
  || { echo "    the page written by hand is not counted once"; cat "$S/json"; exit 1; }
# the same verdict over MCP, as a client asks it: one session against this fixture, alone,
# binding no port and writing no lease, and the typed answer of the tools/call or nothing
mcp="$( { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3170","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_release_site","arguments":{"head":"%s"}}}\n' "$C4"
} | "$RB" mcp --standalone --repo "$PWD" 2>"$S/mcp.err" | sed -n 2p | jq -c '.result.structuredContent // empty')"
[ -n "$mcp" ] || { echo "    majordomus_release_site gave no typed answer over MCP:"; sed 's/^/      /' "$S/mcp.err" | head -5; exit 1; }
[ "$(printf '%s' "$mcp" | jq -r '.passes')" = true ] && [ "$(printf '%s' "$mcp" | jq -r '.improvements')" = 1 ] \
  || { echo "    over MCP the tool does not answer what the command does: $mcp"; exit 1; }
echo "  asked over MCP, majordomus_release_site gives the command's verdict: it passes, one improvement"

# ---------------------------------------------------------------- a canonical record that names the site
echo "  a canonical record whose commit names the site counts, outside the site's own paths"
printf '# A document the site renders\n' > docs/SITE.md
git add -A >/dev/null && git commit -qm 'docs(site): a document the site renders'
C5="$(git rev-parse HEAD)"
verdict "$C5" --since "$C4"
[ "$code" = 0 ] || { echo "    exit $code, not 0:"; sed 's/^/      /' "$S/out"; exit 1; }
[ "$(jq -r '.changes[0].authored[0]' "$S/json")" = docs/SITE.md ] \
  || { echo "    the canonical document is not what counted"; cat "$S/json"; exit 1; }

# ---------------------------------------------------------------- a regression
echo "  a measure that regressed refuses the release, naming the measure and its delta"
printf -- '---\ntitle: another\n---\nAnother page written by hand.\n' > site/content-src/c.md
git add -A >/dev/null && git commit -qm 'feat(site): another page written by hand'
C6="$(git rev-parse HEAD)"
verdict "$C6"
[ "$code" = 10 ] || { echo "    exit $code, not 10:"; sed 's/^/      /' "$S/out"; exit 1; }
grep -q 'FAIL site-measure-regressed: site.pages-by-hand (site page files written by hand rather than generated) went from 1 to 2 (+1)' "$S/out" \
  || { echo "    the regression is not named with its delta:"; sed 's/^/      /' "$S/out"; exit 1; }
grep -q 'site-not-improved' "$S/out" && { echo "    a window with improvements was called unimproved:"; sed 's/^/      /' "$S/out"; exit 1; }

# ---------------------------------------------------------------- nothing to compare with
echo "  a commit no recorded release precedes has nothing to compare with: exit 12, not a pass"
verdict "$C0"
[ "$code" = 12 ] || { echo "    exit $code, not 12:"; sed 's/^/      /' "$S/out"; exit 1; }
grep -q 'no recorded stable release is an ancestor' "$S/out" \
  || { echo "    the refusal does not say why:"; sed 's/^/      /' "$S/out"; exit 1; }
