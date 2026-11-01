// The UI's one script: `majordomus ui`, the primary surface (ADR 0135).
//
// It adds two things to pages that are complete without it, and neither to a snapshot of
// them (below), which reads nothing again.
//
// A panel whose source is slow is drawn as a placeholder that links to the panel's own
// route (`data-panel-src`). Once the screen has painted, this fetches that route with
// `?fragment` and puts the server's rendering of the panel in its place. The panel is
// HTML the server built and escaped; nothing here decides what it says. If the fetch
// fails, the link stays, and the page says the panel could not be loaded.
//
// The bar at the top is the site's (project.the-ui-wears-the-sites-navbar), and so are its
// menus and its theme toggle, which the site's own nav.js and theme.js run. Two things are the
// UI's: the bar's search button, which the site's search script reveals on the site, is
// revealed here and leads to the UI's own search; and the bar's height is written into
// --ui-bar, so the sidebar stays in view beneath the bar.
//
// The Observatory's screen (`data-observatory`) is handed to `observatory.js`, below.
//
// A relationship graph (`data-graph`) is drawn with the vendored Cytoscape, loaded only when
// the screen has one, from the URL the page names (`data-graph-lib`). The nodes and edges are
// the ones the server put on the page; the table beside them is what a reader without the
// script, or without the library, reads instead. Colours are the design system's tones, read
// from the stylesheet, so nothing here names a colour. Pointing at a node lights it and its
// neighbours and shows a card naming it; the drawing is one keyboard stop, in which the arrows
// walk to the nearest node that way, Enter opens it, + / - / 0 zoom and fit, Shift+arrows pan
// and Escape clears. A plain wheel scrolls the page; Ctrl/Cmd+wheel or a pinch zooms. The
// minute's refresh redraws a graph where the reader left it: nothing moves or replays.
//
// A command to reproduce (`data-mj-copy`) has a copy button, in a snapshot too (I3002): the
// page carries it hidden, and it is shown only where a clipboard can be written, so a button
// that cannot copy is never offered. It copies exactly the text of the command beside it, and
// says what happened on itself and in the status beside it, which a screen reader announces.
//
// An issue's move form is wired by the Cockpit's `moves.js`, and a screen that follows the
// executions this server runs holds one channel through the Cockpit's `socket.js`: both are
// loaded only where the screen needs them, from the URLs the page names, never in a snapshot.

// The site's search button opens the site's search; here the search a reader wants is the
// UI's, so the button, the slash and Ctrl/Cmd+K lead to the UI's search field.
function wireBarSearch() {
  const field = document.getElementById('mj-ui-q');
  if (!field) return;
  for (const button of document.querySelectorAll('[data-site-search-open]')) {
    button.hidden = false;
    button.addEventListener('click', () => field.focus());
  }
  document.addEventListener('keydown', (event) => {
    const typing = event.target.closest && event.target.closest('input, textarea, select, [contenteditable]');
    if ((event.key === '/' && !typing) || ((event.ctrlKey || event.metaKey) && event.key === 'k')) {
      event.preventDefault();
      field.focus();
    }
  });
}

// The bar stays at the top of the window; the sidebar beneath it is as tall as what is left.
function measureBar() {
  const bar = document.querySelector('nav[aria-label="Main"]');
  if (!bar) return;
  const write = () => document.documentElement.style.setProperty('--ui-bar', `${bar.offsetHeight}px`);
  write();
  if (window.ResizeObserver) new ResizeObserver(write).observe(bar);
}

// A slow panel's source can take minutes (the integration queue spawns git per pull request),
// and a browser holds at most six connections to one host: a fetch per panel per visit, again
// on every refresh, and never given up, filled all six and stalled the whole UI for that
// browser. So a panel is fetched once at a time, its rendering is kept for PANEL_KEEP_MS and
// reused by a refresh, a fetch is given up after PANEL_TIMEOUT_MS, and every fetch in flight
// is given up when the page goes away.
const PANEL_TIMEOUT_MS = 45000;
const PANEL_KEEP_MS = 300000;
const panels = new Map(); // src -> { at, html } once answered, { pending } while asked
const inFlight = new Set();
window.addEventListener('pagehide', () => {
  for (const controller of inFlight) controller.abort();
});

async function panelHtml(src) {
  const known = panels.get(src);
  if (known && known.pending) return known.pending;
  if (known && Date.now() - known.at < PANEL_KEEP_MS) {
    // a panel that did not answer is not asked again until it is due: one slow source must
    // not take a connection every refresh
    if (known.error) throw new Error(known.error);
    return known.html;
  }
  const controller = new AbortController();
  inFlight.add(controller);
  const timer = setTimeout(() => controller.abort(), PANEL_TIMEOUT_MS);
  const pending = (async () => {
    try {
      const response = await fetch(`${src}${src.includes('?') ? '&' : '?'}fragment`, {
        headers: { Accept: 'text/html' },
        signal: controller.signal,
      });
      if (!response.ok) throw new Error(`${response.status}`);
      const html = await response.text();
      panels.set(src, { at: Date.now(), html });
      return html;
    } catch (e) {
      const error = controller.signal.aborted ? new Error('no answer in time') : e;
      panels.set(src, { at: Date.now(), error: error.message });
      throw error;
    } finally {
      clearTimeout(timer);
      inFlight.delete(controller);
    }
  })();
  panels.set(src, { pending });
  return pending;
}

