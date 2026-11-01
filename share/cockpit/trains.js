// The trains viewer (ADR 0138). The trains come from /api/v1/pull-requests/trains, the
// projection of `integration.trains`; this file translates that answer into Cytoscape's
// shape at the boundary and nowhere else.
//
// It is drawn in the manner of the Neo4j Browser, because that is the picture the owner
// asked for: round nodes with their caption inside, coloured by what they are, labelled
// directed relationships between them, a force layout that can be dragged, zoomed and
// panned, the neighbours of the node under the pointer lit, and a click that opens every
// property of a node in a panel.
//
// Progressive enhancement in the strict sense: the page already lists every train as a
// table of its links with every fact this drawing shows. If Cytoscape is not vendored, or
// fails, the tables stay and nothing is lost.
//
// What this file decides, and what it does not:
//
//   the colours     are not decided here. A disposition's colour is one of the status or
//                   tone roles share/design/tokens.yaml declares, named below by its token;
//                   this file only decides which role a disposition wears. A node is filled
//                   with the role's ground and ringed with its strong colour, and its caption
//                   is the page's own text colour, which is the pairing the declaration
//                   measures for contrast.
//   the vocabulary  is the capability's: dispositions, relation words and prepared-head
//                   states are shown as the answer spells them.
//   the refresh     is a poll of the same HTTP route, every `data-mj-trains-poll` seconds,
//                   paused while the tab is hidden. No stream is invented for it. A link that
//                   is new, or whose disposition moved, glows once in the info role, so a
//                   reader sees what the last answer changed without diffing the picture.
//   the motion      is the declaration's: every duration is a multiple of `--mj-motion-fast`,
//                   and a reader who asked for stillness gets none (`motion()` is zero).
//   the emphasis    is one colour, the accent, and only for interaction: the halo under the
//                   node the pointer is on, the selection's ring, the search's matches and
//                   the relations lit around them. Everything else steps back.

import { api, vendor, explainMissing, motionAllowed } from './cockpit.js';

/**
 * Which declared role each disposition wears: `fill` is the ground inside the circle and
 * `ring` the strong colour around it. `landed`, `closed_unmerged` and `unread` are the
 * states of a pull request beneath a train, which is no longer open.
 */
const ROLES = {
  ready: { fill: '--mj-ok-bg', ring: '--mj-ok' },
  waiting_for_checks: { fill: '--mj-info-bg', ring: '--mj-info' },
  needs_refresh: { fill: '--mj-tone-cyan-bg', ring: '--mj-tone-cyan' },
  waiting_for_review: { fill: '--mj-tone-blue-bg', ring: '--mj-tone-blue' },
  waiting_for_dependency: { fill: '--mj-tone-blue-bg', ring: '--mj-tone-blue' },
  draft: { fill: '--mj-neutral-bg', ring: '--mj-neutral' },
  needs_repair: { fill: '--mj-bad-bg', ring: '--mj-bad' },
  conflicting: { fill: '--mj-tone-rose-bg', ring: '--mj-tone-rose' },
  blocked: { fill: '--mj-tone-orange-bg', ring: '--mj-tone-orange' },
  unsafe: { fill: '--mj-tone-orange-bg', ring: '--mj-tone-orange' },
  other_base: { fill: '--mj-tone-orange-bg', ring: '--mj-tone-orange' },
  unknown: { fill: '--mj-neutral-bg', ring: '--mj-neutral' },
  redundant: { fill: '--mj-warn-bg', ring: '--mj-warn' },
  superseded: { fill: '--mj-warn-bg', ring: '--mj-warn' },
  possibly_redundant: { fill: '--mj-warn-bg', ring: '--mj-warn' },
  obsolete: { fill: '--mj-warn-bg', ring: '--mj-warn' },
  landed: { fill: '--mj-tone-violet-bg', ring: '--mj-tone-violet' },
  closed_unmerged: { fill: '--mj-neutral-bg', ring: '--mj-neutral' },
  unread: { fill: '--mj-neutral-bg', ring: '--mj-neutral' },
};
const MASTER_ROLE = { fill: '--mj-accent-soft', ring: '--mj-accent' };

/** A title in a circle is a caption, not a sentence: this many characters, then an ellipsis. */
const CAPTION_CHARS = 16;

/** How far Shift and an arrow key move the drawing, in screen pixels (site/diagrams.js's step). */
const PAN_STEP = 48;

/**
 * Above this many nodes the drawing stops animating its state changes and simplifies itself
 * while it moves: a train of that size is not one a reader follows by its transitions.
 */
const LARGE = 400;

/**
 * The closest a fit comes. A fit to one link, or to a train of two, would otherwise fill the
 * frame with a single circle; past this the reader zooms on purpose.
 */
const FIT_ZOOM = 1.5;

/** The closest a reader can zoom on purpose. */
const MAX_ZOOM = 3;

const frame = document.querySelector('[data-mj-trains]');
if (frame) {
  start(frame).catch((error) => explainMissing(frame, error));
}

