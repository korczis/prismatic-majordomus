// A native disclosure: its summary opens the details it heads, and closes them again.
//
// Two things measured over 313 controls on the 126 pages that carry a summary, 1218 toggles in all:
// a delivered click flips `details.open` in 2 ms at the median, 9 ms at p90 and 177 ms at the very
// worst — that worst under ten parallel browsers and four CPU spinners, far past anything CI does —
// and not one toggle ever exceeded it. When the browser is saturated beyond that, Playwright does
// not deliver the click at all and throws, which reads as `threw: locator.click: Timeout`.
//
// So a click that returns and leaves the state unchanged for two seconds was not slow, it was
// dropped: delivered to a browser that did nothing with it. Waiting longer cannot find it, and one
// dropped click used to read as two findings, because the second assertion was judged against a
// state the first never reached. Hence: a bounded wait for the state each click must reach, one
// retry when it does not arrive, a note on stderr when the retry happens so a degrading runner
// stays visible, and a finding only when two clicks in a row change nothing.
const SETTLE_MS = 2000;   // 11x the worst toggle ever measured; the runner itself is unmeasured

export default {
  id: 'disclosure',
  title: 'a summary opens and closes the details it heads',
  width: 1280,
  claims: (el) => el.tag === 'summary',
  async exercise({ controls, page, route, fail }) {
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
      // One click, and a second only if the first changed nothing — re-reading first, so a toggle
      // that landed between the last poll and now is not undone by a retry it no longer needs.
      const press = async (want) => {
        await c.locator.click();
        if (await settles(want)) return true;
        if (await state() === want) return true;
        process.stderr.write(`interaction-probe: ${route}: a disclosure did not reach open=${want} in ${SETTLE_MS} ms; clicking once more\n`);
        await c.locator.click();
        return settles(want);
      };
      const before = await state();
      if (before === null) { fail('a summary is not the heading of a details element'); n++; continue; }
      await c.locator.evaluate((s) => { for (let d = s.parentElement.parentElement && s.parentElement.parentElement.closest('details'); d; d = d.parentElement && d.parentElement.closest('details')) d.open = true; s.scrollIntoView(); });
      if (!await press(!before)) {
        fail(`pressing the summary did not ${before ? 'close' : 'open'} its details: open stayed ${before} after two clicks, ${SETTLE_MS} ms each`);
        n++; continue;
      }
      if (!await press(before)) fail(`pressing the summary again did not restore its details: open stayed ${!before} after two clicks, ${SETTLE_MS} ms each, expected ${before}`);
      n++;
    }
    return n;
  },
};
