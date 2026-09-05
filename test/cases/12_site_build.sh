# majordomus-exclusive: builds the site into site/public of this checkout and runs site-check over it
# The built site (when zola is present): routes, sections, derived content, prefix, mermaid, and site-check.
. "$ROOT/test/lib.sh"
command -v zola >/dev/null || { echo "    zola absent; skipping build test"; exit 0; }
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
[ -d "$ROOT/node_modules/tailwindcss" ] || { echo "    node_modules absent; skipping build test"; exit 0; }
expect_exit 0 "$ROOT/scripts/site-build"
P="$ROOT/site/public"
for r in "" why why/two-agents-one-bug outcomes outcomes/faster-onboarding getting-started limitations roadmap profiles profiles/debugging policy guarantees guarantees/guaranteed guarantees/init-refuses supervises supervises/finish commands commands/doctor architecture docs docs/cli docs/design render-test; do [ -f "$P/$r/index.html" ] || { echo "    missing route /$r/"; exit 1; }; done
# homepage sections come from data, not templates
expect_grep 'A session grades its own homework' "$P/index.html"
# the hero: every proposition's headline from marketing.toml is on the homepage, the first is the one h1
n_props="$(grep -c '^  { slug = ' "$ROOT/site/data/marketing.toml")"; [ "$n_props" -ge 3 ]
found=0; while read -r h; do grep -qF ">$h<" "$P/index.html" && found=$((found+1)); done <<EOF_H
$(grep -oE 'headline = "[^"]*"' "$ROOT/site/data/marketing.toml" | sed 's/^headline = "//; s/"$//')
EOF_H
[ "$found" = "$n_props" ] || { echo "    $found of $n_props hero propositions on the homepage"; exit 1; }
expect_grep "<h1[^>]*>$(grep -oE 'headline = "[^"]*"' "$ROOT/site/data/marketing.toml" | head -1 | sed 's/^headline = "//; s/"$//')</h1>" "$P/index.html"
expect_grep 'id="hero-p-1"[^>]* checked' "$P/index.html"
expect_grep 'id="how-it-works"' "$P/index.html"
for slug in $(grep -oE 'slug = "[^"]*"' "$ROOT/site/data/marketing.toml" | sed 's/^slug = "//; s/"$//'); do [ -f "$P/outcomes/$slug/index.html" ] || { echo "    no page for outcome $slug"; exit 1; }; expect_grep "/outcomes/$slug/" "$P/index.html"; done
expect_grep "v$(jq -r .version "$ROOT/site/data/generated/project.json")" "$P/index.html"
for p in $(jq -r '.profiles[].slug' "$ROOT/site/data/generated/profiles.json"); do expect_grep "$p" "$P/profiles/index.html"; done
expect_grep "$(jq -r '.principles[0]' "$ROOT/site/data/generated/lifecycle.json" | cut -d' ' -f1-3)" "$P/supervises/index.html"
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
# navigation: five groups, dropdown menus present with Flowbite hooks; render-test is noindex
# a dropdown per navigation group that has items; the count comes from the data, not from
# a number written here, so adding a group to site/data/nav.toml does not break this case
want_dropdowns=$(awk '/^\[\[groups\]\]/{g++} /^items *=/{i++} END{print i+0}' "$ROOT/site/data/nav.toml")
[ "$(grep -o 'data-dropdown-toggle="nav-menu-' "$P/index.html" | wc -l | tr -d ' ')" = "$want_dropdowns" ]
expect_grep '<meta name="robots" content="noindex">' "$P/render-test/index.html"
expect_no_grep '<meta name="robots" content="noindex">' "$P/index.html"
# homepage: one primary CTA in the final block, boundary statement present
expect_grep 'Refused acceptance, not runtime prevention' "$P/index.html"
expect_grep 'Install Majordomus' "$P/index.html"
# every homepage tile is a link to a page that exists
for href in $(grep -oE 'href="[^"]*/(supervises|profiles|guarantees|commands|why)/[a-z0-9_-]+/"' "$P/index.html" | sed -E 's#.*/prismatic-majordomus/##; s#"$##' | sort -u); do [ -f "$P/$href/index.html" ] || { echo "    homepage tile links to missing $href"; exit 1; }; done
expect_grep 'href="[^"]*/supervises/"' "$P/index.html"
[ "$(grep -oE 'href="[^"]*/supervises/[a-z]+/"' "$P/supervises/index.html" | sort -u | wc -l | tr -d ' ')" = "$(jq '.does | length' "$ROOT/site/data/generated/readme.json")" ]
n_why=0; for f in "$ROOT"/site/content-src/why/*.md; do case "$(basename "$f")" in _index.md) ;; *) n_why=$((n_why + 1)) ;; esac; done
[ "$(grep -oE 'href="[^"]*/why/[a-z-]+/"' "$P/index.html" | sort -u | wc -l | tr -d ' ')" = "$n_why" ]
# every claim listed on the homepage links to its page; claim pages carry provenance and a verify command
[ "$(grep -oE 'href="[^"]*/guarantees/[a-z-]+/"' "$P/index.html" | grep -vE '/(guaranteed|advisory|planned|rejected)/' | sort -u | wc -l | tr -d ' ')" -ge 12 ]
expect_grep 'bash test/run.sh 03_update' "$P/guarantees/wiring-reconciliation/index.html"
expect_grep 'supervises/doctor/' "$P/guarantees/wiring-reconciliation/index.html"
expect_grep 'why/two-rulebooks-one-repository/' "$P/guarantees/wiring-reconciliation/index.html"
expect_grep 'guarantees/finish-contract/' "$P/commands/finish/index.html"
# guarantees table rows == claims
[ "$(grep -o '<tr class="align-top">' "$P/guarantees/index.html" | wc -l | tr -d ' ')" = "$(jq '.claims | length' "$ROOT/site/data/generated/capabilities.json")" ]
# the full check passes
expect_exit 0 "$ROOT/scripts/site-check"
