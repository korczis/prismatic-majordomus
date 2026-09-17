// A native disclosure: its summary opens the details it heads, and closes them again.
//
// A click is not always a toggle. In a contended headless browser one can be coalesced with the
// next or swallowed outright, and reading `details.open` in the instruction after the click then
// reports a working disclosure as broken — twice over, because the second click's assertion is
// judged against a state the first never reached. That is how one swallowed click on one page of
// 1083 turned a probe red on 2026-09-17 while the same markup passed on 112 sibling pages. So
// each click is followed by a bounded wait for the state the disclosure must reach, the failure
// names that state, and a first click that never settles ends the control rather than producing a
// second finding for the same fault.
const SETTLE_MS = 2000;

export default {
  id: 'disclosure',
  title: 'a summary opens and closes the details it heads',
  width: 1280,
  claims: (el) => el.tag === 'summary',
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      const state = () => c.locator.evaluate((s) => s.parentElement && s.parentElement.tagName === 'DETAILS' ? s.parentElement.open : null);
      const settles = async (want) => {
        const until = Date.now() + SETTLE_MS;
        for (;;) {
          if (await state() === want) return true;
          if (Date.now() >= until) return false;
          await page.waitForTimeout(25);
        }
      };
      const before = await state();
      if (before === null) { fail('a summary is not the heading of a details element'); n++; continue; }
      await c.locator.evaluate((s) => { for (let d = s.parentElement.parentElement && s.parentElement.parentElement.closest('details'); d; d = d.parentElement && d.parentElement.closest('details')) d.open = true; s.scrollIntoView(); });
      await c.locator.click();
      if (!await settles(!before)) {
        fail(`pressing the summary did not ${before ? 'close' : 'open'} its details: open stayed ${before} for ${SETTLE_MS} ms`);
        n++; continue;
      }
      await c.locator.click();
      if (!await settles(before)) fail(`pressing the summary again did not restore its details: open stayed ${!before} for ${SETTLE_MS} ms, expected ${before}`);
      n++;
    }
    return n;
  },
};
