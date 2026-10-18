// The monitor page keeps itself current (ADR 0133, ADR 0176). The server rendered the first
// page and put on the frame its cursor and the live channel that continues from it: while the
// page is open the server pushes each new signal over that channel the moment the board or
// the mesh journal changes, and this adds it as a row on top. The cursor carries every
// source's position, so nothing comes twice and nothing is computed here: a row is the
// capability's signal, written with textContent only.
//
// A page whose channel cannot be opened — a proxy that does not carry WebSockets, a server
// that refuses it — asks /api/v1/monitor for the page after the cursor every few seconds
// instead, and says that it polls. It is an enhancement: without JavaScript the page is
// complete and a refresh shows the newest signals.

import { api } from './cockpit.js';
import { follow as channel, LIVE, RECONNECTING } from './socket.js';

/** How often the stream is asked for when the channel cannot be opened, while the page is
 * visible. Declared before the first call below: a `const` is not hoisted. */
const INTERVAL_MS = 5000;

/** How long the channel may take to open before the page polls instead. */
const OPEN_MS = 5000;

/** The most rows the live list keeps; older ones fall off the bottom. */
const MAX_ROWS = 500;

const frame = document.querySelector('[data-mj-monitor]');
if (frame) {
  live(frame);
}

function clock(at) {
  const day = at % 86400;
  const two = (n) => String(n).padStart(2, '0');
  return `${two(Math.floor(day / 3600))}:${two(Math.floor((day % 3600) / 60))}:${two(day % 60)}`;
}

function cell(text) {
  const td = document.createElement('td');
  td.textContent = text;
  return td;
}

function row(signal) {
  const tr = document.createElement('tr');
  const mark = signal.severity === 'info' ? '' : signal.severity;
  tr.dataset.mjSeverity = mark;
  tr.dataset.mjForYou = signal.for_you ? 'yes' : 'no';
  const time = cell(clock(signal.at || 0));
  time.className = 'mj-mono';
  tr.append(time, cell(signal.source || ''), cell(signal.machine || ''));
  const severity = document.createElement('td');
  if (mark) {
    const badge = document.createElement('span');
    badge.className = 'mj-tag';
    badge.textContent = mark;
    severity.append(badge);
  }
  tr.append(severity);
  const what = document.createElement('td');
  if (signal.link) {
    const a = document.createElement('a');
    a.href = signal.link;
    a.textContent = signal.summary || '';
    what.append(a);
  } else {
    what.textContent = signal.summary || '';
  }
  tr.append(what);
  return tr;
}

function body(frame) {
  let tbody = frame.querySelector('tbody');
  if (tbody) return tbody;
  // the first page was empty: build the table the server would have rendered
  const empty = frame.querySelector('[data-mj-monitor-empty]');
  const wrap = document.createElement('div');
  wrap.className = 'mj-table-wrap';
  wrap.tabIndex = 0;
  const table = document.createElement('table');
  table.className = 'mj-table';
  const head = document.createElement('tr');
  for (const h of ['Time (UTC)', 'Source', 'Machine', '', 'What happened']) {
    const th = document.createElement('th');
    th.scope = 'col';
    th.textContent = h;
    head.append(th);
  }
  const thead = document.createElement('thead');
  thead.append(head);
  tbody = document.createElement('tbody');
  table.append(thead, tbody);
  wrap.append(table);
  if (empty) empty.replaceWith(wrap);
  else frame.append(wrap);
  return tbody;
}

function add(frame, signals) {
  if (!signals.length) return;
  const tbody = body(frame);
  // oldest first in the answer: each goes on top, so the newest ends up first
  for (const signal of signals) tbody.prepend(row(signal));
  while (tbody.rows.length > MAX_ROWS) tbody.deleteRow(-1);
}

function live(frame) {
  const path = frame.dataset.mjMonitorLive;
  const note = frame.querySelector('[data-mj-monitor-note]');
  const say = (text) => {
    if (note) note.textContent = text;
  };
  if (!path || typeof WebSocket === 'undefined') {
    poll(frame, frame.dataset.mjMonitorCursor || '', say);
    return;
  }
  let opened = false;
  const followed = channel(path, {
    message(f) {
      if (f.type !== 'blackboard.signals' || !f.data) return;
      const signals = f.data.signals || [];
      add(frame, signals);
      const at = new Date().toLocaleTimeString();
      say(`Live: pushed by the server; ${signals.length} new at ${at}.`);
    },
    state(name) {
      if (name === LIVE) {
        opened = true;
        say('Live: the server pushes what happens as it happens.');
      } else if (name === RECONNECTING && opened) {
        say('Reconnecting to the live channel; the list shows the last signals it pushed.');
      }
    },
    resync(reason) {
      // a reconnection resumes from the channel's own cursor: nothing is missing
      if (reason === 'reconnect') return;
      say(
        `The server could not continue from this page's cursor (${reason}); ` +
          'a refresh shows what may be missing.',
      );
    },
  });
  // a channel that never opens is not waited for: the page polls, and says so
  window.setTimeout(() => {
    if (opened) return;
    followed.close();
    poll(frame, frame.dataset.mjMonitorCursor || '', say);
  }, OPEN_MS);
}

function poll(frame, from, say) {
  const source = frame.dataset.mjMonitor;
  let cursor = from;
  let polling = 0;
  let busy = false;

  async function sample() {
    if (busy) return;
    busy = true;
    try {
      const url = cursor ? `${source}&after=${encodeURIComponent(cursor)}` : source;
      const answer = await api(url);
      if (!answer.ok || !answer.body) {
        say('Polling: the stream could not be read; the list shows the last answer.');
        return;
      }
      const page = answer.body;
      const signals = page.signals || [];
      add(frame, signals);
      if (page.cursor) cursor = page.cursor;
      const at = new Date().toLocaleTimeString();
      say(
        `Polling every ${INTERVAL_MS / 1000} s, the live channel could not be opened: ` +
          `checked ${at}; ${signals.length} new.`,
      );
      if (page.more) window.setTimeout(sample, 0);
    } finally {
      busy = false;
    }
  }

  const start = () => {
    if (polling) return;
    sample();
    polling = window.setInterval(sample, INTERVAL_MS);
  };
  const stop = () => {
    window.clearInterval(polling);
    polling = 0;
  };
  // never poll a server for a page nobody is looking at
  const decide = () => (document.visibilityState === 'visible' ? start() : stop());
  document.addEventListener('visibilitychange', decide);
  decide();
}
