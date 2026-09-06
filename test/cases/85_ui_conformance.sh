# UI conformance: the page set and the width set are discovered, the markup invariants are
# executable, and the normalisations the build applies to somebody else's output are exact.
#
# The case this exists for is the last kind: a page added to the site tomorrow, and a
# breakpoint added to the theme tomorrow, are audited tomorrow — without a list changing
# anywhere. Everything else here is the machinery that makes that claim checkable without a
# browser, so this case never needs one.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || { echo "    skip: no node"; exit 0; }

# The modules under test come from the checkout; the fixtures are built here. Nothing in this
# case writes into the checkout, so it runs in the parallel phase.
UI="$ROOT/scripts/lib"

# ---------------------------------------------------------------- the page set is discovered
# A built site is a filesystem of index.html and a sitemap. Both are made here, disagreeing
# on purpose: a page only in the filesystem is an orphan, a page only in the sitemap is a
# stale entry, and the audit visits the union so that neither hides.
mkdir -p public/alpha public/beta/nested
printf '<html></html>' > public/index.html
printf '<html></html>' > public/alpha/index.html
printf '<html></html>' > public/beta/nested/index.html
cat > public/sitemap.xml <<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<urlset><url><loc>https://example.test/</loc></url><url><loc>https://example.test/alpha/</loc></url>
<url><loc>https://example.test/gamma/</loc></url></urlset>
XML
cat > theme.css <<'CSS'
@media (min-width: 640px) { .a { color: red } }
@media (min-width: 48rem) { .b { color: red } }
.overflow-x-auto { overflow-x: auto }
.format :where(pre):not(:where([class~=not-format] *)) { overflow-x: auto }
.\[\&_table\]\:overflow-x-auto table { overflow-x: auto }
CSS

