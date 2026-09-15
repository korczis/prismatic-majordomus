// The navigation dropdowns: Flowbite's dropdown. Each toggle opens the menu it names; pressing it again closes it.
export default {
  id: 'nav-dropdown',
  title: 'each navigation dropdown opens and closes the menu it names',
  width: 1280,
  claims: (el) => 'data-dropdown-toggle' in el.attrs,
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      const id = c.attrs['data-dropdown-toggle'];
      const menu = page.locator(`#${id}`);
      if (await menu.count() !== 1) { fail(`a dropdown names #${id}, which the page does not carry once`); n++; continue; }
      if (!(await c.locator.isVisible())) { fail(`the toggle for #${id} is not visible at 1280px`); n++; continue; }
      await c.locator.click();
      if (!(await menu.isVisible())) fail(`pressing the toggle did not open #${id}`);
      await c.locator.click();
      await page.waitForTimeout(50);
      if (await menu.isVisible()) fail(`pressing the toggle again did not close #${id}`);
      n++;
    }
    return n;
  },
};