async function fillPanel(placeholder) {
  const src = placeholder.dataset.panelSrc;
  if (!src) return;
  placeholder.setAttribute('aria-busy', 'true');
  try {
    const html = await panelHtml(src);
    const holder = document.createElement('div');
    holder.innerHTML = html;
    const panel = holder.firstElementChild;
    if (!panel) throw new Error('empty');
    placeholder.replaceWith(panel);
  } catch (e) {
    placeholder.setAttribute('aria-busy', 'false');
    const note = document.createElement('p');
    note.className = 'mj-ui-unanswered';
    note.textContent = `The panel could not be loaded here (${e.message}); its link still works.`;
    placeholder.append(note);
  }
}

function loadScript(src) {
  return new Promise((resolve, reject) => {
    const s = document.createElement('script');
    s.src = src;
    s.onload = resolve;
    s.onerror = () => reject(new Error(`could not load ${src}`));
    document.head.append(s);
  });
}

// A colour as sRGB hex. The tokens resolve to OKLCH, which the graph library cannot read, so
// the browser resolves the colour on a probe element (which also resolves a `color-mix()` of
// two tokens: the soft fills below) and it is painted on one pixel whose channels are read
// back. This converts; it chooses no colour. A token that does not resolve keeps the caller's
// fallback, and without one the page's own body colour, as the Cockpit's palette() does: a
// drawing without its tokens is monochrome, which is honest, and never a literal.
const probe = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
const swatch = document.createElement('span');
const painted = new Map(); // css -> hex, emptied when the theme changes
const pageStyle = () => getComputedStyle(document.body || document.documentElement);
function paint(css) {
  if (!css || !probe || !document.body) return '';
  if (painted.has(css)) return painted.get(css);
  swatch.style.color = '';
  swatch.style.color = css;
  if (!swatch.style.color) return ''; // not a colour this browser reads
  document.body.append(swatch);
  const resolved = getComputedStyle(swatch).color;
  swatch.remove();
  probe.clearRect(0, 0, 1, 1);
  probe.fillStyle = 'transparent';
  probe.fillStyle = resolved;
  probe.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = probe.getImageData(0, 0, 1, 1).data;
  const hex = (c) => c.toString(16).padStart(2, '0');
  const out = a === 0 ? '' : `#${hex(r)}${hex(g)}${hex(b)}`;
  painted.set(css, out);
  return out;
}
const declared = (name) => Boolean(pageStyle().getPropertyValue(name).trim());
function token(name, fallback) {
  return (declared(name) && paint(`var(${name})`)) || fallback || paint(pageStyle().color);
}
// `percent` of one token over another, opaque: a soft fill is a role over the ground it sits on
function mix(name, percent, ground) {
  if (!declared(name) || !declared(ground)) return token(name);
  return paint(`color-mix(in oklch, var(${name}) ${percent}%, var(${ground}))`) || token(name);
}
// a length or a duration the stylesheet declares, as the number the library wants (px or ms)
const measure = (name) => parseFloat(pageStyle().getPropertyValue(name));
function motionMs() {
  const v = pageStyle().getPropertyValue('--mj-motion-fast').trim();
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return 0;
  return v.endsWith('ms') ? n : n * 1000;
}

// A relationship graph reads like the rest of the design: a node of the graph's main kind is a
// soft disc in its tone with a ring of the tone's ink and its name on a chip of the page's
// ground; a leaf is a small solid dot, named when pointed at or chosen. Pointing at a node
// lights it and its neighbours and dims the rest; choosing one (a click, or the keyboard) rings
// it in the accent, the one colour that means interactive. A plain wheel scrolls the page as it
// does everywhere else, and Ctrl/Cmd+wheel or a pinch zooms; the frame's own buttons and keys
// do the rest. The minute's refresh redraws each graph where the reader left it, so nothing
// moves or animates again. Every number below is geometry; every colour is a token.
const ZOOM_MIN = 0.2;
const ZOOM_MAX = 3;
const ZOOM_STEP = 1.25;
const NATURAL = 1.25; // fitting never enlarges a small graph past this: it is read, not blown up
const FIT_PADDING = 24;
const PAN_STEP = 48;
const DIM = 0.15;
const HALO_HOVER = 0.14;
const HALO_CHOSEN = 0.22;
const LARGE = 400; // past this many nodes, no transitions and a texture while moving
const live = []; // { cy, key, holder, directed, moving, stop }, one per graph drawn
const seen = new Map(); // key -> { positions, zoom, pan, moved, chosen }, what a refresh restores
let graphCount = 0;

