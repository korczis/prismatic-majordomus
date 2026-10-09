// The evidence figure (ADR 0122, rule project.figures-are-maps-of-evidence).
//
// The server draws the whole figure: the boxes, the lines, the legend, the subtrees as
// native <details>, and the data table under it. A reader with no script has all of it.
// This module adds what only a script can:
//
//   choosing      a box or a line (pointer, finger, Enter or Space) shows its explanation,
//                 kept inert in a <template> beside it, in the information box under the
//                 drawing; the choice is pressed (aria-pressed) and announced (aria-live);
//   tracing       what is joined to the chosen element stays lit and the rest steps back,
//                 so a fan of lines reads as the one path the reader asked about;
//   opening       choosing a box that has members opens its subtree; the two buttons open
//                 or close every subtree, every level;
//   addressing    the choice is the URL's fragment (#<figure>:<key>), so a reload, the
//                 back button and a link sent to somebody restore it;
//   filtering     each legend entry is a switch for its claim: the boxes and lines that
//                 make it leave the drawing, and so does a box left with no line, so a
//                 reader can look at what is recorded with the inferences taken out;
//   leaving       Escape, or a click on the empty drawing, clears the choice.
//
// Nothing here computes a fact. Every word the information box shows was rendered by the
// server from a capability's answer.

import { motionAllowed } from './cockpit.js';

for (const figure of document.querySelectorAll('[data-mj-figure]')) {
  enhance(figure);
}

function enhance(figure) {
  const id = figure.dataset.mjFigure;
  const info = figure.querySelector('[data-mj-figure-info]');
  const notes = new Map(
    [...figure.querySelectorAll('template[data-mj-note]')].map((t) => [t.dataset.mjNote, t]),
  );
  const elements = [...figure.querySelectorAll('svg [data-k]')];
  const byKey = new Map(elements.map((e) => [e.dataset.k, e]));
  const edges = elements.filter((e) => e.dataset.from !== undefined);
  for (const hidden of figure.querySelectorAll('[data-mj-js]')) hidden.hidden = false;
  const empty = info ? info.innerHTML : '';

  function related(key) {
    const lit = new Set([key]);
    const chosen = byKey.get(key);
    if (chosen && chosen.dataset.from !== undefined) {
      lit.add(chosen.dataset.from);
      lit.add(chosen.dataset.to);
    } else {
      for (const e of edges) {
        if (e.dataset.from === key || e.dataset.to === key) {
          lit.add(e.dataset.k);
          lit.add(e.dataset.from);
          lit.add(e.dataset.to);
        }
      }
    }
    return lit;
  }

  function choose(key, { remember = true } = {}) {
    const lit = key ? related(key) : new Set();
    figure.classList.toggle('has-choice', Boolean(key));
    for (const e of elements) {
      e.setAttribute('aria-pressed', String(e.dataset.k === key));
      e.classList.toggle('is-lit', lit.has(e.dataset.k));
    }
    if (info) {
      const note = key && notes.get(key);
      if (note) info.replaceChildren(note.content.cloneNode(true));
      else info.innerHTML = empty;
    }
    if (key) {
      const sub = figure.querySelector(`details[data-mj-sub="${CSS.escape(key)}"]`);
      if (sub) sub.open = true;
    }
    if (remember) {
      const fragment = key ? `#${id}:${key}` : window.location.pathname + window.location.search;
      window.history.replaceState(null, '', fragment);
    }
  }

  for (const e of elements) {
    e.addEventListener('click', () => choose(e.dataset.k));
    e.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        choose(e.dataset.k);
      }
    });
  }
  figure.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') choose(null);
  });
  const canvas = figure.querySelector('.mj-figure-canvas');
  if (canvas) {
    canvas.addEventListener('click', (event) => {
      if (!event.target.closest('[data-k]')) choose(null);
    });
  }

  // the legend as switches: one per claim the drawing makes
  const off = new Set();
  function filter() {
    for (const e of elements) e.classList.toggle('is-off', off.has(e.dataset.claim));
    // a box with lines, all of them off, leaves too; the subject never does
    for (const node of elements.filter((e) => e.dataset.from === undefined)) {
      const lines = edges.filter((e) => e.dataset.from === node.dataset.k || e.dataset.to === node.dataset.k);
      const stranded = lines.length > 0 && lines.every((e) => e.classList.contains('is-off'));
      if (stranded && !node.classList.contains('mj-flow-node--focus')) node.classList.add('is-off');
    }
  }
  for (const key of figure.querySelectorAll('[data-mj-claim]')) {
    key.setAttribute('role', 'button');
    key.setAttribute('tabindex', '0');
    key.setAttribute('aria-pressed', 'true');
    key.title = 'Show or hide what makes this claim';
    const toggle = () => {
      const claim = key.dataset.mjClaim;
      if (off.has(claim)) off.delete(claim);
      else off.add(claim);
      key.setAttribute('aria-pressed', String(!off.has(claim)));
      filter();
    };
    key.addEventListener('click', toggle);
    key.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        toggle();
      }
    });
  }

  // a chip is part of its box: choosing the box opens the subtree, and the subtree is
  // brought into view, because it opens below the drawing and may be off screen
  for (const chip of figure.querySelectorAll('svg [data-mj-sub]')) {
    chip.addEventListener('click', () => {
      const sub = figure.querySelector(`details[data-mj-sub="${CSS.escape(chip.dataset.mjSub)}"]`);
      if (!sub) return;
      sub.open = true;
      sub.scrollIntoView({ block: 'nearest', behavior: motionAllowed() ? 'smooth' : 'auto' });
    });
  }

  for (const button of figure.querySelectorAll('[data-mj-figure-all]')) {
    button.addEventListener('click', () => {
      const open = button.dataset.mjFigureAll === 'open';
      for (const d of figure.querySelectorAll('.mj-figure-subtrees details')) d.open = open;
    });
  }

  const restore = () => {
    const fragment = decodeURIComponent(window.location.hash.slice(1));
    const cut = fragment.indexOf(':');
    if (cut < 0 || fragment.slice(0, cut) !== id) return;
    const key = fragment.slice(cut + 1);
    if (byKey.has(key)) choose(key, { remember: false });
  };
  window.addEventListener('hashchange', restore);
  restore();
}
