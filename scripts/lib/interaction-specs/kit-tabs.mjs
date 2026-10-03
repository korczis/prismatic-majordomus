// The kit's tab strip (site/templates/kit/code.html `tabs`, wired by site/kit.js): the strip is
// shown once the script runs; pressing a tab selects it alone and shows its panel alone; the arrow
// keys move the selection, as the ARIA tabs pattern asks, and only the selected tab is in the tab
// order.
export default {
  id: 'kit-tabs',
  title: 'a kit tab selects itself alone, shows its panel alone, and the arrow keys move it',
  width: 1280,
  claims: (el) => el.attrs.role === 'tab' && 'data-kit-tab' in el.attrs,
  async exercise({ controls, page, fail }) {
    const groups = await page.evaluate((ns) => {
      const byList = new Map();
      for (const n of ns) {
        const list = document.querySelector(`[data-mj-control="${n}"]`).closest('[role="tablist"]');
        if (!byList.has(list)) byList.set(list, []);
        byList.get(list).push(n);
      }
      return [...byList.values()];
    }, controls.map((c) => c.n));
    const read = (group, i) => page.evaluate(({ group, i }) => {
      const tabs = group.map((g) => document.querySelector(`[data-mj-control="${g}"]`));
      const panels = tabs.map((t) => document.getElementById(t.getAttribute('aria-controls')));
      const shown = panels.map((p) => !!p && !p.hidden && p.getClientRects().length > 0);
      const selected = tabs.map((t) => t.getAttribute('aria-selected') === 'true');
      const focusable = tabs.map((t) => t.tabIndex === 0);
      return {
        missing: panels.filter((p) => !p).length,
        listHidden: tabs[0].closest('[role="tablist"]').hidden,
        ok: selected[i] && selected.filter(Boolean).length === 1 && shown[i] && shown.filter(Boolean).length === 1
          && focusable[i] && focusable.filter(Boolean).length === 1,
      };
    }, { group, i });
    let n = 0;
    for (const group of groups) {
      for (const [i, id] of group.entries()) {
        const tab = page.locator(`[data-mj-control="${id}"]`);
        await tab.scrollIntoViewIfNeeded();
        await tab.click();
        const state = await read(group, i);
        if (state.listHidden) fail('a tab strip is still hidden after the script ran');
        else if (state.missing) fail(`a tab strip has ${state.missing} tab(s) whose panel does not exist`);
        else if (!state.ok) fail(`pressing tab ${i + 1} of ${group.length} does not select it alone, show its panel alone and put it alone in the tab order`);
        n++;
      }
      // from the last tab, ArrowRight wraps to the first
      await page.locator(`[data-mj-control="${group[group.length - 1]}"]`).press('ArrowRight');
      if (!(await read(group, 0)).ok) fail('ArrowRight on the last tab does not select the first');
    }
    return n;
  },
};
