# majordomus-exclusive: builds the site into site/public of this checkout and runs site-check over it
# The built site (when zola is present): routes, sections, derived content, prefix, mermaid, and site-check.
. "$ROOT/test/lib.sh"
command -v zola >/dev/null || { echo "    zola absent; skipping build test"; exit 0; }
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
[ -d "$ROOT/node_modules/tailwindcss" ] || { echo "    node_modules absent; skipping build test"; exit 0; }
expect_exit 0 "$ROOT/scripts/site-build"
P="$ROOT/site/public"
for r in "" why why/two-agents-one-bug features features/matrix getting-started limitations roadmap profiles profiles/debugging policy guarantees guarantees/guaranteed guarantees/init-refuses supervises supervises/finish commands commands/doctor architecture docs docs/cli docs/design render-test; do [ -f "$P/$r/index.html" ] || { echo "    missing route /$r/"; exit 1; }; done
# homepage sections come from data, not templates: every chapter is a feature that declares
# itself featured, and the marks beside it are the ones the model derived
PD="$ROOT/site/data/registry/product.json"
for id in $(jq -r '.features[] | select(.featured and .status != "draft") | .id' "$PD"); do
  expect_grep "/features/$id/" "$P/index.html"
  [ -f "$P/features/$id/index.html" ] || { echo "    featured feature $id has no page"; exit 1; }
done
# every public feature has its page, and the section lists it
for id in $(jq -r '.features[] | select(.status != "draft") | .id' "$PD"); do
  [ -f "$P/features/$id/index.html" ] || { echo "    feature $id has no page"; exit 1; }
  expect_grep "/features/$id/" "$P/features/index.html"
done
# the matrix shows exactly the marks the model derived, on the homepage and on its own page
want="$(jq '[.matrix.rows[] | select(.status != "draft") | .exposed | length] | add' "$PD")"
got="$(grep -o '✓' "$P/features/matrix/index.html" | wc -l | tr -d ' ')"
[ "$want" = "$got" ] || { echo "    /features/matrix/ shows $got marks, the model derived $want"; exit 1; }
# the hero's positioning words are marketing.toml's, and the h1 is its title
expect_grep "<h1[^>]*>$(sed -n 's/^title = "\(.*\)"$/\1/p' "$ROOT/site/data/marketing.toml" | head -1)</h1>" "$P/index.html"
expect_grep 'id="install"' "$P/index.html"
expect_grep 'id="chapters"' "$P/index.html"
# the routes that moved answer with a redirect rather than a 404
for from in $(awk '/^from *= *"/ { f=$0; sub(/^from *= *"/,"",f); sub(/".*$/,"",f); print f }' "$ROOT/site/data/nav.toml"); do
  [ -f "$P${from}index.html" ] || { echo "    the moved route $from has no redirect"; exit 1; }
  expect_grep 'Redirect' "$P${from}index.html"