async function start(frame) {
  const [cytoscape, first] = await Promise.all([
    vendor('cytoscape.min.js', 'cytoscape'),
    api(frame.dataset.mjTrainsSrc),
  ]);
  if (!first.ok) throw new Error('the trains endpoint answered ' + first.status);

  const state = {
    view: first.body,
    text: JSON.stringify(first.body),
    links: new Map(),
    order: [],
    train: '',
    hidden: new Set(),
    needle: '',
    selected: '',
    layout: 'force',
    // whether the reader has zoomed or panned since the last fit: a resize refits only if not
    moved: false,
    // the timer that ends the glow of the links the last answer changed
    pulse: 0,
    // the poll's interval in milliseconds, set below: how long a still glow lasts
    period: 0,
    // whether the drawing is past LARGE nodes; every answer decides it again (`replace`)
    large: false,
    // how many layouts are holding the zoom at FIT_ZOOM while they fit (`runLayout`)
    capped: 0,
    // the overlays the drawing places on its frame, made below
    tip: null,
    empty: null,
    count: null,
  };
  const initial = elements(state);
  state.large = initial.filter((e) => !e.data.source).length > LARGE;
  const transitions = () => (state.large ? 0 : motion());

  const cy = cytoscape({
    container: frame,
    elements: initial,
    layout: { name: 'preset' },
    minZoom: 0.15,
    maxZoom: MAX_ZOOM,
    style: style(drawable(), typeScale(), transitions()),
    hideEdgesOnViewport: state.large,
    textureOnViewport: state.large,
  });
  runLayout(cy, state, true);

  // The hover card and the note a filter that leaves nothing shows. Both sit on the frame,
  // over the canvas Cytoscape put there; the card repeats what the panel says, so it is
  // hidden from assistive technology, which hears the panel.
  state.tip = document.createElement('div');
  state.tip.className = 'mj-graph-tip';
  state.tip.hidden = true;
  state.tip.setAttribute('aria-hidden', 'true');
  state.empty = document.createElement('p');
  state.empty.className = 'mj-graph-empty';
  state.empty.textContent = 'Nothing matches: every link is filtered out. The tables below still list them all.';
  state.empty.hidden = true;
  frame.append(state.tip, state.empty);

  // ------------------------------------------------------------------- the pointer
  cy.on('mouseover', 'node', (event) => {
    const node = event.target;
    frame.style.cursor = 'pointer';
    showTip(cy, state, node);
    // the selection outranks the pointer: what is lit stays lit until it is cleared
    if (state.selected) return;
    cy.batch(() => {
      cy.elements().addClass('mj-dim');
      node.closedNeighborhood().removeClass('mj-dim').addClass('mj-near');
      node.addClass('mj-point');
    });
  });
  cy.on('mouseout', 'node', () => {
    frame.style.cursor = '';
    hideTip(state);
    if (state.selected) return;
    restore(cy, state);
  });
  cy.on('tap', 'node', (event) => select(cy, state, event.target.id(), false));
  cy.on('dbltap', 'node', (event) => open(state, event.target.id()));
  cy.on('tap', (event) => {
    if (event.target === cy) clear(cy, state);
  });
  // a card left behind by a moving picture points at nothing
  cy.on('viewport grab', () => hideTip(state));
  cy.on('scrollzoom pinchzoom dragpan', () => {
    state.moved = true;
  });

  // ------------------------------------------------------------------ the keyboard
  frame.addEventListener('keydown', (event) => {
    // the browser's own shortcuts (zoom the page, find in it) stay the browser's
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const key = event.key;
    const visible = state.order.filter((id) => cy.getElementById(id).visible());
    const arrow = key === 'ArrowRight' || key === 'ArrowDown' || key === 'ArrowLeft' || key === 'ArrowUp';
    if (arrow && event.shiftKey) {
      // Shift moves the picture, not the selection; the step and its sense are diagrams.js's
      const dx = key === 'ArrowLeft' ? PAN_STEP : key === 'ArrowRight' ? -PAN_STEP : 0;
      const dy = key === 'ArrowUp' ? PAN_STEP : key === 'ArrowDown' ? -PAN_STEP : 0;
      cy.stop(true, false);
      cy.panBy({ x: dx, y: dy });
      state.moved = true;
    } else if (arrow) {
      if (!visible.length) return;
      const step = key === 'ArrowRight' || key === 'ArrowDown' ? 1 : -1;
      const at = visible.indexOf(state.selected);
      const next = at < 0 ? (step > 0 ? 0 : visible.length - 1) : (at + step + visible.length) % visible.length;
      select(cy, state, visible[next], true);
    } else if (key === 'Home' || key === 'End') {
      // the first and last link; master is where the arrows start, not a link
      const links = visible.filter((id) => id !== 'master');
      if (!links.length) return;
      select(cy, state, key === 'Home' ? links[0] : links[links.length - 1], true);
    } else if (key === 'Enter') {
      if (state.selected) open(state, state.selected);
    } else if (key === '+' || key === '=') {
      zoom(cy, state, 1.25);
    } else if (key === '-' || key === '_') {
      zoom(cy, state, 0.8);
    } else if (key === '0') {
      fit(cy, state);
    } else if (key === 'Escape') {
      hideTip(state);
      clear(cy, state);
    } else {
      return;
    }
    event.preventDefault();
  });
  // The note the frame is described by is the page's, written for the keys every version of
  // this file has bound; Home, End and Shift with an arrow are this file's, so it says them.
  const keysNote = (frame.getAttribute('aria-describedby') || '')
    .split(/\s+/)
    .map((id) => id && document.getElementById(id))
    .find(Boolean);
  if (keysNote && !keysNote.hasAttribute('data-mj-trains-keys-more')) {
    keysNote.setAttribute('data-mj-trains-keys-more', '');
    keysNote.append(' Home and End go to the first and last link; Shift and an arrow move the picture.');
  }

  // ------------------------------------------------------------------ the controls
  const on = (selector, kind, handler) => {
    const element = document.querySelector(selector);
    if (element) element.addEventListener(kind, handler);
    return element;
  };
  on('[data-mj-trains-zoom-in]', 'click', () => zoom(cy, state, 1.25));
  on('[data-mj-trains-zoom-out]', 'click', () => zoom(cy, state, 0.8));
  on('[data-mj-trains-fit]', 'click', () => fit(cy, state));
  // − + Fit as one segmented group, the order every drawing's zoom reads in; the page sends
  // them as separate buttons, so a page without this script keeps them as they were
  const zoomIn = document.querySelector('[data-mj-trains-zoom-in]');
  const zoomOut = document.querySelector('[data-mj-trains-zoom-out]');
  const fitButton = document.querySelector('[data-mj-trains-fit]');
  if (zoomIn && zoomOut && fitButton && zoomIn.parentNode) {
    const group = document.createElement('div');
    group.className = 'mj-graph-zoom';
    group.setAttribute('role', 'group');
    group.setAttribute('aria-label', 'Zoom');
    zoomIn.parentNode.insertBefore(group, zoomIn);
    group.append(zoomOut, zoomIn, fitButton);
  }
  const layoutButton = on('[data-mj-trains-layout]', 'click', () => {
    state.layout = state.layout === 'force' ? 'layered' : 'force';
    layoutButton.textContent = state.layout === 'force' ? 'Layered' : 'Force';
    runLayout(cy, state, true);
  });
  on('[data-mj-trains-train]', 'change', (event) => {
    state.train = event.target.value;
    applyFilters(cy, state);
    fit(cy, state);
  });
  // The search waits for a pause in the typing before it lights and refits, so the picture
  // does not lurch once per keystroke; Enter and Escape act on what is typed at once.
  let typing = 0;
  const find = () => {
    clearTimeout(typing);
    typing = 0;
    state.needle = search.value.trim().toLowerCase();
    // a search replaces the selection: what is lit is what matches
    state.selected = '';
    cy.elements().removeClass('mj-focus').removeClass('mj-near');
    applyFilters(cy, state);
    const matching = cy.nodes('.mj-match');
    if (matching.length) {
      fitTo(cy, matching, 48);
      state.moved = true;
    }
  };
  const search = on('[data-mj-trains-search]', 'input', () => {
    clearTimeout(typing);
    typing = setTimeout(find, 120);
  });
  if (search) {
    // how many links the search lit, out of how many are shown, said politely
    state.count = document.createElement('span');
    state.count.className = 'mj-graph-count';
    state.count.setAttribute('aria-live', 'polite');
    search.insertAdjacentElement('afterend', state.count);
    search.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') {
        if (!search.value) return;
        search.value = '';
        find();
        event.preventDefault();
        return;
      }
      if (event.key !== 'Enter') return;
      if (typing) find();
      const first = cy.nodes('.mj-match').first();
      if (first.length) {
        select(cy, state, first.id(), true);
        frame.focus();
      }
      event.preventDefault();
    });
  }

  const legendHost = legend(frame, cy, state);

  // --------------------------------------------------------------------- the poll
  const asked = document.querySelector('[data-mj-trains-asked]');
  const observed = document.querySelector('[data-mj-trains-observed]');
  const seconds = Math.max(5, parseInt(frame.dataset.mjTrainsPoll || '30', 10) || 30);
  state.period = seconds * 1000;
  const say = (message) => {
    if (asked) asked.textContent = message;
  };
  // one ask at a time: each answer decides every link's relation to master again, and a
  // second ask started while the first is out would only queue behind it on the server
  let asking = false;
  const poll = async () => {
    if (asking) return;
    asking = true;
    say('asking now …');
    let answer;
    try {
      answer = await api(frame.dataset.mjTrainsSrc);
    } catch (error) {
      say('the last ask failed (' + error.message + '); asking again in ' + seconds + ' s');
      return;
    } finally {
      asking = false;
    }
    const time = new Date().toLocaleTimeString();
    if (!answer.ok) {
      say('the last ask, at ' + time + ', was answered ' + answer.status + '; asking again in ' + seconds + ' s');
      return;
    }
    const message = 'at ' + time + ', and again every ' + seconds + ' s while this tab is visible';
    say(message);
    if (observed && answer.body.observed_at) observed.textContent = answer.body.observed_at;
    const text = JSON.stringify(answer.body);
    if (text === state.text) return;
    state.view = answer.body;
    state.text = text;
    const changed = replace(cy, state, transitions);
    legend(frame, cy, state, legendHost);
    if (changed) say(message + '; ' + changed + ' link(s) changed');
  };
  on('[data-mj-trains-refresh]', 'click', poll);
  say('when this page was rendered; asking again every ' + seconds + ' s while this tab is visible');
  setInterval(() => {
    if (document.visibilityState === 'visible') poll();
  }, seconds * 1000);

  // ------------------------------------------------------------- theme and resize
  // The theme is a class on <html>, set by the page (theme.js); this file never writes it.
  // When it changes, every colour is read again from the declaration and the stylesheet is
  // replaced in place: nothing is laid out again and no entry motion is replayed.
  const root = document.documentElement;
  const ground = () => getComputedStyle(root).getPropertyValue('--mj-bg').trim();
  let theme = ground();
  new MutationObserver(() => {
    const now = ground();
    if (now === theme) return;
    theme = now;
    hideTip(state);
    cy.style(style(drawable(), typeScale(), transitions()));
  }).observe(root, { attributes: true, attributeFilter: ['class'] });
  // The frame flexes with the panel beside it. Cytoscape measures its container only on a
  // window resize, so it is told; and the picture is refitted only while the reader has not
  // zoomed or panned it since the last fit. The first observation is the frame's first size.
  if ('ResizeObserver' in window) {
    let seen = false;
    let queued = false;
    new ResizeObserver(() => {
      if (!seen) {
        seen = true;
        return;
      }
      if (queued) return;
      queued = true;
      requestAnimationFrame(() => {
        queued = false;
        cy.resize();
        // at once: a frame being resized is already moving
        if (!state.moved && state.empty.hidden) fitTo(cy, cy.elements(':visible'), 32, true);
      });
    }).observe(frame);
  }

  // What the drawing holds, for a probe that cannot read a canvas's meaning.
  frame.mjTrains = {
    cy,
    nodes: () => cy.nodes().map((n) => n.id()),
    select: (id) => select(cy, state, id, true),
    selected: () => state.selected,
    poll,
  };
  frame.dataset.mjTrainsReady = 'true';
}

