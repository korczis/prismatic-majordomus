# majordomus-covers: none
# The behaviour probe drives every control in a browser and refuses what does not hold: scripts/interaction-probe,
# run against a fixture site and fixture specs this case writes, so the verdict for each page is known before the
# probe runs. Rule: project.every-link-and-control-is-tested.
#
# What is proved: a page whose control does what its spec says passes and is counted; and each of these fails with
# a finding that names the page and the spec: a control whose behaviour is broken, a live page holding a different
# number of a spec's controls than its markup declares, a console error, a request to an origin that is not the
# site's own, and a spec that drives fewer controls than it claims. It needs a browser: without one it skips
# locally and fails under CI=true, because a behaviour nobody ran is not a behaviour that holds. The pages driven
# are the site's own: a broken control under /ref, a mount the fixture's topology composes (ADR 0086), is that
# surface's business and is not driven; under a mount the topology does not compose it is, and fails.
#
# The last part runs the repository's own specs, scroll-region and kit-tabs, against pages wired by the repository's
# own site/kit.js, because a fixture spec cannot say what the shipped one does. A region in a hidden tab panel is
# revealed through its tab, also through two nested strips. A click the tab never heard (the page moves between the
# press and the release) is the driver's miss: it is noted on stderr and made once more, and the page passes. A tab
# that hears the pointer and reveals nothing fails, though it answers Enter: the pointer is never replaced by a key,
# and the two regions of that panel are two findings. A hidden region with no tab is a finding of its own.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || skip "no node"
[ -d "$ROOT/node_modules/playwright" ] || { [ "${CI:-}" = true ] && { echo "    playwright absent under CI"; exit 1; }; skip "playwright absent (npm ci)"; }
PROBE="$ROOT/scripts/interaction-probe"

F="$PWD/site"; mkdir -p "$F/site/public/good" "$F/site/public/broken" "$F/site/public/vanishing" "$F/site/public/noisy" "$F/site/public/foreign" "$F/site/public/lazy" specs
printf 'base_url = "https://fixture.test"\n' > "$F/site/config.toml"
mkdir -p "$F/docs/generated" "$F/site/public/ref/api"
COMPOSING='{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"},{"id":"ref","kind":"static-directory","mount":"/ref","artifact":"target/web/ref","availability":"both"}]}'
printf '%s\n' "$COMPOSING" > "$F/docs/generated/web.json"
page() { printf '<!doctype html><html><body>%s</body></html>' "$2" > "$F/site/public/$1/index.html"; }
# a counter: pressing it must add one
page good '<button type="button" x-on:click="n++" id="c">0</button><script>document.getElementById("c").onclick=function(){this.textContent=String(Number(this.textContent)+1)}</script>'
page broken '<button type="button" x-on:click="n++" id="c">0</button>'
page vanishing '<button type="button" x-on:click="n++" id="c">0</button><button type="button" x-on:click="n++" id="gone">0</button><script>document.getElementById("gone").remove();document.getElementById("c").onclick=function(){this.textContent=String(Number(this.textContent)+1)}</script>'
page noisy '<button type="button" x-on:click="n++" id="c">0</button><script>document.getElementById("c").onclick=function(){this.textContent=String(Number(this.textContent)+1)};console.error("fixture: something broke")</script>'
page foreign '<button type="button" x-on:click="n++" id="c">0</button><img src="https://elsewhere.example/pixel.png" alt=""><script>document.getElementById("c").onclick=function(){this.textContent=String(Number(this.textContent)+1)}</script>'
page lazy '<button type="button" x-on:click="skip" id="s">skip</button>'
# the composed surface's own page, with a control that does not do what the site's spec says
page ref/api '<button type="button" x-on:click="n++" id="c">0</button>'
cat > specs/counter.mjs <<'JS'
export default {
  id: 'counter', title: 'pressing a counter adds one',
  claims: (el) => el.attrs['x-on:click'] === 'n++',
  async exercise({ controls, fail }) {
    for (const c of controls) {
      const before = Number(await c.locator.innerText());
      await c.locator.click();
      if (Number(await c.locator.innerText()) !== before + 1) fail('pressing the counter did not add one');
    }
    return controls.length;
  },
};
JS
cat > specs/lazy.mjs <<'JS'
export default {
  id: 'lazy', title: 'a spec that drives nothing',
  claims: (el) => el.attrs['x-on:click'] === 'skip',
  async exercise() { return 0; },
};
JS
run() { rc=0; MJ_ROOT="$F" MJ_INTERACTION_SPECS="$PWD/specs" MJ_PROBE_JOBS=2 "$PROBE" "$@" > out.txt 2>&1 || rc=$?; }

