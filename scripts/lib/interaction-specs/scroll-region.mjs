// A keyboard-reachable scroll region (a wide table, a long listing): a reader can reach it, it takes focus, and when
// its content overflows the arrow keys scroll it: horizontally with ArrowRight, vertically with ArrowDown. The box
// that scrolls is the region itself or the nearest ancestor that clips it (a <pre> inside its scrolling wrapper).
// A region inside a hidden tab panel is revealed the way a reader would reveal it, by choosing that tab; one that
// nothing on the page can reveal is the finding. A panel whose tab was chosen and that did not appear in the time
// given is a different finding and says so: the page offers the way in, and what failed is the wait or the tab.
const OTHER = /^data-(collapse|dropdown|modal|drawer|tabs|accordion|tooltip|popover)-(toggle|target)$/;
// The keys start a smooth scroll that outlives the wait below, which ends at the first pixel. A box the previous step
// scrolled (this region's other axis, or a <pre> sharing its wrapper's box) can still be animating, and a reset and
// keys pressed into that animation are lost with it: on a loaded runner the box then never moves. Such a box is first
// let finish, until its scrollend (or, without one, until neither offset has changed for 200 ms), polled on a timer
// and bounded at 10 s
const settle = (page) => page.evaluate(() => new Promise((done) => {
  const b = document.querySelector('[data-mj-box="1"]');
  if (b !== window.__mjScrolled) { done(); return; }
  const ends = 'onscrollend' in window;
  const t0 = Date.now(); let last = '', same = 0;
  const tick = () => {
    const v = `${b.scrollLeft},${b.scrollTop}`; same = v === last ? same + 1 : 0; last = v;
    if ((ends ? !b.__mjMoving && same >= 1 : same >= 4) || Date.now() - t0 > 10000) done(); else setTimeout(tick, 50);
  };
  tick();
}));
// the box is marked as the one this step scrolls, and from then on says whether it is moving
const mark = (page, prop) => page.evaluate((prop) => {
  const b = document.querySelector('[data-mj-box="1"]');
  if (!b.__mjTracked) {
    b.__mjTracked = true;
    b.addEventListener('scroll', () => { b.__mjMoving = true; });
    b.addEventListener('scrollend', () => { b.__mjMoving = false; });
  }
  b[prop] = 0; window.__mjScrolled = b;
}, prop);
// How long a chosen tab's panel is waited for, in milliseconds. x-show draws a panel on a later animation frame, and a
// loaded runner can hold that frame back for seconds; the homepage alone has eighteen regions behind such a tab. The
// bound is generous because it costs nothing when the panel appears, finite because a panel that never does must
// still be judged, and named in the finding. MJ_PROBE_REVEAL_MS shortens it for a case that drives a panel which
// never appears.
const REVEAL_MS = Number(process.env.MJ_PROBE_REVEAL_MS) > 0 ? Number(process.env.MJ_PROBE_REVEAL_MS) : 30000;
export default {
  id: 'scroll-region',
  title: 'a scroll region can be reached, takes focus, and scrolls from the keyboard when it overflows',
  width: 1280,
  claims: (el) => el.attrs.tabindex === '0' && el.tag !== 'a' && !['button', 'input', 'select', 'textarea', 'summary'].includes(el.tag)
    && !Object.keys(el.attrs).some((k) => k.startsWith('x-on:') || k.startsWith('@') || k === 'x-model' || k.startsWith('data-graph-') || OTHER.test(k))
    && !['tab', 'switch', 'checkbox', 'radio', 'menuitem', 'slider', 'option'].includes(el.attrs.role),
  async exercise({ controls, page, fail }) {
    let n = 0;
    for (const c of controls) {
      const sel = `[data-mj-control="${c.n}"]`;
      // reveal: open enclosing disclosures, and choose the tab whose panel hides the region
      const reveal = await page.evaluate((sel) => {
        const el = document.querySelector(sel);
        for (let d = el.closest('details'); d; d = d.parentElement && d.parentElement.closest('details')) d.open = true;
        let hidden = null;
        for (let p = el; p; p = p.parentElement) if (getComputedStyle(p).display === 'none') hidden = p;
        if (!hidden) return { label: null };
        const scope = hidden.parentElement && hidden.parentElement.closest('[x-data]');
        let tab = hidden.id ? document.querySelector(`[role="tab"][aria-controls="${hidden.id}"]`) : null;
        if (!tab && scope) {
          const panels = [...scope.children].filter((k) => k.getAttribute('role') !== 'tablist' && k.hasAttribute('x-show'));
          const tabs = [...scope.querySelectorAll('[role="tab"]')];
          const i = panels.indexOf(hidden);
          if (i >= 0 && tabs[i]) tab = tabs[i];
        }
        if (!tab) return { label: `${hidden.tagName.toLowerCase()}${hidden.getAttribute('x-show') ? '[x-show]' : ''}` };
        tab.setAttribute('data-mj-reveal', '1');
        hidden.setAttribute('data-mj-panel', '1');
        return { tab: true };
      }, sel);
      // what the page's own state says of a panel that did not appear: 'shown', 'hidden', or null when it cannot say
      let state = null, appeared = true;
      if (reveal.tab) {
        await page.locator('[data-mj-reveal="1"]').click();
        await page.evaluate(() => document.querySelector('[data-mj-reveal="1"]').removeAttribute('data-mj-reveal'));
        // x-show shows a panel on the next animation frame, not in the click, and a loaded runner can hold that frame
        // back for seconds: the bound is generous, because it costs nothing when the panel appears and only a region
        // that stays hidden waits it out. The wait polls on an interval, not on animation frames (Playwright's
        // default), because a starved renderer delays the polls as much as the panel
        appeared = await page.waitForFunction((sel) => document.querySelector(sel).getClientRects().length > 0, sel,
          { timeout: REVEAL_MS, polling: 100 }).then(() => true, () => false);
        // Alpine knows whether the tab selected the panel, whatever the renderer has drawn: its x-show expression is
        // true for a panel that was chosen and not yet shown, false for one the click did not select
        state = await page.evaluate((appeared) => {
          const panel = document.querySelector('[data-mj-panel="1"]');
          panel.removeAttribute('data-mj-panel');
          const shown = panel.getAttribute('x-show');
          if (appeared || !shown || !window.Alpine) return null;
          try { return window.Alpine.evaluate(panel, shown) ? 'shown' : 'hidden'; } catch (e) { return null; }
        }, appeared);
      }
      const m = await page.evaluate((sel) => {
        const el = document.querySelector(sel);
        const visible = el.getClientRects().length > 0;
        let box = el;
        for (let p = el; p; p = p.parentElement) {
          const s = getComputedStyle(p);
          if (/(auto|scroll)/.test(s.overflowX + s.overflowY)) { box = p; break; }
        }
        box.setAttribute('data-mj-box', '1');
        el.focus();
        const label = `${el.tagName.toLowerCase()}${el.className ? '.' + String(el.className).trim().split(/\s+/)[0] : ''}`;
        return { visible, focused: document.activeElement === el, label,
          h: box.scrollWidth > box.clientWidth + 1 && /(auto|scroll)/.test(getComputedStyle(box).overflowX),
          v: box.scrollHeight > box.clientHeight + 1 && /(auto|scroll)/.test(getComputedStyle(box).overflowY) };
      }, sel);
      if (!m.visible && reveal.tab) {
        const why = state === 'shown' ? ": the page's state says the panel is shown, so it was not drawn in that time"
          : state === 'hidden' ? ": the page's state says the panel is still hidden, so choosing the tab did not select it" : '';
        fail(`a ${m.label} scroll region is in a tab panel; its tab was chosen and the panel did not appear in ${REVEAL_MS / 1000} s${why}`);
      } else if (!m.visible) fail(`a ${m.label} scroll region is hidden${reveal.label ? ` by ${reveal.label}` : ''} and nothing on the page reveals it`);
      else if (!m.focused) fail(`a visible ${m.label} scroll region with tabindex="0" does not take focus`);
      else {
        for (const [axis, key, prop] of [['h', 'ArrowRight', 'scrollLeft'], ['v', 'ArrowDown', 'scrollTop']]) {
          if (!m[axis]) continue;
          await settle(page);
          await mark(page, prop);
          await page.locator(sel).focus();
          await page.keyboard.press(key); await page.keyboard.press(key);
          // the keys scroll on the renderer's frames, which a loaded runner delays by seconds: the wait polls on an
          // interval, since polling on those same frames sees a region that has scrolled as one that has not
          const moved = await page.waitForFunction((prop) => document.querySelector('[data-mj-box="1"]')[prop] > 0, prop,
            { timeout: 10000, polling: 100 }).then(() => true, () => false);
          if (!moved) fail(`a ${m.label} scroll region overflows ${axis === 'h' ? 'sideways' : 'downwards'} and ${key} does not scroll it`);
        }
      }
      await page.evaluate(() => { const b = document.querySelector('[data-mj-box="1"]'); if (b) b.removeAttribute('data-mj-box'); });
      n++;
    }
    return n;
  },
};
