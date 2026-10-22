// An Alpine tab strip (the recorded runs on the homepage, the scenarios on command and responsibility pages):
// pressing a tab selects it alone and shows its panel alone. Alpine applies a change after the click, so each
// verdict waits for the page to settle before it is read.
const settle = async (page, ms = 1500) => {
  const until = Date.now() + ms;
  while (Date.now() < until) { await page.waitForTimeout(40); if (await page.evaluate(() => document.readyState === 'complete')) return; }
};
export default {
  id: 'tabs',
  title: 'a tab selects itself alone and shows its panel alone',
  width: 1280,
  claims: (el) => el.attrs.role === 'tab' && 'x-on:click' in el.attrs,
  async exercise({ controls, page, fail }) {
    const groups = await page.evaluate((ns) => {
      const byList = new Map();
      for (const n of ns) {
        const tab = document.querySelector(`[data-mj-control="${n}"]`);
        const list = tab.closest('[role="tablist"]');
        if (!byList.has(list)) byList.set(list, []);
        byList.get(list).push(n);
      }
      return [...byList.values()];
    }, controls.map((c) => c.n));
    const read = (group, i) => page.evaluate(({ group, i }) => {
      const tabs = group.map((g) => document.querySelector(`[data-mj-control="${g}"]`));
      const scope = tabs[0].closest('[x-data]');
      const loose = [...scope.children].filter((c) => c.getAttribute('role') !== 'tablist' && c.hasAttribute('x-show'));
      const panels = tabs.map((t, k) => (t.getAttribute('aria-controls') ? document.getElementById(t.getAttribute('aria-controls')) : loose[k]) || null);
      const visible = (el) => !!el && getComputedStyle(el).display !== 'none' && el.getClientRects().length > 0;
      const selected = tabs.map((t) => t.getAttribute('aria-selected'));
      const shown = panels.map(visible);
      return {
        missing: panels.filter((p) => !p).length,
        ok: selected[i] === 'true' && selected.filter((v) => v === 'true').length === 1 && shown[i] && shown.filter(Boolean).length === 1,
        selectedCount: selected.filter((v) => v === 'true').length, shownCount: shown.filter(Boolean).length, shownMine: shown[i],
      };
    }, { group, i });
    let n = 0;
    for (const group of groups) {
      for (const [i, id] of group.entries()) {
        await page.locator(`[data-mj-control="${id}"]`).click();
        let state = await read(group, i);
        for (let t = 0; !state.ok && t < 30; t++) { await page.waitForTimeout(50); state = await read(group, i); }
        if (state.missing) fail(`a tab strip has ${state.missing} tab(s) without a panel to show`);
        else if (!state.ok) fail(`pressing tab ${i + 1} of ${group.length} leaves ${state.selectedCount} tab(s) selected and ${state.shownCount} panel(s) shown, its own ${state.shownMine ? 'among them' : 'hidden'}`);
        n++;
      }
    }
    await settle(page, 1);
    return n;
  },
};