routes() { node --input-type=module -e '
import { plan } from "'"$UI"'/ui-discover.mjs";
const p = plan("'"$T"'/public", "'"$T"'/theme.css");
console.log(JSON.stringify({ routes: p.pages.map((x) => x.route), widths: p.viewports }));
'; }

expect_exit 0 routes
expect_grep '"/beta/nested/"' -                    # an orphan is still audited
expect_grep '"/gamma/"' -                          # so is a page only the sitemap knows
expect_grep '"/alpha/"' -

# a page added to the site appears without anything else changing
mkdir -p public/delta && printf '<html></html>' > public/delta/index.html
expect_exit 0 routes
expect_grep '"/delta/"' -

# ---------------------------------------------------------------- the width set is derived
# The breakpoints are the media queries the CSS build emitted, in px and in rem, each with
# the pixel below it, where a layout discontinuity hides.
expect_grep '640' -
expect_grep '639' -
expect_grep '768' -                                # 48rem, converted
printf '@media (min-width: 900px) { .c { color: red } }\n' >> theme.css
expect_exit 0 routes
expect_grep '900' -
expect_grep '899' -

# ---------------------------------------------------------------- markup we write
# A scrolling box a keyboard cannot reach is refused wherever this repository writes one.
scan() { node --input-type=module -e '
import { readFileSync } from "node:fs";
import { scan, scrollingTags } from "'"$UI"'/ui-static.mjs";
const tags = process.argv[2] === "rendered" ? scrollingTags(readFileSync("'"$T"'/theme.css", "utf8")) : [];
const found = scan(readFileSync(process.argv[1], "utf8"), { scrollingTagNames: tags });
for (const o of found) console.log(`${o.severity ?? "error"} ${o.rule} line ${o.line}`);
console.log(`${found.filter((o) => o.severity !== "warning").length} failure(s)`);
' "$@"; }

printf '<div class="overflow-x-auto"><table></table></div>\n' > bad.html
expect_exit 0 scan bad.html
expect_grep 'ui.scrollable-region-focusable' -
expect_grep '^1 failure' -

printf '<div class="overflow-x-auto" tabindex="0"><table></table></div>\n' > good.html
expect_exit 0 scan good.html
expect_grep '^0 failure' -

# an element a browser already puts in the tab order needs nothing
printf '<a class="overflow-x-auto">x</a>\n' > link.html
expect_exit 0 scan link.html
expect_grep '^0 failure' -

# ---------------------------------------------------------------- markup we do not write
# Which elements the theme makes scroll is read from the stylesheet, never named here: a
# theme that stops making <pre> scroll stops the build touching it.
printf '<pre>x</pre><table></table>\n' > rendered.html
expect_exit 0 scan rendered.html                   # as a source: nothing declares scrolling
expect_grep '^0 failure' -
expect_exit 0 scan rendered.html rendered          # as a rendered page: the theme scrolls both
expect_grep '^2 failure' -

normalise() { node --input-type=module -e '
import { readFileSync } from "node:fs";
import { normalise, scrollingTags } from "'"$UI"'/ui-static.mjs";
const tags = scrollingTags(readFileSync("'"$T"'/theme.css", "utf8"));
const once = normalise(readFileSync(process.argv[1], "utf8"), { scrollingTagNames: tags });
const twice = normalise(once.text, { scrollingTagNames: tags });
console.log(once.text.trim());
console.log(`first ${once.changed}, second ${twice.changed}`);
' "$@"; }

expect_exit 0 normalise rendered.html
expect_grep '<pre tabindex="0">' -
expect_grep '<table tabindex="0">' -
expect_grep '^first 2, second 0$' -                # normalising a normalised page changes nothing

# a markdown task list arrives as a control with no name; the name is the text beside it
printf '<li><input disabled="" type="checkbox" checked=""/>ship the thing\n' > tasks.html
node --input-type=module -e '
import { readFileSync } from "node:fs";
import { nameTaskCheckboxes } from "'"$UI"'/ui-static.mjs";
const r = nameTaskCheckboxes(readFileSync("'"$T"'/tasks.html", "utf8"));
console.log(r.text.trim());
console.log(`changed ${r.changed}, again ${nameTaskCheckboxes(r.text).changed}`);
' > tasks.out 2>&1
expect_grep 'aria-label="ship the thing — done"' tasks.out
expect_grep 'type="checkbox" checked=""' tasks.out
expect_no_grep 'checkbox"/ ' tasks.out             # the self-closing slash is not an attribute
expect_grep '^changed 1, again 0$' tasks.out

# ---------------------------------------------------------------- the palette
# Every colour of a generated highlight palette reaches the contrast threshold against the
# background the same stylesheet declares, keeping its hue; a palette that already passes is
# rewritten to itself.
cat > palette.css <<'CSS'
.z-l-code { color: #5C6166; background-color: #F8F9FA; }
.z-l-1 { color: #22A4E6; }
.z-l-2 { color: #55B4D480; }
.z-l-3 { color: #111111; }
CSS
node --input-type=module -e '
import { readFileSync } from "node:fs";
import { enforce, measure, MINIMUM } from "'"$UI"'/ui-contrast.mjs";
const before = readFileSync("'"$T"'/palette.css", "utf8");
const first = enforce(before);
const second = enforce(first.css);
const after = measure(first.css);
console.log(first.css.trim());
console.log(`background ${first.background}`);
console.log(`below ${after.colours.filter((c) => c.ratio < MINIMUM).length}`);
console.log(`first ${first.changed}, second ${second.changed}`);
' > palette.out 2>&1
expect_grep 'background #F8F9FA' palette.out
expect_grep '^below 0$' palette.out                # every colour reaches the threshold
expect_grep '^first 2, second 0$' palette.out      # and doing it again changes nothing
expect_grep 'background-color: #F8F9FA' palette.out # the ground it measures against is not moved
expect_grep 'color: #111111' palette.out           # a colour that already passes is untouched

# ---------------------------------------------------------------- the report
# The rendering is a section of the test surface, declares no surface of its own, and a run
# narrowed for iteration says so on its own page rather than reading as a clean audit.
BIN="$(rust_bin)" || { echo "    no toolchain; the rest of this case needs one"; exit 0; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null                              # the executable resolves a repository, not a directory
cat > run-ui.json <<'JSON'
{
  "schema": "ui-audit/v1", "origin": "http://127.0.0.1:1", "complete": false,
  "source": {"pages": "the built site", "viewports": "the stylesheet"},
  "breakpoints": [640], "viewports": [320, 639, 640, 1440],
  "pages": 1, "visits": 1, "seconds": 2,
  "findings": [{"route": "/", "width": 320, "tier": "sweep",
    "rule": "responsive.horizontal-overflow", "detail": "the document is 420px wide",
    "elements": [{"selector": "pre.code", "right": 420}]}]
}
JSON
expect_exit 0 "$BIN" web report ui --from run-ui.json
expect_file target/web/tests/ui/index.html
expect_file target/web/tests/ui/results.json
expect_grep 'narrowed' target/web/tests/ui/index.html
expect_grep 'responsive.horizontal-overflow' target/web/tests/ui/index.html
expect_grep 'pre.code' target/web/tests/ui/index.html
[ -f target/web/tests/ui/surface.json ] && { echo "    a section of another surface declared one of its own"; exit 1; }
# the enclosing surface exists, and there is exactly one of it
"$BIN" web list > list.txt
expect_grep '^tests +static +/tests ' list.txt
expect_no_grep '/tests/ui' list.txt

# a document from a contract this executable does not read is refused, never guessed
sed 's|ui-audit/v1|ui-audit/v9|' run-ui.json > future.json
expect_exit 10 "$BIN" web report ui --from future.json
expect_grep 'ui-audit/v1' -

# ---------------------------------------------------------------- a native route over the site
# The topology exempts the root application from the nesting rule; that exemption was blind
# to the application having pages at a path a native route claims. It is a warning, with the
# count, because which of the two moves is a person's decision.
mkdir -p site/public/docs/adoption
printf '<html></html>' > site/public/index.html
printf '<html></html>' > site/public/docs/adoption/index.html
cat > site/config.toml <<'TOML'
base_url = "https://example.test"
title = "fixture"
TOML
"$BIN" web validate > shadow.txt 2>&1
expect_grep 'surface.shadows-application' shadow.txt
expect_grep 'swagger' shadow.txt
expect_grep '1 page' shadow.txt