function graphStyle(directed, moving) {
  const size = measure('--text-label') || parseFloat(pageStyle().fontSize);
  const mono = pageStyle().getPropertyValue('--font-mono').trim() || pageStyle().fontFamily;
  const ink = token('--mj-fg');
  const ground = token('--mj-bg');
  const raised = token('--mj-raised');
  const accent = token('--mj-accent');
  // --mj-line is the frame's own ground in the dark theme, so an edge in it vanished there: an
  // edge is the muted ink softened over the frame, which reads, and recedes, in both themes
  const edge = mix('--mj-muted', 55, '--mj-sunken');
  const tones = new Map();
  const tone = (t) => {
    if (!tones.has(t)) {
      const named = /^[a-z]+$/.test(t || '') && declared(`--mj-tone-${t}`);
      const ring = named ? `--mj-tone-${t}` : '--mj-accent';
      tones.set(t, {
        solid: token(`--mj-tone-${t}-fill`, token('--mj-accent-fill')),
        ring: token(ring),
        soft: mix(ring, 14, '--mj-raised'),
      });
    }
    return tones.get(t);
  };
  const fast = moving ? motionMs() : 0;
  const transition = fast
    ? {
      'transition-property':
        'opacity, text-opacity, border-color, border-width, underlay-opacity, line-color, width',
      'transition-duration': fast,
    }
    : {};
  return [
    {
      selector: 'node',
      style: {
        'background-color': (n) => tone(n.data('tone')).soft,
        'border-color': (n) => tone(n.data('tone')).ring,
        'border-width': 2,
        width: 26,
        height: 26,
        label: 'data(label)',
        color: ink,
        'font-size': size,
        'font-family': mono,
        'text-valign': 'bottom',
        'text-margin-y': 6,
        'text-wrap': 'ellipsis',
        'text-max-width': 160,
        'text-background-color': ground,
        'text-background-opacity': 1,
        'text-background-shape': 'round-rectangle',
        'text-background-padding': 2,
        'min-zoomed-font-size': 7,
        'overlay-opacity': 0,
        'underlay-color': accent,
        'underlay-opacity': 0,
        'underlay-padding': 8,
        'underlay-shape': 'ellipse',
        ...transition,
      },
    },
    {
      selector: 'node[kind = "leaf"]',
      style: {
        'background-color': (n) => tone(n.data('tone')).solid,
        'border-color': raised,
        label: '',
        width: 12,
        height: 12,
        'underlay-padding': 6,
      },
    },
    {
      selector: 'edge',
      style: {
        width: 1.5,
        'line-color': edge,
        'target-arrow-color': edge,
        'target-arrow-shape': 'triangle',
        'arrow-scale': 0.8,
        ...(directed
          ? { 'curve-style': 'taxi', 'taxi-direction': 'downward', 'taxi-turn': '50%' }
          : { 'curve-style': 'bezier' }),
        ...transition,
      },
    },
    { selector: '.mj-dim', style: { opacity: DIM, 'text-opacity': 0 } },
    { selector: 'node.mj-hover', style: { 'underlay-opacity': HALO_HOVER, label: 'data(label)' } },
    {
      selector: 'edge.mj-near',
      style: { 'line-color': accent, 'target-arrow-color': accent, width: 2.2, 'z-index': 9 },
    },
    {
      selector: 'node:selected',
      style: {
        'border-color': accent,
        'border-width': 3,
        'underlay-opacity': HALO_CHOSEN,
        label: 'data(label)',
      },
    },
  ];
}

// The colours are read again when the theme changes, and every graph on the screen is restyled
// where it stands: nothing is laid out again and nothing replays.
let themeWatch = null;
let themeGround = '';
function watchTheme() {
  if (themeWatch || !window.MutationObserver) return;
  themeGround = pageStyle().getPropertyValue('--mj-bg').trim();
  themeWatch = new MutationObserver(() => {
    const ground = pageStyle().getPropertyValue('--mj-bg').trim();
    if (ground === themeGround) return;
    themeGround = ground;
    painted.clear();
    for (const g of live) g.cy.style(graphStyle(g.directed, g.moving));
  });
  themeWatch.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
}

// A graph whose screen was replaced is given back: where its nodes stood, how far it was zoomed,
// which node was chosen and whether the keyboard was in it are kept under its name, for the
// same graph on the new screen. Only a graph that was laid out in a frame of its own is kept,
// and the first one of a name is the one kept, so a drawing into a screen already replaced
// (a zero-size frame) never overwrites where the reader left the real one.
function letGo() {
  const saved = new Set();
  for (let i = 0; i < live.length; i++) {
    const g = live[i];
    const container = g.cy.container();
    if (container && container.isConnected) continue;
    if (!saved.has(g.key) && g.cy.width() > 0 && g.cy.height() > 0) {
      saved.add(g.key);
      const positions = {};
      g.cy.nodes().forEach((n) => {
        positions[n.id()] = { ...n.position() };
      });
      const chosen = g.cy.nodes(':selected');
      seen.set(g.key, {
        positions,
        zoom: g.cy.zoom(),
        pan: { ...g.cy.pan() },
        moved: g.moved,
        chosen: chosen.length ? chosen[0].id() : null,
        focused: g.focused,
      });
    }
    g.stop();
    g.cy.destroy();
    live.splice(i, 1);
    i--;
  }
}

