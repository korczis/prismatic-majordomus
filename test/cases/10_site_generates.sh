. "$ROOT/test/lib.sh"
# The site generator runs against the repository checkout, not a scratch repo: its input
# is the canonical documentation. It writes only into the output directory it is given.
OUT="$T/out"

# --no-css must need nothing from node_modules: run it with a HOME-less, empty PATH view
# of the toolchain is overkill, but the mode is exercised here precisely because CI runs
# it in a checkout that has not run npm ci.
expect_exit 0 bash "$ROOT/scripts/generate-pages" --out "$OUT" --no-css
expect_grep 'generate-pages: ok'

# every declared route exists as a directory with an index
for r in "" getting-started profiles guarantees docs docs/concepts docs/design docs/cli \
         docs/schemas docs/adoption docs/economics docs/extraction-report docs/contract; do
  [ -f "$OUT/$r/index.html" ] || { echo "    missing route: /$r/"; exit 1; }
done

# metadata and landmarks on every page
for f in $(find "$OUT" -name index.html); do
  expect_grep '<title>[^<]' "$f"
  expect_grep 'name="description" content="[^"]' "$f"
  expect_grep 'rel="canonical"' "$f"
  expect_grep 'property="og:image"' "$f"
  expect_grep '<main id="main"' "$f"
  expect_no_grep '\{\{[A-Z_]+\}\}' "$f"
  expect_no_grep '<[^>]* style="' "$f"
done

# assets and metadata files. --no-css skips the two Node-derived assets (the compiled
# stylesheet and the Flowbite bundle), so this run must not expect them and the pages
# must not reference them.
[ -f "$OUT/site-manifest.json" ] && [ -f "$OUT/sitemap.xml" ] && [ -f "$OUT/robots.txt" ] \
  && [ -f "$OUT/.nojekyll" ] && [ -f "$OUT/assets/theme.js" ] \
  && [ -f "$OUT/assets/logo-mark.svg" ] && [ -f "$OUT/assets/social-card.png" ]
[ ! -f "$OUT/assets/flowbite.min.js" ] || { echo "    --no-css produced a Node-derived asset"; exit 1; }
expect_no_grep 'assets/flowbite.min.js' "$OUT/index.html"
expect_no_grep 'assets/app.css' "$OUT/index.html"

# the site carries no stylesheet of its own; presentation is Tailwind and Flowbite only
[ -z "$(find "$OUT/assets" -name '*.css')" ]

# deterministic: a second run over unchanged sources produces the same manifest hash
sha1="$(sed -n 's/.*"input_sha256": "\([0-9a-f]*\)".*/\1/p' "$OUT/site-manifest.json")"
expect_exit 0 bash "$ROOT/scripts/generate-pages" --out "$T/out2" --no-css
sha2="$(sed -n 's/.*"input_sha256": "\([0-9a-f]*\)".*/\1/p' "$T/out2/site-manifest.json")"
[ "$sha1" = "$sha2" ] || { echo "    manifest hash is not deterministic: $sha1 vs $sha2"; exit 1; }
