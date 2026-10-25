# majordomus-covers: none
# majordomus-skip-runs-in: ui
# The IDD loop, walked end to end in a real browser over the real UI and a real server on one
# fixture repository (I2812, the owner's acceptance for v1.0.0-alpha.1). Nothing here calls a
# capability the UI would not; every move is a click, and the page is never reloaded by the
# test:
#
#   pick        the intent is chosen in /ui/idd from the list of intents, and its title is the
#               page's heading with the screen's name above it (I3001)
#               and its panels stand in the order a person asks (I2916); it offers one
#               primary action, the move its next issue's status admits: Start I0001 (I2917)
#   read        its issues with their status, and the dependency: I0002 waits for I0001; every
#               issue id on the screen is a link, and the issues that share a scope (I0001 and
#               I0003) are one counted line that opens to the pairs (I2900)
#   move        I0001 is started from its move form, opened by the primary action; the row
#               reads active
#   work        the work I0001 owes is written into its scope and committed
#   test        the issue's own check, a real test of that work, fails before the work and
#               passes after it; its command and result are recorded as I0001's evidence
#   live        the first tab reopens the workspace with its channel held closed, I0001 is
#               verified from a second tab, and once the channel opens the first tab shows
#               the move without navigating: the server replays what ended before a channel
#               opened, so the race between a move and a channel's opening is held, not timed
#   done        I0001 is done from the first tab; I0002 becomes ready
#   explained   the workspace's standing panel explains the intent from
#               intent_realization.explain, and the Cockpit's issue page shows the
#               validation command and the evidence present; the criterion's code span is
#               drawn as code and no backtick reaches the reader (I3004)
#   reproduced  the recorded command, run again, gives the recorded result
#   clean       no console error, no uncaught exception, no failed request; a distribution
#               without the vendored graph library is never asked for it (I2900)
#
# Needs a browser scripts/lib/browser.mjs can launch. The suite runners have none, so the case
# defers to the ui job (majordomus-skip-runs-in: ui), which has one and runs it with
# `--no-skips --runs-in ui`, where a skip is a failure.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
# The loop is walked where it is declared to be (majordomus-skip-runs-in: ui), not wherever a
# browser happens to exist: on a loaded suite shard its live step met a screen that never settled
# (runs 39087400247, 39097403850), while the ui job is the run this case is written for.
# A local run without CI still walks it.
if [ -n "${CI:-}" ] && [ "${MAJORDOMUS_RUNS_IN:-}" != ui ]; then
  skip "the IDD loop is walked in the ui job (--runs-in ui), not in this CI run"
fi
command -v node >/dev/null 2>&1 || skip "node is absent, so the IDD loop was not walked"
[ -d "$ROOT/node_modules/playwright" ] \
  || skip "playwright is not installed (npm ci), so the IDD loop was not walked"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST

# ---------------------------------------------------------------- the fixture
"$MJ" init >/dev/null
pj_init
pj_milestone loop 1
pj_issue I0001 loop
pj_issue I0002 loop I0001
# I0003 shares I0001's scope, so the waves name a pair that may not run at the same time
pj_issue I0003 loop
sed -i.bak 's#^  - src/I0003$#  - src/I0001#' .ai/repo/project/issues/I0003.yaml && rm -f .ai/repo/project/issues/I0003.yaml.bak
# titles that do not repeat an id, so that every id the screen shows is a reference to an issue
for t in "I0001:The answer is written" "I0002:The answer is read" "I0003:The answer is checked again"; do
  sed -i.bak "s#^title: Issue ${t%%:*}\$#title: ${t#*:}#" ".ai/repo/project/issues/${t%%:*}.yaml" && rm -f ".ai/repo/project/issues/${t%%:*}.yaml.bak"
