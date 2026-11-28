# majordomus-covers: none
# majordomus-skip-runs-in: ui
# A move on the published workspace reads "live only" (I2910), held in a real browser over the
# UI's real snapshot of a fixture repository of this case's own. A publication has no process
# behind it, so a move there can do nothing; it must not look as if it could:
#
#   published   `web report ui-snapshot` writes the intent workspace as files
#   disabled    every move on the published /ui/idd — the one primary action and each move in the
#               table of the work — is a disabled control, and no link or enabled button offers one
#   says why    each reads "live only", beside the move it would be
#   leads live  "live only" is a link, and following it opens the snapshot's page about the live
#               UI, carrying the route of the move it stands for
#   live        the same workspace served by a process still offers the moves as links
#
# Needs a browser scripts/lib/browser.mjs can launch; like case 3220 it is run in the ui job
# (majordomus-skip-runs-in: ui), where a skip is a failure.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
if [ -n "${CI:-}" ] && [ "${MAJORDOMUS_RUNS_IN:-}" != ui ]; then
  skip "a published move is read in the ui job (--runs-in ui), not in this CI run"
fi
mj_runs node || skip "$(mj_runs_why node)"
[ -d "$ROOT/node_modules/playwright" ] \
  || skip "playwright is not installed (npm ci), so no published move was read"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_HTTP_HOST

# ---------------------------------------------------------------- the fixture
"$MJ" init >/dev/null
pj_init
pj_milestone loop 1
pj_issue I0001 loop
pj_issue I0002 loop
printf 'serves:\n  - the-move-is-live#it-moves\n' >> .ai/repo/project/issues/I0001.yaml
mkdir -p .ai/repo/project/intents
cat > .ai/repo/project/intents/the-move-is-live.yaml <<'YAML'
id: the-move-is-live
title: The move is live
statement: "A move is offered only where it can be made."
invariants:
  - A published page offers no move
milestones:
  - loop
satisfaction:
  - id: it-moves
    criterion: A move is made where a process stands behind it
    evidence: test
    ref: checks/it-moves.sh
YAML
git add -A >/dev/null && git commit -qm "an intent with two ready issues" >/dev/null

# ---------------------------------------------------------------- published
S="$PWD/target/web/ui-snapshot"
"$RB" web report ui-snapshot --limit 40 >/dev/null 2> snapshot.err \
  || { echo "    the snapshot was not written:"; tail -5 snapshot.err | sed 's/^/    | /'; exit 1; }
[ -f "$S/idd/index.html" ] || { echo "    the snapshot holds no intent workspace"; exit 1; }
# nothing a process would have to answer is left for a script to turn into a move
if grep -q 'data-live-move' "$S/idd/index.html"; then
  echo "    the published workspace still carries an unrewritten move"; exit 1
fi

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1

# ---------------------------------------------------------------- the reading
cat > read.mjs <<'JS'
import { createServer } from 'node:http';
import { readFileSync, existsSync, statSync } from 'node:fs';
import { join, extname } from 'node:path';
const [root, browserLib, live] = process.argv.slice(2);
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.svg': 'image/svg+xml', '.json': 'application/json' };
// the snapshot served as a publication serves it: files, and nothing that answers a query
const server = createServer((req, res) => {
  let path = join(root, decodeURIComponent(new URL(req.url, 'http://x').pathname));
  if (existsSync(path) && statSync(path).isDirectory()) path = join(path, 'index.html');
  if (!existsSync(path)) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': types[extname(path)] || 'application/octet-stream' });
  res.end(readFileSync(path));
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${server.address().port}`;
const { launch } = await import(browserLib);
const browser = await launch();
if (!browser) { console.log('NO-BROWSER'); server.close(); process.exit(3); }
const fail = (why) => { console.log('    ' + why); process.exitCode = 1; throw new Error(why); };
const MOVES = /^(start|verify|done|Start I\d+|Verify I\d+|Mark I\d+ done)$/;
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 1200 } });
  await page.goto(`${base}/idd/index.html`, { waitUntil: 'load' });
  if (!(await page.locator('[data-snapshot-of]').count())) fail('the page read is not the snapshot');

  // disabled: no link and no enabled button anywhere in the page offers a move
  const offered = await page.evaluate((src) => {
    const re = new RegExp(src);
    return [...document.querySelectorAll('main a, main button')]
      .filter((e) => re.test(e.textContent.trim()) && !(e.tagName === 'BUTTON' && e.disabled))
      .map((e) => `${e.tagName.toLowerCase()} "${e.textContent.trim()}"`);
  }, MOVES.source);
  if (offered.length) fail(`the published workspace offers moves: ${offered.join(', ')}`);

  // says why: each move is a disabled control, and "live only" stands beside it
  const moves = page.locator('[data-live-only]');
  const n = await moves.count();
  if (n < 3) fail(`${n} move(s) read "live only"; the primary action and both ready issues' moves should`);
  for (let k = 0; k < n; k++) {
    const m = moves.nth(k);
    const control = m.locator('button');
    if (!(await control.isDisabled())) fail(`move ${k} is not disabled`);
    if (!MOVES.test((await control.innerText()).trim())) fail(`move ${k} does not say which move it is`);
    const why = (await m.locator('a').innerText()).trim();
    if (why !== 'live only') fail(`move ${k} reads "${why}", not "live only"`);
  }
  const primary = page.locator('#next-move [data-live-only] button');
  if ((await primary.innerText()).trim() !== 'Start I0001') fail('the primary action is not the disabled "Start I0001"');
  console.log(`    disabled: ${n} move(s) on the published workspace, each a disabled control reading "live only"`);

  // leads live: "live only" opens the page about the live UI, with the move's route
  await page.locator('#next-move [data-live-only] a').click();
  await page.waitForURL(/\/live\/index\.html#/);
  const fragment = decodeURIComponent(new URL(page.url()).hash);
  if (!fragment.includes('/ui/idd') || !fragment.includes('issue=I0001')) fail(`"live only" leads to ${page.url()}, not the move's live route`);
  console.log(`    leads live: "live only" opens the live page for ${fragment.slice(1)}`);

  // live: the process still offers the moves
  const served = await browser.newPage();
  await served.goto(`${live}/ui/idd?intent=the-move-is-live`, { waitUntil: 'networkidle' });
  if (await served.locator('a.mj-btn--primary').innerText() !== 'Start I0001') fail('the live workspace offers no primary move');
  if (await served.locator('[data-live-only]').count()) fail('the live workspace says "live only"');
  console.log('    live: the served workspace offers "Start I0001" as a link, and nothing reads "live only"');
} catch (error) {
  if (!process.exitCode) { console.log(`    ${error.message}`); process.exitCode = 1; }
} finally {
  await browser.close();
  server.close();
}
JS
rc=0
node read.mjs "$S" "$ROOT/scripts/lib/browser.mjs" "$U" > read.out 2>&1 || rc=$?
cat read.out
case "$rc" in
  0) ;;
  3) skip "no browser could be launched, so no published move was read" ;;
  *) echo "    the reading failed (exit $rc)"; exit 1 ;;
esac
echo "    every move on the published workspace is disabled, reads \"live only\" and leads to the live UI"
