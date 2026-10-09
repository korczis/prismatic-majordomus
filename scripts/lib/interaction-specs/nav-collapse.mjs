// The mobile menu: the navbar's collapse (site/nav.js). The toggle opens the panel it names and says so, closes it
// again, and Escape closes it and returns focus to the toggle.
export default {
  id: 'nav-collapse',
  title: 'the mobile menu opens and closes the panel it names',
  width: 390,
  claims: (el) => 'data-collapse-toggle' in el.attrs,
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      const target = page.locator(`#${c.attrs['data-collapse-toggle']}`);
      if (await target.count() !== 1) { fail(`the toggle names #${c.attrs['data-collapse-toggle']}, which the page does not carry once`); n++; continue; }
      if (await target.isVisible()) fail('the menu panel is open before the toggle is pressed');
      await c.locator.click();
      if (!(await target.isVisible())) fail('pressing the toggle did not open the menu panel');
      if (await c.locator.getAttribute('aria-expanded') !== 'true') fail('the open toggle does not say aria-expanded="true"');
      await c.locator.click();
      if (await target.isVisible()) fail('pressing the toggle again did not close the menu panel');
      // Escape closes it too, and gives focus back to the toggle: a menu three screens tall
      // on a phone must not be left open with no way out but scrolling back to the top
      await c.locator.click();
      await page.keyboard.press('Escape');
      if (await target.isVisible()) fail('Escape did not close the open menu panel');
      if (await c.locator.getAttribute('aria-expanded') !== 'false') fail('after Escape the toggle still says aria-expanded="true"');
      if (!(await c.locator.evaluate((b) => b === document.activeElement))) fail('after Escape focus did not return to the toggle');
      n++;
    }
    return n;
  },
};
