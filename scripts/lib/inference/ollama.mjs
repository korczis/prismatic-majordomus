// Runtime adapter: Ollama. Identified by its native `/api/version`; the inventory is
// `/api/tags`, what is loaded is `/api/ps`, and what each model can do is the
// `capabilities` list `/api/show` returns — advertised, until a probe says more.

import { AdvisorFailure } from '../advisors/contract.mjs';
import { call, PROBE_MS, PROBE_PROMPT } from './transport.mjs';

/** Ollama's capability words, in the catalogue's vocabulary. */
const WORDS = { completion: 'text', tools: 'tools', vision: 'vision', embedding: 'embeddings', thinking: 'reasoning' };
/** At most this many models are asked `/api/show`; the rest keep inferred evidence. */
const SHOW_LIMIT = 32;

export async function observe(endpoint, io = {}) {
  const v = await call(`${endpoint}/api/version`, {}, io);
  if (typeof v?.version !== 'string') {
    return { unidentified: 'GET /api/version did not answer with a version' };
  }
  const tags = await call(`${endpoint}/api/tags`, {}, io);
  const ps = await call(`${endpoint}/api/ps`, {}, io).catch(() => undefined);
  const running = new Map((ps?.models ?? []).map((m) => [m.name, m]));
  const listed = Array.isArray(tags?.models) ? tags.models : [];
  const models = [];
  for (const [i, m] of listed.entries()) {
    if (typeof m?.name !== 'string') continue;
    const d = m.details ?? {};
    const live = running.get(m.name);
    const model = {
      native_id: m.name,
      digest: typeof m.digest === 'string' ? `sha256:${m.digest.replace(/^sha256:/, '')}` : undefined,
      family: d.family,
      parameters: d.parameter_size,
      quantization: d.quantization_level,
      format: d.format,
      size_bytes: Number.isFinite(m.size) ? m.size : undefined,
      loaded: ps ? Boolean(live) : undefined,
      context_window: {},
      capabilities: [],
    };
    if (Number.isFinite(live?.context_length)) model.context_window.configured = live.context_length;
    const show = i < SHOW_LIMIT
      ? await call(`${endpoint}/api/show`, { method: 'POST', body: { model: m.name } }, io).catch(() => undefined)
      : undefined;
    if (Array.isArray(show?.capabilities)) {
      for (const word of show.capabilities) {
        if (WORDS[word]) model.capabilities.push({ word: WORDS[word], evidence: 'advertised', source: `POST /api/show capabilities: ${word}` });
      }
    } else {
      model.capabilities.push({ word: 'text', evidence: 'inferred', source: 'listed by GET /api/tags; /api/show reported no capabilities' });
    }
    const info = show?.model_info ?? {};
    const arch = info['general.architecture'];
    const ctx = arch ? info[`${arch}.context_length`] : undefined;
    if (Number.isFinite(ctx)) model.context_window.advertised = ctx;
    models.push(model);
  }
  return { version: v.version, identified_by: 'GET /api/version answered a version', models };
}

/** The words a probe may try on a model: the ones it advertises or is inferred to have. */
export function candidates(model) {
  return model.capabilities.map((c) => c.word).filter((w) => w === 'text' || w === 'embeddings');
}

export async function probe(endpoint, model, word, io = {}) {
  if (word === 'embeddings') {
    const j = await call(`${endpoint}/api/embed`, { method: 'POST', body: { model: model.native_id, input: 'probe' }, deadlineMs: PROBE_MS }, io);
    if (!(j?.embeddings?.[0]?.length > 0)) throw new AdvisorFailure('malformed', 'the answer carries no embedding');
    return;
  }
  const j = await call(`${endpoint}/api/chat`, {
    method: 'POST',
    body: { model: model.native_id, stream: false, options: { num_predict: 4 }, messages: [{ role: 'user', content: PROBE_PROMPT }] },
    deadlineMs: PROBE_MS,
  }, io);
  if (typeof j?.message?.content !== 'string') throw new AdvisorFailure('malformed', 'the answer carries no message');
}