// Where the nodes a refreshed screen adds stand: beside what they touch, never on top of what
// is there. A node with a neighbour already standing goes to the middle of its standing
// neighbours, a row below them in a tree (above, when it points at them) or an edge's length
// away in a force layout, spread when several share those neighbours, and moved along until
// it stands on nobody. What touches nothing standing is laid out among itself below the rest.
const EDGE_LENGTH = 70;
// two nodes stand on each other when nearer than ROOM one above the other, or twice that side
// by side, where their names are
const ROOM = 48;
function placeNew(cy, before, directed, placement) {
  const at = {};
  const where = (id) => at[id] || before.positions[id];
  const taken = [];
  cy.nodes().forEach((node) => {
    const p = before.positions[node.id()];
    if (p) taken.push(p);
  });
  // a tree's row height is the smallest step between the rows it already has
  let row = 0;
  if (directed) {
    const ys = [...new Set(taken.map((p) => Math.round(p.y)))].sort((a, b) => a - b);
    for (let i = 1; i < ys.length; i++) {
      const step = ys[i] - ys[i - 1];
      if (step > ROOM / 2 && (!row || step < row)) row = step;
    }
  }
  row = row || 2 * EDGE_LENGTH;
  const free = (p) => taken.every((q) => {
    const dx = (q.x - p.x) / (2 * ROOM);
    const dy = (q.y - p.y) / ROOM;
    return dx * dx + dy * dy >= 1;
  });
  let left = cy.nodes().filter((node) => !before.positions[node.id()]);
  for (let progress = true; progress && left.length;) {
    progress = false;
    const groups = new Map(); // the neighbours a node stands by -> the nodes standing by them
    left.forEach((node) => {
      const by = node.neighborhood('node').filter((m) => where(m.id()));
      if (!by.length) return;
      const ids = by.map((m) => m.id()).sort().join(' ');
      if (!groups.has(ids)) groups.set(ids, []);
      groups.get(ids).push({ node, by });
    });
    groups.forEach((group) => {
      group.forEach(({ node, by }, i) => {
        let x = 0;
        let y = 0;
        by.forEach((m) => {
          x += where(m.id()).x;
          y += where(m.id()).y;
        });
        x /= by.length;
        y /= by.length;
        const spread = i - (group.length - 1) / 2;
        let p;
        if (directed) {
          // below what points at it, above what it points at
          const child = node.incomers('node').some((m) => where(m.id()));
          p = { x: x + spread * 2 * ROOM, y: y + (child ? row : -row) };
        } else {
          const turn = (2 * Math.PI * i) / group.length + Math.PI / 2;
          p = { x: x + EDGE_LENGTH * Math.cos(turn), y: y + EDGE_LENGTH * Math.sin(turn) };
        }
        for (let step = 0; step < 100 && !free(p); step++) p.x += ROOM / 2;
        at[node.id()] = p;
        taken.push(p);
        progress = true;
      });
    });
    left = left.filter((node) => !at[node.id()]);
  }
  cy.batch(() => {
    for (const id of Object.keys(at)) cy.getElementById(id).position(at[id]);
  });
  if (!left.length) return;
  const all = cy.nodes().not(left).boundingBox();
  left.layout({
    ...placement,
    animate: false,
    fit: false,
    boundingBox: {
      x1: all.x1,
      y1: all.y2 + row,
      w: Math.max(all.w, 4 * EDGE_LENGTH),
      h: Math.max(row, 2 * EDGE_LENGTH),
    },
  }).run();
}

// The node nearest `from` in the direction (dx, dy), within 60 degrees either side of it.
function nearestToward(cy, from, dx, dy) {
  const p = from.position();
  let best = null;
  let score = Infinity;
  cy.nodes(':visible').forEach((n) => {
    if (n.same(from)) return;
    const q = n.position();
    const vx = q.x - p.x;
    const vy = q.y - p.y;
    const d = Math.hypot(vx, vy);
    if (!d) return;
    const cos = (vx * dx + vy * dy) / d;
    if (cos < 0.5) return;
    const s = d * (2 - cos);
    if (s < score) {
      score = s;
      best = n;
    }
  });
  return best;
}

// the node nearest the middle of what the frame shows: where the keyboard starts
function nearestMiddle(cy) {
  const z = cy.zoom();
  const pan = cy.pan();
  const mx = (cy.width() / 2 - pan.x) / z;
  const my = (cy.height() / 2 - pan.y) / z;
  let best = null;
  let score = Infinity;
  cy.nodes(':visible').forEach((n) => {
    const q = n.position();
    const d = Math.hypot(q.x - mx, q.y - my);
    if (d < score) {
      score = d;
      best = n;
    }
  });
  return best;
}

