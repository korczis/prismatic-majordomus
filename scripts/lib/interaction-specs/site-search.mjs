// The site's one search (site/search.js, partials/site-search.html): the navbar's trigger opens a
// modal dialog with focus in its field, as `/` and Ctrl+K do; a query some page matches lists
// options, the arrows move the active one and its address answers 200; a query nothing matches
// lists none and says so; Escape and the close button close it and focus returns to what opened
// it. Exercised at a phone's width, where the dialog is a full-screen sheet: it must not be
// wider than the viewport.
export default {
  id: 'site-search',
  title: 'the site search opens from the navbar and the keyboard, lists pages that exist, and closes back to its opener',
  width: 390,
  claims: (el) => 'data-site-search-open' in el.attrs || 'data-site-search-close' in el.attrs
    || el.attrs.id === 'site-search-input' || 'data-site-search-result' in el.attrs,
  async exercise({ controls, page, fail }) {
    const trigger = controls.find((c) => 'data-site-search-open' in c.attrs);
    const field = controls.find((c) => c.attrs.id === 'site-search-input');
    const closer = controls.find((c) => 'data-site-search-close' in c.attrs);
    if (!trigger || !field || !closer) {
      fail('the page does not carry the trigger, the field and the close button together');
      return controls.length;
    }
    const dialog = page.locator('#site-search');
    const options = page.locator('#site-search-results [role="option"]');
    const isOpen = () => dialog.evaluate((d) => d.open);
    const focusIn = () => page.evaluate(() => document.activeElement && document.activeElement.id);

    // the trigger: opens the dialog, focus in the field, and says so
    await trigger.locator.click();
    if (!(await isOpen())) fail('pressing the trigger did not open the dialog');
    if (await focusIn() !== 'site-search-input') fail('the open dialog does not put focus in its field');
    if (await trigger.locator.getAttribute('aria-expanded') !== 'true') fail('the trigger does not say aria-expanded="true" while the dialog is open');
    const box = await dialog.boundingBox();
    const vw = page.viewportSize().width;
    if (!box || box.width > vw + 0.5) fail(`the dialog is ${box ? Math.round(box.width) : 0}px wide in a ${vw}px viewport`);

    // the field: a query many pages match lists options, each a page that answers
    await field.locator.fill('decision');
    await page.waitForFunction(() => document.querySelectorAll('#site-search-results [role="option"]').length > 0,
      null, { timeout: 15000, polling: 100 }).catch(() => {});
    if (await options.count() === 0) fail('a query many pages match ("decision") lists no option');
    else {
      const href = await options.first().getAttribute('href');
      const status = await page.evaluate((h) => fetch(h).then((r) => r.status), href);
      if (status !== 200) fail(`the first option ${href} answers ${status}, not 200`);
      const first = await field.locator.getAttribute('aria-activedescendant');
      if (await options.count() > 1) {
        await field.locator.press('ArrowDown');
        const second = await field.locator.getAttribute('aria-activedescendant');
        if (!second || second === first) fail('ArrowDown does not move the active option');
        if (await page.locator(`#${second}`).getAttribute('aria-selected') !== 'true') fail('the active option does not say aria-selected="true"');
      }
    }
    await field.locator.fill('zz-no-such-thing-zz');
    await page.waitForTimeout(120);
    if (await options.count() !== 0) fail('a query nothing matches still lists options');
    const said = await page.locator('#site-search-status').innerText();
    if (!/^No page matches/.test(said.trim())) fail(`a query nothing matches does not say so: "${said.trim()}"`);

    // Escape closes, and focus returns to the trigger that opened it
    await field.locator.press('Escape');
    if (await isOpen()) fail('Escape did not close the dialog');
    if (!(await trigger.locator.evaluate((t) => t === document.activeElement))) fail('closing with Escape did not return focus to the trigger');
    if (await trigger.locator.getAttribute('aria-expanded') !== 'false') fail('the closed dialog leaves the trigger aria-expanded="true"');

    // the keyboard: `/` opens it from the page, and the close button closes it
    await page.evaluate(() => document.activeElement && document.activeElement.blur());
    await page.keyboard.press('/');
    if (!(await isOpen())) fail('pressing / on the page did not open the dialog');
    await closer.locator.click();
    if (await isOpen()) fail('the close button did not close the dialog');
    await page.keyboard.press('Control+k');
    if (!(await isOpen())) fail('Ctrl+K did not open the dialog');
    await page.keyboard.press('Escape');
    if (await isOpen()) fail('Escape did not close the dialog opened with Ctrl+K');
    if (await options.count() !== 0) fail('the closed dialog leaves options in the page');
    return 3 + controls.filter((c) => 'data-site-search-result' in c.attrs).length;
  },
};
