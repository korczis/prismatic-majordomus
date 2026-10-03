// A copy button: it puts exactly the text of the snippet it belongs to on the clipboard, and says so. It is the
// site's Alpine copy button (components.html `code` and `install`, partials/terminal-run.html), the one app.css
// pages carry; the kit's data-copy button is kit-copy.mjs. When the clipboard refuses, the button says so too, and
// its component's live region announces it: a copy that silently did nothing leaves the reader pasting stale text.
const FAILED = 'Copy failed — select the text';
export default {
  id: 'copy-button',
  title: 'a copy button puts its snippet on the clipboard and says Copied, or says the copy failed',
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
      // a button in a tab panel that is not selected (a recorded run's command, one tab of
      // several) is reached the way a reader reaches it: by selecting its tab first
      const tab = await c.locator.evaluate((b) => {
        const panel = b.closest('[role="tabpanel"]');
        return panel && (panel.hidden || getComputedStyle(panel).display === 'none') ? panel.getAttribute('aria-labelledby') : null;
      });
      if (tab) await page.locator(`[id="${tab}"]`).click();
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
      // the refusal: a clipboard that rejects (a denied permission, an insecure origin) is said on the button and in
      // the component's live region, and the failure stays until the reader acts, so no timer hides it
      const refused = await c.locator.evaluate((b, failed) => new Promise((done) => {
        const clip = navigator.clipboard;
        Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: () => Promise.reject(new Error('denied')) } });
        const live = () => {
          const scope = b.closest('[x-data]');
          const region = scope && scope.querySelector('[aria-live]');
          return region ? region.textContent.trim() : null;
        };
        b.click();
        const until = Date.now() + 10000;
        (function poll() {
          const said = b.textContent.trim(), label = b.getAttribute('aria-label'), region = live();
          if ((said === failed && label === failed && region === failed) || Date.now() > until) {
            Object.defineProperty(navigator, 'clipboard', { configurable: true, value: clip });
            done({ said, label, region });
          } else setTimeout(poll, 50);
        })();
      }), FAILED);
      if (refused.said !== FAILED) fail(`when the clipboard refuses, the button says "${refused.said}", not "${FAILED}"`);
      if (refused.label !== FAILED) fail(`when the clipboard refuses, the button is named "${refused.label}", not "${FAILED}"`);
      if (refused.region === null) fail('the copy button\'s component has no aria-live region to announce a refused copy');
      else if (refused.region !== FAILED) fail(`when the clipboard refuses, the live region says "${refused.region}", not "${FAILED}"`);
      n++;
    }
    return n;
  },
};