function drawGraph(holder, canvas, data, still) {
  const key = canvas.getAttribute('aria-label') || `graph ${live.length}`;
  const before = seen.get(key);
  const directed = data.layout !== 'force';
  const nodes = data.nodes.length;
  const moving = !still && nodes <= LARGE;
  const fast = still ? 0 : motionMs();
  const n = ++graphCount;

  canvas.hidden = false;
  // a graph drawn again by the minute's refresh does not fade in again
  if (before) holder.classList.add('mj-ui-graph--again');
  holder.classList.add('mj-ui-graph--drawn');
  canvas.tabIndex = 0;
  canvas.setAttribute('role', 'application');
  canvas.setAttribute('aria-roledescription', 'graph');

  const keys = document.createElement('p');
  keys.id = `mj-ui-graph-keys-${n}`;
  keys.className = 'mj-sr-only';
  keys.textContent = 'Arrow keys move between nodes, Enter opens the chosen one, Escape clears. '
    + 'Home and End reach the first and last node. '
    + 'Plus and minus zoom, 0 fits the graph, Shift with an arrow pans. '
    + 'Ctrl or Cmd with the wheel zooms; the wheel alone scrolls the page.';
  const said = document.createElement('p');
  said.className = 'mj-sr-only';
  said.setAttribute('aria-live', 'polite');
  canvas.setAttribute('aria-describedby', keys.id);

  const tip = document.createElement('div');
  tip.className = 'mj-graph-tip';
  tip.hidden = true;
  tip.setAttribute('aria-hidden', 'true');

  const tools = document.createElement('div');
  tools.className = 'mj-ui-graph-tools';
  tools.setAttribute('role', 'group');
  tools.setAttribute('aria-label', 'Graph view');
  const tool = (text, label, act) => {
    const b = document.createElement('button');
    b.type = 'button';
    b.className = 'mj-button';
    b.textContent = text;
    b.title = label;
    b.setAttribute('aria-label', label);
    b.addEventListener('click', act);
    tools.append(b);
  };
  canvas.after(tools, tip, keys, said);

  const positioned = before
    ? data.nodes.filter((d) => before.positions[d.id]).length
    : 0;
  const placement = directed
    ? { name: 'breadthfirst', directed: true, spacingFactor: 1.15, padding: FIT_PADDING }
    : {
      name: 'cose',
      padding: FIT_PADDING,
      nodeRepulsion: () => 9000,
      idealEdgeLength: () => EDGE_LENGTH,
    };
  const layout = before
    ? { name: 'preset', positions: (node) => before.positions[node.id()], fit: false }
    : {
      ...placement,
      // a force layout grows out of its middle; a tree settles into its rows
      animate: fast ? (directed ? true : 'end') : false,
      animationDuration: 3 * fast,
      // the first fit stops at the natural size; after it, the reader may zoom to the bound
      stop: (event) => event.cy.maxZoom(ZOOM_MAX),
    };

  const cy = window.cytoscape({
    container: canvas,
    elements: [
      ...data.nodes.map((d) => ({ data: d })),
      ...data.edges.map((e, i) => ({ data: { id: `e${i}`, ...e } })),
    ],
    layout,
    minZoom: ZOOM_MIN,
    maxZoom: before ? ZOOM_MAX : NATURAL,
    wheelSensitivity: 0.2,
    hideEdgesOnViewport: nodes > LARGE,
    textureOnViewport: nodes > LARGE,
    style: graphStyle(directed, moving),
  });
  holder.mjGraph = cy;

  const g = { cy, key, holder, directed, moving, moved: false, focused: false, stop: () => {} };
  live.push(g);

  if (before) {
    // the nodes this screen adds are placed beside what they touch, without moving the others
    if (positioned < nodes) placeNew(cy, before, directed, placement);
    cy.viewport({ zoom: before.zoom, pan: before.pan });
    g.moved = before.moved;
  }

  // a plain wheel scrolls the page; Ctrl/Cmd+wheel (a pinch arrives as one) reaches the drawing
  holder.addEventListener('wheel', (event) => {
    if (!event.ctrlKey && !event.metaKey) event.stopPropagation();
  }, { capture: true, passive: true });

  const zoomTo = (level) => {
    const z = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, level));
    const at = { x: cy.width() / 2, y: cy.height() / 2 };
    g.moved = true;
    if (fast) cy.animate({ zoom: { level: z, renderedPosition: at } }, { duration: 2 * fast });
    else cy.zoom({ level: z, renderedPosition: at });
  };
  // the whole graph in the frame, and no larger than its natural size, centred
  const framing = () => {
    const all = cy.elements();
    const vp = cy.getFitViewport(all, FIT_PADDING);
    if (!vp || vp.zoom <= NATURAL) return vp;
    const bb = all.boundingBox();
    return {
      zoom: NATURAL,
      pan: {
        x: cy.width() / 2 - NATURAL * (bb.x1 + bb.w / 2),
        y: cy.height() / 2 - NATURAL * (bb.y1 + bb.h / 2),
      },
    };
  };
  const fit = (animated) => {
    const vp = framing();
    g.moved = false;
    if (!vp) return;
    if (fast && animated !== false) cy.animate(vp, { duration: 2 * fast });
    else cy.viewport(vp);
  };
  tool('−', 'Zoom out', () => zoomTo(cy.zoom() / ZOOM_STEP));
  tool('+', 'Zoom in', () => zoomTo(cy.zoom() * ZOOM_STEP));
  tool('Fit', 'Fit the graph to the frame', () => fit());
  cy.on('scrollzoom pinchzoom dragpan', () => {
    g.moved = true;
  });

  // Lit: one node and the nodes and edges it touches; everything else dims. A chosen node
  // outranks the pointer: while one is held, pointing elsewhere names that node (its card and
  // a halo) and leaves the picture as it is.
  const light = (node) => {
    cy.batch(() => {
      cy.elements().removeClass('mj-near mj-dim');
      if (!node) return;
      const near = node.closedNeighborhood();
      near.addClass('mj-near');
      cy.elements().not(near).addClass('mj-dim');
    });
  };
  const chosen = () => {
    const c = cy.nodes(':selected');
    return c.length ? c[0] : null;
  };

  const coarse = window.matchMedia('(pointer: coarse)').matches;
  const hideTip = () => {
    tip.hidden = true;
  };
  const showTip = (node) => {
    const label = document.createElement('div');
    label.className = 'mj-graph-tip-title';
    label.textContent = node.data('label');
    // The card says what the server said about the node and no more: its name and how many
    // links it has. The node's `kind` is how it is drawn (a disc or a leaf), not what it is,
    // and its tone is a status the data does not name, so neither is printed or keyed here.
    const row = document.createElement('div');
    row.className = 'mj-graph-tip-row';
    const name = document.createElement('span');
    name.className = 'mj-graph-tip-name';
    name.textContent = 'Links';
    const links = node.connectedEdges().length;
    const value = document.createElement('span');
    value.className = 'mj-graph-tip-value';
    value.textContent = String(links);
    row.append(name, value);
    tip.replaceChildren(label, row);
    if (node.data('href')) {
      const foot = document.createElement('div');
      foot.className = 'mj-graph-tip-foot';
      foot.textContent = 'Enter or click to open';
      tip.append(foot);
    }
    tip.hidden = false;
    // read once, then written once: the card is kept inside the frame and off the node and
    // its name: above them, or beside them where there is no room above, to the right when
    // the card fits there and otherwise on the side with more room
    const w = tip.offsetWidth;
    const h = tip.offsetHeight;
    const p = node.renderedPosition();
    const box = node.renderedBoundingBox({ includeLabels: true, includeOverlays: false });
    const gap = 8;
    const inside = (v, room) => Math.min(Math.max(v, 4), room - 4);
    let left = inside(p.x - w / 2, cy.width() - w);
    let top = box.y1 - gap - h;
    if (top < 4) {
      top = inside(p.y - h / 2, cy.height() - h);
      const right = box.x2 + gap + w <= cy.width() - 4 || p.x < cy.width() / 2;
      left = inside(right ? box.x2 + gap : box.x1 - gap - w, cy.width() - w);
    }
    left += canvas.offsetLeft;
    top += canvas.offsetTop;
    tip.style.transform = `translate(${Math.round(left)}px, ${Math.round(top)}px)`;
  };
  let restoring = false; // a refresh gives the chosen node back without announcing it again
  const say = (node) => {
    if (restoring) return;
    const links = node.connectedEdges().length;
    said.textContent = `${node.data('label')}, ${links} ${links === 1 ? 'link' : 'links'}.`
      + `${node.data('href') ? ' Enter opens it.' : ''}`;
  };

  cy.on('mouseover', 'node', (evt) => {
    const node = evt.target;
    if (!chosen()) light(node);
    node.addClass('mj-hover');
    canvas.style.cursor = node.data('href') ? 'pointer' : '';
    if (!coarse) showTip(node);
  });
  cy.on('mouseout', 'node', (evt) => {
    evt.target.removeClass('mj-hover');
    canvas.style.cursor = '';
    hideTip();
    light(chosen());
  });
  cy.on('viewport', hideTip);
  cy.on('select', 'node', (evt) => {
    light(evt.target);
    say(evt.target);
  });
  cy.on('unselect', 'node', () => {
    if (!chosen()) light(null);
  });
  cy.on('tap', 'node', (evt) => {
    const href = evt.target.data('href');
    if (href) window.location.assign(href);
    // the keyboard carries on from what the pointer chose
    else if (document.activeElement !== canvas) canvas.focus({ preventScroll: true });
  });

  // The keyboard: the drawing is one stop, and inside it the arrows walk the graph.
  const choose = (node) => {
    if (!node) return;
    cy.nodes(':selected').unselect();
    node.select();
    const p = node.renderedPosition();
    const outside = p.x < FIT_PADDING || p.y < FIT_PADDING
      || p.x > cy.width() - FIT_PADDING || p.y > cy.height() - FIT_PADDING;
    if (!outside) {
      showTip(node);
    } else if (fast) {
      g.moved = true;
      cy.animate({ center: { eles: node } }, {
        duration: 2 * fast,
        complete: () => showTip(node),
      });
    } else {
      g.moved = true;
      cy.center(node);
      showTip(node);
    }
  };
  const ARROWS = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
  canvas.addEventListener('keydown', (event) => {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const k = event.key;
    const current = chosen();
    if (ARROWS[k] && event.shiftKey) {
      const [dx, dy] = ARROWS[k];
      g.moved = true;
      cy.panBy({ x: -dx * PAN_STEP, y: -dy * PAN_STEP });
    } else if (ARROWS[k]) {
      const [dx, dy] = ARROWS[k];
      choose(current ? nearestToward(cy, current, dx, dy) || current : nearestMiddle(cy));
    } else if (k === 'Home' || k === 'End') {
      const all = cy.nodes(':visible');
      choose(k === 'Home' ? all.first() : all.last());
    } else if (k === 'Enter') {
      const href = current && current.data('href');
      if (!href) return;
      window.location.assign(href);
    } else if (k === '+' || k === '=') {
      zoomTo(cy.zoom() * ZOOM_STEP);
    } else if (k === '-' || k === '_') {
      zoomTo(cy.zoom() / ZOOM_STEP);
    } else if (k === '0') {
      fit();
    } else if (k === 'Escape') {
      cy.nodes(':selected').unselect();
      hideTip();
      said.textContent = '';
    } else {
      return;
    }
    event.preventDefault();
  });
  // whether the keyboard is in the drawing, so a refresh gives it back: a blur that comes with
  // the screen being replaced (the canvas already out of the page) is not the reader leaving
  canvas.addEventListener('focus', () => {
    g.focused = true;
  });
  canvas.addEventListener('blur', () => {
    hideTip();
    if (canvas.isConnected) g.focused = false;
  });

  // a frame that changes size is fitted again, unless the reader has zoomed or panned since
  if (window.ResizeObserver) {
    let size = `${canvas.clientWidth}x${canvas.clientHeight}`;
    const watch = new ResizeObserver(() => {
      const now = `${canvas.clientWidth}x${canvas.clientHeight}`;
      if (now === size) return;
      size = now;
      cy.resize();
      if (!g.moved) fit(false);
    });
    watch.observe(canvas);
    g.stop = () => watch.disconnect();
  }

  if (before && before.chosen) {
    const again = cy.getElementById(before.chosen);
    restoring = true;
    if (again.length) again.select();
    restoring = false;
  }
  // the keyboard that was walking the graph walks on from where it was, unless the reader has
  // since moved it somewhere else on the page
  const idle = !document.activeElement || document.activeElement === document.body;
  if (before && before.focused && idle) canvas.focus({ preventScroll: true });
}

