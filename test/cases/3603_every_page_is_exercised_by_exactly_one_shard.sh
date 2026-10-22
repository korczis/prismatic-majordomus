# majordomus-covers: none
# The interaction probe runs as N shards side by side (I3602), cut by the stride scripts/site-probe
# --partition defines for both browser probes (I2841, case 3602 holds the rule itself). This case
# holds the probe's half, on a fixture site of seven pages in a real browser:
#
#   shards     each of shards 1..3 drives exactly the pages --partition K/3 gives for the sorted
#              pages, says so in its OK line, and writes them to MJ_PROBE_REPORT
#   fan-in     scripts/site-probe --verify-report accepts the three reports, and refuses them
#              with one shard's report missing
#   finding    a broken control still fails the shard that drives it, naming the page
#   refused    a malformed MJ_PROBE_SHARD is refused (exit 2) before a browser is opened
#   wired      the workflow runs interaction-probe-shard 1..N, each told MJ_PROBE_SHARD=<shard>/N,
#              and the interactions job verifies their reports before the gate's receipt
. "$ROOT/test/lib.sh"
PROBE="$ROOT/scripts/interaction-probe"
SITEPROBE="$ROOT/scripts/site-probe"
WF="$ROOT/.github/workflows/validate.yml"

# --- the wiring first: it needs no browser
job_of() { awk -v j="  $1:" '$0 == j {f=1; next} f && /^  [a-z][a-z-]*:$/{exit} f' "$WF"; }
job="$(job_of interaction-probe-shard)"
[ -n "$job" ] || { echo "    FAIL: validate.yml has no interaction-probe-shard job"; exit 1; }
shards="$(printf '%s\n' "$job" | sed -n 's/^ *shard: *\[\(.*\)\] *$/\1/p' | tr -d ' ')"
n="$(printf '%s\n' "$shards" | tr ',' '\n' | grep -c .)"
[ "$shards" = "$(seq 1 "$n" | paste -sd, -)" ] || { echo "    FAIL: the interaction-probe-shard matrix is [$shards], not 1..$n"; exit 1; }
printf '%s\n' "$job" | grep -qF "MJ_PROBE_SHARD: \${{ matrix.shard }}/$n" \
  || { echo "    FAIL: the interaction-probe-shard job does not hand each shard MJ_PROBE_SHARD=<shard>/$n"; exit 1; }
printf '%s\n' "$job" | grep -q 'scripts/interaction-probe' || { echo "    FAIL: the shard job does not run the probe"; exit 1; }
fanin="$(job_of interactions)"
printf '%s\n' "$fanin" | grep -q 'scripts/site-probe --verify-report' \
  || { echo "    FAIL: the interactions job does not verify the shards' reports"; exit 1; }
! printf '%s\n' "$fanin" | grep -q '^ *run:.*scripts/interaction-probe' \
  || { echo "    FAIL: the interactions job still drives the whole probe beside the shards"; exit 1; }
echo "    ok: the workflow runs shards 1..$n, each told MJ_PROBE_SHARD=<shard>/$n, and the interactions job verifies them"

F="$PWD/site"; mkdir -p "$F/site/public" "$F/docs/generated" specs
rc=0; MJ_ROOT="$F" MJ_PROBE_SHARD=4/3 "$PROBE" > out.txt 2>&1 || rc=$?
[ "$rc" = 2 ] && grep -q 'MJ_PROBE_SHARD must be K/N' out.txt \
  || { echo "    FAIL: MJ_PROBE_SHARD=4/3 exited $rc"; cat out.txt; exit 1; }
echo "    ok: a malformed shard is refused before a browser is opened"

# --- the probe itself, in a browser
command -v node >/dev/null 2>&1 || skip "no node, so the shards were not driven"
[ -d "$ROOT/node_modules/playwright" ] || { [ "${CI:-}" = true ] && { echo "    playwright absent under CI"; exit 1; }; skip "playwright absent (npm ci)"; }
printf 'base_url = "https://fixture.test"\n' > "$F/site/config.toml"
printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$F/docs/generated/web.json"
good='<button type="button" x-on:click="n++" id="c">0</button><script>document.getElementById("c").onclick=function(){this.textContent=String(Number(this.textContent)+1)}</script>'
for p in a b c d e f; do mkdir -p "$F/site/public/$p"; printf '<!doctype html><html><body>%s</body></html>' "$good" > "$F/site/public/$p/index.html"; done
mkdir -p "$F/site/public/g"; printf '<!doctype html><html><body><button type="button" x-on:click="n++" id="c">0</button></body></html>' > "$F/site/public/g/index.html"
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
printf '/a/\n/b/\n/c/\n/d/\n/e/\n/f/\n/g/\n' > pages
for k in 1 2 3; do
  mkdir -p "reports/$k"
  rc=0; MJ_ROOT="$F" MJ_INTERACTION_SPECS="$PWD/specs" MJ_PROBE_JOBS=2 MJ_PROBE_SHARD="$k/3" \
    MJ_PROBE_REPORT="$PWD/reports/$k/interaction-probe-routes.txt" "$PROBE" > "out.$k" 2>&1 || rc=$?
  if grep -q '^SKIP interaction-probe: no browser' "out.$k"; then
    [ "${CI:-}" = true ] && { echo "    the probe skipped under CI"; exit 1; }
    skip "no browser could be started"
  fi
  want="$("$SITEPROBE" --partition "$k/3" < pages)"
  got="$(tail -n +2 "reports/$k/interaction-probe-routes.txt" 2>/dev/null)"
  [ "$got" = "$want" ] || { echo "    FAIL: shard $k/3 drove [$(echo $got)], not [$(echo $want)]"; cat "out.$k"; exit 1; }
  head -1 "reports/$k/interaction-probe-routes.txt" | grep -qx "# shard $k/3 of 7 routes" \
    || { echo "    FAIL: shard $k/3's report header is '$(head -1 "reports/$k/interaction-probe-routes.txt")'"; exit 1; }
  if printf '%s\n' "$want" | grep -qx '/g/'; then
    [ "$rc" = 10 ] && grep -qF '/g/: [counter] pressing the counter did not add one' "out.$k" \
      || { echo "    FAIL: the shard holding the broken /g/ exited $rc without naming it"; cat "out.$k"; exit 1; }
  else
    [ "$rc" = 0 ] && grep -q "^OK   behaviour    $(printf '%s\n' "$want" | grep -c .) control(s) on $(printf '%s\n' "$want" | grep -c .) page(s) (shard $k/3 of 7)" "out.$k" \
      || { echo "    FAIL: shard $k/3 exited $rc or did not say which shard it drove"; cat "out.$k"; exit 1; }
  fi
done
echo "    ok: shards 1..3 each drove exactly their stride of the 7 pages and reported it; the broken page failed its own shard"

"$SITEPROBE" --verify-report reports > v.out 2>&1 || { echo "    FAIL: the three shards' reports were refused"; cat v.out; exit 1; }
rm -r reports/2
rc=0; "$SITEPROBE" --verify-report reports > v.out 2>&1 || rc=$?
[ "$rc" = 10 ] || { echo "    FAIL: two of three shards' reports were accepted (exit $rc)"; cat v.out; exit 1; }
echo "    ok: the fan-in accepts the three shards' reports and refuses them with one missing"
