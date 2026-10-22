// An interactive graph: once the drawing is up, an edge-kind filter hides exactly the edges of its kind and
// brings them back; a name in the text list focuses its node in the drawing and describes it; reset clears
// the focus. The drawing is read through the figure's own mjGraph (site/graph.js).
export default {
  id: 'graph',
  title: 'a filter hides its edge kind; a name focuses its node; reset clears the focus',
  width: 1280,
  claims: (el) => 'data-graph-node' in el.attrs || 'data-graph-filter' in el.attrs || 'data-graph-reset' in el.attrs,
  async exercise({ controls, page, fail }) {
    // A figure may sit inside a closed disclosure (the homepage keeps its graph one click away). A reader opens
    // it before using the drawing, so the spec does too; it used to pass only because the disclosure spec had
    // opened the outer details on its way to a summary nested inside it, and left it open.
    const figures = await page.evaluate(() => [...document.querySelectorAll('figure[data-graph]')].map((f, i) => {
      for (let d = f.closest('details'); d; d = d.parentElement && d.parentElement.closest('details')) d.open = true;
      f.setAttribute('data-mj-figure', String(i)); return i;
    }));
    for (const f of figures) {
      await page.locator(`[data-mj-figure="${f}"]`).scrollIntoViewIfNeeded();
      try { await page.waitForFunction((i) => !!document.querySelector(`[data-mj-figure="${i}"]`).mjGraph, f, { timeout: 15000 }); }
      catch { fail(`graph ${f + 1} never drew (no Cytoscape instance on its figure after 15s)`); }
    }
    let n = 0;
    for (const c of controls) {
      const fig = await c.locator.evaluate((el) => el.closest('figure[data-graph]').getAttribute('data-mj-figure'));
      const ready = await page.evaluate((i) => !!document.querySelector(`[data-mj-figure="${i}"]`).mjGraph, fig);
      if (!ready) { n++; continue; }                      // already reported once for the figure
      if ('data-graph-filter' in c.attrs) {
        const kind = c.attrs['data-graph-filter'];
        const hidden = () => page.evaluate(({ i, k }) => {
          const cy = document.querySelector(`[data-mj-figure="${i}"]`).mjGraph;
          const edges = cy.edges().filter((e) => e.data('kind') === k);
          return { total: edges.length, hidden: edges.filter((e) => e.style('display') === 'none').length };
        }, { i: fig, k: kind });
        await c.locator.scrollIntoViewIfNeeded(); await c.locator.uncheck();
        const off = await hidden();
        if (off.total === 0) fail(`the filter ${kind} names an edge kind the drawing has none of`);
        else if (off.hidden !== off.total) fail(`unchecking ${kind} hides ${off.hidden} of its ${off.total} edges`);
        await c.locator.check();
        if ((await hidden()).hidden !== 0) fail(`checking ${kind} again leaves edges hidden`);
      } else if ('data-graph-reset' in c.attrs) {
        const detail = () => page.evaluate((i) => document.querySelector(`[data-mj-figure="${i}"] [data-graph-detail]`).textContent.trim(), fig);
        const empty = await detail();
        await page.evaluate((i) => { const cy = document.querySelector(`[data-mj-figure="${i}"]`).mjGraph; cy.nodes()[0].emit('tap'); }, fig);
        if (await detail() === empty) fail('tapping a node did not describe it, so reset has nothing to clear');
        await c.locator.scrollIntoViewIfNeeded(); await c.locator.click();
        if (await detail() !== empty) fail('reset did not clear the description of the focused node');
      } else {
        const id = c.attrs['data-graph-node'];
        const result = await page.evaluate(({ i, id, n }) => {
          const fig = document.querySelector(`[data-mj-figure="${i}"]`);
          const cy = fig.mjGraph;
          const node = cy.getElementById(id);
          if (!node || !node.length) return { missing: true };
          const el = document.querySelector(`[data-mj-control="${n}"]`);
          const d = el.closest('details'); if (d) d.open = true;
          const ev = new MouseEvent('click', { bubbles: true, cancelable: true });
          const notPrevented = el.dispatchEvent(ev);
          const text = fig.querySelector('[data-graph-detail]').textContent;
          return { missing: false, selected: node.selected(), described: text.includes(node.data('label')), navigates: notPrevented && el.tagName === 'A' };
        }, { i: fig, id, n: c.n });
        if (result.missing) fail(`the list names node ${id}, which the drawing does not have`);
        else {
          if (!result.selected) fail(`clicking ${id} in the list did not select its node`);
          if (!result.described) fail(`clicking ${id} in the list did not describe its node`);
          if (result.navigates) fail(`clicking ${id} in the list navigates away instead of focusing its node`);
        }
      }
      n++;
    }
    return n;
  },
};