async function drawGraphs() {
  letGo();
  const holders = [...document.querySelectorAll('[data-graph]')];
  const lib = document.body && document.body.dataset.graphLib;
  if (!holders.length || !lib) return;
  try {
    if (!window.cytoscape) await loadScript(lib);
  } catch (e) {
    return; // the tables stand
  }
  const still = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  for (const holder of holders) {
    // drawn already, or on a screen a refresh replaced while the library was loading
    if (holder.mjGraph || !holder.isConnected) continue;
    const data = JSON.parse(holder.dataset.graph || '{"nodes":[],"edges":[]}');
    const canvas = holder.querySelector('.mj-ui-graph-canvas');
    if (!canvas || !data.nodes.length) continue;
    drawGraph(holder, canvas, data, still);
  }
  watchTheme();
}

// Reading the sources again: while the screen is visible, every minute, the server's
// rendering of this screen replaces the one on the page, and the time it was read is
// updated. This is a refresh, said as one. A screen that follows the server's live channel
// (`data-follow`, below) is read again when the server says it changed instead, and the
// minute's refresh leaves it alone while that channel is open. A screen that follows the
// executions this server runs is also read again when one ends (`followExecutions`, below),
// and its line says what that channel is doing; nothing claims more than its channel carries.
//
// The server never makes a screen wait for a source. A value it read before the repository
// last changed, or has not read yet, is shown as that, and its source line carries
// `data-catching-up` while the server reads it again. A screen with such a line is read
// again after CATCH_UP_MS, then after twice that, up to CATCH_UP_MAX_MS, until none is left.
const REFRESH_MS = 60000;
const CATCH_UP_MS = 400;
const CATCH_UP_MAX_MS = 6400;
let catchUpIn = CATCH_UP_MS;
let catchUpTimer = null;
function catchUp() {
  if (catchUpTimer) return;
  if (!document.querySelector('#main [data-catching-up]')) {
    catchUpIn = CATCH_UP_MS;
    return;
  }
  catchUpTimer = setTimeout(async () => {
    catchUpTimer = null;
    catchUpIn = Math.min(catchUpIn * 2, CATCH_UP_MAX_MS);
    await refresh();
    catchUp();
  }, catchUpIn);
}
// One reading at a time. A screen asked to be read again while a reading is in flight is read
// once more after it, so a later answer is never overwritten by an earlier one that arrived
// last, and a request that came during a failed reading is not lost.
let reading = null;
let readAgain = false;
async function refresh() {
  if (reading) {
    readAgain = true;
    return reading;
  }
  reading = (async () => {
    do {
      readAgain = false;
      await readScreen();
    } while (readAgain);
  })();
  try {
    await reading;
  } finally {
    reading = null;
  }
  // whoever asked for this reading, a live channel's change frame included: a screen that came
  // back with a value the server is still reading again is read again until it has caught up,
  // because no other frame may follow the one that asked
  catchUp();
}
async function readScreen() {
  if (document.visibilityState !== 'visible') return;
  const main = document.getElementById('main');
  if (!main || main.querySelector('[aria-busy="true"]')) return;
  if (main.querySelector('[data-observatory]')) return;
  // the screen as it is shown, filters included: the query is the reader's, not this script's
  const query = window.location.search ? `${window.location.search}&` : '?';
  const controller = new AbortController();
  inFlight.add(controller);
  const timer = setTimeout(() => controller.abort(), PANEL_TIMEOUT_MS);
  try {
    const response = await fetch(`${window.location.pathname}${query}fragment`, {
      headers: { Accept: 'text/html' },
      signal: controller.signal,
    });
    if (!response.ok) return;
    const holder = document.createElement('div');
    holder.innerHTML = await response.text();
    const screen = holder.firstElementChild;
    const current = main.lastElementChild;
    if (!screen || !current) return;
    current.replaceWith(screen);
    const stamp = document.querySelector('[data-read-at]');
    if (stamp) {
      const now = new Date().toISOString().replace(/\.\d+Z$/, 'Z');
      stamp.setAttribute('datetime', now);
      stamp.textContent = now.replace('T', ' ');
    }
    enhance();
  } catch (e) {
    /* the page as it is stays; the next round tries again */
  } finally {
    clearTimeout(timer);
    inFlight.delete(controller);
  }
}

