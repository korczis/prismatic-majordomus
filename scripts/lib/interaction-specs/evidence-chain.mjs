// The homepage's evidence chain: a node is a button, and choosing it shows that node's detail —
// and only that one — in the panel beside the drawing, marks the node pressed and every other
// node not, and the detail it shows carries the figure the node itself shows.
//
// A choice is an Alpine update, so the verdict waits, bounded, for the state it asserts rather
// than reading the page once; a click that has not shown within the bound is judged to have
// failed, and the finding names what the panel shows instead.
const SETTLE_MS = 2000;

export default {
  id: 'evidence-chain',
  title: 'choosing a node of the evidence chain shows that node, alone, in the panel',
  width: 1280,
  claims: (el) => el.tag === 'button' && 'data-chain-node' in el.attrs,
  async exercise({ controls, page, fail }) {
    let n = 0;
    // the last node first, so the first click always moves the panel off its default
    for (const c of [...controls].reverse()) {
      const id = c.attrs['data-chain-node'];
      const read = () => c.locator.evaluate((btn) => {
        const fig = btn.closest('[data-chain]');
        const shown = [...fig.querySelectorAll('[data-chain-detail]')]
          .filter((d) => getComputedStyle(d).display !== 'none')
          .map((d) => d.getAttribute('data-chain-detail'));
        const pressed = [...fig.querySelectorAll('[data-chain-node]')]
          .filter((b) => b.getAttribute('aria-pressed') === 'true')
          .map((b) => b.getAttribute('data-chain-node'));
        const detail = fig.querySelector(`[data-chain-detail="${btn.getAttribute('data-chain-node')}"]`);
        const figure = btn.querySelector('span') ? btn.querySelector('span').textContent.trim() : null;
        return { shown, pressed, figure, detailText: detail ? detail.textContent : '' };
      });
      await c.locator.click();
      const until = Date.now() + SETTLE_MS;
      let now = await read();
      while (!(now.shown.length === 1 && now.shown[0] === id && now.pressed.length === 1 && now.pressed[0] === id) && Date.now() < until) {
        await page.waitForTimeout(50);
        now = await read();
      }
      if (now.shown.length !== 1 || now.shown[0] !== id) fail(`choosing ${id} shows [${now.shown.join(', ')}] after ${SETTLE_MS} ms`);
      else if (now.pressed.length !== 1 || now.pressed[0] !== id) fail(`choosing ${id} leaves [${now.pressed.join(', ')}] pressed`);
      else if (now.figure && !now.detailText.includes(now.figure)) fail(`the node ${id} shows ${now.figure}, and its detail does not`);
      n++;
    }
    return n;
  },
};
