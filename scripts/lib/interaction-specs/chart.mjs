// A chart (site/charts.js, ADR 0152, I4400): every <figure data-chart> on the page is drawn, and is read by
// pointing at it. Its frame takes focus and the arrow keys move along the categories with one tooltip that names
// every visible series there; a key held with ctrl or meta is left to the browser; pressing a legend button hides
// its series, says so with aria-pressed, and leaves every other series in its colour; a plain wheel over a chart
// scrolls the page; a line chart with more categories than fit is zoomable, and the Reset control gives the whole
// range back. The two controls are the templates the build wrote in site/templates/partials/charts.html (the
// legend's button and the Reset button, the elements this spec claims); the state is read through
// window.mjCharts, which the component publishes for exactly this.
export default {
  id: 'chart',
  title: 'a chart is drawn; its keys move one tooltip over every series; its legend buttons hide a series without repainting the rest; the wheel scrolls the page; Reset gives a zoomed chart its whole range back',
  width: 1280,
  // the templates' buttons; a copy under a chart carries data-chart-for and is driven here through its chart
  claims: (el) => el.tag === 'button' && ['reset', 'series'].includes(el.attrs['data-chart-action'])
    && !('data-chart-for' in el.attrs),
  async exercise({ controls, conditional, page, fail }) {
    const started = await page.waitForFunction(() => !!window.mjCharts, null, { timeout: 15000, polling: 100 }).then(() => true, () => false);
    if (!started) { fail('the page carries a chart and the component never started (no window.mjCharts after 15 s)'); return controls.length + conditional; }
    const ids = await page.evaluate(() => window.mjCharts.ids());
    const view = (id) => page.evaluate((i) => window.mjCharts.state(i), id);
    for (const id of ids) {
      const figure = page.locator(`[id="${id}"]`);
      await figure.scrollIntoViewIfNeeded();
      const drawn = await page.waitForFunction((i) => { const s = window.mjCharts.state(i); return s && s.rendered; }, id, { timeout: 15000, polling: 100 }).then(() => true, () => false);
      if (!drawn) {
        const error = await figure.getAttribute('data-chart-error');
        fail(`chart ${id} was not drawn${error ? `: ${error.slice(0, 120)}` : ''}`); continue;
      }
      const before = await view(id);
      if (!before.series.every((s) => /^#[0-9a-f]{6}$/.test(s.colour))) fail(`chart ${id} drew a series in no colour of the page: ${before.series.map((s) => s.colour).join(', ')}`);
      // the keyboard: the frame takes focus; Home and an arrow move the tooltip, which names every visible series
      const frame = page.locator(`[id="${id}"] [data-chart-frame]`);
      await frame.focus();
      if (!(await frame.evaluate((el) => document.activeElement === el))) { fail(`the frame of chart ${id} does not take focus`); continue; }
      await page.keyboard.press('Home');
      await page.keyboard.press('ArrowRight');
      const pointed = await view(id);
      const expected = Math.min(before.window[0] + 1, before.categories.length - 1);
      if (pointed.cursor !== expected) fail(`ArrowRight after Home left chart ${id} at category ${pointed.cursor}, not ${expected}`);
      for (const s of before.series) if (!pointed.tip.includes(s.name)) fail(`the tooltip of chart ${id} does not name series ${s.name}: "${pointed.tip.slice(0, 160)}"`);
      const shown = await page.locator(`[id="${id}"] .mj-chart-tip`).first().isVisible().catch(() => false);
      if (!shown) fail(`pointing at a category of chart ${id} from the keyboard opened no tooltip`);
      await page.keyboard.press('Escape');
      // a key held with ctrl or meta is the browser's (page zoom, Back), and the chart leaves it alone
      const kept = await frame.evaluate((el) => ['ctrlKey', 'metaKey', 'altKey'].map((m) => {
        const e = new KeyboardEvent('keydown', { key: m === 'altKey' ? 'ArrowLeft' : '-', [m]: true, bubbles: true, cancelable: true });
        el.dispatchEvent(e);
        return e.defaultPrevented ? m : '';
      }).filter(Boolean));
      if (kept.length) fail(`chart ${id} cancels the browser's own shortcut with ${kept.join(', ')}`);
      // the legend: its buttons are reachable and pressed; hiding one series leaves every other in its colour
      if (before.series.length > 1) {
        const keys = page.locator(`[data-chart-action="series"][data-chart-for="${id}"]`);
        const count = await keys.count();
        if (count !== before.series.length) fail(`chart ${id} has ${count} legend button(s) for ${before.series.length} series`);
      }
      if (before.series.length > 1 && (await page.locator(`[data-chart-action="series"][data-chart-for="${id}"]`).count())) {
        const key = page.locator(`[data-chart-action="series"][data-chart-for="${id}"]`).first();
        const hidden = before.series[0].name;
        await key.focus();
        await page.keyboard.press('Enter');
        const after = await view(id);
        if (after.series[0].visible) fail(`pressing the legend button of ${hidden} left it visible on chart ${id}`);
        if ((await key.getAttribute('aria-pressed')) !== 'false') fail(`the legend button of ${hidden} on chart ${id} does not say it is off`);
        after.series.forEach((s, k) => { if (s.colour !== before.series[k].colour) fail(`hiding ${hidden} repainted ${s.name} on chart ${id} (${before.series[k].colour} to ${s.colour})`); });
        await key.click();
        if (!(await view(id)).series[0].visible) fail(`pressing ${hidden} again did not bring it back on chart ${id}`);
        if ((await key.getAttribute('aria-pressed')) !== 'true') fail(`the legend button of ${hidden} on chart ${id} does not say it is on again`);
      }
      // the wheel: a plain turn over the chart scrolls the page, zoomable or not
      await frame.scrollIntoViewIfNeeded();
      const room = await page.evaluate(() => document.scrollingElement.scrollHeight - window.innerHeight - window.scrollY);
      if (room > 120) {
        const box = await frame.boundingBox();
        await page.mouse.move(box.x + box.width / 2, box.y + Math.min(box.height / 2, 120));
        const y0 = await page.evaluate(() => window.scrollY);
        await page.mouse.wheel(0, 100);
        const scrolled = await page.waitForFunction((y) => window.scrollY !== y, y0, { timeout: 2000, polling: 50 }).then(() => true, () => false);
        if (!scrolled) fail(`a wheel turned over chart ${id} did not scroll the page`);
        await frame.scrollIntoViewIfNeeded();
      }
      // the zoom: a crowded line chart narrows, and its Reset brings back the whole range
      if (!before.zoomable) continue;
      const reset = page.locator(`[data-chart-action="reset"][data-chart-for="${id}"]`);
      if (!(await reset.isVisible())) { fail(`zoomable chart ${id} shows no Reset control`); continue; }
      if (!(await reset.isDisabled())) fail(`Reset of chart ${id} is enabled while the whole range is shown`);
      const last = before.categories.length - 1;
      await page.evaluate(([i, a, b]) => window.mjCharts.zoom(i, a, b), [id, Math.max(0, last - 5), last]);
      const zoomed = await view(id);
      if (zoomed.whole) { fail(`zooming chart ${id} left it showing the whole range`); continue; }
      if (await reset.isDisabled()) { fail(`Reset of chart ${id} stays disabled once it is zoomed`); continue; }
      await reset.click();
      const back = await view(id);
      if (!back.whole || back.window[0] !== 0 || back.window[1] !== last) fail(`Reset left chart ${id} at categories ${back.window.join('-')}, not 0-${last}`);
    }
    return controls.length + conditional;
  },
};
