// A copy button: it puts exactly the text of the snippet it belongs to on the clipboard, and says so.
export default {
  id: 'copy-button',
  title: 'a copy button puts its snippet on the clipboard and says Copied',
  width: 1280,
  claims: (el) => el.tag === 'button' && /navigator\.clipboard\.writeText\(\$refs\.src\.innerText\)/.test(el.attrs['x-on:click'] || ''),
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      const expected = await c.locator.evaluate((b) => {
        const scope = b.closest('[x-data]');
        const src = scope && scope.querySelector('[x-ref="src"]');
        return src ? src.innerText : null;
      });
      if (expected === null) { fail('a copy button has no x-ref="src" snippet in its component'); n++; continue; }
      await c.locator.scrollIntoViewIfNeeded();
      // "Copied" lasts 1600 ms before the button says Copy again, and a stalled driver can read past it: an observer
      // set before the click records the label when it changes, so the check judges what the button said, not what
      // it says when the driver gets round to reading it
      await c.locator.evaluate((b) => {
        window.__mjClipboard = null;
        const seen = window.__mjCopySeen = { said: null };
        if (window.__mjCopyObserver) window.__mjCopyObserver.disconnect();
        window.__mjCopyObserver = new MutationObserver(() => {
          if (seen.said === null && b.textContent.trim() === 'Copied') seen.said = 'Copied';
        });
        window.__mjCopyObserver.observe(b, { subtree: true, childList: true, characterData: true, attributes: true });
      });
      await c.locator.click();
      // the wait polls on a timer inside the page, not on animation frames, which a starved renderer delays too
      const r = await page.evaluate(() => new Promise((done) => {
        const t0 = Date.now();
        const tick = () => {
          const seen = window.__mjCopySeen;
          if ((seen.said !== null && window.__mjClipboard !== null) || Date.now() - t0 > 10000) {
            window.__mjCopyObserver.disconnect();
            done({ said: seen.said, clip: window.__mjClipboard });
          } else setTimeout(tick, 50);
        };
        tick();
      }));
      const got = r.clip ?? '';
      if (got !== expected) fail(`the clipboard holds ${JSON.stringify(got.slice(0, 60))}, not the snippet ${JSON.stringify(expected.slice(0, 60))}`);
      if (r.said !== 'Copied') fail(`after copying the button never says "Copied" (it says "${(await c.locator.innerText()).trim()}")`);
      n++;
    }
    return n;
  },
};