// ------------------------------------------------------------------------ the answer

/** The one translation of the capability's answer into the drawing library's elements. */
function elements(state) {
  const view = state.view || {};
  const out = [];
  state.links = new Map();
  state.order = [];
  if (!view.observed) return out;
  const sha = (view.master_sha || '').slice(0, 7);
  out.push({
    data: { id: 'master', kind: 'master', caption: (view.base || 'master') + '\n' + sha, trains: [] },
  });
  state.links.set('master', { kind: 'master', view });
  state.order.push('master');
  const beneath = new Map();
  for (const train of view.trains || []) {
    for (const link of train.links) {
      const id = 'pr-' + link.number;
      state.links.set(id, { kind: 'link', link, train });
      state.order.push(id);
      out.push({
        data: {
          id,
          kind: 'link',
          number: link.number,
          disposition: link.disposition,
          caption: '#' + link.number + '\n' + caption(link.title),
          haystack: ('#' + link.number + ' ' + link.title + ' ' + link.head_ref).toLowerCase(),
          trains: [train.id],
          turn: link.at_turn ? 'yes' : 'no',
        },
      });
      if (!link.on.length) {
        out.push({
          data: { id: 'targets-' + link.number, source: id, target: 'master', label: 'TARGETS', kind: 'targets' },
        });
      }
    }
    for (const b of train.beneath) {
      const id = 'pr-' + b.number;
      const known = beneath.get(id);
      if (known) {
        known.data.trains.push(train.id);
        continue;
      }
      const fate = b.state === 'merged' ? 'landed' : b.state;
      const node = {
        data: {
          id,
          kind: 'beneath',
          number: b.number,
          disposition: fate,
          caption: '#' + b.number + '\n' + fate.replace('_', ' '),
          haystack: ('#' + b.number).toLowerCase(),
          trains: [train.id],
          turn: 'no',
        },
      };
      beneath.set(id, node);
      state.links.set(id, { kind: 'beneath', beneath: b, train });
    }
    for (const e of train.edges) {
      out.push({
        data: {
          id: 'edge-' + e.from + '-' + e.to,
          source: 'pr-' + e.from,
          target: 'pr-' + e.to,
          label: e.kind === 'stacked_on' ? 'STACKED_ON' : 'DEPENDS_ON',
          kind: e.kind,
          satisfied: e.satisfied ? 'yes' : 'no',
        },
      });
    }
  }
  for (const [id, node] of beneath) {
    out.push(node);
    state.order.push(id);
    if (node.data.disposition === 'landed') {
      out.push({
        data: { id: 'merged-' + node.data.number, source: id, target: 'master', label: 'MERGED_INTO', kind: 'merged' },
      });
    }
  }
  return out;
}

