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
// A channel that follows the blackboard (`follow=blackboard`, ADR 0176) has no sequence:
// its cursor is the opaque string of the last `blackboard.signals` frame, good only for the
// server process whose epoch `stream.ready` named. A reconnection sends both, and a cursor
// the server cannot serve is answered with `stream.resync`, which is handed to the page.
//
// It speaks the protocol the server describes at /api/v1/executions/protocol, and every
// URL it uses is given to it by the server-rendered markup — there is no path written
// down here.

/** How long to wait before the first reconnection attempt. */
const FIRST_DELAY_MS = 500;

/** The longest wait between attempts; a browser tab left open must not become a heater. */
const MAX_DELAY_MS = 30_000;

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
  // a blackboard channel resumes by the opaque cursor and the epoch that issued it
  const board = followsBoard(path);
  let tail = board ? paramOf(path, 'after') : '';
  let epoch = board ? paramOf(path, 'epoch') : '';
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
    if (board) {
      target.searchParams.delete('after');
      target.searchParams.delete('epoch');
      if (tail && epoch) {
        target.searchParams.set('after', tail);
        target.searchParams.set('epoch', epoch);
      }
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
      attempt = 0;
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
        const data = frame.data || {};
        if (Number(data.heartbeat_seconds) > 0) heartbeat = Number(data.heartbeat_seconds);
        rebase = data.truncated === true;
        // a cursor of another process is worth nothing here: the server says resync
        if (board && typeof data.epoch === 'string') epoch = data.epoch;
        arm();
        if (handlers.ready) handlers.ready(data);
        return;
      }
      if (frame.type === 'stream.resync') {
        // the server could not serve the cursor and starts again from what it holds now:
        // the page's picture before this frame may miss something
        tail = '';
        if (handlers.resync) handlers.resync((frame.data && frame.data.reason) || 'resync');
        return;
      }
      if (board && frame.data && typeof frame.data.cursor === 'string') {
        tail = frame.data.cursor;
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

function followsBoard(path) {
  return paramOf(path, 'follow') === 'blackboard';
}

function paramOf(path, name) {
  try {
    return new URL(path, window.location.href).searchParams.get(name) || '';
  } catch (e) {
    return '';
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
