// The mobile menu: Flowbite's collapse. The toggle opens the panel it names and says so, and closes it again.
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
      n++;
    }
    return n;
  },
};