done
mkdir -p checks .ai/repo/project/intents
cat > checks/the-answer-holds.sh <<'SH'
#!/bin/sh
# The check I0001's work answers to: the answer it owes is in its scope.
if [ "$(cat src/I0001/answer.txt 2>/dev/null)" = 42 ]; then echo "the answer holds"; else echo "the answer does not hold"; exit 1; fi
SH
# I0001 is checked by that script and serves the intent's one criterion
sed -i.bak 's#^  - "true"$#  - "sh checks/the-answer-holds.sh"#' .ai/repo/project/issues/I0001.yaml
rm -f .ai/repo/project/issues/I0001.yaml.bak
printf 'serves:\n  - the-answer-is-known#the-answer-holds\n' >> .ai/repo/project/issues/I0001.yaml
cat > .ai/repo/project/intents/the-answer-is-known.yaml <<'YAML'
id: the-answer-is-known
title: The answer is known
statement: "Whoever asks finds the answer where the work put it."
invariants:
  - The answer is written once, in the scope of the issue that owes it
milestones:
  - loop
satisfaction:
  - id: the-answer-holds
    criterion: The answer the work owes is in `src/I0001/answer.txt`
    evidence: test
    ref: checks/the-answer-holds.sh
YAML
git add -A >/dev/null && git commit -qm "an intent, its milestone, an issue that serves it and one that waits for it"

# the check is a real test: before the work it fails
sh checks/the-answer-holds.sh >/dev/null 2>&1 && { echo "    the check passed before the work was done"; exit 1; }

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1

# ---------------------------------------------------------------- the walk
cat > walk.mjs <<'JS'
import { execFileSync } from 'node:child_process';
import { writeFileSync, mkdirSync } from 'node:fs';
const [base, browserLib, mj, shots, graphLib] = process.argv.slice(2);
// The vendored graph library is built by scripts/cockpit-assets and not committed; a page is
// complete without it (the table beside the graph says the same). When this distribution has
// no copy, the page must not ask for it at all (I2900): no request, no 404, no console error.
const GRAPH_LIB = '/ui/assets/vendor/cytoscape.min.js';
let graphAsked = 0;
const { launch } = await import(browserLib);
const browser = await launch();
if (!browser) { console.log('NO-BROWSER'); process.exit(3); }

const problems = [];
// every wait is bounded at a minute: a loaded runner is slow, and a step that never comes is
// named by the step, not by a bare timeout
const WAIT = 60000;
let step = 'launch';
const fail = (why) => { console.log('    ' + why); process.exitCode = 1; throw new Error(why); };
const watch = (page, name) => {
  page.on('pageerror', (e) => problems.push(`${name}: uncaught ${e.message}`));
  // a failed request is named by its URL; the console's own line for it names none
  page.on('response', (r) => {
    const path = new URL(r.url()).pathname;
    if (path === GRAPH_LIB) graphAsked += 1;
    if (r.status() >= 400) problems.push(`${name}: ${r.status()} ${path}`);
  });
  page.on('console', (m) => {
    if (m.type() === 'error' && !m.text().startsWith('Failed to load resource')) problems.push(`${name}: console ${m.text()}`);
  });
  page.setDefaultTimeout(WAIT);
  page.on('dialog', (d) => d.accept()); // a move asks to confirm a write; the walker confirms
  let navigations = 0;
  page.on('framenavigated', (f) => { if (f === page.mainFrame()) navigations += 1; });
  return () => navigations;
};
const status = async (page, issue) =>
  (await page.locator(`#issue-${issue} td`).nth(2).innerText()).trim().toLowerCase();
