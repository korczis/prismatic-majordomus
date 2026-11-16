# majordomus-covers: none
# majordomus-skip-runs-in: ui
# A command to reproduce is a code block a reader can copy (I3002), held in a real browser over
# the real UI and a real server on a fixture repository of this case's own. The intent's one
# criterion is settled by a command written with single quotes, angle brackets and an ampersand,
# the characters a page most easily mangles:
#
#   block       the intent workspace draws the command as a code block, its text exactly the
#               command the record holds
#   keyboard    the block's copy button is offered, reached by Tab and pressed with Enter
#   copied      the clipboard then holds exactly the command, and the button and the status
#               beside it say "Copied"
#   withheld    the same page where no clipboard can be written, once without
#               navigator.clipboard and once outside a secure context, keeps the button hidden
#               after ui.js has run, so a control that cannot copy is never offered
#
# Needs a browser scripts/lib/browser.mjs can launch; like case 3220 it is run in the ui job
# (majordomus-skip-runs-in: ui), where a skip is a failure.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
if [ -n "${CI:-}" ] && [ "${MAJORDOMUS_RUNS_IN:-}" != ui ]; then
  skip "a copy is pressed in the ui job (--runs-in ui), not in this CI run"
fi
mj_runs node || skip "$(mj_runs_why node)"
[ -d "$ROOT/node_modules/playwright" ] \
  || skip "playwright is not installed (npm ci), so no copy was pressed"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_HTTP_HOST

# ---------------------------------------------------------------- the fixture
"$MJ" init >/dev/null
pj_init
pj_milestone loop 1
pj_issue I0001 loop
mkdir -p .ai/repo/project/intents
COMMAND="test -n 'a <b> & c' && echo held"
cat > .ai/repo/project/intents/the-command-is-copied.yaml <<'YAML'
id: the-command-is-copied
title: The command is copied
statement: "Whoever reads a criterion can run its command again without retyping it."
invariants:
  - The command copied is the command recorded
milestones:
  - loop
satisfaction:
  - id: it-is-run-again
    criterion: The command runs again as recorded
    evidence: command
    ref: "test -n 'a <b> & c' && echo held"
YAML
git add -A >/dev/null && git commit -qm "an intent settled by a command" >/dev/null

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1

# ---------------------------------------------------------------- the copy
cat > copy.mjs <<'JS'
const [base, browserLib, want] = process.argv.slice(2);
const { launch } = await import(browserLib);
const browser = await launch();
if (!browser) { console.log('NO-BROWSER'); process.exit(3); }
const problems = [];
const fail = (why) => { console.log('    ' + why); process.exitCode = 1; throw new Error(why); };
try {
  const context = await browser.newContext();
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: base });
  const page = await context.newPage();
  page.setDefaultTimeout(60000);
  page.on('pageerror', (e) => problems.push(`uncaught ${e.message}`));
  page.on('console', (m) => { if (m.type() === 'error') problems.push(`console ${m.text()}`); });
  await page.goto(`${base}/ui/idd?intent=the-command-is-copied`, { waitUntil: 'networkidle' });

  // block: the command is a code block, its text the record's
  const block = page.locator('.mj-ui-command').first();
  if (!(await block.count())) fail('no command is drawn as a code block');
  const shown = await block.locator('code').textContent();
  if (shown !== want) fail(`the block reads "${shown}", not "${want}"`);
  console.log('    block: the command is drawn as a code block, exactly as recorded');

  // keyboard: the copy button is offered and pressed from the keyboard
  const copy = block.locator('button[data-mj-copy]');
  await copy.waitFor({ state: 'visible' }).catch(() => fail('the copy button is never offered'));
  await page.evaluate(() => {
    const b = document.querySelector('.mj-ui-command button[data-mj-copy]');
    const all = [...document.querySelectorAll('a[href], button, input, select, textarea, [tabindex]')]
      .filter((e) => !e.disabled && e.tabIndex >= 0 && e.offsetParent !== null);
    // focus the control just before it, so a Tab is what reaches the button
    const at = all.indexOf(b);
    if (at > 0) all[at - 1].focus();
  });
  await page.keyboard.press('Tab');
  if (!(await copy.evaluate((b) => b === document.activeElement))) fail('Tab does not reach the copy button');
  await page.keyboard.press('Enter');
  console.log('    keyboard: the copy button is reached by Tab and pressed with Enter');

  // copied: the clipboard holds exactly the command, and the page says so
  await page.waitForFunction((b) => b.textContent === 'Copied', await copy.elementHandle(), { timeout: 10000 })
    .catch(async () => fail(`the button reads "${await copy.textContent()}" after the copy`));
  const said = await block.locator('[role="status"]').textContent();
  if (said !== 'Copied') fail(`the status says "${said}"`);
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  if (copied !== want) fail(`the clipboard holds "${copied}", not "${want}"`);
  console.log('    copied: the clipboard holds exactly the command, and the button and its status say Copied');

  // withheld: where no clipboard can be written the button stays hidden. Each init script runs
  // before ui.js; the page then proves its own setup took, and that ui.js ran (measureBar writes
  // --ui-bar just before the copy buttons are judged), so a hidden button is the guard's doing.
  const without = {
    'without navigator.clipboard': [() => { delete Navigator.prototype.clipboard; },
      () => navigator.clipboard === undefined && window.isSecureContext],
    'outside a secure context': [() => {
      Object.defineProperty(window, 'isSecureContext', { get: () => false, configurable: true });
    }, () => Boolean(navigator.clipboard) && window.isSecureContext === false],
  };
  for (const [where, [unwritable, holds]] of Object.entries(without)) {
    const bare = await browser.newContext();
    await bare.addInitScript(unwritable);
    const other = await bare.newPage();
    other.setDefaultTimeout(60000);
    other.on('pageerror', (e) => problems.push(`uncaught ${where}: ${e.message}`));
    await other.goto(`${base}/ui/idd?intent=the-command-is-copied`, { waitUntil: 'networkidle' });
    if (!(await other.evaluate(holds))) fail(`the page ${where} could not be set up`);
    const ran = await other.evaluate(() => document.documentElement.style.getPropertyValue('--ui-bar'));
    if (!ran) fail(`ui.js never ran on the page ${where}`);
    const button = other.locator('.mj-ui-command button[data-mj-copy]');
    if (!(await button.count())) fail(`the page ${where} carries no copy button at all`);
    if (!(await button.first().evaluate((b) => b.hidden)) || (await button.first().isVisible())) {
      fail(`the copy button is offered ${where}, where it cannot copy`);
    }
    await bare.close();
  }
  console.log('    withheld: without navigator.clipboard, and outside a secure context, the copy button stays hidden');
  if (problems.length) fail('the page reported: ' + problems.join(' | '));
} catch (error) {
  process.exitCode = 1;
} finally {
  await browser.close();
}
JS
rc=0
node copy.mjs "$U" "$ROOT/scripts/lib/browser.mjs" "$COMMAND" > copy.out 2>&1 || rc=$?
cat copy.out
case "$rc" in
  0) ;;
  3) skip "no browser could be launched, so no copy was pressed" ;;
  *) echo "    the copy failed (exit $rc)"; exit 1 ;;
esac
