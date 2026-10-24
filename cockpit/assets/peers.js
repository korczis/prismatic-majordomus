// The peers page keeps itself current (ADR 0176). The server rendered it from
// blackboard.snapshot and named on it the channel that says when the blackboard changes
// (`data-mj-peers-live`: the snapshot frames /ui/peers follows too). Every snapshot frame reads
// the page again from its own address, a connection's first one too: the blackboard may have
// changed between the page being served and its channel opening, and the server sends no
// snapshot that says nothing new, so a change in that gap would otherwise never reach a page
// that says it is live. The server renders the page, once and in one place: what is read
// replaces the element the page marks `data-mj-peers`, and nothing here draws a row or
// computes a fact.
//
// One reading runs at a time. A frame that arrives during one asks for one more after it, so
// an earlier answer that arrives last never replaces a later one, and a frame that arrives
// while the page is hidden is read once the page is shown again.
//
// The line the page marks `data-mj-peers-note` says how the page is fed, and never more than is
// true: live while the socket is open, reconnecting while it is not, offline when the channel
// is closed or cannot be opened at all, and that the page shows the reading before when the
// last one failed. It is an enhancement: without JavaScript the page is complete, and a reload
// reads it again.

import { follow, LIVE, RECONNECTING } from './socket.js?v=ba950eef18e47db4';

/** What the state line says, by the state of the channel. */
export const SAID = {
  connecting: 'Opening the live channel; until it is open this page is as it was served.',
  live: 'Live: the server says when the blackboard changes, and this page is read again.',
  reconnecting: 'Reconnecting to the live channel; this page is as it was last read.',
  offline:
    'Offline: the live channel is closed, and this page is as it was last read. ' +
    'Reload it to read it again.',
};

/** What the line adds while the newest reading of the page has failed. */
export const UNREAD = ' The last reading of this page failed: it shows the one before.';

/** How long one reading of the page may take before it is given up. Declared before the
 * first call below: a `const` is not hoisted. */
const READ_MS = 30000;

const region = document.querySelector('[data-mj-peers]');
if (region) {
  watch(region.dataset.mjPeersLive || '');
}

function watch(path) {
  let state = 'connecting';
  let failed = false;
  // the line is part of what a reading replaces, so it is written again after every one
  const say = () => {
    const line = document.querySelector('[data-mj-peers-note]');
    if (line) line.textContent = SAID[state] + (failed ? UNREAD : '');
  };
  if (!path || typeof WebSocket === 'undefined') {
    state = 'offline';
    say();
    return;
  }
  say();

  async function read() {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), READ_MS);
    try {
      const response = await fetch(`${window.location.pathname}${window.location.search}`, {
        headers: { Accept: 'text/html' },
        signal: controller.signal,
      });
      if (!response.ok) throw new Error(`the page answered ${response.status}`);
      const page = new DOMParser().parseFromString(await response.text(), 'text/html');
      const fresh = page.querySelector('[data-mj-peers]');
      const current = document.querySelector('[data-mj-peers]');
      if (!fresh || !current) throw new Error('the page holds no blackboard');
      current.replaceWith(fresh);
      failed = false;
    } catch (e) {
      // the page as it was stays, and says so; the next frame reads it again
      failed = true;
    } finally {
      clearTimeout(timer);
    }
    say();
  }

  let reading = null;
  let again = false;
  let missed = false;
  async function refresh() {
    if (document.visibilityState !== 'visible') {
      missed = true;
      return;
    }
    if (reading) {
      again = true;
      return;
    }
    reading = (async () => {
      do {
        again = false;
        await read();
      } while (again);
    })();
    try {
      await reading;
    } finally {
      reading = null;
    }
  }

  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible' && missed) {
      missed = false;
      refresh();
    }
  });

  follow(path, {
    message(frame) {
      if (frame.type !== 'blackboard.snapshot' || !frame.data) return;
      refresh();
    },
    state(name) {
      if (name === LIVE) state = 'live';
      else if (name === RECONNECTING) state = 'reconnecting';
      else state = 'offline';
      say();
    },
  });
}
