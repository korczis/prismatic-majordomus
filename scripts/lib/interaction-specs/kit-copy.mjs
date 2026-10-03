// The kit's copy button (site/templates/kit/base.html `copy`, wired by site/kit.js): it is shown
// only once the script runs, puts exactly the text of the element it names on the clipboard, and
// says Copied in its accessible name and its state.
export default {
  id: 'kit-copy',
  title: 'a kit copy button puts the text it names on the clipboard and says Copied',
  width: 1280,
  claims: (el) => el.tag === 'button' && 'data-copy' in el.attrs,
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      if (await c.locator.evaluate((b) => b.hidden)) { fail('a copy button is still hidden after the script ran'); n++; continue; }
      // a button in a tab panel that is not selected is reached the way a reader reaches it
      const tab = await c.locator.evaluate((b) => {
        const panel = b.closest('[role="tabpanel"]');
        return panel && panel.hidden ? panel.getAttribute('aria-labelledby') : null;
      });
      if (tab) await page.locator(`[id="${tab}"]`).click();
      const expected = await c.locator.evaluate((b) => {
        const t = document.getElementById(b.getAttribute('data-copy'));
        return t ? t.innerText.trim() : null;
      });
      if (expected === null) { fail(`a copy button names ${await c.locator.getAttribute('data-copy')}, which no element carries`); n++; continue; }
      await c.locator.scrollIntoViewIfNeeded();
      // the button says Copied for 1.6 s and then reverts, and a stalled driver can arrive later than that:
      // the page records the state the moment the script sets it, and the verdict reads the record
      await c.locator.evaluate((b) => {
        window.__mjClipboard = null;
        const said = (b.__mjSaid = { label: false, copied: false });
        new MutationObserver(() => {
          if (b.getAttribute('aria-label') === 'Copied') said.label = true;
          if (b.getAttribute('data-copied') === 'true') said.copied = true;
        }).observe(b, { attributes: true, attributeFilter: ['aria-label', 'data-copied'] });
      });
      await c.locator.click();
      await c.locator.evaluate((b) => new Promise((done) => {
        const until = Date.now() + 10000;
        (function poll() {
          if ((b.__mjSaid.label && b.__mjSaid.copied && window.__mjClipboard !== null) || Date.now() > until) done();
          else setTimeout(poll, 50);
        })();
      }));
      const r = await c.locator.evaluate((b) => ({ got: window.__mjClipboard ?? '', ...b.__mjSaid }));
      if (r.got !== expected) fail(`the clipboard holds ${JSON.stringify(r.got.slice(0, 60))}, not ${JSON.stringify(expected.slice(0, 60))}`);
      if (!r.label) fail('after copying, the button is not named "Copied"');
      if (!r.copied) fail('after copying, the button does not carry data-copied="true"');
      n++;
    }
    return n;
  },
};