/** A title cut to a caption. */
function caption(title) {
  const words = String(title || '').replace(/^[a-z]+(\([^)]*\))?:\s*/, '');
  return words.length > CAPTION_CHARS ? words.slice(0, CAPTION_CHARS - 1) + '…' : words;
}

/**
 * Put a new answer in place of the old one, keeping where the reader left every node, and
 * mark the nodes the answer changed: new ones, and ones whose disposition moved. Returns how
 * many of them are train links, the count the tables would agree with; a pull request beneath
 * a train glows too but is not a link. `transitions` is the state-change duration, decided
 * again here because an answer can carry the drawing past LARGE nodes or back under it.
 */
function replace(cy, state, transitions) {
  const positions = new Map();
  const was = new Map();
  cy.nodes().forEach((n) => {
    positions.set(n.id(), { ...n.position() });
    was.set(n.id(), n.data('disposition'));
  });
  hideTip(state);
  const next = elements(state);
  cy.batch(() => {
    cy.elements().remove();
    cy.add(next);
  });
  const large = cy.nodes().length > LARGE;
  if (large !== state.large) {
    state.large = large;
    cy.style(style(drawable(), typeScale(), transitions()));
    // the library takes these two only at creation and has no setter; its renderer reads
    // its own copies on every frame, so those are what is set
    const renderer = cy.renderer && cy.renderer();
    if (renderer) {
      renderer.hideEdgesOnViewport = large;
      renderer.textureOnViewport = large;
    }
  }
  let unplaced = 0;
  const changed = [];
  cy.nodes().forEach((n) => {
    const at = positions.get(n.id());
    if (at) n.position(at);
    else unplaced += 1;
    if (n.data('kind') !== 'master' && (!was.has(n.id()) || was.get(n.id()) !== n.data('disposition'))) {
      changed.push(n);
    }
  });
  if (unplaced) runLayout(cy, state, false);
  applyFilters(cy, state);
  if (state.selected && cy.getElementById(state.selected).length) {
    select(cy, state, state.selected, false);
  } else {
    clear(cy, state);
  }
  pulse(cy, state, changed);
  return changed.filter((n) => n.data('kind') === 'link').length;
}

/**
 * The glow of what an answer changed: one class, taken off again by one timer, so nothing
 * runs while nothing changes. Taking a class off is not motion: for a reader who asked for
 * stillness the transitions are zero, so the glow simply goes, after one poll interval
 * rather than sixteen motion steps, which leaves it on long enough to be found.
 */
function pulse(cy, state, changed) {
  clearTimeout(state.pulse);
  state.pulse = 0;
  if (!changed.length) return;
  cy.collection(changed).addClass('mj-changed');
  const fast = motion();
  state.pulse = setTimeout(
    () => {
      state.pulse = 0;
      cy.nodes('.mj-changed').removeClass('mj-changed');
    },
    fast ? 16 * fast : state.period,
  );
}

// ------------------------------------------------------------------------ the layout

/**
 * The force layout is Cytoscape's own `cose`, the one built-in layout that behaves like the
 * Neo4j Browser's: linked nodes pull together, unlinked ones push apart, and each train
 * settles as its own cluster around master. `layered` is `breadthfirst` from master, for a
 * reader who wants the landing order as rows. Animation only for a reader who wants motion.
 */
function runLayout(cy, state, fitAfter) {
  const fast = motion();
  cy.stop(true, false);
  if (fitAfter) state.moved = false;
  const common = {
    animate: fast > 0,
    animationDuration: 3 * fast,
    animationEasing: 'ease-out-cubic',
    fit: fitAfter,
    padding: 32,
  };
  const options =
    state.layout === 'layered'
      ? { ...common, name: 'breadthfirst', directed: false, roots: '#master', spacingFactor: 1.25 }
      : {
          ...common,
          name: 'cose',
          randomize: false,
          numIter: 1200,
          nodeRepulsion: () => 14000,
          idealEdgeLength: () => 96,
          edgeElasticity: () => 80,
          gravity: 0.3,
          componentSpacing: 80,
          nodeOverlap: 12,
        };
  const layout = cy.elements(':visible').layout(options);
  if (fitAfter) {
    // The layout's own fit, no closer than FIT_ZOOM: the zoom is capped while it runs, so a
    // small train is not first fitted to the library's closest and then pulled back out.
    // Overlapping layouts each hold the cap; the last one to stop gives the reader it back.
    state.capped += 1;
    cy.maxZoom(FIT_ZOOM);
    layout.one('layoutstop', () => {
      state.capped -= 1;
      if (!state.capped) cy.maxZoom(MAX_ZOOM);
    });
  }
  layout.run();
}

/** Zoom about the middle of the frame, within the frame's bounds. */
function zoom(cy, state, factor) {
  const level = Math.min(cy.maxZoom(), Math.max(cy.minZoom(), cy.zoom() * factor));
  const about = { level, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } };
  const fast = motion();
  cy.stop(true, false);
  if (fast) cy.animate({ zoom: about, duration: fast, easing: 'ease-out-cubic' });
  else cy.zoom(about);
  state.moved = true;
}

/** Fit what is shown; a filter that leaves nothing shown leaves the view where it was. */
function fit(cy, state) {
  if (!state.empty.hidden) return;
  fitTo(cy, cy.elements(':visible'), 32);
  state.moved = false;
}

/**
 * Fit these elements, no closer than FIT_ZOOM, ending any viewport animation still running so
 * the last ask wins; `instant` for a caller that is already moving the picture.
 */
