# majordomus-covers: none
# The behaviour probe drives every control in a browser and refuses what does not hold: scripts/interaction-probe,
# run against a fixture site and fixture specs this case writes, so the verdict for each page is known before the
# probe runs. Rule: project.every-link-and-control-is-tested.
#
# What is proved: a page whose control does what its spec says passes and is counted; and each of these fails with
# a finding that names the page and the spec: a control whose behaviour is broken, a live page holding a different
# number of a spec's controls than its markup declares, a console error, a request to an origin that is not the
# site's own, and a spec that drives fewer controls than it claims. The pages are driven heaviest first; a spec or a
# page that never answers is a finding naming the page and the step once MJ_PROBE_SPEC_SECONDS or
# MJ_PROBE_ROUTE_SECONDS runs out, and the next page is still driven; a SIGTERM names the pages in flight (F02).
# It needs a browser: without one it skips
# locally and fails under CI=true, because a behaviour nobody ran is not a behaviour that holds. The pages driven
# are the site's own: a broken control under /ref, a mount the fixture's topology composes (ADR 0086), is that
# surface's business and is not driven; under a mount the topology does not compose it is, and fails.
. "$ROOT/test/lib.sh"
mj_runs node || skip "$(mj_runs_why node)"
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

# ---------------------------------------------------------------- the heaviest page first; a page that hangs is named (F02)
# One worker, so the order the trace spec meets the pages in is the order they were driven. /z-heavy/ declares three
# controls and sorts last by name: driven first, it was ordered by its cost. /hung/'s spec never answers, as a resize
# into a hung renderer did on CI: under the old pool it held its worker until the job's bound killed the shard with
# nothing printed. Here it must be a finding naming the page, the spec and the step, inside a bound this case keeps
# itself, and the page after it must still be driven.
mkdir -p "$F/site/public/a" "$F/site/public/m" "$F/site/public/z-heavy" "$F/site/public/hung"
page a '<button type="button" x-on:click="trace">t</button>'
page m '<button type="button" x-on:click="trace">t</button>'
page z-heavy '<button type="button" x-on:click="trace">1</button><button type="button" x-on:click="trace">2</button><button type="button" x-on:click="trace">3</button>'
page hung '<button type="button" x-on:click="hang">h</button>'
cat > specs/trace.mjs <<'JS'
import { appendFileSync } from 'node:fs';
export default {
  id: 'trace', title: 'a spec that says which page it was run on',
  claims: (el) => el.attrs['x-on:click'] === 'trace',
  async exercise({ controls, route }) { appendFileSync(process.env.MJ_CASE_TRACE, `${route}\n`); return controls.length; },
};
JS
cat > specs/hang.mjs <<'JS'
import { appendFileSync } from 'node:fs';
export default {
  id: 'hang', title: 'a spec whose page never answers',
  claims: (el) => el.attrs['x-on:click'] === 'hang',
  async exercise({ route }) { appendFileSync(process.env.MJ_CASE_TRACE, `${route}\n`); await new Promise(() => {}); return 1; },
};
JS
# the probe in the background under a bound of $1 seconds, so a probe that hangs fails this case instead of holding
# it; with MJ_CASE_TERM_ON set, it is sent SIGTERM once the trace names that page, as scripts/ci/timed sends it. TERM
# comes first either way, so the browser is closed by the probe's own handlers rather than orphaned by a KILL
bounded() {
  local bound="$1" i=0 pid; shift; rc=0; : > trace
  MJ_ROOT="$F" MJ_INTERACTION_SPECS="$PWD/specs" MJ_PROBE_JOBS=1 MJ_CASE_TRACE="$PWD/trace" "$PROBE" "$@" > out.txt 2>&1 &
  pid=$!
  while kill -0 "$pid" 2>/dev/null; do
    if [ -n "${MJ_CASE_TERM_ON:-}" ] && grep -qxF "$MJ_CASE_TERM_ON" trace; then sleep 1; break; fi
    [ "$i" -lt $((bound * 10)) ] || { rc=timeout; break; }
    sleep 0.1; i=$((i + 1))
  done
  kill -TERM "$pid" 2>/dev/null || true
  for i in $(seq 1 100); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
  kill -KILL "$pid" 2>/dev/null || true
  if [ "$rc" = timeout ]; then wait "$pid" 2>/dev/null || true; else wait "$pid" || rc=$?; fi
}
start=$SECONDS
MJ_PROBE_SPEC_SECONDS=2 MJ_PROBE_ROUTE_SECONDS=30 bounded 60 --only '/a/ /hung/ /m/ /z-heavy/'
took=$((SECONDS - start))
[ "$rc" = 10 ] || { echo "    a page whose spec hangs ended the probe with '$rc' after ${took}s, not exit 10 with a finding"; cat out.txt; exit 1; }
expect_grep 'FAIL behaviour +/hung/: \[hang\] did not finish within 2\.5s: still at hang: exercise$' out.txt
[ "$(grep -c '^FAIL' out.txt)" = 1 ] || { echo "    the hung page's closed context was reported as more than its one finding"; cat out.txt; exit 1; }
order="$(tr '\n' ' ' < trace)"
[ "$order" = '/z-heavy/ /a/ /hung/ /m/ ' ] \
  || { echo "    the pages were driven in the order [$order], not heaviest first, then by route, /m/ after /hung/"; cat out.txt; exit 1; }
expect_grep 'p50 [0-9]+s, p95 [0-9]+s per page; slowest: /hung/ [0-9]+s' out.txt
echo "    ok: the heaviest page was driven first, and a hung spec was a finding naming its page and step in ${took}s"

# the page's own deadline names the step a page was at when it ran out, whichever spec it was in
MJ_PROBE_SPEC_SECONDS=60 MJ_PROBE_ROUTE_SECONDS=2 bounded 60 --only /hung/
[ "$rc" = 10 ] || { echo "    a page past its deadline ended the probe with '$rc', not exit 10"; cat out.txt; exit 1; }
expect_grep 'FAIL behaviour +/hung/: did not finish within 2\.5s: still at hang: exercise$' out.txt

# stopped from outside, the probe names every page still in flight and its step before it exits
MJ_PROBE_SPEC_SECONDS=120 MJ_PROBE_ROUTE_SECONDS=120 MJ_CASE_TERM_ON=/hung/ bounded 60 --only /hung/
[ "$rc" = 143 ] || { echo "    a SIGTERM ended the probe with '$rc', not 143"; cat out.txt; exit 1; }
expect_grep 'interaction-probe: SIGTERM after [0-9]+s, 0/1 page\(s\) done; in flight:' out.txt
expect_grep '^  /hung/ at hang: exercise, [0-9]+s$' out.txt

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
