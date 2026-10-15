// The Observatory's live channel: the executions of this process, heard as they happen.
//
// The server already pushes every execution event over one WebSocket, `/events`
// (apps/majordomus-cli/src/http/events.rs); this file is the Observatory's reader of it. It
// opens no other channel and keeps no store of its own: the picture is the snapshot the
// server answers (`executions.list`) brought forward by the events that arrive after it.
//
// It follows the rules the Cockpit's client (share/cockpit/socket.js) follows for the same
// protocol: sequences are dense per execution, so an event at or below the last one seen is
// a duplicate and is dropped, and one beyond the next is a gap; a gap, or a `stream.lagged`
// from the server, is answered by reading the snapshot again rather than by guessing; a
// channel silent for a few heartbeats is dead, whatever its socket says; reconnection backs
// off exponentially with jitter. It never writes a frame: the channel is read-only.
//
// The reducer is pure — the same snapshot and the same events give the same state, whatever
// the order duplicates arrive in — so it is tested without a browser (case 3046). Nothing in
// this file touches the document; the Observatory's renderer reads `state()` and draws it.
(function (root) {
  'use strict';

  const FIRST_DELAY_MS = 500;
  const MAX_DELAY_MS = 30000;
  // How many heartbeat intervals of silence make a channel dead rather than quiet.
  const SILENT_HEARTBEATS = 2.5;
  const DEFAULT_HEARTBEAT_SECONDS = 20;
  // After this long without a live channel the picture is STALE, not RECONNECTING: the
  // display stops implying that it is about to be current again.
  const STALE_AFTER_MS = 30000;
  // How many executions the picture holds at most: the server's bound on one scene.
  const BOUND = 200;

  // The state an event moves an execution to. An event that is not here (a log line, a
  // progress report, a step) says the execution is alive and leaves its state as it was.
  const STATE_OF = {
    'execution.created': 'queued',
    'execution.queued': 'queued',
    'execution.started': 'running',
    'execution.cancelling': 'cancelling',
    'execution.completed': 'succeeded',
    'execution.failed': 'failed',
    'execution.cancelled': 'cancelled',
  };

  // ---------------------------------------------------------------- the pure part

  // One lane from an execution as executions.list answers it, or as the page's projection
  // carries it: its identity, the capability it runs, its state and its last sequence.
  function lane(e) {
    return {
      id: String(e.id),
      capability: String(e.capability || ''),
      state: String(e.state || 'unknown'),
      last_sequence: Number(e.last_sequence) || 0,
      events: Number(e.event_count || e.events) || 0,
    };
  }

  // The state a snapshot gives: the executions it lists, newest first as it listed them.
  function fromSnapshot(executions) {
    const lanes = {};
    const order = [];
    for (const e of (executions || []).slice(0, BOUND)) {
      const l = lane(e);
      if (lanes[l.id]) continue;
      lanes[l.id] = l;
      order.push(l.id);
    }
    return { lanes, order, heard: 0 };
  }

  // Fold one frame into a state. Returns `{ state, outcome }`, the input left untouched:
  //   applied    the event was the next one of its execution (or the first of a new one)
  //   duplicate  at or below the last sequence seen: dropped
  //   gap        beyond the next one, or the middle of an execution the snapshot did not
  //              list: the state is unchanged and the caller reads the snapshot again
  //   ignored    not an execution event
  function reduce(state, frame) {
    if (!frame || typeof frame.execution_id !== 'string' || typeof frame.sequence !== 'number') {
      return { state, outcome: 'ignored' };
    }
    const id = frame.execution_id;
    const known = state.lanes[id];
    const last = known ? known.last_sequence : 0;
    if (frame.sequence <= last) return { state, outcome: 'duplicate' };
    if (frame.sequence !== last + 1) return { state, outcome: 'gap' };
    const next = known
      ? { ...known }
      : { id, capability: String((frame.data && frame.data.capability) || ''), state: 'queued', last_sequence: 0, events: 0 };
    next.last_sequence = frame.sequence;
    next.events += 1;
    if (STATE_OF[frame.type]) next.state = STATE_OF[frame.type];
    if (!next.capability && frame.data && frame.data.capability) next.capability = String(frame.data.capability);
    const lanes = { ...state.lanes, [id]: next };
    let order = known ? state.order : [id, ...state.order];
    if (order.length > BOUND) {
      for (const dropped of order.slice(BOUND)) delete lanes[dropped];
      order = order.slice(0, BOUND);
    }
    return { state: { lanes, order, heard: state.heard + 1 }, outcome: 'applied' };
  }

  // The lanes in order, as a list a renderer draws.
  function lanesOf(state) {
    return state.order.map((id) => state.lanes[id]).filter(Boolean);
  }

  // ---------------------------------------------------------------- the channel

  // Follow the live channel. `options`:
  //   base      the origin to connect to (the page's own, by default)
  //   path      the channel's request target, as the page rendered it (`/events`)
  //   snapshot  the URL of executions.list over HTTP
  //   initial   the executions the page already carries, so the first picture is not empty
  //   on        `{ change(state, event), status(word) }`, both optional
  //   staleAfterMs  how long without a live channel before the word is STALE (30 s)
  //   WebSocket, fetch, setTimeout, clearTimeout, now: injectable for tests
  function connect(options) {
    const o = options || {};
    const WS = o.WebSocket || root.WebSocket;
    const get = o.fetch || (root.fetch && root.fetch.bind(root));
    const later = o.setTimeout || root.setTimeout.bind(root);
    const cancel = o.clearTimeout || root.clearTimeout.bind(root);
    const now = o.now || (() => Date.now());
    const on = o.on || {};
    const base = o.base || (root.location && root.location.href) || 'http://127.0.0.1/';
    const staleAfter = Number(o.staleAfterMs) > 0 ? Number(o.staleAfterMs) : STALE_AFTER_MS;

    let state = fromSnapshot(o.initial || []);
    let status = 'reconnecting';
    let socket = null;
    let closed = false;
    let attempt = 0;
    let heartbeat = DEFAULT_HEARTBEAT_SECONDS;
    let watchdog = null;
    let lostAt = now();
    // frames that arrived before the snapshot they follow was read
    let held = null;

    const announce = (word) => {
      if (status === word) return;
      status = word;
      if (on.status) on.status(word);
    };
    const changed = (event) => { if (on.change) on.change(state, event); };

    const url = () => {
      const target = new URL(o.path || '/events', base);
      target.protocol = target.protocol === 'https:' ? 'wss:' : 'ws:';
      return target.toString();
    };

    const disarm = () => { if (watchdog !== null) cancel(watchdog); watchdog = null; };
    const arm = () => {
      disarm();
      watchdog = later(() => { watchdog = null; if (socket) retire(socket); }, heartbeat * SILENT_HEARTBEATS * 1000);
    };

    function fold(frame) {
      const r = reduce(state, frame);
      if (r.outcome === 'applied') { state = r.state; changed(frame); }
      else if (r.outcome === 'gap') resnapshot();
    }

    // read the snapshot again; the frames that arrive meanwhile are held and folded after it
    async function resnapshot() {
      if (held !== null || !o.snapshot || !get) return;
      held = [];
      try {
        const response = await get(new URL(o.snapshot, base).toString(), { headers: { Accept: 'application/json' } });
        if (!response.ok) throw new Error(String(response.status));
        const body = await response.json();
        state = fromSnapshot(body.executions || []);
        changed(null);
      } catch (e) {
        // the picture stays what it was; the next gap or reconnect asks again
      }
      const pending = held;
      held = null;
      for (const f of pending) fold(f);
    }

    function retire(ws) {
      if (socket !== ws) return;
      socket = null;
      disarm();
      try { ws.close(); } catch (e) { /* already gone */ }
      schedule();
    }

    function open() {
      if (closed) return;
      let ws;
      try {
        ws = new WS(url());
      } catch (e) {
        schedule();
        return;
      }
      socket = ws;
      arm();
      ws.addEventListener('message', (event) => {
        if (socket !== ws) return;
        arm();
        let frame;
        try { frame = JSON.parse(event.data); } catch (e) { return; }
        if (frame.type === 'stream.heartbeat') return;
        if (frame.type === 'stream.ready') {
          const d = frame.data || {};
          if (Number(d.heartbeat_seconds) > 0) heartbeat = Number(d.heartbeat_seconds);
          attempt = 0;
          announce('live');
          // subscribed first, then the snapshot: nothing between them is missed, and what
          // both carry is a duplicate the reducer drops
          resnapshot();
          return;
        }
        if (frame.type === 'stream.lagged') { resnapshot(); return; }
        if (frame.type === 'stream.closing') { retire(ws); return; }
        if (held !== null) { held.push(frame); return; }
        fold(frame);
      });
      // a browser follows a failed connection's 'error' with 'close'; some runtimes report
      // only the 'error', so either one ends this socket, and only the first one counts
      const lost = () => { if (socket === ws) { socket = null; disarm(); schedule(); } };
      ws.addEventListener('close', lost);
      ws.addEventListener('error', lost);
    }

    let staleTimer = null;
    function schedule() {
      if (closed) return;
      if (status === 'live') {
        lostAt = now();
        // the word turns STALE on time, whether or not an attempt happens to end then
        if (staleTimer !== null) cancel(staleTimer);
        staleTimer = later(() => { staleTimer = null; if (!closed && status !== 'live') announce('stale'); }, staleAfter);
      }
      announce(now() - lostAt >= staleAfter ? 'stale' : 'reconnecting');
      const ceiling = Math.min(FIRST_DELAY_MS * 2 ** attempt, MAX_DELAY_MS);
      const wait = ceiling / 2 + Math.random() * (ceiling / 2);
      attempt += 1;
      later(() => {
        if (now() - lostAt >= staleAfter) announce('stale');
        open();
      }, wait);
    }

    if (!WS) announce('stale');
    else open();

    return {
      state: () => state,
      lanes: () => lanesOf(state),
      status: () => status,
      close() {
        closed = true;
        disarm();
        if (socket) { const ws = socket; socket = null; try { ws.close(); } catch (e) { /* gone */ } }
        announce('stale');
      },
    };
  }

  const api = { fromSnapshot, reduce, lanesOf, connect, STATE_OF };
  root.mjObservatoryStream = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
