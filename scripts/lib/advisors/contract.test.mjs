// The advisor transport contract suite (ADR 0098): every adapter against fakes — no
// network, no model, no credential — so it runs in CI as it runs anywhere.
//
//   node --test scripts/lib/advisors/

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { EventEmitter } from 'node:events';
import path from 'node:path';
import {
  AdvisorFailure, buildPrompt, consultOne, httpFailure, normalise, scrub, STATUSES,
} from './contract.mjs';
import { consultSelected, loadAdapter } from './driver.mjs';

const HERE = path.dirname(new URL(import.meta.url).pathname);
const ROOT = path.resolve(HERE, '../../..');
const ANSWER = JSON.stringify({
  conclusion: 'keep the boundary at the service layer', stance: 'supports',
  assumptions: ['callers never retry'], risks: ['a retry storm'], actions: ['add a failure test'],
  falsifiers: ['a caller that retries'], confidence: 'high',
});

function response(status, body, headers = {}) {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: (k) => headers[k.toLowerCase()] },
    json: async () => (typeof body === 'string' ? JSON.parse(body) : body),
  };
}

function fakeFetch(routes) {
  const calls = [];
  const f = async (url, init = {}) => {
    calls.push({ url, init });
    for (const [pattern, handler] of routes) {
      if (url.includes(pattern)) return handler(url, init);
    }
    throw new TypeError('fetch failed');
  };
  f.calls = calls;
  return f;
}

function fakeSpawn({ stdout = '', stderr = '', code = 0, error, delayMs = 0 }) {
  return (exe, args) => {
    const child = new EventEmitter();
    child.stdout = new EventEmitter();
    child.stderr = new EventEmitter();
    child.pid = 999999;
    child.kill = () => {};
    child.args = args;
    setTimeout(() => {
      if (error) { child.emit('error', Object.assign(new Error(error), { code: error })); return; }
      if (stdout) child.stdout.emit('data', stdout);
      if (stderr) child.stderr.emit('data', stderr);
      child.emit('close', code);
    }, delayMs);
    return child;
  };
}

const api = { id: 'remote', adapter: 'openai-chat', model: 'm-1', credential: 'TEST_KEY', capabilities: ['independent_reasoning'] };

// ---------------------------------------------------------------- the contract itself

test('every adapter the catalogue names exists and exports exchange()', async () => {
  const yaml = readFileSync(path.join(ROOT, 'share/advisors.yaml'), 'utf8');
  const names = [...yaml.matchAll(/^\s+adapter:\s*([a-z0-9-]+)/gm)].map((m) => m[1]);
  assert.ok(names.length >= 5, `the catalogue names adapters (${names})`);
  for (const n of names) {
    const mod = await loadAdapter(n);
    assert.equal(typeof mod.exchange, 'function', n);
  }
});

test('no adapter module is orphaned from the catalogue', () => {
  const yaml = readFileSync(path.join(ROOT, 'share/advisors.yaml'), 'utf8');
  const names = new Set([...yaml.matchAll(/^\s+adapter:\s*([a-z0-9-]+)/gm)].map((m) => m[1]));
  const infrastructure = new Set(['contract', 'driver', 'process', 'http']);
  for (const f of readdirSync(HERE).filter((f) => f.endsWith('.mjs') && !f.endsWith('.test.mjs'))) {
    const stem = f.replace(/\.mjs$/, '');
    if (!infrastructure.has(stem)) assert.ok(names.has(stem), `${f} is an adapter no advisor names`);
  }
});

test('a structured answer is normalised and nothing raw is kept', () => {
  const n = normalise(`Sure! Here it is:\n${ANSWER}\nHope that helps.`);
  assert.equal(n.conclusion, 'keep the boundary at the service layer');
  assert.equal(n.stance, 'supports');
  assert.deepEqual(n.falsifiers, ['a caller that retries']);
  assert.equal(n.confidence, 'high');
  assert.equal(Object.keys(n).includes('raw'), false);
});

