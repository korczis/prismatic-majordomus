// The local inference transport suite (ADR 0174): every runtime adapter against fakes of
// the responses its runtime documents — no network, no model — so it runs in CI as it
// runs anywhere.
//
//   node --test scripts/lib/inference/inference.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { isLoopback, normalise, plan } from './endpoint.mjs';
import { observeAll } from './observer.mjs';

function response(status, body) {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: () => undefined },
    json: async () => body,
  };
}

/** A fetch answering `METHOD path` routes on one origin; anything else is unreachable. */
function fakeFetch(routes) {
  const calls = [];
  const f = async (url, init = {}) => {
    const u = new URL(url);
    const key = `${init.method ?? 'GET'} ${u.pathname}`;
    calls.push({ origin: u.origin, key, body: init.body ? JSON.parse(init.body) : undefined });
    const handler = routes[`${u.origin} ${key}`];
    if (!handler) throw new TypeError('fetch failed');
    const [status, body] = typeof handler === 'function' ? handler(init) : handler;
    return response(status, body);
  };
  f.calls = calls;
  return f;
}

const OLLAMA = 'http://127.0.0.1:11434';
const LMSTUDIO = 'http://127.0.0.1:1234';
const LLAMA = 'http://127.0.0.1:8080';

const VENDORS = [
  { id: 'anthropic', inference: 'remote' },
  { id: 'ollama', inference: 'local', runtime: { adapter: 'ollama', endpoint: OLLAMA, endpoint_env: 'OLLAMA_HOST' } },
  { id: 'lmstudio', inference: 'local', runtime: { adapter: 'lmstudio', endpoint: LMSTUDIO, endpoint_env: 'LMSTUDIO_BASE_URL' } },
  { id: 'llamacpp', inference: 'local', runtime: { adapter: 'llamacpp', endpoint: LLAMA, endpoint_env: 'LLAMA_SERVER_URL' } },
];

const ollamaRoutes = {
  [`${OLLAMA} GET /api/version`]: [200, { version: '0.40.4' }],
  [`${OLLAMA} GET /api/tags`]: [200, { models: [
    { name: 'qwen2.5:14b', size: 9000000000, digest: 'abc123', details: { format: 'gguf', family: 'qwen2', parameter_size: '14.8B', quantization_level: 'Q4_K_M' } },
    { name: 'nomic-embed-text:latest', size: 274000000, digest: 'def456', details: { family: 'nomic-bert' } },
  ] }],
  [`${OLLAMA} GET /api/ps`]: [200, { models: [{ name: 'qwen2.5:14b', context_length: 4096 }] }],
  [`${OLLAMA} POST /api/show`]: (init) => {
    const { model } = JSON.parse(init.body);
    return model === 'qwen2.5:14b'
      ? [200, { capabilities: ['completion', 'tools'], model_info: { 'general.architecture': 'qwen2', 'qwen2.context_length': 32768 } }]
      : [200, { capabilities: ['embedding'] }];
  },
  [`${OLLAMA} POST /api/chat`]: [200, { message: { role: 'assistant', content: 'ok' } }],
  [`${OLLAMA} POST /api/embed`]: [200, { embeddings: [[0.1, 0.2]] }],
};

const lmstudioRoutes = {
  [`${LMSTUDIO} GET /api/v0/models`]: [200, { object: 'list', data: [
    { id: 'text-embedding-nomic-embed-text-v1.5', type: 'embeddings', arch: 'nomic-bert', quantization: 'Q4_K_M', compatibility_type: 'gguf', state: 'not-loaded', max_context_length: 2048 },
    { id: 'qwen2-vl-7b', type: 'vlm', arch: 'qwen2_vl', compatibility_type: 'mlx', state: 'loaded', max_context_length: 32768, loaded_context_length: 8192, capabilities: ['tool_use'] },
  ] }],
  [`${LMSTUDIO} POST /v1/chat/completions`]: [200, { choices: [{ message: { content: 'ok' } }] }],
  [`${LMSTUDIO} POST /v1/embeddings`]: [200, { data: [{ embedding: [0.3] }] }],
};

const llamaRoutes = {
  [`${LLAMA} GET /props`]: [200, { total_slots: 1, default_generation_settings: { n_ctx: 8192 }, build_info: 'b6500-abcdef', modalities: { vision: false } }],
  [`${LLAMA} GET /v1/models`]: [200, { data: [{ id: 'gemma-3-4b-it-Q4_K_M.gguf', meta: { n_ctx_train: 131072, n_params: 3880000000, size: 2490000000 } }] }],
  [`${LLAMA} POST /v1/chat/completions`]: [200, { choices: [{ message: { content: 'ok' } }] }],
  [`${LLAMA} POST /v1/embeddings`]: [501, { error: { message: 'This server does not support embeddings' } }],
};