function fitTo(cy, eles, padding, instant) {
  if (!eles.length) return;
  cy.stop(true, false);
  const box = eles.boundingBox();
  const fits = Math.min((cy.width() - 2 * padding) / Math.max(box.w, 1), (cy.height() - 2 * padding) / Math.max(box.h, 1));
  const zoom = Math.max(cy.minZoom(), Math.min(cy.maxZoom(), FIT_ZOOM, fits));
  const pan = { x: cy.width() / 2 - (box.x1 + box.w / 2) * zoom, y: cy.height() / 2 - (box.y1 + box.h / 2) * zoom };
  const fast = instant ? 0 : motion();
  if (fast) cy.animate({ zoom, pan, duration: 2 * fast, easing: 'ease-out-cubic' });
  else cy.viewport({ zoom, pan });
}

/**
 * The declared motion step, `--mj-motion-fast`, in milliseconds; every duration here is a
 * multiple of it. Zero when the reader asked for reduced motion, so a caller that animates
 * by it does not animate at all.
 */
function motion() {
  if (!motionAllowed()) return 0;
  const raw = getComputedStyle(document.documentElement).getPropertyValue('--mj-motion-fast').trim();
  const value = parseFloat(raw);
  if (!Number.isFinite(value) || value <= 0) return 0;
  return /ms$/.test(raw) || !/s$/.test(raw) ? value : value * 1000;
}

// ------------------------------------------------------------------- filter and find

/** The train chosen, the dispositions hidden and the search, applied together. */
function applyFilters(cy, state) {
  let links = 0;
  let shownLinks = 0;
  let matches = 0;
  cy.batch(() => {
    cy.nodes().forEach((n) => {
      const kind = n.data('kind');
      const inTrain = !state.train || kind === 'master' || (n.data('trains') || []).includes(state.train);
      const shown = inTrain && (kind === 'master' || !state.hidden.has(n.data('disposition')));
      n.toggleClass('mj-hidden', !shown);
      const match = Boolean(state.needle) && shown && (n.data('haystack') || '').includes(state.needle);
      n.toggleClass('mj-match', match);
      if (kind === 'master') return;
      links += 1;
      if (shown) shownLinks += 1;
      if (match) matches += 1;
    });
    if (state.needle) {
      cy.elements().addClass('mj-dim');
      const matching = cy.nodes('.mj-match');
      matching.removeClass('mj-dim');
      matching.connectedEdges().removeClass('mj-dim');
    } else if (!state.selected) {
      cy.elements().removeClass('mj-dim');
    }
  });
  if (state.empty) state.empty.hidden = !(links && !shownLinks);
  if (state.count) state.count.textContent = state.needle ? matches + ' of ' + shownLinks : '';
}

/**
 * Back to what the filters and the selection say is lit, after the pointer has lit something
 * else for a moment (a node under it, a legend entry it is on).
 */
function restore(cy, state) {
  cy.elements().removeClass('mj-near').removeClass('mj-point');
  applyFilters(cy, state);
  const node = state.selected && cy.getElementById(state.selected);
  if (node && node.length) light(cy, node);
}

/** Light one node and its closed neighbourhood, and step everything else back. */
function light(cy, node) {
  cy.batch(() => {
    cy.elements().removeClass('mj-near').removeClass('mj-point').addClass('mj-dim').removeClass('mj-focus');
    node.closedNeighborhood().removeClass('mj-dim').addClass('mj-near');
    node.addClass('mj-focus');
  });
}

/**
 * While the pointer is on a legend entry: that disposition lit, every other one stepped back.
 * The selection's lighting steps back with it, as it does under the pointer, and `restore`
 * lights it again; a hidden disposition has nothing shown to light, so it previews nothing.
 */
function preview(cy, state, word) {
  if (state.hidden.has(word)) return;
  cy.batch(() => {
    const lit = cy.nodes().filter((n) => n.data('disposition') === word);
    cy.elements().removeClass('mj-near').removeClass('mj-focus').removeClass('mj-point').addClass('mj-dim');
    lit.removeClass('mj-dim');
    lit.edgesWith(lit).removeClass('mj-dim');
  });
}

/**
 * The legend, which is also the filter: one entry per disposition present, with its colour
 * and count, and a press that takes it out of the picture. Rebuilt in place on every answer.
 */
function legend(frame, cy, state, existing) {
  const host = existing || document.createElement('div');
  host.className = 'mj-graph-legend';
  host.setAttribute('data-mj-trains-legend', '');
  host.setAttribute('role', 'group');
  host.setAttribute('aria-label', 'Dispositions: press one to hide or show it');
  host.textContent = '';
  const counts = new Map();
  cy.nodes().forEach((n) => {
    const d = n.data('disposition');
    if (d) counts.set(d, (counts.get(d) || 0) + 1);
  });
  const words = [...counts.keys()].sort((a, b) => counts.get(b) - counts.get(a) || (a < b ? -1 : 1));
  for (const word of words) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'mj-graph-legend-entry';
    button.setAttribute('aria-pressed', state.hidden.has(word) ? 'true' : 'false');
    button.classList.toggle('is-hidden', state.hidden.has(word));
    button.title = word + ' — press to hide or show';
    const swatch = document.createElement('span');
    swatch.className = 'mj-graph-swatch';
    // the DOM reads the declaration's own value; only the canvas needs channels
    swatch.style.background = 'var(' + (ROLES[word] || ROLES.unknown).ring + ')';
    const name = document.createElement('span');
    name.className = 'mj-graph-legend-name';
    name.textContent = word;
    const count = document.createElement('span');
    count.className = 'mj-graph-legend-count';
    count.textContent = String(counts.get(word));
    button.append(swatch, name, count);
    // whether a pointer (not a finger) is resting on this entry, so it is previewing it
    let resting = false;
    button.addEventListener('click', () => {
      if (state.hidden.has(word)) state.hidden.delete(word);
      else state.hidden.add(word);
      button.setAttribute('aria-pressed', state.hidden.has(word) ? 'true' : 'false');
      button.classList.toggle('is-hidden', state.hidden.has(word));
      // what the filters and the selection say, and the preview again if the pointer is
      // still on the entry and it is shown (a hidden one previews nothing)
      restore(cy, state);
      if (resting) preview(cy, state, word);
    });
    // a pointer resting on an entry previews it: that disposition lit, the rest stepped back
    button.addEventListener('pointerenter', (event) => {
      if (event.pointerType === 'touch') return;
      resting = true;
      preview(cy, state, word);
    });
    button.addEventListener('pointerleave', () => {
      resting = false;
      restore(cy, state);
    });
    host.append(button);
  }
  // under the drawing and its panel, not between them
  if (!existing) (frame.parentElement || frame).insertAdjacentElement('afterend', host);
  return host;
}

