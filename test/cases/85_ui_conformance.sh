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

# ------------------------------------------- the page set of a surface with no directory
# A surface the executable renders has no directory to read, so its pages come from the
# surface itself: its own anchors, crawled from its mount. The crawl takes its fetcher from
# the caller, so this stays a case with no server, no browser and no port.
#
# The fixture is shaped like the Cockpit and nothing about that shape is in the module: an
# entry that advertises a listing, a listing that paginates, and leaves that link only back.
cat > surface.mjs <<'MJS'
import { crawl, sample, anchors, family, owner, spread } from "@UI@/ui-routes.mjs";

const LEAVES = 25, PER_PAGE = 5, PAGES = LEAVES / PER_PAGE;
const link = (href) => `<a class="x" href="${href}">t</a>`;
let fetched = 0;
const fetchText = async (route) => {
  fetched += 1;
  if (route === "/panel") return `<link href="/panel/assets/a.css">${link("/panel/list")}${link("/panel/health")}`;
  if (route === "/panel/health") return link("/panel");
  if (route.startsWith("/panel/list")) {
    const page = Number(new URLSearchParams(route.split("?")[1] || "").get("page") || 1);
    if (page > PAGES) return null;
    const items = Array.from({ length: PER_PAGE }, (_, i) =>
      link(`/panel/item/i${String((page - 1) * PER_PAGE + i).padStart(2, "0")}`));
    return [link("/panel"), ...items, link(`/panel/list?page=${page + 1}`)].join("");
  }
  if (route.startsWith("/panel/item/")) return link("/panel");
  return null;
};

const derived = await crawl({ mount: "/panel", mounts: ["/", "/panel"], fetchText });
console.log(`routes ${derived.routes.length} nav ${derived.navigation.length} families ${Object.keys(derived.families).length} fetched ${fetched} truncated ${derived.truncated}`);
console.log(`leaves ${derived.routes.filter((r) => r.startsWith("/panel/item/")).length}`);
console.log(`sample ${sample(derived, 4).length}`);
console.log(`assets ${derived.routes.filter((r) => r.includes("/assets/")).length}`);
const quiet = await crawl({ mount: "/api", mounts: ["/", "/api"], fetchText: async () => null });
console.log(`document ${quiet.document} pages ${sample(quiet).length}`);
console.log(`anchors ${JSON.stringify(anchors('<link href="/a.css"><a href="/b?x=1&amp;y=2">b</a><a href="#c">c</a>'))}`);
console.log(`family ${family("/panel/item/i01")} ${family("/panel/list?page=2")} ${family("/panel")}`);
console.log(`owner ${owner("/panel/health", ["/", "/panel"])} ${owner("/elsewhere", ["/", "/panel"])}`);
console.log(`spread ${JSON.stringify(spread(["a", "b", "c", "d", "e"], 3))}`);
MJS
sed "s|@UI@|$UI|" surface.mjs > surface.run.mjs
expect_exit 0 node surface.run.mjs
expect_grep '^leaves 25$' -                        # every leaf the listing paginates to is found
expect_grep 'truncated false' -
expect_grep '^assets 0$' -                         # a <link> is not an anchor, so it is not a route
expect_grep '^document false pages 0$' -           # a mount that answers no document has no pages
expect_grep '^anchors \["/b\?x=1&y=2"\]$' -         # the entity is decoded; a fragment is not a route
expect_grep '^family /panel/item/\* /panel/list\?page /\*$' -
expect_grep '^owner /panel /$' -                   # the longest mount owns the route
expect_grep '^spread \["a","c","e"\]$' -           # the first, the last, and the spread between
# the crawl pays for the listing it has to follow and not for the leaves that teach it
# nothing: 33 routes for fewer than half as many requests
expect_grep 'routes 33 nav 2 families 4 fetched 1[0-6] ' -
expect_grep '^sample 11$' -                        # the entry, what it advertises, four per family

# and a crawled surface reaches the plan: its pages are tiered by family, because a family
# is where the renderer changes, exactly as a directory is on a built surface
cat > native.mjs <<'MJS'
import { crawl } from "@UI@/ui-routes.mjs";
import { planSurfaces } from "@UI@/ui-discover.mjs";
const fetchText = async (route) =>
  route === "/panel" ? '<a href="/panel/a">a</a><a href="/panel/x/1">x</a><a href="/panel/x/2">y</a>' : '<a href="/panel">u</a>';
const derived = await crawl({ mount: "/panel", mounts: ["/panel"], fetchText });
const surface = { id: "panel", mount: "/panel", kind: "native-route", derived, pages: derived.routes };
const p = planSurfaces([surface], "@T@/theme.css");
for (const page of p.pages) console.log(`${page.route} ${page.tier} ${page.family} ${page.surface}`);
console.log(JSON.stringify(p.surfaces));
console.log(p.source.pages);
MJS
sed "s|@UI@|$UI|;s|@T@|$T|" native.mjs > native.run.mjs
expect_exit 0 node native.run.mjs
expect_grep '/panel/x/1 sweep /panel/x/\*' -       # one member of each family takes the boundary sweep
expect_grep '/panel/x/2 critical /panel/x/\*' -    # the rest take the critical widths
expect_grep '"kind":"native-route"' -
expect_grep '"routes":4' -                         # what the surface has, beside what was visited
expect_grep "crawled from the surface's own anchors" -

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
  "surfaces": [{"id": "docs", "mount": "/docs", "kind": "static-directory"},
               {"id": "cockpit", "mount": "/cockpit", "kind": "native-route",
                "routes": 2645, "families": 15, "sampled": 115, "truncated": false}],
  "foreign": [{"route": "/swagger", "selector": "main#swagger-ui",
               "declares": "swagger-ui-dist@5.17.14"}],
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
# a surface the executable renders reports what it has beside what was visited, so a sample
# can never be read as a total; a built surface reports that every page of it was visited
expect_grep '2645 route\(s\) in 15 family\(ies\)' target/web/tests/ui/index.html
expect_grep '>115<' target/web/tests/ui/index.html
expect_grep 'every page' target/web/tests/ui/index.html
# and what the run did not measure is on the page, with what declared it
expect_grep 'Not measured, and why' target/web/tests/ui/index.html
expect_grep 'swagger-ui-dist@5.17.14' target/web/tests/ui/index.html
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
# to the application having pages at a path a native route claims. Proved by mutation: the
# site grows a page under a route the executable answers itself, and the validator says so
# with the count. A warning, not an error, because which of the two moves is intent.
mkdir -p site/public
printf '<html></html>' > site/public/index.html
cat > site/config.toml <<'TOML'
base_url = "https://example.test"
title = "fixture"
TOML
"$BIN" web validate > clean.txt 2>&1
expect_no_grep 'surface.shadows-application' clean.txt

SWAGGER="$("$BIN" web list | awk '$1=="swagger"{print $3}')"
[ -n "$SWAGGER" ] || { echo "    the topology has no swagger surface to test against"; exit 1; }
mkdir -p "site/public${SWAGGER}"
printf '<html></html>' > "site/public${SWAGGER}/index.html"
"$BIN" web validate > shadow.txt 2>&1
expect_grep 'surface.shadows-application' shadow.txt
expect_grep 'swagger' shadow.txt
expect_grep '1 page' shadow.txt
# and the executable still serves; a shadowing is reported, never refused
expect_exit 0 "$BIN" web validate
