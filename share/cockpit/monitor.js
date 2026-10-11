// The monitor page keeps itself current (ADR 0133). The server rendered the first page and
// put its cursor on the frame; while the page is visible this asks /api/v1/monitor for the
// page after that cursor every few seconds and adds each new signal as a row on top. The
// cursor carries every source's position, so nothing comes twice and nothing is computed
// here: a row is the capability's signal, written with textContent only.
//
// It is an enhancement: without JavaScript the page is complete and a refresh shows the
// newest signals.

import { api } from './cockpit.js';

/** How often the stream is asked for, while the page is visible. Declared before the
 * first call below: a `const` is not hoisted. */
const INTERVAL_MS = 5000;

/** The most rows the live list keeps; older ones fall off the bottom. */
const MAX_ROWS = 500;

const frame = document.querySelector('[data-mj-monitor]');
if (frame) {
  follow(frame);
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

function follow(frame) {
  const source = frame.dataset.mjMonitor;
  let cursor = frame.dataset.mjMonitorCursor || '';
  const note = frame.querySelector('[data-mj-monitor-note]');
  let polling = 0;
  let busy = false;

  async function sample() {
    if (busy) return;
    busy = true;
    try {
      const url = cursor ? `${source}&after=${encodeURIComponent(cursor)}` : source;
      const answer = await api(url);
      if (!answer.ok || !answer.body) {
        if (note) note.textContent = 'The stream could not be read; the list shows the last answer.';
        return;
      }
      const page = answer.body;
      const signals = page.signals || [];
      if (signals.length) {
        const tbody = body(frame);
        // oldest first in the answer: each goes on top, so the newest ends up first
        for (const signal of signals) tbody.prepend(row(signal));
        while (tbody.rows.length > MAX_ROWS) tbody.deleteRow(-1);
      }
      if (page.cursor) cursor = page.cursor;
      if (note) {
        note.textContent = `Live: checked ${new Date().toLocaleTimeString()}; ${signals.length} new.`;
      }
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
