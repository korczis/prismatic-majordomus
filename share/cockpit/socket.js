// The one WebSocket client of the Cockpit. Nothing else in the browser opens a socket:
// a page asks for a subscription, this module keeps exactly one connection per
// subscription, reconnects it with a bounded backoff, and hands frames to whoever asked.
//
// It hands each event over once and in order. Sequences are dense per execution, so an
// event at or below the last one seen is a duplicate and is dropped, and one beyond the
// next is a gap: a channel that follows one execution reconnects from the last sequence it
// saw and the server replays what was missed. And it notices a dead channel: the server
// writes `stream.heartbeat` on every quiet interval, so a channel silent for a few of them
// is retired and reconnected from its cursor instead of showing "live" forever.
//
// It speaks the protocol the server describes at /api/v1/executions/protocol, and every
// URL it uses is given to it by the server-rendered markup — there is no path written
// down here.

/** How long to wait before the first reconnection attempt. */
const FIRST_DELAY_MS = 500;

/** The longest wait between attempts; a browser tab left open must not become a heater. */
const MAX_DELAY_MS = 30_000;

/** How many attempts in a row may fail before the channel stops trying and reads offline:
 * about two minutes of backoff, after which the page says to reload (I1017). */
export const MAX_ATTEMPTS = 8;

/**
 * The wait before attempt `attempt` (0 is the first retry): exponential, capped at
 * MAX_DELAY_MS, with jitter in its upper half so that many tabs do not knock together.
 * `random` is a number in [0, 1).
 */
export function backoff(attempt, random = Math.random()) {
  const base = Math.min(FIRST_DELAY_MS * 2 ** attempt, MAX_DELAY_MS);
  return base / 2 + random * (base / 2);
}

/** How many heartbeat intervals of silence make a channel dead rather than quiet. */
const SILENT_HEARTBEATS = 2.5;

/** The heartbeat assumed until the server's `stream.ready` names its own, in seconds. */
const DEFAULT_HEARTBEAT_SECONDS = 20;

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
 * @param {object} handlers `{ message(frame), state(name), resync(reason), ready(data) }`
 * @returns {{ close(): void, state(): string }}
 */
export function follow(path, handlers) {
  let socket = null;
  let closed = false;
  let attempt = 0;
  let state = OFFLINE;
  // a channel that follows exactly one execution has one cursor, and the server replays
  // from it; any other channel keeps the last sequence of each execution to drop repeats
  const single = followsOne(path);
  let cursor = cursorOf(path);
  const last = new Map();
  // the replay began after the cursor's history was already dropped: the first event it
  // writes starts the picture, and is not a gap
  let rebase = false;
  // the cursor a gap last reconnected from; the same gap twice is not asked a third time
  let gapAt = -1;
  let heartbeat = DEFAULT_HEARTBEAT_SECONDS;
  let watchdog = null;

  const announce = (next) => {
    if (state === next) return;
    state = next;
    if (handlers.state) handlers.state(next);
  };

  const url = () => {
    const target = new URL(path, window.location.href);
    target.protocol = target.protocol === 'https:' ? 'wss:' : 'ws:';
    // a reconnection asks for what it missed rather than for everything again
    if (cursor > 0 && single) {
      target.searchParams.set('after', String(cursor));
    }
    return target.toString();
  };

  const disarm = () => {
    if (watchdog !== null) clearTimeout(watchdog);
    watchdog = null;
  };

  // nothing heard for a few heartbeats: the channel is dead, whatever its state says
  const arm = () => {
    disarm();
    watchdog = setTimeout(() => {
      watchdog = null;
      if (socket) retire(socket);
    }, heartbeat * SILENT_HEARTBEATS * 1000);
  };

  // stop listening to a socket before closing it — a half-open one can take minutes to
  // report its close — and reconnect from the cursor
  const retire = (ws) => {
    if (socket !== ws) return;
    socket = null;
    disarm();
    try {
      ws.close();
    } catch (e) {
      // closing a socket that is already gone has nothing to say
    }
    schedule();
  };

  // whether an event is the next one: `true` hands it over, `false` drops it
  const admit = (ws, frame) => {
    const id = frame.execution_id;
    const previous = single ? cursor : last.get(id) || 0;
    if (frame.sequence <= previous) return false; // already handed over
    if (previous > 0 && frame.sequence !== previous + 1 && !rebase) {
      if (single && gapAt !== previous) {
        // the server replays from the cursor: ask it for what is missing
        gapAt = previous;
        retire(ws);
        return false;
      }
      // a gap no replay fills — asked once already, or a channel with no cursor: the
      // page reads the history over HTTP
      if (handlers.resync) handlers.resync('gap');
    }
    rebase = false;
    if (single) cursor = frame.sequence;
    else last.set(id, frame.sequence);
    return true;
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
    arm();
    ws.addEventListener('open', () => {
      if (socket !== ws) return;
      announce(LIVE);
      arm();
    });
    ws.addEventListener('message', (event) => {
      if (socket !== ws) return; // a retired socket says nothing more
      arm();
      let frame;
      try {
        frame = JSON.parse(event.data);
      } catch (e) {
        return; // a frame this client cannot read is ignored, never thrown
      }
      if (frame.type === 'stream.heartbeat') return; // it kept the watchdog quiet
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
        // the backoff starts over only once the server has spoken the protocol: one that
        // accepts every connection and drops it at once would otherwise be asked again
        // every half second, forever
        attempt = 0;
        const data = frame.data || {};
        if (Number(data.heartbeat_seconds) > 0) heartbeat = Number(data.heartbeat_seconds);
        rebase = data.truncated === true;
        arm();
        if (handlers.ready) handlers.ready(data);
        return;
      }
      if (typeof frame.sequence === 'number' && !admit(ws, frame)) return;
      if (handlers.message) handlers.message(frame);
    });
    ws.addEventListener('close', () => {
      if (socket !== ws) return;
      socket = null;
      disarm();
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
    // a bounded loop: past the last attempt the page shows what it last knew and says so
    if (attempt >= MAX_ATTEMPTS) {
      closed = true;
      announce(OFFLINE);
      return;
    }
    announce(RECONNECTING);
    const wait = backoff(attempt);
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
      disarm();
      announce(OFFLINE);
      if (socket) socket.close();
    },
    state() {
      return state;
    },
  };
}

function followsOne(path) {
  try {
    return new URL(path, window.location.href).searchParams.getAll('execution').length === 1;
  } catch (e) {
    return false;
  }
}

function cursorOf(path) {
  try {
    const after = new URL(path, window.location.href).searchParams.get('after');
    return after ? Number(after) || 0 : 0;
  } catch (e) {
    return 0;
  }
}