test('an empty answer and a malformed one are contained, typed', () => {
  assert.throws(() => normalise('  '), (e) => e.status === 'empty');
  assert.throws(() => normalise('I think so.'), (e) => e.status === 'malformed');
  assert.throws(() => normalise('{not json}'), (e) => e.status === 'malformed');
  assert.throws(() => normalise('{"stance":"supports"}'), (e) => e.status === 'malformed');
  // an unknown stance is not trusted
  assert.equal(normalise('{"conclusion":"x","stance":"vote A"}').stance, 'inconclusive');
});

test('HTTP failures are classified, and authentication is not retried as transient', () => {
  assert.equal(httpFailure(401).status, 'auth_failed');
  assert.equal(httpFailure(403).status, 'auth_failed');
  const limited = httpFailure(429, { get: () => '17' });
  assert.equal(limited.status, 'rate_limited');
  assert.equal(limited.retryAfterSeconds, 17);
  assert.equal(httpFailure(503).status, 'unavailable');
  assert.equal(httpFailure(418).status, 'error');
  assert.throws(() => new AdvisorFailure('completed', 'x'));
  assert.ok(STATUSES.includes('cancelled'));
});

test('secrets are scrubbed from questions and diagnostics', () => {
  const key = `sk-proj-${'a'.repeat(30)}`;
  const gh = `ghp_${'b'.repeat(30)}`;
  const out = scrub(`use ${key} and ${gh} with Bearer ${'c'.repeat(40)}`);
  assert.ok(!out.includes(key) && !out.includes(gh) && !out.includes('c'.repeat(40)), out);
  assert.equal(new AdvisorFailure('error', `failed with ${key}`).diagnostic.includes(key), false);
  const prompt = buildPrompt({ subject: `token ${gh} leaks`, facts: [key] });
  assert.ok(!prompt.includes(gh) && !prompt.includes(key));
});

test('the prompt marks the hypothesis and prior opinions as opinions, and asks for dissent', () => {
  const p = buildPrompt({
    subject: 's', hypothesis: 'A', facts: ['f1'],
    evidence: [{ kind: 'file', reference: 'src/x.rs:1' }], priorOpinions: ['another reviewer said B'],
  });
  assert.match(p, /hypothesis \(an opinion, not a fact\): A/);
  assert.match(p, /Prior external opinions .*opinions, not facts/);
  assert.match(p, /strongest argument against/);
  assert.match(p, /\[file\] src\/x\.rs:1/);
});

// ---------------------------------------------------------------- deadlines and cancellation

test('a timeout is a result, not an exception, and not a hang', async () => {
  const slow = { exchange: () => new Promise(() => {}) };
  const r = await consultOne(slow, api, 'p', { timeoutMs: 30 });
  assert.equal(r.status, 'timeout');
  assert.equal(r.conclusion, undefined, 'no answer, no conclusion');
});

test('cancellation is explicit and leaves no phantom completion', async () => {
  const controller = new AbortController();
  const slow = { exchange: () => new Promise(() => {}) };
  const pending = consultOne(slow, api, 'p', { timeoutMs: 5000, signal: controller.signal });
  controller.abort();
  const r = await pending;
  assert.equal(r.status, 'cancelled');
});

test('usage is reported when the transport reports it, and never invented', async () => {
  const withUsage = { exchange: async () => ({ text: ANSWER, usage: { input_tokens: 10, output_tokens: 5 } }) };
  const without = { exchange: async () => ({ text: ANSWER }) };
  assert.deepEqual((await consultOne(withUsage, api, 'p')).usage, { input_tokens: 10, output_tokens: 5 });
  assert.equal((await consultOne(without, api, 'p')).usage, undefined);
});