run --only /good/
# only a missing browser is a reason to skip; playwright was checked above, so any other SKIP is a failure
if grep -q '^SKIP interaction-probe: no browser' out.txt; then
  [ "${CI:-}" = true ] && { echo "    the probe skipped under CI"; exit 1; }
  skip "no browser could be started"
fi
if grep -q '^SKIP' out.txt; then echo "    the probe skipped for a reason other than a missing browser"; cat out.txt; exit 1; fi
[ "$rc" = 0 ] || { echo "    a page whose control works did not pass (exit $rc)"; cat out.txt; exit 1; }
expect_grep '^OK   behaviour    1 control\(s\) on 1 page\(s\) do what their spec says' out.txt

run --only /broken/
[ "$rc" = 10 ] || { echo "    a broken control exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep 'FAIL behaviour +/broken/: \[counter\] pressing the counter did not add one' out.txt

run --only /vanishing/
[ "$rc" = 10 ] || { echo "    a vanished control exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep '/vanishing/: \[counter\] the markup declares 2 control\(s\) this spec claims and the live page holds 1' out.txt

run --only /noisy/
[ "$rc" = 10 ] || { echo "    a console error exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep '/noisy/: console: fixture: something broke' out.txt

run --only /foreign/
[ "$rc" = 10 ] || { echo "    a foreign request exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep "/foreign/: requested https://elsewhere.example/pixel.png, which is not the site's own origin" out.txt

run --only /lazy/
[ "$rc" = 10 ] || { echo "    a spec driving nothing exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep '/lazy/: \[lazy\] drove 0 of the 1 control\(s\) it claims' out.txt

# ---------------------------------------------------------------- the composed surface is not the site
run --only /ref/api/
[ "$rc" = 0 ] || { echo "    a page under a composed mount was driven (exit $rc)"; cat out.txt; exit 1; }
expect_grep '^OK   behaviour    0 control\(s\) on 0 page\(s\)' out.txt
printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$F/docs/generated/web.json"
run --only /ref/api/
[ "$rc" = 10 ] || { echo "    a broken control under a mount the topology does not compose exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep 'FAIL behaviour +/ref/api/: \[counter\] pressing the counter did not add one' out.txt
rm "$F/docs/generated/web.json"; run --only /good/
[ "$rc" = 12 ] || { echo "    no topology exited $rc, not 12"; cat out.txt; exit 1; }
expect_grep 'web.json is missing' out.txt

# ---------------------------------------------------------------- the shipped specs reveal a tab panel by its tab
G="$PWD/kit"; mkdir -p "$G/site/public" "$G/docs/generated"
printf 'base_url = "https://fixture.test"\n' > "$G/site/config.toml"
printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$G/docs/generated/web.json"
# a page is its body and one script
kpage() { mkdir -p "$G/site/public/$1"; { printf '<!doctype html><html><body>%s<script>' "$2"; cat "$3"; printf '</script></body></html>'; } > "$G/site/public/$1/index.html"; }
region() { printf '<div tabindex="0" style="overflow:auto;width:200px"><div style="width:900px">%s</div></div>' "$1"; }
# a strip in the kit's markup (site/templates/kit/code.html): the first tab is selected, $2 is what the second panel holds
strip() { printf '<div data-tabs><div role="tablist" hidden><button type="button" role="tab" data-kit-tab id="%s-t0" aria-controls="%s-p0">one</button><button type="button" role="tab" data-kit-tab id="%s-t1" aria-controls="%s-p1">two</button></div><div role="tabpanel" id="%s-p0">first</div><div role="tabpanel" id="%s-p1">%s</div></div>' "$1" "$1" "$1" "$1" "$1" "$1" "$2"; }
# the first press of each tab moves the page under the pointer, so that press's click event goes elsewhere
cp "$ROOT/site/kit.js" kit-shift.js
printf '%s' ';document.querySelectorAll("[role=tab]").forEach(function(t){t.addEventListener("mousedown",function(){var s=document.createElement("div");s.style.height="400px";document.body.insertBefore(s,document.body.firstChild)},{once:true})})' >> kit-shift.js
# a tab that answers Enter and not the pointer
printf '%s' 'document.querySelectorAll("[role=tab]").forEach(function(t){var p=document.getElementById(t.getAttribute("aria-controls"));p.style.display="none";t.addEventListener("keydown",function(e){if(e.key==="Enter")p.style.display=""})})' > keys.js
: > none.js
kpage revealed "$(strip a "$(region wide)")" "$ROOT/site/kit.js"
kpage nested "$(strip a "$(strip b "$(region wide)")")" "$ROOT/site/kit.js"
kpage moving "<p>above</p>$(strip a "$(region wide)")" kit-shift.js
kpage keys "<button type=\"button\" role=\"tab\" id=\"k-t1\" aria-controls=\"k-p1\">two</button><div id=\"k-p1\">$(region one)$(region two)</div>" keys.js
kpage orphan "<div style=\"display:none\">$(region wide)</div>" none.js
real() { rc=0; MJ_ROOT="$G" MJ_INTERACTION_SPECS="$ROOT/scripts/lib/interaction-specs" MJ_PROBE_JOBS=1 "$PROBE" "$@" > out.txt 2>&1 || rc=$?; }

real --only /revealed/ --spec scroll-region
[ "$rc" = 0 ] || { echo "    a region in a hidden kit tab panel was not revealed by its tab (exit $rc)"; cat out.txt; exit 1; }
expect_grep '^OK   behaviour    1 control\(s\) on 1 page\(s\)' out.txt
expect_no_grep 'did not reach it' out.txt

real --only /nested/ --spec scroll-region
[ "$rc" = 0 ] || { echo "    a region under two nested hidden panels was not revealed, panel by panel (exit $rc)"; cat out.txt; exit 1; }

real --only /moving/ --spec scroll-region
[ "$rc" = 0 ] || { echo "    a click the tab never heard was not made once more (exit $rc)"; cat out.txt; exit 1; }
expect_grep '/moving/: \[scroll-region\] tab a-t1 was clicked and the click event did not reach it; clicking it once more' out.txt
real --only /moving/ --spec kit-tabs
[ "$rc" = 0 ] || { echo "    kit-tabs did not make a click the tab never heard once more (exit $rc)"; cat out.txt; exit 1; }
expect_grep '/moving/: \[kit-tabs\] tab 1 of 2 was clicked and the click event did not reach it; clicking it once more' out.txt
expect_grep '^OK   behaviour    2 control\(s\) on 1 page\(s\)' out.txt

real --only /keys/ --spec scroll-region
[ "$rc" = 10 ] || { echo "    a tab that answers Enter and not the pointer exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep 'FAIL behaviour +/keys/: \[scroll-region\] a div scroll region is hidden: its tab k-t1 was chosen and the panel stayed hidden \(the tab.s aria-selected="null", panel.hidden=false, its display is none\)' out.txt
expect_grep '^interaction-probe: 2 finding\(s\)' out.txt
expect_no_grep 'did not reach it' out.txt

real --only /orphan/ --spec scroll-region
[ "$rc" = 10 ] || { echo "    a hidden region nothing reveals exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep 'FAIL behaviour +/orphan/: \[scroll-region\] a div scroll region is hidden by div and nothing on the page reveals it' out.txt
