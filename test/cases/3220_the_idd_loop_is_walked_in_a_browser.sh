# majordomus-covers: none
# majordomus-skip-runs-in: ui
# The IDD loop, walked end to end in a real browser over the real UI and a real server on one
# fixture repository (I2812, the owner's acceptance for v1.0.0-alpha.1). Nothing here calls a
# capability the UI would not; every move is a click, and the page is never reloaded by the
# test:
#
#   pick        the intent is chosen in /ui/idd from the list of intents
#   read        its issues with their status, and the dependency: I0002 waits for I0001
#   move        I0001 is started from its move form; the row reads active
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
#               validation command and the evidence present
#   reproduced  the recorded command, run again, gives the recorded result
#   clean       no console error, no uncaught exception
#
# Needs a browser scripts/lib/browser.mjs can launch. The suite runners have none, so the case
# defers to the ui job (majordomus-skip-runs-in: ui), which has one and runs it with
# `--no-skips --runs-in ui`, where a skip is a failure.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
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
    criterion: The answer the work owes is in its scope
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
// no copy, its one URL answering 404 is that absence, and every other failed request still fails.
const absent = graphLib === 'absent' ? '/ui/assets/vendor/cytoscape.min.js' : null;
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
    if (r.status() >= 400 && path !== absent) problems.push(`${name}: ${r.status()} ${path}`);
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

  // read: the issues and the dependency, as the plan derives them
  if (await status(a, 'I0001') !== 'ready') fail(`I0001 reads ${await status(a, 'I0001')}, not ready`);
  // I0002's row names what it waits for, wherever the table puts it
  const waits = (await a.locator('#issue-I0002').innerText()).replace(/\s+/g, ' ').trim();
  if (!waits.includes('I0001')) fail(`I0002 does not say it waits for I0001: "${waits}"`);
  console.log(`    read: I0001 ${await status(a, 'I0001')}, I0002 ${await status(a, 'I0002')} waiting for I0001`);
  await a.screenshot({ path: `${shots}/1-picked.png`, fullPage: true });

  step = 'move';
  // move: start I0001 from its move form
  await a.click('#issue-I0001 a[href^="/ui/idd?"][href*="issue=I0001"]');
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
  // the test step's commit moved the repository, so the reopened screen may still be reading it
  // again; it is read whole first (its catch-up still allowed), and only then is every other
  // way of reading it again held, or that catch-up would carry the move instead of the channel
  await a.waitForFunction(() => !document.querySelector('#main [data-catching-up]'), null, { timeout: WAIT })
    .catch(() => fail('the reopened first tab never finished reading the repository again'));
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
    const s = document.querySelector('section[aria-label="Why it stands where it stands"]');
    return s && !s.hasAttribute('data-panel-src') && s.querySelector('.mj-ui-source');
  }, null, { timeout: WAIT }).catch(() => fail('the standing panel was never filled'));
  const standing = await a.locator('section[aria-label="Why it stands where it stands"]').innerText();
  for (const want of ['the-answer-holds', 'intent_realization.explain']) {
    if (!standing.includes(want)) fail(`the standing panel does not explain ${want}`);
  }
  console.log('    explained: the standing panel names the criterion, its evidence and its source');
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
[ -f "$ROOT/share/ui/vendor/cytoscape.min.js" ] || { graph=absent; echo "    the vendored graph library is not built here, so its 404 is not counted"; }
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