test('results come back in the plan order, whatever order they finish in', async () => {
  const dir = path.join(HERE, '..', '..', '..', 'test', 'fixtures', 'advisors');
  const plan = { selected: [{ advisor: 'slow' }, { advisor: 'fast' }] };
  const advisors = [
    { id: 'slow', adapter: 'fixture-slow' },
    { id: 'fast', adapter: 'fixture-fast' },
  ];
  const results = await consultSelected({ plan, advisors, prompt: 'p', adapterDir: dir, io: { timeoutMs: 2000 } });
  assert.deepEqual(results.map((r) => r.advisor), ['slow', 'fast']);
  assert.ok(results.every((r) => r.status === 'completed'));
});

test('an empty plan consults nobody, and an undeclared adapter is a typed failure', async () => {
  assert.deepEqual(await consultSelected({ plan: { selected: [] }, advisors: [], prompt: 'p' }), []);
  const r = await consultSelected({
    plan: { selected: [{ advisor: 'x' }] }, advisors: [{ id: 'x', adapter: 'no-such-adapter' }], prompt: 'p',
  });
  assert.equal(r[0].status, 'unavailable');
});

// ---------------------------------------------------------------- the adapters

test('api adapter: request normalisation, credential, usage', async () => {
  const { exchange } = await loadAdapter('openai-chat');
  const fetch = fakeFetch([['/chat/completions', () => response(200, {
    choices: [{ message: { content: ANSWER } }], usage: { prompt_tokens: 12, completion_tokens: 7 },
  })]]);
  const r = await consultOne({ exchange }, api, 'question', { fetch, env: { TEST_KEY: 'k' } });
  assert.equal(r.status, 'completed');
  assert.deepEqual(r.usage, { input_tokens: 12, output_tokens: 7 });
  const sent = JSON.parse(fetch.calls[0].init.body);
  assert.equal(sent.model, 'm-1', 'the model comes from the catalogue');
  assert.equal(fetch.calls[0].init.headers.authorization, 'Bearer k');
  assert.equal(JSON.stringify(r).includes('Bearer'), false, 'the credential reaches no result');
});

test('api adapter: absent credential, 401, 429 and an unreachable host are typed', async () => {
  const { exchange } = await loadAdapter('openai-chat');
  const cases = [
    [{}, fakeFetch([]), 'unavailable'],
    [{ TEST_KEY: 'k' }, fakeFetch([['/chat', () => response(401, {})]]), 'auth_failed'],
    [{ TEST_KEY: 'k' }, fakeFetch([['/chat', () => response(429, {}, { 'retry-after': '30' })]]), 'rate_limited'],
    [{ TEST_KEY: 'k' }, fakeFetch([]), 'unavailable'],
    [{ TEST_KEY: 'k' }, fakeFetch([['/chat', () => response(200, { choices: [] })]]), 'malformed'],
    [{ TEST_KEY: 'k' }, fakeFetch([['/chat', () => response(200, { choices: [{ message: { content: '' } }] })]]), 'empty'],
  ];
  for (const [env, fetch, want] of cases) {
    const r = await consultOne({ exchange }, api, 'q', { fetch, env });
    assert.equal(r.status, want, JSON.stringify(r));
  }
  const limited = await consultOne({ exchange }, api, 'q', {
    fetch: fakeFetch([['/chat', () => response(429, {}, { 'retry-after': '30' })]]), env: { TEST_KEY: 'k' },
  });
  assert.equal(limited.retry_after_seconds, 30);
});