// ------------------------------------------------------------------ select and show

function select(cy, state, id, centre) {
  const node = cy.getElementById(id);
  if (!node.length) return;
  state.selected = id;
  light(cy, node);
  if (centre) {
    cy.stop(true, false);
    const fast = motion();
    if (fast) cy.animate({ center: { eles: node }, duration: 2 * fast, easing: 'ease-out-cubic' });
    else cy.center(node);
  }
  describe(state, id);
}

function clear(cy, state) {
  state.selected = '';
  cy.elements().removeClass('mj-focus').removeClass('mj-near').removeClass('mj-point');
  applyFilters(cy, state);
  if (!state.needle) cy.elements().removeClass('mj-dim');
  const panel = document.querySelector('[data-mj-trains-panel]');
  if (panel) {
    panel.textContent = '';
    const p = document.createElement('p');
    p.className = 'mj-note';
    p.textContent = 'Select a link to see every field of it here.';
    panel.append(p);
  }
}

/** Open the forge's page of a pull request; master and nodes without a page open nothing. */
function open(state, id) {
  const entry = state.links.get(id);
  const url = entry && (entry.link ? entry.link.url : entry.beneath ? entry.beneath.url : null);
  if (url) window.open(url, '_blank', 'noopener');
}

/** The property panel: every field of the selected node, as the capability answered it. */
function describe(state, id) {
  const panel = document.querySelector('[data-mj-trains-panel]');
  const entry = state.links.get(id);
  if (!panel || !entry) return;
  panel.textContent = '';
  const card = document.createElement('div');
  card.className = 'mj-alert mj-alert--info';
  const heading = document.createElement('strong');
  const list = document.createElement('dl');
  list.className = 'mj-facts';
  const fact = (key, value) => {
    const dt = document.createElement('dt');
    dt.className = 'mj-facts-key';
    dt.textContent = key;
    const dd = document.createElement('dd');
    dd.className = 'mj-facts-value';
    if (value instanceof Node) dd.append(value);
    else dd.textContent = value === undefined || value === null || value === '' ? '—' : String(value);
    list.append(dt, dd);
  };
  const anchor = (href, text) => {
    const a = document.createElement('a');
    a.className = 'mj-link';
    a.href = href;
    a.textContent = text;
    if (/^https?:/.test(href)) a.rel = 'noopener';
    return a;
  };
  const code = (text) => {
    const c = document.createElement('code');
    c.className = 'mj-mono';
    c.textContent = text;
    return c;
  };
  const numbers = (ns) => (ns && ns.length ? ns.map((n) => '#' + n).join(' ') : '');

  if (entry.kind === 'master') {
    const v = entry.view;
    heading.textContent = v.base || 'master';
    fact('Kind', 'the integration base every train lands on');
    fact('Commit', code(v.master_sha || ''));
    fact('Repository', v.repository);
    fact('Observed', v.observed_at);
    fact('Trains', String((v.trains || []).length));
  } else if (entry.kind === 'beneath') {
    const b = entry.beneath;
    heading.textContent = '#' + b.number;
    fact('Kind', 'beneath ' + entry.train.id + ', no longer open');
    fact('State', b.state);
    fact('Pull request', b.url ? anchor(b.url, b.url) : '');
  } else {
    const l = entry.link;
    heading.textContent = '#' + l.number + ' ' + l.title;
    fact('Train', entry.train.id + ' — turn ' + l.position + ' of ' + entry.train.links.length);
    fact('At its turn', l.at_turn ? 'yes: every link below it has landed' : 'no');
    fact('Disposition', l.disposition + ' (' + l.lane + ')');
    fact('Reasons', l.reasons.join(', '));
    fact('Next action', l.next_action);
    fact('Required checks', l.required_checks);
    fact('Checks', l.checks.map((c) => c.name + ': ' + c.state).join(', '));
    const relation = l.relation_kind + (l.behind !== null && l.behind !== undefined ? ' — ' + l.behind + ' behind' : '');
    fact('Relation to master', relation + (l.carries_master ? ', carries master' : ''));
    fact(
      'Prepared head',
      l.prep ? l.prep.reference + ' ' + l.prep.sha.slice(0, 10) + ' — ' + l.prep.state : 'none',
    );
    fact('Head', code(l.head_sha));
    fact('Branch', code(l.head_ref + ' → ' + l.base_ref));
    fact('Stands on', numbers(l.on) || 'master');
    fact('Carries', numbers(l.carries) || 'nothing');
    fact('Depth', String(l.depth));
    fact('Queue rank', String(l.rank));
    fact('In a cycle', l.in_cycle ? 'yes' : 'no');
    fact('Author', l.author);
    const links = document.createElement('span');
    if (l.url) links.append(anchor(l.url, 'the pull request'));
    if (l.checks_url) links.append(' · ', anchor(l.checks_url, 'its checks'));
    links.append(' · ', anchor('/cockpit/capabilities/integration.explain?number=' + l.number, 'explain'));
    fact('Open', links);
  }
  card.append(heading, list);
  panel.append(card);
}

// ------------------------------------------------------------------- the hover card

/** A screen without hover has no pointer to rest on a node; it gets the panel and no card. */
function coarse() {
  return window.matchMedia('(pointer: coarse)').matches;
}

/**
 * The card beside the node under the pointer: what it is, its disposition as a badge in the
 * node's own role, and where it stands in its train. It says less than the panel, which a tap
 * fills; it is there so a reader can sweep the picture without clicking.
 */