const until = async (page, issue, want, what) => {
  try {
    await page.waitForFunction(
      ([id, w]) => (document.querySelector(`#issue-${id} td:nth-child(3)`)?.innerText || '').trim().toLowerCase() === w,
      [issue, want], { timeout: WAIT });
  } catch (e) { fail(`${what}: ${issue} never read ${want} (it reads ${await status(page, issue)})`); }
};
const sh = (cmd, args) => execFileSync(cmd, args, { encoding: 'utf8' }).trim();
// Every issue id the screen shows leads to that issue (I2900): an id in a text node that is not
// inside a link fails, named with where it sits. A path segment (src/I0001) is a path, not an id.
const noBareIds = async (page, when) => {
  const bare = await page.evaluate(() => {
    const found = [];
    const walker = document.createTreeWalker(document.querySelector('main'), NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      // a link is what is wanted; a command to copy (code, pre, kbd) keeps its text whole
      if (n.parentElement.closest('a, code, pre, kbd, script, style, [hidden]')) continue;
      for (const m of n.textContent.matchAll(/(?<![\/\w-])I\d{4}\b/g)) {
        const where = n.parentElement.closest('section[aria-label], [id]');
        const label = where ? (where.getAttribute('aria-label') || '#' + where.id) : 'main';
        found.push(`${m[0]} (in ${label}: "${n.textContent.trim().slice(0, 60)}")`);
      }
    }
    return found;
  });
  if (bare.length) fail(`${when}: issue ids on the screen that lead nowhere: ${[...new Set(bare)].join(', ')}`);
};
// A move made in a tab ends with that tab reading itself again (moves.js reloads it once the
// execution has ended): the move is clicked and that load awaited, so that nothing the tab
// does on its own is later taken for something the live channel did.
const move = async (page, transition) => {
  await page.waitForSelector(`#move [data-mj-move="${transition}"]:not([disabled])`);
  const reread = page.waitForEvent('load', { timeout: WAIT });
  await page.click(`#move [data-mj-move="${transition}"]`);
  await reread.catch(() => fail(`${transition}: the tab that made the move never read itself again`));
};

