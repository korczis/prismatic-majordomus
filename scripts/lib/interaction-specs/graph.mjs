// An interactive graph: once the drawing is up, an edge-kind filter hides exactly the edges of its kind and
// brings them back; a name in the text list focuses its node in the drawing and describes it; reset clears
// the focus; the search keeps what it finds lit and dims the rest; zoom in and out move the zoom within its
// bounds; Fit brings every visible node back into view. The drawing is read through the figure's own mjGraph
// (site/graph.js). The drawing itself (the canvas, focusable once mounted) is not a control the markup
// declares, so no spec claims it: its keys are site/graph.js's and are described on the page.
const GRAPH = ['data-graph-node', 'data-graph-filter', 'data-graph-reset', 'data-graph-search', 'data-graph-zoom', 'data-graph-fit'];

export default {
  id: 'graph',
  title: 'a filter hides its edge kind; a name focuses its node; reset clears the focus; search, zoom and fit move the view',
  width: 1280,
  claims: (el) => GRAPH.some((k) => k in el.attrs),
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
      } else if ('data-graph-search' in c.attrs) {
        // the first node's label finds at least that node; whatever does not match steps back; clearing restores
        const probe = await page.evaluate((i) => {
          const cy = document.querySelector(`[data-mj-figure="${i}"]`).mjGraph;
          return cy.nodes()[0].data('label');
        }, fig);
        await c.locator.scrollIntoViewIfNeeded(); await c.locator.fill(probe);
        try {
          await page.waitForFunction((i) => document.querySelector(`[data-mj-figure="${i}"] [data-graph-count]`).textContent.trim() !== '', fig, { timeout: 5000 });
        } catch { fail('typing in the search never reported a count of matches'); }
        const found = await page.evaluate(({ i, q }) => {
          const cy = document.querySelector(`[data-mj-figure="${i}"]`).mjGraph;
          const needle = q.trim().toLowerCase();
          const matches = (n) => [n.data('label'), n.data('kind'), n.data('summary')].some((s) => s != null && String(s).toLowerCase().includes(needle));
          const wrong = cy.nodes().filter((n) => Boolean(n.data('dimmed')) === matches(n)).length;
          return { first: !cy.nodes()[0].data('dimmed'), wrong, count: document.querySelector(`[data-mj-figure="${i}"] [data-graph-count]`).textContent.trim() };
        }, { i: fig, q: probe });
        if (!found.first) fail(`searching for "${probe}" dims the node it names`);
        if (found.wrong) fail(`searching for "${probe}" dims ${found.wrong} node(s) the wrong way (a match dimmed, or a non-match left lit)`);
        if (!/^\d+ of \d+$/.test(found.count)) fail(`searching for "${probe}" reports "${found.count}", not "N of M"`);
        await c.locator.fill('');
        // cleared, the search keeps nothing lit or dimmed of its own; a node a reader had selected keeps its
        // neighbourhood lit, as it did before the search
        try {
          await page.waitForFunction((i) => {
            const cy = document.querySelector(`[data-mj-figure="${i}"]`).mjGraph;
            const held = cy.$('node:selected');
            const lit = held.length ? held[0].closedNeighborhood().nodes() : cy.nodes();
            return cy.nodes().every((n) => !n.data('match')) && lit.every((n) => !n.data('dimmed'));
          }, fig, { timeout: 5000 });
        } catch { fail('clearing the search leaves nodes dimmed'); }
      } else if ('data-graph-zoom' in c.attrs || 'data-graph-fit' in c.attrs) {
        const view = () => page.evaluate((i) => {
          const cy = document.querySelector(`[data-mj-figure="${i}"]`).mjGraph;
          return { z: cy.zoom(), min: cy.minZoom(), max: cy.maxZoom() };
        }, fig);
        // a fit or a search may still be easing the view: read it once it has come to rest
        const still = () => page.waitForFunction((i) => !document.querySelector(`[data-mj-figure="${i}"]`).mjGraph.animated(), fig, { timeout: 5000 }).catch(() => {});
        await still();
        if ('data-graph-zoom' in c.attrs) {
          const dir = c.attrs['data-graph-zoom'];
          const before = await view();
          await c.locator.scrollIntoViewIfNeeded(); await c.locator.click();
          await still();
          const after = await view();
          if (after.z < after.min - 1e-9 || after.z > after.max + 1e-9) fail(`zooming ${dir} leaves the zoom ${after.z} outside ${after.min}..${after.max}`);
          if (dir === 'in' && before.z < before.max && !(after.z > before.z)) fail(`zoom in did not raise the zoom (${before.z} -> ${after.z})`);
          if (dir === 'out' && before.z > before.min && !(after.z < before.z)) fail(`zoom out did not lower the zoom (${before.z} -> ${after.z})`);
        } else {
          // zoom in past the fitted view, then Fit: the zoom comes back down and every visible node is in view
          const zoomed = await page.evaluate((i) => {
            const cy = document.querySelector(`[data-mj-figure="${i}"]`).mjGraph;
            cy.zoom({ level: Math.min(cy.maxZoom(), cy.zoom() * 2), renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
            return cy.zoom();
          }, fig);
          await c.locator.scrollIntoViewIfNeeded(); await c.locator.click();
          await still();
          const fitted = await page.evaluate((i) => {
            const cy = document.querySelector(`[data-mj-figure="${i}"]`).mjGraph;
            const e = cy.extent(), tol = 1;
            const out = cy.nodes(':visible').filter((n) => {
              const b = n.boundingBox();
              return b.x1 < e.x1 - tol || b.x2 > e.x2 + tol || b.y1 < e.y1 - tol || b.y2 > e.y2 + tol;
            }).length;
            return { z: cy.zoom(), out };
          }, fig);
          if (fitted.z > zoomed + 1e-9) fail(`Fit after zooming in left the zoom higher (${zoomed} -> ${fitted.z})`);
          if (fitted.out) fail(`Fit leaves ${fitted.out} visible node(s) outside the view`);
        }
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
