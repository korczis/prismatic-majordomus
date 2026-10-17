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
// The theme toggle is revealed and wired to the same storage key and class the Cockpit
// and the site use. The page carries both as data attributes, so no name is written here.
//
// The Observatory's screen (`data-observatory`) is handed to `observatory.js`, below.
//
// A relationship graph (`data-graph`) is drawn with the vendored Cytoscape, loaded only when
// the screen has one, from the URL the page names (`data-graph-lib`). The nodes and edges are
// the ones the server put on the page; the table beside them is what a reader without the
// script, or without the library, reads instead. Colours are the design system's tones, read
// from the stylesheet, so nothing here names a colour.

const THEME_KEY = (document.body && document.body.dataset.themeKey) || null;
const THEME_CLASS = (document.body && document.body.dataset.themeClass) || 'dark';

function storeTheme(value) {
  if (!THEME_KEY) return;
  try {
    localStorage.setItem(THEME_KEY, value);
  } catch (e) {
    /* storage refused: the toggle still works for this page */
  }
}

function wireTheme() {
  for (const button of document.querySelectorAll('[data-theme-toggle]')) {
    button.hidden = false;
    button.addEventListener('click', () => {
      const dark = document.documentElement.classList.toggle(THEME_CLASS);
      storeTheme(dark ? 'dark' : 'light');
    });
  }
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

// A token's colour as sRGB hex: the tokens resolve to OKLCH, which the graph library cannot
// read, so the colour is painted on one pixel and its channels are read back. This converts a
// token; it chooses no colour, and a token that does not resolve keeps the caller's fallback.
const probe = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
function token(name, fallback) {
  const v = getComputedStyle(document.body || document.documentElement).getPropertyValue(name).trim();
  if (!v || !probe) return fallback;
  probe.clearRect(0, 0, 1, 1);
  probe.fillStyle = '#000';
  probe.fillStyle = v;
  probe.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = probe.getImageData(0, 0, 1, 1).data;
  const hex = (c) => c.toString(16).padStart(2, '0');
  return a === 0 ? fallback : `#${hex(r)}${hex(g)}${hex(b)}`;
}

async function drawGraphs() {
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
    const data = JSON.parse(holder.dataset.graph || '{"nodes":[],"edges":[]}');
    const canvas = holder.querySelector('.mj-ui-graph-canvas');
    if (!canvas || !data.nodes.length) continue;
    canvas.hidden = false;
    holder.classList.add('mj-ui-graph--drawn');
    const fill = (tone) => token(`--mj-tone-${tone}-fill`, token('--mj-accent-fill', '#888'));
    const cy = window.cytoscape({
      container: canvas,
      elements: [
        ...data.nodes.map((n) => ({ data: n })),
        ...data.edges.map((e, i) => ({ data: { id: `e${i}`, ...e } })),
      ],
      layout: data.layout === 'force'
        ? { name: 'cose', animate: !still, padding: 24, nodeRepulsion: () => 9000, idealEdgeLength: () => 70 }
        : { name: 'breadthfirst', directed: true, spacingFactor: 1.15, animate: !still, padding: 16 },
      wheelSensitivity: 0.2,
      style: [
        {
          selector: 'node',
          style: {
            'background-color': (n) => fill(n.data('tone')),
            label: 'data(label)',
            color: token('--mj-fg', '#888'),
            'font-size': 11,
            'font-family': getComputedStyle(document.body).getPropertyValue('--font-mono').trim() || 'monospace',
            'text-valign': 'bottom',
            'text-margin-y': 6,
            width: 22,
            height: 22,
            'border-width': 3,
            'border-color': token('--mj-raised', '#000'),
            'overlay-opacity': 0,
          },
        },
        {
          selector: 'edge',
          style: {
            width: 1.5,
            'line-color': token('--mj-line', '#888'),
            'target-arrow-color': token('--mj-muted', '#888'),
            'target-arrow-shape': 'triangle',
            'curve-style': 'bezier',
          },
        },
        { selector: 'node[kind = "leaf"]', style: { label: '', width: 12, height: 12, 'border-width': 2 } },
        { selector: 'node:active, node:selected', style: { 'border-color': token('--mj-accent', '#888'), label: 'data(label)' } },
      ],
    });
    cy.on('tap', 'node', (evt) => {
      const href = evt.target.data('href');
      if (href) window.location.assign(href);
    });
  }
}

// Reading the sources again: while the screen is visible, every minute, the server's
// rendering of this screen replaces the one on the page, and the time it was read is
// updated. This is a refresh, said as one: nothing here claims a live stream.
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
async function refresh() {
  if (document.visibilityState !== 'visible') return;
  const main = document.getElementById('main');
  if (!main || main.querySelector('[aria-busy="true"]')) return;
  // a screen that holds what the reader is doing (the Observatory's scene, a computation
  // asked in the browser) is not read again under them
  if (main.querySelector('[data-observatory], [data-keeps-its-place]')) return;
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

function enhance() {
  if (!SNAPSHOT) {
    for (const placeholder of document.querySelectorAll('[data-panel-src]')) {
      fillPanel(placeholder);
    }
  }
  drawGraphs();
  drawObservatory();
}

wireTheme();
enhance();
if (SNAPSHOT) {
  showSnapshotPath();
  window.addEventListener('hashchange', showSnapshotPath);
} else if (!window.location.pathname.includes('/panels/')) {
  setInterval(refresh, REFRESH_MS);
  catchUp();
}
