# majordomus-covers: none
# The UI audit runs as N shards side by side (I3940), cut by the stride scripts/site-probe
# --partition cuts routes by (I2841, case 3602), over the visits of the plan instead. One serial
# audit took 57-80 minutes (runs 39762797718, 39780445745) and was the longest pole of a full
# run; splitting it must not drop a visit or drive one twice, because an audit that silently
# skips a page is the failure the full audit exists to prevent. This case holds the rule on a
# fixed fixture surface, with no browser and no server:
#
#   partition   for n in {1, 4}, the visits of the fixture's plan fall to exactly one of the n
#               shards, the shards' sizes differ by at most one, a page's widths may be split
#               between shards, and a second call gives the same shards
#   union       `scripts/ui union` joins the four shards' documents into the one document a
#               single audit writes (same visits, same findings, same pages) and refuses a set
#               with a shard missing, a visit twice, a visit missing or two different plans
#   refused     a malformed MJ_UI_SHARD is refused (exit 2) before a browser is opened or a
#               site is built
#   wired       the workflow runs ui-shard 1..N, each told MJ_UI_SHARD=<shard>/N under a bound;
#               the bar, the warm-screen budget and the browser cases run once, in ui-checks,
#               never in a shard; and the ui job joins the shards with `scripts/ui union`
#
# A mutation of shardPages that keeps every visit in every shard, or drops the last stride, turns
# `partition` red; a union that does not count turns `union` red; a matrix that loses a shard
# turns `wired` red.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || skip "no node"
UI="$ROOT/scripts/lib"
WF="$ROOT/.github/workflows/validate.yml"

# --- the wiring first
job_of() { awk -v j="  $1:" '$0 == j {f=1; next} f && /^  [a-z][a-z-]*:$/{exit} f' "$WF"; }
shardjob="$(job_of ui-shard)"
[ -n "$shardjob" ] || { echo "    FAIL: validate.yml has no ui-shard job"; exit 1; }
shards="$(printf '%s\n' "$shardjob" | sed -n 's/^ *shard: *\[\(.*\)\] *$/\1/p' | tr -d ' ')"
n="$(printf '%s\n' "$shards" | tr ',' '\n' | grep -c .)"
[ "$shards" = "$(seq 1 "$n" | paste -sd, -)" ] || { echo "    FAIL: the ui-shard matrix is [$shards], not 1..$n"; exit 1; }
printf '%s\n' "$shardjob" | grep -qF "MJ_UI_SHARD: \${{ matrix.shard }}/$n" \
  || { echo "    FAIL: the ui-shard job does not hand each shard MJ_UI_SHARD=<shard>/$n"; exit 1; }
printf '%s\n' "$shardjob" | grep -qE 'scripts/ci/timed --bound [0-9]+ ui-audit .*scripts/ui audit' \
  || { echo "    FAIL: the ui-shard job does not run the audit under a bound"; exit 1; }
for once in 'scripts/ui navbar' 'bench system.http.' 'runs-in ui'; do
  ! printf '%s\n' "$shardjob" | grep -qF -- "$once" \
    || { echo "    FAIL: a ui-shard job runs '$once', which is judged once, in ui-checks"; exit 1; }
  [ "$(job_of ui-checks | grep -cF -- "$once")" = 1 ] \
    || { echo "    FAIL: ui-checks does not run '$once' exactly once"; exit 1; }
done
! printf '%s\n' "$shardjob" | grep -q -- '--report' || { echo "    FAIL: a shard renders the report, which the joined document owns"; exit 1; }
agg="$(job_of ui)"
printf '%s\n' "$agg" | grep -q 'scripts/ui union --from' || { echo "    FAIL: the ui job does not join the shards"; exit 1; }
printf '%s\n' "$agg" | grep -qE 'needs: \[plan, ui-shard, ui-checks\]' || { echo "    FAIL: the ui job does not wait for the shards and the checks"; exit 1; }
! printf '%s\n' "$agg" | grep -qE '^ *run:.*scripts/ui audit' || { echo "    FAIL: the ui job still drives the audit beside the shards"; exit 1; }
echo "    ok: the workflow runs ui-shard 1..$n, each told MJ_UI_SHARD=<shard>/$n; the bar, the budget and the cases run once; the ui job joins the shards"

