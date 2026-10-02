// The kit's search field (site/templates/kit/docs.html `search`): it submits its query to the
// documentation hub as ?q=, and the hub arrives with the query in its box and pages listed, each a
// page this build published.
export default {
  id: 'kit-search',
  title: 'a kit search field takes its query to the documentation hub, which lists what matches',
  width: 1280,
  order: 8,
  claims: (el) => el.tag === 'input' && 'data-kit-search' in el.attrs,
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      const from = page.url();
      await c.locator.scrollIntoViewIfNeeded();
      await c.locator.fill('decision');
      await Promise.all([page.waitForLoadState('load'), c.locator.press('Enter')]);
      await page.waitForFunction(() => document.querySelectorAll('[data-docs-search-results] a').length > 0,
        null, { timeout: 5000 }).catch(() => {});
      const box = await page.locator('#docs-search-input').inputValue().catch(() => null);
      if (box !== 'decision') fail(`the hub's search box holds ${JSON.stringify(box)}, not the query`);
      const results = page.locator('[data-docs-search-results] a');
      if (await results.count() === 0) fail('the hub lists nothing for a query many pages match ("decision")');
      else {
        const href = await results.first().getAttribute('href');
        const status = await page.evaluate((h) => fetch(h).then((r) => r.status), href);
        if (status !== 200) fail(`the first result ${href} answers ${status}, not 200`);
      }
      await page.goto(from, { waitUntil: 'load' });
      n++;
    }
    return n;
  },
};