try {
  const a = await browser.newPage({ viewport: { width: 1440, height: 1400 } });
  const navigationsOfA = watch(a, 'workspace');
  // the first tab's live channel can be held closed: while `holdChannel` is set, the request
  // for the module that opens it waits (routing also turns the tab's HTTP cache off)
  let holdChannel = null;
  // and while it is held, nothing but a timer can read the first tab's screen again: its
  // catch-up and its minute's refresh fetch the screen's fragment. While `holdRefresh` is set
  // those are given up, so a move that shows there before the channel opens is the channel's
  // failure and never a refresh that happened to come first (run 39087400247, shard 4)
  let holdRefresh = false;
  let timerRefreshes = 0;
  await a.route(/[?&]fragment(&|$)/, async (route) => {
    if (holdRefresh) { timerRefreshes += 1; return route.abort(); }
    return route.continue();
  });
  await a.route('**/cockpit/assets/socket.js*', async (route) => {
    if (holdChannel) await holdChannel;
    await route.continue();
  });

  step = 'pick';
  // pick: the intent is chosen from the workspace's own list
  await a.goto(`${base}/ui/idd`, { waitUntil: 'networkidle' });
  await a.click('a[href="/ui/idd?intent=the-answer-is-known"]');
  await a.waitForURL(/intent=the-answer-is-known/);
  console.log('    pick: the workspace opened the-answer-is-known');
  // the page is titled by the intent it opened, the screen's name above it (I3001)
  const h1 = (await a.locator('h1').innerText()).trim();
  if (h1 !== 'The answer is known') fail(`the page's heading reads "${h1}", not the intent's title`);
  const eyebrow = (await a.locator('.mj-ui-eyebrow').innerText()).trim();
  if (eyebrow.toLowerCase() !== 'intent workspace') fail(`the eyebrow reads "${eyebrow}"`);
  // and it reads in the order a person asks: the intent, what it misses, the work, what was done (I2916)
  const heads = await a.locator('.mj-ui-panel-title').allInnerTexts();
  const order = ['What is this intent for?', 'What should happen next?', 'Why does it stand where it stands?', 'Which milestones carry it?',
    'What work serves it?', 'What waits for what?', 'What was done here?'].map((t) => heads.indexOf(t));
  if (order.some((i, k) => i < 0 || (k > 0 && i < order[k - 1]))) fail(`the panels stand out of order: ${heads.join(' | ')}`);
  // one primary action: the plan's next issue of the intent and the move its status admits (I2917)
  const primary = a.locator('a.mj-btn--primary');
  if (await primary.count() !== 1) fail(`the workspace offers ${await primary.count()} primary actions, not one`);
  const asks = (await primary.innerText()).trim();
  if (asks !== 'Start I0001') fail(`the primary action reads "${asks}", not "Start I0001"`);

  // read: the issues and the dependency, as the plan derives them
  if (await status(a, 'I0001') !== 'ready') fail(`I0001 reads ${await status(a, 'I0001')}, not ready`);
  // I0002's row names what it waits for, wherever the table puts it
  const waits = (await a.locator('#issue-I0002').innerText()).replace(/\s+/g, ' ').trim();
  if (!waits.includes('I0001')) fail(`I0002 does not say it waits for I0001: "${waits}"`);
  // every issue id the screen shows leads to that issue: none is bare text
  await noBareIds(a, 'read');
  // the issues that share a scope are one counted line, folded, that opens to the pairs
  const shared = a.locator('details:has(> summary:has-text("Shared scope"))');
  if (await shared.count() !== 1) fail('the shared-scope pairs are not one folded line');
  const summary = (await shared.locator('summary').innerText()).trim();
  if (!/Shared scope: \d+ pair\(s\)/.test(summary)) fail(`the shared-scope line gives no count: "${summary}"`);
  if (await shared.evaluate((d) => d.open)) fail('the shared-scope pairs are open before they are asked for');
  await shared.locator('summary').click();
  for (const id of ['I0001', 'I0003']) {
    if (!(await shared.locator(`a[href*="issue=${id}"]`).count())) fail(`the shared-scope pairs do not lead to ${id}`);
  }
  console.log(`    read: I0001 ${await status(a, 'I0001')}, I0002 ${await status(a, 'I0002')} waiting for I0001; every issue id is a link; "${summary}" opens to the pairs`);
  await a.screenshot({ path: `${shots}/1-picked.png`, fullPage: true });

  step = 'move';
  // move: start I0001 from its move form, opened by the primary action (I2917)
  await a.click('a.mj-btn--primary');
  await move(a, 'start');
  await until(a, 'I0001', 'active', 'move');
  console.log('    move: I0001 started from the UI and reads active');

  // work, then the real test of it, recorded as evidence at the commit that holds the work
  mkdirSync('src/I0001', { recursive: true });
  writeFileSync('src/I0001/answer.txt', '42\n');
  sh('git', ['add', 'src/I0001/answer.txt']);
  sh('git', ['commit', '-qm', 'the work of I0001: the answer']);
  const result = sh('sh', ['checks/the-answer-holds.sh']);
  if (result !== 'the answer holds') fail(`the check after the work answered "${result}"`);
  sh(mj, ['plan', 'evidence', 'I0001', '--covers', 'proof', '--type', 'test',
    '--command', 'sh checks/the-answer-holds.sh', '--result', result]);
  console.log(`    test: the check answered "${result}" and is recorded as I0001's evidence`);

  step = 'live';
  // live: the first tab reopens the workspace with its channel held closed, so the move
  // below ends before that tab's channel opens: the hardest moment for it to be shown
  let release;
  holdChannel = new Promise((r) => { release = r; });
  const reopened = a.waitForEvent('load');
  await a.click('a[href="/ui/idd?intent=the-answer-is-known"]');
  await reopened;
  const before = navigationsOfA();
  if (await a.locator('[data-live-state="live"]').count()) fail('the first tab\'s channel opened while it was held');
  // from here every other way the first tab reads its screen again is held: the test step's
  // commit, and every move after it, leave its screen catching up, and on a loaded runner that
  // catch-up never settles (run 39097403850), so the step does not wait for it; it holds it.
  // Whatever the screen reads meanwhile, only the channel can now show it the move
  holdRefresh = true;
  const b = await browser.newPage({ viewport: { width: 1200, height: 1000 } });
  watch(b, 'second tab');
  await b.goto(`${base}/ui/idd?intent=the-answer-is-known&issue=I0001#move`, { waitUntil: 'networkidle' });
  await move(b, 'verify');
  if (await status(a, 'I0001') === 'verify') fail('the first tab showed the move before its channel opened');
  holdRefresh = false;
  release();
  holdChannel = null;
  await a.waitForFunction(() => document.querySelector('[data-live-state]')?.dataset.liveState === 'live')
    .catch(() => fail('the first tab\'s channel never went live'));
  await until(a, 'I0001', 'verify', 'live');
  if (navigationsOfA() !== before) fail(`the first tab navigated ${navigationsOfA() - before} time(s) to show the move`);
  console.log(`    live: verified in a second tab before the first tab's channel opened (${timerRefreshes} timer refresh(es) held meanwhile); once it opened, the first tab caught up without navigating`);
  await b.close();

  step = 'done';
  // done, from the first tab; what waited for I0001 is now ready
  await a.click('#issue-I0001 a[href^="/ui/idd?"][href*="issue=I0001"]');
  await move(a, 'done');
  await until(a, 'I0001', 'done', 'done');
  await until(a, 'I0002', 'ready', 'dependency');
  console.log('    done: I0001 done from the UI, and I0002 is ready');

  step = 'explained';
  // explained: the standing panel, filled from its own route, explains the intent
  await a.waitForFunction(() => {
    const s = document.querySelector('section[aria-label="Why does it stand where it stands?"]');
    return s && !s.hasAttribute('data-panel-src') && s.querySelector('.mj-ui-source');
  }, null, { timeout: WAIT }).catch(() => fail('the standing panel was never filled'));
  const standing = await a.locator('section[aria-label="Why does it stand where it stands?"]').innerText();
  for (const want of ['the-answer-holds', 'intent_realization.explain']) {
    if (!standing.includes(want)) fail(`the standing panel does not explain ${want}`);
  }
  await noBareIds(a, 'explained');
  // the criterion's code span is drawn as code, and no backtick reaches the reader (I3004)
  if (!(await a.locator('main code.mj-ui-code', { hasText: 'src/I0001/answer.txt' }).count())) {
    fail('the criterion\'s code span is not drawn as code');
  }
  const raw = await a.evaluate(() => document.querySelector('main').innerText.match(/`[^`\s][^`]*`/g) || []);
  if (raw.length) fail(`raw markdown reaches the reader: ${raw.slice(0, 3).join(', ')}`);
  console.log('    explained: the standing panel names the criterion, its evidence and its source, every issue id a link, its code span drawn as code');
  await a.screenshot({ path: `${shots}/2-done.png`, fullPage: true });

  // the evidence, as the Cockpit's issue page shows it
  const c = await browser.newPage();
  watch(c, 'issue page');
  await c.goto(`${base}/cockpit/plan/issues/I0001`, { waitUntil: 'networkidle' });
  const issuePage = await c.locator('main').innerText();
  for (const want of ['sh checks/the-answer-holds.sh', 'proof']) {
    if (!issuePage.includes(want)) fail(`the issue page does not show ${want}`);
  }
  console.log('    evidence: the issue page shows the validation command and the evidence present');

  // reproduced: the recorded command gives the recorded result again
  const again = sh('sh', ['checks/the-answer-holds.sh']);
  if (again !== result) fail(`the recorded command now answers "${again}", not "${result}"`);
  console.log('    reproduced: the recorded command answers the recorded result again');

  if (graphLib === 'absent' && graphAsked) fail(`the graph library is not in this distribution and was asked for ${graphAsked} time(s)`);
  if (problems.length) fail('the pages reported: ' + problems.join(' | '));
} catch (error) {
  console.log(`    the walk stopped at "${step}": ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser.close();
}
JS
mkdir -p shots
graph=present
[ -f "$ROOT/share/ui/vendor/cytoscape.min.js" ] || { graph=absent; echo "    the vendored graph library is not built here, so the page must not ask for it"; }
rc=0
node walk.mjs "$U" "$ROOT/scripts/lib/browser.mjs" "$MJ" "$PWD/shots" "$graph" > walk.out 2>&1 || rc=$?
cat walk.out
case "$rc" in
  0) ;;
  3) skip "no browser could be launched, so the IDD loop was not walked" ;;
  *) echo "    the walk failed (exit $rc)"; exit 1 ;;
esac

# the record the UI moved holds what the walk did, stamped with the events behind them
grep -q '^completed_at: ' .ai/repo/project/issues/I0001.yaml || { echo "    I0001 carries no completion"; exit 1; }
grep -q 'command: "sh checks/the-answer-holds.sh"' .ai/repo/project/issues/I0001.yaml \
  || { echo "    I0001's evidence does not name the check"; exit 1; }
echo "    the intent was picked, its issue moved, tested, verified live and done, and explained"