test('local runtime adapters discover the served model and report usage', async () => {
  const ollama = await loadAdapter('ollama-chat');
  const f1 = fakeFetch([
    ['/api/tags', () => response(200, { models: [{ name: 'llama' }] })],
    ['/api/chat', () => response(200, { message: { content: ANSWER }, prompt_eval_count: 3, eval_count: 4 })],
  ]);
  const r1 = await consultOne(ollama, { id: 'o', adapter: 'ollama-chat' }, 'q', { fetch: f1, env: {} });
  assert.equal(r1.status, 'completed');
  assert.equal(JSON.parse(f1.calls[1].init.body).model, 'llama');
  const lms = await loadAdapter('lmstudio-chat');
  const f2 = fakeFetch([
    ['/models', () => response(200, { data: [{ id: 'qwen' }] })],
    ['/chat/completions', () => response(200, { choices: [{ message: { content: ANSWER } }] })],
  ]);
  assert.equal((await consultOne(lms, { id: 'l', adapter: 'lmstudio-chat' }, 'q', { fetch: f2, env: {} })).status, 'completed');
  // a runtime that is not running
  assert.equal((await consultOne(ollama, { id: 'o' }, 'q', { fetch: fakeFetch([]), env: {} })).status, 'unavailable');
  const empty = fakeFetch([['/api/tags', () => response(200, { models: [] })]]);
  assert.equal((await consultOne(ollama, { id: 'o' }, 'q', { fetch: empty, env: {} })).status, 'unavailable');
});

test('cli adapters: output, absent executable, authentication and rate limits', async () => {
  const gemini = await loadAdapter('gemini-cli');
  const codex = await loadAdapter('codex-exec');
  const cli = { id: 'c', executable: 'tool' };
  assert.equal((await consultOne(gemini, cli, 'q', { spawn: fakeSpawn({ stdout: ANSWER }) })).status, 'completed');
  assert.equal((await consultOne(codex, cli, 'q', { spawn: fakeSpawn({ stdout: ANSWER }) })).status, 'completed');
  assert.equal((await consultOne(gemini, cli, 'q', { spawn: fakeSpawn({ error: 'ENOENT' }) })).status, 'unavailable');
  assert.equal((await consultOne(gemini, cli, 'q', { spawn: fakeSpawn({ code: 1, stderr: 'Error: 401 Unauthorized' }) })).status, 'auth_failed');
  assert.equal((await consultOne(codex, cli, 'q', { spawn: fakeSpawn({ code: 1, stderr: 'rate limit exceeded' }) })).status, 'rate_limited');
  assert.equal((await consultOne(codex, cli, 'q', { spawn: fakeSpawn({ code: 2, stderr: 'boom' }) })).status, 'error');
  assert.equal((await consultOne(gemini, cli, 'q', { timeoutMs: 20, spawn: fakeSpawn({ stdout: ANSWER, delayMs: 500 }) })).status, 'timeout');
  // codex runs read-only
  let seen;
  const spy = (exe, args) => { seen = args; return fakeSpawn({ stdout: ANSWER })(exe, args); };
  await consultOne(codex, cli, 'q', { spawn: spy });
  assert.ok(seen.includes('read-only'));
});

test('peer adapter: a review request answered through the mesh', async () => {
  const peer = await loadAdapter('mesh-review');
  let polls = 0;
  const fetch = fakeFetch([
    ['/api/v1/mesh/reviews', () => response(200, { key: 'n1/r-1', event: 'e', lamport: 1 })],
    ['/api/v1/mesh/state', () => {
      polls += 1;
      const answers = polls > 1 ? [{ verdict: 'changes_requested', note: 'exit instead' }] : [];
      return response(200, { state: { reviews: [{ key: 'n1/r-1', answers }] } });
    }],
  ]);
  const r = await consultOne(peer, { id: 'peer:n1-r1' }, 'Subject: x', {
    fetch, env: { MAJORDOMUS_SERVER_URL: 'http://127.0.0.1:1', MAJORDOMUS_MESH_POLL_MS: '1' },
  });
  assert.equal(r.status, 'completed');
  assert.equal(r.stance, 'opposes');
  assert.equal(JSON.parse(fetch.calls[0].init.body).reviewer, 'n1-r1');
  assert.equal((await consultOne(peer, { id: 'peer:x' }, 'q', { env: {} })).status, 'unavailable');
});
