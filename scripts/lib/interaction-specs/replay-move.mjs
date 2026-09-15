// The movement buttons of a replay (the challenge on the homepage and on /challenge/): Next selects the
// following step alone and Back the one before, and neither moves past either end. The step tabs themselves
// are the tabs spec's; this one asks only that the buttons move the same selection the tabs show.
export default {
  id: 'replay-move',
  title: 'Next and Back move the replay one step, and stop at either end',
  width: 1280,
  claims: (el) => 'data-replay-move' in el.attrs,
  async exercise({ controls, page, fail }) {
    const roots = await page.evaluate((ns) => [...new Set(ns.map((n) => {
      const r = document.querySelector(`[data-mj-control="${n}"]`).closest('[data-challenge-replay]');
      if (!r.id) r.id = `replay-${Math.random().toString(36).slice(2)}`;
      return r.id;
    }))], controls.map((c) => c.n));
    const selected = (id) => page.evaluate((id) => {
      const tabs = [...document.getElementById(id).querySelectorAll('[role="tab"]')];
      return { at: tabs.findIndex((t) => t.getAttribute('aria-selected') === 'true'), count: tabs.filter((t) => t.getAttribute('aria-selected') === 'true').length, total: tabs.length };
    }, id);
    const settleTo = async (id, want) => {
      let s = await selected(id);
      for (let t = 0; (s.at !== want || s.count !== 1) && t < 30; t++) { await page.waitForTimeout(50); s = await selected(id); }
      return s;
    };
    let n = 0;
    for (const id of roots) {
      const next = page.locator(`#${id} [data-replay-move="1"]`);
      const back = page.locator(`#${id} [data-replay-move="-1"]`);
      const { total } = await selected(id);
      await page.locator(`#${id} [role="tab"]`).first().click();
      let s = await settleTo(id, 0);
      if (s.at !== 0) { fail(`selecting the first step leaves step ${s.at + 1} selected`); continue; }
      await next.click(); s = await settleTo(id, 1); n++;
      if (s.at !== 1 || s.count !== 1) fail(`Next from the first step selects step ${s.at + 1} (${s.count} selected), not step 2`);
      await back.click(); s = await settleTo(id, 0); n++;
      if (s.at !== 0 || s.count !== 1) fail(`Back from step 2 selects step ${s.at + 1} (${s.count} selected), not step 1`);
      if ((await back.getAttribute('aria-disabled')) !== 'true') fail('Back is not marked aria-disabled on the first step');
      await page.locator(`#${id} [role="tab"]`).last().click(); s = await settleTo(id, total - 1);
      await next.click(); s = await settleTo(id, total - 1); n++;
      if (s.at !== total - 1) fail(`Next on the last step moves the replay to step ${s.at + 1}`);
      if ((await next.getAttribute('aria-disabled')) !== 'true') fail('Next is not marked aria-disabled on the last step');
    }
    return n;
  },
};
