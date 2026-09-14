// A native disclosure: its summary opens the details it heads, and closes them again.
export default {
  id: 'disclosure',
  title: 'a summary opens and closes the details it heads',
  width: 1280,
  claims: (el) => el.tag === 'summary',
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      const state = () => c.locator.evaluate((s) => s.parentElement && s.parentElement.tagName === 'DETAILS' ? s.parentElement.open : null);
      const before = await state();
      if (before === null) { fail('a summary is not the heading of a details element'); n++; continue; }
      await c.locator.evaluate((s) => { for (let d = s.parentElement.parentElement && s.parentElement.parentElement.closest('details'); d; d = d.parentElement && d.parentElement.closest('details')) d.open = true; s.scrollIntoView(); });
      await c.locator.click();
      if (await state() === before) fail(`pressing the summary did not ${before ? 'close' : 'open'} its details`);
      await c.locator.click();
      if (await state() !== before) fail('pressing the summary again did not restore its details');
      n++;
    }
    return n;
  },
};
