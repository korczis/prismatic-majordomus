// Browser test of the outlook chapters of the built Czech edition. Build first: python3 handbook/build.py
// Every figure drawn is compared with the data the page embeds, and every control is driven:
// the scenario toggles, the keyboard on the projection, the proof checklist and its reset,
// the table toggles of the new charts. Exits 1 on the first group that does not hold.
const ROOT = require('path').resolve(__dirname, '../../..');
const PAGE = process.env.HANDBOOK_PAGE || require('path').join(ROOT, 'target/handbook/cs/prirucka-majordomusu.html');
const OUT = process.env.HANDBOOK_TEST_OUT || require('path').join(ROOT, 'target/handbook/cs/tests');
require('fs').mkdirSync(OUT, { recursive: true });
const { chromium } = require('playwright');
const fs = require('fs'), path = require('path');

let failures = 0;
function check(label, ok, detail) {
  console.log((ok ? 'ok   ' : 'FAIL ') + label + (detail === undefined ? '' : ' ' + JSON.stringify(detail)));
  if (!ok) failures++;
}

(async () => {
  fs.writeFileSync(path.join(OUT, 'pw-o.html'), fs.readFileSync(PAGE, 'utf8'));
  const url = 'file://' + path.join(OUT, 'pw-o.html');
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: 1400, height: 950 }, reducedMotion: 'reduce' });
  const p = await ctx.newPage();
  const errs = []; p.on('pageerror', (e) => errs.push(e.message));
  p.on('console', (m) => { if (m.type() === 'error') errs.push('console: ' + m.text().slice(0, 140)); });
  await p.goto(url); await p.waitForTimeout(1000);

  // the chapters are there, each with what it rests on, and no caption kept a placeholder
  const base = await p.evaluate(() => {
    const ids = ['v-vyhled', 'v-trh', 'v-realita', 'v-tokeny'];
    return {
      missing: ids.filter((id) => !document.getElementById(id)),
      noEvidence: ids.filter((id) => !document.getElementById('ev-' + id)),
      placeholders: (document.body.innerText.match(/%%[a-z]+%%/g) || []).slice(0, 5),
    };
  });
  check('four outlook chapters', base.missing.length === 0, base.missing);
  check('each rests on the repository', base.noEvidence.length === 0, base.noEvidence);
  check('no placeholder left', base.placeholders.length === 0, base.placeholders);

  // what is drawn is what the page embeds
  const drawn = await p.evaluate(() => {
    const D = JSON.parse(document.getElementById('data-outlook').textContent);
    const M = JSON.parse(document.getElementById('data-outlook-model').textContent);
    const rows = (id) => document.querySelectorAll('#' + id + ' tbody tr').length;
    const marks = (id) => document.querySelectorAll('#' + id + ' svg .mark').length;
    return {
      potential: [marks('chart-v-potential'), M.potential.length],
      position: [marks('chart-v-position'), M.position.length * 2],
      scores: [document.querySelectorAll('#v-scores .meter-row').length, M.scores.length],
      weeks: [marks('chart-v-weeks'), D.integration.weeks.length],
      types: [marks('chart-v-types'), D.integration.windowTypes.length],
      plan: [marks('chart-v-plan'), D.plan.status.length],
      intents: [rows('v-intents'), D.intents.length],
      telemetry: [rows('v-telemetry'), D.plan.telemetry.issues.length],
      chain: [rows('v-telemetry-chain'), D.plan.telemetry.issues.length],
      unmet: [document.querySelectorAll('#v-unmet li').length, D.economics.unmet.length],
      suites: [rows('v-suites'), D.economics.suites.length],
      statement: [document.getElementById('v-statement').textContent, D.economics.statement],
      gauge: [document.querySelectorAll('#v-gauge circle').length, 2],
      lines: [document.querySelectorAll('#scen-svg .vs-line').length, M.scenarios.length],
    };
  });
  for (const [k, v] of Object.entries(drawn)) check('drawn ' + k, v[0] === v[1], v);

  // the scenario toggles hide and show their curve, and the readout follows
  const toggles = p.locator('#scen-toggles button');
  check('one toggle per scenario', (await toggles.count()) === drawn.lines[1], await toggles.count());
  await p.locator('#scen-svg rect[tabindex]').focus();
  for (let i = 0; i < 10; i++) await p.keyboard.press('ArrowRight');
  const at30 = await p.evaluate(() => ({ text: document.getElementById('scen-readout').textContent, v: Math.round(window.mjOutlook.at('real', 30)) }));
  check('keyboard moves the projection to day 30', at30.text.indexOf('Den 30') === 0 && at30.text.indexOf('realistický ' + at30.v) > 0, at30.text);
  await toggles.nth(1).click();
  const off = await p.evaluate(() => ({
    pressed: document.querySelectorAll('#scen-toggles button')[1].getAttribute('aria-pressed'),
    hidden: getComputedStyle(document.querySelectorAll('#scen-svg .vs-g')[1]).display === 'none',
    text: document.getElementById('scen-readout').textContent,
  }));
  check('a toggle hides its scenario', off.pressed === 'false' && off.hidden && off.text.indexOf('realistický') < 0, off);
  await toggles.nth(1).click();
  check('and shows it again', (await p.evaluate(() => getComputedStyle(document.querySelectorAll('#scen-svg .vs-g')[1]).display)) !== 'none');

  // the proof checklist is remembered by this browser and forgotten on reset
  await p.locator('#v-proof-2').check();
  check('a checked proof is counted', (await p.locator('#v-proofs-count').textContent()).indexOf('1 z 6') === 0, await p.locator('#v-proofs-count').textContent());
  await p.reload(); await p.waitForTimeout(800);
  check('and remembered after a reload', await p.locator('#v-proof-2').isChecked());
  await p.locator('#v-proofs-reset').click();
  check('reset forgets it', !(await p.locator('#v-proof-2').isChecked()) && (await p.locator('#v-proofs-count').textContent()).indexOf('0 z 6') === 0);

  // every chart of the chapters can show its numbers as a table
  const tables = await p.evaluate(() => {
    const out = [];
    document.querySelectorAll('#v-vyhled .chart, #v-trh .chart, #v-realita .chart, #v-tokeny .chart').forEach((c) => {
      const btn = Array.from(c.querySelectorAll(':scope > button')).pop(), box = c.querySelector(':scope > .table-wrap');
      if (!btn || !box) { out.push(c.id + ': no toggle'); return; }
      btn.click();
      if (box.hidden || !box.querySelectorAll('tbody tr').length) out.push(c.id + ': table did not open');
      btn.click();
      if (!box.hidden) out.push(c.id + ': table did not close');
    });
    return out;
  });
  check('every chart toggles its table', tables.length === 0, tables);

  // a bar's tooltip says its value
  await p.locator('#chart-v-weeks svg .mark').first().hover();
  check('a week explains itself on hover', ((await p.locator('.tip').textContent()) || '').indexOf('merge') > 0, await p.locator('.tip').textContent());
  check('no page error', errs.length === 0, errs.slice(0, 4));
  await ctx.close();

  // phone width: nothing in the chapters pushes the page sideways
  const phone = await (await b.newContext({ viewport: { width: 390, height: 800 }, reducedMotion: 'reduce' })).newPage();
  await phone.goto(url); await phone.waitForTimeout(800);
  const wide = await phone.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
  check('no horizontal scroll at 390 px', !wide);
  await phone.locator('#v-realita').screenshot({ path: path.join(OUT, 'v-realita-phone.png') });
  await b.close();
  console.log(failures ? 'FAILURES: ' + failures : 'OUTLOOK OK');
  process.exit(failures ? 1 : 0);
})();