// A snapshot (ADR 0141) is the UI read once at a commit and published as files: nothing on it
// is read again, because a host answers a query it ignores with the same file, and a panel's
// placeholder already links to the panel's own page. The body says so (`data-snapshot`).
// Its page about the live UI names the path a link meant, which the link carries after `#`.
const SNAPSHOT = Boolean(document.body && 'snapshot' in document.body.dataset);

function showSnapshotPath() {
  const target = document.querySelector('[data-snapshot-path]');
  if (target && window.location.hash.length > 1) {
    target.textContent = decodeURIComponent(window.location.hash.slice(1));
  }
}

// The Observatory (`data-observatory`) is drawn by its own script, loaded only on its screen
// from the URL the page names. It reads its page again by itself, so this script's refresh
// leaves that screen alone: replacing it would take the display down every minute.
let observatory = null;
async function drawObservatory() {
  const holder = document.querySelector('[data-observatory]');
  if (!holder || observatory) return;
  const lib = holder.dataset.observatoryLib;
  if (!lib) return;
  try {
    if (!window.mjObservatory) await loadScript(lib);
  } catch (e) {
    return; // the table of scenes stands
  }
  observatory = window.mjObservatory.mount(holder, { snapshot: SNAPSHOT });
  holder.observatory = observatory;
}