const clock = () => 1760702400000;

test('endpoints normalise the way runtimes spell them, and only loopback is admitted by default', () => {
  assert.equal(normalise('0.0.0.0:11434'), 'http://127.0.0.1:11434');
  assert.equal(normalise('http://127.0.0.1:1234/v1'), 'http://127.0.0.1:1234');
  assert.equal(normalise('localhost'), 'http://localhost');
  assert.throws(() => normalise('http://u:p@127.0.0.1:1'), /credential/);
  assert.throws(() => normalise('file:///etc/passwd'), /not an HTTP endpoint/);
  assert.ok(isLoopback('http://127.0.0.1:8080'));
  assert.ok(isLoopback('http://[::1]:8080'));
  assert.ok(!isLoopback('http://192.168.1.10:11434'));

  const p = plan(VENDORS, { env: { OLLAMA_HOST: '192.168.1.10:11434', LMSTUDIO_BASE_URL: 'http://127.0.0.1:1234/v1' } });
  assert.deepEqual(p.admitted.map((a) => [a.vendor, a.endpoint, a.source]), [
    ['lmstudio', LMSTUDIO, '$LMSTUDIO_BASE_URL'],
    ['llamacpp', LLAMA, 'catalogue'],
  ]);
  assert.equal(p.refused[0].vendor, 'ollama');
  assert.match(p.refused[0].reason, /--allow-remote/);
  const remote = plan(VENDORS, { env: { OLLAMA_HOST: '192.168.1.10:11434' }, allowRemote: true });
  assert.equal(remote.admitted[0].endpoint, 'http://192.168.1.10:11434');
});

test('an extra endpoint must name a declared local runtime, and a duplicate is observed once', () => {
  const p = plan(VENDORS, { extra: [
    { vendor: 'anthropic', endpoint: 'http://127.0.0.1:9' },
    { vendor: 'ollama', endpoint: OLLAMA },
    { vendor: 'llamacpp', endpoint: 'http://127.0.0.1:8081' },
  ] });
  assert.equal(p.admitted.filter((a) => a.vendor === 'ollama').length, 1);
  assert.ok(p.admitted.some((a) => a.endpoint === 'http://127.0.0.1:8081'));
  assert.match(p.refused.find((r) => r.vendor === 'anthropic').reason, /not a local runtime/);
});

test('each runtime is identified by its own signature and its inventory is typed', async () => {
  const fetch = fakeFetch({ ...ollamaRoutes, ...lmstudioRoutes, ...llamaRoutes });
  const { admitted } = plan(VENDORS);
  const o = await observeAll({ admitted, io: { fetch }, now: clock });
  assert.equal(o.version, 1);
  assert.equal(o.probe, 'none');
  assert.deepEqual(o.runtimes.map((r) => [r.vendor, r.state]), [
    ['ollama', 'identified'], ['lmstudio', 'identified'], ['llamacpp', 'identified'],
  ]);

  const [ollama, lmstudio, llama] = o.runtimes;
  assert.equal(ollama.version, '0.40.4');
  assert.deepEqual(ollama.models.map((m) => m.native_id), ['nomic-embed-text:latest', 'qwen2.5:14b'], 'sorted');
  const qwen = ollama.models[1];
  assert.equal(qwen.digest, 'sha256:abc123');
  assert.equal(qwen.quantization, 'Q4_K_M');
  assert.equal(qwen.loaded, true);
  assert.deepEqual(qwen.context_window, { advertised: 32768, configured: 4096 });
  assert.deepEqual(qwen.capabilities.map((c) => `${c.word}:${c.evidence}`), ['text:advertised', 'tools:advertised']);
  assert.deepEqual(ollama.models[0].capabilities.map((c) => c.word), ['embeddings']);
  assert.equal(ollama.models[0].loaded, false);

  assert.equal(lmstudio.version, undefined, 'LM Studio reports no version; none is invented');
  const vl = lmstudio.models.find((m) => m.native_id === 'qwen2-vl-7b');
  assert.deepEqual(vl.capabilities.map((c) => c.word), ['text', 'vision', 'tools']);
  assert.deepEqual(vl.context_window, { advertised: 32768, configured: 8192 });

  assert.equal(llama.version, 'b6500-abcdef');
  assert.deepEqual(llama.models[0].capabilities, [
    { word: 'text', evidence: 'inferred', source: 'served by llama-server; GET /props reports no task' },
  ]);
  assert.ok(!fetch.calls.some((c) => c.key.startsWith('POST /api/chat') || c.key.includes('completions')),
    'passive discovery generates nothing');
});

