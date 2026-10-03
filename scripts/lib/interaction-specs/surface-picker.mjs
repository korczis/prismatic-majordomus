// The homepage's "one declaration, every surface" picker: choosing a capability shows that
// capability — its id, and a command line, HTTP operation and MCP tool that are its own — read
// from the slice the build publishes (graphs/surfaces.json), fetched on first use.
//
// A choice is a fetch and an Alpine update, so the verdict waits, bounded, for the state it
// asserts rather than reading the page once. The bound covers the fetch of a ~30 KB file from the
// probe's own server; a choice that has not shown within it is judged to have failed, and the
// finding names what the page shows instead.
const SETTLE_MS = 5000;

export default {
  id: 'surface-picker',
  title: 'choosing a capability shows that capability on every surface it reaches',
  width: 1280,
  claims: (el) => el.tag === 'select' && 'data-surface-picker' in el.attrs,
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      const read = () => c.locator.evaluate((sel) => {
        const scope = sel.closest('[data-surfaces]');
        const text = (q) => { const el = scope && scope.querySelector(q); return el ? el.textContent.trim() : null; };
        const id = scope && scope.querySelector('[data-surface-id]');
        return { id: id ? id.getAttribute('data-surface-id') : null, idText: text('[data-surface-id]'),
                 cli: text('[data-surface="cli"]'), http: text('[data-surface="http"]'), mcp: text('[data-surface="mcp"]'),
                 options: [...sel.options].map((o) => o.value), current: sel.value };
      });
      const start = await read();
      // a capability other than the one shown, taken from the picker's own list: the last one
      const target = [...start.options].reverse().find((v) => v !== start.current);
      if (!target) { fail('the picker offers a single capability, so choosing shows nothing new'); n++; continue; }
      const slice = await page.evaluate(async () => {
        const url = document.querySelector('[data-surfaces]').getAttribute('x-data').match(/fetch\('([^']+)'\)/)[1];
        return (await fetch(url)).json();
      });
      const want = slice.find((s) => s.id === target);
      if (!want) { fail(`the picker offers ${target}, which the published slice does not carry`); n++; continue; }
      const expect = { cli: want.cli || 'not exposed', http: want.http ? `${want.http.method} ${want.http.path}` : 'not exposed', mcp: want.mcp || 'not exposed' };
      await c.locator.selectOption(target);
      const until = Date.now() + SETTLE_MS;
      let now = await read();
      while (!(now.id === target && now.idText === target && now.cli === expect.cli && now.http === expect.http && now.mcp === expect.mcp) && Date.now() < until) {
        await page.waitForTimeout(50);
        now = await read();
      }
      if (now.id !== target || now.idText !== target) fail(`choosing ${target} left ${now.id} shown after ${SETTLE_MS} ms`);
      else for (const k of ['cli', 'http', 'mcp']) if (now[k] !== expect[k]) fail(`choosing ${target} shows ${k} ${JSON.stringify(now[k])}; the published slice says ${JSON.stringify(expect[k])}`);
      n++;
    }
    return n;
  },
};
