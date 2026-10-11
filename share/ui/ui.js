// The UI's one script: `majordomus ui`, the primary surface (ADR 0135).
//
// It adds two things to pages that are complete without it.
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

async function fillPanel(placeholder) {
  const src = placeholder.dataset.panelSrc;
  if (!src) return;
  placeholder.setAttribute('aria-busy', 'true');
  try {
    const response = await fetch(`${src}${src.includes('?') ? '&' : '?'}fragment`, { headers: { Accept: 'text/html' } });
    if (!response.ok) throw new Error(`${response.status}`);
    const html = await response.text();
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

// A token's colour as rgb(): the tokens resolve to oklch(), which the graph library cannot
// read, so the colour is painted on one pixel and read back in sRGB.
const probe = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
function token(name, fallback) {
  const v = getComputedStyle(document.body || document.documentElement).getPropertyValue(name).trim();
  if (!v || !probe) return fallback;
  probe.clearRect(0, 0, 1, 1);
  probe.fillStyle = '#000';
  probe.fillStyle = v;
  probe.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = probe.getImageData(0, 0, 1, 1).data;
  return a === 0 ? fallback : `rgb(${r}, ${g}, ${b})`;
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
const REFRESH_MS = 60000;
async function refresh() {
  if (document.visibilityState !== 'visible') return;
  const main = document.getElementById('main');
  if (!main || main.querySelector('[aria-busy="true"]')) return;
  try {
    const response = await fetch(`${window.location.pathname}?fragment`, { headers: { Accept: 'text/html' } });
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
  }
}

function enhance() {
  for (const placeholder of document.querySelectorAll('[data-panel-src]')) {
    fillPanel(placeholder);
  }
  drawGraphs();
}

wireTheme();
enhance();
if (!window.location.pathname.includes('/panels/')) setInterval(refresh, REFRESH_MS);