test('a compatible server is not an identity, and silence is unreachable', async () => {
  const fetch = fakeFetch({
    [`${LMSTUDIO} GET /api/v0/models`]: [404, {}],
    [`${LMSTUDIO} GET /v1/models`]: [200, { data: [{ id: 'x' }] }],
    [`${OLLAMA} GET /api/version`]: [200, { hello: 'world' }],
  });
  const { admitted } = plan(VENDORS);
  const o = await observeAll({ admitted, io: { fetch }, now: clock });
  const by = Object.fromEntries(o.runtimes.map((r) => [r.vendor, r]));
  assert.equal(by.lmstudio.state, 'unidentified');
  assert.match(by.lmstudio.diagnostic, /OpenAI-compatible/);
  assert.equal(by.ollama.state, 'unidentified');
  assert.equal(by.llamacpp.state, 'unreachable');
  assert.match(by.llamacpp.diagnostic, /^unavailable: /);
  for (const r of o.runtimes) assert.deepEqual(r.models, [], 'no models without an identity');
});

test('a runtime that does not answer in time is a timeout, not a hang', async () => {
  const fetch = (url, init) => new Promise((_, reject) => {
    init.signal.addEventListener('abort', () => reject(new Error('aborted')));
  });
  const admitted = [{ vendor: 'ollama', adapter: 'ollama', endpoint: OLLAMA }];
  const started = Date.now();
  const o = await observeAll({ admitted, io: { fetch } });
  assert.equal(o.runtimes[0].state, 'unreachable');
  assert.match(o.runtimes[0].diagnostic, /^timeout: /);
  assert.ok(Date.now() - started < 5000);
});

test('probing loaded models proves or refutes words, and leaves unloaded ones alone', async () => {
  const fetch = fakeFetch({ ...ollamaRoutes, ...llamaRoutes });
  const admitted = plan(VENDORS).admitted.filter((a) => a.vendor !== 'lmstudio');
  const o = await observeAll({ admitted, probe: true, io: { fetch }, now: clock });
  assert.equal(o.probe, 'loaded');
  const qwen = o.runtimes[0].models.find((m) => m.native_id === 'qwen2.5:14b');
  assert.deepEqual(qwen.probes.map((p) => [p.word, p.outcome]), [['text', 'passed']]);
  const nomic = o.runtimes[0].models.find((m) => m.native_id === 'nomic-embed-text:latest');
  assert.equal(nomic.probes, undefined, 'an unloaded model is not loaded by a probe');
  const llama = o.runtimes[1].models[0];
  assert.deepEqual(llama.probes.map((p) => [p.word, p.outcome]), [['text', 'passed'], ['embeddings', 'failed']]);
  const chat = fetch.calls.find((c) => c.key === 'POST /api/chat');
  assert.equal(chat.body.options.num_predict, 4, 'a probe asks for a handful of tokens');

  const all = await observeAll({ admitted, probe: 'all', io: { fetch }, now: clock });
  const nomicAll = all.runtimes[0].models.find((m) => m.native_id === 'nomic-embed-text:latest');
  assert.deepEqual(nomicAll.probes.map((p) => [p.word, p.outcome]), [['embeddings', 'passed']]);
});

test('the document does not depend on which runtime answered first', async () => {
  const slow = (routes, ms) => Object.fromEntries(Object.entries(routes).map(([k, v]) => [k, (init) => {
    const start = Date.now();
    while (Date.now() - start < ms) { /* a busy runtime */ }
    return typeof v === 'function' ? v(init) : v;
  }]));
  const { admitted } = plan(VENDORS);
  const a = await observeAll({ admitted, io: { fetch: fakeFetch({ ...slow(ollamaRoutes, 5), ...lmstudioRoutes, ...llamaRoutes }) }, now: clock });
  const b = await observeAll({ admitted, io: { fetch: fakeFetch({ ...ollamaRoutes, ...lmstudioRoutes, ...slow(llamaRoutes, 5) }) }, now: clock });
  const strip = (o) => JSON.stringify(o, (k, v) => (k === 'duration_ms' ? undefined : v));
  assert.equal(strip(a), strip(b));
});

test('a weights path is not a model id, and a runtime failure names the runtime', async () => {
  const fetch = fakeFetch({
    ...llamaRoutes,
    [`${LLAMA} GET /v1/models`]: [200, { data: [{ id: '/Users/someone/models/tiny.gguf', meta: {} }] }],
    [`${LLAMA} POST /v1/chat/completions`]: [500, {}],
  });
  const admitted = [{ vendor: 'llamacpp', adapter: 'llamacpp', endpoint: LLAMA }];
  const o = await observeAll({ admitted, probe: true, io: { fetch }, now: clock });
  const model = o.runtimes[0].models[0];
  assert.equal(model.native_id, 'tiny.gguf');
  assert.ok(!JSON.stringify(o).includes('/Users/someone'), 'no home directory in the document');
  assert.equal(model.probes[0].diagnostic, 'unavailable: the runtime failed (HTTP 500)');
});
