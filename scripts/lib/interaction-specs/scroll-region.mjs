// A keyboard-reachable scroll region (a wide table, a long listing): a reader can reach it, it takes focus, and when
// its content overflows the arrow keys scroll it: horizontally with ArrowRight, vertically with ArrowDown. The box
// that scrolls is the region itself or the nearest ancestor that clips it (a <pre> inside its scrolling wrapper).
// A region inside a hidden tab panel is revealed the way a reader would reveal it, by choosing that tab, and the
// reveal is confirmed before the region is judged. Three things can be wrong and each is its own finding: nothing on
// the page reveals the region; its tab was chosen, twice, and the panel stayed hidden; it has no box and nothing hides it.
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
// A kit tab strip is rendered with every panel shown and is wired by site/kit.js, which marks it data-tabs-live as it
// hides the panels that are not selected. A strip the script has not reached yet is not a page that cannot reveal its
// panel, so every strip that has a tablist is let become live first, bounded: one that never does is judged as it is
const WAIT_MS = 10000;
const stripsLive = (page) => page.waitForFunction(() => [...document.querySelectorAll('[data-tabs]')]
  .every((s) => !s.querySelector('[role="tablist"]') || s.hasAttribute('data-tabs-live')), null, { timeout: WAIT_MS, polling: 100 }).catch(() => {});
// x-show shows a panel on the next animation frame, not in the click, and a loaded runner can hold that frame back
// for seconds: the bound is generous, because it costs nothing when the panel appears and only a region that stays
// hidden waits it out. The wait polls on an interval, not on animation frames (Playwright's default), because a
// starved renderer delays the polls as much as the panel
const rendered = (page, sel) => page.waitForFunction((sel) => document.querySelector(sel).getClientRects().length > 0, sel,
  { timeout: WAIT_MS, polling: 100 }).then(() => true, () => false);
export default {
  id: 'scroll-region',
  title: 'a scroll region can be reached, takes focus, and scrolls from the keyboard when it overflows',
  width: 1280,
  claims: (el) => el.attrs.tabindex === '0' && el.tag !== 'a' && !['button', 'input', 'select', 'textarea', 'summary'].includes(el.tag)
    && !Object.keys(el.attrs).some((k) => k.startsWith('x-on:') || k.startsWith('@') || k === 'x-model' || k.startsWith('data-graph-') || OTHER.test(k))
    && !['tab', 'switch', 'checkbox', 'radio', 'menuitem', 'slider', 'option'].includes(el.attrs.role),
  async exercise({ controls, page, route, fail }) {
    let n = 0;
    await stripsLive(page);
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
        tab.setAttribute('data-mj-reveal', '1'); hidden.setAttribute('data-mj-hidden', '1');
        // the document scrolls smoothly (html.scroll-smooth), and the focus and keys of the region before this one can
        // still be moving it: the tab is brought into view at once, which ends that scroll, so the pointer finds it still
        tab.scrollIntoView({ block: 'center', behavior: 'instant' });
        return { tab: tab.id || JSON.stringify(tab.textContent.trim().slice(0, 40)) };
      }, sel);
      let chosen = null;
      if (reveal.tab) {
        const tab = page.locator('[data-mj-reveal="1"]');
        await tab.click();
        // A click that returns is not a click that chose the tab: the driver checks what is under the pointer at the
        // press only, so on a page that moves before the release the click event goes to another element and the tab
        // never hears it (disclosure.mjs measured the same of a summary on a saturated runner). So the panel is waited
        // for, and when it has not rendered the tab is chosen once more as a keyboard reader chooses it, which depends
        // on no coordinates; a note on stderr keeps a runner that needs the second attempt visible
        if (!(await rendered(page, sel))) {
          process.stderr.write(`interaction-probe: ${route}: tab ${reveal.tab} was clicked and its panel did not render in ${WAIT_MS} ms; choosing it once more from the keyboard\n`);
          await tab.focus();
          await page.keyboard.press('Enter');
          await rendered(page, sel);
        }
        // what the tab and the panel say for themselves is read while they are still marked, for the finding
        chosen = await page.evaluate(() => {
          const t = document.querySelector('[data-mj-reveal="1"]'), p = document.querySelector('[data-mj-hidden="1"]');
          t.removeAttribute('data-mj-reveal'); p.removeAttribute('data-mj-hidden');
          return { selected: t.getAttribute('aria-selected'), hidden: p.hidden, display: getComputedStyle(p).display };
        });
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
      if (!m.visible && chosen) fail(`a ${m.label} scroll region is hidden: its tab ${reveal.tab} was chosen and the panel stayed hidden (the tab's aria-selected="${chosen.selected}", panel.hidden=${chosen.hidden}, its display is ${chosen.display})`);
      else if (!m.visible && reveal.label) fail(`a ${m.label} scroll region is hidden by ${reveal.label} and nothing on the page reveals it`);
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
