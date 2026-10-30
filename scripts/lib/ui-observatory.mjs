// ui-observatory — the Observatory's evidence, taken in a browser (I2680, ADR 0158).
//
// It visits one origin's Observatory (a running server, or the published site) and writes
// what it measured, never what it expected: the revision the page names, a screenshot of the
// kiosk at 1920x1080 and at 3840x2160, the frames the display painted in a stated window, that
// reduced motion stops the decoration, the identity of the newest event in the display's own
// model, and — when asked — a soak with the page's memory read at the start and at the end.
// The environment (machine, operating system, browser) is read here and written beside every
// figure, because a frame rate without its hardware is not a fact.
//
//   node scripts/lib/ui-observatory.mjs --origin URL --out DIR [--soak-minutes N] [--drills no]
//
// Exit: 0 when every check it ran passed, 10 when one failed, 12 when the page could not be
// measured at all. What it did not run it says it did not run.
import { chromium } from 'playwright';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const args = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && i + 1 < args.length ? args[i + 1] : fallback;
};
const origin = (opt('origin', '') || '').replace(/\/$/, '');
const out = opt('out', 'target/web/observatory-evidence');
const soakMinutes = Number(opt('soak-minutes', '0'));
if (!origin) {
  console.error('ui-observatory: --origin URL is required');
  process.exit(2);
}
mkdirSync(out, { recursive: true });