// A screen that follows a live channel (`data-follow`, ADR 0176) is read again when the server
// says what it shows changed. The module that follows it, `live.js`, is loaded only on such a
// screen, from the URL the page names (`data-follow-lib`), and opens the channel through the
// one socket client the Cockpit runs too. The line the screen marks `data-follow-state` says
// what the module says: live, reconnecting, or read every minute instead. A change that
// arrives while the page is hidden is read once it is shown again.
let followed = null;
let missed = false;
function sayFollowing() {
  const line = document.querySelector('#main [data-follow-state]');
  if (line && followed) line.textContent = followed.said();
}
async function followLive() {
  const holder = document.querySelector('#main [data-follow]');
  if (!holder || followed || SNAPSHOT) return;
  const lib = holder.dataset.followLib;
  if (!lib) return;
  let module;
  try {
    module = await import(lib);
  } catch (e) {
    return; // the minute's refresh stands
  }
  followed = module.followScreen(holder.dataset.follow, {
    refresh: () => {
      if (document.visibilityState === 'visible') refresh();
      else missed = true;
    },
    say: sayFollowing,
  });
  sayFollowing();
}
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible' && missed) {
    missed = false;
    refresh();
  }
});

// The moves of an issue — start, verify, done — wherever a screen renders the form: the one
// module the Cockpit's issue page loads too, named by the page (`data-moves-lib`). Never in a
// snapshot, which no server stands behind: there the buttons stay disabled, as rendered.
async function wireMoves() {
  const lib = document.body && document.body.dataset.movesLib;
  if (!lib || !document.querySelector('[data-mj-transition]')) return;
  try {
    (await import(lib)).installAll();
  } catch (e) {
    // the buttons stay disabled and the commands beside them still say what to run
  }
}

// A screen that follows the executions this server runs (`data-follows-executions`) holds one
// channel to the stream the page names (`data-events`), through the Cockpit's one WebSocket
// client (`data-socket-lib`). When an execution ends — completed, failed or cancelled — the
// screen is read again, once for a burst of them and at most every READ_AGAIN_MS, so a move
// made anywhere on this server shows here without waiting for the minute's refresh. A channel
// that reconnected, or saw a gap, may have missed an end, so that reads the screen again too.
// The line beside the read time says what the channel is doing, from the socket's own state
// and nothing more. Only executions of this server travel on it: a move another process makes
// still waits for the minute. Never in a snapshot, which no server stands behind.
const ENDED = new Set(['execution.completed', 'execution.failed', 'execution.cancelled']);
const READ_AGAIN_MS = 2000;
let following = null;
let readAgainTimer = null;
let readAgainAt = 0;
function readAgainSoon() {
  if (readAgainTimer) return;
  const wait = Math.max(250, readAgainAt + READ_AGAIN_MS - Date.now());
  readAgainTimer = setTimeout(async () => {
    readAgainTimer = null;
    readAgainAt = Date.now();
    await refresh();
    catchUp();
  }, wait);
}
function showChannel(state) {
  const line = document.querySelector('[data-live-state]');
  if (!line) return;
  line.hidden = false;
  line.dataset.liveState = state;
  line.textContent = ` · executions ${state}`;
}
async function followExecutions() {
  const body = document.body;
  const lib = body && body.dataset.socketLib;
  const path = body && body.dataset.events;
  if (following || !lib || !path) return;
  if (!document.querySelector('#main [data-follows-executions]')) return;
  following = true; // before the import: a refresh's enhance() must not open a second one
  let socket;
  try {
    socket = await import(lib);
  } catch (e) {
    following = null;
    return; // the minute's refresh still reads the screen again
  }
  const channel = socket.follow(path, {
    message(frame) {
      if (ENDED.has(frame.type)) readAgainSoon();
    },
    state: showChannel,
    resync() {
      readAgainSoon();
    },
  });
  following = channel;
  window.addEventListener('pagehide', () => channel.close());
}

const COPY_SAID_MS = 1600;
function showCopyButtons() {
  if (!(navigator.clipboard && window.isSecureContext)) return;
  for (const button of document.querySelectorAll('button[data-mj-copy][hidden]')) button.hidden = false;
}
// one listener for every copy button, so a screen read again keeps its buttons working
document.addEventListener('click', async (event) => {
  const button = event.target.closest && event.target.closest('button[data-mj-copy]');
  const block = button && button.closest('.mj-ui-command');
  const code = block && block.querySelector('code');
  if (!code) return;
  const status = block.querySelector('[role="status"]');
  const say = (words) => {
    button.textContent = words;
    if (status) status.textContent = words;
  };
  try {
    await navigator.clipboard.writeText(code.textContent);
    say('Copied');
  } catch (e) {
    say('Copy failed — select the text');
  }
  clearTimeout(button.copySaid);
  button.copySaid = setTimeout(() => {
    button.textContent = 'Copy';
    if (status) status.textContent = '';
  }, COPY_SAID_MS);
});

function enhance() {
  showCopyButtons();
  if (!SNAPSHOT) {
    for (const placeholder of document.querySelectorAll('[data-panel-src]')) {
      fillPanel(placeholder);
    }
    wireMoves();
    followExecutions();
  }
  drawGraphs();
  drawObservatory();
  followLive();
  sayFollowing();
}

wireBarSearch();
measureBar();
enhance();
if (SNAPSHOT) {
  showSnapshotPath();
  window.addEventListener('hashchange', showSnapshotPath);
} else if (!window.location.pathname.includes('/panels/')) {
  setInterval(() => {
    if (!followed || !followed.live()) refresh();
  }, REFRESH_MS);
  catchUp();
}
