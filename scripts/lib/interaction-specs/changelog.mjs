// The changelog: a change-kind chip hides its group of changes and shows it again; "Expand all" opens every
// version's changes and "Collapse all" closes them.
export default {
  id: 'changelog',
  title: 'a kind chip hides and shows its group; expand all opens every version',
  width: 1280,
  claims: (el) => el.tag === 'button' && (/^toggle\('/.test(el.attrs['@click'] || '') || el.attrs['@click'] === 'open = !open'),
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      await c.locator.scrollIntoViewIfNeeded();
      if (c.attrs['@click'] === 'open = !open') {
        const openCount = () => c.locator.evaluate((b) => [...b.closest('[x-data]').querySelectorAll('details')].map((d) => d.open));
        const before = await openCount();
        if (!before.length) { fail('an expand-all button has no versions to open'); n++; continue; }
        await c.locator.click(); await page.waitForTimeout(40);
        const after = await openCount();
        const wantOpen = !before.every(Boolean);
        if (!after.every((o) => o === wantOpen)) fail(`"${(await c.locator.innerText()).trim()}" did not ${wantOpen ? 'open' : 'close'} every version`);
        await c.locator.click(); await page.waitForTimeout(40);
      } else {
        const kind = c.attrs['@click'].match(/^toggle\('([^']+)'\)/)[1];
        const shown = () => c.locator.evaluate((b, k) => {
          const groups = [...b.closest('[x-data]').querySelectorAll('[x-show]')].filter((g) => (g.getAttribute('x-show') || '').includes(`'${k}'`));
          return groups.map((g) => getComputedStyle(g).display !== 'none');
        }, kind);
        const before = await shown();
        if (!before.length) { fail(`the chip ${kind} controls no group`); n++; continue; }
        await c.locator.click(); await page.waitForTimeout(40);
        if ((await shown()).some(Boolean)) fail(`the chip ${kind} did not hide its group`);
        if (await c.locator.getAttribute('aria-pressed') !== 'false') fail(`the chip ${kind} does not say aria-pressed="false" while its group is hidden`);
        await c.locator.click(); await page.waitForTimeout(40);
        if (!(await shown()).every(Boolean)) fail(`pressing the chip ${kind} again did not show its group`);
      }
      n++;
    }
    return n;
  },
};
