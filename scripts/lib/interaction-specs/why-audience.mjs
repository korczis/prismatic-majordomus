// The Why page's "who you are" chooser: a radio per audience, pressed through its label, shows that audience's
// panel and no other (pure CSS, :checked ~ sibling, so it works without JavaScript too).
export default {
  id: 'why-audience',
  title: 'choosing an audience shows its panel alone',
  width: 1280,
  claims: (el) => el.tag === 'input' && el.attrs.type === 'radio' && el.attrs.name === 'why-who',
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      const who = String(c.attrs.id || '').replace(/^who-/, '');
      const label = page.locator(`label[for="${c.attrs.id}"]`);
      if (await label.count() !== 1) { fail(`the audience ${who} has no label to press`); n++; continue; }
      await label.scrollIntoViewIfNeeded(); await label.click();
      const state = await page.evaluate((w) => {
        const panels = [...document.querySelectorAll('.why-who-panel')];
        return { shown: panels.filter((p) => getComputedStyle(p).display !== 'none').map((p) => p.dataset.who), total: panels.length };
      }, who);
      if (!(await c.locator.isChecked())) fail(`pressing the label for ${who} did not choose it`);
      if (state.shown.length !== 1 || state.shown[0] !== who) fail(`choosing ${who} shows ${JSON.stringify(state.shown)} of ${state.total} panels`);
      n++;
    }
    return n;
  },
};
