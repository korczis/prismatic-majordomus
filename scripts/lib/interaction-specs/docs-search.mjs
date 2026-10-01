// The documentation hub's search (site/templates/partials/docs-hub.html): a search box over
// /docs/index.json. A query some page matches lists results, each a link to a page this build
// published; a query nothing matches lists none and says so; clearing the box empties the list.
export default {
  id: 'docs-search',
  title: 'the documentation search lists pages that exist, and nothing for a query nothing matches',
  width: 1280,
  order: 7,
  claims: (el) => el.tag === 'input' && el.attrs.id === 'docs-search-input',
  async exercise({ controls, page, fail }) {
    let exercised = 0;
    for (const c of controls) {
      const loc = page.locator(`[data-mj-control="${c.n}"]`);
      const results = page.locator('[data-docs-search-results] a');
      await loc.scrollIntoViewIfNeeded();
      await loc.fill('decision');
      await page.waitForFunction(() => document.querySelectorAll('[data-docs-search-results] a').length > 0,
        null, { timeout: 5000 }).catch(() => {});
      const n = await results.count();
      if (n === 0) fail('a query many pages match ("decision") lists no result');
      else {
        const href = await results.first().getAttribute('href');
        const status = await page.evaluate((h) => fetch(h).then((r) => r.status), href);
        if (status !== 200) fail(`the first result ${href} answers ${status}, not 200`);
      }
      await loc.fill('zz-no-such-thing-zz');
      await page.waitForTimeout(120);
      if (await results.count() !== 0) fail('a query nothing matches still lists results');
      const said = await page.locator('[data-docs-search-status]').innerText();
      if (!/^0 page/.test(said.trim())) fail(`a query nothing matches does not say so: "${said.trim()}"`);
      await loc.fill('');
      await page.waitForTimeout(120);
      if (await results.count() !== 0) fail('clearing the search leaves results listed');
      exercised++;
    }
    return exercised;
  },
};