# --- a malformed shard is refused before anything is built or opened
for bad in 4/3 0/4 1/0 a/4 4 1/ /4 -1/4 1/4/4; do
  rc=0; MAJORDOMUS_BIN=/nonexistent MJ_UI_SHARD="$bad" "$ROOT/scripts/ui" audit > out.txt 2>&1 || rc=$?
  [ "$rc" = 2 ] && grep -q 'MJ_UI_SHARD must be K/N' out.txt \
    || { echo "    FAIL: MJ_UI_SHARD=$bad exited $rc"; cat out.txt; exit 1; }
done
echo "    ok: a malformed MJ_UI_SHARD is refused (exit 2) before a site is built or a browser is opened"

# --- the fixture: two built surfaces and a theme, planned as the audit plans them
mkdir -p a/docs/x a/docs/y a/docs/z a/guide b/one b/two
for p in a/index.html a/docs/index.html a/docs/x/index.html a/docs/y/index.html a/docs/z/index.html a/guide/index.html b/index.html b/one/index.html b/two/index.html; do
  printf '<html></html>' > "$p"
done
cat > theme.css <<'CSS'
@media (min-width: 640px) { .a { color: red } }
@media (min-width: 1024px) { .b { color: red } }
CSS
cat > plan.mjs <<'MJS'
import { planSurfaces } from "@UI@/ui-discover.mjs";
import { shardPages, visitsOf, visitId } from "@UI@/ui-shard.mjs";
const surfaces = [
  { id: "app", mount: "/", kind: "static-directory", dir: "@T@/a" },
  { id: "other", mount: "/other", kind: "static-directory", dir: "@T@/b" },
];
const pages = planSurfaces(surfaces, "@T@/theme.css").pages;
const whole = visitsOf(pages).map((v) => visitId(v.route, v.width));
const out = { whole, shards: {} };
for (const n of [1, 4]) {
  out.shards[n] = [];
  for (let k = 1; k <= n; k++) out.shards[n].push(visitsOf(shardPages(pages, { index: k, of: n })).map((v) => visitId(v.route, v.width)));
}
console.log(JSON.stringify(out));
MJS
sed "s|@UI@|$UI|;s|@T@|$PWD|" plan.mjs > plan.run.mjs
node plan.run.mjs > one.json
node plan.run.mjs > two.json
cmp -s one.json two.json || { echo "    FAIL: two calls cut the plan differently"; exit 1; }
cat > check.mjs <<'MJS'
import { readFileSync } from "node:fs";
const { whole, shards } = JSON.parse(readFileSync("one.json", "utf8"));
const fail = (m) => { console.log(`    FAIL: ${m}`); process.exit(1); };
if (whole.length < 20) fail(`the fixture plan has only ${whole.length} visits`);
if (new Set(whole).size !== whole.length) fail("the plan itself lists a visit twice");
for (const [n, parts] of Object.entries(shards)) {
  const all = parts.flat();
  if (parts.length !== Number(n)) fail(`${n} shards were cut as ${parts.length}`);
  if (all.length !== whole.length) fail(`${n} shard(s) drove ${all.length} visits of ${whole.length}`);
  if (new Set(all).size !== all.length) fail(`${n} shard(s) drove a visit twice`);
  if (JSON.stringify([...all].sort()) !== JSON.stringify([...whole].sort())) fail(`${n} shard(s) did not drive the plan's visits`);
  const sizes = parts.map((p) => p.length);
  if (Math.max(...sizes) - Math.min(...sizes) > 1) fail(`${n} shard(s) hold ${sizes.join(", ")} visits`);
}
if (JSON.stringify(shards[1][0]) !== JSON.stringify(whole)) fail("the one shard of one is not the plan, in its order");
const routes = new Set(shards[4].flatMap((p, i) => p.map((id) => id.slice(0, id.lastIndexOf("@")) + "#" + i)));
const pages = new Set(whole.map((id) => id.slice(0, id.lastIndexOf("@"))));
if (routes.size <= pages.size) fail("no page had its widths split between shards: the stride is not over visits");
console.log(`    ok: ${whole.length} visit(s) of ${pages.size} page(s): one shard drives all of them, four drive each exactly once (${shards[4].map((p) => p.length).join("+")}), and a second call cuts the same`);
MJS
node check.mjs || exit 1

