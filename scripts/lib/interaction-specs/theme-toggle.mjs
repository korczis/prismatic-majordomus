// The theme toggle flips the page between light and dark, and the choice survives a reload.
export default {
  id: 'theme-toggle',
  title: 'the theme toggle flips the theme and the choice survives a reload',
  width: 1280,
  order: 9,                  // it reloads the page or leaves state behind, so it runs after the specs that do not
  claims: (el) => el.attrs.id === 'theme-toggle',
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      const dark = () => page.evaluate(() => document.documentElement.classList.contains(document.documentElement.dataset.themeClass || 'dark'));
      const before = await dark();
      await c.locator.click();
      const after = await dark();
      if (after === before) fail('pressing the theme toggle did not change the theme');
      await page.reload({ waitUntil: 'load' });
      if (await dark() !== after) fail('the theme chosen with the toggle did not survive a reload');
      n++;
    }
    return n;
  },
};