const result = {
  schema: 'observatory-evidence/v1',
  origin,
  measured_at: new Date().toISOString().replace(/\.\d+Z$/, 'Z'),
  environment: {
    machine: `${os.cpus()[0]?.model || 'unknown cpu'} x${os.cpus().length}, ${Math.round(os.totalmem() / 2 ** 30)} GiB`,
    os: `${os.type()} ${os.release()} ${os.arch()}`,
    browser: null,
    headless: true,
    precise_memory: true,
  },
  revision: null,
  status_word: null,
  checks: {},
};
let failed = false;
const check = (name, ok, detail) => {
  result.checks[name] = { verdict: ok ? 'PASS' : 'FAIL', ...detail };
  if (!ok) failed = true;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}  ${JSON.stringify(detail).slice(0, 220)}`);
};

// without this flag Chromium rounds performance.memory to coarse buckets, and a soak would
// compare two buckets rather than two readings
const browser = await chromium.launch({ args: ['--enable-precise-memory-info'] });
result.environment.browser = `chromium ${browser.version()}`;
const kiosk = `${origin}/ui/kiosk/`;

async function open(viewport, extra = {}) {
  const page = await browser.newPage({ viewport, ...extra });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(kiosk, { waitUntil: 'load' });
  await page.waitForSelector('.mj-obs--drawn', { timeout: 30000 });
  return { page, errors };
}

const words = (page) =>
  page.evaluate(() => ({
    status: document.querySelector('.mj-obs-status')?.textContent || null,
    scene: document.querySelector('.mj-obs-scene')?.textContent || null,
    fact: document.querySelector('.mj-obs-fact')?.textContent || null,
  }));

try {
  // ---------------------------------------------------------------- screenshots
  for (const [name, w, h] of [['1080p', 1920, 1080], ['4k', 3840, 2160]]) {
    const { page, errors } = await open({ width: w, height: h });
    await page.waitForTimeout(3000);
    const file = path.join(out, `kiosk-${name}.png`);
    await page.screenshot({ path: file });
    const seen = await words(page);
    const data = await page.evaluate(() => JSON.parse(document.querySelector('[data-observatory]').dataset.observatory));
    result.revision = data.revision || result.revision;
    result.status_word = seen.status;
    check(`screenshot-${name}`, errors.length === 0, { file, viewport: `${w}x${h}`, ...seen, script_errors: errors });
    await page.close();
  }

  // ---------------------------------------------------------------- accessibility
  // The same engine and rule set as the repository's UI audit (scripts/lib/ui-audit.mjs),
  // evaluated by the debugging protocol so that the page's own policy is not relaxed for it,
  // on the screen and on the kiosk, with the controls revealed as a pointer reveals them.
  {
    const axe = readFileSync(new URL('../../node_modules/axe-core/axe.min.js', import.meta.url), 'utf8');
    for (const [name, route] of [['screen', '/ui/observatory/'], ['kiosk', '/ui/kiosk/']]) {
      const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
      await page.goto(`${origin}${route}`, { waitUntil: 'load' });
      await page.waitForSelector('.mj-obs--drawn', { timeout: 30000 });
      await page.hover('.mj-obs');
      // the controls fade in on hover; contrast is judged once they have
      await page.waitForTimeout(1000);
      await page.evaluate(axe);
      const violations = await page.evaluate(async () => {
        // eslint-disable-next-line no-undef
        const r = await axe.run(document, { resultTypes: ['violations'], runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] } });
        return r.violations.map((v) => ({ id: v.id, impact: v.impact, nodes: v.nodes.length }));
      });
      check(`accessibility-${name}`, violations.length === 0, { route, rules: 'wcag2a wcag2aa wcag21a wcag21aa wcag22aa', violations });
      await page.close();
    }
  }

  // ---------------------------------------------------------------- frames painted
  {
    const { page, errors } = await open({ width: 1920, height: 1080 });
    const window_ms = 10000;
    const frames = await page.evaluate(
      (ms) =>
        new Promise((resolve) => {
          const canvas = document.querySelector('.mj-obs-canvas');
          const ctx = canvas.getContext('2d');
          let painted = 0;
          const clear = ctx.clearRect.bind(ctx);
          // the display clears its canvas once per frame it paints: counting the clears counts
          // the frames, without timing the browser's own compositor instead of the display
          ctx.clearRect = (...a) => { painted += 1; return clear(...a); };
          const t0 = performance.now();
          setTimeout(() => resolve({ painted, seconds: (performance.now() - t0) / 1000 }), ms);
        }),
      window_ms,
    );
    const fps = frames.painted / frames.seconds;
    check('frames-1080p', errors.length === 0 && fps > 0, {
      frames_per_second: Math.round(fps * 10) / 10,
      window_seconds: Math.round(frames.seconds * 10) / 10,
      note: 'headless chromium paints without a display; this is the display loop\'s rate on the machine named, not a panel\'s',
    });
    await page.close();
  }

  // ---------------------------------------------------------------- reduced motion
  {
    const { page } = await open({ width: 1280, height: 720 }, { reducedMotion: 'reduce' });
    await page.waitForTimeout(2500);
    const a = await page.locator('.mj-obs-canvas').screenshot();
    await page.waitForTimeout(400);
    const b = await page.locator('.mj-obs-canvas').screenshot();
    const still = Buffer.compare(a, b) === 0;
    await page.close();
    const moving = await open({ width: 1280, height: 720 });
    await moving.page.waitForTimeout(2500);
    const c = await moving.page.locator('.mj-obs-canvas').screenshot();
    await moving.page.waitForTimeout(400);
    const d = await moving.page.locator('.mj-obs-canvas').screenshot();
    const moves = Buffer.compare(c, d) !== 0;
    await moving.page.close();
    check('reduced-motion', still && moves, {
      reduced_frame_unchanged_over_400ms: still,
      default_frame_changed_over_400ms: moves,
    });
  }

  // ---------------------------------------------------------------- the newest event in the display's model
  {
    const { page } = await open({ width: 1280, height: 720 });
    await page.waitForTimeout(8000);
    const river = await page.evaluate(() => {
      const holder = document.querySelector('[data-observatory]');
      const live = holder.observatory && holder.observatory.data ? holder.observatory.data() : null;
      const data = live || JSON.parse(holder.dataset.observatory);
      const scene = data.scenes.find((s) => s.id === 'activity');
      return scene && scene.available ? { replay: Boolean(scene.replay), newest: scene.model.events[0] || null, held: scene.model.events.length } : null;
    });
    let api = null;
    try {
      const r = await fetch(`${origin}/api/v1/observatory/events`);
      api = r.ok ? await r.json() : null;
    } catch (e) {
      api = null;
    }
    if (!river) {
      result.checks['event-reaches-the-display'] = { verdict: 'NOT RUN', reason: 'the activity scene is not available at this origin' };
    } else if (!api) {
      result.checks['event-reaches-the-display'] = {
        verdict: 'NOT RUN',
        reason: 'this origin serves no API (a published snapshot); the display holds a recording',
        replay: river.replay,
        newest_in_display: river.newest ? { kind: river.newest.kind, lamport: river.newest.lamport, at: river.newest.at } : null,
      };
    } else {
      const newestApi = api.events[0] || null;
      const held = newestApi && river.newest && newestApi.id === river.newest.id;
      // the evidence is published: an event is recorded by its kind and clocks, and whether
      // the two identities are one, never by the identity itself (it names a machine)
      const shape = (e) => (e ? { kind: e.kind, lamport: e.lamport, at: e.at } : null);
      check('event-reaches-the-display', Boolean(held), {
        newest_in_journal: shape(newestApi),
        newest_in_display: shape(river.newest),
        same_identity: Boolean(held),
        events_held: river.held,
      });
    }
    await page.close();
  }

  // ---------------------------------------------------------------- recovery drills
  // A scene whose renderer throws is quarantined and the rotation goes on; a server that
  // stops answering makes the display say STALE and, answering again, LIVE; an expired
  // cursor makes it read its page again and say RESYNCED. Each is driven in the browser.
  if (opt('drills', 'yes') === 'yes') {
    const { page } = await open({ width: 1280, height: 720 });
    await page.waitForTimeout(2000);
    const inspectable = await page.evaluate(() => Boolean(document.querySelector('[data-observatory]').observatory));
    if (!inspectable) {
      for (const name of ['drill-scene-failure', 'drill-lost-server', 'drill-expired-cursor']) {
        result.checks[name] = { verdict: 'NOT RUN', reason: 'the display at this origin exposes no inspector (observatory.data()); it predates it' };
      }
      await page.close();
    } else {
    // 1. a renderer that throws: the current scene is given a model its renderer cannot draw
    const broken = await page.evaluate(async () => {
      const holder = document.querySelector('[data-observatory]');
      const api = holder.observatory;
      const data = api.data();
      const id = holder.dataset.scene;
      const scene = data.scenes.find((s) => s.id === id);
      scene.model = { machines: 5, events: 5, milestones: 5, checks: 5, releases: 5, sessions: 5, sources: 5 };
      await new Promise((r) => setTimeout(r, 1500));
      return { id, quarantined: api.quarantined(), now: holder.dataset.scene };
    });
    const isolated = Boolean(broken.quarantined[broken.id]) && broken.now !== broken.id;
    check('drill-scene-failure', isolated, broken);

    const live = await page.evaluate(() => Boolean(document.querySelector('[data-observatory]').observatory)) &&
      !(await page.evaluate(() => 'snapshot' in document.body.dataset));
    if (!live) {
      result.checks['drill-lost-server'] = { verdict: 'NOT RUN', reason: 'a published snapshot reads nothing again; there is no server to lose' };
      result.checks['drill-expired-cursor'] = { verdict: 'NOT RUN', reason: 'a published snapshot follows no cursor' };
    } else {
      // 2. the server stops answering for longer than the live bound, then answers again
      await page.route('**/*', (route) => (route.request().url().includes('/ui/') || route.request().url().includes('/api/') ? route.abort() : route.continue()));
      await page.waitForTimeout(95000);
      const stale = (await words(page)).status;
      await page.unroute('**/*');
      let back = null;
      const until = Date.now() + 150000;
      while (Date.now() < until) {
        await page.waitForTimeout(5000);
        back = (await words(page)).status;
        if (back && back.startsWith('LIVE')) break;
      }
      check('drill-lost-server', Boolean(stale && stale.startsWith('STALE') && back && back.startsWith('LIVE')), { while_lost: stale, after: back });
      // 3. the journal answers that the cursor expired: the page is read again, and said so
      await page.route('**/api/v1/observatory/events**', (route) =>
        route.fulfill({ contentType: 'application/json', body: JSON.stringify({ active: true, after: 1, cursor: 1, expired: true, events: [] }) }));
      let resynced = null;
      const deadline = Date.now() + 30000;
      while (Date.now() < deadline) {
        await page.waitForTimeout(1000);
        const w = (await words(page)).status;
        if (w && w.startsWith('RESYNCED')) { resynced = w; break; }
      }
      await page.unroute('**/api/v1/observatory/events**');
      check('drill-expired-cursor', Boolean(resynced), { status: resynced });

      // 4. new events interrupt the rotation for the river after the minimum dwell, and not
      //    again within the cooldown. The events are the test's own, delivered by intercepting
      //    the stream in this browser only: synthetic data in a test, never on a display.
      await page.reload({ waitUntil: 'load' });
      await page.waitForSelector('.mj-obs--drawn', { timeout: 30000 });
      const policy = await page.evaluate(() => document.querySelector('[data-observatory]').observatory.policy);
      let n = 0;
      const inject = () => page.route('**/api/v1/observatory/events**', (route) => {
        n += 1;
        const at = Math.floor(Date.now() / 1000);
        route.fulfill({ contentType: 'application/json', body: JSON.stringify({
          active: true, after: 1, cursor: 1, expired: false,
          events: [{ id: `drill:${n}`, kind: 'drill', node: 'drill', lamport: 1, at }],
        }) });
      });
      const scene = () => page.evaluate(() => document.querySelector('[data-observatory]').dataset.scene);
      const before = await scene();
      await inject();
      let after = before;
      const interruptBy = Date.now() + (policy.min_dwell_seconds + 15) * 1000;
      while (Date.now() < interruptBy && after !== 'activity') { await page.waitForTimeout(1000); after = await scene(); }
      await page.unroute('**/api/v1/observatory/events**');
      const reasons = await page.evaluate(() => document.querySelector('[data-observatory]').observatory.trace().map((t) => t.reason));
      check('drill-interrupt', before !== 'activity' && after === 'activity' && reasons.some((r) => r.startsWith('interrupt')),
        { policy, before, after, last_reason: reasons[reasons.length - 1] });
      // leave the river by a reader's step, give the rotation back, and deliver new events again
      await page.keyboard.press('ArrowRight');
      await page.keyboard.press(' ');
      await page.keyboard.press(' ');
      const left = await scene();
      await inject();
      await page.waitForTimeout((policy.min_dwell_seconds + 6) * 1000);
      const still = await scene();
      await page.unroute('**/api/v1/observatory/events**');
      const interrupts = (await page.evaluate(() => document.querySelector('[data-observatory]').observatory.trace()))
        .filter((t) => String(t.reason).startsWith('interrupt')).length;
      check('drill-interrupt-cooldown', left !== 'activity' && still !== 'activity' && interrupts === 1,
        { left_for: left, after_new_events_within_cooldown: still, interrupts });
    }
    await page.close();
    }
  }

  // ---------------------------------------------------------------- soak
  if (soakMinutes > 0) {
    const { page, errors } = await open({ width: 1920, height: 1080 });
    const read = () =>
      page.evaluate(() => ({
        heap_bytes: performance.memory ? performance.memory.usedJSHeapSize : null,
        dom_nodes: document.getElementsByTagName('*').length,
      }));
    await page.waitForTimeout(5000);
    const start = await read();
    const samples = [start];
    const end_at = Date.now() + soakMinutes * 60000;
    while (Date.now() < end_at) {
      await page.waitForTimeout(Math.min(60000, end_at - Date.now()));
      samples.push(await read());
    }
    const end = samples[samples.length - 1];
    const grew = start.heap_bytes && end.heap_bytes ? end.heap_bytes / start.heap_bytes : null;
    check('soak', errors.length === 0 && (grew === null || grew < 2) && end.dom_nodes <= start.dom_nodes + 50, {
      minutes: soakMinutes,
      start,
      end,
      heap_growth_ratio: grew && Math.round(grew * 100) / 100,
      samples: samples.length,
      script_errors: errors,
    });
    await page.close();
  } else {
    result.checks.soak = { verdict: 'NOT RUN', reason: 'no --soak-minutes was given' };
  }
} catch (e) {
  result.error = String(e && e.message ? e.message : e);
  console.error(`ui-observatory: ${result.error}`);
  writeFileSync(path.join(out, 'evidence.json'), `${JSON.stringify(result, null, 2)}\n`);
  await browser.close();
  process.exit(12);
}

await browser.close();
writeFileSync(path.join(out, 'evidence.json'), `${JSON.stringify(result, null, 2)}\n`);
console.log(`ui-observatory: ${failed ? 'a check failed' : 'every check it ran passed'}; ${path.join(out, 'evidence.json')}`);
process.exit(failed ? 10 : 0);
