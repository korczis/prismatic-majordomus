// A keyboard-reachable scroll region (a wide table, a long listing): a reader can reach it, it takes focus, and when
// its content overflows the arrow keys scroll it: horizontally with ArrowRight, vertically with ArrowDown. The box
// that scrolls is the region itself or the nearest ancestor that clips it (a <pre> inside its scrolling wrapper).
// A region inside a hidden tab panel is revealed the way a reader would reveal it, by choosing that tab with the
// pointer, panel by panel from the outermost inwards, and each reveal is confirmed before the region is judged. What
// can be wrong is told apart, each its own finding: nothing on the page reveals the region; the click event did not
// reach its tab, twice; the tab heard the click and its panel stayed hidden; the region has no box and nothing hides it.
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
// x-show shows a panel on the next animation frame, not in the click, and a loaded runner can hold that frame back
// for seconds: the bound is generous, because it costs nothing when the panel appears and only a panel that stays
// hidden waits it out, once (the regions it holds after the first are not made to wait again). The wait polls on an
// interval, not on animation frames (Playwright's default), because a starved renderer delays the polls as much as
// the panel
const WAIT_MS = 10000;
const DEPTH = 8;
const shown = (page) => page.waitForFunction(() => getComputedStyle(document.querySelector('[data-mj-hidden="1"]')).display !== 'none', null,
  { timeout: WAIT_MS, polling: 100 }).then(() => true, () => false);
