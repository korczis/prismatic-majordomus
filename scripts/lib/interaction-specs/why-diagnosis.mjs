// The Why diagnosis: ticking a signal matches the moment that declares it, reset clears every tick, and
// "copy a link to this" puts a link carrying the ticked signals on the clipboard.
export default {
  id: 'why-diagnosis',
  title: 'a ticked signal matches its moment; reset clears; the share link carries the picks',
  width: 1280,
  order: 7,                  // it reloads the page or leaves state behind, so it runs after the specs that do not
  claims: (el) => (el.tag === 'input' && el.attrs['x-model'] === 'picked')
    || (el.tag === 'button' && el.attrs['x-on:click'] === 'picked = []')
    || (el.tag === 'a' && el.attrs['x-on:click'] === 'share()'),
  async exercise({ controls, page, fail }) {
    // the counter lives in <template x-if="picked.length > 0">: with nothing ticked it is not rendered at all, and the
    // empty state is. So no counter is zero matches only when the empty state stands in its place.
    const matched = () => page.evaluate(() => {
      const scope = document.getElementById('diagnose');
      const n = scope && scope.querySelector('[x-text="matched.length"]');
      if (n) return Number(n.textContent.trim() || 0);
      const empty = scope && [...scope.querySelectorAll('template')].some((t) => t.getAttribute('x-if') === 'picked.length === 0' && t.nextElementSibling);
      return empty ? 0 : NaN;
    });
    await page.evaluate(() => { try { localStorage.removeItem('why.picked'); } catch (e) {} });
    let n = 0;
    const boxes = controls.filter((c) => c.tag === 'input');
    for (const c of boxes) {
      await c.locator.evaluate((el) => { const d = el.closest('details'); if (d) d.open = true; el.scrollIntoView({ block: 'center' }); });
      await c.locator.check(); await page.waitForTimeout(30);
      if (!(await c.locator.isChecked())) fail(`the signal ${c.attrs.value} did not tick`);
      if (!((await matched()) >= 1)) fail(`ticking the signal ${c.attrs.value} matched no moment`);
      await c.locator.uncheck(); await page.waitForTimeout(30);
      n++;
    }
    // the reset and the share link are rendered only once something is ticked (they sit in a <template>), so they
    // are found after a tick rather than from the marks the page carried when it loaded
    const later = [];
    if (boxes.length) {
      await boxes[0].locator.check(); await page.waitForTimeout(60);
      const found = page.locator('#diagnose button, #diagnose a').filter({ hasText: /^(reset|copy a link to this)$/ });
      for (let k = 0; k < await found.count(); k++) {
        const loc = found.nth(k);
        const tag = await loc.evaluate((el) => el.tagName.toLowerCase());
        later.push({ tag, locator: loc });
      }
      await boxes[0].locator.uncheck();
    }
    for (const c of [...controls.filter((x) => x.tag !== 'input'), ...later]) {
      if (boxes.length) { await boxes[0].locator.check(); await page.waitForTimeout(30); }
      if (c.tag === 'a') {
        await c.locator.scrollIntoViewIfNeeded(); await c.locator.click(); await page.waitForTimeout(80);
        const clip = await page.evaluate(() => window.__mjClipboard ?? '');
        const want = boxes.length ? boxes[0].attrs.value : '';
        if (!clip.includes('#diagnose?signals=') || (want && !clip.includes(want))) fail(`the share link is ${JSON.stringify(clip.slice(0, 80))}, which does not carry the ticked signal`);
      } else {
        await c.locator.scrollIntoViewIfNeeded(); await c.locator.click(); await page.waitForTimeout(50);
        if ((await matched()) !== 0) fail('reset leaves a moment matched');
        if (boxes.length && await boxes[0].locator.isChecked()) fail('reset leaves a signal ticked');
      }
      n++;
    }
    return n;
  },
};