# --- the union: four shards' documents are the one document a single audit writes
cat > docs.mjs <<'MJS'
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { RESULTS_SCHEMA } from "@UI@/ui-results.mjs";
const { whole, shards } = JSON.parse(readFileSync("one.json", "utf8"));
const doc = (driven, shard, extra = {}) => ({
  schema: RESULTS_SCHEMA, origin: "http://127.0.0.1:1", complete: shard.of === 1, source: { pages: "fixture", viewports: "fixture" },
  breakpoints: [640, 1024], viewports: [320, 639, 640, 1023, 1024, 1440], surfaces: [],
  pages: new Set(driven.map((id) => id.slice(0, id.lastIndexOf("@")))).size, visits: driven.length, seconds: 10 + shard.index,
  concurrency: 4, shard: { ...shard, planned: whole.length, narrowed: false, driven },
  rules: {}, foreign: [],
  findings: driven.filter((id) => id.endsWith("@320") && id.startsWith("/docs/x/")).map((id) => ({ route: id.slice(0, id.lastIndexOf("@")), width: 320, rule: "responsive.overflow" })),
  ...extra,
});
const put = (dir, k, d) => { mkdirSync(`${dir}/ui-audit-shard-${k}`, { recursive: true }); writeFileSync(`${dir}/ui-audit-shard-${k}/run-ui-shard-${k}.json`, JSON.stringify(d)); };
const mode = process.argv[2];
const four = shards[4].map((driven, i) => doc(driven, { index: i + 1, of: 4 }));
if (mode === "whole") put("whole", 1, doc(whole, { index: 1, of: 1 }));
else {
  four.forEach((d, i) => {
    if (mode === "missing" && i === 2) return;
    if (mode === "twice" && i === 1) d = doc([...d.shard.driven, whole[0]], { index: 2, of: 4 });
    if (mode === "dropped" && i === 3) d = doc(d.shard.driven.slice(1), { index: 4, of: 4 });
    if (mode === "alike" && i === 0) d.viewports = [320, 1440];
    put(mode, i + 1, d);
  });
}
MJS
sed "s|@UI@|$UI|" docs.mjs > docs.run.mjs
for mode in whole four missing twice dropped alike; do node docs.run.mjs "$mode"; done

# one audit's document, and the four shards' joined: the same visits, findings and pages
expect_exit 10 "$ROOT/scripts/ui" union --from whole --out whole.out.json   # a finding fails, as a single audit's does
expect_exit 10 "$ROOT/scripts/ui" union --from four --out four.out.json
expect_grep 'OK   shards     4 shard\(s\) drove all [0-9]+ visit\(s\) of [0-9]+ page\(s\), each once' -
expect_grep '1 finding\(s\)|[0-9]+ finding\(s\)' -
node --input-type=module -e '
import { readFileSync } from "node:fs";
const a = JSON.parse(readFileSync("whole.out.json", "utf8")), b = JSON.parse(readFileSync("four.out.json", "utf8"));
const pick = (d) => JSON.stringify({ pages: d.pages, visits: d.visits, findings: d.findings, rules: d.rules });
if (pick(a) !== pick(b)) { console.log("    FAIL: the four shards joined are not the one audit"); console.log(pick(a)); console.log(pick(b)); process.exit(1); }
if (!b.complete || b.shard) { console.log("    FAIL: the joined document is partial or still names a shard"); process.exit(1); }
console.log(`    ok: four shards joined are the single audit: ${b.visits} visit(s), ${b.pages} page(s), ${b.findings.length} finding(s)`);
'
for mode in missing twice dropped alike; do
  rm -f "$mode.out.json"
  expect_exit 10 "$ROOT/scripts/ui" union --from "$mode" --out "$mode.out.json"
  expect_grep '^FAIL shards ' -
  [ ! -f "$mode.out.json" ] || { echo "    FAIL: a set with problems ($mode) was written as if it were the audit"; exit 1; }
done
expect_exit 10 "$ROOT/scripts/ui" union --from missing --out missing.out.json
expect_grep 'not each of 1\.\.4 once' -
expect_exit 10 "$ROOT/scripts/ui" union --from twice --out twice.out.json
expect_grep 'driven by more than one shard' -
expect_exit 10 "$ROOT/scripts/ui" union --from dropped --out dropped.out.json
expect_grep 'drove [0-9]+ visit\(s\) of the [0-9]+ the plan has' -
expect_exit 10 "$ROOT/scripts/ui" union --from alike --out alike.out.json
expect_grep 'not planned alike' -
mkdir -p empty
expect_exit 10 "$ROOT/scripts/ui" union --from empty --out empty.out.json
expect_grep 'no shard left a results document' -
expect_exit 2 "$ROOT/scripts/ui" union --from nowhere
echo "    ok: a union with a shard missing, a visit twice, a visit dropped, two plans or nothing at all is refused and writes no document"
