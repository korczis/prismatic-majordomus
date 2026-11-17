// A diagram's viewer (site/diagrams.js, ADR 0152): the frame is the diagram's own <pre>, drawn into by the
// time the viewer says it is ready, and the toolbar the build wrote beside it is revealed above it.
// Each control is held to what it says: a zoom button changes the scale and Fit restores the one the frame
// was fitted at; the same from the keyboard, where an arrow key also moves the drawing; Link puts the
// diagram's own address in the address bar; Expand covers the page and gives it back; and every node the
// build linked beside the diagram is a link in the drawing to the same page.
// The state is read through window.mjDiagrams, which the viewer publishes for exactly this.
const mermaidPre = (el) => el.tag === 'pre' && /(^|\s)mermaid(\s|$)/.test(el.attrs.class || '') && el.attrs.tabindex === '0';
export default {
  id: 'diagram',
  title: 'a diagram is drawn in its frame; its toolbar and keys zoom, fit and pan it; Link gives its address; its linked nodes are links',
  width: 1280,
  claims: (el) => 'data-diagram-action' in el.attrs || mermaidPre(el),
  async exercise({ controls, page, fail }) {
    const started = await page.waitForFunction(() => !!window.mjDiagrams, null, { timeout: 15000, polling: 100 }).then(() => true, () => false);
    if (!started) { fail('the page carries a diagram and the viewer never started (no window.mjDiagrams after 15 s)'); return controls.length; }
    // every diagram on the page is drawn or has failed; a large dependency graph takes Mermaid seconds
    await page.evaluate(() => Promise.race([window.mjDiagrams.ready(), new Promise((r) => setTimeout(r, 60000))]));
    const view = (name) => page.evaluate((n) => window.mjDiagrams.state(n), name);
    const near = (a, b) => Math.abs(a - b) < 1e-6;
    let n = 0;
    for (const c of controls) {
      n++;
      const where = await c.locator.evaluate((el) => {
        const pre = el.tagName === 'PRE' ? el : document.getElementById(el.closest('[data-diagram-bar]').getAttribute('data-diagram-bar'));
        for (let d = pre.closest('details'); d; d = d.parentElement && d.parentElement.closest('details')) d.open = true;
        return { name: pre.getAttribute('data-diagram'), id: pre.id, error: pre.getAttribute('data-diagram-error'),
                 svg: !!pre.querySelector('svg'), framed: !!pre.closest('.mj-diagram') };
      });
      if (where.error || !where.svg) { fail(`diagram ${where.name} was not drawn${where.error ? `: ${where.error.slice(0, 120)}` : ''}`); continue; }
      if (!where.framed) { fail(`diagram ${where.name} was drawn outside a viewer frame`); continue; }
      await c.locator.scrollIntoViewIfNeeded();
      // a frame inside a disclosure this spec just opened is fitted when it gets its width
      await page.waitForFunction((name) => { const s = window.mjDiagrams.state(name); return s && s.scale > 0; }, where.name, { timeout: 5000, polling: 50 }).catch(() => {});
      const before = await view(where.name);
      if (!before || !before.rendered || !(before.scale > 0)) { fail(`diagram ${where.name} reports no drawing to control`); continue; }
      if ('data-diagram-action' in c.attrs) {
        const action = c.attrs['data-diagram-action'];
        if (!(await c.locator.isVisible())) { fail(`the ${action} control of diagram ${where.name} is still hidden after the diagram was drawn`); continue; }
        if (action === 'zoom-in' || action === 'zoom-out') {
          await c.locator.click();
          const after = await view(where.name);
          if (action === 'zoom-in' ? !(after.scale > before.scale) : !(after.scale < before.scale)) fail(`${action} left diagram ${where.name} at scale ${after.scale} (it was ${before.scale})`);
          await page.locator(`[data-diagram-bar="${where.id}"] [data-diagram-action="fit"]`).click();
        } else if (action === 'fit') {
          await page.locator(`[data-diagram-bar="${where.id}"] [data-diagram-action="zoom-in"]`).click();
          await c.locator.click();
          const after = await view(where.name);
          if (!near(after.scale, after.fit)) fail(`Fit left diagram ${where.name} at scale ${after.scale}, not the ${after.fit} it fits at`);
        } else if (action === 'link') {
          await c.locator.click();
          const hash = await page.evaluate(() => window.location.hash);
          if (hash !== `#${where.id}`) fail(`Link left the address at "${hash}", not #${where.id}`);
        } else if (action === 'expand') {
          await c.locator.click();
          const covered = await page.waitForFunction((id) => {
            const wrap = document.getElementById(id).closest('.mj-diagram');
            return document.fullscreenElement === wrap || wrap.classList.contains('is-expanded');
          }, where.id, { timeout: 3000, polling: 50 }).then(() => true, () => false);
          if (!covered) fail(`Expand did not put diagram ${where.name} in full screen`);
          await c.locator.click();
          const back = await page.waitForFunction((id) => {
            const wrap = document.getElementById(id).closest('.mj-diagram');
            return document.fullscreenElement !== wrap && !wrap.classList.contains('is-expanded');
          }, where.id, { timeout: 3000, polling: 50 }).then(() => true, () => false);
          if (!back) fail(`a second Expand did not bring diagram ${where.name} back out of full screen`);
        } else if (action === 'download') {
          const [file] = await Promise.all([page.waitForEvent('download', { timeout: 5000 }).catch(() => null), c.locator.click()]);
          if (!file) fail(`SVG started no download for diagram ${where.name}`);
          else if (file.suggestedFilename() !== `${where.name}.svg`) fail(`SVG offered ${file.suggestedFilename()}, not ${where.name}.svg`);
        } else fail(`diagram ${where.name} carries a control this spec does not know: ${action}`);
        continue;
      }
      // the frame itself, from the keyboard
      await c.locator.focus();
      if (!(await c.locator.evaluate((el) => document.activeElement === el))) { fail(`the frame of diagram ${where.name} does not take focus`); continue; }
      await page.keyboard.press('+');
      const zoomed = await view(where.name);
      if (!(zoomed.scale > before.scale)) fail(`+ did not zoom diagram ${where.name} in`);
      await page.keyboard.press('ArrowRight');
      const moved = await view(where.name);
      if (!(moved.x < zoomed.x)) fail(`ArrowRight did not move diagram ${where.name}`);
      await page.keyboard.press('0');
      const fitted = await view(where.name);
      if (!near(fitted.scale, fitted.fit)) fail(`0 did not fit diagram ${where.name} to its frame`);
      // what the build linked beside the diagram is linked in the drawing, to the same page
      const links = await page.evaluate((id) => {
        const listed = [...document.querySelectorAll(`[data-diagram-links="${id}"] a[data-node]`)].map((a) => [a.getAttribute('data-node'), a.href]);
        const drawn = Object.fromEntries([...document.getElementById(id).querySelectorAll('svg a [data-node]')]
          .map((g) => [g.getAttribute('data-node'), g.closest('a').href.baseVal || g.closest('a').getAttribute('href')]));
        return { listed, drawn };
      }, where.id);
      for (const [node, href] of links.listed) {
        if (!(node in links.drawn)) fail(`diagram ${where.name} lists node ${node} as linked and its drawing has no link on it`);
        else if (new URL(links.drawn[node], page.url()).href !== href) fail(`node ${node} of diagram ${where.name} links to ${links.drawn[node]} in the drawing and to ${href} in the list`);
      }
    }
    return n;
  },
};
