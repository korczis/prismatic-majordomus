// The finish contract demonstration: switching a line off marks it unmet and the verdict says how many lines
// fail and which; switching it back on restores it.
export default {
  id: 'contract',
  title: 'switching a contract line off marks it unmet and changes the verdict',
  width: 1280,
  claims: (el) => el.tag === 'input' && /^off = /.test(el.attrs['x-on:change'] || ''),
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      const state = () => c.locator.evaluate((i) => {
        const label = i.closest('label').querySelector('[x-text]');
        const scope = i.closest('[x-data]');
        const count = scope.querySelector('[x-text="off.length || \'n\'"]');
        return { label: label.textContent.trim(), unmet: count ? count.textContent.trim() : null };
      });
      await c.locator.scrollIntoViewIfNeeded();
      await c.locator.uncheck(); await page.waitForTimeout(40);
      const off = await state();
      if (off.label !== 'unmet') fail(`switching a line off leaves it saying "${off.label}"`);
      if (off.unmet !== '1') fail(`with one line off the verdict counts ${off.unmet} unmet`);
      await c.locator.check(); await page.waitForTimeout(40);
      const on = await state();
      if (on.label !== 'met') fail(`switching the line back on leaves it saying "${on.label}"`);
      n++;
    }
    return n;
  },
};