function showTip(cy, state, node) {
  const tip = state.tip;
  const entry = state.links.get(node.id());
  if (!tip || !entry || coarse()) return;
  tip.textContent = '';
  const line = (className, text) => {
    const span = document.createElement('span');
    span.className = className;
    span.textContent = text;
    return span;
  };
  const row = document.createElement('span');
  row.className = 'mj-graph-tip-row';
  let url = null;
  if (entry.kind === 'master') {
    const v = entry.view;
    tip.append(line('mj-graph-tip-title', (v.base || 'master') + ' ' + (v.master_sha || '').slice(0, 7)));
    const trains = (v.trains || []).length;
    tip.append(line('mj-graph-tip-meta', 'the base every train lands on · ' + trains + ' train(s)'));
  } else if (entry.kind === 'beneath') {
    const b = entry.beneath;
    url = b.url;
    tip.append(line('mj-graph-tip-title', '#' + b.number));
    row.append(badge(node.data('disposition')));
    tip.append(row, line('mj-graph-tip-meta', 'beneath ' + entry.train.id + ', no longer open'));
  } else {
    const l = entry.link;
    url = l.url;
    tip.append(line('mj-graph-tip-title', '#' + l.number + ' ' + l.title));
    row.append(badge(l.disposition));
    if (l.at_turn) row.append(line('mj-graph-tip-value', 'at its turn'));
    tip.append(row, line('mj-graph-tip-meta', entry.train.id + ' · turn ' + l.position + ' of ' + entry.train.links.length));
  }
  if (url) tip.append(line('mj-graph-tip-hint', 'Double-click, or select and press Enter, to open it'));

  // Beside the node, on whichever side has room, and never past the frame's edge. One read
  // of the card's size per hover; the frame's size is Cytoscape's, already measured.
  tip.hidden = false;
  const at = node.renderedPosition();
  const reach = node.renderedOuterWidth() / 2;
  const width = tip.offsetWidth;
  const height = tip.offsetHeight;
  const gap = 8;
  let x = at.x + reach + gap;
  if (x + width > cy.width() - gap) x = at.x - reach - gap - width;
  x = Math.max(gap, Math.min(x, cy.width() - width - gap));
  const y = Math.max(gap, Math.min(at.y - height / 2, cy.height() - height - gap));
  tip.style.transform = 'translate(' + Math.round(x) + 'px, ' + Math.round(y) + 'px)';
}

function hideTip(state) {
  if (state.tip) state.tip.hidden = true;
}

/**
 * A disposition as a badge. Its colours are the node's own role from ROLES, named by token,
 * so the card and the circle beside it cannot disagree: the status sheet's meaning of a word
 * like `ready` is a different vocabulary's, and the drawing is what the card annotates.
 */
function badge(word) {
  const role = ROLES[word] || ROLES.unknown;
  const span = document.createElement('span');
  // the status sheet's class for the word, where it files one, for whatever reads the class;
  // the role's own values below outrank it, so the badge always matches its circle
  span.className = 'mj-badge mj-badge--' + (ROLES[word] ? word : 'neutral');
  span.style.setProperty('--mj-status-fg', 'var(' + role.ring + ')');
  span.style.setProperty('--mj-status-bg', 'var(' + role.fill + ')');
  span.style.setProperty('--mj-status-line', 'var(' + role.ring + '-line)');
  const dot = document.createElement('span');
  dot.className = 'mj-badge-dot';
  span.append(dot, word);
  return span;
}

// --------------------------------------------------------------------------- the style

/**
 * The stylesheet, from the colours and type the declaration resolved to. `transition` is the
 * duration of a state change in milliseconds, zero for no motion; a theme change rebuilds
 * the whole sheet from here and hands it to `cy.style`.
 *
 * Emphasis is a halo (Cytoscape's underlay) rather than an overlay, so the node's own colour
 * still reads through it: the accent at 0.14 around the lit neighbourhood (graph.js's value,
 * so the relation surfaces share it), 0.18 under the node the pointer is on, 0.22 under the
 * selection, and the info role at 0.3 under a link the last answer changed. Master wears no
 * halo at rest, so the accent stays the interaction's; its own rules below only keep its halo
 * as wide as its larger circle needs. The opacities are numbers, not colours.
 */
function style(colours, type, transition) {
  const role = (d) => colours.roles[d] || colours.roles.unknown;
  const moving = transition
    ? [
        {
          selector: 'node, edge',
          style: {
            'transition-property':
              'opacity, text-opacity, underlay-opacity, border-width, line-color, target-arrow-color, width',
            'transition-duration': transition,
            'transition-timing-function': 'ease-out',
          },
        },
      ]
    : [];
  return [
    {
      selector: 'node',
      style: {
        shape: 'ellipse',
        width: 66,
        height: 66,
        'background-color': (n) => role(n.data('disposition')).fill,
        'border-width': 2,
        'border-color': (n) => role(n.data('disposition')).ring,
        label: 'data(caption)',
        color: colours.text,
        'font-size': type.caption,
        'font-family': type.sans,
        'font-weight': type.medium,
        'line-height': 1.3,
        'text-valign': 'center',
        'text-halign': 'center',
        'text-wrap': 'wrap',
        'text-max-width': '56px',
        // a caption too small to read is not drawn; zooming in brings it back
        'min-zoomed-font-size': 6,
        'overlay-opacity': 0,
        'underlay-color': colours.accent,
        'underlay-opacity': 0,
        'underlay-padding': 6,
        'underlay-shape': 'ellipse',
      },
    },
    { selector: 'node[turn = "yes"]', style: { 'border-width': 4 } },
    {
      selector: 'node[kind = "beneath"]',
      style: { 'border-style': 'dashed', width: 50, height: 50, 'text-max-width': '42px' },
    },
    {
      // the anchor: larger, heavier and in the accent's soft ground, so the eye finds the
      // centre every train lands on before anything else, without a halo the pointer's would
      // have to outshine
      selector: 'node[kind = "master"]',
      style: {
        width: 84,
        height: 84,
        'background-color': colours.master.fill,
        'border-color': colours.master.ring,
        'border-width': 3,
        'font-weight': type.semibold,
        'text-max-width': '72px',
      },
    },
    {
      selector: 'edge',
      style: {
        width: 2,
        'curve-style': 'bezier',
        'line-color': colours.edge,
        'target-arrow-color': colours.edge,
        'target-arrow-shape': 'triangle',
        'arrow-scale': 0.9,
        'target-distance-from-node': 2,
        label: 'data(label)',
        'font-size': type.meta,
        'font-family': type.mono,
        // the muted ink at full strength on an opaque ground: that pair is the one the
        // declaration measures, so the words recede by their size and ink, never by opacity.
        // They drop out where they would be unreadably small, and return on a zoom in or
        // around the node under the pointer
        color: colours.muted,
        'min-zoomed-font-size': 7,
        'text-rotation': 'autorotate',
        'text-background-color': colours.background,
        'text-background-opacity': 1,
        'text-background-padding': 3,
        'text-background-shape': 'round-rectangle',
        'overlay-opacity': 0,
      },
    },
    { selector: 'edge[kind = "depends_on"]', style: { 'line-style': 'dashed' } },
    { selector: 'edge[satisfied = "yes"], edge[kind = "merged"]', style: { 'line-style': 'dotted' } },
    { selector: 'edge[kind = "targets"], edge[kind = "merged"]', style: { width: 1.25, 'arrow-scale': 0.75 } },
    ...moving,
    { selector: '.mj-dim', style: { opacity: 0.15, 'text-opacity': 0 } },
    { selector: '.mj-hidden', style: { display: 'none' } },
    {
      selector: 'edge.mj-near',
      style: {
        'line-color': colours.accent,
        'target-arrow-color': colours.accent,
        color: colours.text,
        width: 2.2,
        'text-opacity': 1,
        'min-zoomed-font-size': 0,
        'z-index': 1,
      },
    },
    { selector: 'node.mj-near', style: { 'underlay-color': colours.accent, 'underlay-opacity': 0.14 } },
    { selector: 'node.mj-point', style: { 'underlay-opacity': 0.18, 'underlay-padding': 8 } },
    { selector: 'node:active', style: { 'underlay-color': colours.accent, 'underlay-opacity': 0.22 } },
    {
      selector: 'node.mj-changed',
      style: { 'underlay-color': colours.info, 'underlay-opacity': 0.3, 'underlay-padding': 10 },
    },
    {
      selector: 'node.mj-match',
      style: { 'border-width': 4, 'border-color': colours.accent, 'underlay-color': colours.accent, 'underlay-opacity': 0.16 },
    },
    {
      selector: 'node.mj-focus',
      style: {
        'border-width': 5,
        'border-color': colours.accent,
        'underlay-color': colours.accent,
        'underlay-opacity': 0.22,
        'underlay-padding': 6,
        'underlay-shape': 'ellipse',
      },
    },
    // master's circle is a third larger than a link's, so its halo is too: lit, pointed at
    // or selected, it only ever grows
    { selector: 'node[kind = "master"].mj-near', style: { 'underlay-padding': 10 } },
    { selector: 'node[kind = "master"].mj-point, node[kind = "master"].mj-focus', style: { 'underlay-padding': 12 } },
  ];
}

