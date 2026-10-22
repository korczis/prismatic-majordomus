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
// from the stylesheet, so nothing here names a colour.
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
