# majordomus-covers: none
# A scroll region behind a tab is revealed by choosing the tab, and a panel that then does not appear is said to be
# that — not a region "nothing on the page reveals". Rule: project.every-link-and-control-is-tested.
#
# The homepage keeps eighteen scroll regions in the tab panels of the recorded run. The probe chooses each one's tab
# and waits for its panel, which x-show draws on a later animation frame. On a loaded runner that wait once ran out,
# and the finding read "a pre.m-0 scroll region is hidden and nothing on the page reveals it": it blamed the markup
# of a page whose tab had just been found and pressed, and the same tree passed on the rerun.
#
# Held here, by the real spec (scripts/lib/interaction-specs/scroll-region.mjs) over a fixture site with the real
# Alpine, the wait shortened through MJ_PROBE_REVEAL_MS so that a panel which never appears costs a second:
#   1. a region in a panel its tab shows is reached, and passes;
#   2. a panel the page's state shows and the renderer never draws: the finding names the tab, the time waited, and
#      that the state says shown — a wait that ran out, not a page that hides its content;
#   3. a panel the tab does not select: the same finding, saying the state is still hidden;
#   4. a region nothing reveals keeps the finding it always had, with what hides it.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || skip "no node"
[ -d "$ROOT/node_modules/playwright" ] || { [ "${CI:-}" = true ] && { echo "    playwright absent under CI"; exit 1; }; skip "playwright absent (npm ci)"; }
ALPINE="$ROOT/node_modules/alpinejs/dist/cdn.min.js"
[ -f "$ALPINE" ] || { [ "${CI:-}" = true ] && { echo "    alpinejs absent under CI"; exit 1; }; skip "alpinejs absent (npm ci)"; }
PROBE="$ROOT/scripts/interaction-probe"
SPEC="$ROOT/scripts/lib/interaction-specs/scroll-region.mjs"
[ -f "$SPEC" ] || { echo "    $SPEC is missing"; exit 1; }

F="$PWD/site"; mkdir -p "$F/site/public/js" "$F/docs/generated" specs
cp "$SPEC" specs/
cp "$ALPINE" "$F/site/public/js/alpine.min.js"
printf 'base_url = "https://fixture.test"\n' > "$F/site/config.toml"
printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$F/docs/generated/web.json"

# one region, wider than its box, so that it must scroll once it is reached
REGION='<div tabindex="0" style="overflow-x:auto;width:120px"><pre style="margin:0;width:900px">a line far wider than the box that holds it, so that the box scrolls sideways</pre></div>'
page() {   # <route> <extra head> <second tab's handler> <second panel>
  mkdir -p "$F/site/public/$1"
  printf '<!doctype html><html><head><script defer src="/js/alpine.min.js"></script>%s</head><body><div x-data="{ s: 0 }"><div role="tablist"><button type="button" role="tab" aria-controls="p0" x-on:click="s = 0">first</button><button type="button" role="tab" aria-controls="p1" %s>second</button></div><div id="p0" x-show="s === 0">the first panel</div>%s</div></body></html>' \
    "$2" "$3" "$4" > "$F/site/public/$1/index.html"
}
page shown '' 'x-on:click="s = 1"' "<div id=\"p1\" x-show=\"s === 1\">$REGION</div>"
# the state says shown and nothing is drawn: what a renderer that withholds its frames looks like, held still
page withheld '<style>#p1 { display: none !important; }</style>' 'x-on:click="s = 1"' "<div id=\"p1\" x-show=\"s === 1\">$REGION</div>"
# the tab is there and choosing it selects nothing
page unselected '' 'x-on:click="s = 0"' "<div id=\"p1\" x-show=\"s === 1\">$REGION</div>"
mkdir -p "$F/site/public/nothing"
printf '<!doctype html><html><body><div style="display:none">%s</div></body></html>' "$REGION" > "$F/site/public/nothing/index.html"

run() { rc=0; MJ_ROOT="$F" MJ_INTERACTION_SPECS="$PWD/specs" MJ_PROBE_JOBS=1 MJ_PROBE_REVEAL_MS=1500 "$PROBE" --spec scroll-region "$@" > out.txt 2>&1 || rc=$?; }

# ---------------------------------------------------------------- 1. a panel its tab shows
run --only /shown/
if grep -q '^SKIP interaction-probe: no browser' out.txt; then
  [ "${CI:-}" = true ] && { echo "    the probe skipped under CI"; exit 1; }
  skip "no browser could be started"
fi
if grep -q '^SKIP' out.txt; then echo "    the probe skipped for a reason other than a missing browser"; cat out.txt; exit 1; fi
[ "$rc" = 0 ] || { echo "    a region in a panel its tab shows did not pass (exit $rc)"; cat out.txt; exit 1; }
expect_grep '^OK   behaviour    1 control\(s\) on 1 page\(s\) do what their spec says' out.txt

# ---------------------------------------------------------------- 2. shown by the state, never drawn
run --only /withheld/
[ "$rc" = 10 ] || { echo "    a panel that never appeared exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep "/withheld/: \[scroll-region\] a div scroll region is in a tab panel; its tab was chosen and the panel did not appear in 1\.5 s: the page's state says the panel is shown, so it was not drawn in that time" out.txt
if grep -q 'nothing on the page reveals it' out.txt; then
  echo "    a panel whose tab was chosen is still reported as one nothing reveals:"; cat out.txt; exit 1
fi

# ---------------------------------------------------------------- 3. the tab selects nothing
run --only /unselected/
[ "$rc" = 10 ] || { echo "    a tab that selects nothing exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep "/unselected/: \[scroll-region\] a div scroll region is in a tab panel; its tab was chosen and the panel did not appear in 1\.5 s: the page's state says the panel is still hidden, so choosing the tab did not select it" out.txt

# ---------------------------------------------------------------- 4. nothing reveals it
run --only /nothing/
[ "$rc" = 10 ] || { echo "    a region nothing reveals exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep '/nothing/: \[scroll-region\] a div scroll region is hidden by div and nothing on the page reveals it' out.txt
if grep -q 'its tab was chosen' out.txt; then echo "    a region with no tab is reported as one whose tab was chosen:"; cat out.txt; exit 1; fi

# the bound a run uses when nothing shortens it is the one the finding would name
grep -q 'MJ_PROBE_REVEAL_MS) : 30000;' "$SPEC" || { echo "    the spec's own bound is no longer 30 s; this case's reading of it is stale"; exit 1; }
echo "    a chosen tab's panel that does not appear is named as that, with the time waited and what the page's state says"