// ----------------------------------------------------------------------- the palette

/**
 * The sRGB channels of whatever a token resolves to, measured by painting it. The tokens
 * are `oklch`, which Cytoscape's colour parser does not read (see graph.js, `channels`,
 * for the measurement that found it); this is colour arithmetic at a library boundary, and
 * the decision is still share/design/tokens.yaml's. A token that does not resolve falls back
 * to the page's own computed colour, never to a literal.
 */
function channels(value, fallback) {
  if (!value) return fallback;
  try {
    const canvas = document.createElement('canvas');
    canvas.width = 1;
    canvas.height = 1;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    ctx.clearRect(0, 0, 1, 1);
    ctx.fillStyle = value;
    ctx.fillRect(0, 0, 1, 1);
    const px = ctx.getImageData(0, 0, 1, 1).data;
    if (!px[3]) return fallback;
    return [px[0], px[1], px[2]];
  } catch (e) {
    return fallback;
  }
}

/** Every colour the drawing uses, read from the declaration and turned into channels. */
function drawable() {
  const root = getComputedStyle(document.documentElement);
  const body = getComputedStyle(document.body);
  const text = channels(body.color, [0, 0, 0]);
  const ground = channels(body.backgroundColor, [255, 255, 255]);
  const read = (token, fallback) => channels(root.getPropertyValue(token).trim(), fallback);
  const accent = read('--mj-accent', text);
  const muted = read('--mj-muted', text);
  const roles = {};
  for (const [word, role] of Object.entries(ROLES)) {
    roles[word] = { fill: read(role.fill, ground), ring: read(role.ring, accent) };
  }
  return {
    roles,
    master: { fill: read(MASTER_ROLE.fill, ground), ring: read(MASTER_ROLE.ring, accent) },
    text: read('--mj-fg', text),
    muted,
    accent,
    info: read('--mj-info', accent),
    // A relation recedes behind the nodes it joins: the muted ink, part way to the frame's
    // ground, opaque, so the arrowheads and the dotted and dashed kinds keep their shape.
    // Heavier than graph.js's `--mj-line` at 0.7, on purpose: there an edge is one of
    // thousands and the nodes carry the picture, here a train *is* its edges, and the line
    // role is a border's (gray-200 on the gray-50 well, and the well's own gray-800 in the
    // dark), which would leave a stack's order and its dashes unreadable.
    edge: mixed('color-mix(in oklch, var(--mj-muted) 70%, var(--mj-sunken))', muted),
    background: read('--mj-sunken', ground),
  };
}

/**
 * A colour composed of tokens (`color-mix` over `var()`), resolved by the browser and then
 * measured like any token. The canvas cannot read `var()`, so a hidden element computes it.
 */
function mixed(expression, fallback) {
  const probe = document.createElement('span');
  probe.style.display = 'none';
  probe.style.color = expression;
  if (!probe.style.color) return fallback;
  document.body.append(probe);
  const value = getComputedStyle(probe).color;
  probe.remove();
  return channels(value, fallback);
}

/** The declared type scale, for a canvas that cannot be given a CSS class. */
function typeScale() {
  const root = getComputedStyle(document.documentElement);
  const step = (name) => {
    const value = parseFloat(root.getPropertyValue(name));
    return Number.isFinite(value) && value > 0 ? value : 0;
  };
  const body = getComputedStyle(document.body);
  const meta = step('--text-meta') || step('--text-label') || parseFloat(body.fontSize);
  // the families are the declaration's too: names in the sans, relation words in the mono
  const family = (name) => root.getPropertyValue(name).trim() || body.fontFamily;
  // and the weights: names in the medium, master in the semibold, the body's own weight if
  // the scale does not answer
  const weight = (name) => step(name) || parseFloat(body.fontWeight) || 'normal';
  return {
    meta,
    // a caption inside a circle is read at a glance: the label step when the scale has one
    caption: step('--text-label') || meta,
    sans: family('--font-sans'),
    mono: family('--font-mono'),
    medium: weight('--font-weight-medium'),
    semibold: weight('--font-weight-semibold'),
  };
}
