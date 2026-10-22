// The Cockpit's one WebSocket client reconnects within a bound (share/cockpit/socket.js,
// I1017): a server that refuses every connection, or accepts each one and drops it at once,
// is asked a bounded number of times with a growing wait and the channel then reads offline;
// a server that speaks the protocol before it drops starts the backoff over. The browser is
// a fixture written here: a fake WebSocket, a fake clock, and no network.
//
//   node --test scripts/lib/cockpit-socket.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';

// what socket.js reads from the page, before it is imported
globalThis.window = { location: { href: 'http://127.0.0.1:1/cockpit/executions' } };

let now = 0;
let timers = [];
let nextId = 1;
globalThis.setTimeout = (fn, ms) => {
  const id = nextId++;
  timers.push({ id, at: now + (ms || 0), fn });
  return id;
};
globalThis.clearTimeout = (id) => {
  timers = timers.filter((t) => t.id !== id);
};

/** Run timers in time order until none is left or `limit` have run; returns how many ran. */
function drain(limit = 10_000) {
  let ran = 0;
  while (timers.length && ran < limit) {
    timers.sort((a, b) => a.at - b.at);
    const t = timers.shift();
    now = t.at;
    t.fn();
    ran += 1;
  }
  return ran;
}

/** A WebSocket whose server behaves as `serve` says, one tick after it is opened. */
function fakeServer(serve) {
  const opened = [];
  globalThis.WebSocket = class {
    constructor(url) {
      this.url = url;
      this.listeners = {};
      opened.push({ socket: this, at: now });
      setTimeout(() => serve(this), 0);
    }
    addEventListener(type, fn) {
      (this.listeners[type] ||= []).push(fn);
    }
    fire(type, event = {}) {
      for (const fn of this.listeners[type] || []) fn(event);
    }
    close() {}
  };
  return opened;
}

const { follow, backoff, MAX_ATTEMPTS, OFFLINE, RECONNECTING } = await import(
  '../../share/cockpit/socket.js'
);

function reset() {
  now = 0;
  timers = [];
}

test('the wait grows, is capped, and keeps its jitter in the upper half', () => {
  assert.equal(backoff(0, 0), 250);
  assert.ok(backoff(0, 0.999) < 500);
  assert.ok(backoff(3, 0) > backoff(2, 0));
  assert.equal(backoff(30, 0), 15_000);
  assert.ok(backoff(30, 0.999) < 30_000);
});

test('a server that refuses every connection is asked a bounded number of times', () => {
  reset();
  const opened = fakeServer((ws) => ws.fire('close'));
  const states = [];
  follow('/events', { state: (s) => states.push(s) });
  drain();
  assert.equal(opened.length, 1 + MAX_ATTEMPTS, 'the first connection and MAX_ATTEMPTS retries');
  assert.equal(states.at(-1), OFFLINE, 'past the bound the channel reads offline');
  assert.ok(states.includes(RECONNECTING));
  assert.equal(timers.length, 0, 'nothing is left scheduled');
  // each wait is at least the lower half of its backoff: no storm
  for (let i = 1; i < opened.length; i++) {
    assert.ok(opened[i].at - opened[i - 1].at >= backoff(i - 1, 0), `retry ${i} waited`);
  }
});

test('a server that accepts and drops at once does not reset the backoff', () => {
  reset();
  const opened = fakeServer((ws) => {
    ws.fire('open');
    ws.fire('close');
  });
  const states = [];
  follow('/events', { state: (s) => states.push(s) });
  drain();
  assert.equal(opened.length, 1 + MAX_ATTEMPTS, 'an open with nothing said is no success');
  assert.equal(states.at(-1), OFFLINE);
});

test('a server that speaks the protocol before it drops starts the backoff over', () => {
  reset();
  const opened = fakeServer((ws) => {
    ws.fire('open');
    ws.fire('message', { data: JSON.stringify({ type: 'stream.ready', data: {} }) });
    ws.fire('close');
  });
  const states = [];
  const channel = follow('/events', { state: (s) => states.push(s) });
  drain(4 * MAX_ATTEMPTS);
  assert.ok(opened.length > 1 + MAX_ATTEMPTS, 'each ready connection earned another attempt');
  assert.notEqual(channel.state(), OFFLINE);
  channel.close();
  drain();
  assert.equal(channel.state(), OFFLINE);
});
