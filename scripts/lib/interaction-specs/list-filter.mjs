// A filtered list (skills, use cases, doctrines, the Why catalogue): a search box, choice buttons and a reset
// in one Alpine component, over items that x-show hides. A query nothing matches hides every item and an
// empty one restores them; a choice selects itself and leaves what it names visible; reset restores all,
// including the "clear the filters" button the component renders only when nothing matches.
const CHOICE = /^\s*[a-z]+ = (\(|'|")/;
const isItem = (el) => el.matches('li[x-show], [data-why-id]');
export default {
  id: 'list-filter',
  title: 'a search, a choice and a reset filter the list they belong to',
  width: 1280,
  order: 8,                  // it reloads the page or leaves state behind, so it runs after the specs that do not
  claims: (el) => (el.tag === 'input' && el.attrs['x-model'] === 'q')
    || (el.tag === 'button' && el.attrs.role !== 'tab' && (CHOICE.test(el.attrs['x-on:click'] || '') || el.attrs['x-on:click'] === 'reset()')),
  async exercise({ controls, page, fail, conditional }) {
    const count = (n) => page.evaluate(({ n, itemSel }) => {
      const scope = document.querySelector(`[data-mj-control="${n}"]`).closest('[x-data]');
      const items = [...scope.querySelectorAll(itemSel)];
      const shown = items.filter((i) => getComputedStyle(i).display !== 'none');
      return { total: items.length, shown: shown.length };
    }, { n, itemSel: 'li[x-show], [data-why-id]' });
    let exercised = 0, renderedClears = 0;
    for (const c of controls) {
      await page.reload({ waitUntil: 'load' });
      await page.waitForTimeout(80);
      const loc = page.locator(`[data-mj-control="${c.n}"]`);
      // a reload drops the marks; put them back the same way the runner did
      if (await loc.count() === 0) {
        const { isControl, markControls } = await import('../interactions.mjs');
        await page.evaluate(markControls, isControl.toString());
      }
      const base = await count(c.n);
      if (base.total === 0) { fail(`${c.tag} ${JSON.stringify(c.attrs['x-on:click'] || c.attrs['x-model'])} filters no list item`); exercised++; continue; }
      if (base.shown !== base.total) fail(`${base.total - base.shown} of ${base.total} items are hidden before any filter is touched`);
      if (c.tag === 'input') {
        await loc.scrollIntoViewIfNeeded(); await loc.fill('zz-no-such-thing-zz'); await page.waitForTimeout(80);
        if ((await count(c.n)).shown !== 0) fail('a query nothing matches leaves items visible');
        await loc.fill(''); await page.waitForTimeout(80);
        if ((await count(c.n)).shown !== base.total) fail('clearing the query does not restore every item');
      } else if (c.attrs['x-on:click'] === 'reset()') {
        // make reset have something to undo: a query nothing matches, typed into the component's own search
        const search = page.locator('[data-mj-control]').filter({ has: page.locator('xpath=self::input') });
        const input = await page.evaluate((n) => {
          const scope = document.querySelector(`[data-mj-control="${n}"]`).closest('[x-data]');
          const i = scope.querySelector('input[x-model="q"]'); return i ? i.getAttribute('data-mj-control') : null;
        }, c.n);
        if (!input) { fail('a reset button belongs to a component with no search to reset'); exercised++; continue; }
        await page.locator(`[data-mj-control="${input}"]`).fill('zz-no-such-thing-zz'); await page.waitForTimeout(80);
        // the conditional "clear the filters" button exists only now; it resets too
        const clear = page.locator('button', { hasText: 'Clear the filters' });
        if (conditional && renderedClears < conditional && await clear.count()) {
          await clear.first().click(); await page.waitForTimeout(80);
          if ((await count(c.n)).shown !== base.total) fail('the "Clear the filters" button does not restore every item');
          renderedClears++;
          await page.locator(`[data-mj-control="${input}"]`).fill('zz-no-such-thing-zz'); await page.waitForTimeout(80);
        }
        await loc.scrollIntoViewIfNeeded();
        if (!(await loc.isVisible())) fail('the reset button is not shown while a filter is active');
        else { await loc.click(); await page.waitForTimeout(80); }
        const after = await count(c.n);
        if (after.shown !== base.total) fail(`reset leaves ${after.shown} of ${base.total} items shown`);
        if (await page.locator(`[data-mj-control="${input}"]`).inputValue() !== '') fail('reset does not empty the search');
      } else {
        await loc.scrollIntoViewIfNeeded(); await loc.click(); await page.waitForTimeout(80);
        const pressed = await loc.getAttribute('aria-pressed');
        if (pressed !== null && pressed !== 'true') fail(`pressing the choice "${(await loc.innerText()).trim()}" does not mark it pressed`);
        const after = await count(c.n);
        if (after.shown === 0) fail(`the choice "${(await loc.innerText()).trim()}" hides every item, though it is a value some item declares`);
        if (after.shown > base.total) fail('a choice shows more items than the list has');
      }
      exercised++;
    }
    return exercised + renderedClears;
  },
};
