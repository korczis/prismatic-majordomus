# The built site (when zola is present): routes, sections, derived content, prefix, mermaid, and site-check.
. "$ROOT/test/lib.sh"
command -v zola >/dev/null || { echo "    zola absent; skipping build test"; exit 0; }
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
[ -d "$ROOT/node_modules/tailwindcss" ] || { echo "    node_modules absent; skipping build test"; exit 0; }
expect_exit 0 "$ROOT/scripts/site-build"
P="$ROOT/site/public"
for r in "" why why/two-agents-one-bug getting-started concepts profiles profiles/debugging policy guarantees guarantees/guaranteed guarantees/init-refuses supervises supervises/finish commands commands/doctor architecture docs docs/cli docs/design render-test; do [ -f "$P/$r/index.html" ] || { echo "    missing route /$r/"; exit 1; }; done
# homepage sections come from data, not templates
expect_grep 'Most AI-assisted work fails for boring reasons' "$P/index.html"
expect_grep "v$(jq -r .version "$ROOT/site/data/generated/project.json")" "$P/index.html"
for p in $(jq -r '.profiles[].slug' "$ROOT/site/data/generated/profiles.json"); do expect_grep "$p" "$P/profiles/index.html"; done
for pr in $(jq -r '.principles[0]' "$ROOT/site/data/generated/lifecycle.json" | cut -d' ' -f1-3); do expect_grep "$pr" "$P/index.html"; done
expect_grep 'class="mermaid' "$P/index.html"; expect_grep 'js/mermaid.min.js' "$P/index.html"
expect_grep 'stateDiagram-v2' "$P/getting-started/index.html"
# docs pages: typography container, syntax classes, anchors, wrapped tables, footnote, callout, fenced mermaid
expect_grep 'class="format' "$P/docs/cli/index.html"
expect_grep 'class="z-[a-z]' "$P/render-test/index.html"   # Zola/giallo class-based highlighting
expect_grep 'id="second-level-heading"' "$P/render-test/index.html"
expect_grep 'overflow-x-auto' "$P/render-test/index.html"
expect_grep 'role="note"' "$P/render-test/index.html"
expect_grep 'footnote' "$P/render-test/index.html"
expect_grep '<pre class="mermaid">' "$P/render-test/index.html"
# every homepage tile is a link to a page that exists
for href in $(grep -oE 'href="[^"]*/(supervises|profiles|guarantees|commands|why)/[a-z0-9_-]+/"' "$P/index.html" | sed -E 's#.*/prismatic-majordomus/##; s#"$##' | sort -u); do [ -f "$P/$href/index.html" ] || { echo "    homepage tile links to missing $href"; exit 1; }; done
[ "$(grep -oE 'href="[^"]*/supervises/[a-z]+/"' "$P/index.html" | sort -u | wc -l | tr -d ' ')" = "$(jq '.does | length' "$ROOT/site/data/generated/readme.json")" ]
[ "$(grep -oE 'href="[^"]*/why/[a-z-]+/"' "$P/index.html" | sort -u | wc -l | tr -d ' ')" = "$(ls "$ROOT"/site/content-src/why/*.md | grep -vc _index)" ]
# guarantees table rows == claims
[ "$(grep -o '<tr class="align-top">' "$P/guarantees/index.html" | wc -l | tr -d ' ')" = "$(jq '.claims | length' "$ROOT/site/data/generated/capabilities.json")" ]
# the full check passes
expect_exit 0 "$ROOT/scripts/site-check"