// The tab is brought into view at once, which ends a smooth scroll of the document still in flight (html.scroll-smooth;
// the focus and keys of the region before this one start one), and is made to say whether the next click event reaches it
const arm = (page) => page.evaluate(() => {
  const t = document.querySelector('[data-mj-reveal="1"]');
  t.scrollIntoView({ block: 'center', behavior: 'instant' });
  t.__mjHeard = false;
  if (!t.__mjArmed) { t.__mjArmed = true; t.addEventListener('click', () => { t.__mjHeard = true; }, true); }
});
export default {
  id: 'scroll-region',
  title: 'a scroll region can be reached, takes focus, and scrolls from the keyboard when it overflows',
  width: 1280,
  claims: (el) => el.attrs.tabindex === '0' && el.tag !== 'a' && !['button', 'input', 'select', 'textarea', 'summary'].includes(el.tag)
    && !Object.keys(el.attrs).some((k) => k.startsWith('x-on:') || k.startsWith('@') || k === 'x-model' || k.startsWith('data-graph-') || OTHER.test(k))
    && !['tab', 'switch', 'checkbox', 'radio', 'menuitem', 'slider', 'option'].includes(el.attrs.role),
  async exercise({ controls, page, route, fail }) {
    let n = 0;
    for (const c of controls) {
      const sel = `[data-mj-control="${c.n}"]`;
      // reveal: open enclosing disclosures, and choose the tab of each panel that hides the region, outermost first
      let blocked = null;
      for (let depth = 0; depth < DEPTH && !blocked; depth++) {
        const reveal = await page.evaluate((sel) => {
          const el = document.querySelector(sel);
          for (let d = el.closest('details'); d; d = d.parentElement && d.parentElement.closest('details')) d.open = true;
          let hidden = null;
          for (let p = el; p; p = p.parentElement) if (getComputedStyle(p).display === 'none') hidden = p;
          if (!hidden) return null;
          if (hidden.__mjBlocked) return { blocked: hidden.__mjBlocked };
          const scope = hidden.parentElement && hidden.parentElement.closest('[x-data]');
          let tab = hidden.id ? document.querySelector(`[role="tab"][aria-controls="${hidden.id}"]`) : null;
          if (!tab && scope) {
            const panels = [...scope.children].filter((k) => k.getAttribute('role') !== 'tablist' && k.hasAttribute('x-show'));
            const tabs = [...scope.querySelectorAll('[role="tab"]')];
            const i = panels.indexOf(hidden);
            if (i >= 0 && tabs[i]) tab = tabs[i];
          }
          const label = `${hidden.tagName.toLowerCase()}${hidden.getAttribute('x-show') ? '[x-show]' : ''}`;
          if (!tab) return { blocked: `is hidden by ${label} and nothing on the page reveals it` };
          tab.setAttribute('data-mj-reveal', '1'); hidden.setAttribute('data-mj-hidden', '1');
          return { tab: tab.id || JSON.stringify(tab.textContent.trim().slice(0, 40)) };
        }, sel);
        if (!reveal) break;
        if (reveal.blocked) { blocked = reveal.blocked; break; }
        // A click that returns is not a click the tab heard: the event goes to whatever is under the pointer when the
        // button is released, and disclosure.mjs measured a summary missing its click on a saturated runner. Whether
        // that is what failed on CI here is not known, so the tab is asked. A click it did not hear is the driver's
        // miss, not the page's defect: it is noted on stderr and made once more with the pointer. A click it did hear
        // is never repeated and never replaced by a key: a tab that hears the pointer and reveals nothing is the finding
        const tab = page.locator('[data-mj-reveal="1"]');
        let heard = false;
        for (let attempt = 1; attempt <= 2 && !heard; attempt++) {
          await arm(page);
          await tab.click();
          heard = await page.evaluate(() => document.querySelector('[data-mj-reveal="1"]').__mjHeard);
          if (!heard && attempt === 1) process.stderr.write(`interaction-probe: ${route}: [scroll-region] tab ${reveal.tab} was clicked and the click event did not reach it; clicking it once more\n`);
        }
        const out = heard && await shown(page);
        // what the tab and the panel say for themselves is read while they are still marked, for the finding, and a
        // panel that stayed hidden keeps the verdict for the other regions it holds
        blocked = await page.evaluate(({ out, heard, name }) => {
          const t = document.querySelector('[data-mj-reveal="1"]'), p = document.querySelector('[data-mj-hidden="1"]');
          t.removeAttribute('data-mj-reveal'); p.removeAttribute('data-mj-hidden');
          if (out) return null;
          p.__mjBlocked = heard
            ? `is hidden: its tab ${name} was chosen and the panel stayed hidden (the tab's aria-selected="${t.getAttribute('aria-selected')}", panel.hidden=${p.hidden}, its display is ${getComputedStyle(p).display})`
            : `is hidden: its tab ${name} was clicked twice and the click event reached it neither time`;
          return p.__mjBlocked;
        }, { out, heard, name: reveal.tab });
      }
      const m = await page.evaluate((sel) => {
        const el = document.querySelector(sel);
        const visible = el.getClientRects().length > 0;
        let box = el, none = false;
        for (let p = el; p; p = p.parentElement) if (getComputedStyle(p).display === 'none') none = true;
        for (let p = el; p; p = p.parentElement) {
          const s = getComputedStyle(p);
          if (/(auto|scroll)/.test(s.overflowX + s.overflowY)) { box = p; break; }
        }
        box.setAttribute('data-mj-box', '1');
        el.focus();
        const label = `${el.tagName.toLowerCase()}${el.className ? '.' + String(el.className).trim().split(/\s+/)[0] : ''}`;
        return { visible, none, focused: document.activeElement === el, label,
          h: box.scrollWidth > box.clientWidth + 1 && /(auto|scroll)/.test(getComputedStyle(box).overflowX),
          v: box.scrollHeight > box.clientHeight + 1 && /(auto|scroll)/.test(getComputedStyle(box).overflowY) };
      }, sel);
      if (!m.visible && blocked) fail(`a ${m.label} scroll region ${blocked}`);
      else if (!m.visible && m.none) fail(`a ${m.label} scroll region is still hidden after ${DEPTH} panels around it were revealed`);
      else if (!m.visible) fail(`a ${m.label} scroll region has no box, though neither it nor an ancestor of it is display:none`);
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
