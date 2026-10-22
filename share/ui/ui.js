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
// A relationship graph (`data-graph`) is drawn with the vendored Cytoscape, loaded only when
// the screen has one, from the URL the page names (`data-graph-lib`). The nodes and edges are
// the ones the server put on the page; the table beside them is what a reader without the
// script, or without the library, reads instead. Colours are the design system's tones, read
// from the stylesheet, so nothing here names a colour.
//
// The application shell's conveniences, each one a way to reach or arrange what the server
// already put on the page, and none of them a fact of its own:
// - a long table gets a filter, sortable headers and pages. The rows are the server's rows;
//   this hides and orders them and says how many of how many are shown. What the reader
//   chose is kept across the minute's refresh.
// - the sidebar folds to a rail of icons, and the choice is remembered.
// - a palette (Ctrl or Cmd K, or `/`) lists the screens, the other surfaces and the panels
//   of this screen, read from the page's own links and headings, and hands anything else to
//   the search the top bar already has.

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

// ------------------------------------------------------------------ long tables

const TABLE_MIN_ROWS = 8;
const TABLE_PAGE = 25;
const tableChoices = new Map(); // table key -> { q, col, dir, page }
const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

function make(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function enhanceTable(table, key) {
  const body = table.tBodies[0];
  const head = table.tHead && table.tHead.rows[0];
  const wrap = table.closest('.mj-table-wrap');
  if (!body || !head || !wrap || 'enhanced' in table.dataset) return;
  const rows = [...body.rows];
  if (rows.length < TABLE_MIN_ROWS) return;
  table.dataset.enhanced = '';
  const texts = rows.map((r) => r.textContent.toLowerCase());
  const choice = tableChoices.get(key) || { q: '', col: -1, dir: 0, page: 0 };
  tableChoices.set(key, choice);

  const tools = make('div', 'mj-ui-table-tools');
  const id = `mj-ui-filter-${key.replace(/[^a-z0-9]+/gi, '-')}`;
  const label = make('label', 'mj-sr-only', `Filter these ${rows.length} rows`);
  label.htmlFor = id;
  const filter = make('input', 'mj-ui-table-filter');
  filter.type = 'search';
  filter.id = id;
  filter.placeholder = `Filter ${rows.length} rows…`;
  filter.value = choice.q;
  const count = make('p', 'mj-ui-table-count');
  count.setAttribute('aria-live', 'polite');
  const pager = make('div', 'mj-ui-table-pager');
  const prev = make('button', 'mj-ui-table-step', 'Previous');
  const next = make('button', 'mj-ui-table-step', 'Next');
  prev.type = 'button';
  next.type = 'button';
  pager.append(prev, next);
  tools.append(label, filter, count, pager);
  wrap.before(tools);

  const buttons = [...head.cells].map((th, col) => {
    const button = make('button', 'mj-ui-sort', th.textContent);
    button.type = 'button';
    button.title = 'Sort by this column';
    th.textContent = '';
    th.append(button);
    button.addEventListener('click', () => {
      // ascending, descending, then the order the server gave
      if (choice.col !== col) {
        choice.col = col;
        choice.dir = 1;
      } else if (choice.dir === 1) {
        choice.dir = -1;
      } else {
        choice.col = -1;
        choice.dir = 0;
      }
      choice.page = 0;
      apply();
    });
    return button;
  });

  function apply() {
    const words = choice.q.toLowerCase().split(/\s+/).filter(Boolean);
    const shown = rows.map((_, i) => i).filter((i) => words.every((w) => texts[i].includes(w)));
    if (choice.col >= 0) {
      const value = (i) => {
        const cell = rows[i].cells[choice.col];
        return cell ? cell.textContent.trim() : '';
      };
      shown.sort((a, b) => choice.dir * collator.compare(value(a), value(b)) || a - b);
    }
    const pages = Math.max(1, Math.ceil(shown.length / TABLE_PAGE));
    choice.page = Math.min(Math.max(choice.page, 0), pages - 1);
    const from = choice.page * TABLE_PAGE;
    const visible = new Set(shown.slice(from, from + TABLE_PAGE));
    for (const i of shown) body.append(rows[i]);
    rows.forEach((row, i) => {
      row.hidden = !visible.has(i);
    });
    [...head.cells].forEach((th, col) => {
      const sorted = col === choice.col ? (choice.dir === 1 ? 'ascending' : 'descending') : 'none';
      th.setAttribute('aria-sort', sorted);
      buttons[col].dataset.dir = sorted;
    });
    const to = Math.min(from + TABLE_PAGE, shown.length);
    const all = shown.length === rows.length ? '' : ` matching, ${rows.length} in all`;
    count.textContent = shown.length === 0
      ? `none of ${rows.length} rows match`
      : `${from + 1}–${to} of ${shown.length}${all}`;
    pager.hidden = pages === 1;
    prev.disabled = choice.page === 0;
    next.disabled = choice.page >= pages - 1;
  }

  filter.addEventListener('input', () => {
    choice.q = filter.value;
    choice.page = 0;
    apply();
  });
  prev.addEventListener('click', () => {
    choice.page -= 1;
    apply();
  });
  next.addEventListener('click', () => {
    choice.page += 1;
    apply();
  });
  apply();
}

function enhanceTables() {
  const seen = new Map();
  for (const table of document.querySelectorAll('#main .mj-table')) {
    // a table is known by the panel it stands in and its headers, so a refresh finds it again
    const panel = table.closest('.mj-ui-panel');
    const title = panel && panel.querySelector('.mj-ui-panel-title');
    const heads = table.tHead ? table.tHead.textContent.trim() : '';
    const base = `${window.location.pathname}|${title ? title.textContent.trim() : ''}|${heads}`;
    const n = seen.get(base) || 0;
    seen.set(base, n + 1);
    enhanceTable(table, `${base}|${n}`);
  }
}

// --------------------------------------------------------------- the sidebar's rail

const RAIL_KEY = 'mj-ui-rail';
const RAIL_CLASS = 'mj-ui-rail';

function wireRail() {
  const sidebar = document.querySelector('.mj-ui-sidebar');
  const nav = sidebar && sidebar.querySelector('.mj-ui-nav');
  if (!nav) return;
  let folded = false;
  try {
    folded = localStorage.getItem(RAIL_KEY) === '1';
  } catch (e) {
    /* storage refused: the rail still folds for this page */
  }
  const button = make('button', 'mj-ui-rail-toggle');
  button.type = 'button';
  const mark = make('span', 'mj-ui-rail-mark');
  mark.setAttribute('aria-hidden', 'true');
  const text = make('span', 'mj-ui-label');
  button.append(mark, text);
  const set = (value) => {
    folded = value;
    document.body.classList.toggle(RAIL_CLASS, folded);
    const say = folded ? 'Unfold the sidebar' : 'Fold the sidebar';
    button.setAttribute('aria-pressed', String(folded));
    button.setAttribute('aria-label', say);
    button.title = say;
    text.textContent = say;
  };
  button.addEventListener('click', () => {
    set(!folded);
    try {
      localStorage.setItem(RAIL_KEY, folded ? '1' : '0');
    } catch (e) {
      /* storage refused */
    }
  });
  set(folded);
  nav.after(button);
}

// -------------------------------------------------------------------- the palette

function paletteEntries() {
  const entries = [];
  for (const a of document.querySelectorAll('.mj-ui-nav-item')) {
    const group = a.closest('.mj-ui-nav-group');
    const heading = group && group.querySelector('.mj-ui-nav-heading');
    const note = [heading ? heading.textContent.trim() : '', a.title || ''].filter(Boolean).join(' · ');
    entries.push({ kind: 'Screen', name: a.textContent.trim(), note, href: a.getAttribute('href') });
  }
  for (const a of document.querySelectorAll('.mj-ui-tools a.mj-ui-tool')) {
    const href = a.getAttribute('href');
    entries.push({ kind: 'Surface', name: a.getAttribute('aria-label') || a.textContent.trim(), note: href, href });
  }
  for (const h of document.querySelectorAll('#main .mj-ui-panel-title')) {
    const panel = h.closest('.mj-ui-panel');
    const sub = panel && panel.querySelector('.mj-ui-panel-sub');
    entries.push({ kind: 'On this screen', name: h.textContent.trim(), note: sub ? sub.textContent.trim() : '', target: panel || h });
  }
  return entries;
}

function wirePalette() {
  const form = document.querySelector('.mj-ui-topbar .mj-ui-search');
  const field = form && form.querySelector('input[type="search"]');
  if (!form || !field || typeof HTMLDialogElement === 'undefined') return;
  const dialog = make('dialog', 'mj-ui-palette');
  dialog.setAttribute('aria-label', 'Go to a screen, a panel or a search');
  const input = make('input', 'mj-ui-palette-input');
  // a text field: in a search field the first Escape empties it, and the dialog stays open
  input.type = 'text';
  input.autocomplete = 'off';
  input.spellcheck = false;
  input.placeholder = 'Go to a screen or a panel, or search everything…';
  input.setAttribute('aria-label', 'Go to');
  input.setAttribute('role', 'combobox');
  input.setAttribute('aria-expanded', 'true');
  input.setAttribute('aria-controls', 'mj-ui-palette-list');
  const list = make('ul', 'mj-ui-palette-list');
  list.id = 'mj-ui-palette-list';
  list.setAttribute('role', 'listbox');
  list.setAttribute('aria-label', 'Places');
  dialog.append(input, list);
  document.body.append(dialog);

  const hint = make('button', 'mj-ui-kbd', /Mac|iPhone|iPad/.test(navigator.platform) ? '⌘K' : 'Ctrl K');
  hint.type = 'button';
  hint.title = 'Open the palette';
  hint.setAttribute('aria-label', 'Open the palette');
  form.append(hint);

  let entries = [];
  let shown = [];
  let active = 0;

  function go(entry) {
    dialog.close();
    if (entry.search !== undefined) {
      field.value = entry.search;
      form.submit();
    } else if (entry.target) {
      entry.target.scrollIntoView({ block: 'start' });
      if (!entry.target.hasAttribute('tabindex')) entry.target.setAttribute('tabindex', '-1');
      entry.target.focus({ preventScroll: true });
    } else if (entry.href) {
      window.location.assign(entry.href);
    }
  }

  function draw() {
    const q = input.value.trim();
    const words = q.toLowerCase().split(/\s+/).filter(Boolean);
    shown = entries.filter((e) => {
      const text = `${e.kind} ${e.name} ${e.note}`.toLowerCase();
      return words.every((w) => text.includes(w));
    });
    if (q) {
      shown.push({ kind: 'Search', name: `Search everything for “${q}”`, note: form.getAttribute('action') || '', search: q });
    }
    active = Math.min(active, Math.max(shown.length - 1, 0));
    list.textContent = '';
    shown.forEach((entry, i) => {
      const item = make('li', 'mj-ui-palette-item');
      item.id = `mj-ui-palette-${i}`;
      item.setAttribute('role', 'option');
      item.setAttribute('aria-selected', String(i === active));
      item.append(make('span', 'mj-ui-palette-kind', entry.kind), make('span', 'mj-ui-palette-name', entry.name));
      if (entry.note) item.append(make('span', 'mj-ui-palette-note', entry.note));
      item.addEventListener('click', () => go(entry));
      list.append(item);
    });
    if (shown.length) {
      input.setAttribute('aria-activedescendant', `mj-ui-palette-${active}`);
      const current = list.children[active];
      if (current) current.scrollIntoView({ block: 'nearest' });
    } else {
      input.removeAttribute('aria-activedescendant');
      list.append(make('li', 'mj-ui-palette-none', 'Nothing here is called that.'));
    }
  }

  function open() {
    if (dialog.open) return;
    entries = paletteEntries();
    input.value = '';
    active = 0;
    draw();
    dialog.showModal();
    input.focus();
  }

  input.addEventListener('input', () => {
    active = 0;
    draw();
  });
  input.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (shown.length) active = (active + (e.key === 'ArrowDown' ? 1 : shown.length - 1)) % shown.length;
      draw();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (shown[active]) go(shown[active]);
    }
  });
  // a click on the backdrop is a click on the dialog itself
  dialog.addEventListener('click', (e) => {
    if (e.target === dialog) dialog.close();
  });
  hint.addEventListener('click', open);
  document.addEventListener('keydown', (e) => {
    const at = document.activeElement;
    const typing = Boolean(at) && /^(INPUT|TEXTAREA|SELECT)$/.test(at.tagName);
    if ((e.key === 'k' || e.key === 'K') && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      open();
    } else if (e.key === '/' && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      open();
    }
  });
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
  // not under the reader's hands: a filter being typed into is not replaced mid-word
  const at = document.activeElement;
  if (at && main.contains(at) && at.matches('.mj-ui-table-filter')) return;
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

function enhance() {
  if (!SNAPSHOT) {
    for (const placeholder of document.querySelectorAll('[data-panel-src]')) {
      fillPanel(placeholder);
    }
  }
  drawGraphs();
  enhanceTables();
}

wireTheme();
wireRail();
if (!SNAPSHOT) wirePalette();
enhance();
if (SNAPSHOT) {
  showSnapshotPath();
  window.addEventListener('hashchange', showSnapshotPath);
} else if (!window.location.pathname.includes('/panels/')) {
  setInterval(refresh, REFRESH_MS);
  catchUp();
}
