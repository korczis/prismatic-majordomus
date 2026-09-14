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
      await c.locator.click();
      await page.waitForTimeout(60);
      const got = await page.evaluate(() => window.__mjClipboard ?? '');
      if (got !== expected) fail(`the clipboard holds ${JSON.stringify(got.slice(0, 60))}, not the snippet ${JSON.stringify(expected.slice(0, 60))}`);
      const label = (await c.locator.innerText()).trim();
      if (label !== 'Copied') fail(`after copying the button says "${label}", not "Copied"`);
      n++;
    }
    return n;
  },
};
