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
      const expected = await c.locator.evaluate((b) => {
        const t = document.getElementById(b.getAttribute('data-copy'));
        return t ? t.innerText.trim() : null;
      });
      if (expected === null) { fail(`a copy button names ${await c.locator.getAttribute('data-copy')}, which no element carries`); n++; continue; }
      if (await c.locator.evaluate((b) => b.hidden)) { fail('a copy button is still hidden after the script ran'); n++; continue; }
      // a button in a tab panel that is not selected is reached the way a reader reaches it
      const tab = await c.locator.evaluate((b) => {
        const panel = b.closest('[role="tabpanel"]');
        return panel && panel.hidden ? panel.getAttribute('aria-labelledby') : null;
      });
      if (tab) await page.locator(`[id="${tab}"]`).click();
      await c.locator.scrollIntoViewIfNeeded();
      await c.locator.click();
      await page.waitForTimeout(60);
      const got = await page.evaluate(() => window.__mjClipboard ?? '');
      if (got !== expected) fail(`the clipboard holds ${JSON.stringify(got.slice(0, 60))}, not ${JSON.stringify(expected.slice(0, 60))}`);
      if ((await c.locator.getAttribute('aria-label')) !== 'Copied') fail('after copying, the button is not named "Copied"');
      if ((await c.locator.getAttribute('data-copied')) !== 'true') fail('after copying, the button does not carry data-copied="true"');
      n++;
    }
    return n;
  },
};