done
expect_grep "v$(jq -r .version "$ROOT/site/data/generated/project.json")" "$P/index.html"
for p in $(jq -r '.profiles[].slug' "$ROOT/site/data/generated/profiles.json"); do expect_grep "$p" "$P/profiles/index.html"; done
expect_grep "$(jq -r '.principles[0]' "$ROOT/site/data/generated/lifecycle.json" | cut -d' ' -f1-3)" "$P/supervises/index.html"
# the homepage's figure is the derived product graph, and the list under it is the same
# graph without a script: the page is readable with Cytoscape blocked
expect_grep 'data-graph="product-data"' "$P/index.html"
expect_grep 'js/graph.js' "$P/index.html"
expect_grep 'data-graph-fallback' "$P/index.html"
expect_grep 'class="mermaid' "$P/getting-started/index.html"; expect_grep 'js/mermaid.min.js' "$P/getting-started/index.html"
expect_grep 'stateDiagram-v2' "$P/getting-started/index.html"
# docs pages: typography container, syntax classes, anchors, wrapped tables, footnote, callout, fenced mermaid
# the typography container is on a rendered document; /docs/cli/ is the generated command
# tree, and the shell CLI's own prose lives at its own slug (doc_slug, CLI_SPEC_SLUG)
expect_grep 'class="format' "$P/docs/cli-specification/index.html"
expect_grep 'class="z-[a-z]' "$P/render-test/index.html"   # Zola/giallo class-based highlighting
expect_grep 'id="second-level-heading"' "$P/render-test/index.html"
expect_grep 'overflow-x-auto' "$P/render-test/index.html"
expect_grep 'role="note"' "$P/render-test/index.html"
expect_grep 'footnote' "$P/render-test/index.html"
# The class, not the whole tag: the build's conformance pass puts every scrolling element
# in the tab order, so this `pre` carries a `tabindex` the diagram does not choose. The
# bracket keeps it from matching a class that merely starts with the word.
expect_grep '<pre class="mermaid"[ >]' "$P/render-test/index.html"
# navigation: five groups, dropdown menus present with Flowbite hooks; render-test is noindex
# a dropdown per navigation group that has items; the count comes from the data, not from
# a number written here, so adding a group to site/data/nav.toml does not break this case
want_dropdowns=$(awk '/^\[\[groups\]\]/{g++} /^items *=/{i++} END{print i+0}' "$ROOT/site/data/nav.toml")
[ "$(grep -o 'data-dropdown-toggle="nav-menu-' "$P/index.html" | wc -l | tr -d ' ')" = "$want_dropdowns" ]
expect_grep '<meta name="robots" content="noindex">' "$P/render-test/index.html"
expect_no_grep '<meta name="robots" content="noindex">' "$P/index.html"
# homepage: one primary CTA in the final block, and the boundary statement marketing.toml holds
expect_grep "$(sed -n 's/^boundary = "\(.\{0,60\}\).*$/\1/p' "$ROOT/site/data/marketing.toml" | head -1)" "$P/index.html"
expect_grep 'Install Majordomus' "$P/index.html"
# every homepage tile is a link to a page that exists
for href in $(grep -oE 'href="[^"]*/(features|profiles|guarantees|commands|why|doctrines)/[a-z0-9_-]+/"' "$P/index.html" | sed -E 's#.*/prismatic-majordomus/##; s#"$##' | sort -u); do [ -f "$P/$href/index.html" ] || { echo "    homepage tile links to missing $href"; exit 1; }; done
expect_grep 'href="[^"]*/features/"' "$P/index.html"
[ "$(grep -oE 'href="[^"]*/supervises/[a-z]+/"' "$P/supervises/index.html" | sort -u | wc -l | tr -d ' ')" = "$(jq '.does | length' "$ROOT/site/data/generated/readme.json")" ]
# The recognition grid is the moments that declare themselves featured, counted from the
# catalogue the executable derives. It used to be counted from site/content-src/why/*.md,
# which stopped existing when a moment became an object of the layer (ADR 0018): the glob
# then matched nothing, the count was one, and a bare comparison under `set -e` failed with
# no message. The count must also be non-zero, or the comparison passes by both sides being
# empty — a check that cannot fail is worse than the one it replaced.
n_why="$(jq '[.moments[] | select(.status == "stable" and .featured)] | length' "$ROOT/site/data/registry/why.json")"
[ "$n_why" -gt 0 ] || { echo "    the catalogue features no stable moment; the homepage would link none"; exit 1; }
n_linked="$(grep -oE 'href="[^"]*/why/[a-z-]+/"' "$P/index.html" | sort -u | wc -l | tr -d ' ')"
[ "$n_linked" = "$n_why" ] || { echo "    the homepage links $n_linked why moment(s); $n_why declare themselves featured"; exit 1; }
# the doctrine section is the dataset's own rule list: every rule it names has its page
for r in $(jq -r '.rules[].id' "$ROOT/site/data/registry/product.json"); do
  slug="$(jq -r --arg i "$r" '.doctrines[] | select(.id == $i) | .slug' "$ROOT/site/data/generated/doctrines.json")"
  # a rule the site publishes is linked; one it does not is still named, never a dead link
  [ -n "$slug" ] || continue
  expect_grep "/doctrines/$slug/" "$P/index.html"
  [ -f "$P/doctrines/$slug/index.html" ] || { echo "    the homepage names rule $r, which has no page"; exit 1; }
done
# claim pages carry provenance and a verify command
expect_grep 'bash test/run.sh 03_update' "$P/guarantees/wiring-reconciliation/index.html"
expect_grep 'supervises/doctor/' "$P/guarantees/wiring-reconciliation/index.html"
expect_grep 'why/two-rulebooks-one-repository/' "$P/guarantees/wiring-reconciliation/index.html"
expect_grep 'guarantees/finish-contract/' "$P/commands/finish/index.html"
# guarantees table rows == claims
[ "$(grep -o '<tr class="align-top">' "$P/guarantees/index.html" | wc -l | tr -d ' ')" = "$(jq '.claims | length' "$ROOT/site/data/generated/capabilities.json")" ]
# the full check passes
expect_exit 0 "$ROOT/scripts/site-check"
