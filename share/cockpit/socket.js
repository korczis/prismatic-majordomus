// The one WebSocket client of the Cockpit. Nothing else in the browser opens a socket:
// a page asks for a subscription, this module keeps exactly one connection per
// subscription, reconnects it with a bounded backoff, and hands frames to whoever asked.
//
// It speaks the protocol the server describes at /api/v1/executions/protocol, and every
// URL it uses is given to it by the server-rendered markup — there is no path written
// down here.

/** How long to wait before the first reconnection attempt. */
const FIRST_DELAY_MS = 500;

/** The longest wait between attempts; a browser tab left open must not become a heater. */
const MAX_DELAY_MS = 30_000;

/** What a connection is doing, for the indicator a page shows. */
export const LIVE = 'live';
/** Trying to get back. */
export const RECONNECTING = 'reconnecting';
/** Not connected and not trying: the page still shows its last known state. */
export const OFFLINE = 'offline';

/**
 * Open one live channel.
 *
 * @param {string} path the request target the server rendered, e.g. `/events?execution=x-…`
 * @param {object} handlers `{ message(frame), state(name), resync(reason) }`
 * @returns {{ close(): void, state(): string }}
 */
export function follow(path, handlers) {
  let socket = null;
  let closed = false;
  let attempt = 0;
  let state = OFFLINE;
  let cursor = cursorOf(path);

  const announce = (next) => {
    if (state === next) return;
    state = next;
    if (handlers.state) handlers.state(next);
  };

  const url = () => {
    const target = new URL(path, window.location.href);
    target.protocol = target.protocol === 'https:' ? 'wss:' : 'ws:';
    // a reconnection asks for what it missed rather than for everything again
    if (cursor > 0 && target.searchParams.has('execution')) {
      target.searchParams.set('after', String(cursor));
    }
    return target.toString();
  };

  const open = () => {
    if (closed) return;
    let ws;
    try {
      ws = new WebSocket(url());
    } catch (e) {
      schedule();
      return;
    }
    socket = ws;
    ws.addEventListener('open', () => {
      attempt = 0;
      announce(LIVE);
    });
    ws.addEventListener('message', (event) => {
      let frame;
      try {
        frame = JSON.parse(event.data);
      } catch (e) {
        return; // a frame this client cannot read is ignored, never thrown
      }
      if (typeof frame.sequence === 'number' && frame.sequence > cursor) {
        cursor = frame.sequence;
      }
      if (frame.type === 'stream.lagged') {
        // the server dropped events for this client rather than buffering them: the
        // page reads the snapshot and the history again from where it is
        if (handlers.resync) handlers.resync('lagged');
        return;
      }
      if (frame.type === 'stream.closing') {
        announce(OFFLINE);
        return;
      }
      if (frame.type === 'stream.ready') {
        if (handlers.ready) handlers.ready(frame.data);
        return;
      }
      if (handlers.message) handlers.message(frame);
    });
    ws.addEventListener('close', () => {
      socket = null;
      schedule();
    });
    ws.addEventListener('error', () => {
      // 'close' follows; nothing to do here but keep the console quiet
    });
  };

  const schedule = () => {
    if (closed) {
      announce(OFFLINE);
      return;
    }
    announce(RECONNECTING);
    // exponential, capped, with jitter so that many tabs do not knock together
    const base = Math.min(FIRST_DELAY_MS * 2 ** attempt, MAX_DELAY_MS);
    const wait = base / 2 + Math.random() * (base / 2);
    attempt += 1;
    setTimeout(() => {
      if (handlers.resync) handlers.resync('reconnect');
      open();
    }, wait);
  };

  open();

  return {
    close() {
      closed = true;
      announce(OFFLINE);
      if (socket) socket.close();
    },
    state() {
      return state;
    },
  };
}

function cursorOf(path) {
  try {
    const after = new URL(path, window.location.href).searchParams.get('after');
    return after ? Number(after) || 0 : 0;
  } catch (e) {
    return 0;
  }
}
